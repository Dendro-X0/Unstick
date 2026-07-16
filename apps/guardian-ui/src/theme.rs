use egui::{Color32, RichText, Stroke, Vec2};

pub const BG: Color32 = Color32::from_rgb(0x18, 0x1B, 0x20);
pub const BG_CHROME: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const BG_ELEV: Color32 = Color32::from_rgb(0x20, 0x24, 0x2A);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x1E, 0x22, 0x28);
pub const BG_TAB: Color32 = Color32::from_rgb(0x26, 0x2B, 0x32);
pub const BG_TAB_ACTIVE: Color32 = Color32::from_rgb(0x30, 0x36, 0x3E);
pub const TEXT: Color32 = Color32::from_rgb(0xF4, 0xF6, 0xF8);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8E, 0x98, 0xA4);
pub const TEAL: Color32 = Color32::from_rgb(0x39, 0xC6, 0xB4);
pub const TEAL_DIM: Color32 = Color32::from_rgb(0x2A, 0x9D, 0x8F);
pub const CORAL: Color32 = Color32::from_rgb(0xE6, 0x3D, 0x4A);
pub const AMBER: Color32 = Color32::from_rgb(0xE6, 0x8D, 0x50);
pub const LINE: Color32 = Color32::from_rgb(0x34, 0x3A, 0x44);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x28, 0x2D, 0x35);

pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 10.0;

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = BG;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.inactive.bg_fill = BG_TAB;
    style.visuals.widgets.hovered.bg_fill = BG_TAB_ACTIVE;
    style.visuals.widgets.active.bg_fill = BG_TAB_ACTIVE;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.selection.bg_fill = TEAL.gamma_multiply(0.35);
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    ctx.set_style(style);
}

pub fn brand_title() -> RichText {
    RichText::new("UNSTICK")
        .size(24.0)
        .strong()
        .color(TEXT)
}

pub fn dim(s: impl Into<String>) -> RichText {
    RichText::new(s).size(13.0).color(TEXT_DIM)
}
