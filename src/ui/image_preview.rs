//! Image preview UI component

use crate::app::PreviewState;
use crate::ui::theme::AppColors;
use crate::ui::PointerCursor;
use eframe::egui;

/// Show the image preview panel with optional comparison mode
pub fn show(ui: &mut egui::Ui, preview: &mut PreviewState, colors: &AppColors) {
    // Comparison mode toggle at the top
    ui.horizontal(|ui| {
        ui.checkbox(&mut preview.comparison_mode, "Compare with Original");
    });

    // Animation controls (if animated GIF is loaded)
    show_animation_controls(ui, preview);

    ui.add_space(4.0);

    if preview.comparison_mode {
        show_comparison_view(ui, preview, colors);
    } else {
        show_single_preview(ui, preview, colors);
    }
}

/// Show animation controls when an animated GIF is loaded
fn show_animation_controls(ui: &mut egui::Ui, preview: &mut PreviewState) {
    // Guard: only show controls if we have a multi-frame animation
    let has_animation = preview
        .animation
        .as_ref()
        .is_some_and(|a| a.frames.len() > 1);
    if !has_animation {
        return;
    }

    // Read frame_count once (immutable for the duration of this function)
    let frame_count = preview.animation.as_ref().unwrap().frames.len();

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        // First frame button
        if ui
            .button("|<")
            .pointer_cursor()
            .on_hover_text("First frame")
            .clicked()
        {
            preview.pause_animation();
            preview.set_animation_frame(0);
        }

        // Previous frame button
        if ui
            .button("<")
            .pointer_cursor()
            .on_hover_text("Previous frame")
            .clicked()
        {
            preview.pause_animation();
            let current = preview.animation.as_ref().unwrap().current_frame;
            let prev = if current == 0 {
                frame_count - 1
            } else {
                current - 1
            };
            preview.set_animation_frame(prev);
        }

        // Play/Pause toggle — read fresh playing state
        let is_playing = preview.animation.as_ref().unwrap().playing;
        let play_label = if is_playing { "Pause" } else { "Play" };
        if ui.button(play_label).pointer_cursor().clicked() {
            preview.toggle_animation_playback();
        }

        // Next frame button
        if ui
            .button(">")
            .pointer_cursor()
            .on_hover_text("Next frame")
            .clicked()
        {
            preview.pause_animation();
            let current = preview.animation.as_ref().unwrap().current_frame;
            let next = (current + 1) % frame_count;
            preview.set_animation_frame(next);
        }

        // Last frame button
        if ui
            .button(">|")
            .pointer_cursor()
            .on_hover_text("Last frame")
            .clicked()
        {
            preview.pause_animation();
            preview.set_animation_frame(frame_count - 1);
        }

        ui.separator();

        // Frame label — read fresh current_frame after all button mutations
        let current_frame = preview.animation.as_ref().unwrap().current_frame;
        let mut label = format!("Frame {} / {}", current_frame + 1, frame_count);

        // Only append original count when it differs (comparison mode)
        if preview.comparison_mode {
            if let Some(orig) = preview.original_animation.as_ref() {
                if orig.frames.len() != frame_count {
                    label.push_str(&format!(" (original: {})", orig.frames.len()));
                }
            }
        }
        ui.label(label);
    });
}

/// Show the comparison view with original and current images side-by-side
fn show_comparison_view(ui: &mut egui::Ui, preview: &PreviewState, colors: &AppColors) {
    let available_size = ui.available_size();

    // Calculate the maximum size for each image (half the width minus spacing)
    let half_width = (available_size.x - 20.0) / 2.0;
    let max_image_size = egui::vec2(half_width, available_size.y - 30.0);

    // Calculate a unified scale factor based on both textures
    let scale = calculate_unified_scale(preview, max_image_size);

    ui.horizontal(|ui| {
        // Left: Original image
        ui.vertical(|ui| {
            ui.heading("Original");
            show_texture_scaled(ui, preview.original_texture.as_ref(), scale, max_image_size);
        });

        ui.separator();

        // Right: Current (working) image
        ui.vertical(|ui| {
            ui.heading("Current");
            // Show decode error indicator if present
            if preview.decode_error.is_some() {
                ui.horizontal(|ui| {
                    ui.colored_label(colors.warning_text, "\u{26A0} Preview may be stale");
                });
            }
            show_texture_scaled(ui, preview.texture.as_ref(), scale, max_image_size);
        });
    });
}

/// Calculate a unified scale factor so both images display at the same size
fn calculate_unified_scale(preview: &PreviewState, max_size: egui::Vec2) -> f32 {
    let mut scale = 1.0_f32;

    // Get the texture that determines our scale
    // Use the largest dimensions from either texture
    if let Some(tex) = &preview.original_texture {
        let tex_size = tex.size_vec2();
        let tex_scale = (max_size.x / tex_size.x).min(max_size.y / tex_size.y);
        scale = scale.min(tex_scale);
    }

    if let Some(tex) = &preview.texture {
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
            ui.label(egui::RichText::new("\u{1F5BC}").size(48.0));
        });
    }
}

/// Show a single image preview (current working buffer)
fn show_single_preview(ui: &mut egui::Ui, preview: &PreviewState, colors: &AppColors) {
    if let Some(texture) = &preview.texture {
        // Show decode error indicator if present
        if preview.decode_error.is_some() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    colors.warning_text,
                    "\u{26A0} Preview may be stale (decode error)",
                );
            });
        }

        let available_size = ui.available_size();
        let texture_size = texture.size_vec2();
        let scale = (available_size.x / texture_size.x)
            .min(available_size.y / texture_size.y)
            .min(1.0);

        show_texture_scaled(ui, Some(texture), scale, available_size);
    } else {
        // No preview available
        ui.centered_and_justified(|ui| {
            if preview.decode_error.is_some() {
                ui.vertical_centered(|ui| {
                    // Broken image indicator
                    ui.label(egui::RichText::new("\u{1F5BC}").size(64.0));
                    ui.label("Unable to decode image");
                    if let Some(err) = &preview.decode_error {
                        ui.label(egui::RichText::new(err).small());
                    }
                });
            } else {
                ui.label("Loading preview...");
            }
        });
    }
}
