//! Hex editor UI component with virtual scrolling

use crate::app::{BendApp, PendingEditType};
use crate::editor::buffer::{EditMode, NibblePosition, WriteMode};
use crate::editor::{is_printable_ascii, is_printable_ascii_char};
use crate::formats::RiskLevel;
use crate::ui::theme::AppColors;
use crate::ui::PointerCursor;
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

/// Number of rows to scroll above target when jumping to an offset
const SCROLL_BUFFER_ROWS: usize = 5;

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

/// Resolve the bright/dim cursor color pair for the current write mode.
/// Insert mode uses the insert-flavor palette; overwrite mode uses the
/// overwrite-flavor palette. Returns `(bright, dim)`.
fn cursor_color_pair(write_mode: WriteMode, colors: &AppColors) -> (egui::Color32, egui::Color32) {
    if write_mode == WriteMode::Insert {
        (colors.cursor_bright_insert, colors.cursor_dim_insert)
    } else {
        (colors.cursor_bright_overwrite, colors.cursor_dim_overwrite)
    }
}

/// Pick the non-cursor background color for a byte based on highlight flags.
/// Priority: selection > current_match > search_match > bookmark > section tint.
/// Returns `None` when the byte has no applicable background. Cursor painting is
/// handled by the caller because it uses split-nibble rendering in the hex column.
fn byte_background_color(highlight: &ByteHighlight, colors: &AppColors) -> Option<egui::Color32> {
    if highlight.is_selected {
        Some(colors.selection_bg)
    } else if highlight.is_current_match {
        Some(colors.current_match_bg)
    } else if highlight.is_search_match {
        Some(colors.search_match_bg)
    } else if highlight.has_bookmark {
        Some(colors.bookmark_bg)
    } else {
        highlight.section_bg
    }
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
    colors: &AppColors,
) -> egui::Response {
    let hex = hex_table()[byte as usize];
    let font_id = TextStyle::Monospace.resolve(ui.style());

    // Determine background and text color based on priority:
    // cursor > selection > current_match > search_match > bookmark > section
    let bg = if highlight.is_cursor {
        None // cursor uses split-nibble backgrounds below
    } else {
        byte_background_color(highlight, colors)
    };

    let text_color = if highlight.is_cursor {
        colors.cursor_text
    } else if bg.is_some() {
        colors.hex_byte_text
    } else {
        ui.visuals().text_color()
    };

    // Layout text and allocate an interactive rect of exactly that size
    let galley = ui.fonts(|f| f.layout_no_wrap(hex.to_string(), font_id.clone(), text_color));
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::click_and_drag());

    // Paint backgrounds
    if highlight.is_cursor {
        let half_width = rect.width() / 2.0;
        let (bright, dim) = cursor_color_pair(write_mode, colors);
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
    } else if let Some(bg_color) = bg {
        ui.painter().rect_filled(rect, 0.0, bg_color);
    }

    // Paint text on top of backgrounds
    ui.painter().galley(rect.min, galley, text_color);

    // Strikethrough for protected bytes
    if highlight.is_protected {
        let y = rect.center().y;
        ui.painter().line_segment(
            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
            egui::Stroke::new(1.0, text_color),
        );
    }

    response
}

