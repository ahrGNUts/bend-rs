use crate::formats::RiskLevel;
use crate::ui::PointerCursor;
use eframe::egui;

use super::BendApp;

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
    /// A destructive action awaiting user confirmation (delete/restore)
    pub pending_confirm: Option<ConfirmAction>,
    /// True for the first frame after `pending_confirm` is set, so the renderer
    /// can seed initial keyboard focus on the Cancel button.
    pub confirm_just_opened: bool,
}

/// A destructive action awaiting user confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteSavePoint(u64),
    RestoreSavePoint(u64),
    DeleteBookmark(u64),
    DeleteBookmarkAnnotation(u64),
}

impl DialogState {
    /// Queue a destructive action for confirmation and arm initial-focus seeding.
    /// Always set both fields together to keep them in sync.
    pub fn open_confirm(&mut self, action: ConfirmAction) {
        self.pending_confirm = Some(action);
        self.confirm_just_opened = true;
    }
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

impl BendApp {
    /// Show the high-risk edit warning dialog and handle user response
    pub(super) fn show_high_risk_warning_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.ui.dialogs.pending_high_risk_edit else {
            return;
        };

        let mut should_proceed = false;
        let mut should_cancel = false;

        let colors = self.ui.colors;
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
                                .color(colors.warning_text),
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
                        &mut self.ui.dialogs.high_risk_dont_show,
                        "Don't warn me again this session",
                    );

