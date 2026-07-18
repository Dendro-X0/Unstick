use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use guardian_core::{
    classify_thermal_power, CoolingMode, ProcessSample, SystemSample, ThermalPowerInputs,
};
use sysinfo::{Disks, ProcessesToUpdate, System};
use tracing::{debug, warn};

pub struct WinSensor {
    sys: System,
    disks: Disks,
    prev_proc_io: HashMap<u32, (u64, u64, Instant)>,
    prev_cpu_times: HashMap<u32, Instant>,
    last_pages: Option<(u64, Instant)>,
    prev_agg_io: Option<(u64, Instant)>,
    #[cfg(windows)]
    pdh: Option<PdhDiskCounters>,
    #[cfg(windows)]
    pdh_mem: Option<PdhMemoryCounters>,
    #[cfg(windows)]
    pdh_cpu: Option<PdhCpuCounters>,
}

impl WinSensor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut disks = Disks::new_with_refreshed_list();
        disks.refresh(true);
        #[cfg(windows)]
        let pdh = match PdhDiskCounters::open() {
            Ok(p) => {
                debug!("PDH system-disk counters ready");
                Some(p)
            }
            Err(e) => {
                warn!(error = %e, "PDH disk counters unavailable; using estimate fallback");
                None
            }
        };
        #[cfg(windows)]
        let pdh_mem = match PdhMemoryCounters::open() {
            Ok(p) => {
                debug!("PDH memory/paging counters ready");
                Some(p)
            }
            Err(e) => {
                warn!(error = %e, "PDH memory counters unavailable");
                None
            }
        };
        #[cfg(windows)]
        let pdh_cpu = match PdhCpuCounters::open() {
            Ok(p) => {
                debug!("PDH DPC/interrupt counters ready");
                Some(p)
            }
            Err(e) => {
                warn!(error = %e, "PDH DPC/interrupt counters unavailable");
                None
            }
        };
        Self {
            sys,
            disks,
            prev_proc_io: HashMap::new(),
            prev_cpu_times: HashMap::new(),
            last_pages: None,
            prev_agg_io: None,
            #[cfg(windows)]
            pdh,
            #[cfg(windows)]
            pdh_mem,
            #[cfg(windows)]
            pdh_cpu,
        }
    }

    pub fn sample(&mut self) -> SystemSample {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.disks.refresh(true);

        let cpu_percent = self.sys.global_cpu_usage();
        let memory_total_bytes = self.sys.total_memory();
        let memory_available_bytes = self.sys.available_memory();

        let memory_commit_percent = commit_percent().unwrap_or_else(|| {
            if memory_total_bytes > 0 {
                let used = memory_total_bytes.saturating_sub(memory_available_bytes);
                (used as f32 / memory_total_bytes as f32) * 100.0
            } else {
                0.0
            }
        });

        let now = Instant::now();
        let mut processes = Vec::new();
        let mut agg_io = 0u64;

        for (pid, proc) in self.sys.processes() {
            let pid_u = pid.as_u32();
            let name = proc.name().to_string_lossy().to_string();
            let cpu_percent = proc.cpu_usage();
            let memory_bytes = proc.memory();
            let parent_pid = proc.parent().map(|p| p.as_u32()).unwrap_or(0);

            let disk_total = proc.disk_usage();
            let read_total = disk_total.read_bytes;
            let write_total = disk_total.written_bytes;
            let (rps, wps) = if let Some((pr, pw, t0)) = self.prev_proc_io.get(&pid_u) {
                let dt = now.duration_since(*t0).as_secs_f64().max(0.001);
                let rps = ((read_total.saturating_sub(*pr)) as f64 / dt) as u64;
                let wps = ((write_total.saturating_sub(*pw)) as f64 / dt) as u64;
                (rps, wps)
            } else {
                (0, 0)
            };
            self.prev_proc_io
                .insert(pid_u, (read_total, write_total, now));
            agg_io = agg_io.saturating_add(rps.saturating_add(wps));

            let disk_bps = rps.saturating_add(wps);
            // Path only when detect/policy may need it (hot, script host, miner-ish, or warm CPU).
            let path = if need_exe_path(&name, cpu_percent, disk_bps) {
                proc.exe().map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };
            // v0.1.2: skip expensive cmd() for quiet processes; detect still
            // sees cmdline for script hosts / hot / miner-ish names.
            let cmd_line = if need_cmdline(&name, cpu_percent, disk_bps) {
                let args: Vec<String> = proc
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect();
                if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                }
            } else {
                None
            };

            processes.push(ProcessSample {
                pid: pid_u,
                parent_pid,
                name,
                path,
                cpu_percent,
                memory_bytes,
                disk_read_bytes_per_sec: rps,
                disk_write_bytes_per_sec: wps,
                cmd_line,
            });
        }

        let live: std::collections::HashSet<u32> = processes.iter().map(|p| p.pid).collect();
        self.prev_proc_io.retain(|k, _| live.contains(k));
        self.prev_cpu_times.retain(|k, _| live.contains(k));

        processes.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let (est_busy, est_queue) =
            estimate_disk(&self.disks, &self.sys, agg_io, &mut self.prev_agg_io, now);

        #[cfg(windows)]
        let (disk_busy_percent, disk_queue_length, disk_latency_sec) = {
            let pdh_vals = self.pdh.as_mut().and_then(|p| p.read());
            match pdh_vals {
                Some((busy, queue, latency)) => (busy, queue, latency),
                None => (est_busy, est_queue, 0.0),
            }
        };
        #[cfg(not(windows))]
        let (disk_busy_percent, disk_queue_length, disk_latency_sec) = (est_busy, est_queue, 0.0);

        let hard_faults_per_sec;
        let pagefile_writes_per_sec;
        let paging_file_pct;
        #[cfg(windows)]
        {
            if let Some((pages, writes, pf_pct)) = self.pdh_mem.as_mut().and_then(|p| p.read()) {
                hard_faults_per_sec = pages;
                pagefile_writes_per_sec = writes;
                paging_file_pct = pf_pct;
            } else {
                hard_faults_per_sec = self.estimate_hard_faults(now);
                pagefile_writes_per_sec = 0.0;
                paging_file_pct = 0.0;
            }
        }
        #[cfg(not(windows))]
        {
            hard_faults_per_sec = self.estimate_hard_faults(now);
            pagefile_writes_per_sec = 0.0;
            paging_file_pct = 0.0;
        }

        let (dpc_time_percent, interrupt_time_percent) = {
            #[cfg(windows)]
            {
                self.pdh_cpu
                    .as_mut()
                    .and_then(|p| p.read())
                    .unwrap_or((0.0, 0.0))
            }
            #[cfg(not(windows))]
            {
                (0.0, 0.0)
            }
        };

        let (on_battery, battery_percent, cooling_mode, cpu_mhz_ratio) = sample_power_thermal();
        let thermal_level = classify_thermal_power(&ThermalPowerInputs {
            on_battery,
            battery_percent,
            cooling: cooling_mode,
            cpu_mhz_ratio,
        });
        let focus_pid = sample_focus_pid();

        SystemSample {
            timestamp: Utc::now(),
            cpu_percent,
            memory_total_bytes,
            memory_available_bytes,
            memory_commit_percent,
            disk_busy_percent,
            disk_queue_length,
            disk_latency_sec,
            disk_io_bytes_per_sec: agg_io,
            hard_faults_per_sec,
            pagefile_writes_per_sec,
            paging_file_pct,
            dpc_time_percent,
            interrupt_time_percent,
            on_battery,
            battery_percent,
            cooling_mode,
            cpu_mhz_ratio,
            thermal_level,
            focus_pid,
            processes,
        }
    }

    fn estimate_hard_faults(&mut self, now: Instant) -> f32 {
        let page_faults = page_fault_count().unwrap_or(self.sys.used_swap());
        let rate = if let Some((prev, t0)) = self.last_pages {
            let dt = now.duration_since(t0).as_secs_f32().max(0.001);
            (page_faults.saturating_sub(prev) as f32) / dt
        } else {
            0.0
        };
        self.last_pages = Some((page_faults, now));
        rate
    }
}

