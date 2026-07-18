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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemLockMode {
    #[default]
    Off,
    Soft,
    Hard,
}

impl MemLockMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Soft => "soft",
            Self::Hard => "hard",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MemLockThresholds {
    pub enabled: bool,
    pub avail_soft_pct: f32,
    pub avail_hard_pct: f32,
    pub commit_soft_pct: f32,
    pub commit_hard_pct: f32,
    pub streak: u32,
    pub hard_requires_paging: bool,
}

impl Default for MemLockThresholds {
    fn default() -> Self {
        Self {
            enabled: true,
            avail_soft_pct: 15.0,
            avail_hard_pct: 8.0,
            commit_soft_pct: 90.0,
            commit_hard_pct: 95.0,
            streak: 2,
            hard_requires_paging: true,
        }
    }
}

impl MemLockThresholds {
    pub fn from_config(cfg: &crate::config::GuardianConfig) -> Self {
        let soft = cfg.mem_avail_soft_pct.clamp(5.0, 40.0);
        let hard = cfg
            .mem_avail_hard_pct
            .clamp(2.0, soft - 0.5)
            .min(soft - 0.5);
        Self {
            enabled: cfg.mem_lock_enabled,
            avail_soft_pct: soft,
            avail_hard_pct: hard.max(2.0),
            commit_soft_pct: cfg.mem_commit_soft_pct.clamp(70.0, 99.0),
            commit_hard_pct: cfg
                .mem_commit_hard_pct
                .max(cfg.mem_commit_soft_pct)
                .clamp(80.0, 99.5),
            streak: cfg.mem_lock_streak.max(1),
            hard_requires_paging: cfg.mem_lock_hard_requires_paging,
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
    pub soft_latency_sec: f32,
    pub hard_latency_sec: f32,
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
            soft_latency_sec: 0.015,
            hard_latency_sec: 0.040,
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
            soft_latency_sec: cfg.disk_latency_soft_sec.clamp(0.001, 1.0),
            hard_latency_sec: cfg
                .disk_latency_hard_sec
                .max(cfg.disk_latency_soft_sec + 0.001)
                .clamp(0.002, 2.0),
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
    /// Avg. Disk sec/Transfer (0 if unknown).
    pub disk_latency_sec: f32,
    pub hard_faults_per_sec: f32,
    pub pagefile_writes_per_sec: f32,
    pub paging_file_pct: f32,
    /// Processor DPC % (optional; boosts cpu_some).
    pub dpc_time_percent: f32,
    /// Processor Interrupt % (optional).
    pub interrupt_time_percent: f32,
    /// Thermal/power stall contribution (from ThermalLevel).
    pub thermal_some: f32,
}

/// PSI-shaped stall fractions (0..1). Linux `/proc/pressure` maps here later.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct StallFractions {
    pub cpu_some: f32,
    pub memory_some: f32,
    pub memory_full: f32,
    pub io_some: f32,
    pub io_full: f32,
    pub thermal_some: f32,
}

#[derive(Debug, Clone)]
pub struct PressureState {
    pub raw_score: f32,
    pub score: f32,
    pub band: PressureBand,
    pub tripwire: Option<&'static str>,
    pub disk_lock: DiskLockMode,
    pub mem_lock: MemLockMode,
    pub stalls: StallFractions,
}

#[derive(Debug, Clone, Default)]
pub struct HysteresisTracker {
    pub band: PressureBand,
    pub high_disk_queue_streak: u32,
    pub disk_busy_soft_streak: u32,
    pub disk_busy_hard_streak: u32,
    pub mem_soft_streak: u32,
    pub mem_hard_streak: u32,
}

const W_CPU: f32 = 0.25;
const W_MEM: f32 = 0.28;
const W_DISK: f32 = 0.32;
const W_FULL: f32 = 0.20;
const W_THERM: f32 = 0.12;
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
    let latency = if inp.disk_latency_sec > 0.0 {
        clamp01(inp.disk_latency_sec / 0.015)
    } else {
        0.0
    };
    busy.max(queue).max(latency)
}

fn fault_pressure(inp: &PressureInputs) -> f32 {
    let raw = clamp01(inp.hard_faults_per_sec / 2000.0);
    if looks_like_mapped_io(inp) {
        raw * 0.15
    } else {
        raw
    }
}

