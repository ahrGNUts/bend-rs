//! bend-rs: A cross-platform databending application for glitch art
//!
//! This application provides a unified workflow for databending - manipulating
//! raw image bytes to create glitch art effects.

mod app;
mod editor;
mod formats;
mod ui;

use app::BendApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "bend-rs - Databending Studio",
        options,
        Box::new(|cc| Ok(Box::new(BendApp::new(cc)))),
    )
}
