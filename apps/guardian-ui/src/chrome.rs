//! Custom frameless window chrome — slim title bar, controls, outer border.

use egui::{
    Align2, Color32, Context, CornerRadius, FontId, LayerId, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2, ViewportCommand,
};

use crate::theme::{BG_CHROME, CORAL, LINE, TEAL, TEXT, TEXT_DIM};

pub const TITLE_BAR_HEIGHT: f32 = 32.0;
const CONTROL_W: f32 = 40.0;
const ICON_SIZE: f32 = 16.0;

pub fn title_bar(ctx: &Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

    egui::TopBottomPanel::top("window_chrome")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(BG_CHROME))
        .show(ctx, |ui| {
            let full = ui.max_rect();
            ui.painter().rect_filled(
                Rect::from_min_max(full.left_top(), Pos2::new(full.right(), full.top() + 2.0)),
                0.0,
                TEAL,
            );

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                draw_shield_icon(ui, ICON_SIZE);
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("v{}", guardian_core::VERSION))
                        .size(11.0)
                        .color(TEXT_DIM)
                        .monospace(),
                );

                // Wide drag strip (no duplicate product title — brand lives in header)
                let drag_w = ui.available_width() - CONTROL_W * 3.0 - 8.0;
                let (drag_rect, drag) =
                    ui.allocate_exact_size(Vec2::new(drag_w.max(40.0), TITLE_BAR_HEIGHT - 4.0), Sense::click().union(Sense::drag()));
                if drag.hovered() {
                    ui.painter().rect_filled(drag_rect, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 6));
                }
                if drag.double_clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                } else if drag.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if window_control(ui, ControlKind::Close).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    if window_control(ui, ControlKind::Maximize).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if window_control(ui, ControlKind::Minimize).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                });
            });
        });
}

pub fn paint_window_border(ctx: &Context) {
    let rect = ctx.screen_rect();
    let painter = ctx.layer_painter(LayerId::background());
    painter.rect_stroke(
        rect.shrink(0.5),
        CornerRadius::same(8),
        Stroke::new(1.0, LINE),
        StrokeKind::Outside,
    );
    let len = 12.0;
    let stroke = Stroke::new(1.5, TEAL.gamma_multiply(0.5));
    painter.line_segment(
        [rect.left_top(), Pos2::new(rect.left() + len, rect.top())],
        stroke,
    );
    painter.line_segment(
        [rect.left_top(), Pos2::new(rect.left(), rect.top() + len)],
        stroke,
    );
    painter.line_segment(
        [rect.right_top(), Pos2::new(rect.right() - len, rect.top())],
        stroke,
    );
    painter.line_segment(
        [rect.right_top(), Pos2::new(rect.right(), rect.top() + len)],
        stroke,
    );
}

enum ControlKind {
    Minimize,
    Maximize,
    Close,
}

fn window_control(ui: &mut Ui, kind: ControlKind) -> Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(CONTROL_W, TITLE_BAR_HEIGHT - 2.0),
        Sense::click(),
    );
    let hovered = resp.hovered();
    let painter = ui.painter();

    if hovered {
        let bg = match kind {
            ControlKind::Close => CORAL.gamma_multiply(0.88),
            _ => Color32::from_rgb(0x38, 0x3F, 0x48),
        };
        painter.rect_filled(rect, CornerRadius::same(4), bg);
    }

    let center = rect.center();
    let icon_color = if hovered { TEXT } else { TEXT_DIM };
    match kind {
        ControlKind::Minimize => {
            painter.line_segment(
                [
                    Pos2::new(center.x - 5.0, center.y + 3.0),
                    Pos2::new(center.x + 5.0, center.y + 3.0),
                ],
                Stroke::new(1.5, icon_color),
            );
        }
        ControlKind::Maximize => {
            let r = Rect::from_center_size(center, Vec2::splat(9.0));
            painter.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.5, icon_color), StrokeKind::Inside);
        }
        ControlKind::Close => {
            painter.line_segment(
                [center + Vec2::new(-4.5, -4.5), center + Vec2::new(4.5, 4.5)],
                Stroke::new(1.5, icon_color),
            );
            painter.line_segment(
                [center + Vec2::new(4.5, -4.5), center + Vec2::new(-4.5, 4.5)],
                Stroke::new(1.5, icon_color),
            );
        }
    }
    resp
}

fn draw_shield_icon(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size + 4.0), Sense::hover());
    let painter = ui.painter();
    let c = rect.center();
    let r = size * 0.45;
    painter.circle_filled(c, r, TEAL.gamma_multiply(0.2));
    painter.circle_stroke(c, r, Stroke::new(1.2, TEAL));
    painter.text(c, Align2::CENTER_CENTER, "U", FontId::proportional(7.5), TEAL);
}
