//! Soft-axis closed-loop control (Option 2 / D3 disk + D4 memory).
//!
//! Bang-bang with hysteresis around envelope `u_set_lo`..=`u_set_hi` (freeze-safe headroom).
//! Soft actuators only — never NtSuspend. Memory WS trim is paging-gated (L4).
//! Release is biased: demotions clear quickly when load eases; soft TTL elsewhere.
//! Stress headroom: hard latency / Disk|Mem Hard / paging / thermal-power → lower band.
//! v0.6: Idle (intensity 3) only after sustained cliff streak at intensity 2.

use serde::{Deserialize, Serialize};

use crate::advisory::{CoolingMode, ThermalLevel};
use crate::policy::{PlannedAction, PolicyEngine};
use crate::types::{SystemSample, ThrottleLevel};

/// Soft ceiling without Idle gate (EcoQoS + I/O / mem-prio) — v0.5 default max.
pub const SOFT_CEILING: u8 = 2;
/// Efficiency Mode Idle ceiling when Idle-under-stress gate is open (v0.6).
pub const IDLE_CEILING: u8 = 3;
/// Minimum ticks between *escalation* steps (anti-chatter).
const MIN_HOLD_ESCALATE: u32 = 2;
/// When stressed (latency / Hard lock / paging / thermal), shift band down for more headroom.
pub const STRESS_BAND_SHIFT: f32 = 0.12;
/// Clear all intensity when u falls this fraction of u_lo.
const FULL_RELEASE_RATIO: f32 = 0.70;
const DEFAULT_IDLE_ESCALATE_STREAK: u32 = 4;
const DEFAULT_IDLE_RELEASE_STREAK: u32 = 2;

