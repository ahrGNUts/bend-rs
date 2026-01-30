//! Go to offset dialog UI component

use crate::app::BendApp;
use crate::editor::go_to_offset::parse_offset;
use eframe::egui;

/// Show the "Go to offset" dialog (modal window)
pub fn show(ctx: &egui::Context, app: &mut BendApp) {
    if !app.go_to_offset_state.dialog_open {
        return;
    }

    let mut close_dialog = false;
    let mut do_navigate = false;

    egui::Window::new("Go to Offset")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label("Enter offset (decimal or 0x hex):");

            ui.add_space(4.0);

            // Input field
            let response = ui.add(
                egui::TextEdit::singleline(&mut app.go_to_offset_state.input_text)
                    .hint_text("e.g., 1024 or 0x400")
                    .desired_width(200.0),
            );

            // Auto-focus the text field when dialog opens
            if response.gained_focus() || app.go_to_offset_state.input_text.is_empty() {
                response.request_focus();
            }

            // Navigate on Enter
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                do_navigate = true;
            }

            // Show file size hint if available
            if let Some(editor) = &app.editor {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "File size: {} bytes (0x{:X})",
                        editor.len(),
                        editor.len()
                    ))
                    .small()
                    .color(egui::Color32::GRAY),
                );
            }

            // Show error message if any
            if let Some(error) = &app.go_to_offset_state.error {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::RED, error);
            }

            ui.add_space(8.0);

            // Buttons
            ui.horizontal(|ui| {
                if ui.button("Go").clicked() {
                    do_navigate = true;
                }
                if ui.button("Cancel").clicked() {
                    close_dialog = true;
                }
            });

            // Handle Escape to close
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_dialog = true;
            }
        });

    // Handle navigation after UI scope ends (to avoid borrow issues)
    if do_navigate {
        match parse_offset(&app.go_to_offset_state.input_text) {
            Ok(offset) => {
                if let Some(editor) = &mut app.editor {
                    let file_len = editor.len();
                    if offset < file_len {
                        editor.set_cursor(offset);
                        close_dialog = true;
                    } else {
                        app.go_to_offset_state.error = Some(format!(
                            "Offset 0x{:X} ({}) is beyond file size (0x{:X} / {} bytes)",
                            offset, offset, file_len, file_len
                        ));
                    }
                } else {
                    app.go_to_offset_state.error = Some("No file loaded".to_string());
                }
            }
            Err(e) => {
                app.go_to_offset_state.error = Some(e);
            }
        }
    }

    if close_dialog {
        app.go_to_offset_state.close_dialog();
    }
}
