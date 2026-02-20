//! Main application state and egui integration

mod dialogs;
mod input;
mod menu_bar;
mod preview;
mod sections;
mod toolbar;

pub use dialogs::{DialogState, PendingEdit, PendingEditType};
pub use preview::PreviewState;

use crate::editor::buffer::{EditMode, WriteMode};
use crate::editor::{EditorState, GoToOffsetState, SearchState};
use crate::formats::{parse_file, FileSection};
use crate::settings::AppSettings;
use crate::ui::{
    bookmarks, go_to_offset_dialog,
    hex_editor::{self, ContextMenuState},
    image_preview, savepoints, search_dialog,
    settings_dialog::{self, SettingsDialogState},
    shortcuts_dialog::{self, ShortcutsDialogState},
    structure_tree,
};
use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Threshold for detecting window size changes (pixels)
const WINDOW_RESIZE_THRESHOLD: f32 = 1.0;

/// Debounce delay for window resize saves (milliseconds)
const WINDOW_RESIZE_DEBOUNCE_MS: u64 = 500;

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
#[derive(Default)]
pub struct BendApp {
    /// Editor state containing buffers, history, and file metadata
    pub editor: Option<EditorState>,

    /// Path to currently loaded file (for display purposes)
    pub current_file: Option<PathBuf>,

    /// Image preview state (textures, dirty flag, comparison mode)
    pub preview: PreviewState,

    /// Dialog state (close confirmation, high-risk warnings)
    pub dialogs: DialogState,

    /// State for the save points panel
    savepoints_state: savepoints::SavePointsPanelState,

    /// Cached parsed file sections for structure visualization
    /// Re-parsed when file is loaded or structure potentially changed
    pub cached_sections: Option<Vec<FileSection>>,

    /// Search and replace state
    pub search_state: SearchState,

    /// Go to offset dialog state
    pub go_to_offset_state: GoToOffsetState,

    /// State for the bookmarks panel
    bookmarks_state: bookmarks::BookmarksPanelState,

    /// Whether header protection is enabled (blocks edits to high-risk sections)
    pub header_protection: bool,

    /// Context menu state for hex editor
    pub context_menu_state: ContextMenuState,

    /// Keyboard shortcuts help dialog state
    pub shortcuts_dialog_state: ShortcutsDialogState,

    /// Settings/preferences dialog state
    pub settings_dialog_state: SettingsDialogState,

    /// Application settings (persisted to disk)
    pub settings: AppSettings,

    /// Pending file path to open (for deferred actions from menus)
    pub(super) pending_open_path: Option<PathBuf>,

    /// Pending scroll offset for hex editor (Some(offset) = scroll to this byte offset)
    pub pending_hex_scroll: Option<usize>,

    /// Last known window size (for change detection)
    last_window_size: Option<egui::Vec2>,

    /// Timer for debouncing window resize saves
    window_resize_timer: Option<Instant>,
}

impl BendApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, settings: AppSettings) -> Self {
        // Apply settings to initial state
        let header_protection = settings.default_header_protection;
        let suppress_warnings = !settings.show_high_risk_warnings;