/// Render the entire ASCII row as a single label with painter-based highlighting.
/// Returns (response, clicked_byte_offset, context_menu_byte_offset).
fn render_ascii_row(
    ui: &mut egui::Ui,
    row_bytes: &[u8],
    row_offset: usize,
    state: &HexDisplayState,
    colors: &AppColors,
) -> (egui::Response, Option<usize>, Option<usize>) {
    // Build the full 16-char string: real chars + NBSP padding
    let mut text = String::with_capacity(BYTES_PER_ROW);
    for byte in row_bytes {
        if is_printable_ascii(*byte) {
            text.push(*byte as char);
        } else {
            text.push('.');
        }
    }
    let missing = BYTES_PER_ROW - row_bytes.len();
    for _ in 0..missing {
        text.push('\u{00A0}'); // non-breaking space (not trimmed by text layout)
    }

    // Paint the row manually: egui's Label changes the text color on hover when
    // given an interactive sense, which makes the whole ASCII row "light up".
    // Allocating the rect ourselves and painting the galley directly avoids that
    // while preserving click/drag detection for selection.
    let font_id = TextStyle::Monospace.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let galley = ui.fonts(|f| f.layout_no_wrap(text, font_id.clone(), text_color));
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::click_and_drag());
    ui.painter().galley(rect.min, galley, text_color);
    let char_width = rect.width() / BYTES_PER_ROW as f32;
    let font_id = TextStyle::Monospace.resolve(ui.style());

    // Draw highlights and re-paint characters on top
    let (cursor_bright, cursor_dim) = cursor_color_pair(state.write_mode, colors);

    for (i, byte) in row_bytes.iter().enumerate() {
        let byte_offset = row_offset + i;
        let is_cursor = byte_offset == state.cursor_pos;
        let is_selected = state
            .selection
            .map(|(start, end)| byte_offset >= start && byte_offset < end)
            .unwrap_or(false);

        if !is_cursor && !is_selected {
            continue;
        }

        let char_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + i as f32 * char_width, rect.min.y),
            egui::vec2(char_width, rect.height()),
        );

        let display_char = if is_printable_ascii(*byte) {
            *byte as char
        } else {
            '.'
        };

        if is_cursor {
            let bg = if state.edit_mode == EditMode::Ascii {
                cursor_bright
            } else {
                cursor_dim
            };
            ui.painter().rect_filled(char_rect, 0.0, bg);
            ui.painter().text(
                char_rect.center(),
                egui::Align2::CENTER_CENTER,
                display_char,
                font_id.clone(),
                colors.cursor_text,
            );
        } else if is_selected {
            ui.painter()
                .rect_filled(char_rect, 0.0, colors.selection_bg);
            ui.painter().text(
                char_rect.center(),
                egui::Align2::CENTER_CENTER,
                display_char,
                font_id.clone(),
                ui.visuals().text_color(),
            );
        }
    }

    // Detect click positions
    let mut clicked_offset = None;
    let mut context_offset = None;

    if response.clicked() || response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let char_idx = ((pos.x - rect.min.x) / char_width).floor() as usize;
            if char_idx < row_bytes.len() {
                let byte_offset = row_offset + char_idx;
                if response.clicked() {
                    clicked_offset = Some(byte_offset);
                }
                if response.secondary_clicked() {
                    context_offset = Some(byte_offset);
                }
            }
        }
    }

    (response, clicked_offset, context_offset)
}

/// Navigation keys that follow the move-by-delta pattern: (key, cursor delta).
const NAV_KEYS: &[(egui::Key, isize)] = &[
    (egui::Key::ArrowLeft, -1),
    (egui::Key::ArrowRight, 1),
    (egui::Key::ArrowUp, -(BYTES_PER_ROW as isize)),
    (egui::Key::ArrowDown, BYTES_PER_ROW as isize),
    (egui::Key::PageUp, -(BYTES_PER_ROW as isize * 16)),
    (egui::Key::PageDown, BYTES_PER_ROW as isize * 16),
];