/// Build PSI-shaped stalls from Windows PDH proxies (or future Linux PSI).
pub fn compute_stalls(inp: &PressureInputs, thr: &DiskLockThresholds) -> StallFractions {
    let cpu_util = clamp01(inp.cpu_percent / 100.0);
    let irq_steal = clamp01((inp.dpc_time_percent + inp.interrupt_time_percent) / 100.0);
    let cpu_some = cpu_util.max(irq_steal);

    let memory_some = memory_pressure(inp);
    let memory_full = if paging_pressure_evidence(inp) {
        let avail_stall = if inp.memory_total_bytes > 0 {
            1.0 - (inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32)
        } else {
            clamp01(inp.memory_commit_percent / 100.0)
        };
        clamp01(fault_pressure(inp).max(avail_stall))
    } else {
        0.0
    };

    let io_some = disk_pressure(inp);
    let hard_latency = inp.disk_latency_sec > 0.0 && inp.disk_latency_sec >= thr.hard_latency_sec;
    let busy_and_sat =
        inp.disk_busy_percent >= thr.hard_pct && thr.saturation >= 0.85;
    let io_full = if hard_latency || busy_and_sat {
        1.0
    } else if inp.disk_latency_sec > 0.0 && inp.disk_latency_sec >= thr.soft_latency_sec {
        clamp01(inp.disk_latency_sec / thr.hard_latency_sec.max(0.001))
    } else if thr.saturation >= 0.72 {
        clamp01(thr.saturation)
    } else {
        0.0
    };

    StallFractions {
        cpu_some,
        memory_some,
        memory_full,
        io_some,
        io_full,
        thermal_some: clamp01(inp.thermal_some),
    }
}

pub fn score_from_stalls(stalls: &StallFractions) -> f32 {
    let some =
        W_CPU * stalls.cpu_some + W_MEM * stalls.memory_some + W_DISK * stalls.io_some;
    let full_boost = W_FULL * stalls.memory_full.max(stalls.io_full);
    let therm = W_THERM * stalls.thermal_some;
    clamp01(some + full_boost + therm)
}

/// High Pages/sec with healthy RAM and quiet pagefile ⇒ likely mapped I/O.
pub fn looks_like_mapped_io(inp: &PressureInputs) -> bool {
    if inp.hard_faults_per_sec < 200.0 {
        return false;
    }
    let avail_ok = if inp.memory_total_bytes > 0 {
        (inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32) > 0.12
    } else {
        true
    };
    avail_ok && inp.paging_file_pct < 15.0 && inp.pagefile_writes_per_sec < 50.0
}

/// Evidence that hard faults are associated with real paging / low RAM.
pub fn paging_pressure_evidence(inp: &PressureInputs) -> bool {
    if inp.paging_file_pct >= 20.0 || inp.pagefile_writes_per_sec >= 100.0 {
        return true;
    }
    if inp.memory_total_bytes > 0 {
        let avail = inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32;
        return avail < 0.08;
    }
    false
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
        || (inp.disk_latency_sec > 0.0 && inp.disk_latency_sec >= thr.hard_latency_sec)
        || thr.saturation >= 0.92;
    let soft_hit = hard_hit
        || inp.disk_busy_percent >= thr.soft_pct
        || inp.disk_queue_length >= thr.soft_queue
        || (inp.disk_latency_sec > 0.0 && inp.disk_latency_sec >= thr.soft_latency_sec)
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

fn available_pct(inp: &PressureInputs) -> Option<f32> {
    if inp.memory_total_bytes == 0 {
        return None;
    }
    Some((inp.memory_available_bytes as f32 / inp.memory_total_bytes as f32) * 100.0)
}

/// Update Mem Lock soft/hard streaks; return current mode.
pub fn update_mem_lock_streaks(
    inp: &PressureInputs,
    tracker: &mut HysteresisTracker,
    thr: &MemLockThresholds,
) -> MemLockMode {
    if !thr.enabled {
        tracker.mem_soft_streak = 0;
        tracker.mem_hard_streak = 0;
        return MemLockMode::Off;
    }

    let avail = available_pct(inp);
    let soft_hit = avail.map(|a| a < thr.avail_soft_pct).unwrap_or(false)
        || inp.memory_commit_percent >= thr.commit_soft_pct;
    let hard_raw = avail.map(|a| a < thr.avail_hard_pct).unwrap_or(false)
        || inp.memory_commit_percent >= thr.commit_hard_pct;
    let hard_hit = hard_raw
        && (!thr.hard_requires_paging || paging_pressure_evidence(inp));

    if hard_hit {
        tracker.mem_hard_streak = tracker.mem_hard_streak.saturating_add(1);
    } else {
        tracker.mem_hard_streak = 0;
    }

    if soft_hit || hard_hit {
        tracker.mem_soft_streak = tracker.mem_soft_streak.saturating_add(1);
    } else {
        tracker.mem_soft_streak = 0;
    }

    if tracker.mem_hard_streak >= thr.streak {
        MemLockMode::Hard
    } else if tracker.mem_soft_streak >= thr.streak {
        MemLockMode::Soft
    } else {
        MemLockMode::Off
    }
}

/// Hard tripwires that force emergency even if EMA lags.
pub fn evaluate_tripwire(inp: &PressureInputs, streak: &mut u32) -> Option<&'static str> {
    evaluate_tripwire_with_locks(
        inp,
        streak,
        DiskLockMode::Off,
        MemLockMode::Off,
        &DiskLockThresholds::default(),
    )
}

