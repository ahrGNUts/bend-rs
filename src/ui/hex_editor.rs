//! Hex editor UI component with virtual scrolling

use crate::app::{BendApp, PendingEditType};
use crate::editor::buffer::{EditMode, NibblePosition, WriteMode};
use crate::formats::RiskLevel;
use eframe::egui::{self, RichText, TextStyle};
use std::sync::OnceLock;

/// Pre-computed lookup table of 256 hex strings ("00" through "FF")
fn hex_table() -> &'static [&'static str; 256] {
    static TABLE: OnceLock<[&'static str; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        // Leak a single allocation containing all 256 two-char hex strings
        let strings: Vec<&'static str> = (0..256u16)
            .map(|i| {
                let s = format!("{:02X}", i);
                &*Box::leak(s.into_boxed_str())
            })
            .collect();
        let mut arr = [""; 256];
        for (i, s) in strings.into_iter().enumerate() {
            arr[i] = s;
        }
        arr
    })
}

/// State for the context menu
#[derive(Default)]
pub struct ContextMenuState {
    /// The byte offset where the context menu was triggered
    pub target_offset: Option<usize>,
}

/// Number of bytes per row
const BYTES_PER_ROW: usize = 16;

/// Number of rows to render above/below viewport for smooth scrolling
const BUFFER_ROWS: usize = 2;

/// Spacing between offset column and hex bytes
const OFFSET_HEX_SPACING: f32 = 8.0;

/// Spacing between hex byte groups (after every 8 bytes)
const HEX_GROUP_SPACING: f32 = 8.0;

/// Spacing between hex bytes and ASCII column
const HEX_ASCII_SPACING: f32 = 16.0;

/// Approximate width of a hex byte display ("XX ")
const HEX_BYTE_WIDTH: f32 = 24.0;

/// Number of rows to scroll above target when jumping to an offset
const SCROLL_BUFFER_ROWS: usize = 5;

/// Cursor highlight colors — bright (active nibble / active mode)
const CURSOR_BRIGHT_OVERWRITE: egui::Color32 = egui::Color32::from_rgb(80, 80, 160);
/// Cursor highlight colors — dim (inactive nibble / inactive mode)
const CURSOR_DIM_OVERWRITE: egui::Color32 = egui::Color32::from_rgb(40, 40, 80);
/// Cursor highlight colors — bright in insert mode (green tint)
const CURSOR_BRIGHT_INSERT: egui::Color32 = egui::Color32::from_rgb(80, 160, 80);
/// Cursor highlight colors — dim in insert mode (green tint)
const CURSOR_DIM_INSERT: egui::Color32 = egui::Color32::from_rgb(40, 80, 40);

/// Pre-computed highlight state for a single byte
struct ByteHighlight {
    is_cursor: bool,
    is_selected: bool,
    is_search_match: bool,
    is_current_match: bool,
    has_bookmark: bool,
    is_protected: bool,
    section_bg: Option<egui::Color32>,
}