/// Handle navigation keys (arrows, page up/down, home/end) with optional selection extension.
fn handle_navigation_keys(editor: &mut crate::editor::EditorState, i: &egui::InputState) {
    let shift = i.modifiers.shift;

    for &(key, delta) in NAV_KEYS {
        if i.key_pressed(key) {
            if shift {
                editor.move_cursor_with_selection(delta);
            } else {
                editor.clear_selection();
                editor.move_cursor(delta);
            }
        }
    }

    // Home/End use set_cursor/extend_selection_to instead of move_cursor
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

/// Collected result of one frame of hex-editor edit input (text entry,
/// backspace, delete, paste). Named fields rather than a tuple so callers
/// read intent at the use site.
#[derive(Default)]
struct EditInputResult {
    /// A high-risk edit the user attempted that now needs confirmation.
    pending_high_risk_edit: Option<(PendingEditType, usize, RiskLevel)>,
    /// Clipboard text pasted this frame, to be parsed and applied outside the
    /// `ui.input_mut` closure (so we can take a `&mut editor` without borrow
    /// conflicts).
    paste_text: Option<String>,
}

/// Handle edit input: text entry, backspace, delete, and paste.
fn handle_edit_input(
    editor: &mut crate::editor::EditorState,
    i: &mut egui::InputState,
    cursor_pos: usize,
    cursor_protected: bool,
    should_warn_for_cursor: bool,
    cursor_risk_level: Option<RiskLevel>,
    current_edit_mode: EditMode,
) -> EditInputResult {
    let mut result = EditInputResult::default();

    if cursor_protected {
        return result;
    }

    // Backspace key
    if i.key_pressed(egui::Key::Backspace) {
        if should_warn_for_cursor && editor.write_mode() == WriteMode::Insert {
            if let Some(risk) = cursor_risk_level {
                result.pending_high_risk_edit =
                    Some((PendingEditType::Backspace, cursor_pos, risk));
            }
        } else {
            editor.handle_backspace();
        }
    }
    // Delete key
    if i.key_pressed(egui::Key::Delete) {
        if should_warn_for_cursor && editor.write_mode() == WriteMode::Insert {
            if let Some(risk) = cursor_risk_level {
                result.pending_high_risk_edit = Some((PendingEditType::Delete, cursor_pos, risk));
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
                                        result.pending_high_risk_edit = Some((
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
                            if is_printable_ascii_char(c) {
                                if should_warn_for_cursor {
                                    if let Some(risk) = cursor_risk_level {
                                        result.pending_high_risk_edit =
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
                result.paste_text = Some(text.clone());
            }
            _ => {}
        }
    }

    result
}

/// Snapshot of pointer state passed into `render_row` so the pure render
/// function can detect drag-over targets without re-reading `ui.input`.
struct PointerContext {
    pointer_pos: Option<egui::Pos2>,
    primary_down: bool,
    drag_active: bool,
}

/// Input events detected while rendering one row. `render_row` returns this
/// without mutating editor state; `handle_row_interactions` applies it afterward.
#[derive(Default)]
struct RowResult {
    /// Byte offset the user clicked or drag-started on, with the originating column.
    cursor_move: Option<(usize, EditMode)>,
    /// True if a drag just started on this row.
    start_drag: bool,
    /// The byte offset the pointer is over during an active drag (for selection extension).
    drag_current_offset: Option<usize>,
    /// The byte offset of a secondary-click target (for opening the context menu).
    context_menu_offset: Option<usize>,
}

impl RowResult {
    /// Fold another row's result into this accumulator. Later non-empty values
    /// overwrite earlier ones (last-click-wins, which matches the original
    /// loop's plain-assignment behavior).
    fn merge(&mut self, other: RowResult) {
        if other.cursor_move.is_some() {
            self.cursor_move = other.cursor_move;
        }
        if other.start_drag {
            self.start_drag = true;
        }
        if other.drag_current_offset.is_some() {
            self.drag_current_offset = other.drag_current_offset;
        }
        if other.context_menu_offset.is_some() {
            self.context_menu_offset = other.context_menu_offset;
        }
    }
}

/// Cached display state for the hex editor, read from BendApp once per frame
struct HexDisplayState {
    total_bytes: usize,
    total_rows: usize,
    cursor_pos: usize,
    cursor_nibble: NibblePosition,
    selection: Option<(usize, usize)>,
    edit_mode: EditMode,
    write_mode: WriteMode,
    cursor_protected: bool,
}

/// Pre-computed highlight lookup data for search matches and bookmarks
struct HighlightLookup<'a> {
    app: &'a BendApp,
    current_match_offset: Option<usize>,
    pattern_len: usize,
}

impl<'a> HighlightLookup<'a> {
    fn new(app: &'a BendApp) -> Self {
        Self {
            current_match_offset: app.ui.search_state.current_match_offset(),
            pattern_len: app.ui.search_state.pattern_length(),
            app,
        }
    }

    fn byte_highlight(&self, byte_offset: usize, state: &HexDisplayState) -> ByteHighlight {
        ByteHighlight {
            is_cursor: byte_offset == state.cursor_pos,
            is_selected: state
                .selection
                .map(|(start, end)| byte_offset >= start && byte_offset < end)
                .unwrap_or(false),
            is_search_match: self.app.ui.search_state.is_within_match(byte_offset),
            is_current_match: self
                .current_match_offset
                .is_some_and(|m| byte_offset >= m && byte_offset < m + self.pattern_len),
            has_bookmark: self
                .app
                .doc
                .editor
                .as_ref()
                .is_some_and(|e| e.has_bookmark_at(byte_offset)),
            is_protected: self.app.doc.is_offset_protected(byte_offset),
            section_bg: self.app.section_color_for_offset(byte_offset),
        }
    }
}

/// All the per-frame read state `render_row` needs, bundled so the function
/// takes one context parameter instead of five individual borrows.
struct RowRenderContext<'a> {
    state: &'a HexDisplayState,
    editor: &'a crate::editor::EditorState,
    colors: &'a AppColors,
    highlights: &'a HighlightLookup<'a>,
    pointer: &'a PointerContext,
}

/// Prepare display state from the current editor state.
/// Returns None if no editor is loaded.
fn prepare_display_state(app: &BendApp) -> Option<HexDisplayState> {
    let editor = app.doc.editor.as_ref()?;
    let total_bytes = editor.len();
    Some(HexDisplayState {
        total_bytes,
        total_rows: total_bytes.div_ceil(BYTES_PER_ROW),
        cursor_pos: editor.cursor(),
        cursor_nibble: editor.nibble(),
        selection: editor.selection(),
        edit_mode: editor.edit_mode(),
        write_mode: editor.write_mode(),
        cursor_protected: app.doc.is_offset_protected(editor.cursor()),
    })
}

/// Render one row (offset column + hex bytes + ASCII column) without mutating
/// editor state. Input events are collected into a `RowResult` so the caller
/// can apply them in a single pass after the whole scroll area has painted.
fn render_row(
    ui: &mut egui::Ui,
    row_idx: usize,
    ctx: &RowRenderContext<'_>,
    scroll_to_me: bool,
) -> RowResult {
    let state = ctx.state;
    let editor = ctx.editor;
    let colors = ctx.colors;
    let highlights = ctx.highlights;
    let pointer = ctx.pointer;
    let offset = row_idx * BYTES_PER_ROW;
    let row_end = (offset + BYTES_PER_ROW).min(state.total_bytes);
    let row_bytes = editor.bytes_in_range(offset, row_end);
    let mut result = RowResult::default();

    let row_response = ui.horizontal(|ui| {
        // Offset column
        ui.add(
            egui::Label::new(RichText::new(format!("{:08X}", offset)).monospace())
                .selectable(false),
        );
        ui.add_space(OFFSET_HEX_SPACING);

        // Hex bytes
        for (i, byte) in row_bytes.iter().enumerate() {
            if i == 8 {
                ui.add_space(HEX_GROUP_SPACING);
            }
            let byte_offset = offset + i;
            let highlight = highlights.byte_highlight(byte_offset, state);
            let response = render_hex_byte(
                ui,
                *byte,
                &highlight,
                state.cursor_nibble,
                state.edit_mode,
                state.write_mode,
                colors,
            );
            if response.clicked() {
                result.cursor_move = Some((byte_offset, EditMode::Hex));
            }
            if response.drag_started_by(egui::PointerButton::Primary) {
                result.cursor_move = Some((byte_offset, EditMode::Hex));
                result.start_drag = true;
            }
            if pointer.primary_down && pointer.drag_active {
                if let Some(pos) = pointer.pointer_pos {
                    if response.rect.contains(pos) {
                        result.drag_current_offset = Some(byte_offset);
                    }
                }
            }
            if response.secondary_clicked() {
                result.context_menu_offset = Some(byte_offset);
            }
        }

        // Pad remaining space if row is incomplete (keeps ASCII column aligned).
        for i in row_bytes.len()..BYTES_PER_ROW {
            if i == 8 {
                ui.add_space(HEX_GROUP_SPACING);
            }
            ui.add(
                egui::Label::new(
                    RichText::new("  ")
                        .monospace()
                        .color(egui::Color32::TRANSPARENT),
                )
                .selectable(false),
            );
        }
        ui.add_space(HEX_ASCII_SPACING);

        // ASCII column — bracketed by non-selectable "|" pipes (commit 6b4fdaf).
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add(egui::Label::new(RichText::new("|").monospace()).selectable(false));
        let (ascii_resp, _ascii_click, ascii_ctx) =
            render_ascii_row(ui, &row_bytes, offset, state, colors);
        if let Some(off) = ascii_ctx {
            result.context_menu_offset = Some(off);
        }
        // ASCII click/drag: map pointer x to a char index within the row.
        let ascii_byte_at_pointer = |pos: egui::Pos2| -> Option<usize> {
            if !ascii_resp.rect.contains(pos) {
                return None;
            }
            let char_width = ascii_resp.rect.width() / BYTES_PER_ROW as f32;
            let char_idx = ((pos.x - ascii_resp.rect.min.x) / char_width).floor() as usize;
            if char_idx < row_bytes.len() {
                Some(offset + char_idx)
            } else {
                None
            }
        };
        if ascii_resp.clicked() {
            if let Some(pos) = ascii_resp.interact_pointer_pos() {
                if let Some(byte_offset) = ascii_byte_at_pointer(pos) {
                    result.cursor_move = Some((byte_offset, EditMode::Ascii));
                }
            }
        }
        // Drag started — use press_origin rather than the current pointer
        // position: on the frame drag_started fires, the pointer may already
        // have moved to a different row, which would fall outside this row's
        // rect and the drag would silently never start.
        if ascii_resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(press_pos) = ui.input(|i| i.pointer.press_origin()) {
                if let Some(byte_offset) = ascii_byte_at_pointer(press_pos) {
                    result.cursor_move = Some((byte_offset, EditMode::Ascii));
                    result.start_drag = true;
                }
            }
        }
        if pointer.primary_down && pointer.drag_active {
            if let Some(pos) = pointer.pointer_pos {
                if let Some(byte_offset) = ascii_byte_at_pointer(pos) {
                    result.drag_current_offset = Some(byte_offset);
                }
            }
        }
        ui.add(egui::Label::new(RichText::new("|").monospace()).selectable(false));
    });

    if scroll_to_me {
        row_response
            .response
            .scroll_to_me(Some(egui::Align::Center));
    }

    result
}

/// Show the hex editor panel.
/// Orchestrator: snapshot frame inputs → compute scroll target → run virtual
/// scrolling, calling `render_row` per visible row → apply the merged
/// `RowResult` → handle keyboard input → show context menu.
pub fn show(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(state) = prepare_display_state(app) else {
        return;
    };

    let row_height = ui.text_style_height(&TextStyle::Monospace);
    let shift_held = ui.input(|i| i.modifiers.shift);

    // Snapshot pointer + drag state once so `render_row` sees a consistent view.
    let drag_id = egui::Id::new("hex_editor_drag");
    let pointer = PointerContext {
        pointer_pos: ui.input(|i| i.pointer.latest_pos()),
        primary_down: ui.input(|i| i.pointer.primary_down()),
        drag_active: ui.data(|d| d.get_temp(drag_id).unwrap_or(false)),
    };

    // Take the pending scroll target so we don't re-scroll on the next frame.
    let scroll_to_row: Option<usize> = app
        .ui
        .pending_hex_scroll
        .take()
        .map(|byte_offset| byte_offset / BYTES_PER_ROW);
    let initial_scroll_offset: Option<f32> = scroll_to_row.map(|target_row| {
        (target_row.saturating_sub(SCROLL_BUFFER_ROWS) as f32 * row_height).max(0.0)
    });

    let colors = app.ui.colors;
    let highlights = HighlightLookup::new(app);

    let mut scroll_area = egui::ScrollArea::both().auto_shrink([false; 2]);
    if let Some(offset_y) = initial_scroll_offset {
        scroll_area = scroll_area.vertical_scroll_offset(offset_y);
    }

    let mut result = RowResult::default();
    scroll_area.show_viewport(ui, |ui, viewport| {
        let first_visible_row = (viewport.min.y / row_height).floor() as usize;
        let last_visible_row =
            ((viewport.max.y / row_height).ceil() as usize).min(state.total_rows);
        let render_start = first_visible_row.saturating_sub(BUFFER_ROWS);
        let render_end = (last_visible_row + BUFFER_ROWS).min(state.total_rows);

        if render_start > 0 {
            ui.allocate_space(egui::vec2(
                ui.available_width(),
                render_start as f32 * row_height,
            ));
        }

        let editor = app.doc.editor.as_ref().unwrap();
        let ctx = RowRenderContext {
            state: &state,
            editor,
            colors: &colors,
            highlights: &highlights,
            pointer: &pointer,
        };
        for row_idx in render_start..render_end {
            let row_result = render_row(ui, row_idx, &ctx, scroll_to_row == Some(row_idx));
            result.merge(row_result);
        }

        let rows_after = state.total_rows.saturating_sub(render_end);
        if rows_after > 0 {
            ui.allocate_space(egui::vec2(
                ui.available_width(),
                rows_after as f32 * row_height,
            ));
        }
    });

    handle_row_interactions(
        ui,
        app,
        result,
        &RowInteractionContext {
            shift_held,
            primary_down: pointer.primary_down,
            drag_id,
        },
    );

    let keyboard_result = handle_keyboard_input(ui, app, state.cursor_pos, state.cursor_protected);
    if let Some((edit_type, offset, risk_level)) = keyboard_result.pending_high_risk_edit {
        app.ui.dialogs.pending_high_risk_edit = Some(crate::app::PendingEdit {
            edit_type,
            offset,
            risk_level,
        });
    }

    show_context_menu(ui, app);
}

/// Per-frame inputs needed to apply a `RowResult` to editor + UI state.
/// Symmetric with `RowRenderContext` — bundle the snapshot once at the call
/// site so the function signature stays narrow.
struct RowInteractionContext {
    /// Whether shift was held this frame (extends selection on click).
    shift_held: bool,
    /// Whether the primary mouse button was down this frame (used to decide
    /// whether to clear the in-egui-data drag flag).
    primary_down: bool,
    /// Stable egui `Id` used to read/write the active-drag flag in egui's
    /// per-frame data store.
    drag_id: egui::Id,
}

/// Apply a `RowResult` (click, drag start/extend, secondary click, context
/// menu target) to editor + UI state. Runs after the scroll area has painted,
/// so mutating the editor here doesn't invalidate any per-frame read state.
fn handle_row_interactions(
    ui: &mut egui::Ui,
    app: &mut BendApp,
    result: RowResult,
    ctx: &RowInteractionContext,
) {
    if let Some((off, mode)) = result.cursor_move {
        if let Some(editor) = &mut app.doc.editor {
            editor.set_edit_mode(mode);
            editor.set_cursor_with_selection(off, ctx.shift_held);
        }
    }
    if result.start_drag {
        ui.data_mut(|d| d.insert_temp(ctx.drag_id, true));
    }
    if let Some(off) = result.drag_current_offset {
        if let Some(editor) = &mut app.doc.editor {
            editor.extend_selection_to(off);
        }
    }
    if !ctx.primary_down {
        ui.data_mut(|d| d.insert_temp::<bool>(ctx.drag_id, false));
    }
    if let Some(offset) = result.context_menu_offset {
        app.ui.context_menu_state.target_offset = Some(offset);
    }
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
    // Don't process hex editor input when any egui widget (text field, etc.) has focus
    if ui.ctx().memory(|m| m.focused().is_some()) {
        return KeyboardResult {
            pending_high_risk_edit: None,
        };
    }

    // Pre-compute warning state before mutable borrow of editor
    let should_warn_for_cursor = app.should_warn_for_edit(cursor_pos);
    let cursor_risk_level = app.doc.get_high_risk_level(cursor_pos);

    // Cache edit mode for text input handling
    let current_edit_mode = app
        .doc
        .editor
        .as_ref()
        .map(|e| e.edit_mode())
        .unwrap_or(EditMode::Hex);

    let (edit_result, copy_requested) = ui.input_mut(|i| {
        let Some(editor) = &mut app.doc.editor else {
            return (EditInputResult::default(), false);
        };

        let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;

        // Ctrl+I / Cmd+I toggles Insert/Overwrite mode
        if ctrl && i.key_pressed(egui::Key::I) {
            editor.toggle_write_mode();
        }

        // Detect copy request (Cmd+C / Ctrl+C)
        let copy_requested = i.events.iter().any(|e| matches!(e, egui::Event::Copy))
            || (ctrl && i.key_pressed(egui::Key::C));

        // Navigation keys (arrows, page up/down, home/end)
        handle_navigation_keys(editor, i);

        // Edit input (text entry, backspace, delete, paste)
        let edit_result = handle_edit_input(
            editor,
            i,
            cursor_pos,
            cursor_protected,
            should_warn_for_cursor,
            cursor_risk_level,
            current_edit_mode,
        );
        (edit_result, copy_requested)
    });

    // Handle copy outside the input closure — always override egui's native copy
    // so that only the column matching the current edit mode ends up on the clipboard
    if copy_requested {
        if let Some(editor) = &app.doc.editor {
            let (start, end) = editor.selection().unwrap_or((cursor_pos, cursor_pos + 1));
            let bytes = editor.bytes_in_range(start, end);
            let formatted = match current_edit_mode {
                EditMode::Hex => format_bytes_as_hex(bytes),
                EditMode::Ascii => format_bytes_as_ascii(bytes),
            };
            ui.output_mut(|o| o.copied_text = formatted);
        }
    }

    // Handle paste outside the input closure
    if let Some(text) = edit_result.paste_text {
        if let Some(bytes) = parse_paste_input(&text, current_edit_mode) {
            if let Some(editor) = &mut app.doc.editor {
                apply_paste_bytes(editor, cursor_pos, &bytes);
            }
        }
    }

    // Check if buffer length changed and invalidate caches
    if let Some(editor) = &mut app.doc.editor {
        if editor.take_length_changed() {
            // Re-parse file structure since offsets shifted
            app.doc.cached_sections = crate::formats::parse_file(editor.working());
        }
    }

    KeyboardResult {
        pending_high_risk_edit: edit_result.pending_high_risk_edit,
    }
}

enum ContextAction {
    CopyHex,
    CopyAscii,
    Paste,
    AddBookmark,
    GoToOffset,
}

/// Show the context menu for the hex editor
fn show_context_menu(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(target_offset) = app.ui.context_menu_state.target_offset else {
        return;
    };

    let mut close_menu = false;
    let mut action: Option<ContextAction> = None;

    // Determine if we have a selection or just cursor
    let (start, end) = app
        .doc
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

                if ui
                    .button(format!("Copy as Hex{}", label_suffix))
                    .pointer_cursor()
                    .clicked()
                {
                    action = Some(ContextAction::CopyHex);
                    close_menu = true;
                }
                if ui
                    .button(format!("Copy as ASCII{}", label_suffix))
                    .pointer_cursor()
                    .clicked()
                {
                    action = Some(ContextAction::CopyAscii);
                    close_menu = true;
                }

                ui.separator();

                if ui.button("Paste").pointer_cursor().clicked() {
                    action = Some(ContextAction::Paste);
                    close_menu = true;
                }

                ui.separator();

                if ui.button("Add Bookmark").pointer_cursor().clicked() {
                    action = Some(ContextAction::AddBookmark);
                    close_menu = true;
                }
                if ui.button("Go to Offset...").pointer_cursor().clicked() {
                    action = Some(ContextAction::GoToOffset);
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

    // Handle action
    match action {
        Some(ContextAction::CopyHex) => copy_as_hex(ui, app, target_offset),
        Some(ContextAction::CopyAscii) => copy_as_ascii(ui, app, target_offset),
        Some(ContextAction::Paste) => paste_hex(ui, app, target_offset),
        Some(ContextAction::AddBookmark) => {
            if let Some(editor) = &mut app.doc.editor {
                editor.add_bookmark(target_offset, format!("Offset 0x{:X}", target_offset));
            }
        }
        Some(ContextAction::GoToOffset) => app.ui.go_to_offset_state.open_dialog(),
        None => {}
    }

    if close_menu {
        app.ui.context_menu_state.target_offset = None;
    }
}

/// Format bytes as space-separated hex pairs (e.g., "FF 00 AB")
fn format_bytes_as_hex(bytes: &[u8]) -> String {
    let table = hex_table();
    let mut hex_string = String::with_capacity(bytes.len() * 3);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 {
            hex_string.push(' ');
        }
        hex_string.push_str(table[b as usize]);
    }
    hex_string
}

/// Format bytes as ASCII string (non-printable bytes become '.')
fn format_bytes_as_ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if is_printable_ascii(b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

/// Copy selected bytes as hex string to clipboard
fn copy_as_hex(ui: &mut egui::Ui, app: &BendApp, target_offset: usize) {
    let Some(editor) = &app.doc.editor else {
        return;
    };

    let (start, end) = editor
        .selection()
        .unwrap_or((target_offset, target_offset + 1));
    let bytes = editor.bytes_in_range(start, end);
    ui.output_mut(|o| o.copied_text = format_bytes_as_hex(bytes));
}

/// Copy selected bytes as ASCII string to clipboard
fn copy_as_ascii(ui: &mut egui::Ui, app: &BendApp, target_offset: usize) {
    let Some(editor) = &app.doc.editor else {
        return;
    };

    let (start, end) = editor
        .selection()
        .unwrap_or((target_offset, target_offset + 1));
    let bytes = editor.bytes_in_range(start, end);
    ui.output_mut(|o| o.copied_text = format_bytes_as_ascii(bytes));
}

/// Paste bytes from clipboard (mode-dependent)
/// - Hex mode: parse clipboard as hex bytes ("FF 00" or "FF00")
/// - ASCII mode: interpret clipboard as raw text, write each character's byte value
fn paste_hex(_ui: &mut egui::Ui, app: &mut BendApp, target_offset: usize) {
    // Read from system clipboard
    let Some(text) = read_clipboard() else {
        return;
    };

    let Some(editor) = &mut app.doc.editor else {
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
            let bytes: Vec<u8> = text.bytes().filter(|&b| is_printable_ascii(b)).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `RowResult::merge` is last-value-wins: a non-`None` field on the
    /// incoming row overwrites the accumulator, but a `None`/`false` does
    /// not clear an earlier value. This matches the original loop's plain
    /// `cursor_move = Some(...)` assignment semantics across rows — the
    /// last row to detect an event "wins" the frame.
    #[test]
    fn row_result_merge_is_last_value_wins() {
        let mut acc = RowResult {
            cursor_move: Some((10, EditMode::Hex)),
            start_drag: false,
            drag_current_offset: Some(7),
            context_menu_offset: Some(5),
        };
        let later = RowResult {
            cursor_move: Some((20, EditMode::Ascii)),
            start_drag: true,
            drag_current_offset: Some(15),
            // Intentionally None — must not clear acc's existing value.
            context_menu_offset: None,
        };

        acc.merge(later);

        // Non-None fields on `later` overwrite acc.
        assert_eq!(acc.cursor_move, Some((20, EditMode::Ascii)));
        assert_eq!(acc.drag_current_offset, Some(15));
        // start_drag is monotonic-true: once set, stays set.
        assert!(acc.start_drag);
        // context_menu_offset on later was None; acc's earlier Some is kept.
        assert_eq!(acc.context_menu_offset, Some(5));
    }

    /// Default + a populated row → that row's values are taken verbatim.
    #[test]
    fn row_result_merge_into_default_takes_other() {
        let mut acc = RowResult::default();
        let row = RowResult {
            cursor_move: Some((42, EditMode::Hex)),
            start_drag: true,
            drag_current_offset: Some(42),
            context_menu_offset: Some(99),
        };

        acc.merge(row);

        assert_eq!(acc.cursor_move, Some((42, EditMode::Hex)));
        assert!(acc.start_drag);
        assert_eq!(acc.drag_current_offset, Some(42));
        assert_eq!(acc.context_menu_offset, Some(99));
    }

    /// Empty row merged into populated accumulator → accumulator unchanged.
    #[test]
    fn row_result_merge_empty_preserves_acc() {
        let mut acc = RowResult {
            cursor_move: Some((1, EditMode::Hex)),
            start_drag: true,
            drag_current_offset: Some(2),
            context_menu_offset: Some(3),
        };

        acc.merge(RowResult::default());

        assert_eq!(acc.cursor_move, Some((1, EditMode::Hex)));
        assert!(acc.start_drag);
        assert_eq!(acc.drag_current_offset, Some(2));
        assert_eq!(acc.context_menu_offset, Some(3));
    }
}