impl Default for WinSensor {
    fn default() -> Self {
        Self::new()
    }
}

/// When true, `sample()` reads process command lines (expensive on Windows).
fn need_cmdline(name: &str, cpu_percent: f32, disk_bps: u64) -> bool {
    const SCRIPT_HOSTS: &[&str] = &[
        "wscript.exe",
        "cscript.exe",
        "mshta.exe",
        "powershell.exe",
    ];
    const MINER_NAME_TOKENS: &[&str] =
        &["xmrig", "minerd", "cpuminer", "nicehash", "monero"];
    let lower = name.to_lowercase();
    if SCRIPT_HOSTS.iter().any(|h| lower == *h) {
        return true;
    }
    if cpu_percent >= 50.0 {
        return true;
    }
    if disk_bps >= 200_000 {
        return true;
    }
    MINER_NAME_TOKENS.iter().any(|t| lower.contains(t))
}

/// Exe path for whitelist / suspicious_path; skip for cold quiet processes.
fn need_exe_path(name: &str, cpu_percent: f32, disk_bps: u64) -> bool {
    if need_cmdline(name, cpu_percent, disk_bps) || cpu_percent >= 8.0 {
        return true;
    }
    // Always resolve path for well-known protected/IDE names so path_substr
    // protection still works when CPU is idle.
    const ALWAYS_PATH: &[&str] = &[
        "cursor.exe",
        "code.exe",
        "code - insiders.exe",
        "devenv.exe",
        "explorer.exe",
    ];
    let lower = name.to_lowercase();
    ALWAYS_PATH.iter().any(|n| lower == *n)
}

