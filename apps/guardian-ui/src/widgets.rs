use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
    Vec2,
};

use crate::theme::{AMBER, BG_ELEV, BG_PANEL, BG_TAB, BG_TAB_ACTIVE, CORAL, LINE, RADIUS_MD, RADIUS_SM, TEAL, TEAL_DIM, TEXT, TEXT_DIM};

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn paint_hero_backdrop(ui: &mut Ui, center_y_offset: f32) {
    let rect = ui.max_rect();
    let center = Pos2::new(rect.center().x, rect.top() + center_y_offset);
    let painter = ui.painter();
    for i in 0..8 {
        let r = 80.0 + i as f32 * 28.0;
        let a = 0.045 - i as f32 * 0.004;
        if a > 0.0 {
            painter.circle_filled(center, r, TEAL.gamma_multiply(a));
        }
    }
}

pub fn live_badge(ui: &mut Ui, online: bool) {
    let (label, color, dot) = if online {
        ("LIVE", TEAL, TEAL)
    } else {
        ("OFFLINE", CORAL, CORAL)
    };
    egui::Frame::NONE
        .fill(dot.gamma_multiply(0.12))
        .corner_radius(RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .stroke(Stroke::new(1.0, dot.gamma_multiply(0.35)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, dot);
                ui.label(egui::RichText::new(label).size(11.0).strong().color(color));
            });
        });
}

pub fn nav_tab_strip(ui: &mut Ui, labels: &[&str], active: usize) -> Option<usize> {
    let gap = 5.0;
    let n = labels.len().max(1) as f32;
    let total_w = ui.available_width();
    let tab_w = ((total_w - gap * (n - 1.0)) / n).max(90.0);
    let h = 40.0;
    let mut picked = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (i, label) in labels.iter().enumerate() {
            if nav_tab_trapezoid(ui, label, i == active, tab_w, h).clicked() {
                picked = Some(i);
            }
        }
    });
    picked
}

/// Booster-style tab: top edge flares slightly wider than the base.
fn nav_tab_trapezoid(ui: &mut Ui, label: &str, active: bool, width: f32, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let painter = ui.painter();
    let hovered = resp.hovered();
    let flare = 5.0;

    let fill = if active {
        BG_TAB_ACTIVE
    } else if hovered {
        BG_TAB
    } else {
        BG_PANEL
    };

    let pts = vec![
        Pos2::new(rect.left() - flare * 0.35, rect.top()),
        Pos2::new(rect.right() + flare * 0.35, rect.top()),
        Pos2::new(rect.right(), rect.bottom()),
        Pos2::new(rect.left(), rect.bottom()),
    ];
    painter.add(Shape::convex_polygon(
        pts.clone(),
        fill,
        Stroke::new(1.0, if active { TEAL_DIM } else { LINE }),
    ));

    if active {
        let bar_y = rect.bottom() - 2.5;
        painter.line_segment(
            [
                Pos2::new(rect.left() + 10.0, bar_y),
                Pos2::new(rect.right() - 10.0, bar_y),
            ],
            Stroke::new(2.5, TEAL),
        );
    }

    painter.text(
        rect.center() + Vec2::new(0.0, 1.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.5),
        if active { TEXT } else { TEXT_DIM },
    );
    resp
}

/// LED gauge — `lit_segments` animates fractionally (0..15).
/// `width` is the full column width (includes horizontal inset).
pub fn led_gauge(ui: &mut Ui, label: &str, value01: f32, lit_segments: f32, width: f32) {
    let value01 = value01.clamp(0.0, 1.0);
    let segments = 12;
    let lit_f = lit_segments.clamp(0.0, GAUGE_SEGMENTS_UI) * (segments as f32 / GAUGE_SEGMENTS_UI);
    let width = width.max(80.0);
    let inset = 10.0;
    let inner = (width - inset * 2.0).max(56.0);

    ui.allocate_ui_with_layout(
        Vec2::new(width, 58.0),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_min_size(Vec2::new(width, 58.0));
            ui.set_max_width(width);

            ui.horizontal(|ui| {
                ui.set_width(inner);
                ui.label(
                    egui::RichText::new(label)
                        .size(10.5)
                        .color(TEXT_DIM)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{:.0}%", value01 * 100.0))
                            .size(11.0)
                            .color(TEXT)
                            .monospace(),
                    );
                });
            });

            ui.add_space(4.0);
            let height = 14.0;
            let (rect, _resp) = ui.allocate_exact_size(Vec2::new(inner, height), Sense::hover());
            let painter = ui.painter();
            let gap = 2.0;
            let seg_w = ((rect.width() - gap * (segments as f32 - 1.0)) / segments as f32).max(2.0);
            let fill = if value01 >= 0.85 {
                CORAL
            } else if value01 >= 0.70 {
                AMBER
            } else {
                TEAL
            };

            painter.rect_filled(
                rect.expand(2.0),
                CornerRadius::same(3),
                Color32::from_rgb(0x16, 0x19, 0x1E),
            );

            for i in 0..segments {
                let x = rect.left() + i as f32 * (seg_w + gap);
                let seg = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(seg_w, height));
                let frac = (lit_f - i as f32).clamp(0.0, 1.0);
                if frac <= 0.001 {
                    painter.rect_filled(seg, CornerRadius::same(2), Color32::from_rgb(0x2A, 0x2F, 0x36));
                } else {
                    let bright = fill.gamma_multiply(0.55 + 0.45 * frac);
                    painter.rect_filled(seg, CornerRadius::same(2), bright);
                }
            }
        },
    );
}

