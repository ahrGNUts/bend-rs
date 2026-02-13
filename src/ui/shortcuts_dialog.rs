//! Keyboard shortcuts help dialog

use eframe::egui;

/// State for the keyboard shortcuts help dialog
#[derive(Default)]
pub struct ShortcutsDialogState {
    /// Whether the dialog is visible
    pub dialog_open: bool,
}

impl ShortcutsDialogState {
    /// Open the shortcuts dialog
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
    }

    /// Close the shortcuts dialog
    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
    }
}

/// Show the keyboard shortcuts help dialog
pub fn show(ctx: &egui::Context, state: &mut ShortcutsDialogState) {
    if !state.dialog_open {
        return;
    }

    let mut close_dialog = false;

    egui::Window::new("Keyboard Shortcuts")
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // File Operations
                ui.heading("File Operations");
                shortcuts_table(ui, "file_ops", &[
                    ("Ctrl+O / Cmd+O", "Open file"),
                    ("Ctrl+E / Cmd+E", "Export file"),
                ]);

                ui.add_space(10.0);

                // Edit Operations
                ui.heading("Edit Operations");
                shortcuts_table(ui, "edit_ops", &[
                    ("Ctrl+Z / Cmd+Z", "Undo"),
                    ("Ctrl+Shift+Z / Cmd+Shift+Z", "Redo"),
                    ("Ctrl+Y / Cmd+Y", "Redo (alternative)"),
                    ("Ctrl+F / Cmd+F", "Find & Replace"),
                    ("Ctrl+G / Cmd+G", "Go to offset"),
                    ("Ctrl+S / Cmd+S", "Create save point"),
                    ("Ctrl+D / Cmd+D", "Add bookmark at cursor"),
                ]);

                ui.add_space(10.0);

                // Navigation
                ui.heading("Navigation");
                shortcuts_table(ui, "navigation", &[
                    ("Arrow Keys", "Move cursor"),
                    ("Page Up / Page Down", "Move cursor by 16 rows"),
                    ("Home", "Go to start of file"),
                    ("End", "Go to end of file"),
                ]);

                ui.add_space(10.0);

                // Selection
                ui.heading("Selection");
                shortcuts_table(ui, "selection", &[
                    ("Shift + Arrow Keys", "Extend selection"),
                    ("Shift + Page Up/Down", "Extend selection by 16 rows"),
                    ("Shift + Home", "Select to start"),
                    ("Shift + End", "Select to end"),
                    ("Shift + Click", "Select range"),
                ]);

                ui.add_space(10.0);

                // Hex Editing
                ui.heading("Hex Editing");
                shortcuts_table(ui, "hex_editing", &[
                    ("0-9, A-F", "Edit hex value at cursor"),
                    ("Right-click", "Context menu (copy, paste, bookmark)"),
                ]);

                ui.add_space(10.0);

                // View
                ui.heading("View");
                shortcuts_table(ui, "view", &[
                    ("F1", "Show this help screen"),
                ]);
            });

            ui.add_space(10.0);

            // Close button
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close_dialog = true;
                }
            });

            // Handle Escape to close
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_dialog = true;
            }
        });

    if close_dialog {
        state.close_dialog();
    }
}

/// Render a table of keyboard shortcuts
fn shortcuts_table(ui: &mut egui::Ui, section: &str, shortcuts: &[(&str, &str)]) {
    egui::Grid::new(ui.id().with(section))
        .num_columns(2)
        .spacing([20.0, 4.0])
        .show(ui, |ui| {
            for (shortcut, description) in shortcuts {
                ui.label(
                    egui::RichText::new(*shortcut)
                        .monospace()
                        .strong()
                );
                ui.label(*description);
                ui.end_row();
            }
        });
}
