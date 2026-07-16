use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use guardian_core::{
    load_config, status_path, ClientRequest, DiskLockMode, GuardianConfig, PressureBand,
    ServerPush, StatusSnapshot, ThrottleSummary,
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
}

const GAUGE_SEGMENTS: f32 = 15.0;

impl UnstickApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);

        let shared = Arc::new(Mutex::new(SharedUi {
            status: None,
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
        };
        app.disk_soft_edit = app.config.disk_busy_soft_pct;
        app.disk_hard_edit = app.config.disk_busy_hard_pct;
        app
    }
}

impl eframe::App for UnstickApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.rounded_applied {
            win_round::try_apply(frame);
            self.rounded_applied = true;
        }

        ctx.request_repaint_after(Duration::from_millis(33));
        self.pulse_t = (self.pulse_t + ctx.input(|i| i.stable_dt)) % 1000.0;

        chrome::title_bar(ctx);
        chrome::paint_window_border(ctx);

        let (status, online, toast) = {
            let g = self.shared.lock().ok();
            let g = g.as_ref();
            (
                g.and_then(|x| x.status.clone()),
                g.map(|x| x.online).unwrap_or(false),
                g.and_then(|x| x.toast.clone()),
            )
        };

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
            self.cpu_disp = lerp(self.cpu_disp, s.cpu_percent / 100.0, t);
            self.ram_disp = lerp(self.ram_disp, ram, t);
            self.disk_disp = lerp(self.disk_disp, s.disk_busy_percent / 100.0, t);
            self.pressure_disp = lerp(self.pressure_disp, s.pressure_score, t);
            self.cpu_lit = lerp(self.cpu_lit, self.cpu_disp * GAUGE_SEGMENTS, t_seg);
            self.ram_lit = lerp(self.ram_lit, self.ram_disp * GAUGE_SEGMENTS, t_seg);
            self.disk_lit = lerp(self.disk_lit, self.disk_disp * GAUGE_SEGMENTS, t_seg);
            self.pressure_lit = lerp(
                self.pressure_lit,
                self.pressure_disp * GAUGE_SEGMENTS,
                t_seg,
            );
        }

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
                    ui.add_space(8.0);
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
                    ui.add_space(10.0);
                    ui.label(theme::dim("Keeps Dev & Play responsive"));
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
                ui.add_space(4.0);
                ui.horizontal_centered(|ui| {
                    let w = ((ui.available_width() - 56.0) / 4.0).clamp(130.0, 220.0);
                    widgets::led_gauge(ui, "CPU", self.cpu_disp, self.cpu_lit, w);
                    widgets::gauge_divider(ui);
                    widgets::led_gauge(ui, "RAM", self.ram_disp, self.ram_lit, w);
                    widgets::gauge_divider(ui);
                    widgets::led_gauge(ui, "DISK", self.disk_disp, self.disk_lit, w);
                    widgets::gauge_divider(ui);
                    widgets::led_gauge(ui, "PRESSURE", self.pressure_disp, self.pressure_lit, w);
                });
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
                        ui.label(egui::RichText::new(msg).color(TEAL));
                        ui.add_space(8.0);
                    }
                }

                match self.tab {
                    Tab::Guard => self.ui_guard(ui, status.as_ref()),
                    Tab::Monitor => self.ui_monitor(ui, status.as_ref()),
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
        let denied = status.map(|s| s.apply_denied.as_slice()).unwrap_or(&[]);
        let recovered = status.map(|s| s.recovered_suspends).unwrap_or(0);
        let tripwire = status.and_then(|s| s.tripwire.as_deref());
        let warn_pulse = matches!(
            status.map(|s| s.pressure_band),
            Some(PressureBand::Warn | PressureBand::Throttle | PressureBand::Emergency)
        ) || disk_lock != DiskLockMode::Off;
        let pulse = if warn_pulse {
            (self.pulse_t * 3.0).sin().abs()
        } else {
            0.0
        };

        widgets::paint_hero_backdrop(ui, 140.0);

        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
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
            ui.horizontal_centered(|ui| {
                widgets::pressure_readout(ui, band, score);
                if let Some(tw) = tripwire {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("tripwire:{tw}"))
                            .size(11.0)
                            .color(CORAL)
                            .monospace(),
                    );
                }
            });
            ui.add_space(10.0);
            if suspended_n > 0 {
                egui::Frame::NONE
                    .fill(CORAL.gamma_multiply(0.12))
                    .corner_radius(theme::RADIUS_SM)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .stroke(egui::Stroke::new(1.0, CORAL.gamma_multiply(0.35)))
                    .show(ui, |ui| {
                        ui.set_max_width(520.0);
                        ui.label(
                            egui::RichText::new(
                                "Safety: some background processes are paused to keep the desktop responsive. They auto-resume when pressure drops, after the max pause timer, or when you Pause Guard. Whitelisted apps are never paused.",
                            )
                            .size(12.0)
                            .color(TEXT),
                        );
                    });
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
                        "Could not control {elev} elevated process(es) — run Guard as admin or whitelist them"
                    )
                } else {
                    format!("Could not apply limits to {} process(es)", denied.len())
                };
                ui.label(egui::RichText::new(msg).size(11.0).color(theme::AMBER));
                ui.add_space(6.0);
            }
            ui.horizontal_centered(|ui| {
                let mut on = critical_on;
                if ui
                    .checkbox(
                        &mut on,
                        egui::RichText::new("Critical Guard")
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
                if disk_lock != DiskLockMode::Off {
                    ui.add_space(8.0);
                    let dl_color = match disk_lock {
                        DiskLockMode::Hard => CORAL,
                        DiskLockMode::Soft => theme::AMBER,
                        DiskLockMode::Off => TEXT_DIM,
                    };
                    let dl_label = match disk_lock {
                        DiskLockMode::Hard => {
                            if let Some(s) = status {
                                format!("Disk Lock HARD · {:.0}%", s.disk_lock_hard_pct)
                            } else {
                                "Disk Lock HARD".into()
                            }
                        }
                        DiskLockMode::Soft => {
                            if let Some(s) = status {
                                format!("Disk Lock SOFT · {:.0}%", s.disk_lock_soft_pct)
                            } else {
                                "Disk Lock SOFT".into()
                            }
                        }
                        DiskLockMode::Off => String::new(),
                    };
                    egui::Frame::NONE
                        .fill(dl_color.gamma_multiply(0.15))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .stroke(egui::Stroke::new(1.0, dl_color.gamma_multiply(0.4)))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(dl_label)
                                    .size(12.0)
                                    .strong()
                                    .color(dl_color),
                            );
                        });
                }
            });
            ui.add_space(14.0);
            // Safe disk usage — user thresholds
            egui::Frame::NONE
                .fill(theme::BG_PANEL)
                .corner_radius(theme::RADIUS_SM)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("Safe disk usage")
                            .size(13.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(theme::dim(
                        "Soft: limit offender I/O when Active Time reaches this %. Hard: temporarily pause top disk processes.",
                    ));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Soft").color(TEXT_DIM));
                        ui.add(
                            egui::Slider::new(&mut self.disk_soft_edit, 50.0..=98.0).suffix("%"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Hard").color(TEXT_DIM));
                        ui.add(
                            egui::Slider::new(&mut self.disk_hard_edit, 55.0..=100.0).suffix("%"),
                        );
                    });
                    if self.disk_hard_edit <= self.disk_soft_edit {
                        self.disk_hard_edit = (self.disk_soft_edit + 1.0).min(100.0);
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(egui::RichText::new("Apply thresholds").color(TEAL))
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
                        ui.add_space(8.0);
                        if ui.small_button("85 / 95").clicked() {
                            self.disk_soft_edit = 85.0;
                            self.disk_hard_edit = 95.0;
                        }
                        if ui.small_button("70 / 90").clicked() {
                            self.disk_soft_edit = 70.0;
                            self.disk_hard_edit = 90.0;
                        }
                    });
                });
            ui.add_space(18.0);

            let card_w = ((ui.available_width() - 20.0) / 2.0).clamp(260.0, 360.0);
            ui.horizontal_centered(|ui| {
                widgets::profile_panel(
                    ui,
                    "DEV BUILDS",
                    "Caps cargo / node / MCP workers under pressure",
                    TEAL,
                    card_w,
                );
                ui.add_space(16.0);
                widgets::profile_panel(
                    ui,
                    "GAMES & PLAY",
                    "Protects foreground + shell; soft-throttles background",
                    theme::AMBER,
                    card_w,
                );
            });
        });
    }

    fn ui_monitor(&mut self, ui: &mut egui::Ui, status: Option<&StatusSnapshot>) {
        ui.label(egui::RichText::new("Live consumers").size(18.0).strong().color(TEXT));
        ui.label(theme::dim("Ranked by CPU — soft-throttle targets non-protected offenders"));
        ui.add_space(12.0);

        let spark_w = ((ui.available_width() - 12.0) / 2.0).clamp(200.0, 400.0);
        ui.horizontal(|ui| {
            widgets::sparkline_panel(
                ui,
                "CPU — last 60s",
                &self.history.cpu_slice(),
                TEAL,
                spark_w,
            );
            ui.add_space(12.0);
            widgets::sparkline_panel(
                ui,
                "DISK — last 60s",
                &self.history.disk_slice(),
                theme::AMBER,
                spark_w,
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
        ui.label(egui::RichText::new("Recent throttles").strong().color(TEXT_DIM));
        if s.recent_throttles.is_empty() {
            ui.label(theme::dim("None this session"));
        } else {
            for t in s.recent_throttles.iter().take(6) {
                throttle_row(ui, t);
            }
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

fn throttle_row(ui: &mut egui::Ui, t: &ThrottleSummary) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&t.name).color(TEXT));
        ui.label(egui::RichText::new(format!("pid {}", t.pid)).color(TEXT_DIM));
        ui.label(egui::RichText::new(format!("{:?}", t.level)).color(AMBER_SAFE));
        ui.label(egui::RichText::new(&t.reason).color(TEXT_DIM));
    });
}

const AMBER_SAFE: egui::Color32 = theme::AMBER;
