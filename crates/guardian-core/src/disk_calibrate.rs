//! Adaptive Disk Lock thresholds from observed system-disk behavior.

use serde::{Deserialize, Serialize};

use crate::config::{config_dir, GuardianConfig};
use crate::pressure::DiskLockThresholds;

/// Learns this machine's system-disk capacity and healthy baseline, then
/// derives soft/hard busy% and queue thresholds (no fixed 85/95 for all hardware).
#[derive(Debug, Clone)]
pub struct DiskCalibrator {
    samples: u32,
    /// Peak useful throughput (bytes/sec) while the drive is working, not stalled.
    peak_io_bps: f32,
    /// Highest busy% seen (slow decay) — hardware "ceiling".
    peak_busy: f32,
    peak_queue: f32,
    /// Baseline when the drive is healthy (low saturation).
    healthy_busy_ema: f32,
    healthy_queue_ema: f32,
    /// Prior from config until primed.
    prior_soft: f32,
    prior_hard: f32,
    streak: u32,
    enabled: bool,
    adaptive: bool,
    persist: bool,
    soft_latency_sec: f32,
    hard_latency_sec: f32,
    /// Last computed thresholds.
    live: DiskLockThresholds,
    last_saturation: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DiskProfileFile {
    peak_io_bps: f32,
    peak_busy: f32,
    peak_queue: f32,
    healthy_busy_ema: f32,
    healthy_queue_ema: f32,
    samples: u32,
}

impl DiskCalibrator {
    pub fn new(cfg: &GuardianConfig) -> Self {
        let mut s = Self::fresh(cfg);
        s.persist = true;
        if let Some(p) = load_profile() {
            s.peak_io_bps = p.peak_io_bps.max(1.0);
            s.peak_busy = p.peak_busy.clamp(30.0, 100.0);
            s.peak_queue = p.peak_queue.max(0.5);
            s.healthy_busy_ema = p.healthy_busy_ema.max(0.0);
            s.healthy_queue_ema = p.healthy_queue_ema.max(0.0);
            s.samples = p.samples;
            s.recompute();
        }
        s
    }

    /// No persisted profile (tests / reset).
    pub fn fresh(cfg: &GuardianConfig) -> Self {
        let mut s = Self {
            samples: 0,
            peak_io_bps: 8.0 * 1024.0 * 1024.0,
            peak_busy: 60.0,
            peak_queue: 2.0,
            healthy_busy_ema: 15.0,
            healthy_queue_ema: 0.3,
            prior_soft: cfg.disk_busy_soft_pct.clamp(50.0, 100.0),
            prior_hard: cfg.disk_busy_hard_pct.clamp(60.0, 100.0),
            streak: cfg.disk_busy_streak.max(1),
            enabled: cfg.disk_lock_enabled,
            adaptive: cfg.disk_lock_adaptive,
            persist: false,
            soft_latency_sec: cfg.disk_latency_soft_sec.clamp(0.001, 1.0),
            hard_latency_sec: cfg
                .disk_latency_hard_sec
                .max(cfg.disk_latency_soft_sec + 0.001)
                .clamp(0.002, 2.0),
            live: DiskLockThresholds::from_config(cfg),
            last_saturation: 0.0,
        };
        s.recompute();
        s
    }

    pub fn sync_from_config(&mut self, cfg: &GuardianConfig) {
        self.enabled = cfg.disk_lock_enabled;
        self.adaptive = cfg.disk_lock_adaptive;
        self.prior_soft = cfg.disk_busy_soft_pct.clamp(50.0, 99.0);
        self.prior_hard = cfg
            .disk_busy_hard_pct
            .clamp(self.prior_soft + 1.0, 100.0);
        self.streak = cfg.disk_busy_streak.max(1);
        self.soft_latency_sec = cfg.disk_latency_soft_sec.clamp(0.001, 1.0);
        self.hard_latency_sec = cfg
            .disk_latency_hard_sec
            .max(cfg.disk_latency_soft_sec + 0.001)
            .clamp(0.002, 2.0);
        self.recompute();
    }

