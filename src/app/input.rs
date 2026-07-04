use eframe::egui;

use crate::editor::buffer::EditMode;

use super::toolbar::InputActions;
use super::BendApp;

impl BendApp {
    /// Handle dropped files and keyboard shortcuts
    /// Returns flags for deferred actions
    pub(super) fn handle_input(&mut self, ctx: &egui::Context) -> InputActions {
        let mut actions = InputActions::default();

        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    self.open_file(path.clone());
                }
            }

            // Global keyboard shortcuts
            let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
            let shift = i.modifiers.shift;
            if ctrl && i.key_pressed(egui::Key::O) {
                actions.open = true;
            }
            if ctrl && i.key_pressed(egui::Key::E) && self.doc.editor.is_some() {
                actions.export = true;
            }
            if ctrl && i.key_pressed(egui::Key::F) && self.doc.editor.is_some() {
                actions.search = true;
            }
            if ctrl && i.key_pressed(egui::Key::G) && self.doc.editor.is_some() {
                actions.go_to = true;
            }
            // Undo: Ctrl+Z / Cmd+Z
            if ctrl && !shift && i.key_pressed(egui::Key::Z) && self.doc.editor.is_some() {
                actions.undo = true;
            }
            // Redo: Ctrl+Shift+Z / Cmd+Shift+Z (or Ctrl+Y on some platforms)
            if ctrl && shift && i.key_pressed(egui::Key::Z) && self.doc.editor.is_some() {
                actions.redo = true;
            }
            if ctrl && i.key_pressed(egui::Key::Y) && self.doc.editor.is_some() {
                actions.redo = true;
            }
            // Create save point: Ctrl+S / Cmd+S
            if ctrl && i.key_pressed(egui::Key::S) && self.doc.editor.is_some() {
                actions.create_save_point = true;
            }
            // Add bookmark: Ctrl+D / Cmd+D
            if ctrl && i.key_pressed(egui::Key::D) && self.doc.editor.is_some() {
                actions.add_bookmark = true;
            }
            // Refresh preview: Ctrl+R / Cmd+R
            if ctrl && i.key_pressed(egui::Key::R) && self.doc.editor.is_some() {
                actions.refresh_preview = true;
            }
            // Toggle edit mode: Ctrl+M / Cmd+M
            if ctrl && i.key_pressed(egui::Key::M) {
                if let Some(editor) = self.doc.editor.as_ref() {
                    actions.set_edit_mode = Some(match editor.edit_mode() {
                        EditMode::Hex => EditMode::Ascii,
                        EditMode::Ascii => EditMode::Hex,
                    });
                }
            }
            // F1: Show keyboard shortcuts help
            if i.key_pressed(egui::Key::F1) {
                self.ui.shortcuts_dialog_state.open();
            }
            // F3 / Shift+F3: Step search matches when the dialog is open.
            // Plain-key only — no Ctrl/Cmd/Alt — to avoid window-manager clashes
            // and to keep the binding decoupled from Find-field focus so the user
            // can hold F3 to scrub through matches. Defers while another dialog
            // is stacked above search, like every other search shortcut.
            // Known cost of the menu-open two-frame window: the first F3 press
            // right after dismissing a menu with Esc may be deferred once
            // (handle_input runs before render_menu_bar updates the flags);
            // the press itself triggers the repaint that clears them.
            if self.ui.search_state.dialog_open
                && !self.modal_above_search_open()
                && !ctrl
                && !i.modifiers.alt
            {
                if !shift && i.key_pressed(egui::Key::F3) {
                    actions.search_next = true;
                }
                if shift && i.key_pressed(egui::Key::F3) {
                    actions.search_prev = true;
                }
            }
        });

        actions
    }
}
