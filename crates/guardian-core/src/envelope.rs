//! Per-machine hardware envelope from idle baselines (Option 2 / D2).
//!
//! Collects quiet-system samples, then publishes ceilings just under Soft/Hard
//! freeze-risk lines. D3 closes the disk loop via `control::DiskControlLoop`.

use serde::{Deserialize, Serialize};

use crate::config::{config_dir, GuardianConfig};
use crate::pressure::{DiskLockMode, MemLockMode, PressureBand};
use crate::types::SystemSample;

const IDLE_WINDOW: usize = 64;
const MIN_IDLE_SAMPLES: u32 = 30;
/// Hold utilization with real headroom under the freeze cliff (freeze-safe band).
/// Sitting at 0.97–0.99 left too little latency budget and could still lock the UI.
pub const U_SET_LO: f32 = 0.80;
pub const U_SET_HI: f32 = 0.88;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeSnapshot {
    pub calibrated: bool,
    pub idle_samples: u32,
    /// Idle latency p50 / p95 (seconds).
    pub idle_latency_p50_sec: f32,
    pub idle_latency_p95_sec: f32,
    pub idle_queue_p50: f32,
    pub idle_queue_p95: f32,
    pub idle_avail_pct_p50: f32,
    /// Ceilings / floor used as the freeze cliff (u → 1.0).
    pub disk_latency_ceiling_sec: f32,
    pub disk_queue_ceiling: f32,
    pub disk_busy_ceiling_pct: f32,
    /// Available RAM % at which memory axis is fully loaded (cliff).
    pub mem_avail_floor_pct: f32,
    pub mem_commit_ceiling_pct: f32,
    pub u_set_lo: f32,
    pub u_set_hi: f32,
    /// Live utilization vs envelope (0..1+; may exceed 1 under overload).
    pub u_disk: f32,
    pub u_mem: f32,
}

