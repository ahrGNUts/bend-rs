use eframe::egui;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::BendApp;
use crate::formats::GifParser;
use crate::formats::ImageFormat;

/// Debounce delay for preview updates (milliseconds)
const PREVIEW_DEBOUNCE_MS: u64 = 150;

/// Minimum frame delay to prevent busy-looping (browsers clamp to 10ms)
const MIN_FRAME_DELAY_MS: u64 = 10;

/// State for animated GIF playback
pub struct AnimationState {
    /// Per-frame decoded images (CPU memory, not GPU textures)
    pub frames: Vec<egui::ColorImage>,
    /// Per-frame delay durations (clamped to minimum 10ms)
    pub delays: Vec<Duration>,
    /// Current frame index being displayed
    pub current_frame: usize,
    /// Whether animation is auto-playing
    pub playing: bool,
    /// Timestamp when current frame was first displayed
    pub last_frame_time: Instant,
}

/// Result type for background animated GIF decode
type AnimationDecodeResult = Result<(Vec<egui::ColorImage>, Vec<Duration>), image::ImageError>;

/// State for image preview rendering and comparison
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
    /// Animation state for multi-frame GIFs (None for static images)
    pub animation: Option<AnimationState>,
    /// Animation state for original buffer (comparison mode)
    pub original_animation: Option<AnimationState>,
    /// Pending background decode for animated GIF re-decode on edits
    pub pending_animation: Option<mpsc::Receiver<AnimationDecodeResult>>,
    /// Pending background decode for original animated GIF (comparison mode)
    pub pending_original_animation: Option<mpsc::Receiver<AnimationDecodeResult>>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            texture: None,
            original_texture: None,
            dirty: false,
            decode_error: None,
            comparison_mode: false,
            last_edit_time: None,
            animation: None,
            original_animation: None,
            pending_animation: None,
            pending_original_animation: None,
        }
    }
}

/// Decode an animated GIF into per-frame ColorImages and delay durations.
/// Uses the `image` crate's AnimationDecoder which handles frame compositing
/// (disposal methods are applied internally, each frame is a full canvas).
fn decode_animated_gif(
    data: &[u8],
) -> Result<(Vec<egui::ColorImage>, Vec<Duration>), image::ImageError> {
    use image::AnimationDecoder;

    let cursor = std::io::Cursor::new(data);
    let decoder = image::codecs::gif::GifDecoder::new(cursor)?;
    let frames = AnimationDecoder::into_frames(decoder).collect_frames()?;

    let mut images = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());

    for frame in &frames {
        // Extract delay — numer_denom_ms returns (numerator, denominator)
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 {
            MIN_FRAME_DELAY_MS
        } else {
            (numer / denom).max(MIN_FRAME_DELAY_MS as u32) as u64
        };
        delays.push(Duration::from_millis(delay_ms));

        // Convert frame buffer to egui::ColorImage
        let rgba = frame.buffer();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.as_raw();
        images.push(egui::ColorImage::from_rgba_unmultiplied(size, pixels));
    }

    Ok((images, delays))
}

