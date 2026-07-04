//! Search and replace dialog UI component

use crate::app::BendApp;
use crate::editor::search::{parse_hex_pattern, parse_hex_replace, SearchMessage, SearchMode};
use crate::ui::PointerCursor;
use eframe::egui;

/// Show the search dialog (modal window)
pub fn show(ctx: &egui::Context, app: &mut BendApp) {
    if !app.ui.search_state.dialog_open {
        return;
    }

    let mut close_dialog = false;
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
                ui.selectable_value(&mut app.ui.search_state.mode, SearchMode::Hex, "Hex")
                    .pointer_cursor();
                ui.selectable_value(&mut app.ui.search_state.mode, SearchMode::Ascii, "ASCII")
                    .pointer_cursor();
            });

            ui.add_space(4.0);

            // Search field
            let mut find_lost_focus = false;
            ui.horizontal(|ui| {
                ui.label("Find:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut app.ui.search_state.query)
                        .hint_text(match app.ui.search_state.mode {
                            SearchMode::Hex => "e.g., FF D8 FF or FF ?? FF",
                            SearchMode::Ascii => "Enter text to search",
                        })
                        .desired_width(250.0),
                );
                find_lost_focus = response.lost_focus();
                // Auto-focus the find field when dialog opens
                if app.ui.search_state.just_opened {
                    response.request_focus();
                    app.ui.search_state.just_opened = false;
                }
                // Enter = next match, Shift+Enter = previous match.
                // do_search_next/do_search_prev handle the initial-run and
                // stale-refresh cases internally — no dispatch logic here.
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if ui.input(|i| i.modifiers.shift) {
                        do_prev = true;
                    } else {
                        do_next = true;
                    }
                    // Keep focus in the Find field after Enter
                    response.request_focus();
                }
            });

            // Replace field
            let mut replace_lost_focus = false;
            ui.horizontal(|ui| {
                ui.label("Replace:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut app.ui.search_state.replace_with)
                        .hint_text(match app.ui.search_state.mode {
                            SearchMode::Hex => "e.g., 00 00 00",
                            SearchMode::Ascii => "Replacement text",
                        })
                        .desired_width(250.0),
                );
                replace_lost_focus = response.lost_focus();
            });

            // Live byte-length indicator for the Replace field. Authoritative
            // length validation still happens in `replace_current` — this is
            // just heads-up feedback so the user isn't surprised on click.
            render_length_indicator(ui, app);

            ui.add_space(4.0);

            // Options
            ui.horizontal(|ui| {
                if app.ui.search_state.mode == SearchMode::Ascii {
                    ui.checkbox(&mut app.ui.search_state.case_sensitive, "Case sensitive");
                } else {
                    ui.label(egui::RichText::new("Tip: Use ?? for wildcard bytes").small());
                }
            });

            ui.add_space(8.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui
                    .button("Next")
                    .on_hover_text("F3")
                    .pointer_cursor()
                    .clicked()
                {
                    do_next = true;
                }
                if ui
                    .button("Previous")
                    .on_hover_text("Shift+F3")
                    .pointer_cursor()
                    .clicked()
                {
                    do_prev = true;
                }
            });

            // Replace acts on the CACHED match list; when the query has
            // drifted since the last search the cached offsets/length no
            // longer describe what the user sees, so disable until they
            // re-search (keeps buttons, indicator, and validation coherent).
            let has_matches = !app.ui.search_state.matches.is_empty();
            let query_drifted = app.ui.search_state.query_changed_since_search();
            let replace_ready =
                has_matches && !query_drifted && app.ui.search_state.current_match.is_some();
            let replace_all_ready = has_matches && !query_drifted;

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(replace_ready, egui::Button::new("Replace"))
                    .on_hover_text("Alt+R or Ctrl/Cmd+Enter")
                    .on_disabled_hover_text("Press Enter to search first")
                    .pointer_cursor()
                    .clicked()
                {
                    do_replace_one = true;
                }
                if ui
                    .add_enabled(replace_all_ready, egui::Button::new("Replace All"))
                    .on_hover_text("Alt+A")
                    .on_disabled_hover_text("Press Enter to search first")
                    .pointer_cursor()
                    .clicked()
                {
                    do_replace_all = true;
                }
            });

            ui.add_space(8.0);

            // Results status
            render_status(ui, app);

            ui.add_space(8.0);

            // Close button
            ui.horizontal(|ui| {
                if ui
                    .button("Close")
                    .on_hover_text("Esc")
                    .pointer_cursor()
                    .clicked()
                {
                    close_dialog = true;
                }
            });

            // Dialog-scoped shortcuts. All defer while another dialog/menu is
            // stacked above this one (search renders first each frame, so
            // those flags still hold — deferring lets the stacked dialog take
            // the key). Gates must run BEFORE consume_key: it removes the
            // event from the queue even when the result is discarded.
            let modal_stacked = app.modal_above_search_open();
            // True while a text field (ours or anyone's) holds keyboard
            // focus. Blocks Alt-composed character entry (macOS Option+A,
            // Windows AltGr) from triggering replacements mid-typing.
            // consume_key(ALT, ..) itself rejects AltGr because AltGr
            // reports as Ctrl+Alt and the match is modifier-exact (it does
            // tolerate extra Shift).
            let typing_in_field = ui.ctx().wants_keyboard_input();

            if !modal_stacked
                && !typing_in_field
                && replace_ready
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::R))
            {
                do_replace_one = true;
            }
            if !modal_stacked
                && !typing_in_field
                && replace_all_ready
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::A))
            {
                do_replace_all = true;
            }
            // Ctrl/Cmd+Enter: replace current. Deliberately NOT gated on
            // typing_in_field — the chained workflow is pressing it from the
            // Find field, and egui's TextEdit only reacts to unmodified
            // Enter, so the event reaches us here.
            if !modal_stacked
                && replace_ready
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
            {
                do_replace_one = true;
            }
            // Esc: close — unless a stacked dialog should take it, or this
            // press was aimed at leaving a text field. egui clears widget
            // focus on Escape at frame START (before any dialog code runs),
            // so wants_keyboard_input() is useless here; the fields'
            // lost_focus() responses flag the unfocus-press frame instead.
            if !modal_stacked
                && !find_lost_focus
                && !replace_lost_focus
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                close_dialog = true;
            }
        });

    // Handle actions after UI is done (to avoid borrow issues)
    if do_next {
        // Route through the BendApp helper so protected matches are skipped
        // and Next/Previous behaves the same regardless of trigger (button,
        // F3, Enter) — including the initial-run and stale-refresh cases.
        app.do_search_next();
    }

    if do_prev {
        app.do_search_prev();
    }

    if do_replace_one {
        handle_replace_one(app);
    }

    if do_replace_all {
        handle_replace_all(app);
    }

    if close_dialog {
        app.ui.search_state.close_dialog();
    }
}