/// Four equal-width responsive gauge columns with gutters.
pub fn footer_gauge_row(
    ui: &mut Ui,
    cpu: (f32, f32),
    ram: (f32, f32),
    disk: (f32, f32),
    pressure: (f32, f32),
) {
    let avail = ui.available_width();
    let gutter = 14.0;
    let n = 4.0;
    let col_w = ((avail - gutter * (n - 1.0)) / n).max(72.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gutter;
        led_gauge(ui, "CPU", cpu.0, cpu.1, col_w);
        led_gauge(ui, "RAM", ram.0, ram.1, col_w);
        led_gauge(ui, "DISK", disk.0, disk.1, col_w);
        led_gauge(ui, "PRESSURE", pressure.0, pressure.1, col_w);
    });
}

const GAUGE_SEGMENTS_UI: f32 = 15.0;

/// Mini sparkline — fills the current UI column/parent width.
pub fn sparkline_panel(ui: &mut Ui, label: &str, history: &[f32], color: Color32) {
    let height = 44.0;
    let width = ui.available_width().max(100.0);
    egui::Frame::NONE
        .fill(BG_PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(width - 1.0);
            ui.label(
                egui::RichText::new(label)
                    .size(10.5)
                    .color(TEXT_DIM)
                    .strong(),
            );
            let plot_w = ui.available_width().max(60.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(plot_w, height), Sense::hover());
            paint_sparkline(ui, rect, history, color);
            if let Some(&last) = history.last() {
                ui.label(
                    egui::RichText::new(format!("now {:.0}%", last))
                        .size(10.0)
                        .color(color)
                        .monospace(),
                );
            } else {
                ui.label(crate::theme::dim("collecting…"));
            }
        });
}

fn paint_sparkline(ui: &mut Ui, rect: Rect, history: &[f32], color: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(0x14, 0x17, 0x1C));

    if history.len() < 2 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "—",
            FontId::proportional(14.0),
            TEXT_DIM,
        );
        return;
    }

    let max_v = history
        .iter()
        .copied()
        .fold(1.0f32, f32::max)
        .max(10.0);
    let n = history.len();
    let mut points = Vec::with_capacity(n);
    for (i, &v) in history.iter().enumerate() {
        let t = i as f32 / (n - 1).max(1) as f32;
        let x = rect.left() + t * rect.width();
        let y = rect.bottom() - (v / max_v).clamp(0.0, 1.0) * rect.height();
        points.push(Pos2::new(x, y));
    }

    // Fill under curve
    if points.len() >= 2 {
        let mut fill_pts = points.clone();
        fill_pts.push(Pos2::new(rect.right(), rect.bottom()));
        fill_pts.push(Pos2::new(rect.left(), rect.bottom()));
        painter.add(Shape::convex_polygon(
            fill_pts,
            color.gamma_multiply(0.12),
            Stroke::NONE,
        ));
    }

    for w in points.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.8, color));
    }
    if let Some(&last) = points.last() {
        painter.circle_filled(last, 2.5, color);
    }
}

pub struct CtaResult {
    pub clicked: bool,
}