#[cfg(test)]
mod cmdline_gate_tests {
    use super::need_cmdline;

    #[test]
    fn quiet_app_skips_cmdline() {
        assert!(!need_cmdline("notepad.exe", 2.0, 0));
    }

    #[test]
    fn script_host_always() {
        assert!(need_cmdline("powershell.exe", 1.0, 0));
    }

    #[test]
    fn hot_cpu_or_disk() {
        assert!(need_cmdline("chrome.exe", 55.0, 0));
        assert!(need_cmdline("chrome.exe", 1.0, 250_000));
    }

    #[test]
    fn minerish_name() {
        assert!(need_cmdline("xmrig.exe", 1.0, 0));
    }
}

fn estimate_disk(
    disks: &Disks,
    sys: &System,
    agg_io_bps: u64,
    prev_agg: &mut Option<(u64, Instant)>,
    now: Instant,
) -> (f32, f32) {
    let mut busy = 0.0f32;
    for d in disks.list() {
        let total = d.total_space().max(1);
        let avail = d.available_space();
        let used_ratio = 1.0 - (avail as f32 / total as f32);
        if used_ratio > 0.95 {
            busy = busy.max(55.0);
        }
    }

    // Low-end DRAM-less SSD saturates far below 150 MB/s — use 20 MB/s budget for fallback.
    let budget = 20.0 * 1024.0 * 1024.0;
    let io_busy = ((agg_io_bps as f32) / budget * 100.0).clamp(0.0, 100.0);
    busy = busy.max(io_busy);

    let avail = sys.available_memory() as f32;
    let total = sys.total_memory().max(1) as f32;
    let mem_pressure = 1.0 - (avail / total);
    if mem_pressure > 0.85 {
        busy = busy.max(70.0 + (mem_pressure - 0.85) * 200.0);
    }

    let swap_ratio = if sys.total_swap() > 0 {
        sys.used_swap() as f32 / sys.total_swap() as f32
    } else {
        0.0
    };
    busy = busy.max(swap_ratio * 100.0).clamp(0.0, 100.0);

    let mut queue = (agg_io_bps as f32 / (4.0 * 1024.0 * 1024.0)).clamp(0.0, 12.0);
    if mem_pressure > 0.9 {
        queue += 4.0;
    }
    if let Some((prev, t0)) = *prev_agg {
        let dt = now.duration_since(t0).as_secs_f32().max(0.001);
        let delta = (agg_io_bps as i64 - prev as i64).unsigned_abs() as f32 / dt;
        if delta > budget * 0.5 {
            queue = queue.max(6.0);
        }
    }
    *prev_agg = Some((agg_io_bps, now));

    (busy, queue.clamp(0.0, 16.0))
}