/// Dispatch a single-match replace, resyncing first if the buffer changed
/// since the last search. Replace must act on the offset the user is looking
/// at: after a stale refresh we re-anchor to the SAME offset (unlike
/// navigation, which advances past the old position) — and if that offset no
/// longer matches the pattern, nothing is written.
pub(crate) fn handle_replace_one(app: &mut BendApp) {
    let prev_offset = app.ui.search_state.current_match_offset();
    if app.refresh_search_if_stale() {
        let anchored = prev_offset.and_then(|off| {
            app.ui
                .search_state
                .matches
                .iter()
                .position(|&m| m == off)
                .map(|idx| (off, idx))
        });
        match anchored {
            Some((_off, idx)) => {
                // The same offset still matches — safe to replace it.
                app.ui.search_state.current_match = Some(idx);
            }
            None => {
                // The match under the cursor vanished (the user's own edit
                // changed it). Don't write anything; reposition nearby and
                // let them re-confirm against the refreshed results.
                if let Some(off) = prev_offset {
                    let pattern_len = app.ui.search_state.pattern_length();
                    let _ = app.ui.search_state.select_visible_after_offset(off, |o| {
                        app.doc.is_range_protected(o, pattern_len)
                    });
                    app.navigate_to_search_match();
                }
                app.ui.search_state.message = Some(SearchMessage::Info(
                    "Buffer changed — results refreshed; press Replace again".to_string(),
                ));
                return;
            }
        }
    }

    match replace_current(app) {
        Ok(ReplaceOutcome::Replaced { offset }) => {
            app.refresh_search();
            // Advance current_match to the next visible match strictly
            // after the replaced offset (wrapping). Avoids snapping back
            // to match 0, which is usually inside a protected header
            // when Protect Headers is on — that caused a ping-pong
            // "skip / replace" cycle on every Replace press.
            if !app.ui.search_state.matches.is_empty() {
                let pattern_len = app.ui.search_state.pattern_length();
                let _ = app
                    .ui
                    .search_state
                    .select_visible_after_offset(offset, |off| {
                        app.doc.is_range_protected(off, pattern_len)
                    });
                app.navigate_to_search_match();
            }
            if app.ui.search_state.message.is_none() {
                app.ui.search_state.message =
                    Some(SearchMessage::Info(format!("Replaced at 0x{:08X}", offset)));
            }
        }
        Ok(ReplaceOutcome::SkippedToUnprotected { skipped_offset }) => {
            // current_match and navigation already updated inside replace_current.
            let total = app.ui.search_state.matches.len();
            let current = app
                .ui
                .search_state
                .current_match
                .map(|i| i + 1)
                .unwrap_or(0);
            app.ui.search_state.message = Some(SearchMessage::Info(format!(
                "Skipped protected match at 0x{:08X} — now on match {} of {}",
                skipped_offset, current, total
            )));
        }
        Err(e) => {
            app.ui.search_state.message = Some(SearchMessage::Error(e));
        }
    }
}