pub fn guard_cta(ui: &mut Ui, armed: bool, pulse: f32) -> CtaResult {
    let diameter = 176.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click());
    let painter = ui.painter();
    let center = rect.center();
    let ring = if armed { CORAL } else { TEAL_DIM };
    let ring_w = if armed {
        7.0 + pulse * 3.0
    } else {
        5.5
    };

    // Soft outer glow rings (armed = coral pulse; paused = calm teal)
    let glow = if armed { ring } else { TEAL };
    for (r_mul, alpha) in [(0.56, 0.07), (0.50, 0.12), (0.44, 0.18)] {
        painter.circle_stroke(
            center,
            diameter * r_mul,
            Stroke::new(
                if armed { 2.0 + pulse } else { 1.5 },
                glow.gamma_multiply(alpha),
            ),
        );
    }

    painter.circle_filled(center, diameter * 0.44, Color32::from_rgb(0x10, 0x12, 0x16));
    painter.circle_stroke(center, diameter * 0.48, Stroke::new(ring_w, ring));
    painter.circle_stroke(
        center,
        diameter * 0.38,
        Stroke::new(1.5, ring.gamma_multiply(0.4)),
    );

    let label = if armed { "ARMED" } else { "PAUSED" };
    painter.text(
        center + Vec2::new(0.0, -8.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(28.0),
        TEXT,
    );
    painter.text(
        center + Vec2::new(0.0, 22.0),
        Align2::CENTER_CENTER,
        if armed {
            "click to pause 15m"
        } else {
            "click to resume"
        },
        FontId::proportional(12.0),
        TEXT_DIM,
    );

    CtaResult {
        clicked: resp.clicked(),
    }
}

pub fn band_chip(ui: &mut Ui, band: &str) {
    let (bg, fg) = match band {
        "emergency" => (CORAL.gamma_multiply(0.22), CORAL),
        "throttle" => (AMBER.gamma_multiply(0.22), AMBER),
        "warn" => (AMBER.gamma_multiply(0.16), AMBER),
        _ => (TEAL.gamma_multiply(0.16), TEAL),
    };
    let text = band.to_uppercase();
    // Fixed compact pill — never stretch with the parent row height.
    let h = 24.0;
    let w = (text.len() as f32 * 8.2 + 22.0).clamp(64.0, 120.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(RADIUS_SM as u8),
        bg,
        Stroke::new(1.0, fg.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.5),
        fg,
    );
}

/// Centered pressure cluster — compact chip + score on one baseline.
pub fn pressure_readout(ui: &mut Ui, band: &str, score: Option<f32>) {
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("PRESSURE")
                .size(10.0)
                .strong()
                .color(TEXT_DIM),
        );
        ui.add_space(6.0);
        ui.allocate_ui_with_layout(
            Vec2::new(200.0, 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // Center the pair inside the allocated strip.
                let pair_w = 150.0;
                let pad = ((ui.available_width() - pair_w) * 0.5).max(0.0);
                ui.add_space(pad);
                band_chip(ui, band);
                ui.add_space(10.0);
                if let Some(s) = score {
                    ui.label(
                        egui::RichText::new(format!("{s:.2}"))
                            .size(15.0)
                            .color(TEXT)
                            .monospace()
                            .strong(),
                    );
                }
            },
        );
    });
}

/// Centered pill used for Controls expand/collapse.
pub fn controls_toggle(ui: &mut Ui, open: bool) -> bool {
    let label = if open { "Controls   v" } else { "Controls   >" };
    let color = if open { TEAL } else { TEXT_DIM };
    let stroke = if open {
        Stroke::new(1.0, TEAL.gamma_multiply(0.55))
    } else {
        Stroke::new(1.0, LINE)
    };
    let mut clicked = false;
    ui.vertical_centered(|ui| {
        let resp = egui::Frame::NONE
            .fill(BG_PANEL)
            .corner_radius(RADIUS_MD)
            .inner_margin(egui::Margin::symmetric(22, 9))
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_min_width(120.0);
                ui.label(egui::RichText::new(label).size(13.0).strong().color(color));
            })
            .response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .interact(Sense::click());
        clicked = resp.clicked();
    });
    clicked
}

pub fn status_chip(ui: &mut Ui, text: impl Into<String>, color: Color32) -> egui::Response {
    egui::Frame::NONE
        .fill(color.gamma_multiply(0.15))
        .corner_radius(RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(12, 5))
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.4)))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text.into())
                    .size(12.0)
                    .strong()
                    .color(color),
            );
        })
        .response
}

