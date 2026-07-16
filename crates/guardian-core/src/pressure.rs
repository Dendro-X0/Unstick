use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressureBand {
    #[default]
    Normal,
    Warn,
    Throttle,
    Emergency,
}

impl PressureBand {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Warn => "warn",
            Self::Throttle => "throttle",
            Self::Emergency => "emergency",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskLockMode {
    #[default]
    Off,
    Soft,
    Hard,
}

impl DiskLockMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Soft => "soft",
            Self::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiskLockThresholds {
    pub enabled: bool,
    pub soft_pct: f32,
    pub hard_pct: f32,
    pub soft_queue: f32,
    pub hard_queue: f32,
    pub streak: u32,
    pub calibrated: bool,
    pub peak_io_bps: f32,
    pub saturation: f32,
}

impl Default for DiskLockThresholds {
    fn default() -> Self {
        Self {
            enabled: true,
            soft_pct: 85.0,
            hard_pct: 95.0,
            soft_queue: 4.0,
            hard_queue: 8.0,
            streak: 2,
            calibrated: false,
            peak_io_bps: 0.0,
            saturation: 0.0,
        }
    }
}

impl DiskLockThresholds {
    pub fn from_config(cfg: &crate::config::GuardianConfig) -> Self {
        Self {
            enabled: cfg.disk_lock_enabled,
            soft_pct: cfg.disk_busy_soft_pct.clamp(50.0, 100.0),
            hard_pct: cfg.disk_busy_hard_pct.clamp(60.0, 100.0),
            soft_queue: 4.0,
            hard_queue: 8.0,
            streak: cfg.disk_busy_streak.max(1),
            calibrated: false,
            peak_io_bps: 0.0,
            saturation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PressureInputs {
    pub cpu_percent: f32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_commit_percent: f32,
    pub disk_busy_percent: f32,
    pub disk_queue_length: f32,
    pub hard_faults_per_sec: f32,
}

#[derive(Debug, Clone)]
pub struct PressureState {
    pub raw_score: f32,
    pub score: f32,
    pub band: PressureBand,
    pub tripwire: Option<&'static str>,
    pub disk_lock: DiskLockMode,
}

#[derive(Debug, Clone, Default)]
pub struct HysteresisTracker {
    pub band: PressureBand,
    pub high_disk_queue_streak: u32,
    pub disk_busy_soft_streak: u32,
    pub disk_busy_hard_streak: u32,
}

const W_CPU: f32 = 0.25;
const W_MEM: f32 = 0.30;
const W_DISK: f32 = 0.35;
const W_FAULT: f32 = 0.10;
const EMA_ALPHA: f32 = 0.35;

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn memory_pressure(inp: &PressureInputs) -> f32 {
    if inp.memory_total_bytes == 0 {
        return clamp01(inp.memory_commit_percent / 100.0);
    }
    let used_ratio =
        1.0 - (inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32);
    let commit = clamp01(inp.memory_commit_percent / 100.0);
    clamp01(used_ratio.max(commit))
}

fn disk_pressure(inp: &PressureInputs) -> f32 {
    let busy = clamp01(inp.disk_busy_percent / 100.0);
    let queue = clamp01(inp.disk_queue_length / 8.0);
    busy.max(queue)
}

fn fault_pressure(inp: &PressureInputs) -> f32 {
    clamp01(inp.hard_faults_per_sec / 2000.0)
}

/// Update Disk Lock soft/hard streaks; return current mode.
/// Uses calibrated busy% and queue thresholds (hardware-relative).
pub fn update_disk_lock_streaks(
    inp: &PressureInputs,
    tracker: &mut HysteresisTracker,
    thr: &DiskLockThresholds,
) -> DiskLockMode {
    if !thr.enabled {
        tracker.disk_busy_soft_streak = 0;
        tracker.disk_busy_hard_streak = 0;
        return DiskLockMode::Off;
    }

    let hard_hit = inp.disk_busy_percent >= thr.hard_pct
        || inp.disk_queue_length >= thr.hard_queue
        || thr.saturation >= 0.92;
    let soft_hit = hard_hit
        || inp.disk_busy_percent >= thr.soft_pct
        || inp.disk_queue_length >= thr.soft_queue
        || thr.saturation >= 0.72;

    if hard_hit {
        tracker.disk_busy_hard_streak = tracker.disk_busy_hard_streak.saturating_add(1);
    } else {
        tracker.disk_busy_hard_streak = 0;
    }

    if soft_hit {
        tracker.disk_busy_soft_streak = tracker.disk_busy_soft_streak.saturating_add(1);
    } else {
        tracker.disk_busy_soft_streak = 0;
    }

    if tracker.disk_busy_hard_streak >= thr.streak {
        DiskLockMode::Hard
    } else if tracker.disk_busy_soft_streak >= thr.streak {
        DiskLockMode::Soft
    } else {
        DiskLockMode::Off
    }
}

/// Hard tripwires that force emergency even if EMA lags.
pub fn evaluate_tripwire(inp: &PressureInputs, streak: &mut u32) -> Option<&'static str> {
    evaluate_tripwire_with_disk(inp, streak, DiskLockMode::Off)
}

fn evaluate_tripwire_with_disk(
    inp: &PressureInputs,
    streak: &mut u32,
    disk_lock: DiskLockMode,
) -> Option<&'static str> {
    if disk_lock == DiskLockMode::Hard {
        return Some("disk_busy_hard");
    }
    if inp.disk_queue_length >= 8.0 {
        *streak = streak.saturating_add(1);
    } else {
        *streak = 0;
    }
    if *streak >= 2 {
        return Some("disk_queue");
    }
    if inp.memory_commit_percent >= 95.0 {
        return Some("commit_charge");
    }
    if inp.memory_total_bytes > 0 {
        let avail_ratio =
            inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32;
        if avail_ratio < 0.05 && inp.hard_faults_per_sec >= 500.0 {
            return Some("ram_and_faults");
        }
    }
    None
}

fn band_with_hysteresis(
    score: f32,
    prev: PressureBand,
    tripwire: Option<&'static str>,
) -> PressureBand {
    if tripwire.is_some() {
        return PressureBand::Emergency;
    }
    match prev {
        PressureBand::Emergency => {
            if score < 0.72 {
                if score >= 0.70 {
                    PressureBand::Throttle
                } else if score >= 0.55 {
                    PressureBand::Warn
                } else {
                    PressureBand::Normal
                }
            } else {
                PressureBand::Emergency
            }
        }
        PressureBand::Throttle => {
            if score >= 0.85 {
                PressureBand::Emergency
            } else if score < 0.62 {
                if score >= 0.55 {
                    PressureBand::Warn
                } else {
                    PressureBand::Normal
                }
            } else {
                PressureBand::Throttle
            }
        }
        PressureBand::Warn => {
            if score >= 0.85 {
                PressureBand::Emergency
            } else if score >= 0.70 {
                PressureBand::Throttle
            } else if score < 0.50 {
                PressureBand::Normal
            } else {
                PressureBand::Warn
            }
        }
        PressureBand::Normal => {
            if score >= 0.85 {
                PressureBand::Emergency
            } else if score >= 0.70 {
                PressureBand::Throttle
            } else if score >= 0.55 {
                PressureBand::Warn
            } else {
                PressureBand::Normal
            }
        }
    }
}

pub fn score_pressure(inp: &PressureInputs, prev_ema: Option<f32>) -> PressureState {
    score_pressure_tracked(inp, prev_ema, &mut HysteresisTracker::default(), None)
}

pub fn score_pressure_tracked(
    inp: &PressureInputs,
    prev_ema: Option<f32>,
    tracker: &mut HysteresisTracker,
    disk_thr: Option<&DiskLockThresholds>,
) -> PressureState {
    let thr = disk_thr.copied().unwrap_or_default();
    let disk_lock = update_disk_lock_streaks(inp, tracker, &thr);

    let cpu = clamp01(inp.cpu_percent / 100.0);
    let mem = memory_pressure(inp);
    let disk = disk_pressure(inp);
    let fault = fault_pressure(inp);

    let raw = W_CPU * cpu + W_MEM * mem + W_DISK * disk + W_FAULT * fault;
    let score = match prev_ema {
        Some(prev) => EMA_ALPHA * raw + (1.0 - EMA_ALPHA) * prev,
        None => raw,
    };

    let tripwire =
        evaluate_tripwire_with_disk(inp, &mut tracker.high_disk_queue_streak, disk_lock);
    let band = band_with_hysteresis(score, tracker.band, tripwire);
    tracker.band = band;

    // Soft Disk Lock can coexist with lower bands; hard always implies emergency band via tripwire.
    let disk_lock = if band == PressureBand::Emergency && thr.enabled && disk_lock != DiskLockMode::Off
    {
        // Elevate soft→hard when already in emergency from other causes and disk soft is active
        if disk_lock == DiskLockMode::Soft && inp.disk_busy_percent >= thr.soft_pct {
            DiskLockMode::Hard
        } else {
            disk_lock
        }
    } else if band == PressureBand::Emergency && thr.enabled && inp.disk_busy_percent >= thr.hard_pct
    {
        DiskLockMode::Hard
    } else {
        disk_lock
    };

    PressureState {
        raw_score: raw,
        score,
        band,
        tripwire,
        disk_lock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> PressureInputs {
        PressureInputs {
            cpu_percent: 20.0,
            memory_available_bytes: 8 * 1024 * 1024 * 1024,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            memory_commit_percent: 40.0,
            disk_busy_percent: 10.0,
            disk_queue_length: 0.2,
            hard_faults_per_sec: 10.0,
        }
    }

    #[test]
    fn idle_is_normal() {
        let s = score_pressure(&base(), None);
        assert_eq!(s.band, PressureBand::Normal);
        assert!(s.score < 0.55);
        assert_eq!(s.disk_lock, DiskLockMode::Off);
    }

    #[test]
    fn saturated_disk_and_ram_is_emergency() {
        let mut inp = base();
        inp.cpu_percent = 95.0;
        inp.memory_available_bytes = 200 * 1024 * 1024;
        inp.memory_commit_percent = 95.0;
        inp.disk_busy_percent = 100.0;
        inp.disk_queue_length = 12.0;
        inp.hard_faults_per_sec = 3000.0;
        let s = score_pressure(&inp, None);
        assert_eq!(s.band, PressureBand::Emergency);
        assert!(s.tripwire.is_some());
    }

    #[test]
    fn ema_smooths_spike() {
        let idle = score_pressure(&base(), None);
        let mut spike = base();
        spike.cpu_percent = 100.0;
        spike.disk_busy_percent = 100.0;
        spike.disk_queue_length = 10.0;
        let once = score_pressure(&spike, Some(idle.score));
        assert!(once.score < score_pressure(&spike, None).score);
    }

    #[test]
    fn hysteresis_holds_emergency_until_exit() {
        let mut tracker = HysteresisTracker {
            band: PressureBand::Emergency,
            high_disk_queue_streak: 0,
            disk_busy_soft_streak: 0,
            disk_busy_hard_streak: 0,
        };
        let mut inp = base();
        inp.cpu_percent = 80.0;
        inp.disk_busy_percent = 80.0;
        inp.disk_queue_length = 4.0;
        let s = score_pressure_tracked(&inp, Some(0.80), &mut tracker, None);
        assert_eq!(s.band, PressureBand::Emergency);

        let quiet = score_pressure_tracked(&base(), Some(0.40), &mut tracker, None);
        assert_ne!(quiet.band, PressureBand::Emergency);
    }

    #[test]
    fn disk_queue_tripwire_needs_two_samples() {
        let mut streak = 0;
        let mut inp = base();
        inp.disk_queue_length = 9.0;
        assert!(evaluate_tripwire(&inp, &mut streak).is_none());
        assert_eq!(streak, 1);
        assert_eq!(evaluate_tripwire(&inp, &mut streak), Some("disk_queue"));
    }

    #[test]
    fn commit_charge_tripwire() {
        let mut streak = 0;
        let mut inp = base();
        inp.memory_commit_percent = 96.0;
        assert_eq!(evaluate_tripwire(&inp, &mut streak), Some("commit_charge"));
    }

    #[test]
    fn disk_busy_soft_needs_streak() {
        let thr = DiskLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        inp.disk_busy_percent = 90.0;
        let once = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr));
        assert_eq!(once.disk_lock, DiskLockMode::Off);
        let twice = score_pressure_tracked(&inp, Some(once.score), &mut tracker, Some(&thr));
        assert_eq!(twice.disk_lock, DiskLockMode::Soft);
        assert_ne!(twice.band, PressureBand::Emergency);
    }

    #[test]
    fn disk_busy_hard_forces_emergency() {
        let thr = DiskLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        inp.disk_busy_percent = 98.0;
        let _ = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr));
        let twice = score_pressure_tracked(&inp, Some(0.3), &mut tracker, Some(&thr));
        assert_eq!(twice.disk_lock, DiskLockMode::Hard);
        assert_eq!(twice.band, PressureBand::Emergency);
        assert_eq!(twice.tripwire, Some("disk_busy_hard"));
    }
}
