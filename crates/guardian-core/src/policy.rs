use crate::config::GuardianConfig;
use crate::pressure::{DiskLockMode, PressureBand};
use crate::types::{ProcessSample, SystemSample, ThrottleLevel};
use crate::{SERVICE_BIN, TRAY_BIN};

#[derive(Debug, Clone)]
pub struct PlannedAction {
    pub pid: u32,
    pub name: String,
    pub level: ThrottleLevel,
    pub apply_job_cap: bool,
    /// VeryLow I/O + EmptyWorkingSet (Disk Lock soft/hard).
    pub apply_disk_lock: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ActionPlan {
    pub boost_foreground: bool,
    pub actions: Vec<PlannedAction>,
    pub disk_lock: DiskLockMode,
}

pub struct ProtectedSet {
    names: Vec<String>,
    path_substr: Vec<String>,
    pids: Vec<u32>,
}

impl ProtectedSet {
    pub fn from_config(cfg: &GuardianConfig, self_pid: u32) -> Self {
        let mut names: Vec<String> = [
            "csrss.exe",
            "winlogon.exe",
            "services.exe",
            "lsass.exe",
            "smss.exe",
            "dwm.exe",
            "fontdrvhost.exe",
            "explorer.exe",
            "system",
            "registry",
            "memory compression",
            "secure system",
            SERVICE_BIN,
            TRAY_BIN,
            "guardian-service.exe",
            "guardian-tray.exe",
            "guardian-ui.exe",
            "guardian-ui",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

        for extra in &cfg.protected_extra {
            names.push(extra.to_lowercase());
        }

        let mut path_substr = vec![
            r"\cursor\".to_string(),
            r"\code.exe".to_string(),
            r"\devenv.exe".to_string(),
        ];

        for entry in &cfg.whitelist {
            let e = entry.trim().to_lowercase();
            if e.is_empty() {
                continue;
            }
            if e.ends_with(".exe") && !e.contains('\\') && !e.contains('/') {
                names.push(e.clone());
            }
            path_substr.push(e);
        }

        let mut pids = cfg.trusted_pids.clone();
        pids.push(self_pid);

        Self {
            names,
            path_substr,
            pids,
        }
    }

    pub fn is_protected(&self, proc: &ProcessSample) -> bool {
        if self.pids.contains(&proc.pid) {
            return true;
        }
        let name = proc.name.to_lowercase();
        if self.names.iter().any(|n| n == &name || name.starts_with(n)) {
            return true;
        }
        if let Some(path) = &proc.path {
            let p = path.to_lowercase();
            if self.path_substr.iter().any(|s| p.contains(s)) {
                return true;
            }
        }
        false
    }
}

const BUILD_NAMES: &[&str] = &[
    "cargo.exe",
    "rustc.exe",
    "cl.exe",
    "link.exe",
    "msbuild.exe",
    "node.exe",
    "npm.exe",
    "pnpm.exe",
    "yarn.exe",
    "python.exe",
    "pip.exe",
    "dotnet.exe",
];

pub fn is_build_or_mcp(proc: &ProcessSample) -> bool {
    let name = proc.name.to_lowercase();
    if BUILD_NAMES.iter().any(|n| name == *n) {
        return true;
    }
    false
}

fn disk_bytes(p: &ProcessSample) -> u64 {
    p.disk_read_bytes_per_sec
        .saturating_add(p.disk_write_bytes_per_sec)
}

fn offender_weight(p: &ProcessSample) -> f32 {
    let io = disk_bytes(p) as f32;
    let io_norm = (io / (32.0 * 1024.0 * 1024.0)).clamp(0.0, 1.0);
    p.cpu_percent * 0.6 + io_norm * 100.0 * 0.4
}

fn disk_weight(p: &ProcessSample) -> f32 {
    let io = disk_bytes(p) as f32;
    // Prefer absolute IO; tiny IO still ranks above idle when disk is saturated
    io + p.cpu_percent * 10_000.0
}

pub struct PolicyEngine {
    pub protected: ProtectedSet,
    pub emergency_suspend: bool,
    pub disk_lock_enabled: bool,
    pub max_actions: usize,
    pub max_suspend_pids: usize,
}

impl PolicyEngine {
    pub fn new(cfg: &GuardianConfig, self_pid: u32) -> Self {
        Self {
            protected: ProtectedSet::from_config(cfg, self_pid),
            emergency_suspend: cfg.emergency_suspend,
            disk_lock_enabled: cfg.disk_lock_enabled,
            max_actions: 8,
            max_suspend_pids: cfg.max_suspend_pids.max(1),
        }
    }

