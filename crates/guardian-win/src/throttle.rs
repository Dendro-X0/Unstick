//! Soft-throttle + Disk Lock + Critical Guard suspend via NtSuspendProcess.

use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use guardian_core::{
    build_persist_file, clear_suspend_ledger, ledger_is_stale, load_suspend_ledger,
    save_suspend_ledger, PersistedSuspendEntry, PlannedAction, ThrottleLevel,
};
use tracing::{debug, info, warn};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectCpuRateControlInformation,
    SetInformationJobObject, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
    JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
};
#[cfg(windows)]
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, SetPriorityClass, SetProcessPriorityBoost, BELOW_NORMAL_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_DUP_HANDLE, PROCESS_QUERY_INFORMATION,
    PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA, PROCESS_SUSPEND_RESUME, PROCESS_TERMINATE,
};

#[derive(Debug, Clone)]
pub struct SuspendEntry {
    pub pid: u32,
    pub name: String,
    pub reason: String,
    pub since: Instant,
    pub suspended_at_unix: i64,
}

pub struct SuspendLedger {
    entries: HashMap<u32, SuspendEntry>,
    max_secs: u64,
}

impl SuspendLedger {
    pub fn new(max_secs: u64) -> Self {
        Self {
            entries: HashMap::new(),
            max_secs: max_secs.max(5),
        }
    }

    pub fn set_max_secs(&mut self, secs: u64) {
        self.max_secs = secs.max(5);
    }

    pub fn contains(&self, pid: u32) -> bool {
        self.entries.contains_key(&pid)
    }

    pub fn list(&self) -> Vec<SuspendEntry> {
        self.entries.values().cloned().collect()
    }

    pub fn expired_pids(&self) -> Vec<u32> {
        self.entries
            .iter()
            .filter(|(_, e)| e.since.elapsed().as_secs() >= self.max_secs)
            .map(|(p, _)| *p)
            .collect()
    }

    pub fn insert(&mut self, pid: u32, name: String, reason: String) {
        self.entries.entry(pid).or_insert(SuspendEntry {
            pid,
            name,
            reason,
            since: Instant::now(),
            suspended_at_unix: Utc::now().timestamp(),
        });
    }

    pub fn remove(&mut self, pid: u32) -> Option<SuspendEntry> {
        self.entries.remove(&pid)
    }

    pub fn clear_stale(&mut self, live: &[u32]) {
        let live: std::collections::HashSet<u32> = live.iter().copied().collect();
        self.entries.retain(|p, _| live.contains(p));
    }
}

#[derive(Debug, Default)]
pub struct ApplyOutcome {
    pub applied: Vec<(u32, ThrottleLevel, String)>,
    pub denied: Vec<(u32, String, String)>,
}

pub struct ThrottleExecutor {
    #[cfg(windows)]
    jobs: HashMap<u32, usize>,
    job_cpu_rate_percent: u32,
    applied: HashMap<u32, ThrottleLevel>,
    pub ledger: SuspendLedger,
    service_pid: u32,
    #[cfg(windows)]
    nt_suspend: Option<NtFns>,
}

#[cfg(windows)]
struct NtFns {
    suspend: unsafe extern "system" fn(HANDLE) -> i32,
    resume: unsafe extern "system" fn(HANDLE) -> i32,
    set_info: unsafe extern "system" fn(HANDLE, u32, *mut std::ffi::c_void, u32) -> i32,
}

impl ThrottleExecutor {
    pub fn new(job_cpu_rate_percent: u32, max_suspend_secs: u64) -> Self {
        #[cfg(windows)]
        probe_io_rate_unsupported();

        Self {
            #[cfg(windows)]
            jobs: HashMap::new(),
            job_cpu_rate_percent: job_cpu_rate_percent.clamp(10, 100),
            applied: HashMap::new(),
            ledger: SuspendLedger::new(max_suspend_secs),
            service_pid: std::process::id(),
            #[cfg(windows)]
            nt_suspend: load_nt_fns(),
        }
    }

    pub fn set_job_cpu_rate(&mut self, percent: u32) {
        self.job_cpu_rate_percent = percent.clamp(10, 100);
    }

    /// P0: resume processes left suspended by a previous crashed service.
    pub fn recover_orphans_from_disk(&mut self) -> Vec<(u32, String, String)> {
        let file = load_suspend_ledger();
        if file.entries.is_empty() {
            return Vec::new();
        }
        let stale = ledger_is_stale(&file, self.service_pid, self.ledger.max_secs as i64);
        info!(
            count = file.entries.len(),
            stale,
            prev_service = file.service_pid,
            "P0 recovering persisted suspends"
        );
        let mut out = Vec::new();
        for e in &file.entries {
            match self.resume_one(e.pid) {
                Ok(()) => {
                    out.push((e.pid, e.name.clone(), "startup_recovery".to_string()));
                    info!(pid = e.pid, name = %e.name, "resumed orphan after crash/restart");
                }
                Err(err) => {
                    warn!(pid = e.pid, name = %e.name, error = %err, "orphan resume failed");
                }
            }
        }
        clear_suspend_ledger();
        out
    }