#[allow(dead_code)] // kept for future interactive profiles
pub fn profile_panel(ui: &mut Ui, title: &str, blurb: &str, accent: Color32, width: f32) {
    egui::Frame::NONE
        .fill(BG_ELEV)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(18, 14))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.0, accent);
                ui.label(egui::RichText::new(title).strong().color(accent).size(13.0));
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(blurb).color(TEXT_DIM).size(12.0));
        });
}

#[allow(dead_code)]
pub fn gauge_divider(ui: &mut Ui) {
    let h = 52.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, h), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [Pos2::new(x, rect.top() + 4.0), Pos2::new(x, rect.bottom() - 4.0)],
        Stroke::new(1.0, LINE),
    );
}

pub struct AbuseSeverity {
    pub bg: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub label: &'static str,
}

pub fn abuse_severity(score: u32) -> AbuseSeverity {
    if score >= 90 {
        AbuseSeverity {
            bg: CORAL.gamma_multiply(0.22),
            border: CORAL,
            accent: CORAL,
            label: "CRITICAL",
        }
    } else if score >= 80 {
        AbuseSeverity {
            bg: CORAL.gamma_multiply(0.14),
            border: CORAL.gamma_multiply(0.7),
            accent: CORAL,
            label: "HIGH",
        }
    } else {
        AbuseSeverity {
            bg: AMBER.gamma_multiply(0.14),
            border: AMBER.gamma_multiply(0.65),
            accent: AMBER,
            label: "WATCH",
        }
    }
}

/// Severity-colored abuse alert with painted iconography. Returns true if Trust clicked.
pub fn abuse_alert_card(
    ui: &mut Ui,
    pid: u32,
    name: &str,
    score: u32,
    reasons: &[String],
) -> bool {
    let mut trusted = false;
    let sev = abuse_severity(score);
    egui::Frame::NONE
        .fill(sev.bg)
        .stroke(Stroke::new(1.0, sev.border.gamma_multiply(0.55)))
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                paint_abuse_icon(ui, score, sev.accent);
                ui.add_space(10.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(sev.label)
                                .size(10.0)
                                .strong()
                                .color(sev.accent),
                        );
                        ui.label(
                            egui::RichText::new(format!("score {score}"))
                                .size(12.0)
                                .strong()
                                .color(sev.accent)
                                .monospace(),
                        );
                        ui.label(egui::RichText::new(name).strong().color(TEXT));
                        ui.label(
                            egui::RichText::new(format!("pid {pid}"))
                                .color(TEXT_DIM)
                                .monospace(),
                        );
                    });
                    ui.label(
                        egui::RichText::new(reasons.join(" · "))
                            .color(TEXT_DIM)
                            .size(12.0),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new("Trust").size(12.0).color(TEAL))
                        .clicked()
                    {
                        trusted = true;
                    }
                });
            });
        });
    ui.add_space(8.0);
    trusted
}

fn paint_abuse_icon(ui: &mut Ui, score: u32, color: Color32) {
    let size = 28.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let painter = ui.painter();
    painter.circle_filled(c, size * 0.48, color.gamma_multiply(0.18));
    painter.circle_stroke(c, size * 0.48, Stroke::new(1.2, color.gamma_multiply(0.6)));

    if score >= 90 {
        // Shield with crack (critical)
        let r = size * 0.22;
        painter.circle_stroke(c + Vec2::new(0.0, 1.0), r, Stroke::new(1.5, color));
        painter.line_segment(
            [c + Vec2::new(-r * 0.5, -r * 0.2), c + Vec2::new(r * 0.6, r * 0.5)],
            Stroke::new(1.5, color),
        );
    } else if score >= 80 {
        // Alert octagon-ish (diamond)
        let h = size * 0.2;
        let pts = vec![
            c + Vec2::new(0.0, -h),
            c + Vec2::new(h, 0.0),
            c + Vec2::new(0.0, h),
            c + Vec2::new(-h, 0.0),
        ];
        painter.add(Shape::closed_line(pts, Stroke::new(1.5, color)));
        painter.text(c, Align2::CENTER_CENTER, "!", FontId::proportional(12.0), color);
    } else {
        // Warning triangle
        let h = size * 0.24;
        let pts = vec![
            c + Vec2::new(0.0, -h),
            c + Vec2::new(h, h * 0.75),
            c + Vec2::new(-h, h * 0.75),
        ];
        painter.add(Shape::closed_line(pts, Stroke::new(1.5, color)));
        painter.text(
            c + Vec2::new(0.0, h * 0.15),
            Align2::CENTER_CENTER,
            "!",
            FontId::proportional(10.0),
            color,
        );
    }
}
