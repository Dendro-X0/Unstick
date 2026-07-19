use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use guardian_core::{
    load_config, read_recent_events, status_path, ApplyDeniedSummary, ClientRequest,
    CriticalGuardMode, DiskControlMode, DiskLockMode, GuardianConfig, GuardianEvent, MemLockMode,
    PressureBand, ServerPush, StatusSnapshot,
};

use crate::client;
use crate::chrome;
use crate::history::MetricHistory;
use crate::theme::{self, BG_ELEV, CORAL, LINE, TEAL, TEXT, TEXT_DIM};
use crate::widgets::{self, lerp};
use crate::win_round;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Guard,
    Monitor,
    Apps,
    Protect,
}

struct SharedUi {
    status: Option<StatusSnapshot>,
    events: Vec<GuardianEvent>,
    online: bool,
    last_error: Option<String>,
    toast: Option<(String, Instant)>,
}

pub struct UnstickApp {
    tab: Tab,
    shared: Arc<Mutex<SharedUi>>,
    cmd_tx: std::sync::mpsc::Sender<ClientRequest>,
    rounded_applied: bool,
    history: MetricHistory,
    // Display lerps (0..1)
    cpu_disp: f32,
    ram_disp: f32,
    disk_disp: f32,
    pressure_disp: f32,
    // Animated segment counts (0..15)
    cpu_lit: f32,
    ram_lit: f32,
    disk_lit: f32,
    pressure_lit: f32,
    pulse_t: f32,
    allow_path_input: String,
    config: GuardianConfig,
    /// Draft safe disk busy% (applied via IPC).
    disk_soft_edit: f32,
    disk_hard_edit: f32,
    mem_soft_edit: f32,
    mem_hard_edit: f32,
    /// Guard secondary controls (Critical Guard, disk thresholds).
    controls_open: bool,
    /// Soft/Hard % sliders (advanced).
    advanced_open: bool,
    /// One-shot auto-expand when Disk Lock / suspend needs attention.
    controls_auto_armed: bool,
}

const GAUGE_SEGMENTS: f32 = 15.0;

impl UnstickApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let shared = Arc::new(Mutex::new(SharedUi {
            status: None,
            events: Vec::new(),
            online: false,
            last_error: None,
            toast: None,
        }));
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<ClientRequest>();
        let shared_bg = shared.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio");
            rt.block_on(async move {
                loop {
                    while let Ok(req) = cmd_rx.try_recv() {
                        match client::request(req).await {
                            Ok(ServerPush::Ok { message }) => {
                                if let Ok(mut g) = shared_bg.lock() {
                                    g.toast = Some((message, Instant::now()));
                                    g.online = true;
                                }
                            }
                            Ok(ServerPush::Error { message }) => {
                                if let Ok(mut g) = shared_bg.lock() {
                                    g.toast = Some((message, Instant::now()));
                                }
                            }
                            Ok(ServerPush::Status(s)) => {
                                if let Ok(mut g) = shared_bg.lock() {
                                    g.status = Some(s);
                                    g.online = true;
                                    g.last_error = None;
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                if let Ok(mut g) = shared_bg.lock() {
                                    g.online = false;
                                    g.last_error = Some(e.to_string());
                                }
                            }
                        }
                    }

                    let snap = match client::request(ClientRequest::GetStatus).await {
                        Ok(ServerPush::Status(s)) => {
                            if let Ok(mut g) = shared_bg.lock() {
                                g.online = true;
                                g.last_error = None;
                            }
                            Some(s)
                        }
                        Ok(_) => None,
                        Err(_) => {
                            let file = std::fs::read_to_string(status_path())
                                .ok()
                                .and_then(|r| serde_json::from_str(&r).ok());
                            if let Ok(mut g) = shared_bg.lock() {
                                g.online = file.is_some();
                                if file.is_none() {
                                    g.last_error =
                                        Some("guardian-service offline".into());
                                }
                            }
                            file
                        }
                    };
                    if let Some(s) = snap {
                        if let Ok(mut g) = shared_bg.lock() {
                            g.status = Some(s);
                        }
                    }

                    match client::request(ClientRequest::Events { limit: 40 }).await {
                        Ok(ServerPush::Events { events }) => {
                            if let Ok(mut g) = shared_bg.lock() {
                                g.events = events;
                            }
                        }
                        Err(_) => {
                            let file_ev = read_recent_events(40);
                            if let Ok(mut g) = shared_bg.lock() {
                                if !file_ev.is_empty() {
                                    g.events = file_ev;
                                }
                            }
                        }
                        _ => {}
                    }

                    tokio::time::sleep(Duration::from_millis(900)).await;
                }
            });
        });

        let mut app = Self {
            tab: Tab::Guard,
            shared,
            cmd_tx,
            rounded_applied: false,
            history: MetricHistory::default(),
            cpu_disp: 0.0,
            ram_disp: 0.0,
            disk_disp: 0.0,
            pressure_disp: 0.0,
            cpu_lit: 0.0,
            ram_lit: 0.0,
            disk_lit: 0.0,
            pressure_lit: 0.0,
            pulse_t: 0.0,
            allow_path_input: String::new(),
            config: load_config(),
            disk_soft_edit: 85.0,
            disk_hard_edit: 95.0,
            mem_soft_edit: 15.0,
            mem_hard_edit: 8.0,
            controls_open: false,
            advanced_open: false,
            controls_auto_armed: false,
        };
        app.disk_soft_edit = app.config.disk_busy_soft_pct;
        app.disk_hard_edit = app.config.disk_busy_hard_pct;
        app.mem_soft_edit = app.config.mem_avail_soft_pct;
        app.mem_hard_edit = app.config.mem_avail_hard_pct;
        app
    }
}