    pub fn apply(&mut self, actions: &[PlannedAction]) -> ApplyOutcome {
        let mut out = ApplyOutcome::default();
        for action in actions {
            match self.apply_one(action) {
                Ok(()) => {
                    self.applied.insert(action.pid, action.level);
                    if action.level == ThrottleLevel::Suspend {
                        self.ledger.insert(
                            action.pid,
                            action.name.clone(),
                            action.reason.clone(),
                        );
                    }
                    out.applied
                        .push((action.pid, action.level, action.reason.clone()));
                }
                Err(e) => {
                    warn!(pid = action.pid, error = %e, "throttle apply failed");
                    out.denied
                        .push((action.pid, action.name.clone(), e.to_string()));
                }
            }
        }
        self.persist_ledger();
        out
    }

    pub fn resume_pids(&mut self, pids: &[u32], reason: &str) -> Vec<(u32, String, String)> {
        let mut out = Vec::new();
        for &pid in pids {
            if let Some(entry) = self.ledger.remove(pid) {
                if let Err(e) = self.resume_one(pid) {
                    warn!(pid, error = %e, "resume failed");
                } else {
                    out.push((pid, entry.name, reason.to_string()));
                }
                self.applied.remove(&pid);
            }
        }
        self.persist_ledger();
        out
    }

    pub fn resume_all_suspended(&mut self, reason: &str) -> Vec<(u32, String, String)> {
        let pids: Vec<u32> = self.ledger.entries.keys().copied().collect();
        self.resume_pids(&pids, reason)
    }

    pub fn restore_all(&mut self) {
        let _ = self.resume_all_suspended("restore_all");
        let pids: Vec<u32> = self.applied.keys().copied().collect();
        for pid in pids {
            let _ = self.restore_pid(pid);
        }
        self.applied.clear();
        self.persist_ledger();
    }

    pub fn restore_missing(&mut self, live_pids: &[u32]) {
        self.ledger.clear_stale(live_pids);
        let live: std::collections::HashSet<u32> = live_pids.iter().copied().collect();
        let stale: Vec<u32> = self
            .applied
            .keys()
            .copied()
            .filter(|p| !live.contains(p))
            .collect();
        for pid in stale {
            self.applied.remove(&pid);
            #[cfg(windows)]
            {
                if let Some(h) = self.jobs.remove(&pid) {
                    unsafe {
                        let _ = CloseHandle(HANDLE(h as *mut _));
                    }
                }
            }
        }
        self.persist_ledger();
    }

    fn persist_ledger(&self) {
        let entries: Vec<PersistedSuspendEntry> = self
            .ledger
            .list()
            .into_iter()
            .map(|e| PersistedSuspendEntry {
                pid: e.pid,
                name: e.name,
                reason: e.reason,
                suspended_at_unix: e.suspended_at_unix,
            })
            .collect();
        if entries.is_empty() {
            clear_suspend_ledger();
        } else if let Err(e) = save_suspend_ledger(&build_persist_file(self.service_pid, entries))
        {
            warn!(error = %e, "failed to persist suspend ledger");
        }
    }