/// Thermal/power constraint → demand more headroom (same band shift as disk/paging stress).
pub fn thermal_power_stress(thermal: ThermalLevel, cooling: CoolingMode) -> bool {
    matches!(thermal, ThermalLevel::Fair | ThermalLevel::Serious)
        || cooling == CoolingMode::Passive
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DiskControlMode {
    #[default]
    Released,
    Holding,
    Capping,
}

impl DiskControlMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Released => "released",
            Self::Holding => "holding",
            Self::Capping => "capping",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskControlState {
    pub intensity: u8,
    pub mode: DiskControlMode,
    pub u_disk: f32,
    pub u_set_lo: f32,
    pub u_set_hi: f32,
}

impl Default for DiskControlState {
    fn default() -> Self {
        Self {
            intensity: 0,
            mode: DiskControlMode::Released,
            u_disk: 0.0,
            u_set_lo: 0.80,
            u_set_hi: 0.88,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskControlLoop {
    intensity: u8,
    hold_ticks: u32,
    mode: DiskControlMode,
    enabled: bool,
    /// Allow intensity 3 (Idle) after cliff streak at intensity 2.
    idle_under_stress_enabled: bool,
    idle_escalate_streak: u32,
    idle_release_streak: u32,
    /// Consecutive ticks at intensity ≥2 while cliff is true.
    at_i2_cliff_ticks: u32,
    /// Consecutive ticks with cliff false while intensity == 3.
    no_cliff_ticks: u32,
}

impl Default for DiskControlLoop {
    fn default() -> Self {
        Self {
            intensity: 0,
            hold_ticks: 0,
            mode: DiskControlMode::Released,
            enabled: true,
            idle_under_stress_enabled: true,
            idle_escalate_streak: DEFAULT_IDLE_ESCALATE_STREAK,
            idle_release_streak: DEFAULT_IDLE_RELEASE_STREAK,
            at_i2_cliff_ticks: 0,
            no_cliff_ticks: 0,
        }
    }
}

impl DiskControlLoop {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_intensity();
        }
    }

    pub fn set_idle_under_stress(&mut self, enabled: bool) {
        self.idle_under_stress_enabled = enabled;
        if !enabled && self.intensity > SOFT_CEILING {
            self.intensity = SOFT_CEILING;
            self.at_i2_cliff_ticks = 0;
            self.no_cliff_ticks = 0;
        }
    }

    pub fn set_idle_streaks(&mut self, escalate: u32, release: u32) {
        self.idle_escalate_streak = escalate.max(1);
        self.idle_release_streak = release.max(1);
    }

    pub fn intensity(&self) -> u8 {
        self.intensity
    }

    fn clear_intensity(&mut self) {
        self.intensity = 0;
        self.hold_ticks = 0;
        self.mode = DiskControlMode::Released;
        self.at_i2_cliff_ticks = 0;
        self.no_cliff_ticks = 0;
    }

    fn ceiling(&self) -> u8 {
        if self.idle_under_stress_enabled
            && self.at_i2_cliff_ticks >= self.idle_escalate_streak
        {
            IDLE_CEILING
        } else {
            SOFT_CEILING
        }
    }

    /// Step the bang-bang controller.
    /// `stress`: band shift (latency Hard / locks / paging / thermal).
    /// `cliff`: OS-disk / paging cliff for Idle-under-stress gate (not thermal alone).
    pub fn step(
        &mut self,
        u_disk: f32,
        u_set_lo: f32,
        u_set_hi: f32,
        stress: bool,
        cliff: bool,
    ) -> DiskControlState {
        let mut u_lo = u_set_lo.min(u_set_hi);
        let mut u_hi = u_set_lo.max(u_set_hi);
        if stress {
            u_lo = (u_lo - STRESS_BAND_SHIFT).max(0.45);
            u_hi = (u_hi - STRESS_BAND_SHIFT).max(u_lo + 0.04);
        }

        if !self.enabled {
            self.clear_intensity();
            return DiskControlState {
                intensity: 0,
                mode: DiskControlMode::Released,
                u_disk,
                u_set_lo: u_lo,
                u_set_hi: u_hi,
            };
        }

        // Hold only blocks escalation; release is always allowed.
        let holding_escalate = self.hold_ticks > 0;
        if self.hold_ticks > 0 {
            self.hold_ticks -= 1;
        }

        // Idle gate bookkeeping.
        if self.intensity >= SOFT_CEILING && cliff && self.idle_under_stress_enabled {
            self.at_i2_cliff_ticks = self.at_i2_cliff_ticks.saturating_add(1);
            self.no_cliff_ticks = 0;
        } else if self.intensity >= IDLE_CEILING && !cliff {
            self.no_cliff_ticks = self.no_cliff_ticks.saturating_add(1);
            if self.no_cliff_ticks >= self.idle_release_streak {
                self.intensity = SOFT_CEILING;
                self.at_i2_cliff_ticks = 0;
                self.no_cliff_ticks = 0;
            }
        } else if !cliff {
            self.at_i2_cliff_ticks = 0;
            self.no_cliff_ticks = 0;
        }

        let max_i = self.ceiling();

        if u_disk < u_lo * FULL_RELEASE_RATIO {
            self.clear_intensity();
        } else if u_disk < u_lo {
            if self.intensity > 0 {
                self.intensity -= 1;
            }
            if self.intensity < SOFT_CEILING {
                self.at_i2_cliff_ticks = 0;
            }
            self.mode = if self.intensity == 0 {
                DiskControlMode::Released
            } else {
                DiskControlMode::Holding
            };
        } else if u_disk > u_hi {
            if !holding_escalate && self.intensity < max_i {
                self.intensity += 1;
                self.hold_ticks = MIN_HOLD_ESCALATE;
            }
            self.mode = DiskControlMode::Capping;
        } else {
            self.mode = if self.intensity == 0 {
                DiskControlMode::Released
            } else {
                DiskControlMode::Holding
            };
        }

        // If Idle disabled mid-flight, clamp.
        if !self.idle_under_stress_enabled && self.intensity > SOFT_CEILING {
            self.intensity = SOFT_CEILING;
        }

        self.state(u_disk, u_lo, u_hi)
    }

    /// Backward-compatible step without cliff (Idle gate never opens).
    pub fn step_simple(&mut self, u_disk: f32, u_set_lo: f32, u_set_hi: f32) -> DiskControlState {
        self.step(u_disk, u_set_lo, u_set_hi, false, false)
    }

    fn state(&self, u_disk: f32, u_lo: f32, u_hi: f32) -> DiskControlState {
        DiskControlState {
            intensity: self.intensity,
            mode: self.mode,
            u_disk,
            u_set_lo: u_lo,
            u_set_hi: u_hi,
        }
    }
}

/// Rank disk offenders and map intensity → soft PlannedAction (no Suspend).
pub fn plan_disk_control_actions(
    engine: &PolicyEngine,
    sample: &SystemSample,
    intensity: u8,
) -> Vec<PlannedAction> {
    if intensity == 0 {
        return Vec::new();
    }

    let focus_tree = crate::policy::focus_tree_pids(sample, sample.focus_pid);
    let mut offenders: Vec<&crate::types::ProcessSample> = sample
        .processes
        .iter()
        .filter(|p| !engine.protected.is_protected(p))
        .filter(|p| !focus_tree.contains(&p.pid))
        .collect();
    offenders.sort_by(|a, b| {
        let da = a.disk_read_bytes_per_sec.saturating_add(a.disk_write_bytes_per_sec);
        let db = b.disk_read_bytes_per_sec.saturating_add(b.disk_write_bytes_per_sec);
        db.cmp(&da).then_with(|| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let (level, apply_disk, reason) = match intensity {
        1 => (
            ThrottleLevel::BelowNormal,
            false,
            "disk_control:ecoqos",
        ),
        2 => (ThrottleLevel::BelowNormal, true, "disk_control:io_verylow"),
        3 => (ThrottleLevel::Idle, true, "disk_control:efficiency_idle"),
        _ => (ThrottleLevel::Idle, true, "disk_control:idle"),
    };

    offenders
        .iter()
        .take(engine.max_actions)
        .map(|proc| PlannedAction {
            pid: proc.pid,
            name: proc.name.clone(),
            level,
            apply_job_cap: false,
            apply_disk_lock: apply_disk,
            apply_mem_lock: false,
            apply_ecoqos: true,
            apply_mem_priority_low: false,
            reason: reason.into(),
        })
        .collect()
}

/// Merge soft-control actions into an existing plan (stronger level / flags win).
pub fn merge_control_actions(plan: &mut crate::policy::ActionPlan, extras: Vec<PlannedAction>) {
    for extra in extras {
        if let Some(existing) = plan.actions.iter_mut().find(|a| a.pid == extra.pid) {
            existing.level = max_soft_level(existing.level, extra.level);
            existing.apply_disk_lock = existing.apply_disk_lock || extra.apply_disk_lock;
            existing.apply_mem_lock = existing.apply_mem_lock || extra.apply_mem_lock;
            existing.apply_ecoqos = existing.apply_ecoqos || extra.apply_ecoqos;
            existing.apply_mem_priority_low =
                existing.apply_mem_priority_low || extra.apply_mem_priority_low;
            if extra.reason.starts_with("disk_control:")
                || extra.reason.starts_with("mem_control:")
            {
                existing.reason = extra.reason;
            }
        } else {
            plan.actions.push(extra);
        }
    }
}

/// Alias for D3 call sites.
pub fn merge_disk_control(plan: &mut crate::policy::ActionPlan, extras: Vec<PlannedAction>) {
    merge_control_actions(plan, extras);
}

/// Rank RSS offenders; WS trim (`apply_mem_lock`) only when `paging` is true (L4 gate).
pub fn plan_mem_control_actions(
    engine: &PolicyEngine,
    sample: &SystemSample,
    intensity: u8,
    paging: bool,
) -> Vec<PlannedAction> {
    if intensity == 0 {
        return Vec::new();
    }

    let focus_tree = crate::policy::focus_tree_pids(sample, sample.focus_pid);
    let mut offenders: Vec<&crate::types::ProcessSample> = sample
        .processes
        .iter()
        .filter(|p| !engine.protected.is_protected(p))
        .filter(|p| !focus_tree.contains(&p.pid))
        .collect();
    offenders.sort_by(|a, b| {
        b.memory_bytes.cmp(&a.memory_bytes).then_with(|| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    // Without paging evidence: EcoQoS + MEMORY_PRIORITY_LOW only (no EmptyWorkingSet / Hard shrink).
    let apply_ws = intensity >= 2 && paging;
    let (level, reason) = match intensity {
        1 => (ThrottleLevel::BelowNormal, "mem_control:ecoqos_memprio"),
        2 => (
            ThrottleLevel::BelowNormal,
            if apply_ws {
                "mem_control:ws_soft"
            } else {
                "mem_control:memprio_only"
            },
        ),
        3 => (ThrottleLevel::Idle, "mem_control:efficiency_idle"),
        _ => (
            ThrottleLevel::Idle,
            if apply_ws {
                "mem_control:ws_idle"
            } else {
                "mem_control:idle_memprio"
            },
        ),
    };

    offenders
        .iter()
        .take(engine.max_actions)
        .map(|proc| PlannedAction {
            pid: proc.pid,
            name: proc.name.clone(),
            level,
            apply_job_cap: false,
            apply_disk_lock: false,
            apply_mem_lock: apply_ws,
            apply_ecoqos: true,
            apply_mem_priority_low: true,
            reason: reason.into(),
        })
        .collect()
}

/// Same bang-bang loop as disk (D4).
pub type MemControlLoop = DiskControlLoop;
pub type MemControlMode = DiskControlMode;
pub type MemControlState = DiskControlState;

fn max_soft_level(a: ThrottleLevel, b: ThrottleLevel) -> ThrottleLevel {
    fn rank(l: ThrottleLevel) -> u8 {
        match l {
            ThrottleLevel::None => 0,
            ThrottleLevel::BelowNormal => 1,
            ThrottleLevel::Idle => 2,
            ThrottleLevel::Suspend => 2, // treat as Idle for soft merge; Suspend only from experimental path
        }
    }
    if rank(b) > rank(a) {
        if b == ThrottleLevel::Suspend {
            ThrottleLevel::Idle
        } else {
            b
        }
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GuardianConfig;
    use crate::envelope::{U_SET_HI, U_SET_LO};

    #[test]
    fn escalates_above_setpoint_hi() {
        let mut loop_ = DiskControlLoop::new(true);
        let s = loop_.step(1.05, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 1);
        assert_eq!(s.mode, DiskControlMode::Capping);
        // escalation hold ticks
        let s = loop_.step(1.05, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 1);
        let s = loop_.step(1.05, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 1);
        let s = loop_.step(1.05, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 2);
        assert!(s.intensity <= SOFT_CEILING);
        // Without cliff, Soft ceiling holds — no Idle.
        for _ in 0..20 {
            let s = loop_.step(1.05, U_SET_LO, U_SET_HI, false, false);
            assert_eq!(s.intensity, SOFT_CEILING);
        }
    }

    #[test]
    fn full_release_when_well_below_lo() {
        let mut loop_ = DiskControlLoop::new(true);
        for _ in 0..12 {
            let _ = loop_.step(1.2, U_SET_LO, U_SET_HI, false, false);
        }
        assert_eq!(loop_.intensity(), SOFT_CEILING);
        let s = loop_.step(0.50, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 0);
        assert_eq!(s.mode, DiskControlMode::Released);
    }

    #[test]
    fn releases_stepwise_just_below_lo() {
        let mut loop_ = DiskControlLoop::new(true);
        for _ in 0..12 {
            let _ = loop_.step(1.2, U_SET_LO, U_SET_HI, false, false);
        }
        assert_eq!(loop_.intensity(), SOFT_CEILING);
        let just_under = U_SET_LO * 0.85;
        assert!(just_under >= U_SET_LO * FULL_RELEASE_RATIO);
        let s = loop_.step(just_under, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, SOFT_CEILING - 1);
        let s = loop_.step(just_under, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, SOFT_CEILING - 2);
    }

    #[test]
    fn holds_inside_band() {
        let mut loop_ = DiskControlLoop::new(true);
        let _ = loop_.step(1.1, U_SET_LO, U_SET_HI, false, false);
        let mid = (U_SET_LO + U_SET_HI) * 0.5;
        let s = loop_.step(mid, U_SET_LO, U_SET_HI, false, false);
        assert_eq!(s.intensity, 1);
        assert_eq!(s.mode, DiskControlMode::Holding);
    }

    #[test]
    fn stress_shifts_band_down() {
        let mut loop_ = DiskControlLoop::new(true);
        let s = loop_.step(0.82, U_SET_LO, U_SET_HI, true, false);
        assert_eq!(s.intensity, 1);
        assert_eq!(s.mode, DiskControlMode::Capping);
        assert!(s.u_set_hi < U_SET_HI);
        assert!((s.u_set_lo - (U_SET_LO - STRESS_BAND_SHIFT)).abs() < 0.001);
    }

    #[test]
    fn thermal_power_stress_flags() {
        use crate::advisory::{CoolingMode, ThermalLevel};
        assert!(!thermal_power_stress(
            ThermalLevel::Nominal,
            CoolingMode::Active
        ));
        assert!(thermal_power_stress(
            ThermalLevel::Fair,
            CoolingMode::Active
        ));
        assert!(thermal_power_stress(
            ThermalLevel::Serious,
            CoolingMode::Active
        ));
        assert!(thermal_power_stress(
            ThermalLevel::Nominal,
            CoolingMode::Passive
        ));
    }

    #[test]
    fn thermal_stress_matches_disk_stress_shift() {
        let mut calm = DiskControlLoop::new(true);
        let mut hot = DiskControlLoop::new(true);
        let mid = (U_SET_LO + U_SET_HI) * 0.5;
        let calm_s = calm.step(mid, U_SET_LO, U_SET_HI, false, false);
        let hot_s = hot.step(mid, U_SET_LO, U_SET_HI, true, false);
        assert_eq!(calm_s.intensity, 0);
        assert_eq!(hot_s.intensity, 1);
        assert_eq!(hot_s.mode, DiskControlMode::Capping);
    }

    #[test]
    fn disabled_stays_released() {
        let mut loop_ = DiskControlLoop::new(false);
        let s = loop_.step(2.0, U_SET_LO, U_SET_HI, false, true);
        assert_eq!(s.intensity, 0);
        assert_eq!(s.mode, DiskControlMode::Released);
    }

    #[test]
    fn freeze_safe_setpoints_leave_headroom() {
        assert!((U_SET_LO - 0.80).abs() < f32::EPSILON);
        assert!((U_SET_HI - 0.88).abs() < f32::EPSILON);
        assert!(U_SET_HI < 0.95);
    }

    #[test]
    fn idle_gate_requires_cliff_streak_at_i2() {
        let mut loop_ = DiskControlLoop::new(true);
        loop_.set_idle_streaks(4, 2);
        // Reach Soft ceiling without cliff.
        for _ in 0..12 {
            let _ = loop_.step(1.2, U_SET_LO, U_SET_HI, true, false);
        }
        assert_eq!(loop_.intensity(), SOFT_CEILING);
        // Three cliff ticks — still Soft ceiling (streak 4).
        for _ in 0..3 {
            let s = loop_.step(1.2, U_SET_LO, U_SET_HI, true, true);
            assert_eq!(s.intensity, SOFT_CEILING);
        }
        // Fourth cliff tick opens gate; then escalate through hold to Idle.
        let mut saw_idle = false;
        for _ in 0..8 {
            let s = loop_.step(1.2, U_SET_LO, U_SET_HI, true, true);
            if s.intensity == IDLE_CEILING {
                saw_idle = true;
                break;
            }
        }
        assert!(saw_idle, "expected intensity 3 after cliff streak");
    }

    #[test]
    fn idle_disabled_never_exceeds_soft_ceiling() {
        let mut loop_ = DiskControlLoop::new(true);
        loop_.set_idle_under_stress(false);
        loop_.set_idle_streaks(2, 2);
        for _ in 0..30 {
            let s = loop_.step(1.2, U_SET_LO, U_SET_HI, true, true);
            assert!(s.intensity <= SOFT_CEILING);
        }
    }

    #[test]
    fn idle_releases_when_cliff_clears() {
        let mut loop_ = DiskControlLoop::new(true);
        loop_.set_idle_streaks(2, 2);
        for _ in 0..20 {
            let _ = loop_.step(1.2, U_SET_LO, U_SET_HI, true, true);
            if loop_.intensity() == IDLE_CEILING {
                break;
            }
        }
        assert_eq!(loop_.intensity(), IDLE_CEILING);
        // Stay above hi but cliff false → after release streak, drop to Soft ceiling.
        let s = loop_.step(1.2, U_SET_LO, U_SET_HI, true, false);
        assert_eq!(s.intensity, IDLE_CEILING);
        let s = loop_.step(1.2, U_SET_LO, U_SET_HI, true, false);
        assert_eq!(s.intensity, SOFT_CEILING);
    }

    #[test]
    fn plan_efficiency_idle_is_idle_not_suspend() {
        use crate::types::ProcessSample;
        use chrono::Utc;
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = SystemSample {
            timestamp: Utc::now(),
            cpu_percent: 50.0,
            memory_total_bytes: 8 << 30,
            memory_available_bytes: 4 << 30,
            memory_commit_percent: 40.0,
            disk_busy_percent: 95.0,
            disk_queue_length: 8.0,
            disk_io_bytes_per_sec: 50_000_000,
            hard_faults_per_sec: 0.0,
            focus_pid: Some(10),
            disk_latency_sec: 0.05,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 0.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            on_battery: false,
            battery_percent: None,
            cooling_mode: Default::default(),
            cpu_mhz_ratio: 1.0,
            thermal_level: Default::default(),
            processes: vec![
                ProcessSample {
                    pid: 10,
                    parent_pid: 0,
                    name: "game.exe".into(),
                    path: None,
                    cpu_percent: 40.0,
                    memory_bytes: 0,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 0,
                    cmd_line: None,
                },
                ProcessSample {
                    pid: 20,
                    parent_pid: 0,
                    name: "hog.exe".into(),
                    path: None,
                    cpu_percent: 10.0,
                    memory_bytes: 0,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 80_000_000,
                    cmd_line: None,
                },
            ],
        };
        let actions = plan_disk_control_actions(&engine, &sample, 3);
        assert!(actions.iter().any(|a| a.pid == 20));
        assert!(actions
            .iter()
            .all(|a| a.level == ThrottleLevel::Idle && a.level != ThrottleLevel::Suspend));
        assert!(actions
            .iter()
            .any(|a| a.reason == "disk_control:efficiency_idle" && a.apply_disk_lock));
    }

    #[test]
    fn plan_actions_never_suspend() {
        use crate::types::ProcessSample;
        use chrono::Utc;

        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let sample = SystemSample {
            timestamp: Utc::now(),
            cpu_percent: 50.0,
            memory_total_bytes: 8 << 30,
            memory_available_bytes: 4 << 30,
            memory_commit_percent: 40.0,
            disk_busy_percent: 95.0,
            disk_queue_length: 8.0,
            disk_io_bytes_per_sec: 50_000_000,
            hard_faults_per_sec: 0.0,
            focus_pid: Some(10),
            disk_latency_sec: 0.05,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 0.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            on_battery: false,
            battery_percent: None,
            cooling_mode: Default::default(),
            cpu_mhz_ratio: 1.0,
            thermal_level: Default::default(),
            processes: vec![
                ProcessSample {
                    pid: 10,
                    parent_pid: 0,
                    name: "game.exe".into(),
                    path: None,
                    cpu_percent: 40.0,
                    memory_bytes: 0,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 0,
                    cmd_line: None,
                },
                ProcessSample {
                    pid: 20,
                    parent_pid: 0,
                    name: "hog.exe".into(),
                    path: None,
                    cpu_percent: 10.0,
                    memory_bytes: 0,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 80_000_000,
                    cmd_line: None,
                },
            ],
        };
        let actions = plan_disk_control_actions(&engine, &sample, 3);
        assert!(!actions.is_empty());
        assert!(actions.iter().all(|a| a.pid != 10));
        assert!(actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
        assert!(actions.iter().any(|a| a.pid == 20 && a.apply_disk_lock));
    }

    fn mem_sample(rss_hog: u64) -> SystemSample {
        use crate::types::ProcessSample;
        use chrono::Utc;
        SystemSample {
            timestamp: Utc::now(),
            cpu_percent: 20.0,
            memory_total_bytes: 8 << 30,
            memory_available_bytes: 400 << 20,
            memory_commit_percent: 90.0,
            disk_busy_percent: 10.0,
            disk_queue_length: 0.2,
            disk_io_bytes_per_sec: 1_000_000,
            hard_faults_per_sec: 0.0,
            focus_pid: Some(10),
            disk_latency_sec: 0.002,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 5.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            on_battery: false,
            battery_percent: None,
            cooling_mode: Default::default(),
            cpu_mhz_ratio: 1.0,
            thermal_level: Default::default(),
            processes: vec![
                ProcessSample {
                    pid: 10,
                    parent_pid: 0,
                    name: "Code.exe".into(),
                    path: None,
                    cpu_percent: 5.0,
                    memory_bytes: 2 << 30,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 0,
                    cmd_line: None,
                },
                ProcessSample {
                    pid: 20,
                    parent_pid: 0,
                    name: "mem-hog.exe".into(),
                    path: None,
                    cpu_percent: 1.0,
                    memory_bytes: rss_hog,
                    disk_read_bytes_per_sec: 0,
                    disk_write_bytes_per_sec: 0,
                    cmd_line: None,
                },
            ],
        }
    }

    #[test]
    fn mem_plan_without_paging_skips_ws_trim() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let actions = plan_mem_control_actions(&engine, &mem_sample(3 << 30), 3, false);
        assert!(actions.iter().any(|a| a.pid == 20));
        assert!(actions.iter().all(|a| !a.apply_mem_lock));
        assert!(actions.iter().all(|a| a.apply_mem_priority_low && a.apply_ecoqos));
        assert!(actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
        assert!(actions.iter().all(|a| a.pid != 10));
    }

    #[test]
    fn mem_plan_with_paging_allows_ws_trim() {
        let cfg = GuardianConfig::default();
        let engine = PolicyEngine::new(&cfg, 1);
        let actions = plan_mem_control_actions(&engine, &mem_sample(3 << 30), 3, true);
        assert!(actions.iter().any(|a| a.pid == 20 && a.apply_mem_lock));
        assert!(actions.iter().all(|a| a.level != ThrottleLevel::Suspend));
    }
}
