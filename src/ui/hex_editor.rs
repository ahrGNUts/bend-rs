//! Hex editor UI component with virtual scrolling

use crate::app::BendApp;
use crate::editor::buffer::NibblePosition;
use crate::formats::RiskLevel;
use eframe::egui::{self, RichText, TextStyle};

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

/// Show the hex editor panel
pub fn show(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(editor) = &app.editor else {
        return;
    };

    let total_bytes = editor.len();
    let total_rows = (total_bytes + BYTES_PER_ROW - 1) / BYTES_PER_ROW;

    // Cache cursor and selection state for rendering
    let cursor_pos = editor.cursor();
    let cursor_nibble = editor.nibble();
    let selection = editor.selection();

    // Get monospace font metrics
    let row_height = ui.text_style_height(&TextStyle::Monospace);

    // Calculate content height for scrolling
    let content_height = total_rows as f32 * row_height;

    // Track clicked byte offset for deferred cursor update
    let mut clicked_offset: Option<usize> = None;
    // Track whether shift was held during click
    let shift_held = ui.input(|i| i.modifiers.shift);
    // Track pending high-risk edit for warning dialog
    let mut pending_high_risk_edit: Option<(u8, usize, RiskLevel)> = None;
    // Track right-clicked byte for context menu
    let mut context_menu_offset: Option<usize> = None;

    // Pre-compute section colors for the entire file
    // This is cached in app.cached_sections so the lookup is fast
    let get_section_color = |offset: usize| -> Option<egui::Color32> {
        app.section_color_for_offset(offset)
    };

    // Check if an offset is within a search match
    let pattern_len = app.search_state.pattern_length();
    let current_match_offset = app.search_state.current_match_offset();
    let is_search_match = |offset: usize| -> bool {
        app.search_state.is_within_match(offset, pattern_len)
    };
    let is_current_match = |offset: usize| -> bool {
        current_match_offset.map_or(false, |m| offset >= m && offset < m + pattern_len)
    };

    // Check if an offset has a bookmark
    let has_bookmark = |offset: usize| -> bool {
        app.editor.as_ref().map_or(false, |e| e.has_bookmark_at(offset))
    };

    // Check if an offset is protected (header protection enabled)
    let is_protected = |offset: usize| -> bool {
        app.is_offset_protected(offset)
    };

    // Check if cursor is at a protected position
    let cursor_protected = app.is_offset_protected(cursor_pos);

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show_viewport(ui, |ui, viewport| {
            // Calculate which rows are visible
            let first_visible_row = (viewport.min.y / row_height).floor() as usize;
            let last_visible_row = ((viewport.max.y / row_height).ceil() as usize).min(total_rows);

            // Add buffer rows
            let render_start = first_visible_row.saturating_sub(BUFFER_ROWS);
            let render_end = (last_visible_row + BUFFER_ROWS).min(total_rows);

            // Reserve space for rows before visible area
            if render_start > 0 {
                ui.allocate_space(egui::vec2(ui.available_width(), render_start as f32 * row_height));
            }

            // Get the editor reference again (immutable borrow for reading bytes)
            let editor = app.editor.as_ref().unwrap();

            // Render visible rows
            for row_idx in render_start..render_end {
                let offset = row_idx * BYTES_PER_ROW;
                let row_end = (offset + BYTES_PER_ROW).min(total_bytes);
                // Copy the bytes to avoid borrow issues
                let row_bytes: Vec<u8> = editor.bytes_in_range(offset, row_end).to_vec();

                ui.horizontal(|ui| {
                    // Offset column (8 hex digits)
                    ui.label(
                        RichText::new(format!("{:08X}", offset))
                            .monospace()
                            .color(egui::Color32::GRAY),
                    );

                    ui.add_space(OFFSET_HEX_SPACING);

                    // Hex bytes
                    for (i, byte) in row_bytes.iter().enumerate() {
                        // Space after every 8 bytes
                        if i == 8 {
                            ui.add_space(HEX_GROUP_SPACING);
                        }

                        let byte_offset = offset + i;
                        let is_cursor = byte_offset == cursor_pos;
                        let is_selected = selection
                            .map(|(start, end)| byte_offset >= start && byte_offset < end)
                            .unwrap_or(false);

                        // Get section color for background (if not cursor/selected)
                        let section_bg = get_section_color(byte_offset);

                        // For cursor position, show nibble highlight
                        if is_cursor {
                            let high_nibble = format!("{:X}", (byte >> 4) & 0x0F);
                            let low_nibble = format!("{:X}", byte & 0x0F);

                            let (high_bg, low_bg) = match cursor_nibble {
                                NibblePosition::High => (
                                    egui::Color32::from_rgb(80, 80, 160),
                                    egui::Color32::from_rgb(40, 40, 80),
                                ),
                                NibblePosition::Low => (
                                    egui::Color32::from_rgb(40, 40, 80),
                                    egui::Color32::from_rgb(80, 80, 160),
                                ),
                            };

                            let high_text = RichText::new(high_nibble)
                                .monospace()
                                .background_color(high_bg);
                            let low_text = RichText::new(low_nibble)
                                .monospace()
                                .background_color(low_bg);

                            let r1 = ui.label(high_text);
                            let r2 = ui.label(low_text);

                            if r1.clicked() || r2.clicked() {
                                clicked_offset = Some(byte_offset);
                            }
                            if r1.secondary_clicked() || r2.secondary_clicked() {
                                context_menu_offset = Some(byte_offset);
                            }
                        } else {
                            let text = format!("{:02X}", byte);
                            let mut rich_text = RichText::new(text).monospace();
                            let byte_protected = is_protected(byte_offset);

                            // Apply background color priority: selection > current_match > search_match > bookmark > protected > section
                            if is_selected {
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(40, 80, 40));
                            } else if is_current_match(byte_offset) {
                                // Current match: bright orange highlight
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(200, 120, 40));
                            } else if is_search_match(byte_offset) {
                                // Other matches: yellow highlight
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(180, 180, 60));
                            } else if has_bookmark(byte_offset) {
                                // Bookmark: cyan highlight
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(60, 160, 180));
                            } else if byte_protected {
                                // Protected region: red-tinted background with strikethrough effect
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(140, 50, 50));
                            } else if let Some(bg) = section_bg {
                                rich_text = rich_text.background_color(bg);
                            }

                            // Make bytes clickable
                            let response = ui.label(rich_text);
                            if response.clicked() {
                                clicked_offset = Some(byte_offset);
                            }
                            if response.secondary_clicked() {
                                context_menu_offset = Some(byte_offset);
                            }
                        }
                    }

                    // Pad remaining space if row is incomplete
                    let missing = BYTES_PER_ROW - row_bytes.len();
                    if missing > 0 {
                        ui.add_space(missing as f32 * HEX_BYTE_WIDTH);
                    }

                    ui.add_space(HEX_ASCII_SPACING);

                    // ASCII column
                    ui.label(
                        RichText::new("|").monospace().color(egui::Color32::DARK_GRAY),
                    );

                    let ascii: String = row_bytes
                        .iter()
                        .map(|&b| {
                            if b.is_ascii_graphic() || b == b' ' {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();

                    ui.label(RichText::new(ascii).monospace());

                    ui.label(
                        RichText::new("|").monospace().color(egui::Color32::DARK_GRAY),
                    );
                });
            }

            // Reserve space for rows after visible area
            let rows_after = total_rows.saturating_sub(render_end);
            if rows_after > 0 {
                ui.allocate_space(egui::vec2(ui.available_width(), rows_after as f32 * row_height));
            }

            // Ensure content height is correct for scroll
            let _ = content_height;
        });

    // Handle deferred click
    if let Some(offset) = clicked_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_cursor_with_selection(offset, shift_held);
            app.preview_dirty = true;
        }
    }

    // Handle keyboard input - track if we need to mark dirty after
    let mut needs_preview_update = false;

    // Pre-compute warning state before mutable borrow of editor
    let should_warn_for_cursor = app.should_warn_for_edit(cursor_pos);
    let cursor_risk_level = app.get_high_risk_level(cursor_pos);

    ui.input(|i| {
        let Some(editor) = &mut app.editor else {
            return;
        };

        let shift = i.modifiers.shift;
        let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;

        // Navigation with optional selection extension
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

        // Undo/Redo

        if ctrl && !shift && i.key_pressed(egui::Key::Z) {
            if editor.undo() {
                needs_preview_update = true;
            }
        }
        if ctrl && shift && i.key_pressed(egui::Key::Z) {
            if editor.redo() {
                needs_preview_update = true;
            }
        }

        // Hex input (0-9, A-F) - nibble-level editing
        // Skip if cursor is at a protected position
        if !cursor_protected {
            for event in &i.events {
                if let egui::Event::Text(text) = event {
                    for c in text.chars() {
                        if let Some(nibble) = c.to_digit(16) {
                            // Check if this edit should trigger a warning
                            if should_warn_for_cursor {
                                // Store pending edit for confirmation
                                if let Some(risk) = cursor_risk_level {
                                    pending_high_risk_edit = Some((nibble as u8, cursor_pos, risk));
                                }
                            } else {
                                let _ = editor.edit_nibble(nibble as u8);
                                needs_preview_update = true;
                            }
                        }
                    }
                }
            }
        }
    });

    // Handle pending high-risk edit (after input borrow ends)
    if let Some((nibble_value, offset, risk_level)) = pending_high_risk_edit {
        app.pending_high_risk_edit = Some(crate::app::PendingEdit {
            nibble_value,
            offset,
            risk_level,
        });
    }

    // Mark preview dirty with debounce timestamp (after editor borrow ends)
    if needs_preview_update {
        app.mark_preview_dirty();
    }

    // Handle context menu right-click
    if let Some(offset) = context_menu_offset {
        app.context_menu_state.target_offset = Some(offset);
    }

    // Show context menu if active
    show_context_menu(ui, app);
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
    let (start, end) = app.editor.as_ref()
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
                if ui.button(format!("Copy as ASCII{}", label_suffix)).clicked() {
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
    let clicked_outside = ctx.input(|i| {
        i.pointer.any_click() && !i.pointer.secondary_down()
    });
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

    let (start, end) = editor.selection().unwrap_or((target_offset, target_offset + 1));
    let bytes = editor.bytes_in_range(start, end);

    let hex_string: String = bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");

    ui.output_mut(|o| o.copied_text = hex_string);
}

