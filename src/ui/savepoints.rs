//! Save points UI panel

use crate::app::{ConfirmAction, DocumentState, UiState};
use crate::ui::PointerCursor;
use eframe::egui::{self, RichText};

/// State for the save points panel
#[derive(Default)]
pub struct SavePointsPanelState {
    /// Whether we're currently editing a save point name
    editing_id: Option<u64>,

    /// Buffer for editing name
    edit_buffer: String,

    /// Buffer for new save point name
    new_name_buffer: String,

    /// Whether the create dialog is open
    show_create_dialog: bool,

    /// Pending create action
    pending_create: bool,
}

impl SavePointsPanelState {
    /// Whether this panel is in a text-entry state that should take
    /// keyboard priority (an in-progress rename or the create dialog) —
    /// e.g. the search dialog defers its Esc handling while this is true.
    pub fn wants_keyboard_priority(&self) -> bool {
        self.editing_id.is_some() || self.show_create_dialog
    }

    /// Abandon any in-progress rename or create-dialog state. Called when
    /// the panel's sidebar section collapses (its own key handlers stop
    /// running), so `wants_keyboard_priority` can't latch true forever.
    pub fn cancel_edits(&mut self) {
        self.editing_id = None;
        self.show_create_dialog = false;
        self.new_name_buffer.clear();
    }
}

/// Show the save points panel
pub fn show(
    ui: &mut egui::Ui,
    doc: &mut DocumentState,
    ui_state: &mut UiState,
    state: &mut SavePointsPanelState,
) {
    // Get save point count for UI (need to read before mutable access)
    let save_point_count = doc
        .editor
        .as_ref()
        .map(|e| e.save_point_count())
        .unwrap_or(0);

    // Get save points to display (need to clone for borrow checker)
    let save_points: Vec<_> = doc
        .editor
        .as_ref()
        .map(|e| {
            e.save_points()
                .iter()
                .map(|sp| (sp.id, sp.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    let has_editor = doc.editor.is_some();

    if !has_editor {
        ui.label(RichText::new("No file loaded").italics());
        return;
    }

    // Sidebar text editors read Esc non-consumingly; while a dialog or
    // menu that also handles Esc is stacked above, defer so one press
    // doesn't cancel both surfaces at once.
    let esc_pressed =
        !ui_state.overlay_wants_escape() && ui.input(|i| i.key_pressed(egui::Key::Escape));

    ui.horizontal(|ui| {
        if ui.button("➕ New").pointer_cursor().clicked() {
            state.show_create_dialog = true;
            state.new_name_buffer = format!("Save Point {}", save_point_count + 1);
            // Only one text editor at a time: an in-progress rename would
            // otherwise share raw Enter/Esc key presses with this form
            // (committing the abandoned rename buffer on Enter).
            state.editing_id = None;
        }
    });

    ui.separator();

    // Create save point dialog
    if state.show_create_dialog {
        ui.label("Name:");
        ui.text_edit_singleline(&mut state.new_name_buffer);
        // Esc cancels (like the rename editor below) — the search dialog
        // defers its own Esc handling while this form is open, so the form
        // must handle the press or Esc would do nothing at all.
        if esc_pressed {
            state.show_create_dialog = false;
            state.new_name_buffer.clear();
        }
        ui.horizontal(|ui| {
            if ui.button("Create").pointer_cursor().clicked() {
                state.pending_create = true;
                state.show_create_dialog = false;
            }
            if ui.button("Cancel").pointer_cursor().clicked() {
                state.show_create_dialog = false;
                state.new_name_buffer.clear();
            }
        });
        ui.separator();
    }

    // Handle pending create action
    if state.pending_create {
        if let Some(editor) = &mut doc.editor {
            editor.create_save_point(state.new_name_buffer.clone());
            state.new_name_buffer.clear();
        }
        state.pending_create = false;
    }

    // Track actions to perform after the loop
    let mut action_start_rename: Option<(u64, String)> = None;
    let mut action_finish_rename: Option<(u64, String)> = None;

    if save_points.is_empty() {
        ui.label(RichText::new("No save points yet").italics());
        ui.label("Create a save point to capture the current state.");
    } else {
        for (id, name) in save_points.iter() {
            if state.editing_id == Some(*id) {
                // Editing mode: text box + save/discard buttons below
                ui.text_edit_singleline(&mut state.edit_buffer);
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action_finish_rename = Some((*id, state.edit_buffer.clone()));
                    state.editing_id = None;
                }
                if esc_pressed {
                    state.editing_id = None;
                }
                ui.horizontal(|ui| {
                    if ui.button("Rename").pointer_cursor().clicked() {
                        action_finish_rename = Some((*id, state.edit_buffer.clone()));
                        state.editing_id = None;
                    }
                    if ui.button("Cancel").pointer_cursor().clicked() {
                        state.editing_id = None;
                    }
                });
            } else {
                // Normal mode: label + action buttons in one row
                ui.horizontal(|ui| {
                    ui.label(name);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Delete button (any save point can be deleted)
                        if ui
                            .button("🗑")
                            .pointer_cursor()
                            .on_hover_text("Delete")
                            .clicked()
                        {
                            ui_state
                                .dialogs
                                .open_confirm(ConfirmAction::DeleteSavePoint(*id));
                        }

                        // Rename button
                        if ui
                            .button("✏")
                            .pointer_cursor()
                            .on_hover_text("Rename")
                            .clicked()
                        {
                            action_start_rename = Some((*id, name.clone()));
                        }

                        // Restore button
                        if ui
                            .button("↩")
                            .pointer_cursor()
                            .on_hover_text("Restore")
                            .clicked()
                        {
                            ui_state
                                .dialogs
                                .open_confirm(ConfirmAction::RestoreSavePoint(*id));
                        }
                    });
                });
            }

            ui.separator();
        }
    }

    // Perform deferred actions
    if let Some((id, name)) = action_start_rename {
        state.editing_id = Some(id);
        state.edit_buffer = name;
        // Mutual exclusion with the create form (see the New button).
        state.show_create_dialog = false;
    }

    if let Some((id, new_name)) = action_finish_rename {
        if let Some(editor) = &mut doc.editor {
            let _ = editor.rename_save_point(id, new_name); // #[must_use] result intentionally ignored — id came from save_points() iteration; rename is idempotent on missing ids
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_edits_clears_keyboard_priority() {
        // A collapsed sidebar section must not leave the panel's edit state
        // latched — a stuck wants_keyboard_priority() would permanently
        // defer the search dialog's keyboard shortcuts.
        let mut state = SavePointsPanelState {
            editing_id: Some(3),
            show_create_dialog: true,
            new_name_buffer: "half-typed".to_string(),
            ..Default::default()
        };
        assert!(state.wants_keyboard_priority());

        state.cancel_edits();

        assert!(!state.wants_keyboard_priority());
        assert!(state.new_name_buffer.is_empty());
    }
}
