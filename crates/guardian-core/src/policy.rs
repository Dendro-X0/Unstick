use std::collections::HashSet;

use crate::advisory::ThermalLevel;
use crate::config::{CriticalGuardMode, GuardianConfig};
use crate::pressure::{DiskLockMode, MemLockMode, PressureBand};
use crate::qos::{plan_qos, NapPolicy, QosPlan};
use crate::types::{FocusProfile, ProcessSample, SystemSample, ThrottleLevel};
use crate::{SERVICE_BIN, TRAY_BIN};

#[derive(Debug, Clone)]
pub struct PlannedAction {
    pub pid: u32,
    pub name: String,
    pub level: ThrottleLevel,
    pub apply_job_cap: bool,
    /// VeryLow I/O + EmptyWorkingSet (Disk Lock soft/hard).
    pub apply_disk_lock: bool,
    /// Working-set trim ladder (Mem Lock soft/hard).
    pub apply_mem_lock: bool,
    /// Microsoft EcoQoS / Efficiency Mode execution-speed throttle.
    pub apply_ecoqos: bool,
    /// ProcessMemoryPriority LOW — prefer before Hard WS shrink.
    pub apply_mem_priority_low: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ActionPlan {
    pub boost_foreground: bool,
    pub actions: Vec<PlannedAction>,
    pub disk_lock: DiskLockMode,
    pub mem_lock: MemLockMode,
    pub qos: QosPlan,
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
            // Interactive shells / terminals — never NtSuspend (user cannot recover without kill).
            "windowsterminal.exe",
            "wt.exe",
            "conhost.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "openconsole.exe",
            // Common browsers — Disk Lock hard was suspending these indefinitely.
            "chrome.exe",
            "msedge.exe",
            "msedgewebview2.exe",
            "firefox.exe",
            "brave.exe",
            // Windows Defender / security — elevated; apply always Access Denied noise.
            "msmpeng.exe",
            "mssense.exe",
            "nissrv.exe",
            "securityhealthservice.exe",
            "securityhealthsystray.exe",
            "smartscreen.exe",
            "mpcmdrun.exe",
            "wdsealthealth.exe",
            // IDEs — must match by name when path is gated (v0.1.2 self-overhead).
            "cursor.exe",
            "code.exe",
            "code - insiders.exe",
            "devenv.exe",
            "idea64.exe",
            "pycharm64.exe",
            "webstorm64.exe",
            "rider64.exe",
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

const DEV_FOCUS: &[&str] = &[
    "code.exe",
    "cursor.exe",
    "devenv.exe",
    "idea64.exe",
    "pycharm64.exe",
    "webstorm64.exe",
    "rider64.exe",
    "windowsterminal.exe",
    "code - insiders.exe",
];

const PLAY_FOCUS: &[&str] = &[
    "steam.exe",
    "steamwebhelper.exe",
    "epicgameslauncher.exe",
    "battle.net.exe",
    "origin.exe",
    "eadesktop.exe",
    "gog galaxy.exe",
    "riotclientservices.exe",
];

pub fn is_build_or_mcp(proc: &ProcessSample) -> bool {
    let name = proc.name.to_lowercase();
    BUILD_NAMES.iter().any(|n| name == *n)
}

/// Classify focused app for UI status only (same policy ladder).
pub fn classify_focus_profile(proc: Option<&ProcessSample>) -> FocusProfile {
    let Some(proc) = proc else {
        return FocusProfile::Other;
    };
    let name = proc.name.to_lowercase();
    let path = proc
        .path
        .as_deref()
        .unwrap_or("")
        .to_lowercase();
    if DEV_FOCUS.iter().any(|n| name == *n)
        || path.contains(r"\cursor\")
        || path.contains(r"\microsoft vs code\")
        || path.contains(r"\vscode\")
        || path.contains(r"\jetbrains\")
    {
        return FocusProfile::Dev;
    }
    if PLAY_FOCUS.iter().any(|n| name == *n)
        || path.contains(r"\steam\steamapps\")
        || path.contains(r"\epic games\")
        || path.contains(r"\xbox games\")
    {
        return FocusProfile::Play;
    }
    FocusProfile::Other
}

/// Focus PID plus descendants via parent_pid walk.
pub fn focus_tree_pids(sample: &SystemSample, focus_pid: Option<u32>) -> HashSet<u32> {
    let Some(root) = focus_pid else {
        return HashSet::new();
    };
    let mut set = HashSet::new();
    set.insert(root);
    let mut changed = true;
    while changed {
        changed = false;
        for p in &sample.processes {
            if set.contains(&p.parent_pid) && set.insert(p.pid) {
                changed = true;
            }
        }
    }
    set
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
    io + p.cpu_percent * 10_000.0
}

pub struct PolicyEngine {
    pub protected: ProtectedSet,
    pub emergency_suspend: bool,
    pub critical_guard_mode: CriticalGuardMode,
    pub suspend_escalation_streak: u32,
    pub disk_lock_enabled: bool,
    pub mem_lock_enabled: bool,
    pub max_actions: usize,
    pub max_suspend_pids: usize,
}

impl PolicyEngine {
    pub fn new(cfg: &GuardianConfig, self_pid: u32) -> Self {
        Self {
            protected: ProtectedSet::from_config(cfg, self_pid),
            emergency_suspend: cfg.emergency_suspend,
            critical_guard_mode: cfg.critical_guard_mode,
            suspend_escalation_streak: cfg.suspend_escalation_streak.max(1),
            disk_lock_enabled: cfg.disk_lock_enabled,
            mem_lock_enabled: cfg.mem_lock_enabled,
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
        mem_lock: MemLockMode,
        hard_pressure_streak: u32,
        thermal: ThermalLevel,
    ) -> ActionPlan {
        let focus_proc = sample
            .focus_pid
            .and_then(|fp| sample.processes.iter().find(|p| p.pid == fp));
        let focus_profile = classify_focus_profile(focus_proc);
        let allow_force_pause =
            self.emergency_suspend && hard_pressure_streak >= self.suspend_escalation_streak;
        let qos = plan_qos(
            self.critical_guard_mode,
            band,
            disk_lock,
            mem_lock,
            focus_profile,
            thermal,
            allow_force_pause,
        );

        let mut plan = ActionPlan {
            disk_lock,
            mem_lock,
            qos,
            ..Default::default()
        };

        let disk_active = self.disk_lock_enabled && disk_lock != DiskLockMode::Off;
        let mem_active = self.mem_lock_enabled && mem_lock != MemLockMode::Off;
        let need_actions = matches!(band, PressureBand::Throttle | PressureBand::Emergency)
            || disk_active
            || mem_active;

        if band == PressureBand::Normal && !disk_active && !mem_active {
            return plan;
        }
        // Warn: boost focus + EcoQoS demote top background offenders (no WS wipe).
        if band == PressureBand::Warn && !disk_active && !mem_active {
            plan.boost_foreground = true;
            let focus_tree = focus_tree_pids(sample, sample.focus_pid);
            let mut offenders: Vec<&ProcessSample> = sample
                .processes
                .iter()
                .filter(|p| !self.protected.is_protected(p))
                .filter(|p| !focus_tree.contains(&p.pid))
                .filter(|p| p.cpu_percent >= 8.0)
                .collect();
            offenders.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for proc in offenders.iter().take(self.max_actions.min(4)) {
                plan.actions.push(PlannedAction {
                    pid: proc.pid,
                    name: proc.name.clone(),
                    level: ThrottleLevel::BelowNormal,
                    apply_job_cap: false,
                    apply_disk_lock: false,
                    apply_mem_lock: false,
                    apply_ecoqos: true,
                    apply_mem_priority_low: false,
                    reason: "pressure:warn:ecoqos".into(),
                });
            }
            return plan;
        }
        if !need_actions {
            return plan;
        }

        plan.boost_foreground = true;

        let focus_tree = focus_tree_pids(sample, sample.focus_pid);

        // QoS → Windows soft ladder: Utility → BelowNormal; Background → Idle.
        let soft_level = match qos.background.to_throttle_level() {
            ThrottleLevel::None => ThrottleLevel::BelowNormal,
            level => level,
        };

        let apply_disk = disk_active;
        let apply_mem = mem_active;
        let reason_base = if apply_mem && (!apply_disk || mem_lock == MemLockMode::Hard) {
            match mem_lock {
                MemLockMode::Hard => "mem_lock:hard".to_string(),
                MemLockMode::Soft => "mem_lock:soft".to_string(),
                MemLockMode::Off => unreachable!(),
            }
        } else if apply_disk {
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
            .filter(|p| !focus_tree.contains(&p.pid))
            .collect();

        if apply_mem && (!apply_disk || mem_lock == MemLockMode::Hard) {
            offenders.retain(|p| p.memory_bytes >= 32 * 1024 * 1024);
            offenders.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        } else if apply_disk {
            offenders.sort_by(|a, b| {
                disk_weight(b)
                    .partial_cmp(&disk_weight(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            offenders.retain(|p| p.cpu_percent >= 5.0 || disk_bytes(p) > 1_000_000);
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
                apply_mem_lock: apply_mem,
                apply_ecoqos: true,
                apply_mem_priority_low: apply_mem,
                reason: reason_base.clone(),
            });
        }

        let hard_pressure = band == PressureBand::Emergency
            || disk_lock == DiskLockMode::Hard
            || mem_lock == MemLockMode::Hard;
        let do_suspend = hard_pressure
            && self.emergency_suspend
            && self.critical_guard_mode == CriticalGuardMode::LastResortSuspend
            && hard_pressure_streak >= self.suspend_escalation_streak
            && thermal != ThermalLevel::Serious
            && qos.nap == NapPolicy::ForcePause;

        if do_suspend {
            let mut suspend_count = 0usize;
            for proc in offenders.iter() {
                if suspend_count >= self.max_suspend_pids {
                    break;
                }
                if self.protected.is_protected(proc) || focus_tree.contains(&proc.pid) {
                    continue;
                }
                let reason = if apply_mem && mem_lock == MemLockMode::Hard {
                    "mem_lock:hard".to_string()
                } else if apply_disk {
                    "disk_lock:hard".to_string()
                } else {
                    format!("{reason_base}:suspend")
                };
                if let Some(existing) = plan.actions.iter_mut().find(|a| a.pid == proc.pid) {
                    existing.level = ThrottleLevel::Suspend;
                    existing.apply_disk_lock = apply_disk || existing.apply_disk_lock;
                    existing.apply_mem_lock = apply_mem || existing.apply_mem_lock;
                    existing.apply_ecoqos = true;
                    existing.apply_mem_priority_low =
                        apply_mem || existing.apply_mem_priority_low;
                    existing.reason = reason;
                } else {
                    plan.actions.push(PlannedAction {
                        pid: proc.pid,
                        name: proc.name.clone(),
                        level: ThrottleLevel::Suspend,
                        apply_job_cap: false,
                        apply_disk_lock: apply_disk,
                        apply_mem_lock: apply_mem,
                        apply_ecoqos: true,
                        apply_mem_priority_low: apply_mem,
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
            focus_pid: None,
            disk_latency_sec: 0.0,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 0.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            on_battery: false,
            battery_percent: None,
            cooling_mode: Default::default(),
            cpu_mhz_ratio: 1.0,
            thermal_level: Default::default(),
            processes: procs,
        }
    }

    fn last_resort_cfg() -> GuardianConfig {
        let mut cfg = GuardianConfig::default();
        cfg.critical_guard_mode = CriticalGuardMode::LastResortSuspend;
        cfg.suspend_escalation_streak = 3;
        cfg
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
        let plan = engine.plan(PressureBand::Throttle, &sample, None, DiskLockMode::Off, MemLockMode::Off, 0, ThermalLevel::Nominal);
        assert!(plan.actions.iter().all(|a| a.pid != 4));
        assert!(plan.actions.iter().any(|a| a.pid == 100));
        assert!(plan.boost_foreground);
        assert!(plan.actions.iter().all(|a| a.level == ThrottleLevel::BelowNormal));
    }

    #[test]
    fn cursor_protected_by_name_without_path() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 7,
                parent_pid: 1,
                name: "Cursor.exe".into(),
                path: None, // v0.1.2 path gate may omit path when idle
                cpu_percent: 2.0,
                memory_bytes: 2_000_000_000,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 8,
                parent_pid: 1,
                name: "memhog.exe".into(),
                path: Some(r"C:\temp\memhog.exe".into()),
                cpu_percent: 1.0,
                memory_bytes: 1_500_000_000,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(
            PressureBand::Warn,
            &sample,
            None,
            DiskLockMode::Off,
            MemLockMode::Soft,
            0,
            ThermalLevel::Nominal,
        );
        assert!(
            plan.actions.iter().all(|a| a.pid != 7),
            "Cursor must not receive mem_lock without path"
        );
        assert!(plan.actions.iter().any(|a| a.pid == 8 && a.apply_mem_lock));
    }

    #[test]
    fn msmpeng_never_mem_locked() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 9,
            parent_pid: 1,
            name: "MsMpEng.exe".into(),
            path: Some(r"C:\ProgramData\Microsoft\Windows Defender\Platform\MsMpEng.exe".into()),
            cpu_percent: 40.0,
            memory_bytes: 800_000_000,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let plan = engine.plan(
            PressureBand::Throttle,
            &sample,
            None,
            DiskLockMode::Soft,
            MemLockMode::Hard,
            0,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().all(|a| a.pid != 9));
    }

    #[test]
    fn warn_boosts_and_ecoqos() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 100,
            parent_pid: 1,
            name: "heavy.exe".into(),
            path: Some(r"C:\temp\heavy.exe".into()),
            cpu_percent: 80.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let plan = engine.plan(PressureBand::Warn, &sample, None, DiskLockMode::Off, MemLockMode::Off, 0, ThermalLevel::Nominal);
        assert!(plan.boost_foreground);
        assert!(plan.actions.iter().any(|a| {
            a.pid == 100 && a.apply_ecoqos && !a.apply_mem_lock && !a.apply_disk_lock
        }));
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn mem_soft_sets_mem_priority_flag() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 21,
            parent_pid: 1,
            name: "hog.exe".into(),
            path: Some(r"C:\temp\hog.exe".into()),
            cpu_percent: 1.0,
            memory_bytes: 400_000_000,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let plan = engine.plan(
            PressureBand::Warn,
            &sample,
            None,
            DiskLockMode::Off,
            MemLockMode::Soft,
            0,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().any(|a| {
            a.pid == 21 && a.apply_mem_lock && a.apply_mem_priority_low && a.apply_ecoqos
        }));
    }

    #[test]
    fn soft_only_never_plans_suspend() {
        let cfg = GuardianConfig::default();
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 300,
            parent_pid: 1,
            name: "burn.exe".into(),
            path: Some(r"C:\temp\burn.exe".into()),
            cpu_percent: 95.0,
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
            MemLockMode::Off,
            99,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().any(|a| a.pid == 300 && a.level == ThrottleLevel::Idle));
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn last_resort_suspends_only_after_streak() {
        let cfg = last_resort_cfg();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 300,
            parent_pid: 1,
            name: "burn.exe".into(),
            path: Some(r"C:\temp\burn.exe".into()),
            cpu_percent: 95.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let early = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("commit_charge"),
            DiskLockMode::Off,
            MemLockMode::Off,
            2,
            ThermalLevel::Nominal,
        );
        assert!(early.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));

        let ready = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("commit_charge"),
            DiskLockMode::Off,
            MemLockMode::Off,
            3,
            ThermalLevel::Nominal,
        );
        assert!(ready
            .actions
            .iter()
            .any(|a| a.pid == 300 && a.level == ThrottleLevel::Suspend));
    }

    #[test]
    fn focus_pid_never_suspended_or_throttled() {
        let cfg = last_resort_cfg();
        let engine = PolicyEngine::new(&cfg, 1);
        let mut sample = sample_with(vec![
            ProcessSample {
                pid: 700,
                parent_pid: 1,
                name: "game.exe".into(),
                path: Some(r"C:\Games\game.exe".into()),
                cpu_percent: 99.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 50_000_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 701,
                parent_pid: 700,
                name: "game-helper.exe".into(),
                path: Some(r"C:\Games\game-helper.exe".into()),
                cpu_percent: 40.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 10_000_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 800,
                parent_pid: 1,
                name: "burn.exe".into(),
                path: Some(r"C:\temp\burn.exe".into()),
                cpu_percent: 90.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 5_000_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        sample.focus_pid = Some(700);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("disk_busy_hard"),
            DiskLockMode::Hard,
            MemLockMode::Off,
            5,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().all(|a| a.pid != 700 && a.pid != 701));
        assert!(plan
            .actions
            .iter()
            .any(|a| a.pid == 800 && a.level == ThrottleLevel::Suspend));
        assert!(plan.boost_foreground);
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
        let plan = engine.plan(PressureBand::Throttle, &sample, None, DiskLockMode::Off, MemLockMode::Off, 0, ThermalLevel::Nominal);
        assert!(plan.actions[0].apply_job_cap);
    }

    #[test]
    fn browsers_and_shells_never_suspended() {
        let cfg = last_resort_cfg();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 501,
                parent_pid: 1,
                name: "chrome.exe".into(),
                path: Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe".into()),
                cpu_percent: 99.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 50_000_000,
                disk_write_bytes_per_sec: 50_000_000,
                cmd_line: None,
            },
            ProcessSample {
                pid: 502,
                parent_pid: 1,
                name: "WindowsTerminal.exe".into(),
                path: None,
                cpu_percent: 40.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 10_000_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 503,
                parent_pid: 1,
                name: "powershell.exe".into(),
                path: None,
                cpu_percent: 80.0,
                memory_bytes: 0,
                disk_read_bytes_per_sec: 5_000_000,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("disk_busy_hard"),
            DiskLockMode::Hard,
            MemLockMode::Off,
            5,
            ThermalLevel::Nominal,
        );
        assert!(
            plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend
                || ![501, 502, 503].contains(&a.pid)),
            "browsers/shells must never receive Suspend: {:?}",
            plan.actions
        );
        assert!(plan.actions.iter().all(|a| ![501, 502, 503].contains(&a.pid)));
    }

    #[test]
    fn whitelist_never_suspended() {
        let mut cfg = last_resort_cfg();
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
            MemLockMode::Off,
            5,
            ThermalLevel::Nominal,
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
        let plan = engine.plan(PressureBand::Normal, &sample, None, DiskLockMode::Soft, MemLockMode::Off, 0, ThermalLevel::Nominal);
        assert!(plan.actions.iter().any(|a| a.pid == 10 && a.apply_disk_lock));
        assert_eq!(plan.actions[0].pid, 10);
        assert_eq!(plan.actions[0].reason, "disk_lock:soft");
        assert_eq!(plan.actions[0].level, ThrottleLevel::BelowNormal);
        assert!(!plan
            .actions
            .iter()
            .any(|a| a.level == ThrottleLevel::Suspend));
    }

    #[test]
    fn disk_lock_hard_soft_only_idles_not_suspends() {
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
            MemLockMode::Off,
            5,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().any(|a| {
            a.pid == 50 && a.level == ThrottleLevel::Idle && a.apply_disk_lock
        }));
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn serious_thermal_suppresses_suspend() {
        let cfg = last_resort_cfg();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 300,
            parent_pid: 1,
            name: "burn.exe".into(),
            path: Some(r"C:\temp\burn.exe".into()),
            cpu_percent: 95.0,
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
            MemLockMode::Off,
            5,
            ThermalLevel::Serious,
        );
        assert!(plan.actions.iter().any(|a| a.pid == 300));
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn classify_dev_and_play() {
        let cursor = ProcessSample {
            pid: 1,
            parent_pid: 0,
            name: "Cursor.exe".into(),
            path: Some(r"C:\Users\x\AppData\Local\Programs\cursor\Cursor.exe".into()),
            cpu_percent: 0.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        };
        assert_eq!(classify_focus_profile(Some(&cursor)), FocusProfile::Dev);
        let steam = ProcessSample {
            pid: 2,
            parent_pid: 0,
            name: "steam.exe".into(),
            path: Some(r"C:\Program Files (x86)\Steam\steam.exe".into()),
            cpu_percent: 0.0,
            memory_bytes: 0,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        };
        assert_eq!(classify_focus_profile(Some(&steam)), FocusProfile::Play);
    }

    #[test]
    fn mem_lock_soft_ranks_by_rss() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![
            ProcessSample {
                pid: 20,
                parent_pid: 1,
                name: "small.exe".into(),
                path: Some(r"C:\temp\small.exe".into()),
                cpu_percent: 1.0,
                memory_bytes: 40 * 1024 * 1024,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
            ProcessSample {
                pid: 21,
                parent_pid: 1,
                name: "hog.exe".into(),
                path: Some(r"C:\temp\hog.exe".into()),
                cpu_percent: 1.0,
                memory_bytes: 800 * 1024 * 1024,
                disk_read_bytes_per_sec: 0,
                disk_write_bytes_per_sec: 0,
                cmd_line: None,
            },
        ]);
        let plan = engine.plan(
            PressureBand::Normal,
            &sample,
            None,
            DiskLockMode::Off,
            MemLockMode::Soft,
            0,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().any(|a| a.pid == 21 && a.apply_mem_lock));
        assert_eq!(plan.actions[0].pid, 21);
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }

    #[test]
    fn mem_lock_soft_only_never_suspends() {
        let cfg = GuardianConfig::default();
        assert_eq!(cfg.critical_guard_mode, CriticalGuardMode::SoftOnly);
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = sample_with(vec![ProcessSample {
            pid: 22,
            parent_pid: 1,
            name: "hog.exe".into(),
            path: Some(r"C:\temp\hog.exe".into()),
            cpu_percent: 5.0,
            memory_bytes: 900 * 1024 * 1024,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
            cmd_line: None,
        }]);
        let plan = engine.plan(
            PressureBand::Emergency,
            &sample,
            Some("mem_lock_hard"),
            DiskLockMode::Off,
            MemLockMode::Hard,
            10,
            ThermalLevel::Nominal,
        );
        assert!(plan.actions.iter().any(|a| a.apply_mem_lock));
        assert!(plan.actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }
}