/// Dispatch Replace All, resyncing against buffer edits first so every
/// write targets an offset that still matches the pattern.
pub(crate) fn handle_replace_all(app: &mut BendApp) {
    if app.refresh_search_if_stale() && app.ui.search_state.matches.is_empty() {
        app.ui.search_state.message = Some(SearchMessage::Info(
            "Buffer changed — no matches remain".to_string(),
        ));
        return;
    }

    match replace_all(app) {
        Ok(_count) => {
            // Save informational message before re-search clears it
            let info_message = app.ui.search_state.message.take();
            app.refresh_search();
            // Restore informational message if re-search didn't produce a new error
            if app.ui.search_state.message.is_none() {
                app.ui.search_state.message = info_message;
            }
        }
        Err(e) => {
            app.ui.search_state.message = Some(SearchMessage::Error(e));
        }
    }
}

/// What the Replace-field byte-length indicator should display.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReplaceIndicator {
    /// Replacement parses and matches the expected length.
    Match(usize),
    /// Replacement parses but its length differs from the pattern's.
    Mismatch { actual: usize, expected: usize },
    /// Replacement is not valid hex (hex mode only).
    InvalidHex { expected: usize },
}

/// Compute the Replace-field indicator from the LIVE query and replacement
/// text — not the cached executed-search pattern length, which goes stale
/// the moment the user edits the query. Returns `None` (indicator hidden)
/// when either field is empty or the live query itself doesn't parse.
pub(crate) fn compute_replace_indicator(
    mode: &SearchMode,
    query: &str,
    replace_with: &str,
) -> Option<ReplaceIndicator> {
    if query.is_empty() || replace_with.is_empty() {
        return None;
    }
    let expected = match mode {
        // An invalid live query has no meaningful expected length — hide
        // the indicator rather than validate against garbage.
        SearchMode::Hex => parse_hex_pattern(query).ok()?.len(),
        SearchMode::Ascii => query.len(),
    };
    if expected == 0 {
        return None;
    }
    let actual = match mode {
        SearchMode::Hex => match parse_hex_replace(replace_with) {
            Ok(bytes) => bytes.len(),
            // Distinguish "invalid hex" from "zero bytes" — reporting an
            // unparseable field as "0 bytes" sends the user padding with
            // more bytes instead of fixing the malformed one.
            Err(_) => return Some(ReplaceIndicator::InvalidHex { expected }),
        },
        SearchMode::Ascii => replace_with.len(),
    };
    if actual == expected {
        Some(ReplaceIndicator::Match(actual))
    } else {
        Some(ReplaceIndicator::Mismatch { actual, expected })
    }
}

