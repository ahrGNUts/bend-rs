//! Hex editor UI component with virtual scrolling

use crate::app::BendApp;
use crate::editor::buffer::NibblePosition;
use eframe::egui::{self, RichText, TextStyle};

/// Number of bytes per row
const BYTES_PER_ROW: usize = 16;

/// Number of rows to render above/below viewport for smooth scrolling
const BUFFER_ROWS: usize = 2;

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

    // Pre-compute section colors for the entire file
    // This is cached in app.cached_sections so the lookup is fast
    let get_section_color = |offset: usize| -> Option<egui::Color32> {
        app.section_color_for_offset(offset)
    };

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

                    ui.add_space(8.0);

                    // Hex bytes
                    for (i, byte) in row_bytes.iter().enumerate() {
                        // Space after every 8 bytes
                        if i == 8 {
                            ui.add_space(8.0);
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
                        } else {
                            let text = format!("{:02X}", byte);
                            let mut rich_text = RichText::new(text).monospace();

                            // Apply background color: selection > section
                            if is_selected {
                                rich_text = rich_text.background_color(egui::Color32::from_rgb(40, 80, 40));
                            } else if let Some(bg) = section_bg {
                                rich_text = rich_text.background_color(bg);
                            }

                            // Make bytes clickable
                            let response = ui.label(rich_text);
                            if response.clicked() {
                                clicked_offset = Some(byte_offset);
                            }
                        }
                    }

                    // Pad remaining space if row is incomplete
                    let missing = BYTES_PER_ROW - row_bytes.len();
                    if missing > 0 {
                        ui.add_space(missing as f32 * 24.0); // Approximate width of "XX "
                    }

                    ui.add_space(16.0);

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
        for event in &i.events {
            if let egui::Event::Text(text) = event {
                for c in text.chars() {
                    if let Some(nibble) = c.to_digit(16) {
                        editor.edit_nibble(nibble as u8);
                        needs_preview_update = true;
                    }
                }
            }
        }
    });

    // Mark preview dirty with debounce timestamp (after editor borrow ends)
    if needs_preview_update {
        app.mark_preview_dirty();
    }
}