impl Default for EnvelopeSnapshot {
    fn default() -> Self {
        Self {
            calibrated: false,
            idle_samples: 0,
            idle_latency_p50_sec: 0.0,
            idle_latency_p95_sec: 0.0,
            idle_queue_p50: 0.0,
            idle_queue_p95: 0.0,
            idle_avail_pct_p50: 50.0,
            disk_latency_ceiling_sec: 0.015,
            disk_queue_ceiling: 4.0,
            disk_busy_ceiling_pct: 95.0,
            mem_avail_floor_pct: 8.0,
            mem_commit_ceiling_pct: 95.0,
            u_set_lo: U_SET_LO,
            u_set_hi: U_SET_HI,
            u_disk: 0.0,
            u_mem: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnvelopeCalibrator {
    latency: Vec<f32>,
    queue: Vec<f32>,
    avail_pct: Vec<f32>,
    idle_samples: u32,
    persist: bool,
    /// Bootstrap / live ceilings from config Soft→Hard.
    soft_latency: f32,
    hard_latency: f32,
    soft_busy: f32,
    hard_busy: f32,
    soft_avail: f32,
    hard_avail: f32,
    soft_commit: f32,
    hard_commit: f32,
    soft_queue: f32,
    live: EnvelopeSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EnvelopeProfileFile {
    idle_samples: u32,
    idle_latency_p50_sec: f32,
    idle_latency_p95_sec: f32,
    idle_queue_p50: f32,
    idle_queue_p95: f32,
    idle_avail_pct_p50: f32,
    disk_latency_ceiling_sec: f32,
    disk_queue_ceiling: f32,
    disk_busy_ceiling_pct: f32,
    mem_avail_floor_pct: f32,
    mem_commit_ceiling_pct: f32,
}

impl EnvelopeCalibrator {
    pub fn new(cfg: &GuardianConfig) -> Self {
        let mut s = Self::fresh(cfg);
        s.persist = true;
        if let Some(p) = load_profile() {
            s.idle_samples = p.idle_samples;
            s.live.idle_samples = p.idle_samples;
            s.live.idle_latency_p50_sec = p.idle_latency_p50_sec;
            s.live.idle_latency_p95_sec = p.idle_latency_p95_sec;
            s.live.idle_queue_p50 = p.idle_queue_p50;
            s.live.idle_queue_p95 = p.idle_queue_p95;
            s.live.idle_avail_pct_p50 = p.idle_avail_pct_p50;
            if p.idle_samples >= MIN_IDLE_SAMPLES {
                s.live.calibrated = true;
                s.apply_persisted_ceilings(&p);
            }
        }
        s.recompute_ceilings();
        s
    }

    pub fn fresh(cfg: &GuardianConfig) -> Self {
        let mut s = Self {
            latency: Vec::with_capacity(IDLE_WINDOW),
            queue: Vec::with_capacity(IDLE_WINDOW),
            avail_pct: Vec::with_capacity(IDLE_WINDOW),
            idle_samples: 0,
            persist: false,
            soft_latency: cfg.disk_latency_soft_sec.clamp(0.001, 1.0),
            hard_latency: cfg
                .disk_latency_hard_sec
                .max(cfg.disk_latency_soft_sec + 0.001)
                .clamp(0.002, 2.0),
            soft_busy: cfg.disk_busy_soft_pct.clamp(50.0, 99.0),
            hard_busy: cfg.disk_busy_hard_pct.clamp(60.0, 100.0),
            soft_avail: cfg.mem_avail_soft_pct.clamp(1.0, 40.0),
            hard_avail: cfg.mem_avail_hard_pct.clamp(1.0, 30.0),
            soft_commit: cfg.mem_commit_soft_pct.clamp(50.0, 99.0),
            hard_commit: cfg.mem_commit_hard_pct.clamp(60.0, 100.0),
            soft_queue: 4.0,
            live: EnvelopeSnapshot::default(),
        };
        s.recompute_ceilings();
        s
    }

    pub fn sync_from_config(&mut self, cfg: &GuardianConfig) {
        self.soft_latency = cfg.disk_latency_soft_sec.clamp(0.001, 1.0);
        self.hard_latency = cfg
            .disk_latency_hard_sec
            .max(cfg.disk_latency_soft_sec + 0.001)
            .clamp(0.002, 2.0);
        self.soft_busy = cfg.disk_busy_soft_pct.clamp(50.0, 99.0);
        self.hard_busy = cfg
            .disk_busy_hard_pct
            .clamp(self.soft_busy + 1.0, 100.0);
        self.soft_avail = cfg.mem_avail_soft_pct.clamp(1.0, 40.0);
        self.hard_avail = cfg
            .mem_avail_hard_pct
            .clamp(1.0, self.soft_avail - 0.5)
            .min(self.soft_avail - 0.5)
            .max(1.0);
        self.soft_commit = cfg.mem_commit_soft_pct.clamp(50.0, 99.0);
        self.hard_commit = cfg
            .mem_commit_hard_pct
            .clamp(self.soft_commit + 1.0, 100.0);
        self.recompute_ceilings();
    }

    pub fn snapshot(&self) -> EnvelopeSnapshot {
        self.live.clone()
    }

    /// Feed one sample. Idle baselines update only when quiet + Guard armed.
    pub fn observe(
        &mut self,
        sample: &SystemSample,
        band: PressureBand,
        disk_lock: DiskLockMode,
        mem_lock: MemLockMode,
        paused: bool,
        armed: bool,
        disk_soft_queue: f32,
    ) -> EnvelopeSnapshot {
        self.soft_queue = disk_soft_queue.max(1.0);
        let avail_pct = if sample.memory_total_bytes > 0 {
            100.0 * sample.memory_available_bytes as f32 / sample.memory_total_bytes as f32
        } else {
            50.0
        };
        let commit = sample.paging_file_pct.clamp(0.0, 100.0);

        let quiet = !paused
            && armed
            && band == PressureBand::Normal
            && disk_lock == DiskLockMode::Off
            && mem_lock == MemLockMode::Off
            && sample.disk_busy_percent < 40.0
            && sample.cpu_percent < 45.0;

        if quiet {
            push_window(&mut self.latency, sample.disk_latency_sec.max(0.0));
            push_window(&mut self.queue, sample.disk_queue_length.max(0.0));
            push_window(&mut self.avail_pct, avail_pct.clamp(0.0, 100.0));
            self.idle_samples = self.idle_samples.saturating_add(1);
            self.live.idle_samples = self.idle_samples;

            if self.latency.len() >= 8 {
                self.live.idle_latency_p50_sec = percentile(&self.latency, 0.50);
                self.live.idle_latency_p95_sec = percentile(&self.latency, 0.95);
                self.live.idle_queue_p50 = percentile(&self.queue, 0.50);
                self.live.idle_queue_p95 = percentile(&self.queue, 0.95);
                self.live.idle_avail_pct_p50 = percentile(&self.avail_pct, 0.50);
            }

            if self.idle_samples >= MIN_IDLE_SAMPLES {
                self.live.calibrated = true;
            }

            self.recompute_ceilings();

            if self.persist && self.idle_samples % 45 == 0 && self.live.calibrated {
                self.save();
            }
        } else {
            // Still refresh ceilings from config / last idle stats.
            self.recompute_ceilings();
        }

        self.live.u_disk = utilization_disk(
            sample.disk_latency_sec,
            sample.disk_queue_length,
            sample.disk_busy_percent,
            &self.live,
        );
        self.live.u_mem = utilization_mem(avail_pct, commit, &self.live);
        self.live.u_set_lo = U_SET_LO;
        self.live.u_set_hi = U_SET_HI;
        self.live.clone()
    }

    fn apply_persisted_ceilings(&mut self, p: &EnvelopeProfileFile) {
        if p.disk_latency_ceiling_sec > 0.0 {
            self.live.disk_latency_ceiling_sec = p.disk_latency_ceiling_sec;
        }
        if p.disk_queue_ceiling > 0.0 {
            self.live.disk_queue_ceiling = p.disk_queue_ceiling;
        }
        if p.disk_busy_ceiling_pct > 0.0 {
            self.live.disk_busy_ceiling_pct = p.disk_busy_ceiling_pct;
        }
        if p.mem_avail_floor_pct > 0.0 {
            self.live.mem_avail_floor_pct = p.mem_avail_floor_pct;
        }
        if p.mem_commit_ceiling_pct > 0.0 {
            self.live.mem_commit_ceiling_pct = p.mem_commit_ceiling_pct;
        }
    }

    fn recompute_ceilings(&mut self) {
        // Bootstrap: Soft lines (conservative). After idle calib, lift toward
        // 98% of Hard (just under freeze-risk tripwires).
        let t = if self.live.calibrated { 0.98 } else { 0.0 };
        let lat_ceil = lerp(self.soft_latency, self.hard_latency * 0.98, t).max(self.soft_latency);
        let busy_ceil = lerp(self.soft_busy, self.hard_busy * 0.98, t).max(self.soft_busy);
        let mut q_ceil = lerp(self.soft_queue, self.soft_queue * 2.2, t).max(self.soft_queue);
        if self.live.calibrated && self.live.idle_queue_p95 > 0.0 {
            q_ceil = q_ceil
                .max(self.live.idle_queue_p95 * 4.0 + 1.0)
                .clamp(1.2, 48.0);
        }
        // Mem: floor = Hard avail (cliff). Commit ceiling → 98% of Hard when calib.
        let avail_floor = self.hard_avail;
        let commit_ceil = lerp(self.soft_commit, self.hard_commit * 0.98, t).max(self.soft_commit);

        // Keep latency ceiling above idle p95 with margin once we know idle.
        let lat_ceil = if self.live.calibrated && self.live.idle_latency_p95_sec > 0.0 {
            lat_ceil.max(self.live.idle_latency_p95_sec * 6.0)
        } else {
            lat_ceil
        };

        self.live.disk_latency_ceiling_sec = lat_ceil;
        self.live.disk_queue_ceiling = q_ceil;
        self.live.disk_busy_ceiling_pct = busy_ceil;
        self.live.mem_avail_floor_pct = avail_floor;
        self.live.mem_commit_ceiling_pct = commit_ceil;
    }

    fn save(&self) {
        let p = EnvelopeProfileFile {
            idle_samples: self.idle_samples,
            idle_latency_p50_sec: self.live.idle_latency_p50_sec,
            idle_latency_p95_sec: self.live.idle_latency_p95_sec,
            idle_queue_p50: self.live.idle_queue_p50,
            idle_queue_p95: self.live.idle_queue_p95,
            idle_avail_pct_p50: self.live.idle_avail_pct_p50,
            disk_latency_ceiling_sec: self.live.disk_latency_ceiling_sec,
            disk_queue_ceiling: self.live.disk_queue_ceiling,
            disk_busy_ceiling_pct: self.live.disk_busy_ceiling_pct,
            mem_avail_floor_pct: self.live.mem_avail_floor_pct,
            mem_commit_ceiling_pct: self.live.mem_commit_ceiling_pct,
        };
        let _ = std::fs::create_dir_all(config_dir());
        if let Ok(raw) = serde_json::to_string_pretty(&p) {
            let _ = std::fs::write(profile_path(), raw);
        }
    }
}

fn profile_path() -> std::path::PathBuf {
    config_dir().join("envelope_profile.json")
}

fn load_profile() -> Option<EnvelopeProfileFile> {
    let raw = std::fs::read_to_string(profile_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn push_window(v: &mut Vec<f32>, x: f32) {
    if v.len() >= IDLE_WINDOW {
        v.remove(0);
    }
    v.push(x);
}

fn percentile(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((s.len() as f32 - 1.0) * p.clamp(0.0, 1.0)).round() as usize;
    s[idx.min(s.len() - 1)]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn utilization_disk(latency: f32, queue: f32, busy: f32, env: &EnvelopeSnapshot) -> f32 {
    let lat_u = if env.disk_latency_ceiling_sec > 1e-6 {
        (latency.max(0.0) / env.disk_latency_ceiling_sec).max(0.0)
    } else {
        0.0
    };
    let q_u = if env.disk_queue_ceiling > 1e-6 {
        (queue.max(0.0) / env.disk_queue_ceiling).max(0.0)
    } else {
        0.0
    };
    let b_u = if env.disk_busy_ceiling_pct > 1e-6 {
        (busy.max(0.0) / env.disk_busy_ceiling_pct).max(0.0)
    } else {
        0.0
    };
    lat_u.max(q_u).max(b_u)
}

pub fn utilization_mem(avail_pct: f32, commit_pct: f32, env: &EnvelopeSnapshot) -> f32 {
    let floor = env.mem_avail_floor_pct;
    let head = env.idle_avail_pct_p50.max(floor + 5.0).max(env.mem_avail_floor_pct + 5.0);
    // u=0 near idle free RAM; u=1 at floor.
    let avail_u = if head > floor {
        ((head - avail_pct.clamp(0.0, 100.0)) / (head - floor)).clamp(0.0, 1.5)
    } else {
        0.0
    };
    let commit_u = if env.mem_commit_ceiling_pct > 1e-6 {
        (commit_pct.max(0.0) / env.mem_commit_ceiling_pct).max(0.0)
    } else {
        0.0
    };
    avail_u.max(commit_u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ProcessSample;

    fn quiet_sample(latency: f32, queue: f32, avail_frac: f32) -> SystemSample {
        SystemSample {
            timestamp: chrono::Utc::now(),
            cpu_percent: 10.0,
            memory_available_bytes: (avail_frac * 16.0 * 1024.0 * 1024.0 * 1024.0) as u64,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            memory_commit_percent: 40.0,
            disk_busy_percent: 8.0,
            disk_queue_length: queue,
            disk_io_bytes_per_sec: 1_000_000,
            hard_faults_per_sec: 0.0,
            focus_pid: None,
            disk_latency_sec: latency,
            pagefile_writes_per_sec: 0.0,
            paging_file_pct: 40.0,
            dpc_time_percent: 0.0,
            interrupt_time_percent: 0.0,
            on_battery: false,
            battery_percent: None,
            cooling_mode: Default::default(),
            cpu_mhz_ratio: 1.0,
            thermal_level: Default::default(),
            processes: Vec::<ProcessSample>::new(),
        }
    }

    #[test]
    fn bootstrap_ceilings_from_soft_until_calibrated() {
        let cfg = GuardianConfig::default();
        let cal = EnvelopeCalibrator::fresh(&cfg);
        let snap = cal.snapshot();
        assert!(!snap.calibrated);
        assert!((snap.disk_latency_ceiling_sec - cfg.disk_latency_soft_sec).abs() < 0.001);
        assert!((snap.disk_busy_ceiling_pct - cfg.disk_busy_soft_pct).abs() < 0.1);
        assert!((snap.mem_avail_floor_pct - cfg.mem_avail_hard_pct).abs() < 0.1);
    }

    #[test]
    fn idle_samples_prime_calibration() {
        let cfg = GuardianConfig::default();
        let mut cal = EnvelopeCalibrator::fresh(&cfg);
        for _ in 0..MIN_IDLE_SAMPLES {
            let _ = cal.observe(
                &quiet_sample(0.002, 0.2, 0.55),
                PressureBand::Normal,
                DiskLockMode::Off,
                MemLockMode::Off,
                false,
                true,
                4.0,
            );
        }
        let snap = cal.snapshot();
        assert!(snap.calibrated);
        assert!(snap.idle_samples >= MIN_IDLE_SAMPLES);
        assert!(snap.disk_latency_ceiling_sec >= cfg.disk_latency_soft_sec);
        assert!(snap.disk_latency_ceiling_sec <= cfg.disk_latency_hard_sec * 0.99 + 0.001);
        assert!(snap.u_disk < 0.5, "idle u_disk={}", snap.u_disk);
        assert!(snap.u_mem < 0.6, "idle u_mem={}", snap.u_mem);
    }

    #[test]
    fn busy_raises_u_disk() {
        let cfg = GuardianConfig::default();
        let mut cal = EnvelopeCalibrator::fresh(&cfg);
        let mut s = quiet_sample(0.050, 10.0, 0.55);
        s.disk_busy_percent = 99.0;
        s.cpu_percent = 80.0;
        let snap = cal.observe(
            &s,
            PressureBand::Throttle,
            DiskLockMode::Soft,
            MemLockMode::Off,
            false,
            true,
            4.0,
        );
        assert!(snap.u_disk >= 1.0, "u_disk={}", snap.u_disk);
        assert!(!snap.calibrated); // busy samples do not count as idle
    }

    #[test]
    fn utilization_mem_hits_one_at_floor() {
        let env = EnvelopeSnapshot {
            mem_avail_floor_pct: 8.0,
            idle_avail_pct_p50: 50.0,
            mem_commit_ceiling_pct: 95.0,
            ..EnvelopeSnapshot::default()
        };
        let u = utilization_mem(8.0, 40.0, &env);
        assert!((u - 1.0).abs() < 0.05, "u={u}");
    }
}