/// Render a single hex byte with cursor/selection/section highlighting.
/// Returns the response for click detection.
fn render_hex_byte(
    ui: &mut egui::Ui,
    byte: u8,
    highlight: &ByteHighlight,
    cursor_nibble: NibblePosition,
    edit_mode: EditMode,
    write_mode: WriteMode,
) -> egui::Response {
    let hex = hex_table()[byte as usize];

    if highlight.is_cursor {
        let rich_text = RichText::new(hex).monospace();
        let response = ui.label(rich_text);

        // Draw nibble highlight backgrounds using painter
        let rect = response.rect;
        let half_width = rect.width() / 2.0;

        let (bright, dim) = if write_mode == WriteMode::Insert {
            (CURSOR_BRIGHT_INSERT, CURSOR_DIM_INSERT)
        } else {
            (CURSOR_BRIGHT_OVERWRITE, CURSOR_DIM_OVERWRITE)
        };
        let (high_bg, low_bg) = if edit_mode == EditMode::Ascii {
            (dim, dim)
        } else {
            match cursor_nibble {
                NibblePosition::High => (bright, dim),
                NibblePosition::Low => (dim, bright),
            }
        };

        let high_rect = egui::Rect::from_min_size(rect.min, egui::vec2(half_width, rect.height()));
        let low_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + half_width, rect.min.y),
            egui::vec2(half_width, rect.height()),
        );

        ui.painter().rect_filled(high_rect, 0.0, high_bg);
        ui.painter().rect_filled(low_rect, 0.0, low_bg);

        // Re-paint text on top so it's visible over the backgrounds
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            hex,
            egui::TextStyle::Monospace.resolve(ui.style()),
            egui::Color32::WHITE,
        );

        response
    } else {
        let mut rich_text = RichText::new(hex).monospace();

        // Apply background color priority: selection > current_match > search_match > bookmark > section
        if highlight.is_selected {
            rich_text = rich_text.background_color(egui::Color32::from_rgb(40, 80, 40));
        } else if highlight.is_current_match {
            rich_text = rich_text.background_color(egui::Color32::from_rgb(200, 120, 40));
        } else if highlight.is_search_match {
            rich_text = rich_text.background_color(egui::Color32::from_rgb(180, 180, 60));
        } else if highlight.has_bookmark {
            rich_text = rich_text.background_color(egui::Color32::from_rgb(60, 160, 180));
        } else if let Some(bg) = highlight.section_bg {
            rich_text = rich_text.background_color(bg);
        }

        if highlight.is_protected {
            rich_text = rich_text.strikethrough();
        }

        ui.label(rich_text)
    }
}

/// Render a single ASCII character with cursor/selection highlighting.
/// Returns the response for click detection.
fn render_ascii_char(
    ui: &mut egui::Ui,
    byte: u8,
    is_cursor: bool,
    is_selected: bool,
    edit_mode: EditMode,
    write_mode: WriteMode,
) -> egui::Response {
    let display_char = if byte.is_ascii_graphic() || byte == b' ' {
        byte as char
    } else {
        '.'
    };

    let mut rich_text = RichText::new(display_char.to_string()).monospace();

    if is_cursor {
        let (bright, dim) = if write_mode == WriteMode::Insert {
            (CURSOR_BRIGHT_INSERT, CURSOR_DIM_INSERT)
        } else {
            (CURSOR_BRIGHT_OVERWRITE, CURSOR_DIM_OVERWRITE)
        };
        if edit_mode == EditMode::Ascii {
            rich_text = rich_text.background_color(bright);
        } else {
            rich_text = rich_text.background_color(dim);
        }
    } else if is_selected {
        rich_text = rich_text.background_color(egui::Color32::from_rgb(40, 80, 40));
    }

    ui.label(rich_text)
}

/// Handle navigation keys (arrows, page up/down, home/end) with optional selection extension.
fn handle_navigation_keys(editor: &mut crate::editor::EditorState, i: &egui::InputState) {
    let shift = i.modifiers.shift;

    if i.key_pressed(egui::Key::ArrowLeft) {
        if shift {
            editor.move_cursor_with_selection(-1);
        } else {
            editor.clear_selection();
            editor.move_cursor(-1);
        }
    }
    if i.key_pressed(egui::Key::ArrowRight) {
        if shift {
            editor.move_cursor_with_selection(1);
        } else {
            editor.clear_selection();
            editor.move_cursor(1);
        }
    }
    if i.key_pressed(egui::Key::ArrowUp) {
        if shift {
            editor.move_cursor_with_selection(-(BYTES_PER_ROW as isize));
        } else {
            editor.clear_selection();
            editor.move_cursor(-(BYTES_PER_ROW as isize));
        }
    }
    if i.key_pressed(egui::Key::ArrowDown) {
        if shift {
            editor.move_cursor_with_selection(BYTES_PER_ROW as isize);
        } else {
            editor.clear_selection();
            editor.move_cursor(BYTES_PER_ROW as isize);
        }
    }
    if i.key_pressed(egui::Key::PageUp) {
        if shift {
            editor.move_cursor_with_selection(-(BYTES_PER_ROW as isize * 16));
        } else {
            editor.clear_selection();
            editor.move_cursor(-(BYTES_PER_ROW as isize * 16));
        }
    }
    if i.key_pressed(egui::Key::PageDown) {
        if shift {
            editor.move_cursor_with_selection(BYTES_PER_ROW as isize * 16);
        } else {
            editor.clear_selection();
            editor.move_cursor(BYTES_PER_ROW as isize * 16);
        }
    }
    if i.key_pressed(egui::Key::Home) {
        if shift {
            editor.extend_selection_to(0);
        } else {
            editor.clear_selection();
            editor.set_cursor(0);
        }
    }
    if i.key_pressed(egui::Key::End) {
        let last = editor.len().saturating_sub(1);
        if shift {
            editor.extend_selection_to(last);
        } else {
            editor.clear_selection();
            editor.set_cursor(last);
        }
    }
}

