//! Hex editor UI component with virtual scrolling

use crate::app::BendApp;
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
    let selection = editor.selection();

    // Get monospace font metrics
    let row_height = ui.text_style_height(&TextStyle::Monospace);

    // Calculate content height for scrolling
    let content_height = total_rows as f32 * row_height;

    // Track clicked byte offset for deferred cursor update
    let mut clicked_offset: Option<usize> = None;

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

                        let text = format!("{:02X}", byte);
                        let mut rich_text = RichText::new(text).monospace();

                        if is_cursor {
                            rich_text = rich_text.background_color(egui::Color32::from_rgb(60, 60, 120));
                        } else if is_selected {
                            rich_text = rich_text.background_color(egui::Color32::from_rgb(40, 80, 40));
                        }

                        // Make bytes clickable
                        let response = ui.label(rich_text);
                        if response.clicked() {
                            clicked_offset = Some(byte_offset);
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
            editor.set_cursor(offset);
            editor.clear_selection();
            app.preview_dirty = true;
        }
    }

    // Handle keyboard input
    ui.input(|i| {
        let Some(editor) = &mut app.editor else {
            return;
        };

        // Navigation
        if i.key_pressed(egui::Key::ArrowLeft) {
            editor.move_cursor(-1);
        }
        if i.key_pressed(egui::Key::ArrowRight) {
            editor.move_cursor(1);
        }
        if i.key_pressed(egui::Key::ArrowUp) {
            editor.move_cursor(-(BYTES_PER_ROW as isize));
        }
        if i.key_pressed(egui::Key::ArrowDown) {
            editor.move_cursor(BYTES_PER_ROW as isize);
        }
        if i.key_pressed(egui::Key::PageUp) {
            editor.move_cursor(-(BYTES_PER_ROW as isize * 16));
        }
        if i.key_pressed(egui::Key::PageDown) {
            editor.move_cursor(BYTES_PER_ROW as isize * 16);
        }
        if i.key_pressed(egui::Key::Home) {
            editor.set_cursor(0);
        }
        if i.key_pressed(egui::Key::End) {
            editor.set_cursor(editor.len().saturating_sub(1));
        }

        // Undo/Redo
        let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
        let shift = i.modifiers.shift;

        if ctrl && !shift && i.key_pressed(egui::Key::Z) {
            if editor.undo() {
                app.preview_dirty = true;
            }
        }
        if ctrl && shift && i.key_pressed(egui::Key::Z) {
            if editor.redo() {
                app.preview_dirty = true;
            }
        }

        // Hex input (0-9, A-F)
        for event in &i.events {
            if let egui::Event::Text(text) = event {
                for c in text.chars() {
                    if let Some(nibble) = c.to_digit(16) {
                        let cursor = editor.cursor();
                        if let Some(current) = editor.byte_at_cursor() {
                            // For simplicity, replace the entire byte
                            // TODO: Implement proper nibble editing
                            let new_value = (nibble as u8) << 4 | (current & 0x0F);
                            editor.edit_byte(cursor, new_value);
                            app.preview_dirty = true;
                        }
                    }
                }
            }
        }
    });
}
