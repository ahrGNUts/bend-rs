//! Search and replace dialog UI component

use crate::app::BendApp;
use crate::editor::search::{execute_search, parse_hex_replace, SearchMode};
use eframe::egui;

/// Show the search dialog (modal window)
pub fn show(ctx: &egui::Context, app: &mut BendApp) {
    if !app.search_state.dialog_open {
        return;
    }

    let mut close_dialog = false;
    let mut do_search = false;
    let mut do_replace_one = false;
    let mut do_replace_all = false;
    let mut do_next = false;
    let mut do_prev = false;

    egui::Window::new("Search & Replace")
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .show(ctx, |ui| {
            // Search mode selection
            ui.horizontal(|ui| {
                ui.label("Mode:");
                ui.selectable_value(&mut app.search_state.mode, SearchMode::Hex, "Hex");
                ui.selectable_value(&mut app.search_state.mode, SearchMode::Ascii, "ASCII");
            });

            ui.add_space(4.0);

            // Search field
            ui.horizontal(|ui| {
                ui.label("Find:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut app.search_state.query)
                        .hint_text(match app.search_state.mode {
                            SearchMode::Hex => "e.g., FF D8 FF or FF ?? FF",
                            SearchMode::Ascii => "Enter text to search",
                        })
                        .desired_width(250.0),
                );
                // Auto-focus the find field when dialog opens
                if app.search_state.just_opened {
                    response.request_focus();
                    app.search_state.just_opened = false;
                }
                // Search on Enter
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    do_search = true;
                }
            });

            // Replace field
            ui.horizontal(|ui| {
                ui.label("Replace:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.search_state.replace_with)
                        .hint_text(match app.search_state.mode {
                            SearchMode::Hex => "e.g., 00 00 00",
                            SearchMode::Ascii => "Replacement text",
                        })
                        .desired_width(250.0),
                );
            });

            ui.add_space(4.0);

            // Options
            ui.horizontal(|ui| {
                if app.search_state.mode == SearchMode::Ascii {
                    ui.checkbox(&mut app.search_state.case_sensitive, "Case sensitive");
                } else {
                    ui.label(
                        egui::RichText::new("Tip: Use ?? for wildcard bytes")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                }
            });

            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Next").clicked() {
                    do_next = true;
                }
                if ui.button("Previous").clicked() {
                    do_prev = true;
                }
            });

            ui.horizontal(|ui| {
                let has_matches = !app.search_state.matches.is_empty();
                let replace_enabled = has_matches && app.search_state.current_match.is_some();

                if ui
                    .add_enabled(replace_enabled, egui::Button::new("Replace"))
                    .clicked()
                {
                    do_replace_one = true;
                }
                if ui
                    .add_enabled(has_matches, egui::Button::new("Replace All"))
                    .clicked()
                {
                    do_replace_all = true;
                }
            });

            ui.add_space(8.0);

            // Results status
            if let Some(error) = &app.search_state.error {
                ui.colored_label(egui::Color32::RED, error);
            } else if !app.search_state.query.is_empty() {
                let match_count = app.search_state.matches.len();
                if match_count == 0 {
                    ui.label("No matches found");
                } else {
                    let current = app.search_state.current_match.map(|i| i + 1).unwrap_or(0);
                    ui.label(format!("Match {} of {}", current, match_count));
                }
            }

            ui.add_space(8.0);

            // Close button
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close_dialog = true;
                }
            });
        });

    // Handle actions after UI is done (to avoid borrow issues)
    if do_search {
        if let Some(editor) = &app.editor {
            execute_search(&mut app.search_state, editor.working());
            // Navigate to first match if found
            if let Some(offset) = app.search_state.current_match_offset() {
                if let Some(editor) = &mut app.editor {
                    editor.set_cursor(offset);
                }
                app.scroll_hex_to_offset(offset);
            }
        }
    }

    if do_next {
        app.search_state.next_match();
        if let Some(offset) = app.search_state.current_match_offset() {
            if let Some(editor) = &mut app.editor {
                editor.set_cursor(offset);
            }
            app.scroll_hex_to_offset(offset);
        }
    }

    if do_prev {
        app.search_state.prev_match();
        if let Some(offset) = app.search_state.current_match_offset() {
            if let Some(editor) = &mut app.editor {
                editor.set_cursor(offset);
            }
            app.scroll_hex_to_offset(offset);
        }
    }

    if do_replace_one {
        if let Err(e) = replace_current(app) {
            app.search_state.error = Some(e);
        } else {
            // Re-execute search to update matches after replacement
            if let Some(editor) = &app.editor {
                execute_search(&mut app.search_state, editor.working());
            }
        }
    }

    if do_replace_all {
        match replace_all(app) {
            Ok(_count) => {
                // Re-execute search (should find nothing now)
                if let Some(editor) = &app.editor {
                    execute_search(&mut app.search_state, editor.working());
                }
            }
            Err(e) => {
                app.search_state.error = Some(e);
            }
        }
    }

    if close_dialog {
        app.search_state.close_dialog();
    }
}

/// Replace the current match
fn replace_current(app: &mut BendApp) -> Result<(), String> {
    let current_offset = app
        .search_state
        .current_match_offset()
        .ok_or("No current match")?;

    let pattern_len = app.search_state.pattern_length();
    if pattern_len == 0 {
        return Err("Invalid search pattern".to_string());
    }

    let replacement = get_replacement_bytes(app)?;

    // Validate replacement length matches pattern for hex mode
    if app.search_state.mode == SearchMode::Hex && replacement.len() != pattern_len {
        return Err(format!(
            "Replace pattern length ({}) must match search pattern length ({})",
            replacement.len(),
            pattern_len
        ));
    }

    let editor = app.editor.as_mut().ok_or("No file loaded")?;

    // Apply the replacement as a single edit operation
    for (i, &byte) in replacement.iter().enumerate() {
        let offset = current_offset + i;
        if offset < editor.len() {
            editor.edit_byte(offset, byte);
        }
    }

    Ok(())
}

/// Replace all matches as a single undoable operation
fn replace_all(app: &mut BendApp) -> Result<usize, String> {
    if app.search_state.matches.is_empty() {
        return Ok(0);
    }

    let pattern_len = app.search_state.pattern_length();
    if pattern_len == 0 {
        return Err("Invalid search pattern".to_string());
    }

    let replacement = get_replacement_bytes(app)?;

    // Validate replacement length matches pattern for hex mode
    if app.search_state.mode == SearchMode::Hex && replacement.len() != pattern_len {
        return Err(format!(
            "Replace pattern length ({}) must match search pattern length ({})",
            replacement.len(),
            pattern_len
        ));
    }

    let editor = app.editor.as_mut().ok_or("No file loaded")?;
    let count = app.search_state.matches.len();

    // Apply all replacements
    // Since we require replacement to be same length, positions don't shift
    // Note: We can borrow app.search_state.matches while editor is mutably borrowed
    // because they are separate fields (split borrowing)
    for &match_offset in &app.search_state.matches {
        for (i, &byte) in replacement.iter().enumerate() {
            let offset = match_offset + i;
            if offset < editor.len() {
                editor.edit_byte(offset, byte);
            }
        }
    }

    Ok(count)
}

/// Get replacement bytes based on current mode
fn get_replacement_bytes(app: &BendApp) -> Result<Vec<u8>, String> {
    match app.search_state.mode {
        SearchMode::Hex => parse_hex_replace(&app.search_state.replace_with),
        SearchMode::Ascii => Ok(app.search_state.replace_with.as_bytes().to_vec()),
    }
}
