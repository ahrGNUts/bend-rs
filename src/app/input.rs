use eframe::egui;

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
            if ctrl && i.key_pressed(egui::Key::E) && self.editor.is_some() {
                actions.export = true;
            }
            if ctrl && i.key_pressed(egui::Key::F) && self.editor.is_some() {
                actions.search = true;
            }
            if ctrl && i.key_pressed(egui::Key::G) && self.editor.is_some() {
                actions.go_to = true;
            }
            // Undo: Ctrl+Z / Cmd+Z
            if ctrl && !shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                actions.undo = true;
            }
            // Redo: Ctrl+Shift+Z / Cmd+Shift+Z (or Ctrl+Y on some platforms)
            if ctrl && shift && i.key_pressed(egui::Key::Z) && self.editor.is_some() {
                actions.redo = true;
            }
            if ctrl && i.key_pressed(egui::Key::Y) && self.editor.is_some() {
                actions.redo = true;
            }
            // Create save point: Ctrl+S / Cmd+S
            if ctrl && i.key_pressed(egui::Key::S) && self.editor.is_some() {
                actions.create_save_point = true;
            }
            // Add bookmark: Ctrl+D / Cmd+D
            if ctrl && i.key_pressed(egui::Key::D) && self.editor.is_some() {
                actions.add_bookmark = true;
            }
            // Refresh preview: Ctrl+R / Cmd+R
            if ctrl && i.key_pressed(egui::Key::R) && self.editor.is_some() {
                actions.refresh_preview = true;
            }
            // F1: Show keyboard shortcuts help
            if i.key_pressed(egui::Key::F1) {
                self.shortcuts_dialog_state.open_dialog();
            }
        });

        actions
    }
}