/// Handle edit input: text entry, backspace, delete, and paste.
/// Returns (pending_high_risk_edit, paste_text).
fn handle_edit_input(
    editor: &mut crate::editor::EditorState,
    i: &mut egui::InputState,
    cursor_pos: usize,
    cursor_protected: bool,
    should_warn_for_cursor: bool,
    cursor_risk_level: Option<RiskLevel>,
    current_edit_mode: EditMode,
) -> (Option<(PendingEditType, usize, RiskLevel)>, Option<String>) {
    let mut pending_high_risk_edit: Option<(PendingEditType, usize, RiskLevel)> = None;
    let mut paste_text: Option<String> = None;

    if cursor_protected {
        return (pending_high_risk_edit, paste_text);
    }

    // Backspace key
    if i.key_pressed(egui::Key::Backspace) {
        if should_warn_for_cursor && editor.write_mode() == WriteMode::Insert {
            if let Some(risk) = cursor_risk_level {
                pending_high_risk_edit = Some((PendingEditType::Backspace, cursor_pos, risk));
            }
        } else {
            editor.handle_backspace();
        }
    }
    // Delete key
    if i.key_pressed(egui::Key::Delete) {
        if should_warn_for_cursor && editor.write_mode() == WriteMode::Insert {
            if let Some(risk) = cursor_risk_level {
                pending_high_risk_edit = Some((PendingEditType::Delete, cursor_pos, risk));
            }
        } else {
            editor.handle_delete();
        }
    }

    for event in &i.events {
        match event {
            egui::Event::Text(text) => {
                for c in text.chars() {
                    match current_edit_mode {
                        EditMode::Hex => {
                            if let Some(nibble) = c.to_digit(16) {
                                if should_warn_for_cursor {
                                    if let Some(risk) = cursor_risk_level {
                                        pending_high_risk_edit = Some((
                                            PendingEditType::Nibble(nibble as u8),
                                            cursor_pos,
                                            risk,
                                        ));
                                    }
                                } else {
                                    let _ = editor.edit_nibble_with_mode(nibble as u8);
                                    // #[must_use] result intentionally ignored — cursor advance handled internally
                                }
                            }
                        }
                        EditMode::Ascii => {
                            let code = c as u32;
                            if (0x20..=0x7E).contains(&code) {
                                if should_warn_for_cursor {
                                    if let Some(risk) = cursor_risk_level {
                                        pending_high_risk_edit =
                                            Some((PendingEditType::Ascii(c), cursor_pos, risk));
                                    }
                                } else {
                                    let _ = editor.edit_ascii_with_mode(c); // #[must_use] result intentionally ignored — acceptance checked by range guard above
                                }
                            }
                        }
                    }
                }
            }
            egui::Event::Paste(text) => {
                paste_text = Some(text.clone());
            }
            _ => {}
        }
    }

    (pending_high_risk_edit, paste_text)
}

