use std::collections::HashMap;
use std::time::Instant;

use chrono::Utc;
use guardian_core::{ProcessSample, SystemSample};
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
        Self {
            sys,
            disks,
            prev_proc_io: HashMap::new(),
            prev_cpu_times: HashMap::new(),
            last_pages: None,
            prev_agg_io: None,
            #[cfg(windows)]
            pdh,
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
            let path = proc.exe().map(|p| p.to_string_lossy().to_string());
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

            let cmd_line = {
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
        let (disk_busy_percent, disk_queue_length) = {
            let pdh_vals = self.pdh.as_mut().and_then(|p| p.read());
            match pdh_vals {
                Some((busy, queue)) => (busy, queue),
                None => (est_busy, est_queue),
            }
        };
        #[cfg(not(windows))]
        let (disk_busy_percent, disk_queue_length) = (est_busy, est_queue);

        let hard_faults_per_sec = self.estimate_hard_faults(now);

        SystemSample {
            timestamp: Utc::now(),
            cpu_percent,
            memory_total_bytes,
            memory_available_bytes,
            memory_commit_percent,
            disk_busy_percent,
            disk_queue_length,
            disk_io_bytes_per_sec: agg_io,
            hard_faults_per_sec,
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

/// PDH counters for system PhysicalDisk Active Time (~ Task Manager) + queue.
#[cfg(windows)]
struct PdhDiskCounters {
    query: isize,
    idle_counter: isize,
    queue_counter: isize,
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

            let mut idle_counter = 0isize;
            let mut queue_counter = 0isize;
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
            if s1 != 0 || s2 != 0 {
                anyhow::bail!("PdhAddEnglishCounter idle={s1:#x} queue={s2:#x}");
            }

            let _ = PdhCollectQueryData(query);

            Ok(Self {
                query,
                idle_counter,
                queue_counter,
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
                return None; // need second sample for rate counters
            }

            let mut idle_val = PDH_FMT_COUNTERVALUE::default();
            let mut queue_val = PDH_FMT_COUNTERVALUE::default();
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
            if s1 != 0 || s2 != 0 {
                return None;
            }

            let idle = idle_val.Anonymous.doubleValue as f32;
            let queue = queue_val.Anonymous.doubleValue as f32;
            let busy = (100.0 - idle).clamp(0.0, 100.0);
            Some((busy, queue.clamp(0.0, 64.0)))
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
