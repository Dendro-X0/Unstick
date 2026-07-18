use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use guardian_core::{
    classify_dpc_isr, classify_focus_profile, dpc_advisory_message, dpc_isr_raw_level,
    events_path, load_config, recent_events_for_client, save_config, score_pressure_tracked,
    status_path, thermal_advisory_message, ActionPlan, ApplyDeniedSummary, ClientRequest,
    CriticalGuardMode, DiskCalibrator, DiskControlLoop, DiskLockMode, EnvelopeCalibrator,
    GuardianConfig, GuardianEvent, HysteresisTracker, MemControlLoop, merge_control_actions,
    paging_pressure_evidence, plan_disk_control_actions, plan_mem_control_actions,
    MemLockMode, MemLockThresholds, PressureBand, PressureInputs, PressureState, ServerPush,
    StatusSnapshot, SuspendedSummary, ThrottleLevel, AbuseSummary, ThrottleSummary, plan_qos,
};
use guardian_detect::{apply_parent_anomaly, AbuseDetector};
use guardian_win::{elevation_likely, ThrottleExecutor, WinSensor};
use tokio::sync::{Mutex, RwLock};

use crate::ipc_server::{self, SharedState};

pub async fn run() -> Result<()> {
    fs::create_dir_all(guardian_core::config_dir())?;

    let cfg = Arc::new(RwLock::new(load_config()));
    let state = Arc::new(Mutex::new(ServiceInner::new(cfg.clone()).await?));
    {
        let mut g = state.lock().await;
        let n = g.recovered_suspends;
        if n > 0 {
            g.push_event(GuardianEvent::Info {
                message: format!(
                    "P0: resumed {n} process(es) left suspended after previous exit"
                ),
                at: Utc::now(),
            });
        }
    }

    let shared = SharedState {
        inner: state.clone(),
    };

    let ipc = tokio::spawn(ipc_server::serve(shared));
    let loop_handle = tokio::spawn(sample_loop(state));

    tokio::select! {
        r = ipc => r??,
        r = loop_handle => r??,
    }
    Ok(())
}

pub struct ServiceInner {
    pub cfg: Arc<RwLock<GuardianConfig>>,
    pub sensor: WinSensor,
    pub policy_self_pid: u32,
    pub ema: Option<f32>,
    pub hysteresis: HysteresisTracker,
    pub disk_cal: DiskCalibrator,
    pub envelope_cal: EnvelopeCalibrator,
    pub disk_control: DiskControlLoop,
    pub mem_control: MemControlLoop,
    pub last_band: PressureBand,
    pub detector: AbuseDetector,
    pub throttle: ThrottleExecutor,
    pub recent_events: Vec<GuardianEvent>,
    pub recent_throttles: Vec<ThrottleSummary>,
    pub recent_abuse: Vec<AbuseSummary>,
    pub last_status: Option<StatusSnapshot>,
    pub started: Instant,
    pub last_sample: Option<guardian_core::SystemSample>,
    pub apply_denied: Vec<ApplyDeniedSummary>,
    pub recovered_suspends: u32,
    /// Consecutive samples at Emergency or Disk Lock Hard (for last-resort Suspend).
    pub hard_pressure_streak: u32,
    /// Consecutive samples with elevated DPC/ISR (detect-only).
    pub dpc_elevated_streak: u32,
    pub last_dpc_advisory_at: Option<Instant>,
    pub last_thermal_advisory_at: Option<Instant>,
    /// Last time `status.json` was written (throttled on Normal).
    pub last_status_write: Option<Instant>,
    /// Rate-limit elevated apply_denied Info events.
    pub last_elev_denied_log: Option<Instant>,
}