/// Show the hex editor panel
pub fn show(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(editor) = &app.editor else {
        return;
    };

    let total_bytes = editor.len();
    let total_rows = total_bytes.div_ceil(BYTES_PER_ROW);

    // Cache cursor and selection state for rendering
    let cursor_pos = editor.cursor();
    let cursor_nibble = editor.nibble();
    let selection = editor.selection();
    let edit_mode = editor.edit_mode();
    let write_mode = editor.write_mode();

    // Get monospace font metrics
    let row_height = ui.text_style_height(&TextStyle::Monospace);

    // Track clicked byte offset for deferred cursor update (from hex column)
    let mut clicked_offset: Option<usize> = None;
    // Track clicked byte offset from ASCII column (switches to ASCII mode)
    let mut clicked_ascii_offset: Option<usize> = None;
    // Track whether shift was held during click
    let shift_held = ui.input(|i| i.modifiers.shift);
    // Track right-clicked byte for context menu
    let mut context_menu_offset: Option<usize> = None;

    // Check if navigation was requested (must be done before closures capture app)
    // We use vertical_scroll_offset to jump to the approximate area, then scroll_to_me() to fine-tune
    let scroll_to_row: Option<usize> = app
        .pending_hex_scroll
        .take()
        .map(|byte_offset| byte_offset / BYTES_PER_ROW);

    // Calculate approximate scroll offset to ensure target row is rendered
    let initial_scroll_offset: Option<f32> = scroll_to_row.map(|target_row| {
        // Jump to a position where the target row will be rendered
        // We aim for slightly before the target so it's in the rendered range
        (target_row.saturating_sub(SCROLL_BUFFER_ROWS) as f32 * row_height).max(0.0)
    });

    // Pre-compute section colors for the entire file
    // This is cached in app.cached_sections so the lookup is fast
    let get_section_color =
        |offset: usize| -> Option<egui::Color32> { app.section_color_for_offset(offset) };

    // Check if an offset is within a search match
    let current_match_offset = app.search_state.current_match_offset();
    let pattern_len = app.search_state.pattern_length();
    let is_search_match = |offset: usize| -> bool { app.search_state.is_within_match(offset) };
    let is_current_match = |offset: usize| -> bool {
        current_match_offset.is_some_and(|m| offset >= m && offset < m + pattern_len)
    };

    // Check if an offset has a bookmark
    let has_bookmark = |offset: usize| -> bool {
        app.editor
            .as_ref()
            .is_some_and(|e| e.has_bookmark_at(offset))
    };

    // Check if an offset is protected (header protection enabled)
    let is_protected = |offset: usize| -> bool { app.is_offset_protected(offset) };

    // Check if cursor is at a protected position
    let cursor_protected = app.is_offset_protected(cursor_pos);

    let mut scroll_area = egui::ScrollArea::both().auto_shrink([false; 2]);

    // Set initial scroll offset to ensure target row is in the rendered range
    if let Some(offset_y) = initial_scroll_offset {
        scroll_area = scroll_area.vertical_scroll_offset(offset_y);
    }

    scroll_area.show_viewport(ui, |ui, viewport| {
        // Calculate which rows are visible
        let first_visible_row = (viewport.min.y / row_height).floor() as usize;
        let last_visible_row = ((viewport.max.y / row_height).ceil() as usize).min(total_rows);

        // Add buffer rows
        let render_start = first_visible_row.saturating_sub(BUFFER_ROWS);
        let render_end = (last_visible_row + BUFFER_ROWS).min(total_rows);

        // Reserve space for rows before visible area
        if render_start > 0 {
            ui.allocate_space(egui::vec2(
                ui.available_width(),
                render_start as f32 * row_height,
            ));
        }

        // Get the editor reference again (immutable borrow for reading bytes)
        let editor = app.editor.as_ref().unwrap();

        // Render visible rows
        for row_idx in render_start..render_end {
            let offset = row_idx * BYTES_PER_ROW;
            let row_end = (offset + BYTES_PER_ROW).min(total_bytes);
            let row_bytes = editor.bytes_in_range(offset, row_end);

            // Check if this is the row we need to scroll to
            let should_scroll_to_this_row = scroll_to_row == Some(row_idx);

            let row_response = ui.horizontal(|ui| {
                // Offset column (8 hex digits)
                ui.label(
                    RichText::new(format!("{:08X}", offset))
                        .monospace()
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(OFFSET_HEX_SPACING);

                // Hex bytes
                for (i, byte) in row_bytes.iter().enumerate() {
                    if i == 8 {
                        ui.add_space(HEX_GROUP_SPACING);
                    }

                    let byte_offset = offset + i;
                    let highlight = ByteHighlight {
                        is_cursor: byte_offset == cursor_pos,
                        is_selected: selection
                            .map(|(start, end)| byte_offset >= start && byte_offset < end)
                            .unwrap_or(false),
                        is_search_match: is_search_match(byte_offset),
                        is_current_match: is_current_match(byte_offset),
                        has_bookmark: has_bookmark(byte_offset),
                        is_protected: is_protected(byte_offset),
                        section_bg: get_section_color(byte_offset),
                    };

                    let response = render_hex_byte(
                        ui,
                        *byte,
                        &highlight,
                        cursor_nibble,
                        edit_mode,
                        write_mode,
                    );
                    if response.clicked() {
                        clicked_offset = Some(byte_offset);
                    }
                    if response.secondary_clicked() {
                        context_menu_offset = Some(byte_offset);
                    }
                }

                // Pad remaining space if row is incomplete
                let missing = BYTES_PER_ROW - row_bytes.len();
                if missing > 0 {
                    ui.add_space(missing as f32 * HEX_BYTE_WIDTH);
                }

                ui.add_space(HEX_ASCII_SPACING);

                // ASCII column - per-character rendering for click handling
                // Set zero spacing between characters so they render tightly together
                ui.spacing_mut().item_spacing.x = 0.0;

                ui.label(
                    RichText::new("|")
                        .monospace()
                        .color(egui::Color32::DARK_GRAY),
                );

                for (i, byte) in row_bytes.iter().enumerate() {
                    let byte_offset = offset + i;
                    let is_cursor = byte_offset == cursor_pos;
                    let is_selected = selection
                        .map(|(start, end)| byte_offset >= start && byte_offset < end)
                        .unwrap_or(false);

                    let response =
                        render_ascii_char(ui, *byte, is_cursor, is_selected, edit_mode, write_mode);
                    if response.clicked() {
                        clicked_ascii_offset = Some(byte_offset);
                    }
                    if response.secondary_clicked() {
                        context_menu_offset = Some(byte_offset);
                    }
                }

                ui.label(
                    RichText::new("|")
                        .monospace()
                        .color(egui::Color32::DARK_GRAY),
                );
            });

            // Scroll to this row if it was the navigation target
            if should_scroll_to_this_row {
                row_response
                    .response
                    .scroll_to_me(Some(egui::Align::Center));
            }
        }

        // Reserve space for rows after visible area
        let rows_after = total_rows.saturating_sub(render_end);
        if rows_after > 0 {
            ui.allocate_space(egui::vec2(
                ui.available_width(),
                rows_after as f32 * row_height,
            ));
        }
    });

    // Handle deferred click from hex column (stays in hex mode)
    if let Some(offset) = clicked_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_edit_mode(EditMode::Hex);
            editor.set_cursor_with_selection(offset, shift_held);
        }
    }

    // Handle deferred click from ASCII column (switches to ASCII mode)
    if let Some(offset) = clicked_ascii_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_edit_mode(EditMode::Ascii);
            editor.set_cursor_with_selection(offset, shift_held);
        }
    }

    // Handle keyboard input
    let keyboard_result = handle_keyboard_input(ui, app, cursor_pos, cursor_protected);

    // Handle pending high-risk edit (after input borrow ends)
    if let Some((edit_type, offset, risk_level)) = keyboard_result.pending_high_risk_edit {
        app.dialogs.pending_high_risk_edit = Some(crate::app::PendingEdit {
            edit_type,
            offset,
            risk_level,
        });
    }

    // Handle context menu right-click
    if let Some(offset) = context_menu_offset {
        app.context_menu_state.target_offset = Some(offset);
    }

    // Show context menu if active
    show_context_menu(ui, app);
}

