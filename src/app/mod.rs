//! Main application state and egui integration

mod dialogs;
mod input;
mod menu_bar;
mod preview;
mod sections;
mod state;
mod toolbar;

pub use dialogs::{DialogState, PendingEdit, PendingEditType};
pub use preview::PreviewState;
pub use state::{AppConfig, DocumentState, IoState, UiState};

use crate::editor::buffer::{EditMode, WriteMode};
use crate::editor::EditorState;
use crate::formats::parse_file;
use crate::ui::theme::AppColors;
use crate::ui::PointerCursor;
use crate::ui::{
    bookmarks, go_to_offset_dialog, hex_editor, image_preview, savepoints, search_dialog,
    settings_dialog, shortcuts_dialog, structure_tree,
};
use eframe::egui;
use state::FileDialogResult;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Spawn a file dialog on a background thread, returning a receiver for the result.
fn spawn_file_dialog<F>(ctx: &egui::Context, dialog_fn: F) -> mpsc::Receiver<FileDialogResult>
where
    F: FnOnce() -> FileDialogResult + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(dialog_fn());
        ctx.request_repaint();
    });
    rx
}

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
    /// Document state: loaded editor, current file, preview, sections, header protection
    pub doc: DocumentState,

    /// UI state: colors, dialogs, panel state, pending scroll
    pub ui: UiState,

    /// Application configuration (persisted settings)
    pub config: AppConfig,

    /// I/O plumbing (file-dialog receivers, deferred paths, resize debounce)
    pub io: IoState,
}

impl BendApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: crate::settings::AppSettings) -> Self {
        settings.theme.apply(&cc.egui_ctx);
        crate::ui::theme::apply_custom_visuals(&cc.egui_ctx);

        // Apply settings to initial state
        let header_protection = settings.default_header_protection;
        let suppress_warnings = !settings.show_high_risk_warnings;

