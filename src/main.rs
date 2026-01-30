//! bend-rs: A cross-platform databending application for glitch art
//!
//! This application provides a unified workflow for databending - manipulating
//! raw image bytes to create glitch art effects.

mod app;
mod editor;
mod formats;
mod settings;
mod ui;

use app::BendApp;
use eframe::NativeOptions;
use settings::AppSettings;

fn main() -> eframe::Result<()> {
    env_logger::init();

    // Load settings for window size
    let settings = AppSettings::load();

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([settings.window_width, settings.window_height])
            .with_min_inner_size([800.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "bend-rs - Databending Studio",
        options,
        Box::new(|cc| Ok(Box::new(BendApp::new(cc, settings)))),
    )
}