/// Result of keyboard input handling
struct KeyboardResult {
    /// Pending high-risk edit awaiting confirmation (edit_type, offset, risk_level)
    pending_high_risk_edit: Option<(PendingEditType, usize, RiskLevel)>,
}

/// Handle keyboard input for navigation and editing
fn handle_keyboard_input(
    ui: &mut egui::Ui,
    app: &mut BendApp,
    cursor_pos: usize,
    cursor_protected: bool,
) -> KeyboardResult {
    // Don't process hex editor input when a text-input dialog is open
    if app.search_state.dialog_open || app.go_to_offset_state.dialog_open {
        return KeyboardResult {
            pending_high_risk_edit: None,
        };
    }

    // Pre-compute warning state before mutable borrow of editor
    let should_warn_for_cursor = app.should_warn_for_edit(cursor_pos);
    let cursor_risk_level = app.get_high_risk_level(cursor_pos);

    // Cache edit mode for text input handling
    let current_edit_mode = app
        .editor
        .as_ref()
        .map(|e| e.edit_mode())
        .unwrap_or(EditMode::Hex);

    let (pending_high_risk_edit, paste_text) = ui.input_mut(|i| {
        let Some(editor) = &mut app.editor else {
            return (None, None);
        };

        let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
        let shift = i.modifiers.shift;

        // Tab key toggles between Hex and ASCII edit mode
        if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
            editor.toggle_edit_mode();
        }

        // Ctrl+I / Cmd+I toggles Insert/Overwrite mode
        if ctrl && i.key_pressed(egui::Key::I) {
            editor.toggle_write_mode();
        }

        // Navigation keys (arrows, page up/down, home/end)
        handle_navigation_keys(editor, i);

        // Undo/Redo
        if ctrl && !shift && i.key_pressed(egui::Key::Z) {
            let _ = editor.undo(); // #[must_use] result intentionally ignored — UI doesn't need to distinguish no-op
        }
        if ctrl && shift && i.key_pressed(egui::Key::Z) {
            let _ = editor.redo(); // #[must_use] result intentionally ignored — UI doesn't need to distinguish no-op
        }

        // Edit input (text entry, backspace, delete, paste)
        handle_edit_input(
            editor,
            i,
            cursor_pos,
            cursor_protected,
            should_warn_for_cursor,
            cursor_risk_level,
            current_edit_mode,
        )
    });

    // Handle paste outside the input closure
    if let Some(text) = paste_text {
        if let Some(bytes) = parse_paste_input(&text, current_edit_mode) {
            if let Some(editor) = &mut app.editor {
                apply_paste_bytes(editor, cursor_pos, &bytes);
            }
        }
    }

    // Check if buffer length changed and invalidate caches
    if let Some(editor) = &mut app.editor {
        if editor.take_length_changed() {
            // Re-parse file structure since offsets shifted
            app.cached_sections = crate::formats::parse_file(editor.working());
            app.mark_preview_dirty();
        }
    }

    KeyboardResult {
        pending_high_risk_edit,
    }
}