impl eframe::App for UnstickApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.rounded_applied {
            win_round::try_apply(frame);
            self.rounded_applied = true;
        }

        self.pulse_t = (self.pulse_t + ctx.input(|i| i.stable_dt)) % 1000.0;

        chrome::title_bar(ctx);
        chrome::paint_window_border(ctx);

        let (status, events, online, toast) = {
            let g = self.shared.lock().ok();
            let g = g.as_ref();
            (
                g.and_then(|x| x.status.clone()),
                g.map(|x| x.events.clone()).unwrap_or_default(),
                g.map(|x| x.online).unwrap_or(false),
                g.and_then(|x| x.toast.clone()),
            )
        };

        let mut gauge_targets = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        if let Some(s) = &status {
            let ram = if s.memory_total_bytes > 0 {
                1.0 - (s.memory_available_bytes as f32 / s.memory_total_bytes as f32)
            } else {
                0.0
            };
            self.history
                .push_sample(s.cpu_percent, s.disk_busy_percent);
            let t = 0.18;
            let t_seg = 0.28;
            gauge_targets = (
                s.cpu_percent / 100.0,
                ram,
                s.disk_busy_percent / 100.0,
                s.pressure_score,
            );
            self.cpu_disp = lerp(self.cpu_disp, gauge_targets.0, t);
            self.ram_disp = lerp(self.ram_disp, gauge_targets.1, t);
            self.disk_disp = lerp(self.disk_disp, gauge_targets.2, t);
            self.pressure_disp = lerp(self.pressure_disp, gauge_targets.3, t);
            self.cpu_lit = lerp(self.cpu_lit, self.cpu_disp * GAUGE_SEGMENTS, t_seg);
            self.ram_lit = lerp(self.ram_lit, self.ram_disp * GAUGE_SEGMENTS, t_seg);
            self.disk_lit = lerp(self.disk_lit, self.disk_disp * GAUGE_SEGMENTS, t_seg);
            self.pressure_lit = lerp(
                self.pressure_lit,
                self.pressure_disp * GAUGE_SEGMENTS,
                t_seg,
            );
        }

        // v0.1.2: ~1 Hz when settled; ~15 Hz only while lerping / interacting.
        let interacting = ctx.input(|i| i.pointer.any_down() || i.any_touches() || !i.keys_down.is_empty());
        let settled = status.as_ref().map_or(true, |_| {
            (self.cpu_disp - gauge_targets.0).abs() < 0.005
                && (self.ram_disp - gauge_targets.1).abs() < 0.005
                && (self.disk_disp - gauge_targets.2).abs() < 0.005
                && (self.pressure_disp - gauge_targets.3).abs() < 0.005
                && (self.cpu_lit - self.cpu_disp * GAUGE_SEGMENTS).abs() < 0.05
                && (self.ram_lit - self.ram_disp * GAUGE_SEGMENTS).abs() < 0.05
                && (self.disk_lit - self.disk_disp * GAUGE_SEGMENTS).abs() < 0.05
                && (self.pressure_lit - self.pressure_disp * GAUGE_SEGMENTS).abs() < 0.05
        });
        let repaint_ms = if interacting || toast.is_some() || !settled {
            66
        } else {
            1000
        };
        ctx.request_repaint_after(Duration::from_millis(repaint_ms));

        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(20, 10))
                    .stroke(egui::Stroke::new(1.0, theme::LINE_SOFT)),
            )
            .show(ctx, |ui| {
                let brand_row = ui.horizontal(|ui| {
                    ui.label(theme::brand_title());
                    ui.add_space(10.0);
                    let ver = status
                        .as_ref()
                        .map(|s| s.version.as_str())
                        .filter(|v| !v.is_empty())
                        .unwrap_or(guardian_core::VERSION);
                    ui.label(
                        egui::RichText::new(format!("v{ver}"))
                            .size(12.0)
                            .color(TEXT_DIM)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        widgets::live_badge(ui, online);
                    });
                });
                if brand_row.response.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                } else if brand_row.response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.add_space(12.0);
                let tabs = ["GUARD", "MONITOR", "WHITELIST", "PROTECT"];
                let active_idx = match self.tab {
                    Tab::Guard => 0,
                    Tab::Monitor => 1,
                    Tab::Apps => 2,
                    Tab::Protect => 3,
                };
                if let Some(i) = widgets::nav_tab_strip(ui, &tabs, active_idx) {
                    self.tab = match i {
                        0 => Tab::Guard,
                        1 => Tab::Monitor,
                        2 => Tab::Apps,
                        3 => Tab::Protect,
                        _ => self.tab,
                    };
                }
            });

        egui::TopBottomPanel::bottom("gauges")
            .exact_height(96.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_ELEV)
                    .stroke(egui::Stroke::new(1.0, LINE))
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        full.left_top(),
                        egui::Pos2::new(full.right(), full.top() + 1.0),
                    ),
                    0.0,
                    theme::TEAL_DIM.gamma_multiply(0.55),
                );
                ui.add_space(6.0);
                widgets::footer_gauge_row(
                    ui,
                    (self.cpu_disp, self.cpu_lit),
                    (self.ram_disp, self.ram_lit),
                    (self.disk_disp, self.disk_lit),
                    (self.pressure_disp, self.pressure_lit),
                );
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(24, 16)))
            .show(ctx, |ui| {
                if !online {
                    egui::Frame::NONE
                        .fill(CORAL.gamma_multiply(0.15))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(
                                    "Service offline — start guardian-service.exe, then this client will connect.",
                                )
                                .color(CORAL),
                            );
                        });
                    ui.add_space(12.0);
                }

                if let Some((msg, at)) = &toast {
                    if at.elapsed() < Duration::from_secs(4) {
                        let short = shorten_toast(msg);
                        ui.vertical_centered(|ui| {
                            egui::Frame::NONE
                                .fill(TEAL.gamma_multiply(0.12))
                                .corner_radius(theme::RADIUS_SM)
                                .inner_margin(egui::Margin::symmetric(14, 6))
                                .stroke(egui::Stroke::new(1.0, TEAL.gamma_multiply(0.35)))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(short)
                                            .size(12.0)
                                            .color(TEAL)
                                            .strong(),
                                    );
                                });
                        });
                        ui.add_space(10.0);
                    }
                }

                match self.tab {
                    Tab::Guard => self.ui_guard(ui, status.as_ref()),
                    Tab::Monitor => self.ui_monitor(ui, status.as_ref(), &events),
                    Tab::Apps => self.ui_apps(ui),
                    Tab::Protect => self.ui_protect(ui, status.as_ref()),
                }
            });
    }
}

