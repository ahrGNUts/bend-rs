//! Main application state and egui integration

use crate::editor::EditorState;
use crate::formats::{parse_file, FileSection, RiskLevel};
use crate::ui::{hex_editor, image_preview, savepoints, structure_tree};
use eframe::egui;
use std::path::PathBuf;
use std::time::Instant;

/// Debounce delay for preview updates (milliseconds)
const PREVIEW_DEBOUNCE_MS: u64 = 150;

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

    /// Whether the close confirmation dialog is showing
    show_close_dialog: bool,

    /// Pending close action (true = confirmed close)
    pending_close: bool,

    /// Timestamp of last edit (for debouncing preview updates)
    last_edit_time: Option<Instant>,

    /// State for the save points panel
    savepoints_state: savepoints::SavePointsPanelState,

    /// Cached parsed file sections for structure visualization
    /// Re-parsed when file is loaded or structure potentially changed
    pub cached_sections: Option<Vec<FileSection>>,
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
            show_close_dialog: false,
            pending_close: false,
            last_edit_time: None,
            savepoints_state: savepoints::SavePointsPanelState::default(),
            cached_sections: None,
        }
    }

    /// Mark the preview as needing update (with debounce timestamp)
    pub fn mark_preview_dirty(&mut self) {
        self.preview_dirty = true;
        self.last_edit_time = Some(Instant::now());
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.editor.as_ref().map_or(false, |e| e.is_modified())
    }

    /// Export the working buffer to a new file
    pub fn export_file(&self) {
        let Some(editor) = &self.editor else {
            return;
        };

        // Suggest a default filename based on original
        let default_name = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{}_glitched", s.to_string_lossy()))
            .unwrap_or_else(|| "export".to_string());

        let extension = self
            .current_file
            .as_ref()
            .and_then(|p| p.extension())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "bmp".to_string());

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.{}", default_name, extension))
            .add_filter("Images", &["bmp", "jpg", "jpeg"])
            .add_filter("All files", &["*"])
            .save_file()
        {
            match std::fs::write(&path, editor.working()) {
                Ok(()) => {
                    log::info!("Exported to: {}", path.display());
                }
                Err(e) => {
                    log::error!("Failed to export: {}", e);
                }
            }
        }
    }

    /// Open a file from a path
    pub fn open_file(&mut self, path: PathBuf) {
        match std::fs::read(&path) {
            Ok(bytes) => {
                log::info!("Loaded file: {} ({} bytes)", path.display(), bytes.len());
                // Parse file structure for section highlighting
                self.cached_sections = parse_file(&bytes);
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

    /// Find the section containing a byte offset
    pub fn section_at_offset(&self, offset: usize) -> Option<&FileSection> {
        fn find_in_sections(sections: &[FileSection], offset: usize) -> Option<&FileSection> {
            for section in sections {
                if offset >= section.start && offset < section.end {
                    // Check children first for more specific match
                    if let Some(child) = find_in_sections(&section.children, offset) {
                        return Some(child);
                    }
                    return Some(section);
                }
            }
            None
        }

        self.cached_sections
            .as_ref()
            .and_then(|sections| find_in_sections(sections, offset))
    }

    /// Get the background color for a byte based on its section's risk level
    pub fn section_color_for_offset(&self, offset: usize) -> Option<egui::Color32> {
        self.section_at_offset(offset).map(|section| {
            // Use subtle background colors for the hex view
            match section.risk {
                RiskLevel::Safe => egui::Color32::from_rgba_unmultiplied(100, 200, 100, 30),
                RiskLevel::Caution => egui::Color32::from_rgba_unmultiplied(200, 180, 80, 30),
                RiskLevel::High => egui::Color32::from_rgba_unmultiplied(200, 130, 80, 30),
                RiskLevel::Critical => egui::Color32::from_rgba_unmultiplied(200, 80, 80, 30),
            }
        })
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
    /// Uses debouncing to prevent excessive re-renders during rapid editing
    pub fn update_preview(&mut self, ctx: &egui::Context) {
        if !self.preview_dirty {
            return;
        }

        // Debounce: wait for edits to settle before re-rendering
        if let Some(edit_time) = self.last_edit_time {
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
        // Handle close confirmation
        if self.pending_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Handle dropped files and keyboard shortcuts
        let mut wants_open = false;
        let mut wants_export = false;
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    self.open_file(path.clone());
                }
            }

            // Global keyboard shortcuts
            let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
            if ctrl && i.key_pressed(egui::Key::O) {
                wants_open = true;
            }
            if ctrl && i.key_pressed(egui::Key::E) && self.editor.is_some() {
                wants_export = true;
            }
        });

        if wants_open {
            self.open_file_dialog();
        }
        if wants_export {
            self.export_file();
        }

        // Update preview if needed
        self.update_preview(ctx);

        // Close confirmation dialog
        if self.show_close_dialog {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. Are you sure you want to exit?");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Export First").clicked() {
                            self.export_file();
                            self.show_close_dialog = false;
                        }
                        if ui.button("Discard & Exit").clicked() {
                            self.pending_close = true;
                            self.show_close_dialog = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_close_dialog = false;
                        }
                    });
                });
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    let has_file = self.editor.is_some();
                    if ui.add_enabled(has_file, egui::Button::new("Export...")).clicked() {
                        self.export_file();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        if self.has_unsaved_changes() {
                            self.show_close_dialog = true;
                        } else {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ui.close_menu();
                    }
                });
            });
        });

        // Status bar at bottom
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Unsaved changes indicator
                if self.has_unsaved_changes() {
                    ui.colored_label(egui::Color32::from_rgb(255, 180, 0), "\u{25CF} Modified");
                    ui.separator();
                }
                if let Some(path) = &self.current_file {
                    ui.label(format!("File: {}", path.display()));
                }
                if let Some(editor) = &self.editor {
                    ui.separator();
                    ui.label(format!("{} bytes", editor.working().len()));
                    ui.separator();
                    ui.label(format!("Cursor: 0x{:08X}", editor.cursor()));
                }
                if let Some(err) = &self.decode_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::YELLOW, err);
                }
            });
        });

        // Structure tree sidebar (when file is loaded)
        if self.editor.is_some() {
            egui::SidePanel::left("structure_panel")
                .resizable(true)
                .default_width(250.0)
                .min_width(150.0)
                .show(ctx, |ui| {
                    // File structure section
                    egui::CollapsingHeader::new("File Structure")
                        .default_open(true)
                        .show(ui, |ui| {
                            structure_tree::show(ui, self);
                        });

                    ui.add_space(10.0);

                    // Save points section
                    egui::CollapsingHeader::new("Save Points")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.savepoints_state);
                            savepoints::show(ui, self, &mut state);
                            self.savepoints_state = state;
                        });
                });
        }

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