    pub fn plan(
        &self,
        band: PressureBand,
        sample: &SystemSample,
        tripwire: Option<&str>,
        disk_lock: DiskLockMode,
    ) -> ActionPlan {
        let mut plan = ActionPlan {
            disk_lock,
            ..Default::default()
        };

        let disk_active = self.disk_lock_enabled && disk_lock != DiskLockMode::Off;
        let need_actions = matches!(band, PressureBand::Throttle | PressureBand::Emergency)
            || disk_active;

        if band == PressureBand::Normal && !disk_active {
            return plan;
        }
        if band == PressureBand::Warn && !disk_active {
            plan.boost_foreground = true;
            return plan;
        }
        if !need_actions {
            return plan;
        }

        plan.boost_foreground = true;

        let soft_level = if band == PressureBand::Emergency
            || disk_lock == DiskLockMode::Hard
            || disk_lock == DiskLockMode::Soft
        {
            ThrottleLevel::Idle
        } else {
            ThrottleLevel::BelowNormal
        };

        let apply_disk = disk_active;
        let reason_base = if apply_disk {
            match disk_lock {
                DiskLockMode::Hard => "disk_lock:hard".to_string(),
                DiskLockMode::Soft => "disk_lock:soft".to_string(),
                DiskLockMode::Off => unreachable!(),
            }
        } else {
            match tripwire {
                Some(t) => format!("pressure:{}:tripwire:{t}", band.as_str()),
                None => format!("pressure:{}", band.as_str()),
            }
        };

        let mut offenders: Vec<&ProcessSample> = sample
            .processes
            .iter()
            .filter(|p| !self.protected.is_protected(p))
            .collect();

        if apply_disk {
            // Disk Lock: take top-N by disk even if each process is under 1 MB/s
            offenders.sort_by(|a, b| {
                disk_weight(b)
                    .partial_cmp(&disk_weight(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            offenders.retain(|p| {
                p.cpu_percent >= 5.0 || disk_bytes(p) > 1_000_000
            });
            offenders.sort_by(|a, b| {
                offender_weight(b)
                    .partial_cmp(&offender_weight(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        for proc in offenders.iter().take(self.max_actions) {
            plan.actions.push(PlannedAction {
                pid: proc.pid,
                name: proc.name.clone(),
                level: soft_level,
                apply_job_cap: is_build_or_mcp(proc),
                apply_disk_lock: apply_disk,
                reason: reason_base.clone(),
            });
        }

        let do_suspend = (band == PressureBand::Emergency || disk_lock == DiskLockMode::Hard)
            && self.emergency_suspend;

        if do_suspend {
            let mut suspend_count = 0usize;
            for proc in offenders.iter() {
                if suspend_count >= self.max_suspend_pids {
                    break;
                }
                if self.protected.is_protected(proc) {
                    continue;
                }
                let reason = if apply_disk {
                    "disk_lock:hard".to_string()
                } else {
                    format!("{reason_base}:suspend")
                };
                if let Some(existing) = plan.actions.iter_mut().find(|a| a.pid == proc.pid) {
                    existing.level = ThrottleLevel::Suspend;
                    existing.apply_disk_lock = apply_disk || existing.apply_disk_lock;
                    existing.reason = reason;
                } else {
                    plan.actions.push(PlannedAction {
                        pid: proc.pid,
                        name: proc.name.clone(),
                        level: ThrottleLevel::Suspend,
                        apply_job_cap: false,
                        apply_disk_lock: apply_disk,
                        reason,
                    });
                }
                suspend_count += 1;
            }
        }

        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_with(procs: Vec<ProcessSample>) -> SystemSample {
        SystemSample {
            timestamp: Utc::now(),
            cpu_percent: 90.0,
            memory_total_bytes: 8 << 30,
            memory_available_bytes: 1 << 30,
            memory_commit_percent: 80.0,
            disk_busy_percent: 90.0,
            disk_queue_length: 5.0,
            disk_io_bytes_per_sec: 10_000_000,
            hard_faults_per_sec: 100.0,
            processes: procs,
        }
    }

    #[test]
    fn never_throttles_protected() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 4,
                parent_pid: 0,
                name: "csrss.exe".into(),
                path: None,
                cpu_percent: 99.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 100,
                parent_pid: 1,
                name: "heavy.exe".into(),
                path: Some(r"C:\temp\heavy.exe".into()),
                cpu_percent: 80.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(PressureBand::Throttle, &sample, None, DiskLockMode::Off);
        assert!(plan.actions.iter().all(|a| a.pid != 4));
        assert!(plan.actions.iter().any(|a| a.pid == 100));
    }

    #[test]
    fn build_tools_get_job_cap() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 200,
            parent_pid: 1,
            name: "cargo.exe".into(),
            path: Some(r"C:\Users\x\.cargo\bin\cargo.exe".into()),
            cpu_percent: 70.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 5_000_000,
            disk_write_bytes_per_sec: 5_000_000,
            cmd_line: None,
        }]);
        let plan = engine.plan(PressureBand::Throttle, &sample, None, DiskLockMode::Off);
        assert!(plan.actions[0].apply_job_cap);
    }

    #[test]
    fn emergency_suspends_non_protected() {
        let cfg = GuardianConfig::default();
        assert!(cfg.emergency_suspend);
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 4,
                parent_pid: 0,
                name: "csrss.exe".into(),
                path: None,
                cpu_percent: 99.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 300,
                parent_pid: 1,
                name: "burn.exe".into(),
                path: Some(r"C:\temp\burn.exe".into()),
                cpu_percent: 95.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("commit_charge"),
            DiskLockMode::Off,
        );
        assert!(plan
            .actions
            .iter()
            .any(|a| a.pid == 300 && a.level == ThrottleLevel::Suspend));
        assert!(plan
            .actions
            .iter()
            .all(|a| a.pid != 4 || a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn whitelist_never_suspended() {
        let mut cfg = GuardianConfig::default();
        cfg.add_whitelist("burn.exe".into());
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 400,
            parent_pid: 1,
            name: "burn.exe".into(),
            path: Some(r"C:\Games\burn.exe".into()),
            cpu_percent: 99.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("commit_charge"),
            DiskLockMode::Hard,
        );
        assert!(plan.actions.iter().all(|a| a.pid != 400));
    }

    #[test]
    fn disk_lock_soft_ranks_low_mb_s_offenders() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 10,
                parent_pid: 1,
                name: "idlecpu.exe".into(),
                path: Some(r"C:\temp\idlecpu.exe".into()),
                cpu_percent: 1.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 400_000,
                disk_write_bytes_per_sec: 200_000,
                cmd_line: None,
            },
            ProcessSample {
                pid: 11,
                parent_pid: 1,
                name: "lessio.exe".into(),
                path: Some(r"C:\temp\lessio.exe".into()),
                cpu_percent: 50.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 10_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(PressureBand::Normal, &sample, None, DiskLockMode::Soft);
        assert!(plan.actions.iter().any(|a| a.pid == 10 && a.apply_disk_lock));
        assert_eq!(plan.actions[0].pid, 10);
        assert_eq!(plan.actions[0].reason, "disk_lock:soft");
        assert!(!plan
            .actions
            .iter()
            .any(|a| a.level == ThrottleLevel::Suspend));
    }

    #[test]
    fn disk_lock_hard_suspends_disk_offender() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 50,
            parent_pid: 1,
            name: "pager.exe".into(),
            path: Some(r"C:\temp\pager.exe".into()),
            cpu_percent: 2.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 500_000,
            disk_write_bytes_per_sec: 500_000,
            cmd_line: None,
        }]);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("disk_busy_hard"),
            DiskLockMode::Hard,
        );
        assert!(plan.actions.iter().any(|a| {
            a.pid == 50 && a.level == ThrottleLevel::Suspend && a.apply_disk_lock
        }));
    }
}