impl ServiceInner {
    async fn new(cfg: Arc<RwLock<GuardianConfig>>) -> Result<Self> {
        let snapshot = cfg.read().await.clone();
        let job_rate = snapshot.job_cpu_rate_percent;
        let mut throttle = ThrottleExecutor::new(job_rate, snapshot.max_suspend_secs);
        let recovered = throttle.recover_orphans_from_disk();
        let recovered_n = recovered.len() as u32;
        if recovered_n > 0 {
            tracing::info!(count = recovered_n, "P0 resumed orphans from suspend ledger");
        }
        Ok(Self {
            cfg,
            sensor: WinSensor::new(),
            policy_self_pid: std::process::id(),
            ema: None,
            hysteresis: HysteresisTracker::default(),
            disk_cal: DiskCalibrator::new(&snapshot),
            envelope_cal: EnvelopeCalibrator::new(&snapshot),
            disk_control: DiskControlLoop::new(snapshot.disk_control_enabled),
            mem_control: MemControlLoop::new(snapshot.mem_control_enabled),
            last_band: PressureBand::Normal,
            detector: AbuseDetector::new(&snapshot),
            throttle,
            recent_events: Vec::new(),
            recent_throttles: Vec::new(),
            recent_abuse: Vec::new(),
            last_status: None,
            started: Instant::now(),
            last_sample: None,
            apply_denied: Vec::new(),
            recovered_suspends: recovered_n,
            hard_pressure_streak: 0,
            dpc_elevated_streak: 0,
            last_dpc_advisory_at: None,
            last_thermal_advisory_at: None,
            last_status_write: None,
            last_elev_denied_log: None,
        })
    }