impl UnstickApp {
    fn ui_guard(&mut self, ui: &mut egui::Ui, status: Option<&StatusSnapshot>) {
        let paused = status.map(|s| s.paused).unwrap_or(false);
        let armed = !paused;
        let band = status
            .map(|s| s.pressure_band.as_str())
            .unwrap_or("normal");
        let score = status.map(|s| s.pressure_score);
        let critical_on = status.map(|s| s.critical_guard).unwrap_or(true);
        let suspended_n = status.map(|s| s.suspended.len()).unwrap_or(0);
        let disk_lock = status.map(|s| s.disk_lock).unwrap_or(DiskLockMode::Off);
        let mem_lock = status.map(|s| s.mem_lock).unwrap_or(MemLockMode::Off);
        let denied = status.map(|s| s.apply_denied.as_slice()).unwrap_or(&[]);
        let recovered = status.map(|s| s.recovered_suspends).unwrap_or(0);
        let tripwire = status.and_then(|s| s.tripwire.as_deref());
        let disk_ctrl_mode = status
            .map(|s| s.disk_control_mode)
            .unwrap_or(DiskControlMode::Released);
        let mem_ctrl_mode = status
            .map(|s| s.mem_control_mode)
            .unwrap_or(DiskControlMode::Released);
        let controlling = disk_ctrl_mode != DiskControlMode::Released
            || mem_ctrl_mode != DiskControlMode::Released;
        let warn_pulse = matches!(
            status.map(|s| s.pressure_band),
            Some(PressureBand::Warn | PressureBand::Throttle | PressureBand::Emergency)
        ) || disk_lock != DiskLockMode::Off
            || mem_lock != MemLockMode::Off
            || disk_ctrl_mode == DiskControlMode::Capping
            || mem_ctrl_mode == DiskControlMode::Capping;
        let pulse = if warn_pulse {
            (self.pulse_t * 3.0).sin().abs()
        } else {
            0.0
        };

        // Auto-expand Controls once when attention is needed; user may still collapse.
        let needs_attention = suspended_n > 0
            || disk_lock != DiskLockMode::Off
            || mem_lock != MemLockMode::Off
            || controlling;
        if needs_attention && !self.controls_auto_armed {
            self.controls_open = true;
            self.controls_auto_armed = true;
        } else if !needs_attention {
            self.controls_auto_armed = false;
        }

        widgets::paint_hero_backdrop(ui, 120.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.ui_guard_body(
                    ui,
                    status,
                    armed,
                    band,
                    score,
                    critical_on,
                    suspended_n,
                    disk_lock,
                    mem_lock,
                    disk_ctrl_mode,
                    mem_ctrl_mode,
                    denied,
                    recovered,
                    tripwire,
                    pulse,
                );
            });
    }

    fn ui_guard_body(
        &mut self,
        ui: &mut egui::Ui,
        status: Option<&StatusSnapshot>,
        armed: bool,
        band: &str,
        score: Option<f32>,
        critical_on: bool,
        suspended_n: usize,
        disk_lock: DiskLockMode,
        mem_lock: MemLockMode,
        disk_ctrl_mode: DiskControlMode,
        mem_ctrl_mode: DiskControlMode,
        denied: &[ApplyDeniedSummary],
        recovered: u32,
        tripwire: Option<&str>,
        pulse: f32,
    ) {
        let controlling = disk_ctrl_mode != DiskControlMode::Released
            || mem_ctrl_mode != DiskControlMode::Released;
        ui.vertical_centered(|ui| {
            // Vertically center the hero when Controls is collapsed.
            if !self.controls_open {
                let hero_h = 320.0;
                let pad = ((ui.available_height() - hero_h) * 0.5).clamp(8.0, 48.0);
                ui.add_space(pad);
            } else {
                ui.add_space(8.0);
            }

            let cta = widgets::guard_cta(ui, armed, pulse);
            if cta.clicked {
                let req = if armed {
                    ClientRequest::Pause { minutes: 15 }
                } else {
                    ClientRequest::Resume
                };
                let _ = self.cmd_tx.send(req);
            }
            ui.add_space(14.0);
            widgets::pressure_readout(ui, band, score);
            if let Some(tw) = tripwire {
                ui.add_space(8.0);
                ui.horizontal_centered(|ui| {
                    let disk_i = status.map(|s| s.disk_control_intensity).unwrap_or(0);
                    let mem_i = status.map(|s| s.mem_control_intensity).unwrap_or(0);
                    let efficiency_idle = disk_i >= 3 || mem_i >= 3;
                    let (label, color) = if controlling {
                        let mut parts = Vec::new();
                        if disk_ctrl_mode != DiskControlMode::Released {
                            parts.push(format!("disk i{disk_i}"));
                        }
                        if mem_ctrl_mode != DiskControlMode::Released {
                            parts.push(format!("ram i{mem_i}"));
                        }
                        let verb = if efficiency_idle {
                            "efficiency idle"
                        } else {
                            "soft capping"
                        };
                        let detail = if parts.is_empty() {
                            verb.into()
                        } else {
                            format!("{verb} · {}", parts.join(" · "))
                        };
                        (format!("{tw} — {detail}"), CORAL)
                    } else {
                        (
                            format!("{tw} — monitoring (u below band)"),
                            theme::AMBER,
                        )
                    };
                    let hover = if efficiency_idle {
                        "Efficiency Mode (Idle+EcoQoS) under sustained disk/RAM cliff — auto-restores."
                    } else {
                        "Tripwire = sensing pressure. Soft capping = actively demoting background offenders. Monitoring without capping is normal for brief spikes."
                    };
                    widgets::status_chip(ui, label, color).on_hover_text(hover);
                });
            } else if controlling {
                ui.add_space(8.0);
                ui.horizontal_centered(|ui| {
                    widgets::status_chip(ui, "Hardware control · actively capping", theme::AMBER)
                        .on_hover_text(
                            "Closed-loop soft control is demoting background disk/RAM offenders.",
                        );
                });
            }
            if let Some(adv) = status.and_then(|s| s.dpc_advisory.as_ref()) {
                if !adv.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal_centered(|ui| {
                        let label = if status
                            .map(|s| s.dpc_time_percent.max(s.interrupt_time_percent) >= 20.0)
                            .unwrap_or(false)
                        {
                            "DPC/ISR · high (drivers)"
                        } else {
                            "DPC/ISR · elevated (drivers)"
                        };
                        widgets::status_chip(ui, label, theme::AMBER)
                            .on_hover_text(adv.as_str());
                    });
                }
            }
            if let Some(s) = status {
                use guardian_core::ThermalLevel;
                match s.thermal_level {
                    ThermalLevel::Nominal => {
                        if s.on_battery {
                            ui.add_space(8.0);
                            ui.horizontal_centered(|ui| {
                                let label = if let Some(p) = s.battery_percent {
                                    format!("Battery · {p}%")
                                } else {
                                    "Battery".into()
                                };
                                widgets::status_chip(ui, label, TEXT_DIM);
                            });
                        }
                    }
                    ThermalLevel::Fair => {
                        ui.add_space(8.0);
                        ui.horizontal_centered(|ui| {
                            widgets::status_chip(ui, "Thermal · fair", theme::AMBER);
                        });
                    }
                    ThermalLevel::Serious => {
                        ui.add_space(8.0);
                        ui.horizontal_centered(|ui| {
                            widgets::status_chip(ui, "Thermal · serious", CORAL);
                        });
                    }
                }
                ui.add_space(8.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    widgets::status_chip(
                        ui,
                        format!("QoS · {}", s.focus_qos.as_str().replace('_', " ")),
                        TEXT_DIM,
                    );
                    let nap_label = match s.nap_policy {
                        guardian_core::NapPolicy::Cooperate => "Nap · cooperate",
                        guardian_core::NapPolicy::ForcePause => "Nap · force pause",
                    };
                    let nap_color = match s.nap_policy {
                        guardian_core::NapPolicy::Cooperate => TEXT_DIM,
                        guardian_core::NapPolicy::ForcePause => theme::AMBER,
                    };
                    widgets::status_chip(ui, nap_label, nap_color);
                });
            }

            // Compact status chips in the first viewport (no long copy).
            if suspended_n > 0
                || disk_lock != DiskLockMode::Off
                || mem_lock != MemLockMode::Off
                || controlling
            {
                ui.add_space(10.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if suspended_n > 0 {
                        widgets::status_chip(ui, format!("{suspended_n} suspended"), CORAL);
                    }
                    if disk_ctrl_mode != DiskControlMode::Released {
                        let i = status.map(|s| s.disk_control_intensity).unwrap_or(0);
                        let (label, color) = match disk_ctrl_mode {
                            DiskControlMode::Capping => {
                                let u = status.map(|s| s.envelope.u_disk).unwrap_or(0.0);
                                if i >= 3 {
                                    (format!("Disk idle · i{i} · u {u:.2}"), CORAL)
                                } else {
                                    (format!("Disk cap · i{i} · u {u:.2}"), CORAL)
                                }
                            }
                            DiskControlMode::Holding => {
                                if i >= 3 {
                                    (format!("Disk idle · hold i{i}"), theme::AMBER)
                                } else {
                                    (format!("Disk hold · i{i}"), theme::AMBER)
                                }
                            }
                            DiskControlMode::Released => (String::new(), TEXT_DIM),
                        };
                        let resp = widgets::status_chip(ui, label, color);
                        if i >= 3 {
                            resp.on_hover_text(
                                "Efficiency Mode (Idle+EcoQoS) under sustained disk cliff — auto-restores.",
                            );
                        }
                    }
                    if mem_ctrl_mode != DiskControlMode::Released {
                        let i = status.map(|s| s.mem_control_intensity).unwrap_or(0);
                        let (label, color) = match mem_ctrl_mode {
                            DiskControlMode::Capping => {
                                let u = status.map(|s| s.envelope.u_mem).unwrap_or(0.0);
                                if i >= 3 {
                                    (format!("RAM idle · i{i} · u {u:.2}"), CORAL)
                                } else {
                                    (format!("RAM cap · i{i} · u {u:.2}"), CORAL)
                                }
                            }
                            DiskControlMode::Holding => {
                                if i >= 3 {
                                    (format!("RAM idle · hold i{i}"), theme::AMBER)
                                } else {
                                    (format!("RAM hold · i{i}"), theme::AMBER)
                                }
                            }
                            DiskControlMode::Released => (String::new(), TEXT_DIM),
                        };
                        let resp = widgets::status_chip(ui, label, color);
                        if i >= 3 {
                            resp.on_hover_text(
                                "Efficiency Mode (Idle+EcoQoS) under sustained RAM cliff — auto-restores.",
                            );
                        }
                    }
                    if disk_lock != DiskLockMode::Off {
                        let dl_color = match disk_lock {
                            DiskLockMode::Hard => CORAL,
                            DiskLockMode::Soft => theme::AMBER,
                            DiskLockMode::Off => TEXT_DIM,
                        };
                        let dl_label = match disk_lock {
                            DiskLockMode::Hard => "Disk Lock HARD".to_string(),
                            DiskLockMode::Soft => "Disk Lock SOFT".to_string(),
                            DiskLockMode::Off => String::new(),
                        };
                        widgets::status_chip(ui, dl_label, dl_color);
                    }
                    if mem_lock != MemLockMode::Off {
                        let ml_color = match mem_lock {
                            MemLockMode::Hard => CORAL,
                            MemLockMode::Soft => theme::AMBER,
                            MemLockMode::Off => TEXT_DIM,
                        };
                        let ml_label = match mem_lock {
                            MemLockMode::Hard => "Mem Lock HARD".to_string(),
                            MemLockMode::Soft => "Mem Lock SOFT".to_string(),
                            MemLockMode::Off => String::new(),
                        };
                        widgets::status_chip(ui, ml_label, ml_color);
                    }
                });
            }

            if let Some(name) = status.and_then(|s| s.focus_name.as_deref()) {
                if !name.is_empty() && armed {
                    ui.add_space(10.0);
                    ui.horizontal_centered(|ui| {
                        widgets::status_chip(ui, format!("Focus · {name}"), TEAL);
                    });
                }
            }

            ui.add_space(18.0);
            if widgets::controls_toggle(ui, self.controls_open) {
                self.controls_open = !self.controls_open;
            }

            if !self.controls_open {
                return;
            }

            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
            egui::Frame::NONE
                .fill(theme::BG_PANEL)
                .corner_radius(theme::RADIUS_MD)
                .inner_margin(egui::Margin::symmetric(18, 14))
                .stroke(egui::Stroke::new(1.0, theme::LINE))
                .show(ui, |ui| {
                    ui.set_width(520.0);
                    if suspended_n > 0 {
                        ui.label(
                            egui::RichText::new(
                                "Background processes are paused so the desktop stays responsive. They auto-resume when pressure drops, after the max pause timer, or when you Pause Guard. Whitelisted apps are never paused.",
                            )
                            .size(12.0)
                            .color(TEXT),
                        );
                        ui.add_space(8.0);
                    }
                    if recovered > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "Recovered {recovered} paused process(es) after last restart"
                            ))
                            .size(11.0)
                            .color(TEAL),
                        );
                        ui.add_space(6.0);
                    }
                    if !denied.is_empty() {
                        let elev = denied.iter().filter(|d| d.elevation_likely).count();
                        let msg = if elev > 0 {
                            format!(
                                "{elev} elevated process(es) skipped (e.g. Defender) — normal unless you need admin Guard"
                            )
                        } else {
                            format!("Could not apply limits to {} process(es)", denied.len())
                        };
                        ui.label(egui::RichText::new(msg).size(11.0).color(theme::AMBER));
                        ui.add_space(6.0);
                    }

                    ui.horizontal(|ui| {
                        let mut on = critical_on;
                        if ui
                            .checkbox(
                                &mut on,
                                egui::RichText::new("Hardware Guard")
                                    .size(13.0)
                                    .strong()
                                    .color(if critical_on { TEAL } else { TEXT_DIM }),
                            )
                            .changed()
                        {
                            let _ = self
                                .cmd_tx
                                .send(ClientRequest::SetCriticalGuard { enabled: on });
                        }
                        ui.add_space(12.0);
                        let chip_color = if suspended_n > 0 { CORAL } else { TEXT_DIM };
                        egui::Frame::NONE
                            .fill(chip_color.gamma_multiply(0.15))
                            .corner_radius(6.0)
                            .inner_margin(egui::Margin::symmetric(10, 4))
                            .stroke(egui::Stroke::new(1.0, chip_color.gamma_multiply(0.4)))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("{suspended_n} suspended"))
                                        .size(12.0)
                                        .strong()
                                        .color(chip_color),
                                );
                            });
                    });

                    if critical_on {
                        ui.add_space(8.0);
                        let mode = status
                            .map(|s| s.critical_guard_mode)
                            .unwrap_or(CriticalGuardMode::SoftOnly);
                        let experimental = status
                            .map(|s| s.experimental_suspend)
                            .unwrap_or(false);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Mode")
                                    .size(12.0)
                                    .color(TEXT_DIM),
                            );
                            ui.add_space(8.0);
                            let soft_sel = mode == CriticalGuardMode::SoftOnly;
                            let soft = ui.selectable_label(
                                soft_sel,
                                egui::RichText::new("Soft only")
                                    .size(12.0)
                                    .color(if soft_sel { TEAL } else { TEXT_DIM }),
                            );
                            if soft.clicked() && !soft_sel {
                                let _ = self.cmd_tx.send(ClientRequest::SetCriticalGuardMode {
                                    mode: CriticalGuardMode::SoftOnly,
                                });
                            }
                            if experimental {
                                ui.add_space(4.0);
                                let last_sel = mode == CriticalGuardMode::LastResortSuspend;
                                let last = ui.selectable_label(
                                    last_sel,
                                    egui::RichText::new("Last-resort pause")
                                        .size(12.0)
                                        .color(if last_sel { CORAL } else { TEXT_DIM }),
                                );
                                if last.clicked() && !last_sel {
                                    let _ = self.cmd_tx.send(ClientRequest::SetCriticalGuardMode {
                                        mode: CriticalGuardMode::LastResortSuspend,
                                    });
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(if experimental {
                                match mode {
                                CriticalGuardMode::SoftOnly => {
                                    "Default: closed-loop soft control for disk/RAM — never pauses processes."
                                }
                                    CriticalGuardMode::LastResortSuspend => {
                                        "Experimental: after sustained emergency, pause top offenders (never the focused app)."
                                    }
                                }
                            } else {
                                "Soft only (product default). NtSuspend is off unless you set experimental_suspend=true in config.json."
                            })
                            .size(11.0)
                            .color(TEXT_DIM),
                        );
                    }

                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("Hardware control")
                            .size(13.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(theme::dim(
                        "Holds OS disk and RAM near 80–88% of the freeze cliff. Soft actuators only (i3 = Efficiency Idle under sustained cliff) — never pauses apps by default.",
                    ));
                    ui.add_space(8.0);
                    if let Some(s) = status {
                        let env = &s.envelope;
                        let calib = if env.calibrated {
                            "calibrated"
                        } else {
                            "learning idle…"
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "Envelope · {calib} · idle samples {}",
                                env.idle_samples
                            ))
                            .size(12.0)
                            .color(if env.calibrated { TEAL } else { TEXT_DIM }),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Disk  u {:.2}  ·  set {:.2}–{:.2}  ·  {} i{}",
                                env.u_disk,
                                env.u_set_lo,
                                env.u_set_hi,
                                s.disk_control_mode.as_str(),
                                s.disk_control_intensity
                            ))
                            .size(12.0)
                            .color(TEXT),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "RAM   u {:.2}  ·  set {:.2}–{:.2}  ·  {} i{}",
                                env.u_mem,
                                env.u_set_lo,
                                env.u_set_hi,
                                s.mem_control_mode.as_str(),
                                s.mem_control_intensity
                            ))
                            .size(12.0)
                            .color(TEXT),
                        );
                    } else {
                        ui.label(theme::dim("Waiting for service status…"));
                    }

                    ui.add_space(12.0);
                    let adv_label = if self.advanced_open {
                        "Advanced thresholds ▾"
                    } else {
                        "Advanced thresholds ▸"
                    };
                    if ui
                        .selectable_label(
                            false,
                            egui::RichText::new(adv_label).size(12.0).color(TEXT_DIM),
                        )
                        .clicked()
                    {
                        self.advanced_open = !self.advanced_open;
                    }

                    if self.advanced_open {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Legacy Soft / Hard tripwires")
                                .size(12.0)
                                .strong()
                                .color(TEXT_DIM),
                        );
                        ui.label(theme::dim(
                            "Optional safety net alongside closed-loop control. Soft = start limiting; Hard = deeper limit (no Suspend unless experimental).",
                        ));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Safe disk usage (Active Time %)")
                                .size(12.0)
                                .color(TEXT),
                        );
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Soft").color(TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut self.disk_soft_edit, 50.0..=98.0)
                                    .suffix("%"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Hard").color(TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut self.disk_hard_edit, 55.0..=100.0)
                                    .suffix("%"),
                            );
                        });
                        if self.disk_hard_edit <= self.disk_soft_edit {
                            self.disk_hard_edit = (self.disk_soft_edit + 1.0).min(100.0);
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new("Apply disk").color(TEAL))
                                .clicked()
                            {
                                let soft = self.disk_soft_edit;
                                let hard = self.disk_hard_edit.max(soft + 1.0);
                                self.config.disk_busy_soft_pct = soft;
                                self.config.disk_busy_hard_pct = hard;
                                let _ = self.cmd_tx.send(ClientRequest::SetDiskSafeThresholds {
                                    soft_pct: soft,
                                    hard_pct: hard,
                                });
                            }
                            if ui.small_button("85 / 95").clicked() {
                                self.disk_soft_edit = 85.0;
                                self.disk_hard_edit = 95.0;
                            }
                            if ui.small_button("70 / 90").clicked() {
                                self.disk_soft_edit = 70.0;
                                self.disk_hard_edit = 90.0;
                            }
                        });

                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new("Safe available RAM %")
                                .size(12.0)
                                .color(TEXT),
                        );
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Soft").color(TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut self.mem_soft_edit, 5.0..=40.0).suffix("%"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Hard").color(TEXT_DIM));
                            ui.add(
                                egui::Slider::new(&mut self.mem_hard_edit, 2.0..=35.0).suffix("%"),
                            );
                        });
                        if self.mem_hard_edit >= self.mem_soft_edit {
                            self.mem_hard_edit = (self.mem_soft_edit - 0.5).max(2.0);
                        }
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(egui::RichText::new("Apply RAM").color(TEAL))
                                .clicked()
                            {
                                let soft = self.mem_soft_edit;
                                let hard = self.mem_hard_edit.min(soft - 0.5).max(2.0);
                                self.config.mem_avail_soft_pct = soft;
                                self.config.mem_avail_hard_pct = hard;
                                let _ = self.cmd_tx.send(ClientRequest::SetMemSafeThresholds {
                                    soft_pct: soft,
                                    hard_pct: hard,
                                });
                            }
                            if ui.small_button("15 / 8").clicked() {
                                self.mem_soft_edit = 15.0;
                                self.mem_hard_edit = 8.0;
                            }
                            if ui.small_button("20 / 10").clicked() {
                                self.mem_soft_edit = 20.0;
                                self.mem_hard_edit = 10.0;
                            }
                        });
                    }
                });
            });
        });
    }

    fn ui_monitor(
        &mut self,
        ui: &mut egui::Ui,
        status: Option<&StatusSnapshot>,
        events: &[GuardianEvent],
    ) {
        ui.label(egui::RichText::new("Live consumers").size(18.0).strong().color(TEXT));
        ui.label(theme::dim("Ranked by CPU — soft-throttle targets non-protected offenders"));
        ui.add_space(12.0);

        ui.columns(2, |cols| {
            widgets::sparkline_panel(
                &mut cols[0],
                "CPU — last 60s",
                &self.history.cpu_slice(),
                TEAL,
            );
            widgets::sparkline_panel(
                &mut cols[1],
                "DISK — last 60s",
                &self.history.disk_slice(),
                theme::AMBER,
            );
        });
        ui.add_space(14.0);

        let Some(s) = status else {
            ui.label(theme::dim("Waiting for status…"));
            return;
        };

        if !s.suspended.is_empty() {
            ui.label(
                egui::RichText::new("Suspended (Critical Guard)")
                    .strong()
                    .color(CORAL),
            );
            for e in &s.suspended {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&e.name).color(CORAL).strong());
                    ui.label(
                        egui::RichText::new(format!("pid {} · {}s · {}", e.pid, e.suspended_secs, e.reason))
                            .color(TEXT_DIM)
                            .size(12.0),
                    );
                });
            }
            ui.add_space(10.0);
        }

        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, p) in s.top_processes.iter().take(12).enumerate() {
                let alt = i % 2 == 0;
                egui::Frame::NONE
                    .fill(if alt { theme::BG_PANEL } else { theme::BG })
                    .corner_radius(theme::RADIUS_SM)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{:>5.1}%", p.cpu_percent))
                                    .color(TEAL)
                                    .strong()
                                    .monospace(),
                            );
                            ui.label(
                                egui::RichText::new(format!("pid {:>6}", p.pid))
                                    .color(TEXT_DIM)
                                    .monospace(),
                            );
                            ui.label(egui::RichText::new(&p.name).color(TEXT));
                            let mem_mb = p.memory_bytes as f64 / (1024.0 * 1024.0);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .small_button(egui::RichText::new("Whitelist").size(11.0).color(TEAL))
                                    .on_hover_text("Never suspend / throttle this program")
                                    .clicked()
                                {
                                    let entry = p
                                        .path
                                        .as_ref()
                                        .filter(|path| !path.is_empty())
                                        .cloned()
                                        .unwrap_or_else(|| p.name.clone());
                                    let _ = self.cmd_tx.send(ClientRequest::AddWhitelist {
                                        entry: entry.clone(),
                                    });
                                    self.config.add_whitelist(entry);
                                }
                                ui.label(
                                    egui::RichText::new(format!("{mem_mb:.0} MB")).color(TEXT_DIM),
                                );
                            });
                        });
                    });
                ui.add_space(4.0);
            }
        });

        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("Event log")
                .strong()
                .color(TEXT_DIM),
        );
        ui.label(theme::dim("Last actions from this session / events.jsonl"));
        ui.add_space(6.0);
        if events.is_empty() {
            ui.label(theme::dim("None yet — Guard actions appear here"));
        } else {
            egui::ScrollArea::vertical()
                .id_salt("event_log")
                .max_height(180.0)
                .show(ui, |ui| {
                    for ev in events.iter().take(40) {
                        event_row(ui, ev);
                    }
                });
        }
    }

    fn ui_apps(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Whitelist").size(18.0).strong().color(TEXT));
        ui.label(theme::dim(
            "Whitelisted programs are never soft-throttled, suspended, or terminated. Match by exe name (game.exe) or path substring (\\steam\\).",
        ));
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.button("Reload").clicked() {
                self.config = load_config();
            }
        });
        ui.add_space(8.0);

        // Prefer live status whitelist; fall back to local config
        let live: Vec<String> = {
            let g = self.shared.lock().ok();
            g.and_then(|x| x.status.as_ref().map(|s| s.whitelist.clone()))
                .unwrap_or_else(|| self.config.whitelist.clone())
        };

        if live.is_empty() {
            egui::Frame::NONE
                .fill(theme::BG_PANEL)
                .corner_radius(theme::RADIUS_SM)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(theme::dim("No whitelist entries yet — add games or apps you never want frozen."));
                });
        } else {
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                for p in &live {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("●").color(TEAL));
                        ui.label(egui::RichText::new(p).monospace().color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button(egui::RichText::new("Remove").color(CORAL))
                                .clicked()
                            {
                                let _ = self.cmd_tx.send(ClientRequest::RemoveWhitelist {
                                    entry: p.clone(),
                                });
                                self.config.remove_whitelist(p);
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            });
        }

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label("Add entry:");
            ui.add(
                egui::TextEdit::singleline(&mut self.allow_path_input)
                    .desired_width(340.0)
                    .hint_text(r"e.g. steam.exe  or  \Epic Games\"),
            );
            if ui.button("Whitelist").clicked() {
                let entry = self.allow_path_input.trim().to_string();
                if !entry.is_empty() {
                    let _ = self.cmd_tx.send(ClientRequest::AddWhitelist {
                        entry: entry.clone(),
                    });
                    self.config.add_whitelist(entry);
                    self.allow_path_input.clear();
                }
            }
        });
        ui.add_space(8.0);
        ui.label(theme::dim(
            "Tip: on Monitor, click Whitelist next to a running process to protect it instantly.",
        ));
    }

    fn ui_protect(&mut self, ui: &mut egui::Ui, status: Option<&StatusSnapshot>) {
        ui.label(egui::RichText::new("Protect").size(18.0).strong());
        ui.label(theme::dim(
            "Behavioral heuristics for high-resource abuse / miner-like activity — not antivirus. Never auto-deletes files.",
        ));
        ui.add_space(10.0);

        let Some(s) = status else {
            ui.label("Waiting for status…");
            return;
        };

        if s.recent_abuse.is_empty() {
            egui::Frame::NONE
                .fill(TEAL.gamma_multiply(0.12))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("No abuse signals right now")
                            .color(TEAL)
                            .strong(),
                    );
                });
        } else {
            for a in &s.recent_abuse {
                if widgets::abuse_alert_card(ui, a.pid, &a.name, a.score, &a.reasons) {
                    let _ = self.cmd_tx.send(ClientRequest::TrustPid { pid: a.pid });
                }
            }
        }
    }
}