    /// User-set safe disk usage (busy %). Soft = limit I/O; hard = pause/suspend.
    pub fn set_safe_thresholds(&mut self, soft_pct: f32, hard_pct: f32) {
        self.prior_soft = soft_pct.clamp(50.0, 99.0);
        self.prior_hard = hard_pct.clamp(self.prior_soft + 1.0, 100.0);
        self.recompute();
    }

    pub fn thresholds(&self) -> DiskLockThresholds {
        self.live
    }

    pub fn saturation(&self) -> f32 {
        self.last_saturation
    }

    pub fn primed(&self) -> bool {
        self.samples >= MIN_SAMPLES || self.live.calibrated
    }

    /// Feed one sample; updates learned peaks/baselines and live thresholds.
    pub fn observe(&mut self, busy: f32, queue: f32, io_bps: u64) -> DiskLockThresholds {
        if !self.enabled {
            self.live.enabled = false;
            return self.live;
        }

        let busy = busy.clamp(0.0, 100.0);
        let queue = queue.max(0.0);
        let io = io_bps as f32;
        self.samples = self.samples.saturating_add(1);

        // Learn peak throughput while drive is delivering work (not idle, not fully stalled).
        if (20.0..80.0).contains(&busy) && io > 512_000.0 {
            if io > self.peak_io_bps {
                self.peak_io_bps = self.peak_io_bps * 0.7 + io * 0.3;
            } else {
                // Slow rise toward observed useful throughput
                self.peak_io_bps = self.peak_io_bps * 0.97 + io * 0.03;
            }
            self.peak_io_bps = self.peak_io_bps.max(io * 0.5);
        }

        // Ceiling tracking with slow decay so a one-off spike doesn't lock forever,
        // but DRAM-less 100% events raise the ceiling.
        self.peak_busy = (self.peak_busy * 0.997).max(busy);
        self.peak_queue = (self.peak_queue * 0.997).max(queue);

        let sat = saturation_index(busy, queue, io, self.peak_io_bps, self.peak_queue);
        self.last_saturation = sat;

        // Healthy baseline: only learn when not already in trouble.
        if sat < 0.45 && busy < 55.0 {
            const A: f32 = 0.08;
            self.healthy_busy_ema = A * busy + (1.0 - A) * self.healthy_busy_ema;
            self.healthy_queue_ema = A * queue + (1.0 - A) * self.healthy_queue_ema;
        }

        self.recompute();

        if self.persist && self.samples % 60 == 0 {
            save_profile(&DiskProfileFile {
                peak_io_bps: self.peak_io_bps,
                peak_busy: self.peak_busy,
                peak_queue: self.peak_queue,
                healthy_busy_ema: self.healthy_busy_ema,
                healthy_queue_ema: self.healthy_queue_ema,
                samples: self.samples,
            });
        }

        self.live
    }