/// Show the context menu for the hex editor
fn show_context_menu(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(target_offset) = app.context_menu_state.target_offset else {
        return;
    };

    let mut close_menu = false;
    let mut do_copy_hex = false;
    let mut do_copy_ascii = false;
    let mut do_paste = false;
    let mut do_add_bookmark = false;
    let mut do_go_to_offset = false;

    // Determine if we have a selection or just cursor
    let (start, end) = app
        .editor
        .as_ref()
        .and_then(|e| e.selection())
        .unwrap_or((target_offset, target_offset + 1));

    let byte_count = end - start;
    let label_suffix = if byte_count > 1 {
        format!(" ({} bytes)", byte_count)
    } else {
        String::new()
    };

    // Show context menu as a window at mouse position
    let ctx = ui.ctx().clone();
    let mouse_pos = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();

    egui::Area::new(egui::Id::new("hex_context_menu"))
        .fixed_pos(mouse_pos)
        .order(egui::Order::Foreground)
        .show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(150.0);

                if ui.button(format!("Copy as Hex{}", label_suffix)).clicked() {
                    do_copy_hex = true;
                    close_menu = true;
                }
                if ui
                    .button(format!("Copy as ASCII{}", label_suffix))
                    .clicked()
                {
                    do_copy_ascii = true;
                    close_menu = true;
                }

                ui.separator();

                if ui.button("Paste").clicked() {
                    do_paste = true;
                    close_menu = true;
                }

                ui.separator();

                if ui.button("Add Bookmark").clicked() {
                    do_add_bookmark = true;
                    close_menu = true;
                }
                if ui.button("Go to Offset...").clicked() {
                    do_go_to_offset = true;
                    close_menu = true;
                }
            });
        });

    // Close menu on click outside or Escape
    let clicked_outside = ctx.input(|i| i.pointer.any_click() && !i.pointer.secondary_down());
    let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    if clicked_outside || escape_pressed {
        close_menu = true;
    }

    // Handle actions
    if do_copy_hex {
        copy_as_hex(ui, app, target_offset);
    }
    if do_copy_ascii {
        copy_as_ascii(ui, app, target_offset);
    }
    if do_paste {
        paste_hex(ui, app, target_offset);
    }
    if do_add_bookmark {
        if let Some(editor) = &mut app.editor {
            editor.add_bookmark(target_offset, format!("Offset 0x{:X}", target_offset));
        }
    }
    if do_go_to_offset {
        app.go_to_offset_state.open_dialog();
    }

    if close_menu {
        app.context_menu_state.target_offset = None;
    }
}

