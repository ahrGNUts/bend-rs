//! Save points UI panel

use crate::app::BendApp;
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

/// Show the save points panel
pub fn show(ui: &mut egui::Ui, app: &mut BendApp, state: &mut SavePointsPanelState) {
    // Get save point count for UI (need to read before mutable access)
    let save_point_count = app
        .editor
        .as_ref()
        .map(|e| e.save_point_count())
        .unwrap_or(0);

    // Get save points to display (need to clone for borrow checker)
    let save_points: Vec<_> = app
        .editor
        .as_ref()
        .map(|e| {
            e.save_points()
                .iter()
                .map(|sp| (sp.id, sp.name.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Get which save points can be deleted
    let can_delete: Vec<_> = save_points
        .iter()
        .map(|(id, _)| {
            app.editor
                .as_ref()
                .map(|e| e.can_delete_save_point(*id))
                .unwrap_or(false)
        })
        .collect();

    let has_editor = app.editor.is_some();

    if !has_editor {
        ui.label(RichText::new("No file loaded").italics());
        return;
    }

    ui.horizontal(|ui| {
        if ui.button("➕ New").clicked() {
            state.show_create_dialog = true;
            state.new_name_buffer = format!("Save Point {}", save_point_count + 1);
        }
    });

    ui.separator();

    // Create save point dialog
    if state.show_create_dialog {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut state.new_name_buffer);
        });
        ui.horizontal(|ui| {
            if ui.button("Create").clicked() {
                state.pending_create = true;
                state.show_create_dialog = false;
            }
            if ui.button("Cancel").clicked() {
                state.show_create_dialog = false;
                state.new_name_buffer.clear();
            }
        });
        ui.separator();
    }

    // Handle pending create action
    if state.pending_create {
        if let Some(editor) = &mut app.editor {
            editor.create_save_point(state.new_name_buffer.clone());
            state.new_name_buffer.clear();
        }
        state.pending_create = false;
    }

    // Track actions to perform after the loop
    let mut action_restore: Option<u64> = None;
    let mut action_delete: Option<u64> = None;
    let mut action_start_rename: Option<(u64, String)> = None;
    let mut action_finish_rename: Option<(u64, String)> = None;

    if save_points.is_empty() {
        ui.label(RichText::new("No save points yet").italics());
        ui.label("Create a save point to capture the current state.");
    } else {
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (idx, (id, name)) in save_points.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Name (editable or label)
                        if state.editing_id == Some(*id) {
                            let response = ui.text_edit_singleline(&mut state.edit_buffer);
                            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                action_finish_rename = Some((*id, state.edit_buffer.clone()));
                                state.editing_id = None;
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                state.editing_id = None;
                            }
                        } else {
                            ui.label(name);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete button (only for leaf)
                            if can_delete.get(idx).copied().unwrap_or(false) {
                                if ui.button("🗑").on_hover_text("Delete").clicked() {
                                    action_delete = Some(*id);
                                }
                            }

                            // Rename button
                            if ui.button("✏").on_hover_text("Rename").clicked() {
                                action_start_rename = Some((*id, name.clone()));
                            }

                            // Restore button
                            if ui.button("↩").on_hover_text("Restore").clicked() {
                                action_restore = Some(*id);
                            }
                        });
                    });

                    ui.separator();
                }
            });
    }

    // Perform deferred actions
    if let Some(id) = action_restore {
        if let Some(editor) = &mut app.editor {
            if editor.restore_save_point(id) {
                app.preview_dirty = true;
            }
        }
    }

    if let Some(id) = action_delete {
        if let Some(editor) = &mut app.editor {
            editor.delete_save_point(id);
        }
    }

    if let Some((id, name)) = action_start_rename {
        state.editing_id = Some(id);
        state.edit_buffer = name;
    }

    if let Some((id, new_name)) = action_finish_rename {
        if let Some(editor) = &mut app.editor {
            editor.rename_save_point(id, new_name);
        }
    }
}
