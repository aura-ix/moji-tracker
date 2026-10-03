mod clipboard;
mod db;
mod japanese;
mod tracker;
mod ui;

use std::sync::mpsc;
use clipboard::spawn_clipboard_watcher;
use ui::MojiTrackerApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([440.0, 165.0])
            .with_min_inner_size([400.0, 150.0])
            .with_title("Moji Tracker"),
        ..Default::default()
    };

    eframe::run_native(
        "Moji Tracker",
        native_options,
        Box::new(|cc| {
            let (tx, rx) = mpsc::channel();
            spawn_clipboard_watcher(tx, cc.egui_ctx.clone());
            Ok(Box::new(MojiTrackerApp::new(cc, rx)))
        }),
    )
}