#[cfg(windows)]
fn sample_power_thermal() -> (bool, Option<u8>, CoolingMode, f32) {
    use windows::Win32::System::Power::{
        CallNtPowerInformation, GetSystemPowerStatus, ProcessorInformation, SystemPowerInformation,
        PROCESSOR_POWER_INFORMATION, SYSTEM_POWER_INFORMATION, SYSTEM_POWER_STATUS,
    };
    use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

    let mut on_battery = false;
    let mut battery_percent = None;
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_ok() {
            on_battery = status.ACLineStatus == 0;
            if status.BatteryLifePercent <= 100 {
                battery_percent = Some(status.BatteryLifePercent);
            }
        }
    }

    let cooling_mode = unsafe {
        let mut info = SYSTEM_POWER_INFORMATION::default();
        let status = CallNtPowerInformation(
            SystemPowerInformation,
            None,
            0,
            Some(&mut info as *mut _ as *mut _),
            std::mem::size_of::<SYSTEM_POWER_INFORMATION>() as u32,
        );
        if status.is_ok() {
            CoolingMode::from_po_tz(info.CoolingMode.0 as u8)
        } else {
            CoolingMode::Unknown
        }
    };

    let cpu_mhz_ratio = unsafe {
        let mut sys = SYSTEM_INFO::default();
        GetSystemInfo(&mut sys);
        let n = sys.dwNumberOfProcessors.max(1) as usize;
        let mut buf = vec![PROCESSOR_POWER_INFORMATION::default(); n];
        let status = CallNtPowerInformation(
            ProcessorInformation,
            None,
            0,
            Some(buf.as_mut_ptr() as *mut _),
            (std::mem::size_of::<PROCESSOR_POWER_INFORMATION>() * n) as u32,
        );
        if status.is_ok() {
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for p in &buf {
                if p.MaxMhz > 0 {
                    sum += (p.CurrentMhz as f32 / p.MaxMhz as f32).clamp(0.0, 1.0);
                    count += 1;
                }
            }
            if count > 0 {
                sum / count as f32
            } else {
                0.0
            }
        } else {
            0.0
        }
    };

    (on_battery, battery_percent, cooling_mode, cpu_mhz_ratio)
}

#[cfg(not(windows))]
fn sample_power_thermal() -> (bool, Option<u8>, CoolingMode, f32) {
    (false, None, CoolingMode::Unknown, 0.0)
}

#[cfg(windows)]
fn sample_focus_pid() -> Option<u32> {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }
}

#[cfg(not(windows))]
fn sample_focus_pid() -> Option<u32> {
    None
}

#[cfg(windows)]
fn commit_percent() -> Option<f32> {
    use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    unsafe {
        let mut info = PERFORMANCE_INFORMATION::default();
        info.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
        GetPerformanceInfo(&mut info, info.cb).ok()?;
        let limit = info.CommitLimit.max(1);
        Some((info.CommitTotal as f32 / limit as f32) * 100.0)
    }
}

#[cfg(not(windows))]
fn commit_percent() -> Option<f32> {
    None
}

#[cfg(windows)]
fn page_fault_count() -> Option<u64> {
    None
}

#[cfg(not(windows))]
fn page_fault_count() -> Option<u64> {
    None
}

/// PDH Processor(_Total) % DPC Time and % Interrupt Time (detect-only advisory).
#[cfg(windows)]
struct PdhCpuCounters {
    query: isize,
    dpc_counter: isize,
    interrupt_counter: isize,
    primed: bool,
}

#[cfg(windows)]
impl PdhCpuCounters {
    fn open() -> anyhow::Result<Self> {
        use std::ffi::CString;
        use windows::core::PCSTR;
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterA, PdhCollectQueryData, PdhOpenQueryA,
        };