/// Copy selected bytes as ASCII string to clipboard
fn copy_as_ascii(ui: &mut egui::Ui, app: &BendApp, target_offset: usize) {
    let Some(editor) = &app.editor else { return };

    let (start, end) = editor.selection().unwrap_or((target_offset, target_offset + 1));
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

/// Paste hex string from clipboard
fn paste_hex(ui: &mut egui::Ui, app: &mut BendApp, target_offset: usize) {
    let clipboard_text = ui.input(|i| i.events.iter().find_map(|e| {
        if let egui::Event::Paste(text) = e {
            Some(text.clone())
        } else {
            None
        }
    }));

    // Try to get text from clipboard via output
    let text = clipboard_text.unwrap_or_else(|| {
        // Fallback: read from platform clipboard if available
        String::new()
    });

    if text.is_empty() {
        return;
    }

    // Try to parse as hex bytes
    let bytes = parse_hex_input(&text);

    if let (Some(editor), Some(bytes)) = (&mut app.editor, bytes) {
        for (i, byte) in bytes.iter().enumerate() {
            let offset = target_offset + i;
            if offset < editor.len() {
                editor.edit_byte(offset, *byte);
            }
        }
        app.mark_preview_dirty();
    }
}

/// Parse hex input string into bytes (supports "FF FF FF" or "FFFFFF" formats)
fn parse_hex_input(input: &str) -> Option<Vec<u8>> {
    let clean: String = input
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();

    if clean.len() % 2 != 0 {
        return None;
    }

    let bytes: Option<Vec<u8>> = (0..clean.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&clean[i..i+2], 16).ok())
        .collect();

    bytes
}