                    ui.add_space(10.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Proceed").pointer_cursor().clicked() {
                            should_proceed = true;
                        }
                        if ui.button("Cancel").pointer_cursor().clicked() {
                            should_cancel = true;
                        }
                    });
                });
            });

        // Handle user response
        if should_proceed {
            // Apply the edit based on type
            if let Some(editor) = &mut self.doc.editor {
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
            if self.ui.dialogs.high_risk_dont_show {
                self.ui.dialogs.suppress_high_risk_warnings = true;
                self.config.settings.show_high_risk_warnings = false;
                self.config.settings.save();
            }
            self.ui.dialogs.high_risk_dont_show = false;
            self.ui.dialogs.pending_high_risk_edit = None;
        } else if should_cancel {
            self.ui.dialogs.high_risk_dont_show = false;
            self.ui.dialogs.pending_high_risk_edit = None;
        }
    }

    /// Show a confirmation dialog for a pending destructive action
    /// (delete save point, restore save point, delete bookmark, delete bookmark note).
    pub(super) fn show_confirmation_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.ui.dialogs.pending_confirm else {
            return;
        };

        let (title, body_lines, confirm_label): (&str, &[&str], &str) = match pending {
            ConfirmAction::DeleteSavePoint(_) => (
                "Delete Save Point",
                &[
                    "This save point will be permanently removed.",
                    "Continue?",
                ],
                "Delete",
            ),
            ConfirmAction::RestoreSavePoint(_) => (
                "Restore Save Point",
                &[
                    "Restoring will replace the current working buffer with this save point's bytes. Any unsaved changes since this save point will be lost.",
                    "Continue?",
                ],
                "Restore",
            ),
            ConfirmAction::DeleteBookmark(_) => (
                "Delete Bookmark",
                &[
                    "This bookmark will be permanently removed.",
                    "Continue?",
                ],
                "Delete",
            ),
            ConfirmAction::DeleteBookmarkAnnotation(_) => (
                "Delete Note",
                &[
                    "The note on this bookmark will be cleared.",
                    "Continue?",
                ],
                "Delete Note",
            ),
        };

        let mut should_confirm = false;
        let mut should_cancel = false;

        let prev_focus = ctx.memory(|m| m.focused());
        let mut confirm_id_opt: Option<egui::Id> = None;
        let mut cancel_id_opt: Option<egui::Id> = None;

        let colors = self.ui.colors;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            egui::RichText::new("\u{26A0}")
                                .size(32.0)
                                .color(colors.warning_text),
                        );
                        ui.vertical(|ui| {
                            for line in body_lines {
                                ui.label(*line);
                            }
                        });
                    });

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        // Cancel rendered first (leftmost) so egui's natural Tab
                        // advance moves focus Cancel → Confirm without a flicker.
                        let cancel_resp = ui.button("Cancel").pointer_cursor();
                        cancel_id_opt = Some(cancel_resp.id);
                        if self.ui.dialogs.confirm_just_opened {
                            cancel_resp.request_focus();
                            self.ui.dialogs.confirm_just_opened = false;
                        }
                        if cancel_resp.clicked() {
                            should_cancel = true;
                        }

                        let confirm_resp = ui.button(confirm_label).pointer_cursor();
                        confirm_id_opt = Some(confirm_resp.id);
                        if confirm_resp.clicked() {
                            should_confirm = true;
                        }
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        should_cancel = true;
                    }
                });
            });

        // Focus trap: keep keyboard focus on one of the two buttons while the
        // dialog is open. egui's Tab/Shift+Tab cycle through every focusable
        // widget in the app — without this, pressing Tab from one of the
        // buttons would leak focus to a menu/sidebar widget.
        if let (Some(confirm_id), Some(cancel_id)) = (confirm_id_opt, cancel_id_opt) {
            let focused = ctx.memory(|m| m.focused());
            if focused != Some(confirm_id) && focused != Some(cancel_id) {
                // Tab from Cancel cycles to Confirm; any other drift (Tab
                // from Confirm leaking `give_to_next`, or initial open with
                // focus outside the dialog) snaps back to Cancel.
                let target = if prev_focus == Some(cancel_id) {
                    confirm_id
                } else {
                    cancel_id
                };
                ctx.memory_mut(|m| {
                    m.request_focus(target);
                    // Absorb any leaked `give_to_next` so it doesn't hand focus
                    // to a menu/sidebar button on the next frame.
                    m.interested_in_focus(target);
                });
            }
        }

        if should_confirm {
            self.apply_confirm_action(pending);
            self.ui.dialogs.pending_confirm = None;
        } else if should_cancel {
            self.ui.dialogs.pending_confirm = None;
        }
    }

    /// Apply a confirmed destructive action to the editor.
    /// Extracted from `show_confirmation_dialog` so it can be unit-tested
    /// without needing an egui `Context`.
    pub(crate) fn apply_confirm_action(&mut self, action: ConfirmAction) {
        let Some(editor) = &mut self.doc.editor else {
            return;
        };
        match action {
            ConfirmAction::DeleteSavePoint(id) => {
                let _ = editor.delete_save_point(id);
            }
            ConfirmAction::RestoreSavePoint(id) => {
                if editor.restore_save_point(id) {
                    self.doc.preview.mark_dirty();
                }
            }
            ConfirmAction::DeleteBookmark(id) => {
                let _ = editor.remove_bookmark(id);
            }
            ConfirmAction::DeleteBookmarkAnnotation(id) => {
                let _ = editor.bookmarks_mut().set_annotation(id, String::new());
            }
        }
    }

    /// Show the close confirmation dialog
    pub(super) fn show_close_dialog(&mut self, ctx: &egui::Context) {
        if !self.ui.dialogs.show_close {
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
                    if ui.button("Export First").pointer_cursor().clicked() {
                        self.export_file(ui.ctx());
                        self.ui.dialogs.show_close = false;
                    }
                    if ui.button("Discard & Exit").pointer_cursor().clicked() {
                        self.ui.dialogs.pending_close = true;
                        self.ui.dialogs.show_close = false;
                    }
                    if ui.button("Cancel").pointer_cursor().clicked() {
                        self.ui.dialogs.show_close = false;
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::EditorState;

    fn app_with_editor() -> BendApp {
        let mut app = BendApp::default();
        app.doc.editor = Some(EditorState::new(vec![0xAA; 16]));
        app
    }

    #[test]
    fn open_confirm_sets_pending_and_just_opened_atomically() {
        let mut state = DialogState::default();
        assert!(state.pending_confirm.is_none());
        assert!(!state.confirm_just_opened);

        state.open_confirm(ConfirmAction::DeleteSavePoint(7));

        assert_eq!(
            state.pending_confirm,
            Some(ConfirmAction::DeleteSavePoint(7))
        );
        assert!(state.confirm_just_opened);
    }

    #[test]
    fn apply_confirm_action_deletes_save_point() {
        let mut app = app_with_editor();
        let editor = app.doc.editor.as_mut().unwrap();
        let id = editor.create_save_point("sp".into());
        assert_eq!(editor.save_point_count(), 1);

        app.apply_confirm_action(ConfirmAction::DeleteSavePoint(id));

        assert_eq!(app.doc.editor.as_ref().unwrap().save_point_count(), 0);
    }

    #[test]
    fn apply_confirm_action_restore_save_point_marks_preview_dirty() {
        let mut app = app_with_editor();
        let editor = app.doc.editor.as_mut().unwrap();
        let id = editor.create_save_point("checkpoint".into());
        // Mutate the working buffer so restore has something to undo.
        let _ = editor.edit_byte(0, 0xFF);
        assert_eq!(editor.working()[0], 0xFF);
        app.doc.preview.dirty = false;

        app.apply_confirm_action(ConfirmAction::RestoreSavePoint(id));

        assert_eq!(app.doc.editor.as_ref().unwrap().working()[0], 0xAA);
        assert!(app.doc.preview.dirty);
    }

    #[test]
    fn apply_confirm_action_restore_unknown_id_does_not_mark_dirty() {
        let mut app = app_with_editor();
        app.doc.preview.dirty = false;

        app.apply_confirm_action(ConfirmAction::RestoreSavePoint(9_999));

        assert!(!app.doc.preview.dirty);
    }

    #[test]
    fn apply_confirm_action_deletes_bookmark() {
        let mut app = app_with_editor();
        let editor = app.doc.editor.as_mut().unwrap();
        let id = editor.add_bookmark(4, "bm".into());
        assert_eq!(editor.bookmarks().all().len(), 1);

        app.apply_confirm_action(ConfirmAction::DeleteBookmark(id));

        assert_eq!(app.doc.editor.as_ref().unwrap().bookmarks().all().len(), 0);
    }

    #[test]
    fn apply_confirm_action_clears_bookmark_annotation() {
        let mut app = app_with_editor();
        let editor = app.doc.editor.as_mut().unwrap();
        let id = editor.add_bookmark(4, "bm".into());
        let _ = editor.bookmarks_mut().set_annotation(id, "note".into());
        assert_eq!(editor.bookmarks().all()[0].annotation, "note");

        app.apply_confirm_action(ConfirmAction::DeleteBookmarkAnnotation(id));

        let bookmarks = app.doc.editor.as_ref().unwrap().bookmarks().all();
        assert_eq!(bookmarks[0].annotation, "");
        assert_eq!(bookmarks.len(), 1, "bookmark itself should remain");
    }

    #[test]
    fn apply_confirm_action_without_editor_is_noop() {
        let mut app = BendApp::default();
        assert!(app.doc.editor.is_none());

        // Must not panic — branch should fall through.
        app.apply_confirm_action(ConfirmAction::DeleteSavePoint(1));
        app.apply_confirm_action(ConfirmAction::RestoreSavePoint(1));
        app.apply_confirm_action(ConfirmAction::DeleteBookmark(1));
        app.apply_confirm_action(ConfirmAction::DeleteBookmarkAnnotation(1));
    }
}