/// Render the bytes-used / bytes-expected indicator below the Replace field.
fn render_length_indicator(ui: &mut egui::Ui, app: &BendApp) {
    let state = &app.ui.search_state;
    let Some(indicator) = compute_replace_indicator(&state.mode, &state.query, &state.replace_with)
    else {
        return;
    };
    let (text, color) = match indicator {
        ReplaceIndicator::Match(n) => (
            format!("{} / {} bytes", n, n),
            ui.visuals().weak_text_color(),
        ),
        ReplaceIndicator::Mismatch { actual, expected } => (
            format!("{} / {} bytes — must match search length", actual, expected),
            app.ui.colors.error_text,
        ),
        ReplaceIndicator::InvalidHex { expected } => (
            format!("invalid hex — {} bytes expected", expected),
            app.ui.colors.error_text,
        ),
    };
    ui.horizontal(|ui| {
        // Indent under the Replace field for visual alignment with the text edit.
        ui.add_space(56.0);
        ui.label(egui::RichText::new(text).color(color).small());
    });
}

/// Render the status area below the action buttons: a transient message (if
/// any) AND the live match counter — the message must not mask the counter.
fn render_status(ui: &mut egui::Ui, app: &BendApp) {
    if let Some(msg) = &app.ui.search_state.message {
        let colors = app.ui.colors;
        match msg {
            SearchMessage::Error(text) => {
                ui.colored_label(colors.error_text, text);
            }
            SearchMessage::Info(text) => {
                ui.colored_label(colors.warning_text, text);
            }
        }
    }
    if app.ui.search_state.query.is_empty() {
        return;
    }
    // Stale = the query/mode/case drifted since the search, OR the buffer
    // was edited since (generation mismatch) — both invalidate the results,
    // including a zero-hit result (an edit can create bytes the query would
    // now match).
    let buffer_stale = app.doc.editor.as_ref().is_some_and(|e| {
        app.ui
            .search_state
            .matches_may_be_stale(e.edit_generation())
    });
    let match_count = app.ui.search_state.matches.len();
    if match_count == 0 {
        // Distinguish "no search has been run for this query yet" from
        // "search ran and genuinely returned zero matches". Without this,
        // typing a query that *would* match shows the misleading
        // "No matches found" before Enter/Next/F3 has fired.
        if app.ui.search_state.query_changed_since_search() {
            ui.weak("Press Enter, F3, or Next to search");
        } else if app.ui.search_state.message.is_none() {
            if buffer_stale {
                ui.weak("No matches found (stale — press F3)");
            } else {
                ui.label("No matches found");
            }
        }
        return;
    }
    // No selected match (e.g. every match protected): show the total
    // rather than an impossible "Match 0 of N".
    let base = match app.ui.search_state.current_match {
        Some(i) => format!("Match {} of {}", i + 1, match_count),
        None => format!("{} matches", match_count),
    };
    if app.ui.search_state.query_changed_since_search() || buffer_stale {
        ui.weak(format!("{} (stale — press F3)", base));
    } else {
        ui.label(base);
    }
}

/// Outcome of a successful `replace_current` call. The dialog body uses this
/// to decide whether to refresh the search and surface a success message, or
/// just navigate to the next unprotected match.
#[derive(Debug)]
enum ReplaceOutcome {
    /// A byte replacement was performed at `offset`.
    Replaced { offset: usize },
    /// The current match was protected; navigation moved to the next
    /// unprotected match instead. `skipped_offset` is the offset of the
    /// protected match that was passed over.
    SkippedToUnprotected { skipped_offset: usize },
}