fn evaluate_tripwire_with_locks(
    inp: &PressureInputs,
    streak: &mut u32,
    disk_lock: DiskLockMode,
    mem_lock: MemLockMode,
    thr: &DiskLockThresholds,
) -> Option<&'static str> {
    if disk_lock == DiskLockMode::Hard {
        if inp.disk_latency_sec > 0.0
            && inp.disk_latency_sec >= thr.hard_latency_sec
            && inp.disk_busy_percent < thr.hard_pct
            && inp.disk_queue_length < thr.hard_queue
        {
            return Some("disk_latency_hard");
        }
        return Some("disk_busy_hard");
    }
    if mem_lock == MemLockMode::Hard {
        return Some("mem_lock_hard");
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
        if avail_ratio < 0.05
            && inp.hard_faults_per_sec >= 500.0
            && paging_pressure_evidence(inp)
        {
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
    score_pressure_tracked(
        inp,
        prev_ema,
        &mut HysteresisTracker::default(),
        None,
        None,
    )
}

pub fn score_pressure_tracked(
    inp: &PressureInputs,
    prev_ema: Option<f32>,
    tracker: &mut HysteresisTracker,
    disk_thr: Option<&DiskLockThresholds>,
    mem_thr: Option<&MemLockThresholds>,
) -> PressureState {
    let thr = disk_thr.copied().unwrap_or_default();
    let mthr = mem_thr.copied().unwrap_or_default();
    let disk_lock = update_disk_lock_streaks(inp, tracker, &thr);
    let mem_lock = update_mem_lock_streaks(inp, tracker, &mthr);

    let stalls = compute_stalls(inp, &thr);
    let raw = score_from_stalls(&stalls);
    let score = match prev_ema {
        Some(prev) => EMA_ALPHA * raw + (1.0 - EMA_ALPHA) * prev,
        None => raw,
    };

    let tripwire = evaluate_tripwire_with_locks(
        inp,
        &mut tracker.high_disk_queue_streak,
        disk_lock,
        mem_lock,
        &thr,
    );
    let band = band_with_hysteresis(score, tracker.band, tripwire);
    tracker.band = band;

    // Soft Disk Lock can coexist with lower bands; hard always implies emergency band via tripwire.
    let disk_lock = if band == PressureBand::Emergency && thr.enabled && disk_lock != DiskLockMode::Off
    {
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
        mem_lock,
        stalls,
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
            disk_latency_sec: 0.0,
            hard_faults_per_sec: 10.0,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 5.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            thermal_some: 0.0,
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
            ..Default::default()
        };
        let mut inp = base();
        inp.cpu_percent = 80.0;
        inp.disk_busy_percent = 80.0;
        inp.disk_queue_length = 4.0;
        let s = score_pressure_tracked(&inp, Some(0.80), &mut tracker, None, None);
        assert_eq!(s.band, PressureBand::Emergency);

        let quiet = score_pressure_tracked(&base(), Some(0.40), &mut tracker, None, None);
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
        let once = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr), None);
        assert_eq!(once.disk_lock, DiskLockMode::Off);
        let twice = score_pressure_tracked(&inp, Some(once.score), &mut tracker, Some(&thr), None);
        assert_eq!(twice.disk_lock, DiskLockMode::Soft);
        assert_ne!(twice.band, PressureBand::Emergency);
    }

    #[test]
    fn disk_busy_hard_forces_emergency() {
        let thr = DiskLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        inp.disk_busy_percent = 98.0;
        let _ = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr), None);
        let twice = score_pressure_tracked(&inp, Some(0.3), &mut tracker, Some(&thr), None);
        assert_eq!(twice.disk_lock, DiskLockMode::Hard);
        assert_eq!(twice.band, PressureBand::Emergency);
        assert_eq!(twice.tripwire, Some("disk_busy_hard"));
    }

    #[test]
    fn disk_latency_soft_needs_streak() {
        let thr = DiskLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        // High service time, low busy% — classic "queue empty but slow" SSD stall.
        inp.disk_latency_sec = 0.020;
        inp.disk_busy_percent = 30.0;
        let once = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr), None);
        assert_eq!(once.disk_lock, DiskLockMode::Off);
        let twice = score_pressure_tracked(&inp, Some(once.score), &mut tracker, Some(&thr), None);
        assert_eq!(twice.disk_lock, DiskLockMode::Soft);
        assert_ne!(twice.band, PressureBand::Emergency);
    }

    #[test]
    fn disk_latency_hard_tripwire() {
        let thr = DiskLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        inp.disk_latency_sec = 0.050;
        inp.disk_busy_percent = 40.0;
        inp.disk_queue_length = 1.0;
        let _ = score_pressure_tracked(&inp, None, &mut tracker, Some(&thr), None);
        let twice = score_pressure_tracked(&inp, Some(0.3), &mut tracker, Some(&thr), None);
        assert_eq!(twice.disk_lock, DiskLockMode::Hard);
        assert_eq!(twice.band, PressureBand::Emergency);
        assert_eq!(twice.tripwire, Some("disk_latency_hard"));
    }

    #[test]
    fn mapped_file_faults_do_not_trip_ram_and_faults() {
        let mut streak = 0;
        let mut inp = base();
        // Low available would trip if we only looked at faults — keep avail healthy.
        inp.memory_available_bytes = 8 * 1024 * 1024 * 1024;
        inp.hard_faults_per_sec = 5000.0;
        inp.paging_file_pct = 3.0;
        inp.pagefile_writes_per_sec = 5.0;
        assert!(looks_like_mapped_io(&inp));
        assert!(evaluate_tripwire(&inp, &mut streak).is_none());
        let s = score_pressure(&inp, None);
        let mut thrash = inp;
        thrash.memory_available_bytes = 100 * 1024 * 1024;
        thrash.paging_file_pct = 45.0;
        thrash.pagefile_writes_per_sec = 200.0;
        thrash.hard_faults_per_sec = 5000.0;
        let thrash_s = score_pressure(&thrash, None);
        assert!(
            thrash_s.score > s.score,
            "paging pressure should score higher than mapped I/O: {} vs {}",
            thrash_s.score,
            s.score
        );
    }

    #[test]
    fn ram_and_faults_requires_paging_evidence() {
        let mut streak = 0;
        let mut inp = base();
        inp.memory_available_bytes = 200 * 1024 * 1024; // ~1.2% of 16GB
        inp.hard_faults_per_sec = 800.0;
        inp.paging_file_pct = 25.0;
        inp.pagefile_writes_per_sec = 120.0;
        assert_eq!(evaluate_tripwire(&inp, &mut streak), Some("ram_and_faults"));

        let mut mapped_low_ram = inp;
        mapped_low_ram.paging_file_pct = 2.0;
        mapped_low_ram.pagefile_writes_per_sec = 0.0;
        mapped_low_ram.memory_available_bytes = (0.06 * 16.0 * 1024.0 * 1024.0 * 1024.0) as u64;
        mapped_low_ram.hard_faults_per_sec = 800.0;
        assert!(evaluate_tripwire(&mapped_low_ram, &mut streak).is_none());
    }

    #[test]
    fn stalls_idle_near_zero() {
        let stalls = compute_stalls(&base(), &DiskLockThresholds::default());
        assert!(stalls.cpu_some < 0.3);
        assert!(stalls.io_some < 0.2);
        assert_eq!(stalls.memory_full, 0.0);
        assert_eq!(stalls.io_full, 0.0);
    }

    #[test]
    fn io_full_from_hard_latency_boosts_score() {
        let mut inp = base();
        inp.disk_latency_sec = 0.050;
        inp.disk_busy_percent = 40.0;
        let thr = DiskLockThresholds::default();
        let stalls = compute_stalls(&inp, &thr);
        assert_eq!(stalls.io_full, 1.0);
        let with_full = score_from_stalls(&stalls);
        let mut no_lat = inp;
        no_lat.disk_latency_sec = 0.0;
        let without = score_from_stalls(&compute_stalls(&no_lat, &thr));
        assert!(with_full > without);
    }

    #[test]
    fn dpc_steal_raises_cpu_some() {
        let mut inp = base();
        inp.cpu_percent = 10.0;
        inp.dpc_time_percent = 25.0;
        inp.interrupt_time_percent = 5.0;
        let stalls = compute_stalls(&inp, &DiskLockThresholds::default());
        assert!(stalls.cpu_some >= 0.29);
    }

    #[test]
    fn thermal_some_raises_score() {
        let mut inp = base();
        inp.thermal_some = 0.70;
        let with = score_from_stalls(&compute_stalls(&inp, &DiskLockThresholds::default()));
        inp.thermal_some = 0.0;
        let without = score_from_stalls(&compute_stalls(&inp, &DiskLockThresholds::default()));
        assert!(with > without);
    }

    #[test]
    fn mem_soft_needs_streak() {
        let thr = MemLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        // ~10% available — below soft 15%, above hard 8%
        inp.memory_available_bytes = (0.10 * 16.0 * 1024.0 * 1024.0 * 1024.0) as u64;
        inp.paging_file_pct = 5.0;
        inp.pagefile_writes_per_sec = 0.0;
        let once = score_pressure_tracked(&inp, None, &mut tracker, None, Some(&thr));
        assert_eq!(once.mem_lock, MemLockMode::Off);
        let twice = score_pressure_tracked(&inp, Some(once.score), &mut tracker, None, Some(&thr));
        assert_eq!(twice.mem_lock, MemLockMode::Soft);
        assert_ne!(twice.band, PressureBand::Emergency);
    }

    #[test]
    fn mem_hard_requires_paging_evidence() {
        let thr = MemLockThresholds::default();
        let mut tracker = HysteresisTracker::default();
        let mut inp = base();
        // Commit hard threshold met, but available healthy and no pagefile pressure.
        inp.memory_commit_percent = 96.0;
        inp.memory_available_bytes = 8 * 1024 * 1024 * 1024;
        inp.paging_file_pct = 2.0;
        inp.pagefile_writes_per_sec = 0.0;
        inp.hard_faults_per_sec = 5000.0;
        assert!(looks_like_mapped_io(&inp) || !paging_pressure_evidence(&inp));
        let _ = score_pressure_tracked(&inp, None, &mut tracker, None, Some(&thr));
        let twice = score_pressure_tracked(&inp, Some(0.3), &mut tracker, None, Some(&thr));
        assert_eq!(twice.mem_lock, MemLockMode::Soft, "hard must not latch without paging");
        assert_ne!(twice.tripwire, Some("mem_lock_hard"));

        let mut thrash = inp;
        thrash.memory_available_bytes = (0.04 * 16.0 * 1024.0 * 1024.0 * 1024.0) as u64;
        thrash.paging_file_pct = 40.0;
        thrash.pagefile_writes_per_sec = 200.0;
        thrash.hard_faults_per_sec = 800.0;
        let mut tracker2 = HysteresisTracker::default();
        let _ = score_pressure_tracked(&thrash, None, &mut tracker2, None, Some(&thr));
        let hard = score_pressure_tracked(&thrash, Some(0.3), &mut tracker2, None, Some(&thr));
        assert_eq!(hard.mem_lock, MemLockMode::Hard);
        assert_eq!(hard.tripwire, Some("mem_lock_hard"));
        assert_eq!(hard.band, PressureBand::Emergency);
    }

}
