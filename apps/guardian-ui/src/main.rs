//! Unstick — polished desktop client.

mod app;
mod chrome;
mod client;
mod history;
mod theme;
mod widgets;
mod win_round;

use app::UnstickApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([780.0, 520.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_title("Unstick"),
        ..Default::default()
    };
    eframe::run_native(
        "Unstick",
        options,
        Box::new(|cc| Ok(Box::new(UnstickApp::new(cc)))),
    )
}
