use eframe::egui;
use std::time::Instant;

use super::BendApp;

/// Debounce delay for preview updates (milliseconds)
const PREVIEW_DEBOUNCE_MS: u64 = 150;

/// State for image preview rendering and comparison
#[derive(Default)]
pub struct PreviewState {
    /// Texture handle for the rendered image preview
    pub texture: Option<egui::TextureHandle>,
    /// Texture handle for the original image (comparison mode)
    pub original_texture: Option<egui::TextureHandle>,
    /// Whether the preview needs to be re-rendered
    pub dirty: bool,
    /// Last decode error message (if any)
    pub decode_error: Option<String>,
    /// Whether comparison mode is enabled (side-by-side original and current)
    pub comparison_mode: bool,
    /// Timestamp of last edit (for debouncing preview updates)
    pub last_edit_time: Option<Instant>,
}

impl BendApp {
    /// Mark the preview as needing update (with debounce timestamp)
    pub fn mark_preview_dirty(&mut self) {
        self.preview.dirty = true;
        self.preview.last_edit_time = Some(Instant::now());
    }

    /// Decode image data into an egui texture handle.
    fn decode_to_texture(
        ctx: &egui::Context,
        data: &[u8],
        name: &str,
    ) -> Result<egui::TextureHandle, image::ImageError> {
        let img = image::load_from_memory(data)?;
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Ok(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
    }

    /// Update the image preview texture from the working buffer
    /// Uses debouncing to prevent excessive re-renders during rapid editing
    pub fn update_preview(&mut self, ctx: &egui::Context) {
        if !self.preview.dirty {
            return;
        }

        // Debounce: wait for edits to settle before re-rendering
        if let Some(edit_time) = self.preview.last_edit_time {
            let elapsed = edit_time.elapsed();
            let debounce_duration = std::time::Duration::from_millis(PREVIEW_DEBOUNCE_MS);
            if elapsed < debounce_duration {
                // Schedule a repaint after the remaining debounce time
                let remaining = debounce_duration - elapsed;
                ctx.request_repaint_after(remaining);
                return;
            }
        }

        let Some(editor) = &self.editor else {
            return;
        };

        // Try to decode the working buffer as an image
        match Self::decode_to_texture(ctx, editor.working(), "preview") {
            Ok(texture) => {
                self.preview.texture = Some(texture);
                self.preview.decode_error = None;
            }
            Err(e) => {
                log::warn!("Failed to decode image: {}", e);
                self.preview.decode_error = Some(format!("Decode error: {}", e));
                // Keep the old texture as "last valid state"
            }
        }

        // Also update original texture if not yet loaded
        if self.preview.original_texture.is_none() {
            if let Ok(texture) = Self::decode_to_texture(ctx, editor.original(), "original") {
                self.preview.original_texture = Some(texture);
            }
        }

        self.preview.dirty = false;
    }
}