    pub async fn handle_request(&mut self, req: ClientRequest) -> ServerPush {
        match req {
            ClientRequest::GetStatus => {
                if let Some(s) = &self.last_status {
                    ServerPush::Status(s.clone())
                } else {
                    ServerPush::Error {
                        message: "no sample yet".into(),
                    }
                }
            }
            ClientRequest::Pause { minutes } => {
                let until = Utc::now() + chrono::Duration::minutes(minutes as i64);
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.pause_until = Some(until);
                    let _ = save_config(&cfg);
                }
                for (pid, name, reason) in self.throttle.resume_all_suspended("paused") {
                    self.push_event(GuardianEvent::Resume {
                        pid,
                        name,
                        reason,
                        at: Utc::now(),
                    });
                }
                self.throttle.restore_all();
                self.push_event(GuardianEvent::Info {
                    message: format!("paused for {minutes} minutes"),
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: format!("paused until {}", until.to_rfc3339()),
                }
            }
            ClientRequest::Resume => {
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.pause_until = None;
                    let _ = save_config(&cfg);
                }
                self.push_event(GuardianEvent::Info {
                    message: "resumed".into(),
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: "resumed".into(),
                }
            }
            ClientRequest::TrustPid { pid } => {
                {
                    let mut cfg = self.cfg.write().await;
                    if !cfg.trusted_pids.contains(&pid) {
                        cfg.trusted_pids.push(pid);
                    }
                    let _ = save_config(&cfg);
                    self.detector.reload_trust(&cfg);
                }
                self.detector.trust_pid(pid);
                if self.throttle.ledger.contains(pid) {
                    for (p, name, reason) in self.throttle.resume_pids(&[pid], "trusted") {
                        self.push_event(GuardianEvent::Resume {
                            pid: p,
                            name,
                            reason,
                            at: Utc::now(),
                        });
                    }
                }
                ServerPush::Ok {
                    message: format!("trusted pid {pid}"),
                }
            }
            ClientRequest::AddAllowPath { path } => {
                {
                    let mut cfg = self.cfg.write().await;
                    if !cfg.allow_paths.iter().any(|p| p == &path) {
                        cfg.allow_paths.push(path.clone());
                    }
                    // Also protect from suspend/throttle
                    cfg.add_whitelist(path.clone());
                    let _ = save_config(&cfg);
                    self.detector.reload_trust(&cfg);
                }
                ServerPush::Ok {
                    message: format!("allow+whitelist path {path}"),
                }
            }
            ClientRequest::AddWhitelist { entry } => {
                let added = {
                    let mut cfg = self.cfg.write().await;
                    let added = cfg.add_whitelist(entry.clone());
                    let _ = save_config(&cfg);
                    self.detector.reload_trust(&cfg);
                    added
                };
                // If a matching process is currently suspended, resume it
                let live_names: Vec<(u32, String)> = self
                    .throttle
                    .ledger
                    .list()
                    .into_iter()
                    .map(|e| (e.pid, e.name))
                    .collect();
                let entry_l = entry.to_lowercase();
                let to_resume: Vec<u32> = live_names
                    .iter()
                    .filter(|(_, n)| {
                        n.to_lowercase() == entry_l
                            || n.to_lowercase().contains(&entry_l)
                            || entry_l.contains(&n.to_lowercase())
                    })
                    .map(|(p, _)| *p)
                    .collect();
                for (pid, name, reason) in self.throttle.resume_pids(&to_resume, "whitelisted") {
                    self.push_event(GuardianEvent::Resume {
                        pid,
                        name,
                        reason,
                        at: Utc::now(),
                    });
                }
                ServerPush::Ok {
                    message: if added {
                        format!("whitelisted {entry}")
                    } else {
                        format!("already whitelisted {entry}")
                    },
                }
            }
            ClientRequest::RemoveWhitelist { entry } => {
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.remove_whitelist(&entry);
                    let _ = save_config(&cfg);
                    self.detector.reload_trust(&cfg);
                }
                ServerPush::Ok {
                    message: format!("removed whitelist {entry}"),
                }
            }
            ClientRequest::Events { limit } => {
                let events = recent_events_for_client(&self.recent_events, limit);
                ServerPush::Events { events }
            }
            ClientRequest::SetCriticalGuard { enabled } => {
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.emergency_suspend = enabled;
                    let _ = save_config(&cfg);
                }
                if !enabled {
                    for (pid, name, reason) in
                        self.throttle.resume_all_suspended("critical_guard_off")
                    {
                        self.push_event(GuardianEvent::Resume {
                            pid,
                            name,
                            reason,
                            at: Utc::now(),
                        });
                    }
                }
                self.push_event(GuardianEvent::Info {
                    message: if enabled {
                        "Critical Guard ON".into()
                    } else {
                        "Critical Guard OFF".into()
                    },
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: format!("critical_guard={enabled}"),
                }
            }
            ClientRequest::SetCriticalGuardMode { mode } => {
                {
                    let mut cfg = self.cfg.write().await;
                    if mode == CriticalGuardMode::LastResortSuspend && !cfg.experimental_suspend {
                        return ServerPush::Error {
                            message: "last_resort_suspend requires experimental_suspend=true in config.json (D1 Soft-only product path)".into(),
                        };
                    }
                    cfg.critical_guard_mode = mode;
                    if mode == CriticalGuardMode::LastResortSuspend && !cfg.emergency_suspend {
                        cfg.emergency_suspend = true;
                    }
                    cfg.normalize_suspend_product_path();
                    let _ = save_config(&cfg);
                }
                if mode == CriticalGuardMode::SoftOnly {
                    for (pid, name, reason) in
                        self.throttle.resume_all_suspended("soft_only_mode")
                    {
                        self.push_event(GuardianEvent::Resume {
                            pid,
                            name,
                            reason,
                            at: Utc::now(),
                        });
                    }
                }
                self.push_event(GuardianEvent::Info {
                    message: format!("Critical Guard mode={}", mode.as_str()),
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: format!("critical_guard_mode={}", mode.as_str()),
                }
            }
            ClientRequest::SetDiskSafeThresholds { soft_pct, hard_pct } => {
                let soft = soft_pct.clamp(50.0, 99.0);
                let hard = hard_pct.clamp(soft + 1.0, 100.0);
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.disk_busy_soft_pct = soft;
                    cfg.disk_busy_hard_pct = hard;
                    let _ = save_config(&cfg);
                }
                self.disk_cal.set_safe_thresholds(soft, hard);
                self.push_event(GuardianEvent::Info {
                    message: format!("safe disk soft={soft:.0}% hard={hard:.0}%"),
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: format!("Disk safe: soft {soft:.0}% · hard {hard:.0}%"),
                }
            }
            ClientRequest::SetMemSafeThresholds { soft_pct, hard_pct } => {
                let soft = soft_pct.clamp(5.0, 40.0);
                let hard = hard_pct.clamp(2.0, soft - 0.5);
                {
                    let mut cfg = self.cfg.write().await;
                    cfg.mem_avail_soft_pct = soft;
                    cfg.mem_avail_hard_pct = hard;
                    let _ = save_config(&cfg);
                }
                self.push_event(GuardianEvent::Info {
                    message: format!("safe mem soft={soft:.0}% hard={hard:.0}% available"),
                    at: Utc::now(),
                });
                ServerPush::Ok {
                    message: format!("RAM safe: soft {soft:.0}% · hard {hard:.0}% avail"),
                }
            }
        }
    }

