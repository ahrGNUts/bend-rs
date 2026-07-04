//! Main application state and egui integration

mod dialogs;
mod input;
mod menu_bar;
mod preview;
mod sections;
mod state;
mod toolbar;

pub use dialogs::{ConfirmAction, DialogState, PendingEdit, PendingEditType};
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

    /// Open the Search & Replace dialog. If a query persists from a prior
    /// session, re-run the search immediately so the dialog shows real
    /// state (match count, current position) instead of the stale "No
    /// matches found" that would otherwise appear because `clear_results`
    /// only cleared `matches` — the `last_searched_*` fields persist, so
    /// `query_changed_since_search()` returns false and the status line
    /// can't distinguish "search produced zero hits" from "search hasn't
    /// run yet against this query".
    pub fn open_search_dialog(&mut self) {
        self.ui.search_state.open_dialog();
        if !self.ui.search_state.query.is_empty() && self.ui.search_state.matches.is_empty() {
            self.rehydrate_search();
        }
    }

    /// Re-execute the current search against the working buffer and record
    /// the generation. After the search runs, advances `current_match` past
    /// any leading protected matches so the user lands on the first
    /// replaceable hit (matters when Protect Headers is on and the very
    /// first matches sit inside the header).
    pub fn refresh_search(&mut self) {
        if let Some(editor) = &self.doc.editor {
            let gen = editor.edit_generation();
            crate::editor::search::execute_search(&mut self.ui.search_state, editor.working());
            self.ui.search_state.set_searched_generation(gen);
        }
        self.enforce_search_selection_visible();
    }

    /// Enforce the "selected match is always visible" invariant against the
    /// CURRENT protection settings: advance off a protected selection, and
    /// deselect when every match is protected (ensure_current_visible cannot
    /// move in that case). Called after every search, and after the Protect
    /// toggle flips — protection can change without any buffer edit, so no
    /// generation-based staleness would catch it.
    pub(crate) fn enforce_search_selection_visible(&mut self) {
        let pattern_len = self.ui.search_state.pattern_length();
        self.ui
            .search_state
            .ensure_current_visible(|off| self.doc.is_range_protected(off, pattern_len));
        if let Some(off) = self.ui.search_state.current_match_offset() {
            if self.doc.is_range_protected(off, pattern_len) {
                self.ui.search_state.current_match = None;
            }
        }
    }

    /// Refresh and, when the result is "matches exist but none are
    /// visitable" (all protected), surface why — used by the rehydrate
    /// paths (dialog reopen, file switch) that would otherwise show a bare
    /// deselected counter.
    fn rehydrate_search(&mut self) {
        self.refresh_search();
        if !self.ui.search_state.matches.is_empty() && self.ui.search_state.current_match.is_none()
        {
            self.set_all_protected_message();
        }
    }

    /// Re-run the search only when the cached matches are stale relative to
    /// the editor's current generation. Returns `true` if a refresh happened.
    pub fn refresh_search_if_stale(&mut self) -> bool {
        let stale = self
            .doc
            .editor
            .as_ref()
            .map(|e| {
                self.ui
                    .search_state
                    .matches_may_be_stale(e.edit_generation())
            })
            .unwrap_or(false);
        if stale {
            self.refresh_search();
        }
        stale
    }

    /// Whether the current query needs an initial search before navigation —
    /// the query/mode/case has drifted since the last executed search.
    /// Deliberately NOT keyed on `matches.is_empty()`: a query that
    /// legitimately found zero hits has already been searched, and re-running
    /// the full-buffer scan on every keypress would be wasted work (buffer
    /// edits that could create new hits are covered by the generation-based
    /// staleness check instead).
    pub(crate) fn search_needs_initial_run(&self) -> bool {
        let s = &self.ui.search_state;
        !s.query.is_empty() && s.query_changed_since_search()
    }

    /// Set the "all matches protected" info message and deselect the
    /// current match. Deselecting keeps every trigger consistent (Next and
    /// Previous previously diverged: Some(0) vs None, which rendered a
    /// nonsense "Match 0 of N" one way and enabled Replace on a protected
    /// match the other way).
    fn set_all_protected_message(&mut self) {
        self.ui.search_state.current_match = None;
        let n = self.ui.search_state.matches.len();
        self.ui.search_state.message = Some(crate::editor::search::SearchMessage::Info(format!(
            "All {} match{} in protected regions — turn off Protect to visit them",
            n,
            if n == 1 { " is" } else { "es are" }
        )));
    }

    /// Advance to the next search match (running an initial search if the
    /// query hasn't been executed yet, or refreshing if buffer edits have
    /// invalidated cached matches) and scroll the hex view to follow it.
    /// Protected matches are skipped when header protection is on (when off,
    /// `is_range_protected` returns false for everything so this degrades to
    /// a plain wrap-around step); when EVERY match is protected an info
    /// message says so instead of navigation silently doing nothing.
    pub fn do_search_next(&mut self) {
        // Transient replace/skip banners yield to live navigation state.
        self.ui.search_state.message = None;

        // A cleared Find field means "no search": drop the previous query's
        // cached results instead of stepping through them.
        if self.ui.search_state.query.is_empty() {
            self.ui.search_state.clear_results();
            return;
        }

        if self.search_needs_initial_run() {
            self.refresh_search();
            if !self.ui.search_state.matches.is_empty() {
                let pattern_len = self.ui.search_state.pattern_length();
                if self
                    .ui
                    .search_state
                    .any_visible_match(|off| self.doc.is_range_protected(off, pattern_len))
                {
                    // refresh_search already advanced current_match past any
                    // leading protected matches; land there without stepping.
                    self.navigate_to_search_match();
                } else {
                    // Don't move the cursor into a match the message says
                    // can't be visited (mirrors do_search_prev).
                    self.set_all_protected_message();
                }
            }
            return;
        }

        // Buffer edits since the last search invalidate the cached offsets.
        // Preserve the user's position across the refresh: "next" after a
        // refresh means "first visible match after where I was", not "second
        // match from the top" (execute_search resets current_match to 0).
        let prev_offset = self.ui.search_state.current_match_offset();
        if self.refresh_search_if_stale() {
            if self.ui.search_state.matches.is_empty() {
                return;
            }
            let pattern_len = self.ui.search_state.pattern_length();
            let moved = match prev_offset {
                Some(off) => self.ui.search_state.select_visible_after_offset(off, |o| {
                    self.doc.is_range_protected(o, pattern_len)
                }),
                // No prior position: keep the refresh landing if visible.
                None => self
                    .ui
                    .search_state
                    .any_visible_match(|o| self.doc.is_range_protected(o, pattern_len)),
            };
            if moved {
                self.navigate_to_search_match();
            } else {
                self.set_all_protected_message();
            }
            return;
        }

        if self.ui.search_state.matches.is_empty() {
            return;
        }
        let pattern_len = self.ui.search_state.pattern_length();
        if self
            .ui
            .search_state
            .step_to_next_visible(|off| self.doc.is_range_protected(off, pattern_len))
        {
            self.navigate_to_search_match();
        } else if !self.ui.search_state.matches.is_empty() {
            self.set_all_protected_message();
        }
    }

    /// Advance to the previous search match. On an initial run, jumps to the
    /// last visible match (mirroring Shift+Enter behavior).
    pub fn do_search_prev(&mut self) {
        self.ui.search_state.message = None;

        if self.ui.search_state.query.is_empty() {
            self.ui.search_state.clear_results();
            return;
        }

        if self.search_needs_initial_run() {
            self.refresh_search();
            if !self.ui.search_state.matches.is_empty() {
                let pattern_len = self.ui.search_state.pattern_length();
                // Clear current_match so step_to_prev_visible starts its
                // backward scan from the end and lands on the last visible
                // match (skipping protected matches at the tail).
                self.ui.search_state.current_match = None;
                if self
                    .ui
                    .search_state
                    .step_to_prev_visible(|off| self.doc.is_range_protected(off, pattern_len))
                {
                    self.navigate_to_search_match();
                } else {
                    self.set_all_protected_message();
                }
            }
            return;
        }

        let prev_offset = self.ui.search_state.current_match_offset();
        if self.refresh_search_if_stale() {
            if self.ui.search_state.matches.is_empty() {
                return;
            }
            let pattern_len = self.ui.search_state.pattern_length();
            // "Previous" after a refresh = last visible match strictly before
            // where the user was. partition_point gives the first index with
            // offset >= prev_offset; scanning backward from just before it
            // lands there (mapping == len to None so step_to_prev_visible
            // starts from the end — correct wrap for "before everything" and
            // "after everything" alike).
            if let Some(off) = prev_offset {
                let pos = self.ui.search_state.matches.partition_point(|&m| m < off);
                self.ui.search_state.current_match =
                    (pos < self.ui.search_state.matches.len()).then_some(pos);
            }
            if self
                .ui
                .search_state
                .step_to_prev_visible(|o| self.doc.is_range_protected(o, pattern_len))
            {
                self.navigate_to_search_match();
            } else {
                self.set_all_protected_message();
            }
            return;
        }

        if self.ui.search_state.matches.is_empty() {
            return;
        }
        let pattern_len = self.ui.search_state.pattern_length();
        if self
            .ui
            .search_state
            .step_to_prev_visible(|off| self.doc.is_range_protected(off, pattern_len))
        {
            self.navigate_to_search_match();
        } else if !self.ui.search_state.matches.is_empty() {
            self.set_all_protected_message();
        }
    }

    /// Whether another dialog/menu/text-entry surface is stacked above the
    /// search dialog this frame. The search dialog renders FIRST in
    /// `show_dialogs`, so these flags still hold their values from when the
    /// stacked UI was opened — search defers its keyboard shortcuts (Esc,
    /// Alt+R/A, Ctrl+Enter) to whatever is on top instead of stealing the
    /// press. Sidebar rename/annotation editors are included because they
    /// render AFTER the dialogs: if search consumed Esc first, their cancel
    /// handlers would never see the event and the rename would stay stuck
    /// in edit mode.
    pub(crate) fn modal_above_search_open(&self) -> bool {
        self.ui.overlay_wants_escape()
            || self.ui.bookmarks_state.renaming.is_some()
            || self.ui.bookmarks_state.editing_annotation.is_some()
            || self.ui.savepoints_state.wants_keyboard_priority()
    }

    /// Called after a new file replaces the editor buffer. Search offsets
    /// from the previous file are meaningless against the new buffer (and
    /// the fresh editor's generation restarts at 0, which can collide with
    /// `searched_at_generation` and defeat stale detection), so drop them —
    /// and re-anchor immediately when the dialog is open with a live query.
    /// Keep this egui-free: `open_file` runs inside `ctx.input()` for
    /// dropped files.
    pub(crate) fn on_file_loaded(&mut self) {
        self.ui.search_state.clear_results();
        if self.ui.search_state.dialog_open && !self.ui.search_state.query.is_empty() {
            self.rehydrate_search();
        }
        // Abandon sidebar edit state from the previous file: the new
        // editor's save points/bookmarks are fresh (and ids restart), so a
        // latched rename would defer search shortcuts forever — or worse,
        // reattach to an unrelated item that later receives the same id.
        self.ui.savepoints_state.cancel_edits();
        self.ui.bookmarks_state.renaming = None;
        self.ui.bookmarks_state.editing_annotation = None;
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
                // Invalidate (and possibly re-anchor) search state — offsets
                // from the previous file don't transfer
                self.on_file_loaded();
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
        self.show_confirmation_dialog(ctx);
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
                    let savepoints_resp = egui::CollapsingHeader::new("Save Points")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.ui.savepoints_state);
                            savepoints::show(ui, &mut self.doc, &mut self.ui, &mut state);
                            self.ui.savepoints_state = state;
                        });
                    savepoints_resp.header_response.pointer_cursor();
                    // Collapsing the section abandons any in-progress edit.
                    // The panel's Esc/Enter/Cancel handlers only run while
                    // its body renders, so a latched edit flag would
                    // otherwise permanently defer the search dialog's
                    // keyboard shortcuts (modal_above_search_open).
                    if savepoints_resp.body_returned.is_none() {
                        self.ui.savepoints_state.cancel_edits();
                    }

                    ui.add_space(10.0);

                    // Bookmarks section
                    let bookmarks_resp = egui::CollapsingHeader::new("Bookmarks")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut state = std::mem::take(&mut self.ui.bookmarks_state);
                            bookmarks::show(ui, &mut self.doc, &mut self.ui, &mut state);
                            self.ui.bookmarks_state = state;
                        });
                    bookmarks_resp.header_response.pointer_cursor();
                    if bookmarks_resp.body_returned.is_none() {
                        self.ui.bookmarks_state.renaming = None;
                        self.ui.bookmarks_state.editing_annotation = None;
                    }
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
    fn test_do_search_next_runs_initial_search_when_matches_empty() {
        // Reproducer for the bug where Next/Previous (and F3) were no-ops on
        // a never-searched query because they only stepped a cached match
        // list — typing "FF" then pressing F3 did nothing until Enter was
        // pressed first.
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        assert!(app.ui.search_state.matches.is_empty());

        app.do_search_next();

        assert_eq!(app.ui.search_state.matches, vec![3, 10, 17]);
        // Initial search lands on the first match — Next does not advance
        // past it on the same press that triggered the search.
        assert_eq!(app.ui.search_state.current_match, Some(0));

        // Subsequent presses step forward as usual.
        app.do_search_next();
        assert_eq!(app.ui.search_state.current_match, Some(1));
    }

    #[test]
    fn test_reopen_search_dialog_reruns_search_for_persisted_query() {
        // Reproducer for the "stale No matches found on reopen" bug. After
        // close + reopen, the dialog showed "No matches found" because
        // matches were cleared but the last_searched_query persisted, so
        // query_changed_since_search() returned false. open_search_dialog
        // now re-runs the search whenever a query persists.
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();

        // Session 1: search, then close.
        app.open_search_dialog();
        assert_eq!(app.ui.search_state.matches, vec![3, 10]);
        app.ui.search_state.close_dialog();
        assert!(app.ui.search_state.matches.is_empty());
        // clear_results now also resets the last-searched fingerprint, so
        // the persisted query registers as needing a fresh search — both
        // this reopen path and F3-after-clearing rely on that.
        assert!(app.ui.search_state.query_changed_since_search());

        // Session 2: reopen. Without the auto-rerun fix this would leave
        // matches empty, causing render_status to show "No matches found".
        app.open_search_dialog();
        assert_eq!(app.ui.search_state.matches, vec![3, 10]);
        assert!(app.ui.search_state.current_match.is_some());
    }

    #[test]
    fn test_reopen_search_dialog_with_empty_query_does_not_search() {
        // Sanity check: opening with no persisted query is still a no-op
        // beyond toggling dialog_open and just_opened.
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(vec![0u8; 20]));
        assert!(app.ui.search_state.query.is_empty());

        app.open_search_dialog();
        assert!(app.ui.search_state.dialog_open);
        assert!(app.ui.search_state.matches.is_empty());
    }

    #[test]
    fn test_do_search_prev_initial_search_jumps_to_last_match() {
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();

        app.do_search_prev();

        assert_eq!(app.ui.search_state.matches, vec![3, 10, 17]);
        // Initial Previous lands on the LAST match (mirrors Shift+Enter).
        assert_eq!(app.ui.search_state.current_match, Some(2));
    }

    /// Helper: app with FF at offsets 3, 10, 17; header (High risk) covering
    /// 0..N so protection tests can choose which matches are protected.
    fn app_with_protected_header(header_end: usize) -> BendApp {
        use crate::formats::traits::{FileSection, RiskLevel};
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.doc.cached_sections = Some(vec![
            FileSection::new("Header", 0, header_end, RiskLevel::High),
            FileSection::new("Data", header_end, 20, RiskLevel::Safe),
        ]);
        app.doc.header_protection = true;
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app
    }

    #[test]
    fn test_do_search_next_stale_refresh_preserves_position() {
        // On match at offset 10 (index 1); an unrelated edit staleness-bumps
        // the generation. F3 must land on offset 17 — "next after where I
        // was" — not reset to the top and step to index 1 again.
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        app.ui.search_state.current_match = Some(1); // offset 10

        app.doc.editor.as_mut().unwrap().edit_byte(0, 0x01); // unrelated edit

        app.do_search_next();
        assert_eq!(app.ui.search_state.current_match_offset(), Some(17));
    }

    #[test]
    fn test_do_search_prev_stale_refresh_preserves_position() {
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        app.ui.search_state.current_match = Some(1); // offset 10

        app.doc.editor.as_mut().unwrap().edit_byte(0, 0x01);

        app.do_search_prev();
        // Previous-before-offset-10 is offset 3.
        assert_eq!(app.ui.search_state.current_match_offset(), Some(3));
    }

    #[test]
    fn test_do_search_next_all_protected_sets_message_and_stays() {
        // Header covers the whole file → every match protected. Navigation
        // must say so instead of silently doing nothing.
        let mut app = app_with_protected_header(20);

        // Initial run: message set, nothing selected (a selection would
        // render "Match 1 of N" and enable Replace on a protected match).
        app.do_search_next();
        assert_eq!(app.ui.search_state.matches, vec![3, 10, 17]);
        assert_eq!(app.ui.search_state.current_match, None);
        match app.ui.search_state.message.as_ref() {
            Some(crate::editor::search::SearchMessage::Info(msg)) => {
                assert!(msg.contains("protected regions"), "got: {msg}")
            }
            other => panic!("expected all-protected Info, got {:?}", other),
        }

        // Step path: message re-set on every press, still nothing selected.
        app.do_search_next();
        assert_eq!(app.ui.search_state.current_match, None);
        assert!(app.ui.search_state.message.is_some());

        // Prev path too — symmetric with Next (both directions deselect).
        app.do_search_prev();
        assert_eq!(app.ui.search_state.current_match, None);
        assert!(app.ui.search_state.message.is_some());
    }

    #[test]
    fn test_reopen_with_all_protected_matches_stays_deselected() {
        // Regression (validation round 3): the dialog-reopen rehydrate ran
        // refresh_search, whose execute_search selects match 0; with every
        // match protected, ensure_current_visible couldn't move — leaving
        // Replace enabled on a protected match with no explanation. The
        // refresh invariant now deselects, and rehydrate surfaces the
        // all-protected message.
        let mut app = app_with_protected_header(20);
        app.ui.search_state.dialog_open = true;
        app.do_search_next();
        assert_eq!(app.ui.search_state.current_match, None);

        app.ui.search_state.close_dialog();
        app.open_search_dialog();

        assert_eq!(app.ui.search_state.matches, vec![3, 10, 17]);
        assert_eq!(app.ui.search_state.current_match, None);
        match app.ui.search_state.message.as_ref() {
            Some(crate::editor::search::SearchMessage::Info(msg)) => {
                assert!(msg.contains("protected regions"), "got: {msg}")
            }
            other => panic!("expected all-protected Info, got {:?}", other),
        }
    }

    #[test]
    fn test_on_file_loaded_abandons_sidebar_edit_state() {
        // A rename latched across a file switch would defer search
        // shortcuts forever (the new file's panel may never render a row
        // with the old id) — or reattach to an unrelated item when ids
        // restart. on_file_loaded must abandon panel edit state.
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(vec![0u8; 8]));
        app.ui.bookmarks_state.renaming = Some(2);
        app.ui.bookmarks_state.editing_annotation = Some(3);

        app.on_file_loaded();

        assert!(app.ui.bookmarks_state.renaming.is_none());
        assert!(app.ui.bookmarks_state.editing_annotation.is_none());
        assert!(!app.ui.savepoints_state.wants_keyboard_priority());
        assert!(!app.modal_above_search_open());
    }

    #[test]
    fn test_retyping_identical_query_after_clear_researches() {
        // Regression: clear_results used to preserve the last-searched
        // fingerprint, so clearing the Find field (which drops results)
        // and retyping the SAME query left every navigation press a no-op
        // with a false "No matches found".
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.do_search_next();
        assert_eq!(app.ui.search_state.matches, vec![3, 10]);

        // Clear the field, press Enter/F3 (empty-query branch clears).
        app.ui.search_state.query.clear();
        app.do_search_next();
        assert!(app.ui.search_state.matches.is_empty());

        // Retype the identical query — must search again, not dead-end.
        app.ui.search_state.query = "FF".to_string();
        app.do_search_next();
        assert_eq!(app.ui.search_state.matches, vec![3, 10]);
        assert_eq!(app.ui.search_state.current_match, Some(0));
    }

    #[test]
    fn test_do_search_prev_initial_skips_protected_tail() {
        // Header protects offsets 0..4 only (match at 3); matches at 10 and
        // 17 are safe. Sanity-check forward too. Then invert: protect the
        // TAIL via a custom section layout and confirm initial Previous
        // skips it.
        use crate::formats::traits::{FileSection, RiskLevel};
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.doc.cached_sections = Some(vec![
            FileSection::new("Data", 0, 15, RiskLevel::Safe),
            FileSection::new("Trailer", 15, 20, RiskLevel::High),
        ]);
        app.doc.header_protection = true;
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();

        app.do_search_prev();

        // Last VISIBLE match is offset 10 (17 is protected by the trailer).
        assert_eq!(app.ui.search_state.current_match_offset(), Some(10));
    }

    #[test]
    fn test_navigation_clears_transient_message() {
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();

        app.ui.search_state.message = Some(crate::editor::search::SearchMessage::Info(
            "Replaced at 0x00000003".to_string(),
        ));

        app.do_search_next();
        // The stale banner yields to the live counter.
        assert!(app.ui.search_state.message.is_none());
    }

    #[test]
    fn test_on_file_loaded_invalidates_stale_search() {
        // Search ran against a 20-byte file (match at 17, generation 0).
        // A new 10-byte file replaces the editor; its generation restarts at
        // 0, defeating generation-based staleness — on_file_loaded must
        // drop the old offsets explicitly.
        let mut data = vec![0u8; 20];
        data[17] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        assert_eq!(app.ui.search_state.matches, vec![17]);

        // Dialog closed: results dropped, nothing re-anchored.
        app.doc.editor = Some(EditorState::new(vec![0u8; 10]));
        app.on_file_loaded();
        assert!(app.ui.search_state.matches.is_empty());
        assert!(app.ui.search_state.current_match.is_none());

        // Dialog open with a live query: re-anchored against the NEW buffer.
        let mut small = vec![0u8; 10];
        small[4] = 0xFF;
        app.ui.search_state.dialog_open = true;
        app.doc.editor = Some(EditorState::new(small));
        app.on_file_loaded();
        assert_eq!(app.ui.search_state.matches, vec![4]);
        assert_eq!(app.ui.search_state.current_match_offset(), Some(4));
    }

    #[test]
    fn test_modal_above_search_open_flag_matrix() {
        let mut app = BendApp::default();
        assert!(!app.modal_above_search_open());

        app.ui.go_to_offset_state.dialog_open = true;
        assert!(app.modal_above_search_open());
        app.ui.go_to_offset_state.dialog_open = false;

        app.ui.shortcuts_dialog_state.dialog_open = true;
        assert!(app.modal_above_search_open());
        app.ui.shortcuts_dialog_state.dialog_open = false;

        app.ui.settings_dialog_state.dialog_open = true;
        assert!(app.modal_above_search_open());
        app.ui.settings_dialog_state.dialog_open = false;

        app.ui.dialogs.show_close = true;
        assert!(app.modal_above_search_open());
        app.ui.dialogs.show_close = false;

        app.ui
            .dialogs
            .open_confirm(crate::app::ConfirmAction::DeleteSavePoint(1));
        assert!(app.modal_above_search_open());
        app.ui.dialogs.pending_confirm = None;

        app.ui.context_menu_state.target_offset = Some(4);
        assert!(app.modal_above_search_open());
        app.ui.context_menu_state.target_offset = None;

        // Menu-bar dropdowns: either frame's flag defers (egui closes the
        // menu during the Esc frame itself).
        app.ui.menu_open_this_frame = true;
        assert!(app.modal_above_search_open());
        app.ui.menu_open_this_frame = false;
        app.ui.menu_open_prev_frame = true;
        assert!(app.modal_above_search_open());
        app.ui.menu_open_prev_frame = false;

        // Sidebar rename/annotation editors render after the dialogs; Esc
        // must reach their cancel handlers.
        app.ui.bookmarks_state.renaming = Some(1);
        assert!(app.modal_above_search_open());
        app.ui.bookmarks_state.renaming = None;
        app.ui.bookmarks_state.editing_annotation = Some(1);
        assert!(app.modal_above_search_open());
        app.ui.bookmarks_state.editing_annotation = None;

        assert!(!app.modal_above_search_open());
    }

    #[test]
    fn test_navigation_with_empty_query_clears_stale_results() {
        // Clearing the Find field then pressing Enter/F3 must not step
        // through the previous query's cached matches.
        let mut data = vec![0u8; 20];
        data[3] = 0xFF;
        data[10] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        assert_eq!(app.ui.search_state.matches.len(), 2);

        app.ui.search_state.query.clear();
        app.do_search_next();
        assert!(app.ui.search_state.matches.is_empty());
        assert!(app.ui.search_state.current_match.is_none());
    }

    #[test]
    fn test_zero_hit_query_does_not_rescan_every_press() {
        // A query that legitimately found nothing has been searched; F3
        // must not re-run the full-buffer scan on every press.
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(vec![0u8; 20]));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        assert!(app.ui.search_state.matches.is_empty());
        // Searched and settled — no initial run pending.
        assert!(!app.search_needs_initial_run());
    }

    #[test]
    fn test_zero_hit_query_refreshes_after_buffer_edit() {
        // ...but an edit that CREATES a hit must be picked up on the next
        // navigation press (generation-based staleness, not query drift).
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(vec![0u8; 20]));
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        assert!(app.ui.search_state.matches.is_empty());

        app.doc.editor.as_mut().unwrap().edit_byte(7, 0xFF);
        app.do_search_next();
        assert_eq!(app.ui.search_state.matches, vec![7]);
        assert_eq!(app.ui.search_state.current_match_offset(), Some(7));
    }

    #[test]
    fn test_all_protected_initial_next_does_not_move_cursor() {
        // The message says the matches can't be visited — the cursor must
        // not move into one anyway (parity with do_search_prev).
        let mut app = app_with_protected_header(20);
        let cursor_before = app.doc.editor.as_ref().unwrap().cursor();

        app.do_search_next();

        assert!(app.ui.search_state.message.is_some());
        assert_eq!(app.doc.editor.as_ref().unwrap().cursor(), cursor_before);
    }

    #[test]
    fn test_replace_after_save_point_restore_is_not_stale_blind() {
        // Reproduces the validation finding: restore_save_point changes the
        // buffer; the staleness machinery must notice so Replace doesn't
        // write at offsets that no longer match.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(data));
        // Save point captures the state WITH the FF at offset 5.
        let id = app
            .doc
            .editor
            .as_mut()
            .unwrap()
            .create_save_point("sp".into());
        // Destroy the match, then search (finds nothing at 5... so instead:
        // search first, then restore to a DIFFERENT buffer state).
        app.ui.search_state.mode = crate::editor::search::SearchMode::Hex;
        app.ui.search_state.query = "FF".to_string();
        app.refresh_search();
        assert_eq!(app.ui.search_state.matches, vec![5]);

        // Change offset 5, then restore the save point (which puts FF back).
        // Either way the generation must move so cached offsets re-verify.
        app.doc.editor.as_mut().unwrap().edit_byte(5, 0x42);
        assert!(app.doc.editor.as_mut().unwrap().restore_save_point(id));

        let gen = app.doc.editor.as_ref().unwrap().edit_generation();
        assert!(
            app.ui.search_state.matches_may_be_stale(gen),
            "restore must mark cached search results stale"
        );
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