    fn recompute(&mut self) {
        // Busy% triggers are always the user's safe thresholds (or config priors).
        let soft = self.prior_soft;
        let hard = self.prior_hard.max(soft + 1.0);

        if !self.adaptive {
            self.live = DiskLockThresholds {
                enabled: self.enabled,
                soft_pct: soft,
                hard_pct: hard,
                soft_queue: 4.0,
                hard_queue: 8.0,
                soft_latency_sec: self.soft_latency_sec,
                hard_latency_sec: self.hard_latency_sec,
                streak: self.streak,
                calibrated: false,
                peak_io_bps: self.peak_io_bps,
                saturation: self.last_saturation,
            };
            return;
        }

        // Adaptive: tune queue sensitivity to this drive; busy% stays user-set.
        let primed = self.samples >= MIN_SAMPLES;
        let soft_q = (self.healthy_queue_ema * 2.8 + 1.2)
            .max(1.2)
            .min(self.peak_queue * 0.55 + 1.0)
            .clamp(1.2, 24.0);
        let hard_q = (self.healthy_queue_ema * 4.5 + 2.0)
            .max(soft_q + 0.8)
            .min(self.peak_queue * 0.85 + 1.5)
            .clamp(soft_q + 0.8, 48.0);

        self.live = DiskLockThresholds {
            enabled: self.enabled,
            soft_pct: soft,
            hard_pct: hard,
            soft_queue: soft_q,
            hard_queue: hard_q,
            soft_latency_sec: self.soft_latency_sec,
            hard_latency_sec: self.hard_latency_sec,
            streak: self.streak,
            calibrated: primed,
            peak_io_bps: self.peak_io_bps,
            saturation: self.last_saturation,
        };
    }
}

const MIN_SAMPLES: u32 = 40;

/// 0–1 saturation for this hardware: busy, relative queue, and IOPS-starvation proxy.
pub fn saturation_index(
    busy: f32,
    queue: f32,
    io_bps: f32,
    peak_io_bps: f32,
    peak_queue: f32,
) -> f32 {
    let busy_n = (busy / 100.0).clamp(0.0, 1.0);
    let q_ceil = peak_queue.max(2.0);
    let queue_n = (queue / q_ceil).clamp(0.0, 1.5).min(1.0);
    let peak = peak_io_bps.max(1.0);
    let thru = (io_bps / peak).clamp(0.0, 1.0);
    // High busy + low throughput vs this drive's peak ⇒ latent saturation (DRAM-less SSD).
    let stall = if busy_n > 0.5 {
        (busy_n * (1.0 - thru * 0.65)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    busy_n.max(queue_n).max(stall)
}

fn profile_path() -> std::path::PathBuf {
    config_dir().join("disk_profile.json")
}

fn load_profile() -> Option<DiskProfileFile> {
    let raw = std::fs::read_to_string(profile_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_profile(p: &DiskProfileFile) {
    let _ = std::fs::create_dir_all(config_dir());
    if let Ok(raw) = serde_json::to_string_pretty(p) {
        let _ = std::fs::write(profile_path(), raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_safe_busy_thresholds_are_authoritative() {
        let mut cfg = GuardianConfig::default();
        cfg.disk_busy_soft_pct = 80.0;
        cfg.disk_busy_hard_pct = 92.0;
        cfg.disk_lock_adaptive = true;
        let mut cal = DiskCalibrator::fresh(&cfg);
        for _ in 0..50 {
            let _ = cal.observe(100.0, 12.0, 4_000_000);
        }
        let t = cal.observe(40.0, 1.0, 8_000_000);
        assert!(
            (t.soft_pct - 80.0).abs() < 0.01,
            "soft={}",
            t.soft_pct
        );
        assert!(
            (t.hard_pct - 92.0).abs() < 0.01,
            "hard={}",
            t.hard_pct
        );
    }

    #[test]
    fn higher_observed_ceiling_raises_queue_threshold() {
        let cfg = GuardianConfig::default();

        let mut modest = DiskCalibrator::fresh(&cfg);
        for _ in 0..50 {
            let _ = modest.observe(18.0, 0.3, 20_000_000);
        }
        for _ in 0..8 {
            let _ = modest.observe(72.0, 2.5, 40_000_000);
        }
        let modest_thr = modest.observe(20.0, 0.4, 15_000_000);

        let mut harsh = DiskCalibrator::fresh(&cfg);
        for _ in 0..50 {
            let _ = harsh.observe(18.0, 0.3, 5_000_000);
        }
        for _ in 0..8 {
            let _ = harsh.observe(100.0, 10.0, 6_000_000);
        }
        let harsh_thr = harsh.observe(20.0, 0.4, 4_000_000);

        assert!(modest_thr.calibrated && harsh_thr.calibrated);
        assert_eq!(modest_thr.soft_pct, harsh_thr.soft_pct); // user busy% identical
        assert!(
            harsh_thr.hard_queue >= modest_thr.soft_queue,
            "harsh hard_q {} vs modest soft_q {}",
            harsh_thr.hard_queue,
            modest_thr.soft_queue
        );
    }

    #[test]
    fn stall_saturation_high_when_busy_with_low_throughput() {
        let sat = saturation_index(100.0, 6.0, 8_000_000.0, 80_000_000.0, 8.0);
        assert!(sat > 0.7, "sat={sat}");
        let healthy = saturation_index(15.0, 0.2, 40_000_000.0, 80_000_000.0, 8.0);
        assert!(healthy < 0.35, "healthy={healthy}");
    }
}