    fn push_event(&mut self, ev: GuardianEvent) {
        append_event_log(&ev);
        self.recent_events.push(ev);
        if self.recent_events.len() > 200 {
            let drain = self.recent_events.len() - 200;
            self.recent_events.drain(0..drain);
        }
    }
}

fn append_event_log(ev: &GuardianEvent) {
    let path = events_path();
    if let Ok(raw) = serde_json::to_string(ev) {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{raw}");
        }
    }
}

async fn sample_loop(state: Arc<Mutex<ServiceInner>>) -> Result<()> {
    {
        let mut g = state.lock().await;
        let _ = g.sensor.sample_ex(true);
    }
    tokio::time::sleep(Duration::from_millis(400)).await;

    loop {
        let sleep_ms = {
            let mut g = state.lock().await;
            tick(&mut g).await?
        };
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }
}

async fn tick(g: &mut ServiceInner) -> Result<u64> {
    let cfg = g.cfg.read().await.clone();
    g.detector.reload_trust(&cfg);
    g.throttle.set_job_cpu_rate(cfg.job_cpu_rate_percent);
    g.throttle.ledger.set_max_secs(cfg.max_suspend_secs);
    g.disk_cal.sync_from_config(&cfg);
    g.envelope_cal.sync_from_config(&cfg);
    g.disk_control.set_enabled(cfg.disk_control_enabled);
    g.mem_control.set_enabled(cfg.mem_control_enabled);

    let paused = cfg
        .pause_until
        .map(|u| u > Utc::now())
        .unwrap_or(false);

    let busy = matches!(
        g.last_band,
        PressureBand::Warn | PressureBand::Throttle | PressureBand::Emergency
    ) || g
        .last_status
        .as_ref()
        .map(|s| {
            s.disk_lock != DiskLockMode::Off || s.mem_lock != MemLockMode::Off
        })
        .unwrap_or(false);
    let sample = g.sensor.sample_ex(busy);
    let inputs = PressureInputs {
        cpu_percent: sample.cpu_percent,
        memory_available_bytes: sample.memory_available_bytes,
        memory_total_bytes: sample.memory_total_bytes,
        memory_commit_percent: sample.memory_commit_percent,
        disk_busy_percent: sample.disk_busy_percent,
        disk_queue_length: sample.disk_queue_length,
        disk_latency_sec: sample.disk_latency_sec,
        hard_faults_per_sec: sample.hard_faults_per_sec,
        pagefile_writes_per_sec: sample.pagefile_writes_per_sec,
        paging_file_pct: sample.paging_file_pct,
        dpc_time_percent: sample.dpc_time_percent,
        interrupt_time_percent: sample.interrupt_time_percent,
        thermal_some: sample.thermal_level.thermal_some(),
    };
    let disk_thr = g.disk_cal.observe(
        sample.disk_busy_percent,
        sample.disk_queue_length,
        sample.disk_io_bytes_per_sec,
    );
    let mem_thr = MemLockThresholds::from_config(&cfg);
    let pressure: PressureState = score_pressure_tracked(
        &inputs,
        g.ema,
        &mut g.hysteresis,
        Some(&disk_thr),
        Some(&mem_thr),
    );
    g.ema = Some(pressure.score);

    let envelope = g.envelope_cal.observe(
        &sample,
        pressure.band,
        pressure.disk_lock,
        pressure.mem_lock,
        paused,
        cfg.emergency_suspend,
        disk_thr.soft_queue,
    );
    let disk_ctrl = if paused || !cfg.emergency_suspend {
        g.disk_control.set_enabled(false);
        g.disk_control.step(0.0, envelope.u_set_lo, envelope.u_set_hi)
    } else {
        g.disk_control.set_enabled(cfg.disk_control_enabled);
        g.disk_control
            .step(envelope.u_disk, envelope.u_set_lo, envelope.u_set_hi)
    };
    let mem_ctrl = if paused || !cfg.emergency_suspend {
        g.mem_control.set_enabled(false);
        g.mem_control.step(0.0, envelope.u_set_lo, envelope.u_set_hi)
    } else {
        g.mem_control.set_enabled(cfg.mem_control_enabled);
        g.mem_control
            .step(envelope.u_mem, envelope.u_set_lo, envelope.u_set_hi)
    };
    let paging = paging_pressure_evidence(&inputs);

    if pressure.band != g.last_band {
        g.push_event(GuardianEvent::Pressure {
            band: pressure.band.as_str().into(),
            score: pressure.score,
            at: Utc::now(),
        });
        g.last_band = pressure.band;
    }

    let mut hits = g.detector.evaluate(&sample);
    let parent_map: std::collections::HashMap<u32, String> = sample
        .processes
        .iter()
        .map(|p| (p.pid, p.name.clone()))
        .collect();
    for hit in &mut hits {
        if let Some(proc) = sample.processes.iter().find(|p| p.pid == hit.pid) {
            let parent_name = parent_map.get(&proc.parent_pid).map(|s| s.as_str());
            let (score, extra) = apply_parent_anomaly(hit.score, parent_name);
            hit.score = score;
            if let Some(r) = extra {
                hit.reasons.push(r);
            }
        }
    }
    hits.retain(|h| h.score >= 70);

    for ev in guardian_detect::AbuseDetector::to_events(&hits) {
        g.push_event(ev);
    }
    g.recent_abuse = hits
        .iter()
        .map(|h| AbuseSummary {
            pid: h.pid,
            name: h.name.clone(),
            score: h.score,
            reasons: h.reasons.clone(),
        })
        .collect();

    let hard_pressure = matches!(pressure.band, PressureBand::Emergency)
        || pressure.disk_lock == DiskLockMode::Hard
        || pressure.mem_lock == MemLockMode::Hard;
    if hard_pressure && !paused {
        g.hard_pressure_streak = g.hard_pressure_streak.saturating_add(1);
    } else {
        g.hard_pressure_streak = 0;
    }

    // Detect-only DPC/ISR advisory — never drives throttle/suspend.
    let dpc_raw = dpc_isr_raw_level(sample.dpc_time_percent, sample.interrupt_time_percent);
    if dpc_raw != guardian_core::DpcAdvisoryLevel::None {
        g.dpc_elevated_streak = g.dpc_elevated_streak.saturating_add(1);
    } else {
        g.dpc_elevated_streak = 0;
    }
    let dpc_level = classify_dpc_isr(
        sample.dpc_time_percent,
        sample.interrupt_time_percent,
        g.dpc_elevated_streak,
        3,
    );
    let dpc_advisory = dpc_advisory_message(dpc_level).map(|s| s.to_string());
    if dpc_level == guardian_core::DpcAdvisoryLevel::High {
        let should_emit = match g.last_dpc_advisory_at {
            Some(t) => t.elapsed() >= Duration::from_secs(300),
            None => true,
        };
        if should_emit {
            if let Some(msg) = dpc_advisory_message(dpc_level) {
                g.push_event(GuardianEvent::Info {
                    message: msg.into(),
                    at: Utc::now(),
                });
                g.last_dpc_advisory_at = Some(Instant::now());
            }
        }
    }

    let thermal_advisory = thermal_advisory_message(sample.thermal_level).map(|s| s.to_string());
    if matches!(
        sample.thermal_level,
        guardian_core::ThermalLevel::Fair | guardian_core::ThermalLevel::Serious
    ) {
        let should_emit = match g.last_thermal_advisory_at {
            Some(t) => t.elapsed() >= Duration::from_secs(300),
            None => true,
        };
        if should_emit {
            if let Some(msg) = thermal_advisory_message(sample.thermal_level) {
                g.push_event(GuardianEvent::Info {
                    message: msg.into(),
                    at: Utc::now(),
                });
                g.last_thermal_advisory_at = Some(Instant::now());
            }
        }
    }

    let engine = guardian_core::PolicyEngine::new(&cfg, g.policy_self_pid);
    let mut plan = if paused {
        ActionPlan::default()
    } else {
        engine.plan(
            pressure.band,
            &sample,
            pressure.tripwire,
            pressure.disk_lock,
            pressure.mem_lock,
            g.hard_pressure_streak,
            sample.thermal_level,
        )
    };

    if !paused && cfg.emergency_suspend && disk_ctrl.intensity > 0 {
        let extras = plan_disk_control_actions(&engine, &sample, disk_ctrl.intensity);
        merge_control_actions(&mut plan, extras);
    }
    if !paused && cfg.emergency_suspend && mem_ctrl.intensity > 0 {
        let extras = plan_mem_control_actions(&engine, &sample, mem_ctrl.intensity, paging);
        merge_control_actions(&mut plan, extras);
    }

    if !paused {
        for hit in &hits {
            let proc = sample.processes.iter().find(|p| p.pid == hit.pid);
            let protected = proc
                .map(|p| engine.protected.is_protected(p))
                .unwrap_or(false);
            if hit.score >= 80 && !protected && !plan.actions.iter().any(|a| a.pid == hit.pid) {
                plan.actions.push(guardian_core::PlannedAction {
                    pid: hit.pid,
                    name: hit.name.clone(),
                    level: ThrottleLevel::BelowNormal,
                    apply_job_cap: false,
                    apply_disk_lock: false,
                    apply_mem_lock: false,
                    apply_ecoqos: true,
                    apply_mem_priority_low: false,
                    reason: format!("abuse:{}", hit.score),
                });
            }
        }
    }

    // Resume expired suspensions
    let expired = g.throttle.ledger.expired_pids();
    if !expired.is_empty() {
        for (pid, name, reason) in g.throttle.resume_pids(&expired, "max_suspend_secs") {
            g.push_event(GuardianEvent::Resume {
                pid,
                name,
                reason,
                at: Utc::now(),
            });
        }
    }

    // Exit emergency → resume all suspended
    if !matches!(pressure.band, PressureBand::Emergency) && !g.throttle.ledger.list().is_empty()
    {
        for (pid, name, reason) in g.throttle.resume_all_suspended("pressure_recovered") {
            g.push_event(GuardianEvent::Resume {
                pid,
                name,
                reason,
                at: Utc::now(),
            });
        }
    }

    if pressure.band == PressureBand::Normal
        && pressure.disk_lock == DiskLockMode::Off
        && pressure.mem_lock == MemLockMode::Off
        && disk_ctrl.intensity == 0
        && mem_ctrl.intensity == 0
        && !paused
    {
        g.throttle.restore_all();
        g.recent_throttles.clear();
        g.apply_denied.clear();
    } else if paused {
        g.throttle.clear_boost();
    } else {
        if plan.boost_foreground {
            g.throttle.boost_foreground(sample.focus_pid);
        } else {
            g.throttle.clear_boost();
        }
        if !plan.actions.is_empty() {
            let outcome = g.throttle.apply(&plan.actions);
            g.apply_denied = outcome
                .denied
                .iter()
                .map(|(pid, name, err)| ApplyDeniedSummary {
                    pid: *pid,
                    name: name.clone(),
                    error: err.clone(),
                    elevation_likely: elevation_likely(err),
                })
                .collect();
            if !g.apply_denied.is_empty() {
                let elev = g.apply_denied.iter().filter(|d| d.elevation_likely).count();
                // Rate-limit: elevated Access Denied is expected for AV leftovers.
                let should_log = elev > 0
                    && g.last_elev_denied_log
                        .map(|t| t.elapsed() >= Duration::from_secs(300))
                        .unwrap_or(true);
                if should_log {
                    g.last_elev_denied_log = Some(Instant::now());
                    g.push_event(GuardianEvent::Info {
                        message: format!(
                            "{elev} elevated process(es) skipped (Defender/AV-style Access Denied is expected)"
                        ),
                        at: Utc::now(),
                    });
                }
            }
            g.recent_throttles = outcome
                .applied
                .iter()
                .map(|(pid, level, reason)| ThrottleSummary {
                    pid: *pid,
                    name: plan
                        .actions
                        .iter()
                        .find(|a| a.pid == *pid)
                        .map(|a| a.name.clone())
                        .unwrap_or_default(),
                    level: *level,
                    reason: reason.clone(),
                })
                .collect();

            let throttle_events: Vec<GuardianEvent> = g
                .recent_throttles
                .iter()
                .map(|t| {
                    if t.level == ThrottleLevel::Suspend {
                        GuardianEvent::Suspend {
                            pid: t.pid,
                            name: t.name.clone(),
                            reason: t.reason.clone(),
                            at: Utc::now(),
                        }
                    } else {
                        GuardianEvent::Throttle {
                            pid: t.pid,
                            name: t.name.clone(),
                            level: t.level,
                            reason: t.reason.clone(),
                            at: Utc::now(),
                        }
                    }
                })
                .collect();
            for ev in throttle_events {
                g.push_event(ev);
            }
        }
        // D1: restore EcoQoS / mem-prio / priority for PIDs no longer in the plan.
        let plan_pids: std::collections::HashSet<u32> =
            plan.actions.iter().map(|a| a.pid).collect();
        g.throttle.restore_not_in_plan(&plan_pids);
    }

    let live: Vec<u32> = sample.processes.iter().map(|p| p.pid).collect();
    g.throttle.restore_missing(&live);

    let suspended: Vec<SuspendedSummary> = g
        .throttle
        .ledger
        .list()
        .into_iter()
        .map(|e| SuspendedSummary {
            pid: e.pid,
            name: e.name,
            reason: e.reason,
            suspended_secs: e.since.elapsed().as_secs(),
        })
        .collect();

    let top: Vec<_> = sample.processes.iter().take(10).cloned().collect();
    let focus_proc = sample
        .focus_pid
        .and_then(|fp| sample.processes.iter().find(|p| p.pid == fp));
    let focus_name = focus_proc.map(|p| p.name.clone());
    let focus_profile = classify_focus_profile(focus_proc);
    let qos = if paused {
        plan_qos(
            cfg.critical_guard_mode,
            PressureBand::Normal,
            DiskLockMode::Off,
            MemLockMode::Off,
            focus_profile,
            sample.thermal_level,
            false,
        )
    } else {
        plan.qos
    };
    let status = StatusSnapshot {
        paused,
        pause_until_unix: cfg.pause_until.map(|t| t.timestamp()),
        critical_guard: cfg.emergency_suspend,
        critical_guard_mode: cfg.critical_guard_mode,
        experimental_suspend: cfg.experimental_suspend,
        focus_pid: sample.focus_pid,
        focus_name,
        focus_profile,
        focus_qos: qos.focus,
        background_qos: qos.background,
        nap_policy: qos.nap,
        pressure_score: pressure.score,
        pressure_band: pressure.band,
        tripwire: pressure.tripwire.map(|s| s.to_string()),
        disk_lock: pressure.disk_lock,
        disk_lock_soft_pct: disk_thr.soft_pct,
        disk_lock_hard_pct: disk_thr.hard_pct,
        disk_calibrated: disk_thr.calibrated,
        disk_lock_adaptive: cfg.disk_lock_adaptive,
        disk_saturation: disk_thr.saturation,
        disk_peak_io_bps: disk_thr.peak_io_bps,
        mem_lock: pressure.mem_lock,
        mem_lock_soft_pct: mem_thr.avail_soft_pct,
        mem_lock_hard_pct: mem_thr.avail_hard_pct,
        cpu_percent: sample.cpu_percent,
        memory_available_bytes: sample.memory_available_bytes,
        memory_total_bytes: sample.memory_total_bytes,
        disk_busy_percent: sample.disk_busy_percent,
        disk_queue_length: sample.disk_queue_length,
        disk_latency_sec: sample.disk_latency_sec,
        hard_faults_per_sec: sample.hard_faults_per_sec,
        pagefile_writes_per_sec: sample.pagefile_writes_per_sec,
        paging_file_pct: sample.paging_file_pct,
        dpc_time_percent: sample.dpc_time_percent,
        interrupt_time_percent: sample.interrupt_time_percent,
        dpc_advisory,
        stall_cpu: pressure.stalls.cpu_some,
        stall_memory: pressure.stalls.memory_some,
        stall_io: pressure.stalls.io_some,
        stall_memory_full: pressure.stalls.memory_full,
        stall_io_full: pressure.stalls.io_full,
        stall_thermal: pressure.stalls.thermal_some,
        envelope,
        disk_control_intensity: disk_ctrl.intensity,
        disk_control_mode: disk_ctrl.mode,
        mem_control_intensity: mem_ctrl.intensity,
        mem_control_mode: mem_ctrl.mode,
        on_battery: sample.on_battery,
        battery_percent: sample.battery_percent,
        cooling_mode: sample.cooling_mode,
        cpu_mhz_ratio: sample.cpu_mhz_ratio,
        thermal_level: sample.thermal_level,
        thermal_advisory,
        top_processes: top,
        recent_throttles: g.recent_throttles.clone(),
        recent_abuse: g.recent_abuse.clone(),
        suspended,
        whitelist: cfg.whitelist.clone(),
        service_uptime_secs: g.started.elapsed().as_secs(),
        version: guardian_core::VERSION.to_string(),
        apply_denied: g.apply_denied.clone(),
        recovered_suspends: g.recovered_suspends,
    };

    // Compact JSON; throttle disk write on Normal (IPC always uses last_status).
    let should_write = match g.last_status_write {
        None => true,
        Some(t) => {
            let busy = matches!(
                pressure.band,
                PressureBand::Warn | PressureBand::Throttle | PressureBand::Emergency
            );
            busy || t.elapsed() >= Duration::from_millis(1000)
        }
    };
    if should_write {
        if let Ok(raw) = serde_json::to_string(&status) {
            let _ = fs::write(status_path(), raw);
            g.last_status_write = Some(Instant::now());
        }
    }

    g.last_status = Some(status);
    g.last_sample = Some(sample);

    let sleep = if matches!(
        pressure.band,
        PressureBand::Warn | PressureBand::Throttle | PressureBand::Emergency
    ) {
        cfg.sample_busy_ms
    } else {
        cfg.sample_idle_ms
    };
    Ok(sleep)
}