        Self {
            doc: DocumentState {
                header_protection,
                ..Default::default()
            },
            ui: UiState {
                dialogs: DialogState {
                    suppress_high_risk_warnings: suppress_warnings,
                    ..Default::default()
                },
                ..Default::default()
            },
            config: AppConfig { settings },
            ..Default::default()
        }
    }

    /// Perform undo on the active editor (if any)
    pub(super) fn do_undo(&mut self) {
        if let Some(editor) = &mut self.doc.editor {
            let _ = editor.undo();
        }
    }

    /// Perform redo on the active editor (if any)
    pub(super) fn do_redo(&mut self) {
        if let Some(editor) = &mut self.doc.editor {
            let _ = editor.redo();
        }
    }

    /// Request the hex editor to scroll to show the given byte offset
    pub fn scroll_hex_to_offset(&mut self, offset: usize) {
        self.ui.pending_hex_scroll = Some(offset);
    }

    /// Navigate the editor cursor and hex view to the current search match
    pub fn navigate_to_search_match(&mut self) {
        if let Some(offset) = self.ui.search_state.current_match_offset() {
            if let Some(editor) = &mut self.doc.editor {
                editor.set_cursor(offset);
            }
            self.scroll_hex_to_offset(offset);
        }
    }

    /// Re-execute the current search against the working buffer and record the generation
    pub fn refresh_search(&mut self) {
        if let Some(editor) = &self.doc.editor {
            let gen = editor.edit_generation();
            crate::editor::search::execute_search(&mut self.ui.search_state, editor.working());
            self.ui.search_state.set_searched_generation(gen);
        }
    }

    /// Check if there are unsaved changes
    pub fn has_unsaved_changes(&self) -> bool {
        self.doc.editor.as_ref().is_some_and(|e| e.is_modified())
    }

    /// Export the working buffer to a new file (non-blocking)
    pub fn export_file(&mut self, ctx: &egui::Context) {
        if self.io.is_dialog_pending() || self.doc.editor.is_none() {
            return;
        }

        let editor = self.doc.editor.as_ref().unwrap();
        let buffer = editor.working().to_vec();

        // Pre-compute filename and extension on main thread
        let default_name = self
            .doc
            .current_file
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| format!("{}_glitched", s.to_string_lossy()))
            .unwrap_or_else(|| "export".to_string());

        let extension = self
            .doc
            .current_file
            .as_ref()
            .and_then(|p| p.extension())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "bmp".to_string());

        let rx = spawn_file_dialog(ctx, move || {
            // Use AsyncFileDialog to avoid NSSavePanel::runModal on macOS,
            // which enters a nested event loop that can trigger a winit panic
            // when drag events fire during the modal dialog.
            let result = pollster::block_on(async {
                rfd::AsyncFileDialog::new()
                    .set_file_name(format!("{}.{}", default_name, extension))
                    .add_filter("Images", &["bmp", "jpg", "jpeg", "gif"])
                    .add_filter("All files", &["*"])
                    .save_file()
                    .await
            });
            if let Some(handle) = result {
                let path = handle.path().to_path_buf();
                match std::fs::write(&path, &buffer) {
                    Ok(()) => FileDialogResult::ExportSuccess(path),
                    Err(e) => FileDialogResult::ExportError(e.to_string()),
                }
            } else {
                FileDialogResult::Cancelled
            }
        });

        self.io.export_dialog_rx = Some(rx);
    }

    /// Check if a file extension is a supported format
    fn is_supported_extension(path: &std::path::Path) -> bool {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => matches!(
                ext.to_ascii_lowercase().as_str(),
                "bmp" | "jpg" | "jpeg" | "gif"
            ),
            None => false,
        }
    }

    /// Open a file from a path
    pub fn open_file(&mut self, path: PathBuf) {
        if !Self::is_supported_extension(&path) {
            self.doc.preview.decode_error = Some(
                "Unsupported file format. Bend supports BMP (.bmp), JPEG (.jpg, .jpeg), and GIF (.gif) files."
                    .to_string(),
            );
            return;
        }

        match std::fs::read(&path) {
            Ok(bytes) => {
                log::info!("Loaded file: {} ({} bytes)", path.display(), bytes.len());
                // Parse file structure for section highlighting
                self.doc.cached_sections = parse_file(&bytes);
                self.doc.editor = Some(EditorState::new(bytes));
                self.doc.current_file = Some(path.clone());
                self.doc.preview.mark_dirty();
                self.doc.preview.decode_error = None;
                // Clear existing textures and animation state
                self.doc.preview.reset_for_new_file();
                // Add to recent files and save settings
                self.config.settings.add_recent_file(path);
                self.config.settings.save();
            }
            Err(e) => {
                log::error!("Failed to load file: {}", e);
                self.doc.preview.decode_error = Some(format!("Failed to load file: {}", e));
            }
        }
    }

    /// Open file dialog on a background thread (non-blocking)
    pub fn open_file_dialog(&mut self, ctx: &egui::Context) {
        if self.io.is_dialog_pending() {
            return;
        }

        let rx = spawn_file_dialog(ctx, || {
            // Use AsyncFileDialog to avoid NSSavePanel::runModal on macOS,
            // which enters a nested event loop that can trigger a winit panic
            // when drag events fire during the modal dialog.
            let result = pollster::block_on(async {
                rfd::AsyncFileDialog::new()
                    .add_filter("Images", &["bmp", "jpg", "jpeg", "gif"])
                    .add_filter("All files", &["*"])
                    .pick_file()
                    .await
            });
            if let Some(handle) = result {
                FileDialogResult::OpenFile(handle.path().to_path_buf())
            } else {
                FileDialogResult::Cancelled
            }
        });

        self.io.open_dialog_rx = Some(rx);
    }

    /// Show all modal dialogs
    fn show_dialogs(&mut self, ctx: &egui::Context) {
        search_dialog::show(ctx, self);
        go_to_offset_dialog::show(ctx, &mut self.doc, &mut self.ui);
        shortcuts_dialog::show(ctx, &mut self.ui.shortcuts_dialog_state);
        // Settings dialog handles saving internally; sync runtime flag on change
        if settings_dialog::show(
            ctx,
            &mut self.ui.settings_dialog_state,
            &mut self.config.settings,
        ) {
            self.ui.dialogs.suppress_high_risk_warnings =
                !self.config.settings.show_high_risk_warnings;
        }
        self.show_high_risk_warning_dialog(ctx);
    }

    /// Render the status bar
    fn render_status_bar(&self, ctx: &egui::Context) {
        let colors = self.ui.colors;
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Unsaved changes indicator
                if self.has_unsaved_changes() {
                    ui.colored_label(colors.modified_indicator, "\u{25CF} Modified");
                    ui.separator();
                }
                if let Some(path) = &self.doc.current_file {
                    ui.label(format!("File: {}", path.display()));
                }
                if let Some(editor) = &self.doc.editor {
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
                if let Some(err) = &self.doc.preview.decode_error {
                    ui.separator();
                    ui.colored_label(colors.warning_text, err);
                }
            });
        });
    }

    /// Render the sidebar with structure tree, save points, and bookmarks
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        if self.doc.editor.is_none() {
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
                            structure_tree::show(ui, &mut self.doc, &mut self.ui);
                        })
                        .header_response
                        .pointer_cursor();

                    ui.add_space(10.0);

                    // Save points section
                    egui::CollapsingHeader::new("Save Points")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.ui.savepoints_state);
                            savepoints::show(ui, &mut self.doc, &mut state);
                            self.ui.savepoints_state = state;
                        })
                        .header_response
                        .pointer_cursor();

                    ui.add_space(10.0);

                    // Bookmarks section
                    egui::CollapsingHeader::new("Bookmarks")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.ui.bookmarks_state);
                            bookmarks::show(ui, &mut self.doc, &mut self.ui, &mut state);
                            self.ui.bookmarks_state = state;
                        })
                        .header_response
                        .pointer_cursor();
                });
            });
    }

    /// Render the main content area with hex editor and image preview
    fn render_main_content(&mut self, ctx: &egui::Context) {
        // Hex editor panel (resizable SidePanel, only shown when file is loaded)
        if self.doc.editor.is_some() {
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
            if self.doc.editor.is_some() {
                ui.heading("Preview");
                image_preview::show(ui, &mut self.doc.preview, &self.ui.colors);
            } else {
                // No file loaded - show welcome message
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Welcome to bend-rs");
                        ui.add_space(20.0);
                        ui.label("Open a BMP, JPEG, or GIF file to begin databending.");
                        ui.add_space(10.0);
                        ui.label("Drag and drop a file here, or use File > Open");
                        ui.add_space(20.0);
                        if ui.button("Open File...").pointer_cursor().clicked() {
                            self.open_file_dialog(ui.ctx());
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
        if self.ui.dialogs.pending_close {
            self.config.settings.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Refresh cached color palette for this frame
        self.ui.colors = AppColors::new(ctx.style().visuals.dark_mode);

        // Track window size changes (debounced save)
        let current_size = ctx.screen_rect().size();
        if let Some(last_size) = self.io.last_window_size {
            if (current_size.x - last_size.x).abs() > WINDOW_RESIZE_THRESHOLD
                || (current_size.y - last_size.y).abs() > WINDOW_RESIZE_THRESHOLD
            {
                self.io.window_resize_timer = Some(Instant::now());
                self.io.last_window_size = Some(current_size);
            }
        } else {
            self.io.last_window_size = Some(current_size);
        }

        // Save window size after debounce period of no resize activity
        if let Some(timer) = self.io.window_resize_timer {
            if timer.elapsed() > Duration::from_millis(WINDOW_RESIZE_DEBOUNCE_MS) {
                self.config.settings.window_width = current_size.x;
                self.config.settings.window_height = current_size.y;
                self.config.settings.save();
                self.io.window_resize_timer = None;
            }
        }

        // Handle deferred file opening from recent files menu
        if let Some(path) = self.io.pending_open_path.take() {
            self.open_file(path);
        }

        // Poll background file dialogs
        if let Some(rx) = &self.io.open_dialog_rx {
            if let Ok(result) = rx.try_recv() {
                if let FileDialogResult::OpenFile(path) = result {
                    self.open_file(path);
                }
                self.io.open_dialog_rx = None;
            }
        }
        if let Some(rx) = &self.io.export_dialog_rx {
            if let Ok(result) = rx.try_recv() {
                match result {
                    FileDialogResult::ExportSuccess(path) => {
                        log::info!("Exported to: {}", path.display());
                    }
                    FileDialogResult::ExportError(e) => {
                        log::error!("Failed to export: {}", e);
                    }
                    _ => {}
                }
                self.io.export_dialog_rx = None;
            }
        }

        // Handle input and process actions
        let input_actions = self.handle_input(ctx);
        self.process_input_actions(input_actions, ctx);

        // Advance animation frames (unconditional — runs independently of edits)
        self.advance_animation(ctx);

        // Update preview if needed
        self.update_preview(ctx);

        // Render UI components
        self.show_close_dialog(ctx);
        self.render_menu_bar(ctx);
        let toolbar_actions = self.render_toolbar(ctx);
        self.process_input_actions(toolbar_actions, ctx);
        self.show_dialogs(ctx);
        self.render_status_bar(ctx);
        self.render_sidebar(ctx);
        self.render_main_content(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_extension() {
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.bmp"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.jpg"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.jpeg"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.BMP"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.JPG"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.gif"
        )));
        assert!(BendApp::is_supported_extension(std::path::Path::new(
            "photo.GIF"
        )));
        assert!(!BendApp::is_supported_extension(std::path::Path::new(
            "document.txt"
        )));
        assert!(!BendApp::is_supported_extension(std::path::Path::new(
            "noext"
        )));
    }

    #[test]
    fn test_open_file_unsupported_extension_sets_error() {
        let mut app = BendApp::default();
        app.open_file(PathBuf::from("/tmp/test.tiff"));

        // Should set an error message
        assert!(app.doc.preview.decode_error.is_some());
        assert!(app
            .doc
            .preview
            .decode_error
            .as_ref()
            .unwrap()
            .contains("Unsupported file format"));

        // Should NOT load a file
        assert!(app.doc.editor.is_none());
        assert!(app.doc.current_file.is_none());
    }

    #[test]
    fn test_settings_sync_suppress_warnings() {
        let mut app = BendApp::default();

        // Initially warnings are not suppressed
        assert!(!app.ui.dialogs.suppress_high_risk_warnings);
        assert!(app.config.settings.show_high_risk_warnings);

        // Simulate what show_dialogs does when settings change:
        // Toggle the setting and sync
        app.config.settings.show_high_risk_warnings = false;
        app.ui.dialogs.suppress_high_risk_warnings = !app.config.settings.show_high_risk_warnings;
        assert!(app.ui.dialogs.suppress_high_risk_warnings);

        // Toggle back
        app.config.settings.show_high_risk_warnings = true;
        app.ui.dialogs.suppress_high_risk_warnings = !app.config.settings.show_high_risk_warnings;
        assert!(!app.ui.dialogs.suppress_high_risk_warnings);
    }
}