        unsafe {
            let mut query = 0isize;
            let status = PdhOpenQueryA(PCSTR::null(), 0, &mut query);
            if status != 0 {
                anyhow::bail!("PdhOpenQueryA cpu {status:#x}");
            }

            let dpc_path = CString::new(r"\Processor(_Total)\% DPC Time").unwrap();
            let irq_path = CString::new(r"\Processor(_Total)\% Interrupt Time").unwrap();

            let mut dpc_counter = 0isize;
            let mut interrupt_counter = 0isize;
            let s1 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(dpc_path.as_ptr() as *const u8),
                0,
                &mut dpc_counter,
            );
            let s2 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(irq_path.as_ptr() as *const u8),
                0,
                &mut interrupt_counter,
            );
            if s1 != 0 || s2 != 0 {
                anyhow::bail!("PdhAddEnglishCounter dpc={s1:#x} interrupt={s2:#x}");
            }

            let _ = PdhCollectQueryData(query);
            Ok(Self {
                query,
                dpc_counter,
                interrupt_counter,
                primed: false,
            })
        }
    }

    fn read(&mut self) -> Option<(f32, f32)> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };

        unsafe {
            let status = PdhCollectQueryData(self.query);
            if status != 0 {
                return None;
            }
            if !self.primed {
                self.primed = true;
                return None;
            }

            let mut dpc_val = PDH_FMT_COUNTERVALUE::default();
            let mut irq_val = PDH_FMT_COUNTERVALUE::default();
            let s1 = PdhGetFormattedCounterValue(
                self.dpc_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut dpc_val,
            );
            let s2 = PdhGetFormattedCounterValue(
                self.interrupt_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut irq_val,
            );
            if s1 != 0 || s2 != 0 {
                return None;
            }

            let dpc = (dpc_val.Anonymous.doubleValue as f32).clamp(0.0, 100.0);
            let irq = (irq_val.Anonymous.doubleValue as f32).clamp(0.0, 100.0);
            Some((dpc, irq))
        }
    }
}

#[cfg(windows)]
impl Drop for PdhCpuCounters {
    fn drop(&mut self) {
        use windows::Win32::System::Performance::PdhCloseQuery;
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

/// PDH Memory\Pages/sec, Page Writes/sec, and Paging File % Usage.
#[cfg(windows)]
struct PdhMemoryCounters {
    query: isize,
    pages_counter: isize,
    writes_counter: isize,
    paging_file_counter: isize,
    primed: bool,
}

#[cfg(windows)]
impl PdhMemoryCounters {
    fn open() -> anyhow::Result<Self> {
        use std::ffi::CString;
        use windows::core::PCSTR;
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterA, PdhCollectQueryData, PdhOpenQueryA,
        };

        unsafe {
            let mut query = 0isize;
            let status = PdhOpenQueryA(PCSTR::null(), 0, &mut query);
            if status != 0 {
                anyhow::bail!("PdhOpenQueryA memory {status:#x}");
            }

            let pages_path = CString::new(r"\Memory\Pages/sec").unwrap();
            let writes_path = CString::new(r"\Memory\Page Writes/sec").unwrap();
            let pf_path = CString::new(r"\Paging File(_Total)\% Usage").unwrap();

            let mut pages_counter = 0isize;
            let mut writes_counter = 0isize;
            let mut paging_file_counter = 0isize;
            let s1 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(pages_path.as_ptr() as *const u8),
                0,
                &mut pages_counter,
            );
            let s2 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(writes_path.as_ptr() as *const u8),
                0,
                &mut writes_counter,
            );
            let s3 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(pf_path.as_ptr() as *const u8),
                0,
                &mut paging_file_counter,
            );
            if s1 != 0 || s2 != 0 || s3 != 0 {
                anyhow::bail!(
                    "PdhAddEnglishCounter pages={s1:#x} writes={s2:#x} paging_file={s3:#x}"
                );
            }