/// Copy selected bytes as hex string to clipboard
fn copy_as_hex(ui: &mut egui::Ui, app: &BendApp, target_offset: usize) {
    let Some(editor) = &app.editor else { return };

    let (start, end) = editor
        .selection()
        .unwrap_or((target_offset, target_offset + 1));
    let bytes = editor.bytes_in_range(start, end);

    let table = hex_table();
    let mut hex_string = String::with_capacity(bytes.len() * 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 {
            hex_string.push(' ');
        }
        hex_string.push_str(table[b as usize]);
    }

    ui.output_mut(|o| o.copied_text = hex_string);
}

/// Copy selected bytes as ASCII string to clipboard
fn copy_as_ascii(ui: &mut egui::Ui, app: &BendApp, target_offset: usize) {
    let Some(editor) = &app.editor else { return };

    let (start, end) = editor
        .selection()
        .unwrap_or((target_offset, target_offset + 1));
    let bytes = editor.bytes_in_range(start, end);

    let ascii_string: String = bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            }
        })
        .collect();

    ui.output_mut(|o| o.copied_text = ascii_string);
}

/// Paste bytes from clipboard (mode-dependent)
/// - Hex mode: parse clipboard as hex bytes ("FF 00" or "FF00")
/// - ASCII mode: interpret clipboard as raw text, write each character's byte value
fn paste_hex(_ui: &mut egui::Ui, app: &mut BendApp, target_offset: usize) {
    // Read from system clipboard
    let Some(text) = read_clipboard() else {
        return;
    };

    let Some(editor) = &mut app.editor else {
        return;
    };

    if let Some(bytes) = parse_paste_input(&text, editor.edit_mode()) {
        apply_paste_bytes(editor, target_offset, &bytes);
    }
}

/// Parse paste/clipboard text into bytes based on the current edit mode
fn parse_paste_input(text: &str, mode: EditMode) -> Option<Vec<u8>> {
    match mode {
        EditMode::Hex => parse_hex_input(text),
        EditMode::Ascii => {
            let bytes: Vec<u8> = text
                .bytes()
                .filter(|&b| (0x20..=0x7E).contains(&b))
                .collect();
            if bytes.is_empty() {
                None
            } else {
                Some(bytes)
            }
        }
    }
}

/// Apply parsed bytes at the given offset, respecting write mode
fn apply_paste_bytes(editor: &mut crate::editor::EditorState, offset: usize, bytes: &[u8]) {
    if editor.write_mode() == WriteMode::Insert {
        editor.insert_bytes(offset, bytes);
    } else {
        for (i, byte) in bytes.iter().enumerate() {
            let target = offset + i;
            if target < editor.len() {
                editor.edit_byte(target, *byte);
            }
        }
    }
}

/// Read text from the system clipboard
fn read_clipboard() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut c| c.get_text().ok())
        .filter(|s| !s.is_empty())
}

/// Parse hex input string into bytes (supports "FF FF FF" or "FFFFFF" formats)
fn parse_hex_input(input: &str) -> Option<Vec<u8>> {
    let clean: String = input.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    if clean.is_empty() || !clean.len().is_multiple_of(2) {
        return None;
    }

    let bytes: Option<Vec<u8>> = (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i + 2], 16).ok())
        .collect();

    bytes
}