fn shorten_toast(msg: &str) -> String {
    // Service often returns "paused until <RFC3339>" — keep the hero clean.
    if let Some(rest) = msg.strip_prefix("paused until ") {
        if let Some(t) = rest.split('T').nth(1) {
            let hhmm = t.get(..5).unwrap_or(t);
            return format!("Paused until {hhmm}");
        }
        return "Paused".into();
    }
    if msg.len() > 64 {
        format!("{}…", &msg[..61])
    } else {
        msg.to_string()
    }
}

fn event_row(ui: &mut egui::Ui, ev: &GuardianEvent) {
    let (kind, detail, when) = match ev {
        GuardianEvent::Pressure { band, score, at } => (
            "pressure",
            format!("{band} ({score:.2})"),
            at.format("%H:%M:%S").to_string(),
        ),
        GuardianEvent::Throttle {
            name,
            pid,
            level,
            reason,
            at,
        } => {
            let kind = if reason.starts_with("disk_control:")
                || reason.starts_with("mem_control:")
                || reason.starts_with("disk_lock:")
                || reason.starts_with("mem_lock:")
            {
                "capped"
            } else {
                "throttle"
            };
            (
                kind,
                format!("{name} pid {pid} {:?} · {reason}", level),
                at.format("%H:%M:%S").to_string(),
            )
        }
        GuardianEvent::Suspend {
            name,
            pid,
            reason,
            at,
        } => (
            "suspend",
            format!("{name} pid {pid} · {reason}"),
            at.format("%H:%M:%S").to_string(),
        ),
        GuardianEvent::Resume {
            name,
            pid,
            reason,
            at,
        } => (
            "resume",
            format!("{name} pid {pid} · {reason}"),
            at.format("%H:%M:%S").to_string(),
        ),
        GuardianEvent::Abuse {
            name,
            pid,
            score,
            reasons,
            at,
        } => (
            "abuse",
            format!("{name} pid {pid} score {score} · {}", reasons.join(",")),
            at.format("%H:%M:%S").to_string(),
        ),
        GuardianEvent::Info { message, at } => {
            ("info", message.clone(), at.format("%H:%M:%S").to_string())
        }
    };
    let color = match kind {
        "suspend" | "abuse" => CORAL,
        "throttle" => theme::AMBER,
        "resume" => TEAL,
        _ => TEXT_DIM,
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&when).monospace().size(11.0).color(TEXT_DIM));
        ui.label(
            egui::RichText::new(kind.to_uppercase())
                .size(11.0)
                .strong()
                .color(color),
        );
        ui.label(egui::RichText::new(detail).size(12.0).color(TEXT));
    });
}

