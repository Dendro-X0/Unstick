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
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, SetPriorityClass, SetProcessInformation, SetProcessPriorityBoost,
    SetProcessWorkingSetSize, ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS, MEMORY_PRIORITY_INFORMATION, MEMORY_PRIORITY_LOW, MEMORY_PRIORITY_NORMAL,
    NORMAL_PRIORITY_CLASS, PROCESS_DUP_HANDLE, PROCESS_POWER_THROTTLING_STATE,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_QUERY_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA, PROCESS_SUSPEND_RESUME,
    PROCESS_TERMINATE, ProcessMemoryPriority, ProcessPowerThrottling,
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

    pub fn get(&self, pid: u32) -> Option<&SuspendEntry> {
        self.entries.get(&pid)
    }

    pub fn max_secs(&self) -> u64 {
        self.max_secs
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

/// Soft demotion state so EcoQoS / mem-priority can be restored when a PID leaves the plan.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct AppliedState {
    level: ThrottleLevel,
    ecoqos: bool,
    mem_prio: bool,
    since: Instant,
}

pub struct ThrottleExecutor {
    #[cfg(windows)]
    jobs: HashMap<u32, usize>,
    job_cpu_rate_percent: u32,
    applied: HashMap<u32, AppliedState>,
    /// Soft demotion max age before forced restore (even if still in plan).
    max_soft_demote_secs: u64,
    /// Currently AboveNormal-boosted foreground PID.
    boosted_pid: Option<u32>,
    pub ledger: SuspendLedger,
    /// After a successful resume, refuse NtSuspend for this PID until Instant.
    suspend_cooldown_until: HashMap<u32, Instant>,
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
            max_soft_demote_secs: 45,
            boosted_pid: None,
            ledger: SuspendLedger::new(max_suspend_secs),
            suspend_cooldown_until: HashMap::new(),
            service_pid: std::process::id(),
            #[cfg(windows)]
            nt_suspend: load_nt_fns(),
        }
    }

    pub fn set_max_soft_demote_secs(&mut self, secs: u64) {
        self.max_soft_demote_secs = secs.max(5);
    }

    pub fn max_soft_demote_secs(&self) -> u64 {
        self.max_soft_demote_secs
    }

    fn arm_suspend_cooldown(&mut self, pid: u32) {
        let secs = self.ledger.max_secs().max(30);
        self.suspend_cooldown_until
            .insert(pid, Instant::now() + std::time::Duration::from_secs(secs));
    }

    fn suspend_on_cooldown(&mut self, pid: u32) -> bool {
        match self.suspend_cooldown_until.get(&pid).copied() {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                self.suspend_cooldown_until.remove(&pid);
                false
            }
            None => false,
        }
    }

    pub fn set_job_cpu_rate(&mut self, percent: u32) {
        self.job_cpu_rate_percent = percent.clamp(10, 100);
    }

    /// Raise focused process to AboveNormal (and allow dynamic priority boosts).
    pub fn boost_foreground(&mut self, pid: Option<u32>) {
        match pid {
            Some(pid) if self.boosted_pid == Some(pid) => {}
            Some(pid) => {
                self.clear_boost();
                if self.apply_boost(pid).is_ok() {
                    self.boosted_pid = Some(pid);
                    debug!(pid, "boosted foreground AboveNormal");
                }
            }
            None => self.clear_boost(),
        }
    }

    pub fn clear_boost(&mut self) {
        if let Some(pid) = self.boosted_pid.take() {
            let _ = self.restore_boost(pid);
            debug!(pid, "restored foreground boost to Normal");
        }
    }

    #[cfg(windows)]
    fn apply_boost(&self, pid: u32) -> anyhow::Result<()> {
        unsafe {
            let handle =
                OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, false, pid)
                    .map_err(|e| anyhow::anyhow!("OpenProcess boost: {e}"))?;
            SetPriorityClass(handle, ABOVE_NORMAL_PRIORITY_CLASS)?;
            // false = do not disable priority boosts
            let _ = SetProcessPriorityBoost(handle, false);
            let _ = CloseHandle(handle);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn apply_boost(&self, _pid: u32) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(windows)]
    fn restore_boost(&self, pid: u32) -> anyhow::Result<()> {
        unsafe {
            let handle =
                OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, false, pid)?;
            SetPriorityClass(handle, NORMAL_PRIORITY_CLASS)?;
            let _ = CloseHandle(handle);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn restore_boost(&self, _pid: u32) -> anyhow::Result<()> {
        Ok(())
    }

    /// P0: resume processes left suspended by a previous crashed service.
    pub fn recover_orphans_from_disk(&mut self) -> Vec<(u32, String, String)> {
        let file = load_suspend_ledger();
        if file.entries.is_empty() {
            return Vec::new();
        }
        let stale = ledger_is_stale(&file, self.service_pid, self.ledger.max_secs() as i64);
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
        for (pid, _, _) in &out {
            self.arm_suspend_cooldown(*pid);
        }
        out
    }

    pub fn apply(&mut self, actions: &[PlannedAction]) -> ApplyOutcome {
        let mut out = ApplyOutcome::default();
        for action in actions {
            // Never re-NtSuspend a PID we just released (max_suspend / recovery).
            if action.level == ThrottleLevel::Suspend && self.suspend_on_cooldown(action.pid) {
                debug!(
                    pid = action.pid,
                    "skip Suspend — post-resume cooldown active"
                );
                continue;
            }
            match self.apply_one(action) {
                Ok(()) => {
                    let since = self
                        .applied
                        .get(&action.pid)
                        .map(|s| s.since)
                        .unwrap_or_else(Instant::now);
                    self.applied.insert(
                        action.pid,
                        AppliedState {
                            level: action.level,
                            ecoqos: action.apply_ecoqos,
                            mem_prio: action.apply_mem_priority_low,
                            since,
                        },
                    );
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
            let Some(entry) = self.ledger.get(pid).cloned() else {
                continue;
            };
            match self.resume_one(pid) {
                Ok(()) => {
                    self.ledger.remove(pid);
                    self.applied.remove(&pid);
                    self.arm_suspend_cooldown(pid);
                    out.push((pid, entry.name, reason.to_string()));
                }
                Err(e) => {
                    // Keep ledger entry so the next tick retries NtResumeProcess.
                    warn!(pid, error = %e, "resume failed — will retry");
                }
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
        self.clear_boost();
        let _ = self.resume_all_suspended("restore_all");
        let pids: Vec<u32> = self.applied.keys().copied().collect();
        for pid in pids {
            let _ = self.restore_pid(pid);
        }
        self.applied.clear();
        self.persist_ledger();
    }

    /// Restore soft demotions (EcoQoS / mem-prio / priority) for PIDs no longer in the plan.
    /// Suspended PIDs stay on the suspend ledger until explicit resume.
    pub fn restore_not_in_plan(&mut self, plan_pids: &std::collections::HashSet<u32>) {
        let stale: Vec<u32> = self
            .applied
            .keys()
            .copied()
            .filter(|p| !plan_pids.contains(p) && !self.ledger.contains(*p))
            .collect();
        for pid in stale {
            match self.restore_pid(pid) {
                Ok(()) => debug!(pid, "restored soft demotion (left plan)"),
                Err(e) => warn!(pid, error = %e, "soft restore failed — will retry"),
            }
        }
    }

    /// Force-restore soft demotions that exceeded TTL even if still in the plan.
    /// Creates recovery windows so demotions cannot linger indefinitely.
    pub fn expire_soft_demotions(&mut self) -> Vec<(u32, String)> {
        let max = self.max_soft_demote_secs;
        let expired: Vec<u32> = self
            .applied
            .iter()
            .filter(|(pid, state)| {
                !self.ledger.contains(**pid) && state.since.elapsed().as_secs() >= max
            })
            .map(|(pid, _)| *pid)
            .collect();
        let mut out = Vec::new();
        for pid in expired {
            match self.restore_pid(pid) {
                Ok(()) => {
                    debug!(pid, max_secs = max, "soft demotion TTL expired — restored");
                    out.push((pid, "soft_demote_ttl".to_string()));
                }
                Err(e) => {
                    // Drop tracking even if OpenProcess fails (exited PID) so demotion
                    // state cannot linger indefinitely in the executor.
                    self.applied.remove(&pid);
                    warn!(pid, error = %e, "soft TTL restore failed — cleared tracking");
                    out.push((pid, "soft_demote_ttl".to_string()));
                }
            }
        }
        out
    }

    /// Test helper: how many soft demotions are tracked.
    pub fn soft_applied_count(&self) -> usize {
        self.applied
            .keys()
            .filter(|p| !self.ledger.contains(**p))
            .count()
    }

    /// Test helper: seed a soft demotion without calling Windows APIs.
    #[cfg(test)]
    pub fn seed_soft_demotion_for_test(&mut self, pid: u32) {
        self.applied.insert(
            pid,
            AppliedState {
                level: ThrottleLevel::BelowNormal,
                ecoqos: true,
                mem_prio: true,
                since: Instant::now(),
            },
        );
    }

    /// Test helper: mark a soft demotion as aged past TTL.
    #[cfg(test)]
    pub fn force_soft_demote_age_for_test(&mut self, pid: u32, secs_ago: u64) {
        if let Some(state) = self.applied.get_mut(&pid) {
            state.since = Instant::now()
                .checked_sub(std::time::Duration::from_secs(secs_ago))
                .unwrap_or_else(Instant::now);
        }
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

            if action.apply_ecoqos {
                set_ecoqos(handle, true);
            }
            if action.apply_mem_priority_low {
                set_memory_priority(handle, true);
            }

            if action.apply_disk_lock {
                self.set_io_priority(handle, 0);
            } else if matches!(
                action.level,
                ThrottleLevel::BelowNormal | ThrottleLevel::Idle | ThrottleLevel::Suspend
            ) {
                self.set_io_priority(handle, 1);
            }

            // EmptyWorkingSet: Disk Lock always; Mem Soft keeps L3 trim; Mem Hard too.
            if action.apply_disk_lock || action.apply_mem_lock {
                let _ = EmptyWorkingSet(handle);
            }

            // Hard Mem Lock WS shrink only on Idle/Suspend ladder (Mem Hard → Background/Idle).
            if action.apply_mem_lock
                && matches!(
                    action.level,
                    ThrottleLevel::Idle | ThrottleLevel::Suspend
                )
            {
                let mut pmc = PROCESS_MEMORY_COUNTERS::default();
                if GetProcessMemoryInfo(
                    handle,
                    &mut pmc,
                    std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                )
                .is_ok()
                {
                    let floor = 32usize * 1024 * 1024;
                    let max = ((pmc.WorkingSetSize as f64) * 0.6).max(floor as f64) as usize;
                    let _ = SetProcessWorkingSetSize(handle, usize::MAX, max);
                }
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
            set_ecoqos(handle, false);
            set_memory_priority(handle, false);
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
            self.resume_one(pid)?;
            self.ledger.remove(pid);
            self.arm_suspend_cooldown(pid);
        }
        unsafe {
            let handle =
                OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, false, pid)?;
            SetPriorityClass(handle, NORMAL_PRIORITY_CLASS)?;
            self.set_io_priority(handle, 2);
            set_ecoqos(handle, false);
            set_memory_priority(handle, false);
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
fn set_ecoqos(handle: HANDLE, enable: bool) {
    let mut state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: if enable {
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED
        } else {
            0
        },
    };
    unsafe {
        let _ = SetProcessInformation(
            handle,
            ProcessPowerThrottling,
            &mut state as *mut _ as *const _,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

#[cfg(windows)]
fn set_memory_priority(handle: HANDLE, low: bool) {
    let mut info = MEMORY_PRIORITY_INFORMATION {
        MemoryPriority: if low {
            MEMORY_PRIORITY_LOW
        } else {
            MEMORY_PRIORITY_NORMAL
        },
    };
    unsafe {
        let _ = SetProcessInformation(
            handle,
            ProcessMemoryPriority,
            &mut info as *mut _ as *const _,
            std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
        );
    }
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

#[cfg(test)]
mod soft_ttl_tests {
    use super::*;

    #[test]
    fn soft_demotion_ttl_forces_restore() {
        let mut ex = ThrottleExecutor::new(70, 45);
        ex.set_max_soft_demote_secs(45);
        ex.seed_soft_demotion_for_test(4242);
        assert_eq!(ex.soft_applied_count(), 1);
        ex.force_soft_demote_age_for_test(4242, 60);
        let expired = ex.expire_soft_demotions();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, 4242);
        assert_eq!(ex.soft_applied_count(), 0);
    }
}