            let _ = PdhCollectQueryData(query);
            Ok(Self {
                query,
                pages_counter,
                writes_counter,
                paging_file_counter,
                primed: false,
            })
        }
    }

    fn read(&mut self) -> Option<(f32, f32, f32)> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };

        unsafe {
            let status = PdhCollectQueryData(self.query);
            if status != 0 {
                return None;
            }
            if !self.primed {
                self.primed = true;
                return None;
            }

            let mut pages_val = PDH_FMT_COUNTERVALUE::default();
            let mut writes_val = PDH_FMT_COUNTERVALUE::default();
            let mut pf_val = PDH_FMT_COUNTERVALUE::default();
            let s1 = PdhGetFormattedCounterValue(
                self.pages_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut pages_val,
            );
            let s2 = PdhGetFormattedCounterValue(
                self.writes_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut writes_val,
            );
            let s3 = PdhGetFormattedCounterValue(
                self.paging_file_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut pf_val,
            );
            if s1 != 0 || s2 != 0 || s3 != 0 {
                return None;
            }

            let pages = (pages_val.Anonymous.doubleValue as f32).clamp(0.0, 1_000_000.0);
            let writes = (writes_val.Anonymous.doubleValue as f32).clamp(0.0, 1_000_000.0);
            let pf_pct = (pf_val.Anonymous.doubleValue as f32).clamp(0.0, 100.0);
            Some((pages, writes, pf_pct))
        }
    }
}

#[cfg(windows)]
impl Drop for PdhMemoryCounters {
    fn drop(&mut self) {
        use windows::Win32::System::Performance::PdhCloseQuery;
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

/// PDH counters for system PhysicalDisk Active Time, queue, and transfer latency.
#[cfg(windows)]
struct PdhDiskCounters {
    query: isize,
    idle_counter: isize,
    queue_counter: isize,
    latency_counter: isize,
    primed: bool,
}

#[cfg(windows)]
impl PdhDiskCounters {
    fn open() -> anyhow::Result<Self> {
        use std::ffi::CString;
        use windows::core::PCSTR;
        use windows::Win32::System::Performance::{
            PdhAddEnglishCounterA, PdhCollectQueryData, PdhOpenQueryA,
        };

        let system_letter = system_drive_letter().unwrap_or('C');
        let instance = find_physical_disk_instance(system_letter)?;

        unsafe {
            let mut query = 0isize;
            let status = PdhOpenQueryA(PCSTR::null(), 0, &mut query);
            if status != 0 {
                anyhow::bail!("PdhOpenQueryA {status:#x}");
            }

            let idle_path =
                CString::new(format!(r"\PhysicalDisk({instance})\% Idle Time")).unwrap();
            let queue_path =
                CString::new(format!(r"\PhysicalDisk({instance})\Avg. Disk Queue Length"))
                    .unwrap();
            let latency_path =
                CString::new(format!(r"\PhysicalDisk({instance})\Avg. Disk sec/Transfer"))
                    .unwrap();

            let mut idle_counter = 0isize;
            let mut queue_counter = 0isize;
            let mut latency_counter = 0isize;
            let s1 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(idle_path.as_ptr() as *const u8),
                0,
                &mut idle_counter,
            );
            let s2 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(queue_path.as_ptr() as *const u8),
                0,
                &mut queue_counter,
            );
            let s3 = PdhAddEnglishCounterA(
                query,
                PCSTR::from_raw(latency_path.as_ptr() as *const u8),
                0,
                &mut latency_counter,
            );
            if s1 != 0 || s2 != 0 || s3 != 0 {
                anyhow::bail!("PdhAddEnglishCounter idle={s1:#x} queue={s2:#x} latency={s3:#x}");
            }

            let _ = PdhCollectQueryData(query);

            Ok(Self {
                query,
                idle_counter,
                queue_counter,
                latency_counter,
                primed: false,
            })
        }
    }