    #[cfg(windows)]
    fn apply_one(&mut self, action: &PlannedAction) -> anyhow::Result<()> {
        unsafe {
            let access = PROCESS_SET_INFORMATION
                | PROCESS_QUERY_INFORMATION
                | PROCESS_SET_QUOTA
                | PROCESS_TERMINATE
                | PROCESS_DUP_HANDLE
                | PROCESS_SUSPEND_RESUME;
            let handle = OpenProcess(access, false, action.pid).map_err(|e| {
                anyhow::anyhow!("OpenProcess: {e}")
            })?;

            let class = match action.level {
                ThrottleLevel::None => NORMAL_PRIORITY_CLASS,
                ThrottleLevel::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
                ThrottleLevel::Idle | ThrottleLevel::Suspend => IDLE_PRIORITY_CLASS,
            };
            SetPriorityClass(handle, class)?;
            let _ = SetProcessPriorityBoost(handle, true);

            if action.apply_disk_lock {
                self.set_io_priority(handle, 0);
                let _ = EmptyWorkingSet(handle);
            } else if matches!(
                action.level,
                ThrottleLevel::BelowNormal | ThrottleLevel::Idle | ThrottleLevel::Suspend
            ) {
                self.set_io_priority(handle, 1);
            }

            if action.apply_job_cap {
                self.ensure_job_cap(action.pid, handle)?;
            }

            if action.level == ThrottleLevel::Suspend {
                if let Some(nt) = &self.nt_suspend {
                    let status = (nt.suspend)(handle);
                    if status < 0 {
                        anyhow::bail!("NtSuspendProcess status {status:#x}");
                    }
                    debug!(pid = action.pid, "NtSuspendProcess ok");
                } else {
                    warn!(pid = action.pid, "NtSuspendProcess unavailable; IDLE only");
                }
            }

            let _ = CloseHandle(handle);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn apply_one(&mut self, action: &PlannedAction) -> anyhow::Result<()> {
        debug!(pid = action.pid, level = ?action.level, "throttle stub (non-windows)");
        if action.level == ThrottleLevel::Suspend {
            self.ledger
                .insert(action.pid, action.name.clone(), action.reason.clone());
        }
        Ok(())
    }

    #[cfg(windows)]
    fn resume_one(&mut self, pid: u32) -> anyhow::Result<()> {
        unsafe {
            let handle = OpenProcess(
                PROCESS_SUSPEND_RESUME | PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION,
                false,
                pid,
            )?;
            if let Some(nt) = &self.nt_suspend {
                let status = (nt.resume)(handle);
                if status < 0 {
                    anyhow::bail!("NtResumeProcess status {status:#x}");
                }
            }
            SetPriorityClass(handle, NORMAL_PRIORITY_CLASS)?;
            self.set_io_priority(handle, 2);
            let _ = CloseHandle(handle);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn resume_one(&mut self, _pid: u32) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn set_io_priority(&self, handle: HANDLE, priority: u32) {
        if let Some(nt) = &self.nt_suspend {
            let mut prio = priority;
            unsafe {
                let _ = (nt.set_info)(
                    handle,
                    33,
                    &mut prio as *mut _ as *mut std::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
    }

    #[cfg(windows)]
    fn ensure_job_cap(&mut self, pid: u32, process: HANDLE) -> anyhow::Result<()> {
        if self.jobs.contains_key(&pid) {
            return Ok(());
        }
        unsafe {
            let job = CreateJobObjectW(None, None)?;
            let mut info = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                    | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                Anonymous: Default::default(),
            };
            info.Anonymous.CpuRate = self.job_cpu_rate_percent * 100;
            SetInformationJobObject(
                job,
                JobObjectCpuRateControlInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            )?;
            AssignProcessToJobObject(job, process)?;
            self.jobs.insert(pid, job.0 as usize);
        }
        Ok(())
    }

    #[cfg(windows)]
    fn restore_pid(&mut self, pid: u32) -> anyhow::Result<()> {
        if self.ledger.contains(pid) {
            let _ = self.resume_one(pid);
            self.ledger.remove(pid);
        }
        unsafe {
            let handle =
                OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, false, pid)?;
            SetPriorityClass(handle, NORMAL_PRIORITY_CLASS)?;
            self.set_io_priority(handle, 2);
            let _ = CloseHandle(handle);
            if let Some(h) = self.jobs.remove(&pid) {
                let _ = CloseHandle(HANDLE(h as *mut _));
            }
        }
        self.applied.remove(&pid);
        Ok(())
    }

    #[cfg(not(windows))]
    fn restore_pid(&mut self, pid: u32) -> anyhow::Result<()> {
        self.ledger.remove(pid);
        self.applied.remove(&pid);
        Ok(())
    }
}

pub fn elevation_likely(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("access is denied")
        || e.contains("access_denied")
        || e.contains("os error 5")
        || e.contains("(os error 5)")
        || e.contains("privilege")
}

#[cfg(windows)]
fn probe_io_rate_unsupported() {
    use windows::Win32::System::JobObjects::{
        CreateJobObjectW, SetIoRateControlInformationJobObject,
        JOBOBJECT_IO_RATE_CONTROL_INFORMATION, JOB_OBJECT_IO_RATE_CONTROL_ENABLE,
    };
    unsafe {
        if let Ok(job) = CreateJobObjectW(None, None) {
            let info = JOBOBJECT_IO_RATE_CONTROL_INFORMATION {
                MaxIops: 100,
                MaxBandwidth: 0,
                ReservationIops: 0,
                VolumeName: windows::core::PCWSTR::null(),
                BaseIoSize: 0,
                ControlFlags: JOB_OBJECT_IO_RATE_CONTROL_ENABLE.0 as u32,
            };
            let ok = SetIoRateControlInformationJobObject(job, &info);
            if ok == 0 {
                info!("Job Object I/O rate control unsupported on this OS; Disk Lock uses VeryLow I/O + EmptyWorkingSet + suspend");
            } else {
                info!("Job Object I/O rate control unexpectedly available");
            }
            let _ = CloseHandle(job);
        }
    }
}

#[cfg(windows)]
fn load_nt_fns() -> Option<NtFns> {
    use windows::core::s;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
    unsafe {
        let ntdll = LoadLibraryA(s!("ntdll.dll")).ok()?;
        let suspend = GetProcAddress(ntdll, s!("NtSuspendProcess"))?;
        let resume = GetProcAddress(ntdll, s!("NtResumeProcess"))?;
        let set_info = GetProcAddress(ntdll, s!("NtSetInformationProcess"))?;
        Some(NtFns {
            suspend: std::mem::transmute(suspend),
            resume: std::mem::transmute(resume),
            set_info: std::mem::transmute(set_info),
        })
    }
}

impl Drop for ThrottleExecutor {
    fn drop(&mut self) {
        let _ = self.resume_all_suspended("drop");
        clear_suspend_ledger();
        #[cfg(windows)]
        {
            for (_, h) in self.jobs.drain() {
                unsafe {
                    let _ = CloseHandle(HANDLE(h as *mut _));
                }
            }
        }
    }
}
