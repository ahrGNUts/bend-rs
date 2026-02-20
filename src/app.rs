//! Main application state and egui integration

use crate::editor::buffer::{EditMode, WriteMode};
use crate::editor::{EditorState, GoToOffsetState, SearchState};
use crate::formats::{parse_file, FileSection, RiskLevel};
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

/// Returns the platform-appropriate modifier key text for shortcuts
fn modifier_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘ " // space to give the character that follows more breathing room
    } else {
        "Ctrl+"
    }
}

/// Menu item with shortcut hint that has better contrast than egui's default.
/// Uses a horizontal layout with the shortcut text aligned right.
/// Shortcut text is dimmer when not hovered, brighter when hovered.
fn menu_item_with_shortcut(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    // Calculate label and shortcut widths for proper sizing
    let label_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
    let shortcut_galley = ui.painter().layout_no_wrap(
        shortcut.to_string(),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );

    // Width = label + gap + shortcut + padding
    let desired_width = label_galley.size().x + 40.0 + shortcut_galley.size().x + 8.0;

    let response = ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(egui::vec2(desired_width, 0.0)),
    );

    // Paint shortcut with brightness based on hover state
    if !shortcut.is_empty() {
        let shortcut_color = if response.hovered() {
            egui::Color32::from_gray(200) // Brighter when hovered
        } else {
            egui::Color32::from_gray(120) // Dimmer when not hovered
        };

        let shortcut_galley = ui.painter().layout_no_wrap(
            shortcut.to_string(),
            egui::FontId::default(),
            shortcut_color,
        );

        let pos = egui::pos2(
            response.rect.right() - shortcut_galley.size().x - 8.0,
            response.rect.center().y - shortcut_galley.size().y / 2.0,
        );
        ui.painter().galley(pos, shortcut_galley, shortcut_color);
    }

    response.clicked()
}

/// Debounce delay for preview updates (milliseconds)
const PREVIEW_DEBOUNCE_MS: u64 = 150;

/// Threshold for detecting window size changes (pixels)
const WINDOW_RESIZE_THRESHOLD: f32 = 1.0;

/// Debounce delay for window resize saves (milliseconds)
const WINDOW_RESIZE_DEBOUNCE_MS: u64 = 500;

/// State for close confirmation and high-risk edit warning dialogs
#[derive(Default)]
pub struct DialogState {
    /// Whether the close confirmation dialog is showing
    pub show_close: bool,
    /// Pending close action (true = confirmed close)
    pub pending_close: bool,
    /// Whether high-risk edit warnings are suppressed for this session
    pub suppress_high_risk_warnings: bool,
    /// Pending high-risk edit waiting for user confirmation
    pub pending_high_risk_edit: Option<PendingEdit>,
    /// Checkbox state for "don't warn again" in high-risk dialog
    pub high_risk_dont_show: bool,
}

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
    pending_open_path: Option<PathBuf>,

    /// Pending scroll offset for hex editor (Some(offset) = scroll to this byte offset)
    pub pending_hex_scroll: Option<usize>,

    /// Last known window size (for change detection)
    last_window_size: Option<egui::Vec2>,

    /// Timer for debouncing window resize saves
    window_resize_timer: Option<Instant>,
}

/// Type of pending edit (hex nibble or ASCII character)
#[derive(Clone, Copy)]
pub enum PendingEditType {
    /// Nibble edit (hex mode): nibble value 0-15
    Nibble(u8),
    /// ASCII edit: character to write
    Ascii(char),
    /// Backspace key (insert mode delete-previous)
    Backspace,
    /// Delete key (insert mode delete-at-cursor)
    Delete,
}

/// A pending edit awaiting user confirmation
#[derive(Clone, Copy)]
pub struct PendingEdit {
    /// The type of edit (nibble or ASCII)
    pub edit_type: PendingEditType,
    /// The byte offset being edited
    pub offset: usize,
    /// Risk level of the section being edited
    pub risk_level: RiskLevel,
}

/// Actions triggered by keyboard/mouse input, processed after input handling
#[derive(Default)]
struct InputActions {
    open: bool,
    export: bool,
    search: bool,
    go_to: bool,
    undo: bool,
    redo: bool,
    create_save_point: bool,
    add_bookmark: bool,
    refresh_preview: bool,
}

/// Actions triggered by toolbar buttons
#[derive(Default)]
struct ToolbarActions {
    undo: bool,
    redo: bool,
    refresh_preview: bool,
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

    /// Mark the preview as needing update (with debounce timestamp)
    pub fn mark_preview_dirty(&mut self) {
        self.preview.dirty = true;
        self.preview.last_edit_time = Some(Instant::now());
    }