/// Replace the current match. When the current match is inside a protected
/// header region, advance `current_match` to the next unprotected match
/// instead of erroring — this matches the polished behavior of Replace All
/// and prevents the user from getting stranded on a non-replaceable match.
fn replace_current(app: &mut BendApp) -> Result<ReplaceOutcome, String> {
    let current_offset = app
        .ui
        .search_state
        .current_match_offset()
        .ok_or("No current match")?;

    let pattern_len = app.ui.search_state.pattern_length();
    if pattern_len == 0 {
        return Err("Invalid search pattern".to_string());
    }

    // Validate the replacement FIRST so an invalid Replace field always
    // errors immediately — even when the current match is protected. (The
    // protected-skip path below must not mask parse/length errors.)
    let replacement = get_replacement_bytes(app)?;
    if replacement.len() != pattern_len {
        return Err(format!(
            "Replace pattern length ({}) must match search pattern length ({})",
            replacement.len(),
            pattern_len
        ));
    }

    // If the current match sits in a protected region, navigate to the next
    // unprotected match (wrapping) and return without performing the replace.
    if app.doc.is_range_protected(current_offset, pattern_len) {
        let next_idx = app
            .ui
            .search_state
            .find_next_unprotected_match(|off| app.doc.is_range_protected(off, pattern_len));
        return match next_idx {
            Some(idx) => {
                app.ui.search_state.current_match = Some(idx);
                app.navigate_to_search_match();
                Ok(ReplaceOutcome::SkippedToUnprotected {
                    skipped_offset: current_offset,
                })
            }
            None => Err("All matches are in protected header regions".to_string()),
        };
    }

    let editor = app.doc.editor.as_mut().ok_or("No file loaded")?;

    // The match list is refreshed before replacing, so an out-of-range offset
    // here means something desynced badly — fail cleanly rather than letting
    // replace_bytes silently no-op (or, before its guard existed, panic).
    if current_offset + pattern_len > editor.working().len() {
        return Err(format!(
            "Match at 0x{:08X} is out of range — the buffer changed; search again",
            current_offset
        ));
    }

    // Apply the replacement as a single undoable operation
    editor.replace_bytes(current_offset, &replacement);

    Ok(ReplaceOutcome::Replaced {
        offset: current_offset,
    })
}

