//! Main application state and egui integration

use crate::editor::EditorState;
use crate::ui::{hex_editor, image_preview};
use eframe::egui;
use std::path::PathBuf;

/// Main application state for bend-rs
///
/// ## Architecture: Dual-Buffer Design
///
/// The application maintains two separate byte buffers:
/// - `original`: Immutable after load. Used for comparison view and as the base
///   for save points. This ensures the source file is never modified.
/// - `working`: All edits apply here. Undo/redo operates on this buffer.
///   This is what gets rendered in the preview.
///
/// This design ensures:
/// 1. Original file is never modified (non-destructive editing)
/// 2. Comparison view always has the pristine original
/// 3. Save points can diff against a stable base
/// 4. Export writes the working buffer to a new file
pub struct BendApp {
    /// Editor state containing buffers, history, and file metadata
    pub editor: Option<EditorState>,

    /// Path to currently loaded file (for display purposes)
    pub current_file: Option<PathBuf>,

    /// Texture handle for the rendered image preview
    pub preview_texture: Option<egui::TextureHandle>,

    /// Texture handle for the original image (comparison mode)
    pub original_texture: Option<egui::TextureHandle>,

    /// Whether the preview needs to be re-rendered
    pub preview_dirty: bool,

    /// Last decode error message (if any)
    pub decode_error: Option<String>,
}

impl BendApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            editor: None,
            current_file: None,
            preview_texture: None,
            original_texture: None,
            preview_dirty: false,
            decode_error: None,
        }
    }

    /// Open a file from a path
    pub fn open_file(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                log::info!("Loaded file: {} ({} bytes)", path.display(), bytes.len());
                self.editor = Some(EditorState::new(bytes));
                self.current_file = Some(path);
                self.preview_dirty = true;
                self.decode_error = None;
                // Clear existing textures - they'll be recreated on next frame
                self.preview_texture = None;
                self.original_texture = None;
            }
            Err(e) => {
                log::error!("Failed to load file: {}", e);
                self.decode_error = Some(format!("Failed to load file: {}", e));
            }
        }
    }

    /// Open file dialog and load selected file
    pub fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["bmp", "jpg", "jpeg"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.open_file(path);
        }
    }

    /// Update the image preview texture from the working buffer
    pub fn update_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_dirty {
            return;
        }

        let Some(editor) = &self.editor else {
            return;
        };

        // Try to decode the working buffer as an image
        match image::load_from_memory(editor.working()) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

                self.preview_texture = Some(ctx.load_texture(
                    "preview",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
                self.decode_error = None;
            }
            Err(e) => {
                log::warn!("Failed to decode image: {}", e);
                self.decode_error = Some(format!("Decode error: {}", e));
                // Keep the old texture as "last valid state"
            }
        }

        // Also update original texture if not yet loaded
        if self.original_texture.is_none() {
            if let Ok(img) = image::load_from_memory(editor.original()) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

                self.original_texture = Some(ctx.load_texture(
                    "original",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        self.preview_dirty = false;
    }
}

impl eframe::App for BendApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle dropped files
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    self.open_file(path.clone());
                }
            }
        });

        // Update preview if needed
        self.update_preview(ctx);

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Status bar at bottom
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(path) = &self.current_file {
                    ui.label(format!("File: {}", path.display()));
                }
                if let Some(editor) = &self.editor {
                    ui.separator();
                    ui.label(format!("{} bytes", editor.working().len()));
                }
                if let Some(err) = &self.decode_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::YELLOW, err);
                }
            });
        });

        // Main content area with split view
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.editor.is_some() {
                // Split view: hex editor on left, image preview on right
                ui.columns(2, |columns| {
                    // Left panel: Hex editor
                    columns[0].group(|ui| {
                        ui.heading("Hex Editor");
                        hex_editor::show(ui, self);
                    });

                    // Right panel: Image preview
                    columns[1].group(|ui| {
                        ui.heading("Preview");
                        image_preview::show(ui, self);
                    });
                });
            } else {
                // No file loaded - show welcome message
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Welcome to bend-rs");
                        ui.add_space(20.0);
                        ui.label("Open a BMP or JPEG file to begin databending.");
                        ui.add_space(10.0);
                        ui.label("Drag and drop a file here, or use File > Open");
                        ui.add_space(20.0);
                        if ui.button("Open File...").clicked() {
                            self.open_file_dialog();
                        }
                    });
                });
            }
        });
    }
}
