//! UI components for bend-rs

pub mod bookmarks;
pub mod go_to_offset_dialog;
pub mod hex_editor;
pub mod image_preview;
pub mod savepoints;
pub mod search_dialog;
pub mod settings_dialog;
pub mod shortcuts_dialog;
pub mod structure_tree;
pub mod theme;

use eframe::egui;

/// Sets the pointing-hand cursor while a clickable widget is hovered.
/// No-op for disabled widgets — disabled buttons keep the default cursor.
pub trait PointerCursor {
    fn pointer_cursor(self) -> Self;
}

impl PointerCursor for egui::Response {
    fn pointer_cursor(self) -> Self {
        if self.enabled() {
            self.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            self
        }
    }
}
