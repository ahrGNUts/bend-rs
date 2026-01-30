//! Image preview UI component

use crate::app::BendApp;
use eframe::egui;

/// Show the image preview panel
pub fn show(ui: &mut egui::Ui, app: &BendApp) {
    if let Some(texture) = &app.preview_texture {
        // Get available size for the preview
        let available_size = ui.available_size();

        // Calculate scaled size maintaining aspect ratio
        let texture_size = texture.size_vec2();
        let scale = (available_size.x / texture_size.x).min(available_size.y / texture_size.y);
        let scaled_size = texture_size * scale.min(1.0); // Don't upscale

        // Center the image
        ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
            // Show decode error indicator if present
            if app.decode_error.is_some() {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "\u{26A0} Preview may be stale (decode error)",
                    );
                });
            }

            // Display the image
            ui.image((texture.id(), scaled_size));
        });
    } else {
        // No preview available
        ui.centered_and_justified(|ui| {
            if app.decode_error.is_some() {
                ui.vertical_centered(|ui| {
                    // Broken image indicator
                    ui.label(
                        egui::RichText::new("\u{1F5BC}")
                            .size(64.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.label("Unable to decode image");
                    if let Some(err) = &app.decode_error {
                        ui.label(egui::RichText::new(err).small().color(egui::Color32::GRAY));
                    }
                });
            } else {
                ui.label("Loading preview...");
            }
        });
    }
}