        Self {
            header_protection,
            dialogs: DialogState {
                suppress_high_risk_warnings: suppress_warnings,
                ..Default::default()
            },
            settings,
            ..Default::default()
        }
    }

    /// Perform undo on the active editor (if any)
    pub(super) fn do_undo(&mut self) {
        if let Some(editor) = &mut self.editor {
            let _ = editor.undo();
        }
    }

    /// Perform redo on the active editor (if any)
    pub(super) fn do_redo(&mut self) {
        if let Some(editor) = &mut self.editor {
            let _ = editor.redo();
        }
    }

    /// Request the hex editor to scroll to show the given byte offset
    pub fn scroll_hex_to_offset(&mut self, offset: usize) {
        self.pending_hex_scroll = Some(offset);
    }

    /// Navigate the editor cursor and hex view to the current search match
    pub fn navigate_to_search_match(&mut self) {
        if let Some(offset) = self.search_state.current_match_offset() {
            if let Some(editor) = &mut self.editor {
                editor.set_cursor(offset);
            }
            self.scroll_hex_to_offset(offset);
        }
    }

    /// Re-execute the current search against the working buffer and record the generation
    pub fn refresh_search(&mut self) {
        if let Some(editor) = &self.editor {
            let gen = editor.edit_generation();
            crate::editor::search::execute_search(&mut self.search_state, editor.working());
            self.search_state.set_searched_generation(gen);
        }
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.is_modified())
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
                self.current_file = Some(path.clone());
                self.mark_preview_dirty();
                self.preview.decode_error = None;
                // Clear existing textures - they'll be recreated on next frame
                self.preview.texture = None;
                self.preview.original_texture = None;
                // Add to recent files and save settings
                self.settings.add_recent_file(path);
                self.settings.save();
            }
            Err(e) => {
                log::error!("Failed to load file: {}", e);
                self.preview.decode_error = Some(format!("Failed to load file: {}", e));
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

    /// Show all modal dialogs
    fn show_dialogs(&mut self, ctx: &egui::Context) {
        search_dialog::show(ctx, self);
        go_to_offset_dialog::show(ctx, self);
        shortcuts_dialog::show(ctx, &mut self.shortcuts_dialog_state);
        // Settings dialog handles saving internally
        let _ = settings_dialog::show(ctx, &mut self.settings_dialog_state, &mut self.settings);
        self.show_high_risk_warning_dialog(ctx);
    }

    /// Render the status bar
    fn render_status_bar(&self, ctx: &egui::Context) {
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
                    ui.separator();
                    // Edit mode indicator
                    let mode_text = match editor.edit_mode() {
                        EditMode::Hex => "HEX",
                        EditMode::Ascii => "ASCII",
                    };
                    ui.label(format!("Mode: {}", mode_text));
                    ui.separator();
                    // Write mode indicator (Insert/Overwrite)
                    let write_mode_text = match editor.write_mode() {
                        WriteMode::Insert => "INS",
                        WriteMode::Overwrite => "OVR",
                    };
                    ui.label(write_mode_text);
                }
                if let Some(err) = &self.preview.decode_error {
                    ui.separator();
                    ui.colored_label(egui::Color32::YELLOW, err);
                }
            });
        });
    }

    /// Render the sidebar with structure tree, save points, and bookmarks
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        if self.editor.is_none() {
            return;
        }

        egui::SidePanel::left("structure_panel")
            .resizable(true)
            .default_width(255.0)
            .min_width(150.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
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
            });
    }

    /// Render the main content area with hex editor and image preview
    fn render_main_content(&mut self, ctx: &egui::Context) {
        // Hex editor panel (resizable SidePanel, only shown when file is loaded)
        if self.editor.is_some() {
            egui::SidePanel::left("hex_panel")
                .resizable(true)
                .default_width(620.0)
                .min_width(400.0)
                .max_width(ctx.screen_rect().width() - 400.0) // Leave room for preview
                .show(ctx, |ui| {
                    ui.heading("Hex Editor");
                    hex_editor::show(ui, self);
                });
        }

        // Preview panel (CentralPanel takes remaining space)
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.editor.is_some() {
                ui.heading("Preview");
                image_preview::show(ui, self);
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

impl eframe::App for BendApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle close confirmation
        if self.dialogs.pending_close {
            self.settings.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Track window size changes (debounced save)
        let current_size = ctx.screen_rect().size();
        if let Some(last_size) = self.last_window_size {
            if (current_size.x - last_size.x).abs() > WINDOW_RESIZE_THRESHOLD
                || (current_size.y - last_size.y).abs() > WINDOW_RESIZE_THRESHOLD
            {
                self.window_resize_timer = Some(Instant::now());
                self.last_window_size = Some(current_size);
            }
        } else {
            self.last_window_size = Some(current_size);
        }

        // Save window size after debounce period of no resize activity
        if let Some(timer) = self.window_resize_timer {
            if timer.elapsed() > Duration::from_millis(WINDOW_RESIZE_DEBOUNCE_MS) {
                self.settings.window_width = current_size.x;
                self.settings.window_height = current_size.y;
                self.settings.save();
                self.window_resize_timer = None;
            }
        }

        // Handle deferred file opening from recent files menu
        if let Some(path) = self.pending_open_path.take() {
            self.open_file(path);
        }

        // Handle input and process actions
        let input_actions = self.handle_input(ctx);
        self.process_input_actions(input_actions);

        // Update preview if needed
        self.update_preview(ctx);

        // Render UI components
        self.show_close_dialog(ctx);
        self.render_menu_bar(ctx);
        let toolbar_actions = self.render_toolbar(ctx);
        self.process_input_actions(toolbar_actions);
        self.show_dialogs(ctx);
        self.render_status_bar(ctx);
        self.render_sidebar(ctx);
        self.render_main_content(ctx);
    }
}
