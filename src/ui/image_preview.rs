//! Image preview UI component

use crate::app::BendApp;
use eframe::egui;

/// Show the image preview panel with optional comparison mode
pub fn show(ui: &mut egui::Ui, app: &mut BendApp) {
    // Comparison mode toggle at the top
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.preview.comparison_mode, "Compare with Original");
    });

    ui.add_space(4.0);

    if app.preview.comparison_mode {
        // Side-by-side comparison view
        show_comparison_view(ui, app);
    } else {
        // Single preview view (current working buffer)
        show_single_preview(ui, app);
    }
}

/// Show the comparison view with original and current images side-by-side
fn show_comparison_view(ui: &mut egui::Ui, app: &BendApp) {
    let available_size = ui.available_size();

    // Calculate the maximum size for each image (half the width minus spacing)
    let half_width = (available_size.x - 20.0) / 2.0;
    let max_image_size = egui::vec2(half_width, available_size.y - 30.0);

    // Calculate a unified scale factor based on both textures
    let scale = calculate_unified_scale(app, max_image_size);

    ui.horizontal(|ui| {
        // Left: Original image
        ui.vertical(|ui| {
            ui.heading("Original");
            show_texture_scaled(
                ui,
                app.preview.original_texture.as_ref(),
                scale,
                max_image_size,
            );
        });

        ui.separator();

        // Right: Current (working) image
        ui.vertical(|ui| {
            ui.heading("Current");
            // Show decode error indicator if present
            if app.preview.decode_error.is_some() {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::YELLOW, "\u{26A0} Preview may be stale");
                });
            }
            show_texture_scaled(ui, app.preview.texture.as_ref(), scale, max_image_size);
        });
    });
}

/// Calculate a unified scale factor so both images display at the same size
fn calculate_unified_scale(app: &BendApp, max_size: egui::Vec2) -> f32 {
    let mut scale = 1.0_f32;

    // Get the texture that determines our scale
    // Use the largest dimensions from either texture
    if let Some(tex) = &app.preview.original_texture {
        let tex_size = tex.size_vec2();
        let tex_scale = (max_size.x / tex_size.x).min(max_size.y / tex_size.y);
        scale = scale.min(tex_scale);
    }

    if let Some(tex) = &app.preview.texture {
        let tex_size = tex.size_vec2();
        let tex_scale = (max_size.x / tex_size.x).min(max_size.y / tex_size.y);
        scale = scale.min(tex_scale);
    }

    // Don't upscale
    scale.min(1.0)
}

/// Show a texture with the given scale factor
fn show_texture_scaled(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    scale: f32,
    max_size: egui::Vec2,
) {
    if let Some(tex) = texture {
        let texture_size = tex.size_vec2();
        let scaled_size = texture_size * scale;

        // Clamp to max_size to prevent overflow
        let clamped_size = egui::vec2(scaled_size.x.min(max_size.x), scaled_size.y.min(max_size.y));

        // Allocate exact space - this reserves the area
        let (rect, _response) = ui.allocate_exact_size(max_size, egui::Sense::hover());

        // Center the clamped image within allocated space
        let image_pos = egui::pos2(
            rect.center().x - clamped_size.x / 2.0,
            rect.center().y - clamped_size.y / 2.0,
        );
        let image_rect = egui::Rect::from_min_size(image_pos, clamped_size);

        // Use a painter clipped to the allocated rect to prevent overflow
        ui.painter().with_clip_rect(rect).image(
            tex.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        // No texture - show placeholder
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("\u{1F5BC}")
                    .size(48.0)
                    .color(egui::Color32::GRAY),
            );
        });
    }
}

/// Show a single image preview (current working buffer)
fn show_single_preview(ui: &mut egui::Ui, app: &BendApp) {
    if let Some(texture) = &app.preview.texture {
        // Show decode error indicator if present
        if app.preview.decode_error.is_some() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "\u{26A0} Preview may be stale (decode error)",
                );
            });
        }

        // Get available size for the preview
        let available_size = ui.available_size();

        // Calculate scaled size maintaining aspect ratio
        let texture_size = texture.size_vec2();
        let scale = (available_size.x / texture_size.x)
            .min(available_size.y / texture_size.y)
            .min(1.0); // Don't upscale
        let scaled_size = texture_size * scale;

        // Clamp to available size
        let clamped_size = egui::vec2(
            scaled_size.x.min(available_size.x),
            scaled_size.y.min(available_size.y),
        );

        // Allocate exact space
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        // Center the image within allocated space
        let image_pos = egui::pos2(
            rect.center().x - clamped_size.x / 2.0,
            rect.center().y - clamped_size.y / 2.0,
        );
        let image_rect = egui::Rect::from_min_size(image_pos, clamped_size);

        // Use a painter clipped to the allocated rect to prevent overflow
        ui.painter().with_clip_rect(rect).image(
            texture.id(),
            image_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        // No preview available
        ui.centered_and_justified(|ui| {
            if app.preview.decode_error.is_some() {
                ui.vertical_centered(|ui| {
                    // Broken image indicator
                    ui.label(
                        egui::RichText::new("\u{1F5BC}")
                            .size(64.0)
                            .color(egui::Color32::GRAY),
                    );
                    ui.label("Unable to decode image");
                    if let Some(err) = &app.preview.decode_error {
                        ui.label(egui::RichText::new(err).small().color(egui::Color32::GRAY));
                    }
                });
            } else {
                ui.label("Loading preview...");
            }
        });
    }
}