/// Replace all matches as a single undoable operation
fn replace_all(app: &mut BendApp) -> Result<usize, String> {
    if app.ui.search_state.matches.is_empty() {
        return Ok(0);
    }

    let pattern_len = app.ui.search_state.pattern_length();
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
        .ui
        .search_state
        .matches
        .iter()
        .partition(|&&offset| app.doc.is_range_protected(offset, pattern_len));

    if replaceable.is_empty() {
        return Err(format!(
            "All {} matches are in protected header regions",
            protected.len()
        ));
    }

    let editor = app.doc.editor.as_mut().ok_or("No file loaded")?;

    // Apply all replacements as a single atomic undo/redo operation
    // Since we require replacement to be same length, positions don't shift
    editor.replace_all_bytes(&replaceable, &replacement);

    let replaced_count = replaceable.len();
    let skipped_count = protected.len();

    if skipped_count > 0 {
        app.ui.search_state.message = Some(SearchMessage::Info(format!(
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
    match app.ui.search_state.mode {
        SearchMode::Hex => parse_hex_replace(&app.ui.search_state.replace_with),
        SearchMode::Ascii => Ok(app.ui.search_state.replace_with.as_bytes().to_vec()),
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
        app.doc.editor = Some(EditorState::new(data.to_vec()));
        app.doc.cached_sections = Some(sections);
        app.ui.search_state.mode = SearchMode::Hex;
        app.ui.search_state.query = query.to_string();
        app.ui.search_state.replace_with = replace.to_string();
        // Execute search to populate matches
        if let Some(editor) = &app.doc.editor {
            execute_search(&mut app.ui.search_state, editor.working());
        }
        app
    }

    #[test]
    fn test_replace_current_errors_when_only_match_is_protected() {
        // Data: 20 bytes, FF at offset 5 (only match, in High region)
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.doc.header_protection = true;
        app.ui.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected header region"));

        // Verify byte was NOT changed
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
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
        app.doc.header_protection = true;
        app.ui.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(matches!(
            result,
            Ok(ReplaceOutcome::Replaced { offset: 15 })
        ));

        // Verify byte WAS changed
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);
    }

    #[test]
    fn test_replace_current_errors_when_single_match_spans_boundary() {
        // Data: 20 bytes, "AA BB" at offset 9 (spans Safe at 9 and High at 10)
        let mut data = vec![0u8; 20];
        data[9] = 0xAA;
        data[10] = 0xBB;
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("High", 10, 20, RiskLevel::High),
        ];
        let mut app = setup_app(&data, sections, "AA BB", "00 00");
        app.doc.header_protection = true;
        app.ui.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("protected header region"));

        // Verify bytes NOT changed
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[9], 0xAA);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[10], 0xBB);
    }

    #[test]
    fn test_replace_current_skips_to_next_unprotected_match() {
        // Data: FF at offset 5 (High) and FF at offset 15 (Safe).
        // current_match starts at index 0 (protected). Replace should
        // advance current_match to index 1 without modifying any bytes.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        let mut app = setup_app(&data, sections, "FF", "00");
        app.doc.header_protection = true;
        app.ui.search_state.current_match = Some(0);

        let result = replace_current(&mut app);
        match result {
            Ok(ReplaceOutcome::SkippedToUnprotected { skipped_offset }) => {
                assert_eq!(skipped_offset, 5);
            }
            other => panic!("Expected SkippedToUnprotected, got {:?}", other),
        }

        // Navigated to the safe match.
        assert_eq!(app.ui.search_state.current_match, Some(1));

        // No bytes were modified — both matches retain their original values.
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0xFF);
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
        app.doc.header_protection = true;

        let result = replace_all(&mut app);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1); // Only 1 replaced

        // Protected byte unchanged
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
        // Safe byte replaced
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);

        // Informational message set (should be Info variant, not Error)
        match app.ui.search_state.message.as_ref().unwrap() {
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
        app.doc.header_protection = true;

        let result = replace_all(&mut app);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("All 1 matches are in protected"));

        // Byte unchanged
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
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

        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0x00);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);

        // No informational message
        assert!(app.ui.search_state.message.is_none());
    }

    #[test]
    fn test_search_needs_initial_run_states() {
        // The single initial-search predicate now lives on BendApp
        // (search_needs_initial_run) — the dialog and F3 both route
        // through it via do_search_next/do_search_prev.
        let mut app = BendApp::default();
        let mut buf = vec![0u8; 20];
        buf[5] = 0xAA;
        app.doc.editor = Some(EditorState::new(buf));

        // Empty query → never needs a search.
        assert!(!app.search_needs_initial_run());

        // Query typed, no search has run → needs a search.
        app.ui.search_state.query = "AA".to_string();
        assert!(app.search_needs_initial_run());

        // After a real search, the predicate settles.
        app.refresh_search();
        assert_eq!(app.ui.search_state.matches, vec![5]);
        assert!(!app.search_needs_initial_run());

        // Change query → drift → needs another search.
        app.ui.search_state.query = "BB".to_string();
        assert!(app.search_needs_initial_run());
    }

    #[test]
    fn test_replace_current_validates_replacement_before_protected_skip() {
        // A protected current match must NOT mask an invalid Replace field:
        // validation runs first, so the user learns about the bad
        // replacement immediately instead of one or more presses later.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let sections = vec![
            FileSection::new("Header", 0, 10, RiskLevel::High),
            FileSection::new("Data", 10, 20, RiskLevel::Safe),
        ];
        // Replacement "00 00" is 2 bytes for a 1-byte pattern → length error.
        let mut app = setup_app(&data, sections, "FF", "00 00");
        app.doc.header_protection = true;
        app.ui.search_state.current_match = Some(0); // protected match

        let result = replace_current(&mut app);
        let err = result.unwrap_err();
        assert!(
            err.contains("must match search pattern length"),
            "expected length error, got: {err}"
        );
        // No navigation happened — the protected-skip path never ran.
        assert_eq!(app.ui.search_state.current_match, Some(0));
        // Nothing was written.
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0xFF);
    }

    #[test]
    fn test_handle_replace_one_stale_reanchors_to_same_offset() {
        // Buffer edited elsewhere since the search: Replace must re-anchor to
        // the SAME offset the user is looking at (not advance) and then
        // replace it.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let mut app = setup_app(&data, vec![], "FF", "00");
        app.ui.search_state.current_match = Some(1); // offset 15

        // Edit an unrelated byte → generation bump → matches stale.
        app.doc.editor.as_mut().unwrap().edit_byte(0, 0x01);

        handle_replace_one(&mut app);

        // Offset 15 was replaced; offset 5 untouched.
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xFF);
        match app.ui.search_state.message.as_ref().unwrap() {
            SearchMessage::Info(msg) => assert!(msg.contains("Replaced at"), "got: {msg}"),
            other => panic!("expected Info, got {:?}", other),
        }
    }

    #[test]
    fn test_handle_replace_one_stale_target_vanished_writes_nothing() {
        // The user's own edit destroyed the match under the cursor: Replace
        // must not write anything — it refreshes, repositions, and asks for
        // re-confirmation.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let mut app = setup_app(&data, vec![], "FF", "00");
        app.ui.search_state.current_match = Some(0); // offset 5

        // Destroy the match at offset 5 directly in the editor.
        app.doc.editor.as_mut().unwrap().edit_byte(5, 0xAB);

        handle_replace_one(&mut app);

        // Nothing replaced: 5 holds the user's edit, 15 still the original.
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xAB);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0xFF);
        // Repositioned onto the surviving match.
        assert_eq!(app.ui.search_state.current_match_offset(), Some(15));
        match app.ui.search_state.message.as_ref().unwrap() {
            SearchMessage::Info(msg) => {
                assert!(msg.contains("Buffer changed"), "got: {msg}")
            }
            other => panic!("expected Info, got {:?}", other),
        }
    }

    #[test]
    fn test_handle_replace_all_stale_refresh_then_replaces_fresh_list() {
        // Replace All after buffer edits must act on refreshed offsets.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        data[15] = 0xFF;
        let mut app = setup_app(&data, vec![], "FF", "00");

        // Destroy one match; the other must still be replaced correctly.
        app.doc.editor.as_mut().unwrap().edit_byte(5, 0xAB);

        handle_replace_all(&mut app);

        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xAB);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);
    }

    #[test]
    fn test_handle_replace_all_stale_no_matches_remain() {
        // All matches destroyed by user edits → Replace All writes nothing
        // and says why.
        let mut data = vec![0u8; 20];
        data[5] = 0xFF;
        let mut app = setup_app(&data, vec![], "FF", "00");

        app.doc.editor.as_mut().unwrap().edit_byte(5, 0xAB);

        handle_replace_all(&mut app);

        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0xAB);
        match app.ui.search_state.message.as_ref().unwrap() {
            SearchMessage::Info(msg) => {
                assert!(msg.contains("no matches remain"), "got: {msg}")
            }
            other => panic!("expected Info, got {:?}", other),
        }
    }

    #[test]
    fn test_compute_replace_indicator_truth_table() {
        use super::ReplaceIndicator as RI;

        // Hidden: empty query or empty replacement.
        assert_eq!(compute_replace_indicator(&SearchMode::Hex, "", "00"), None);
        assert_eq!(compute_replace_indicator(&SearchMode::Hex, "FF", ""), None);

        // Hidden: live query itself is invalid hex — no meaningful expected
        // length to validate against.
        assert_eq!(
            compute_replace_indicator(&SearchMode::Hex, "GG", "00"),
            None
        );

        // Match: live query length, not the cached executed-search length.
        assert_eq!(
            compute_replace_indicator(&SearchMode::Hex, "FF D8", "AB CD"),
            Some(RI::Match(2))
        );

        // Mismatch reports live expected length.
        assert_eq!(
            compute_replace_indicator(&SearchMode::Hex, "FF D8 FF", "00"),
            Some(RI::Mismatch {
                actual: 1,
                expected: 3
            })
        );

        // Invalid hex replacement is flagged as such, not as "0 bytes".
        assert_eq!(
            compute_replace_indicator(&SearchMode::Hex, "FF D8", "GG"),
            Some(RI::InvalidHex { expected: 2 })
        );
        assert_eq!(
            compute_replace_indicator(&SearchMode::Hex, "FF D8", "AB C"),
            Some(RI::InvalidHex { expected: 2 })
        );

        // ASCII mode counts raw bytes of both fields.
        assert_eq!(
            compute_replace_indicator(&SearchMode::Ascii, "abc", "xyz"),
            Some(RI::Match(3))
        );
        assert_eq!(
            compute_replace_indicator(&SearchMode::Ascii, "abc", "xy"),
            Some(RI::Mismatch {
                actual: 2,
                expected: 3
            })
        );
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
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[5], 0x00);
        assert_eq!(app.doc.editor.as_ref().unwrap().working()[15], 0x00);

        // A single undo should revert ALL replacements
        let editor = app.doc.editor.as_mut().unwrap();
        assert!(editor.undo());
        assert_eq!(editor.working()[5], 0xFF);
        assert_eq!(editor.working()[15], 0xFF);

        // No more undo — it was a single atomic operation
        assert!(!editor.can_undo());
    }
}
