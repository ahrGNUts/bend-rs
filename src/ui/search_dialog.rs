//! Search and replace dialog UI component

use crate::app::BendApp;
use crate::editor::search::{execute_search, parse_hex_replace, SearchMessage, SearchMode};
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
    let mut navigate_to_last_after_search = false;

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
                // Enter = next match, Shift+Enter = previous match
                // If no matches yet or query changed, run the search first
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let shift_held = ui.input(|i| i.modifiers.shift);
                    let needs_search = app.search_state.matches.is_empty()
                        || app.search_state.query_changed_since_search();
                    if needs_search {
                        do_search = true;
                        if shift_held {
                            navigate_to_last_after_search = true;
                        }
                    } else if shift_held {
                        do_prev = true;
                    } else {
                        do_next = true;
                    }
                    // Keep focus in the Find field after Enter
                    response.request_focus();
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
            if let Some(msg) = &app.search_state.message {
                match msg {
                    SearchMessage::Error(text) => {
                        ui.colored_label(egui::Color32::RED, text);
                    }
                    SearchMessage::Info(text) => {
                        ui.colored_label(egui::Color32::YELLOW, text);
                    }
                }
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
            let gen = editor.edit_generation();
            execute_search(&mut app.search_state, editor.working());
            app.search_state.set_searched_generation(gen);
            // Navigate to last match if Shift+Enter was used on first search
            if navigate_to_last_after_search && !app.search_state.matches.is_empty() {
                app.search_state.current_match = Some(app.search_state.matches.len() - 1);
            }
            // Scroll to current match if found
            if let Some(offset) = app.search_state.current_match_offset() {
                if let Some(editor) = &mut app.editor {
                    editor.set_cursor(offset);
                }
                app.scroll_hex_to_offset(offset);
            }
        }
    }

    // Auto-re-search if buffer was edited since last search (stale matches)
    if (do_next || do_prev) && !do_search {
        if let Some(editor) = &app.editor {
            if app
                .search_state
                .matches_may_be_stale(editor.edit_generation())
            {
                let gen = editor.edit_generation();
                execute_search(&mut app.search_state, editor.working());
                app.search_state.set_searched_generation(gen);
                if app.search_state.matches.is_empty() {
                    do_next = false;
                    do_prev = false;
                }
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
        let prev_index = app.search_state.current_match.unwrap_or(0);
        if let Err(e) = replace_current(app) {
            app.search_state.message = Some(SearchMessage::Error(e));
        } else {
            // Re-execute search to update matches after replacement
            if let Some(editor) = &app.editor {
                let gen = editor.edit_generation();
                execute_search(&mut app.search_state, editor.working());
                app.search_state.set_searched_generation(gen);
            }
            // Restore match position (clamped to new matches length)
            if !app.search_state.matches.is_empty() {
                let clamped = prev_index.min(app.search_state.matches.len() - 1);
                app.search_state.current_match = Some(clamped);
                if let Some(offset) = app.search_state.current_match_offset() {
                    if let Some(editor) = &mut app.editor {
                        editor.set_cursor(offset);
                    }
                    app.scroll_hex_to_offset(offset);
                }
            }
        }
    }

    if do_replace_all {
        match replace_all(app) {
            Ok(_count) => {
                // Save informational message (e.g. "N skipped in protected regions")
                // before re-search, which clears message via clear_results()
                let info_message = app.search_state.message.take();

                // Re-execute search to update remaining matches
                if let Some(editor) = &app.editor {
                    let gen = editor.edit_generation();
                    execute_search(&mut app.search_state, editor.working());
                    app.search_state.set_searched_generation(gen);
                }

                // Restore informational message if re-search didn't produce a new error
                if app.search_state.message.is_none() {
                    app.search_state.message = info_message;
                }
            }
            Err(e) => {
                app.search_state.message = Some(SearchMessage::Error(e));
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

    // Validate replacement length matches pattern (fixed-size buffer requires same length)
    if replacement.len() != pattern_len {
        return Err(format!(
            "Replace pattern length ({}) must match search pattern length ({})",
            replacement.len(),
            pattern_len
        ));
    }

    // Check header protection
    if app.is_range_protected(current_offset, pattern_len) {
        return Err(format!(
            "Cannot replace: match at offset 0x{:08X} is in a protected header region",
            current_offset
        ));
    }

    let editor = app.editor.as_mut().ok_or("No file loaded")?;

    // Apply the replacement as a single undoable operation
    editor.replace_bytes(current_offset, &replacement);

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

    // Validate replacement length matches pattern (fixed-size buffer requires same length)
    if replacement.len() != pattern_len {
        return Err(format!(
            "Replace pattern length ({}) must match search pattern length ({})",
            replacement.len(),
            pattern_len
        ));
    }

    // Partition matches into protected vs replaceable
    let (protected, replaceable): (Vec<usize>, Vec<usize>) = app
        .search_state
        .matches
        .iter()
        .partition(|&&offset| app.is_range_protected(offset, pattern_len));

    if replaceable.is_empty() {
        return Err(format!(
            "All {} matches are in protected header regions",
            protected.len()
        ));
    }

    let editor = app.editor.as_mut().ok_or("No file loaded")?;

    // Apply all replacements as a single atomic undo/redo operation
    // Since we require replacement to be same length, positions don't shift
    editor.replace_all_bytes(&replaceable, &replacement);

    let replaced_count = replaceable.len();
    let skipped_count = protected.len();

    if skipped_count > 0 {
        app.search_state.message = Some(SearchMessage::Info(format!(
            "Replaced {} of {} matches ({} skipped in protected regions)",
            replaced_count,
            replaced_count + skipped_count,
            skipped_count
        )));
    }

    Ok(replaced_count)
}

/// Get replacement bytes based on current mode
fn get_replacement_bytes(app: &BendApp) -> Result<Vec<u8>, String> {
    match app.search_state.mode {
        SearchMode::Hex => parse_hex_replace(&app.search_state.replace_with),
        SearchMode::Ascii => Ok(app.search_state.replace_with.as_bytes().to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::buffer::EditorState;
    use crate::editor::search::{execute_search, SearchMessage, SearchMode};
    use crate::formats::traits::{FileSection, RiskLevel};

    /// Helper: create a BendApp with file data, sections, and a hex search pre-executed
    fn setup_app(data: &[u8], sections: Vec<FileSection>, query: &str, replace: &str) -> BendApp {
        let mut app = BendApp::default();
        app.editor = Some(EditorState::new(data.to_vec()));
        app.cached_sections = Some(sections);
        app.search_state.mode = SearchMode::Hex;
        app.search_state.query = query.to_string();
        app.search_state.replace_with = replace.to_string();
        // Execute search to populate matches
        if let Some(editor) = &app.editor {
            execute_search(&mut app.search_state, editor.working());
        }
        app
    }

    #[test]
    fn test_replace_current_blocked_in_protected_region() {
        // Data: 20 bytes, FF at offset 5 (in High region)
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.header_protection = true;
        app.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected header region"));

        // Verify byte was NOT changed
        assert_eq!(app.editor.as_ref().unwrap().working()[5], 0xFF);
    }

    #[test]
    fn test_replace_current_allowed_in_safe_region() {
        // Data: 20 bytes, FF at offset 15 (in Safe region)
        let mut data = vec![0u8; 20];
        data[15] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.header_protection = true;
        app.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(result.is_ok());

        // Verify byte WAS changed
        assert_eq!(app.editor.as_ref().unwrap().working()[15], 0x00);
    }

    #[test]
    fn test_replace_current_blocked_when_spanning_boundary() {
        // Data: 20 bytes, "AA BB" at offset 9 (spans Safe at 9 and High at 10)
        let mut data = vec![0u8; 20];
        data[9] = 0xAA;
        data[10] = 0xBB;
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("High", 10, 20, RiskLevel::High),
        ];
        let mut app = setup_app(&data, sections, "AA BB", "00 00");
        app.header_protection = true;
        app.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected header region"));

        // Verify bytes NOT changed
        assert_eq!(app.editor.as_ref().unwrap().working()[9], 0xAA);
        assert_eq!(app.editor.as_ref().unwrap().working()[10], 0xBB);
    }

    #[test]
    fn test_replace_all_skips_protected_replaces_safe() {
        // Data: FF at offset 5 (High) and FF at offset 15 (Safe)
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.header_protection = true;

        let result = replace_all(&mut app);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Only 1 replaced

        // Protected byte unchanged
        assert_eq!(app.editor.as_ref().unwrap().working()[5], 0xFF);
        // Safe byte replaced
        assert_eq!(app.editor.as_ref().unwrap().working()[15], 0x00);

        // Informational message set (should be Info variant, not Error)
        match app.search_state.message.as_ref().unwrap() {
            SearchMessage::Info(msg) => {
                assert!(msg.contains("1 skipped"));
                assert!(msg.contains("Replaced 1 of 2"));
            }
            SearchMessage::Error(_) => panic!("Expected Info, got Error"),
        }
    }

    #[test]
    fn test_replace_all_errors_when_all_protected() {
        // Data: FF at offset 5 (High), no safe matches
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.header_protection = true;

        let result = replace_all(&mut app);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("All 1 matches are in protected"));

        // Byte unchanged
        assert_eq!(app.editor.as_ref().unwrap().working()[5], 0xFF);
    }

    #[test]
    fn test_replace_all_replaces_everything_when_protection_disabled() {
        // Data: FF at offset 5 (High) and FF at offset 15 (Safe), protection OFF
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        // header_protection is false by default

        let result = replace_all(&mut app);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2); // Both replaced

        assert_eq!(app.editor.as_ref().unwrap().working()[5], 0x00);
        assert_eq!(app.editor.as_ref().unwrap().working()[15], 0x00);

        // No informational message
        assert!(app.search_state.message.is_none());
    }

    #[test]
    fn test_replace_all_atomic_undo() {
        // Data: FF at offsets 5 and 15, no protection
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let mut app = setup_app(&data, vec![], "FF", "00");

        // Replace all matches
        let result = replace_all(&mut app);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Both should be replaced
        assert_eq!(app.editor.as_ref().unwrap().working()[5], 0x00);
        assert_eq!(app.editor.as_ref().unwrap().working()[15], 0x00);

        // A single undo should revert ALL replacements
        let editor = app.editor.as_mut().unwrap();
        assert!(editor.undo());
        assert_eq!(editor.working()[5], 0xFF);
        assert_eq!(editor.working()[15], 0xFF);

        // No more undo — it was a single atomic operation
        assert!(!editor.can_undo());
    }
}