/// Upload a ColorImage to a GPU texture handle
fn upload_frame(ctx: &egui::Context, image: &egui::ColorImage, name: &str) -> egui::TextureHandle {
    ctx.load_texture(name, image.clone(), egui::TextureOptions::LINEAR)
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

    /// Advance animation frame if playing and delay has elapsed.
    /// Must be called unconditionally from BendApp::update() — not guarded by dirty flag.
    pub fn advance_animation(&mut self, ctx: &egui::Context) {
        // Poll pending background animation decode
        if let Some(rx) = &self.preview.pending_animation {
            if let Ok(result) = rx.try_recv() {
                self.preview.pending_animation = None;
                match result {
                    Ok((frames, delays)) => {
                        if frames.len() > 1 {
                            // Preserve playback state
                            let (current_frame, playing) = self
                                .preview
                                .animation
                                .as_ref()
                                .map(|a| (a.current_frame.min(frames.len() - 1), a.playing))
                                .unwrap_or((0, true));

                            let texture = upload_frame(ctx, &frames[current_frame], "preview_anim");
                            self.preview.texture = Some(texture);
                            self.preview.animation = Some(AnimationState {
                                frames,
                                delays,
                                current_frame,
                                playing,
                                last_frame_time: Instant::now(),
                            });
                        } else if frames.len() == 1 {
                            // Single frame — treat as static
                            let texture = upload_frame(ctx, &frames[0], "preview");
                            self.preview.texture = Some(texture);
                            self.preview.animation = None;
                        }
                        self.preview.decode_error = None;
                    }
                    Err(e) => {
                        log::warn!("Background animated GIF decode failed: {}", e);
                        self.preview.decode_error = Some(format!("Decode error: {}", e));
                        // Keep last valid animation state
                    }
                }
            }
        }

        // Poll pending background original animation decode
        if let Some(rx) = &self.preview.pending_original_animation {
            if let Ok(result) = rx.try_recv() {
                self.preview.pending_original_animation = None;
                match result {
                    Ok((frames, delays)) => {
                        if frames.len() > 1 {
                            let texture = upload_frame(ctx, &frames[0], "original_anim");
                            self.preview.original_texture = Some(texture);
                            self.preview.original_animation = Some(AnimationState {
                                frames,
                                delays,
                                current_frame: 0,
                                playing: true,
                                last_frame_time: Instant::now(),
                            });
                        } else if frames.len() == 1 {
                            let texture = upload_frame(ctx, &frames[0], "original");
                            self.preview.original_texture = Some(texture);
                        }
                    }
                    Err(e) => {
                        log::warn!("Background original GIF decode failed: {}", e);
                    }
                }
            }
        }

        // Advance working buffer animation
        if let Some(anim) = &mut self.preview.animation {
            if anim.playing {
                let elapsed = anim.last_frame_time.elapsed();
                let current_delay = anim.delays[anim.current_frame];

                if elapsed >= current_delay {
                    // Advance to next frame (wrap around)
                    anim.current_frame = (anim.current_frame + 1) % anim.frames.len();
                    anim.last_frame_time = Instant::now();

                    // Upload the new frame
                    let texture =
                        upload_frame(ctx, &anim.frames[anim.current_frame], "preview_anim");
                    self.preview.texture = Some(texture);
                }

                // Schedule repaint for next frame advance
                let anim = self.preview.animation.as_ref().unwrap();
                let remaining =
                    anim.delays[anim.current_frame].saturating_sub(anim.last_frame_time.elapsed());
                ctx.request_repaint_after(remaining);
            }
        }

        // Advance original buffer animation (comparison mode)
        if let Some(anim) = &mut self.preview.original_animation {
            if anim.playing {
                let elapsed = anim.last_frame_time.elapsed();
                let current_delay = anim.delays[anim.current_frame];

                if elapsed >= current_delay {
                    anim.current_frame = (anim.current_frame + 1) % anim.frames.len();
                    anim.last_frame_time = Instant::now();

                    let texture =
                        upload_frame(ctx, &anim.frames[anim.current_frame], "original_anim");
                    self.preview.original_texture = Some(texture);
                }

                let anim = self.preview.original_animation.as_ref().unwrap();
                let remaining =
                    anim.delays[anim.current_frame].saturating_sub(anim.last_frame_time.elapsed());
                ctx.request_repaint_after(remaining);
            }
        }
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

        let working = editor.working();

        // Check if this is a GIF
        if GifParser.can_parse(working) {
            // Spawn background decode for animated GIF
            let data = working.to_vec();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let result = decode_animated_gif(&data);
                let _ = tx.send(result);
            });
            self.preview.pending_animation = Some(rx);

            // Also decode original for comparison if needed (on background thread)
            if self.preview.comparison_mode
                && self.preview.original_animation.is_none()
                && self.preview.pending_original_animation.is_none()
            {
                let original_data = editor.original().to_vec();
                if GifParser.can_parse(&original_data) {
                    let (tx, rx) = mpsc::channel();
                    std::thread::spawn(move || {
                        let result = decode_animated_gif(&original_data);
                        let _ = tx.send(result);
                    });
                    self.preview.pending_original_animation = Some(rx);
                }
            }
        } else {
            // Non-GIF: use existing static decode path
            match Self::decode_to_texture(ctx, working, "preview") {
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

            // Clear any stale animation state
            self.preview.animation = None;
            self.preview.original_animation = None;

            // Also update original texture if not yet loaded
            if self.preview.original_texture.is_none() {
                if let Ok(texture) = Self::decode_to_texture(ctx, editor.original(), "original") {
                    self.preview.original_texture = Some(texture);
                }
            }
        }

        self.preview.dirty = false;
    }

    /// Set the animation to a specific frame (used by UI controls)
    pub fn set_animation_frame(&mut self, ctx: &egui::Context, frame_index: usize) {
        if let Some(anim) = &mut self.preview.animation {
            let idx = frame_index.min(anim.frames.len().saturating_sub(1));
            anim.current_frame = idx;
            anim.last_frame_time = Instant::now();

            let texture = upload_frame(ctx, &anim.frames[idx], "preview_anim");
            self.preview.texture = Some(texture);
        }

        // Sync original animation if in comparison mode
        if let Some(anim) = &mut self.preview.original_animation {
            let idx = frame_index.min(anim.frames.len().saturating_sub(1));
            anim.current_frame = idx;
            anim.last_frame_time = Instant::now();

            let texture = upload_frame(ctx, &anim.frames[idx], "original_anim");
            self.preview.original_texture = Some(texture);
        }
    }

    /// Toggle play/pause for animation
    pub fn toggle_animation_playback(&mut self) {
        if let Some(anim) = &mut self.preview.animation {
            anim.playing = !anim.playing;
            if anim.playing {
                anim.last_frame_time = Instant::now();
            }
        }
        if let Some(anim) = &mut self.preview.original_animation {
            anim.playing = !anim.playing;
            if anim.playing {
                anim.last_frame_time = Instant::now();
            }
        }
    }

    /// Pause animation (used when stepping frame-by-frame)
    pub fn pause_animation(&mut self) {
        if let Some(anim) = &mut self.preview.animation {
            anim.playing = false;
        }
        if let Some(anim) = &mut self.preview.original_animation {
            anim.playing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_animated_gif_minimal() {
        // Build a minimal valid 2-frame animated GIF
        let gif = build_test_animated_gif();
        let result = decode_animated_gif(&gif);
        assert!(result.is_ok(), "decode failed: {:?}", result.err());

        let (frames, delays) = result.unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(delays.len(), 2);

        // Each frame should be 1x1 pixel
        for frame in &frames {
            assert_eq!(frame.size, [1, 1]);
        }

        // Delays should be at least MIN_FRAME_DELAY_MS
        for delay in &delays {
            assert!(delay.as_millis() >= MIN_FRAME_DELAY_MS as u128);
        }
    }

    #[test]
    fn test_decode_animated_gif_single_frame() {
        let gif = build_test_single_frame_gif();
        let result = decode_animated_gif(&gif);
        assert!(result.is_ok());

        let (frames, delays) = result.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(delays.len(), 1);
    }

    #[test]
    fn test_decode_animated_gif_invalid_data() {
        let result = decode_animated_gif(b"not a gif");
        assert!(result.is_err());
    }

    #[test]
    fn test_animation_state_defaults() {
        let anim = AnimationState {
            frames: vec![],
            delays: vec![],
            current_frame: 0,
            playing: true,
            last_frame_time: Instant::now(),
        };
        assert!(anim.playing);
        assert_eq!(anim.current_frame, 0);
    }

    #[test]
    fn test_zero_delay_clamped() {
        // GIF with 0 delay should be clamped to MIN_FRAME_DELAY_MS
        let gif = build_test_zero_delay_gif();
        let result = decode_animated_gif(&gif);
        assert!(result.is_ok());

        let (_, delays) = result.unwrap();
        for delay in &delays {
            assert!(
                delay.as_millis() >= MIN_FRAME_DELAY_MS as u128,
                "Delay {}ms is below minimum {}ms",
                delay.as_millis(),
                MIN_FRAME_DELAY_MS
            );
        }
    }

    // --- Test GIF builders ---

    fn build_test_animated_gif() -> Vec<u8> {
        let mut gif = Vec::new();
        gif.extend_from_slice(b"GIF89a");
        // LSD: 1x1, GCT with 2 entries
        gif.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00]);
        // GCT: black + white
        gif.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF]);
        // NETSCAPE extension for looping
        gif.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        gif.extend_from_slice(b"NETSCAPE2.0");
        gif.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);

        // Frame 1: GCE + Image Descriptor + Image Data
        gif.extend_from_slice(&[0x21, 0xF9, 0x04, 0x00, 0x0A, 0x00, 0x00, 0x00]); // 10cs delay
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);

        // Frame 2: GCE + Image Descriptor + Image Data
        gif.extend_from_slice(&[0x21, 0xF9, 0x04, 0x00, 0x14, 0x00, 0x00, 0x00]); // 20cs delay
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);

        gif.push(0x3B); // Trailer
        gif
    }

    fn build_test_single_frame_gif() -> Vec<u8> {
        let mut gif = Vec::new();
        gif.extend_from_slice(b"GIF89a");
        gif.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00]);
        gif.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF]);
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);
        gif.push(0x3B);
        gif
    }

    fn build_test_zero_delay_gif() -> Vec<u8> {
        let mut gif = Vec::new();
        gif.extend_from_slice(b"GIF89a");
        gif.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00]);
        gif.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF]);
        gif.extend_from_slice(&[0x21, 0xFF, 0x0B]);
        gif.extend_from_slice(b"NETSCAPE2.0");
        gif.extend_from_slice(&[0x03, 0x01, 0x00, 0x00, 0x00]);

        // Frame with 0 delay
        gif.extend_from_slice(&[0x21, 0xF9, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00]); // 0cs delay
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);

        gif.push(0x3B);
        gif
    }
}