    fn read(&mut self) -> Option<(f32, f32, f32)> {
        use windows::Win32::System::Performance::{
            PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
        };

        unsafe {
            let status = PdhCollectQueryData(self.query);
            if status != 0 {
                return None;
            }
            if !self.primed {
                self.primed = true;
                return None; // need second sample for rate counters
            }

            let mut idle_val = PDH_FMT_COUNTERVALUE::default();
            let mut queue_val = PDH_FMT_COUNTERVALUE::default();
            let mut latency_val = PDH_FMT_COUNTERVALUE::default();
            let s1 = PdhGetFormattedCounterValue(
                self.idle_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut idle_val,
            );
            let s2 = PdhGetFormattedCounterValue(
                self.queue_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut queue_val,
            );
            let s3 = PdhGetFormattedCounterValue(
                self.latency_counter,
                PDH_FMT_DOUBLE,
                None,
                &mut latency_val,
            );
            if s1 != 0 || s2 != 0 || s3 != 0 {
                return None;
            }

            let idle = idle_val.Anonymous.doubleValue as f32;
            let queue = queue_val.Anonymous.doubleValue as f32;
            let latency = (latency_val.Anonymous.doubleValue as f32).clamp(0.0, 30.0);
            let busy = (100.0 - idle).clamp(0.0, 100.0);
            Some((busy, queue.clamp(0.0, 64.0), latency))
        }
    }
}

#[cfg(windows)]
impl Drop for PdhDiskCounters {
    fn drop(&mut self) {
        use windows::Win32::System::Performance::PdhCloseQuery;
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}

#[cfg(windows)]
fn system_drive_letter() -> Option<char> {
    use windows::Win32::System::SystemInformation::GetWindowsDirectoryW;
    unsafe {
        let mut buf = [0u16; 260];
        let n = GetWindowsDirectoryW(Some(&mut buf));
        if n == 0 || n as usize >= buf.len() {
            return None;
        }
        let s = String::from_utf16_lossy(&buf[..n as usize]);
        s.chars().next().map(|c| c.to_ascii_uppercase())
    }
}

#[cfg(windows)]
fn find_physical_disk_instance(system_letter: char) -> anyhow::Result<String> {
    use std::ffi::CString;
    use windows::core::{PCSTR, PSTR};
    use windows::Win32::System::Performance::PdhExpandWildCardPathA;

    let wildcard = CString::new(r"\PhysicalDisk(*)\% Idle Time").unwrap();
    unsafe {
        let mut len = 0u32;
        let _ = PdhExpandWildCardPathA(
            PCSTR::null(),
            PCSTR::from_raw(wildcard.as_ptr() as *const u8),
            PSTR::null(),
            &mut len,
            0,
        );
        if len == 0 {
            anyhow::bail!("PdhExpandWildCardPath size failed");
        }
        let mut buf = vec![0u8; len as usize];
        let status = PdhExpandWildCardPathA(
            PCSTR::null(),
            PCSTR::from_raw(wildcard.as_ptr() as *const u8),
            PSTR::from_raw(buf.as_mut_ptr()),
            &mut len,
            0,
        );
        if status != 0 {
            anyhow::bail!("PdhExpandWildCardPath {status:#x}");
        }

        let needle = format!(" {system_letter}:");
        let text = String::from_utf8_lossy(&buf);
        for path in text.split('\0').filter(|s| !s.is_empty()) {
            if let Some(start) = path.find("PhysicalDisk(") {
                let rest = &path[start + "PhysicalDisk(".len()..];
                if let Some(end) = rest.find(')') {
                    let instance = &rest[..end];
                    if instance != "_Total" && instance.contains(&needle) {
                        return Ok(instance.to_string());
                    }
                }
            }
        }

        for path in text.split('\0').filter(|s| !s.is_empty()) {
            if let Some(start) = path.find("PhysicalDisk(") {
                let rest = &path[start + "PhysicalDisk(".len()..];
                if let Some(end) = rest.find(')') {
                    let instance = &rest[..end];
                    if instance != "_Total" {
                        return Ok(instance.to_string());
                    }
                }
            }
        }
        anyhow::bail!("no PhysicalDisk instance for {system_letter}:");
    }
}
