//! Main application state and egui integration

use crate::editor::{EditorState, GoToOffsetState, SearchState};
use crate::formats::{parse_file, FileSection, RiskLevel};
use crate::ui::{bookmarks, go_to_offset_dialog, hex_editor, image_preview, savepoints, search_dialog, structure_tree};
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

    /// Whether comparison mode is enabled (side-by-side original and current)
    pub comparison_mode: bool,

    /// Search and replace state
    pub search_state: SearchState,

    /// Go to offset dialog state
    pub go_to_offset_state: GoToOffsetState,

    /// State for the bookmarks panel
    bookmarks_state: bookmarks::BookmarksPanelState,

    /// Whether header protection is enabled (blocks edits to high-risk sections)
    pub header_protection: bool,

    /// Whether high-risk edit warnings are suppressed for this session
    pub suppress_high_risk_warnings: bool,

    /// Pending high-risk edit waiting for user confirmation
    pub pending_high_risk_edit: Option<PendingEdit>,
}

/// A pending edit awaiting user confirmation
#[derive(Clone)]
pub struct PendingEdit {
    /// The nibble value to write (0-15)
    pub nibble_value: u8,
    /// The byte offset being edited
    pub offset: usize,
    /// Risk level of the section being edited
    pub risk_level: RiskLevel,
}

impl Default for BendApp {
    fn default() -> Self {
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
            comparison_mode: false,
            search_state: SearchState::default(),
            go_to_offset_state: GoToOffsetState::default(),
            bookmarks_state: bookmarks::BookmarksPanelState::default(),
            header_protection: false,
            suppress_high_risk_warnings: false,
            pending_high_risk_edit: None,
        }
    }
}

