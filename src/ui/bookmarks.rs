//! Bookmarks list UI component

use crate::app::BendApp;
use eframe::egui;

/// State for the bookmarks panel
#[derive(Default)]
pub struct BookmarksPanelState {
    /// Bookmark ID currently being renamed (if any)
    pub renaming: Option<u64>,
    /// Text for renaming
    pub rename_text: String,
    /// Bookmark ID being edited for annotation (if any)
    pub editing_annotation: Option<u64>,
    /// Text for annotation editing
    pub annotation_text: String,
}

/// Show the bookmarks panel
pub fn show(ui: &mut egui::Ui, app: &mut BendApp, state: &mut BookmarksPanelState) {
    let Some(editor) = &app.editor else {
        ui.label("No file loaded");
        return;
    };

    // Get cursor position before borrowing mutably
    let cursor_pos = editor.cursor();
    let bookmarks: Vec<_> = editor.bookmarks().all().to_vec();

    // Add bookmark button
    ui.horizontal(|ui| {
        if ui.button("+ Add Bookmark").clicked() {
            let name = format!("Bookmark at 0x{:08X}", cursor_pos);
            if let Some(editor) = &mut app.editor {
                editor.add_bookmark(cursor_pos, name);
            }
        }
    });

    ui.separator();

    if bookmarks.is_empty() {
        ui.label("No bookmarks yet");
        ui.label(
            egui::RichText::new("Click \"+ Add Bookmark\" to add one at the current cursor position")
                .small()
                .color(egui::Color32::GRAY),
        );
        return;
    }

    // List bookmarks
    let mut action: Option<BookmarkAction> = None;

    for bookmark in bookmarks {
        ui.push_id(bookmark.id, |ui| {
            ui.group(|ui| {
                    // Bookmark name (editable if renaming)
                    if state.renaming == Some(bookmark.id) {
                        ui.horizontal(|ui| {
                            let response = ui.text_edit_singleline(&mut state.rename_text);
                            if response.lost_focus() {
                                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    action = Some(BookmarkAction::FinishRename(
                                        bookmark.id,
                                        std::mem::take(&mut state.rename_text),
                                    ));
                                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    action = Some(BookmarkAction::CancelRename);
                                }
                            }
                            if ui.button("Save").clicked() {
                                action = Some(BookmarkAction::FinishRename(
                                    bookmark.id,
                                    std::mem::take(&mut state.rename_text),
                                ));
                            }
                            if ui.button("Cancel").clicked() {
                                action = Some(BookmarkAction::CancelRename);
                            }
                        });
                    } else {
                        // Normal display mode
                        ui.horizontal(|ui| {
                            // Clickable bookmark name
                            if ui
                                .link(&bookmark.name)
                                .on_hover_text("Click to navigate")
                                .clicked()
                            {
                                action = Some(BookmarkAction::Navigate(bookmark.offset));
                            }
                        });
                    }

                    // Offset display
                    ui.label(
                        egui::RichText::new(format!("Offset: 0x{:08X}", bookmark.offset))
                            .small()
                            .color(egui::Color32::GRAY),
                    );

                    // Annotation (editable if editing)
                    if state.editing_annotation == Some(bookmark.id) {
                        ui.horizontal(|ui| {
                            ui.label("Note:");
                            let response = ui.text_edit_singleline(&mut state.annotation_text);
                            if response.lost_focus() {
                                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    action = Some(BookmarkAction::FinishAnnotation(
                                        bookmark.id,
                                        std::mem::take(&mut state.annotation_text),
                                    ));
                                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    action = Some(BookmarkAction::CancelAnnotation);
                                }
                            }
                            if ui.button("Save").clicked() {
                                action = Some(BookmarkAction::FinishAnnotation(
                                    bookmark.id,
                                    std::mem::take(&mut state.annotation_text),
                                ));
                            }
                        });
                    } else if !bookmark.annotation.is_empty() {
                        ui.label(
                            egui::RichText::new(&bookmark.annotation)
                                .small()
                                .italics()
                                .color(egui::Color32::LIGHT_GRAY),
                        );
                    }

                    // Action buttons
                    ui.horizontal(|ui| {
                        if state.renaming.is_none() && state.editing_annotation.is_none() {
                            if ui.small_button("Rename").clicked() {
                                action = Some(BookmarkAction::StartRename(
                                    bookmark.id,
                                    bookmark.name.clone(),
                                ));
                            }
                            if ui.small_button("Add Note").clicked() {
                                action = Some(BookmarkAction::StartAnnotation(
                                    bookmark.id,
                                    bookmark.annotation.clone(),
                                ));
                            }
                            if ui.small_button("Delete").clicked() {
                                action = Some(BookmarkAction::Delete(bookmark.id));
                            }
                        }
                    });
            });
        });

        ui.add_space(4.0);
    }

    // Process actions
    if let Some(action) = action {
        match action {
            BookmarkAction::Navigate(offset) => {
                if let Some(editor) = &mut app.editor {
                    editor.set_cursor(offset);
                }
                app.scroll_hex_to_offset(offset);
            }
            BookmarkAction::StartRename(id, name) => {
                state.renaming = Some(id);
                state.rename_text = name;
            }
            BookmarkAction::FinishRename(id, name) => {
                if let Some(editor) = &mut app.editor {
                    let _ = editor.bookmarks_mut().rename(id, name); // #[must_use] result intentionally ignored — bookmark existence already verified by UI
                }
                state.renaming = None;
                state.rename_text.clear();
            }
            BookmarkAction::CancelRename => {
                state.renaming = None;
                state.rename_text.clear();
            }
            BookmarkAction::StartAnnotation(id, annotation) => {
                state.editing_annotation = Some(id);
                state.annotation_text = annotation;
            }
            BookmarkAction::FinishAnnotation(id, annotation) => {
                if let Some(editor) = &mut app.editor {
                    let _ = editor.bookmarks_mut().set_annotation(id, annotation); // #[must_use] result intentionally ignored — bookmark existence already verified by UI
                }
                state.editing_annotation = None;
                state.annotation_text.clear();
            }
            BookmarkAction::CancelAnnotation => {
                state.editing_annotation = None;
                state.annotation_text.clear();
            }
            BookmarkAction::Delete(id) => {
                if let Some(editor) = &mut app.editor {
                    let _ = editor.remove_bookmark(id); // #[must_use] result intentionally ignored — bookmark existence already verified by UI
                }
            }
        }
    }
}

/// Actions that can be taken on bookmarks
enum BookmarkAction {
    Navigate(usize),
    StartRename(u64, String),
    FinishRename(u64, String),
    CancelRename,
    StartAnnotation(u64, String),
    FinishAnnotation(u64, String),
    CancelAnnotation,
    Delete(u64),
}
