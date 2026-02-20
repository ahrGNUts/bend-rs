use eframe::egui;

use crate::editor::buffer::EditMode;

use super::BendApp;

/// Actions triggered by keyboard/mouse input, processed after input handling
#[derive(Default)]
pub(super) struct InputActions {
    pub open: bool,
    pub export: bool,
    pub search: bool,
    pub go_to: bool,
    pub undo: bool,
    pub redo: bool,
    pub create_save_point: bool,
    pub add_bookmark: bool,
    pub refresh_preview: bool,
    pub set_edit_mode: Option<EditMode>,
}

impl BendApp {
    /// Render the toolbar and return deferred action flags
    pub(super) fn render_toolbar(&mut self, ctx: &egui::Context) -> InputActions {
        let mut actions = InputActions::default();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let has_file = self.editor.is_some();
                let can_undo = self.editor.as_ref().is_some_and(|e| e.can_undo());
                let can_redo = self.editor.as_ref().is_some_and(|e| e.can_redo());

                // File operations
                if ui.button("Open").clicked() {
                    self.open_file_dialog();
                }
                if ui
                    .add_enabled(has_file, egui::Button::new("Export"))
                    .clicked()
                {
                    self.export_file();
                }

                ui.separator();

                // Undo/Redo
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo"))
                    .clicked()
                {
                    actions.undo = true;
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo"))
                    .clicked()
                {
                    actions.redo = true;
                }

                ui.separator();

                // Navigation/Search
                if ui
                    .add_enabled(has_file, egui::Button::new("Search"))
                    .clicked()
                {
                    self.search_state.open_dialog();
                }
                if ui
                    .add_enabled(has_file, egui::Button::new("Go to"))
                    .clicked()
                {
                    self.go_to_offset_state.open_dialog();
                }

                ui.separator();

                // View toggles
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(self.preview.comparison_mode, "Compare"),
                    )
                    .clicked()
                {
                    self.preview.comparison_mode = !self.preview.comparison_mode;
                }
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(self.header_protection, "Protect"),
                    )
                    .on_hover_text("Protect header regions from editing")
                    .clicked()
                {
                    self.header_protection = !self.header_protection;
                }

                ui.separator();

                // Edit mode selector
                let current_mode = self
                    .editor
                    .as_ref()
                    .map(|e| e.edit_mode())
                    .unwrap_or(EditMode::Hex);
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(current_mode == EditMode::Hex, "HEX"),
                    )
                    .clicked()
                {
                    actions.set_edit_mode = Some(EditMode::Hex);
                }
                if ui
                    .add_enabled(
                        has_file,
                        egui::SelectableLabel::new(current_mode == EditMode::Ascii, "ASCII"),
                    )
                    .clicked()
                {
                    actions.set_edit_mode = Some(EditMode::Ascii);
                }

                ui.separator();

                // Refresh preview
                if ui
                    .add_enabled(has_file, egui::Button::new("Refresh"))
                    .on_hover_text("Refresh preview (Ctrl+R / Cmd+R)")
                    .clicked()
                {
                    actions.refresh_preview = true;
                }
            });
        });

        actions
    }

    /// Process input actions (deferred to avoid borrow conflicts)
    pub(super) fn process_input_actions(&mut self, actions: InputActions) {
        if actions.open {
            self.open_file_dialog();
        }
        if actions.export {
            self.export_file();
        }
        if actions.search {
            self.search_state.open_dialog();
        }
        if actions.go_to {
            self.go_to_offset_state.open_dialog();
        }
        if actions.undo {
            self.do_undo();
        }
        if actions.redo {
            self.do_redo();
        }
        if actions.create_save_point {
            if let Some(editor) = &mut self.editor {
                let count = editor.save_points().len();
                let name = format!("Save Point {}", count + 1);
                editor.create_save_point(name);
            }
        }
        if actions.add_bookmark {
            if let Some(editor) = &mut self.editor {
                let cursor_pos = editor.cursor();
                let name = format!("Bookmark at 0x{:08X}", cursor_pos);
                editor.add_bookmark(cursor_pos, name);
            }
        }
        if actions.refresh_preview {
            self.mark_preview_dirty();
        }
        if let Some(mode) = actions.set_edit_mode {
            if let Some(editor) = &mut self.editor {
                editor.set_edit_mode(mode);
            }
        }
    }
}