impl BendApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
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
        self.section_at_offset(offset).map(|section| section.risk.background_color())
    }

    /// Check if an offset is in a protected region (header protection enabled + High/Critical risk)
    pub fn is_offset_protected(&self, offset: usize) -> bool {
        if !self.header_protection {
            return false;
        }

        self.section_at_offset(offset)
            .map(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .unwrap_or(false)
    }

    /// Check if an offset is in a high-risk region that should show a warning
    /// Returns the risk level if it's High or Critical, None otherwise
    pub fn get_high_risk_level(&self, offset: usize) -> Option<RiskLevel> {
        self.section_at_offset(offset)
            .filter(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .map(|section| section.risk)
    }

    /// Check if a warning should be shown for editing at this offset
    pub fn should_warn_for_edit(&self, offset: usize) -> bool {
        if self.suppress_high_risk_warnings {
            return false;
        }
        self.get_high_risk_level(offset).is_some()
    }

    /// Show the high-risk edit warning dialog and handle user response
    fn show_high_risk_warning_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_high_risk_edit.clone() else {
            return;
        };

        let mut should_proceed = false;
        let mut should_cancel = false;
        let mut dont_show_again = false;

        egui::Window::new("High-Risk Edit Warning")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Warning icon and message
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("\u{26A0}")
                                .size(32.0)
                                .color(egui::Color32::YELLOW),
                        );
                        ui.vertical(|ui| {
                            let risk_name = match pending.risk_level {
                                RiskLevel::High => "high-risk",
                                RiskLevel::Critical => "critical",
                                _ => "sensitive",
                            };
                            ui.label(format!(
                                "You are about to edit a {} region.",
                                risk_name
                            ));
                            ui.label(format!("Offset: 0x{:08X}", pending.offset));
                        });
                    });

                    ui.add_space(10.0);

                    ui.label("Editing this region may corrupt the file or make it unreadable.");
                    ui.label("The image preview may fail to render after this edit.");

                    ui.add_space(10.0);

                    // Don't show again checkbox
                    ui.checkbox(&mut dont_show_again, "Don't warn me again this session");

                    ui.add_space(10.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Proceed").clicked() {
                            should_proceed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_cancel = true;
                        }
                    });
                });
            });

        // Handle user response
        if should_proceed {
            // Apply the edit
            if let Some(editor) = &mut self.editor {
                let _ = editor.edit_nibble(pending.nibble_value);
                self.mark_preview_dirty();
            }
            if dont_show_again {
                self.suppress_high_risk_warnings = true;
            }
            self.pending_high_risk_edit = None;
        } else if should_cancel {
            self.pending_high_risk_edit = None;
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
        let mut wants_search = false;
        let mut wants_go_to = false;
        let mut wants_undo_kb = false;
        let mut wants_redo_kb = false;
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    self.open_file(path.clone());
                }
            }

            // Global keyboard shortcuts
            let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
            let shift = i.modifiers.shift;
            if ctrl && i.key_pressed(egui::Key::O) {
                wants_open = true;
            }
            if ctrl && i.key_pressed(egui::Key::E) && self.editor.is_some() {
                wants_export = true;
            }
            if ctrl && i.key_pressed(egui::Key::F) && self.editor.is_some() {
                wants_search = true;
            }
            if ctrl && i.key_pressed(egui::Key::G) && self.editor.is_some() {
                wants_go_to = true;
            }
            // Undo: Ctrl+Z / Cmd+Z
            if ctrl && !shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                wants_undo_kb = true;
            }
            // Redo: Ctrl+Shift+Z / Cmd+Shift+Z (or Ctrl+Y on some platforms)
            if ctrl && shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                wants_redo_kb = true;
            }
            if ctrl && i.key_pressed(egui::Key::Y) && self.editor.is_some() {
                wants_redo_kb = true;
            }
        });

        if wants_open {
            self.open_file_dialog();
        }
        if wants_export {
            self.export_file();
        }
        if wants_search {
            self.search_state.open_dialog();
        }
        if wants_go_to {
            self.go_to_offset_state.open_dialog();
        }
        if wants_undo_kb {
            if let Some(editor) = &mut self.editor {
                editor.undo();
                self.mark_preview_dirty();
            }
        }
        if wants_redo_kb {
            if let Some(editor) = &mut self.editor {
                editor.redo();
                self.mark_preview_dirty();
            }
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
                ui.menu_button("Edit", |ui| {
                    let has_file = self.editor.is_some();
                    if ui.add_enabled(has_file, egui::Button::new("Find & Replace...")).clicked() {
                        self.search_state.open_dialog();
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_file, egui::Button::new("Go to Offset...")).clicked() {
                        self.go_to_offset_state.open_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(has_file, egui::Checkbox::new(&mut self.header_protection, "Protect Headers")).changed() {
                        // Checkbox already updates the value
                    }
                    ui.separator();
                    // Re-enable warnings option (only shown when warnings are suppressed)
                    if self.suppress_high_risk_warnings {
                        if ui.button("Re-enable High-Risk Warnings").clicked() {
                            self.suppress_high_risk_warnings = false;
                            ui.close_menu();
                        }
                    } else {
                        ui.add_enabled(false, egui::Button::new("High-Risk Warnings: Enabled"));
                    }
                });
            });
        });

        // Toolbar with common actions
        let mut wants_undo = false;
        let mut wants_redo = false;
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let has_file = self.editor.is_some();
                let can_undo = self.editor.as_ref().map_or(false, |e| e.can_undo());
                let can_redo = self.editor.as_ref().map_or(false, |e| e.can_redo());

                // File operations
                if ui.button("Open").clicked() {
                    self.open_file_dialog();
                }
                if ui.add_enabled(has_file, egui::Button::new("Export")).clicked() {
                    self.export_file();
                }

                ui.separator();

                // Undo/Redo
                if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                    wants_undo = true;
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
                    wants_redo = true;
                }

                ui.separator();

                // Navigation/Search
                if ui.add_enabled(has_file, egui::Button::new("Search")).clicked() {
                    self.search_state.open_dialog();
                }
                if ui.add_enabled(has_file, egui::Button::new("Go to")).clicked() {
                    self.go_to_offset_state.open_dialog();
                }

                ui.separator();

                // View toggles
                if ui.add_enabled(has_file, egui::SelectableLabel::new(self.comparison_mode, "Compare"))
                    .clicked()
                {
                    self.comparison_mode = !self.comparison_mode;
                }
                if ui.add_enabled(has_file, egui::SelectableLabel::new(self.header_protection, "Protect"))
                    .on_hover_text("Protect header regions from editing")
                    .clicked()
                {
                    self.header_protection = !self.header_protection;
                }
            });
        });

        // Handle toolbar undo/redo actions
        if wants_undo {
            if let Some(editor) = &mut self.editor {
                editor.undo();
                self.mark_preview_dirty();
            }
        }
        if wants_redo {
            if let Some(editor) = &mut self.editor {
                editor.redo();
                self.mark_preview_dirty();
            }
        }

        // Show search dialog if open
        search_dialog::show(ctx, self);

        // Show go to offset dialog if open
        go_to_offset_dialog::show(ctx, self);

        // Show high-risk edit warning dialog if there's a pending edit
        self.show_high_risk_warning_dialog(ctx);

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

                    ui.add_space(10.0);

                    // Bookmarks section
                    egui::CollapsingHeader::new("Bookmarks")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.bookmarks_state);
                            bookmarks::show(ui, self, &mut state);
                            self.bookmarks_state = state;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test app with cached sections
    fn create_test_app_with_sections(sections: Vec<FileSection>) -> BendApp {
        BendApp {
            cached_sections: Some(sections),
            ..Default::default()
        }
    }

    #[test]
    fn test_section_at_offset_simple() {
        let sections = vec![
            FileSection::new("Header", 0, 14, RiskLevel::Critical),
            FileSection::new("Data", 14, 100, RiskLevel::Safe),
        ];
        let app = create_test_app_with_sections(sections);

        // Test offset in first section
        let section = app.section_at_offset(5);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Header");

        // Test offset in second section
        let section = app.section_at_offset(50);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Data");

        // Test offset beyond all sections
        let section = app.section_at_offset(200);
        assert!(section.is_none());
    }

    #[test]
    fn test_section_at_offset_nested() {
        let parent = FileSection::new("Header", 0, 54, RiskLevel::Caution)
            .with_child(FileSection::new("Magic", 0, 2, RiskLevel::Critical))
            .with_child(FileSection::new("Size", 2, 6, RiskLevel::High));

        let sections = vec![parent, FileSection::new("Data", 54, 100, RiskLevel::Safe)];
        let app = create_test_app_with_sections(sections);

        // Test offset in nested child (should return most specific match)
        let section = app.section_at_offset(0);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Magic");

        let section = app.section_at_offset(4);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Size");

        // Test offset in parent but not in any child
        let section = app.section_at_offset(10);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Header");
    }

    #[test]
    fn test_section_at_offset_boundary() {
        let sections = vec![
            FileSection::new("First", 0, 10, RiskLevel::Safe),
            FileSection::new("Second", 10, 20, RiskLevel::Caution),
        ];
        let app = create_test_app_with_sections(sections);

        // Test at exact boundary (end is exclusive)
        let section = app.section_at_offset(9);
        assert_eq!(section.unwrap().name, "First");

        let section = app.section_at_offset(10);
        assert_eq!(section.unwrap().name, "Second");
    }

    #[test]
    fn test_section_color_for_offset() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let app = create_test_app_with_sections(sections);

        // Verify colors are returned for each risk level
        let color = app.section_color_for_offset(5);
        assert!(color.is_some());
        // Green-ish for Safe
        let c = color.unwrap();
        assert!(c.g() > c.r()); // Green channel should be highest

        let color = app.section_color_for_offset(25);
        assert!(color.is_some());
        // Orange-ish for High
        let c = color.unwrap();
        assert!(c.r() > c.b()); // Red channel higher than blue

        // No color for offset outside sections
        let color = app.section_color_for_offset(100);
        assert!(color.is_none());
    }

    #[test]
    fn test_section_at_offset_no_sections() {
        let app = BendApp::default();

        // Should return None when no sections cached
        assert!(app.section_at_offset(0).is_none());
        assert!(app.section_color_for_offset(0).is_none());
    }

    #[test]
    fn test_header_protection() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);

        // Header protection disabled - nothing protected
        assert!(!app.header_protection);
        assert!(!app.is_offset_protected(5));  // Safe
        assert!(!app.is_offset_protected(15)); // Caution
        assert!(!app.is_offset_protected(25)); // High
        assert!(!app.is_offset_protected(35)); // Critical

        // Enable header protection
        app.header_protection = true;

        // Safe and Caution still not protected
        assert!(!app.is_offset_protected(5));
        assert!(!app.is_offset_protected(15));

        // High and Critical are now protected
        assert!(app.is_offset_protected(25));
        assert!(app.is_offset_protected(35));

        // Offset outside any section is not protected
        assert!(!app.is_offset_protected(100));
    }

    #[test]
    fn test_high_risk_warnings() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);

        // Warnings not suppressed by default
        assert!(!app.suppress_high_risk_warnings);

        // Safe and Caution should not trigger warnings
        assert!(!app.should_warn_for_edit(5));
        assert!(!app.should_warn_for_edit(15));

        // High and Critical should trigger warnings
        assert!(app.should_warn_for_edit(25));
        assert!(app.should_warn_for_edit(35));

        // get_high_risk_level returns correct levels
        assert!(app.get_high_risk_level(5).is_none());
        assert!(app.get_high_risk_level(15).is_none());
        assert_eq!(app.get_high_risk_level(25), Some(RiskLevel::High));
        assert_eq!(app.get_high_risk_level(35), Some(RiskLevel::Critical));

        // Suppress warnings
        app.suppress_high_risk_warnings = true;

        // No warnings when suppressed
        assert!(!app.should_warn_for_edit(25));
        assert!(!app.should_warn_for_edit(35));
    }
}