    /// Perform undo on the active editor (if any)
    fn do_undo(&mut self) {
        if let Some(editor) = &mut self.editor {
            let _ = editor.undo();
        }
    }

    /// Perform redo on the active editor (if any)
    fn do_redo(&mut self) {
        if let Some(editor) = &mut self.editor {
            let _ = editor.redo();
        }
    }

    /// Request the hex editor to scroll to show the given byte offset
    pub fn scroll_hex_to_offset(&mut self, offset: usize) {
        self.pending_hex_scroll = Some(offset);
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
        self.section_at_offset(offset)
            .map(|section| section.risk.background_color())
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

    /// Check if any byte in a range overlaps a protected region
    pub fn is_range_protected(&self, start: usize, len: usize) -> bool {
        if !self.header_protection || len == 0 {
            return false;
        }
        (start..start + len).any(|offset| self.is_offset_protected(offset))
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
        if self.dialogs.suppress_high_risk_warnings {
            return false;
        }
        self.get_high_risk_level(offset).is_some()
    }

    /// Show the high-risk edit warning dialog and handle user response
    fn show_high_risk_warning_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.dialogs.pending_high_risk_edit else {
            return;
        };

        let mut should_proceed = false;
        let mut should_cancel = false;

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
                            ui.label(format!("You are about to edit a {} region.", risk_name));
                            ui.label(format!("Offset: 0x{:08X}", pending.offset));
                        });
                    });

                    ui.add_space(10.0);

                    ui.label("Editing this region may corrupt the file or make it unreadable.");
                    ui.label("The image preview may fail to render after this edit.");

                    ui.add_space(10.0);

                    // Don't show again checkbox
                    ui.checkbox(
                        &mut self.dialogs.high_risk_dont_show,
                        "Don't warn me again this session",
                    );

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
            // Apply the edit based on type
            if let Some(editor) = &mut self.editor {
                match pending.edit_type {
                    PendingEditType::Nibble(nibble_value) => {
                        let _ = editor.edit_nibble_with_mode(nibble_value);
                    }
                    PendingEditType::Ascii(ch) => {
                        let _ = editor.edit_ascii_with_mode(ch);
                    }
                    PendingEditType::Backspace => {
                        editor.handle_backspace();
                    }
                    PendingEditType::Delete => {
                        editor.handle_delete();
                    }
                }
            }
            if self.dialogs.high_risk_dont_show {
                self.dialogs.suppress_high_risk_warnings = true;
            }
            self.dialogs.high_risk_dont_show = false;
            self.dialogs.pending_high_risk_edit = None;
        } else if should_cancel {
            self.dialogs.high_risk_dont_show = false;
            self.dialogs.pending_high_risk_edit = None;
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

    /// Handle dropped files and keyboard shortcuts
    /// Returns flags for deferred actions
    fn handle_input(&mut self, ctx: &egui::Context) -> InputActions {
        let mut actions = InputActions::default();

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
                actions.open = true;
            }
            if ctrl && i.key_pressed(egui::Key::E) && self.editor.is_some() {
                actions.export = true;
            }
            if ctrl && i.key_pressed(egui::Key::F) && self.editor.is_some() {
                actions.search = true;
            }
            if ctrl && i.key_pressed(egui::Key::G) && self.editor.is_some() {
                actions.go_to = true;
            }
            // Undo: Ctrl+Z / Cmd+Z
            if ctrl && !shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                actions.undo = true;
            }
            // Redo: Ctrl+Shift+Z / Cmd+Shift+Z (or Ctrl+Y on some platforms)
            if ctrl && shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                actions.redo = true;
            }
            if ctrl && i.key_pressed(egui::Key::Y) && self.editor.is_some() {
                actions.redo = true;
            }
            // Create save point: Ctrl+S / Cmd+S
            if ctrl && i.key_pressed(egui::Key::S) && self.editor.is_some() {
                actions.create_save_point = true;
            }
            // Add bookmark: Ctrl+D / Cmd+D
            if ctrl && i.key_pressed(egui::Key::D) && self.editor.is_some() {
                actions.add_bookmark = true;
            }
            // Refresh preview: Ctrl+R / Cmd+R
            if ctrl && i.key_pressed(egui::Key::R) && self.editor.is_some() {
                actions.refresh_preview = true;
            }
            // F1: Show keyboard shortcuts help
            if i.key_pressed(egui::Key::F1) {
                self.shortcuts_dialog_state.open_dialog();
            }
        });

        actions
    }

    /// Process input actions (deferred to avoid borrow conflicts)
    fn process_input_actions(&mut self, actions: InputActions) {
        if actions.open {
            self.open_file_dialog();
        }
        if actions.export {
            self.export_file();
        }
        if actions.search {
            self.search_state.open_dialog();
        }
        if actions.go_to {
            self.go_to_offset_state.open_dialog();
        }
        if actions.undo {
            self.do_undo();
        }
        if actions.redo {
            self.do_redo();
        }
        if actions.create_save_point {
            if let Some(editor) = &mut self.editor {
                let count = editor.save_points().len();
                let name = format!("Save Point {}", count + 1);
                editor.create_save_point(name);
            }
        }
        if actions.add_bookmark {
            if let Some(editor) = &mut self.editor {
                let cursor_pos = editor.cursor();
                let name = format!("Bookmark at 0x{:08X}", cursor_pos);
                editor.add_bookmark(cursor_pos, name);
            }
        }
        if actions.refresh_preview {
            self.mark_preview_dirty();
        }
    }

    /// Show the close confirmation dialog
    fn show_close_dialog(&mut self, ctx: &egui::Context) {
        if !self.dialogs.show_close {
            return;
        }

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
                        self.dialogs.show_close = false;
                    }
                    if ui.button("Discard & Exit").clicked() {
                        self.dialogs.pending_close = true;
                        self.dialogs.show_close = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.dialogs.show_close = false;
                    }
                });
            });
    }

    /// Render the top menu bar
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| self.render_file_menu(ui, ctx));
                ui.menu_button("Edit", |ui| self.render_edit_menu(ui));
                ui.menu_button("Help", |ui| self.render_help_menu(ui));
            });
        });
    }

    /// Render the File menu contents
    fn render_file_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mod_str = modifier_key();
        let open_shortcut = format!("{}O", mod_str);
        let export_shortcut = format!("{}E", mod_str);

        if menu_item_with_shortcut(ui, "Open...", &open_shortcut, true) {
            self.open_file_dialog();
            ui.close_menu();
        }
        let has_file = self.editor.is_some();
        if menu_item_with_shortcut(ui, "Export...", &export_shortcut, has_file) {
            self.export_file();
            ui.close_menu();
        }
        ui.separator();

        // Recent files submenu
        let recent_files = self.settings.recent_files().to_vec();
        let has_recent = !recent_files.is_empty();
        ui.menu_button("Recent Files", |ui| {
            if has_recent {
                for path in &recent_files {
                    let display_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.to_string_lossy().into_owned());

                    if ui
                        .button(&display_name)
                        .on_hover_text(path.to_string_lossy())
                        .clicked()
                    {
                        self.pending_open_path = Some(path.clone());
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Clear Recent Files").clicked() {
                    self.settings.clear_recent_files();
                    self.settings.save();
                    ui.close_menu();
                }
            } else {
                ui.label("No recent files");
            }
        });

        ui.separator();
        if ui.button("Exit").clicked() {
            if self.has_unsaved_changes() {
                self.dialogs.show_close = true;
            } else {
                self.settings.save();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ui.close_menu();
        }
    }

    /// Render the Edit menu contents
    fn render_edit_menu(&mut self, ui: &mut egui::Ui) {
        let mod_str = modifier_key();
        let has_file = self.editor.is_some();
        let can_undo = self.editor.as_ref().is_some_and(|e| e.can_undo());
        let can_redo = self.editor.as_ref().is_some_and(|e| e.can_redo());
        let undo_shortcut = format!("{}Z", mod_str);
        let redo_shortcut = format!("{}Shift+Z", mod_str);
        let find_shortcut = format!("{}F", mod_str);
        let goto_shortcut = format!("{}G", mod_str);
        let refresh_shortcut = format!("{}R", mod_str);

        if menu_item_with_shortcut(ui, "Undo", &undo_shortcut, can_undo) {
            self.do_undo();
            ui.close_menu();
        }
        if menu_item_with_shortcut(ui, "Redo", &redo_shortcut, can_redo) {
            self.do_redo();
            ui.close_menu();
        }
        ui.separator();

        if menu_item_with_shortcut(ui, "Find & Replace...", &find_shortcut, has_file) {
            self.search_state.open_dialog();
            ui.close_menu();
        }
        if menu_item_with_shortcut(ui, "Go to Offset...", &goto_shortcut, has_file) {
            self.go_to_offset_state.open_dialog();
            ui.close_menu();
        }
        ui.separator();
        if menu_item_with_shortcut(ui, "Refresh Preview", &refresh_shortcut, has_file) {
            self.mark_preview_dirty();
            ui.close_menu();
        }
        ui.separator();
        if ui
            .add_enabled(
                has_file,
                egui::Checkbox::new(&mut self.header_protection, "Protect Headers"),
            )
            .changed()
        {
            // Checkbox already updates the value
        }
        ui.separator();
        // Re-enable warnings option (only shown when warnings are suppressed)
        if self.dialogs.suppress_high_risk_warnings {
            if ui.button("Re-enable High-Risk Warnings").clicked() {
                self.dialogs.suppress_high_risk_warnings = false;
                ui.close_menu();
            }
        } else {
            ui.add_enabled(false, egui::Button::new("High-Risk Warnings: Enabled"));
        }
        ui.separator();
        if ui.button("Preferences...").clicked() {
            self.settings_dialog_state.open(&self.settings);
            ui.close_menu();
        }
    }

    /// Render the Help menu contents
    fn render_help_menu(&mut self, ui: &mut egui::Ui) {
        if menu_item_with_shortcut(ui, "Keyboard Shortcuts", "F1", true) {
            self.shortcuts_dialog_state.open_dialog();
            ui.close_menu();
        }
    }

    /// Render the toolbar and return deferred action flags
    fn render_toolbar(&mut self, ctx: &egui::Context) -> ToolbarActions {
        let mut actions = ToolbarActions::default();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let has_file = self.editor.is_some();
                let can_undo = self.editor.as_ref().is_some_and(|e| e.can_undo());
                let can_redo = self.editor.as_ref().is_some_and(|e| e.can_redo());

                // File operations
                if ui.button("Open").clicked() {
                    self.open_file_dialog();
                }
                if ui
                    .add_enabled(has_file, egui::Button::new("Export"))
                    .clicked()
                {
                    self.export_file();
                }

                ui.separator();

                // Undo/Redo
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .clicked()
                {
                    actions.undo = true;
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .clicked()
                {
                    actions.redo = true;
                }

                ui.separator();

                // Navigation/Search
                if ui
                    .add_enabled(has_file, egui::Button::new("Search"))
                    .clicked()
                {
                    self.search_state.open_dialog();
                }
                if ui
                    .add_enabled(has_file, egui::Button::new("Go to"))
                    .clicked()
                {
                    self.go_to_offset_state.open_dialog();
                }

                ui.separator();

                // View toggles
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(self.preview.comparison_mode, "Compare"),
                    )
                    .clicked()
                {
                    self.preview.comparison_mode = !self.preview.comparison_mode;
                }
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(self.header_protection, "Protect"),
                    )
                    .on_hover_text("Protect header regions from editing")
                    .clicked()
                {
                    self.header_protection = !self.header_protection;
                }

                ui.separator();

                // Refresh preview
                if ui
                    .add_enabled(has_file, egui::Button::new("Refresh"))
                    .on_hover_text("Refresh preview (Ctrl+R / Cmd+R)")
                    .clicked()
                {
                    actions.refresh_preview = true;
                }
            });
        });

        actions
    }

    /// Process toolbar undo/redo/refresh actions
    fn process_toolbar_actions(&mut self, actions: ToolbarActions) {
        if actions.undo {
            self.do_undo();
        }
        if actions.redo {
            self.do_redo();
        }
        if actions.refresh_preview {
            self.mark_preview_dirty();
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
        match image::load_from_memory(editor.working()) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

                self.preview.texture =
                    Some(ctx.load_texture("preview", color_image, egui::TextureOptions::LINEAR));
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
            if let Ok(img) = image::load_from_memory(editor.original()) {
                let rgba = img.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

                self.preview.original_texture =
                    Some(ctx.load_texture("original", color_image, egui::TextureOptions::LINEAR));
            }
        }

        self.preview.dirty = false;
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
        self.process_toolbar_actions(toolbar_actions);
        self.show_dialogs(ctx);
        self.render_status_bar(ctx);
        self.render_sidebar(ctx);
        self.render_main_content(ctx);
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
        assert!(!app.is_offset_protected(5)); // Safe
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
        assert!(!app.dialogs.suppress_high_risk_warnings);

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
        app.dialogs.suppress_high_risk_warnings = true;

        // No warnings when suppressed
        assert!(!app.should_warn_for_edit(25));
        assert!(!app.should_warn_for_edit(35));
    }

    #[test]
    fn test_is_range_protected() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("High", 10, 20, RiskLevel::High),
            FileSection::new("Critical", 20, 30, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);
        app.header_protection = true;

        // Entirely in safe region — not protected
        assert!(!app.is_range_protected(0, 10));

        // Entirely in protected region
        assert!(app.is_range_protected(10, 5));
        assert!(app.is_range_protected(20, 5));

        // Spanning safe-to-protected boundary
        assert!(app.is_range_protected(8, 4)); // bytes 8..12, crosses into High at 10

        // Zero length — never protected
        assert!(!app.is_range_protected(15, 0));

        // Protection disabled — nothing protected
        app.header_protection = false;
        assert!(!app.is_range_protected(10, 5));
        assert!(!app.is_range_protected(20, 5));
    }
}
