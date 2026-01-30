//! Settings/Preferences dialog UI component

use crate::settings::AppSettings;
use eframe::egui;

/// State for the settings dialog
#[derive(Default)]
pub struct SettingsDialogState {
    /// Whether the dialog is visible
    pub dialog_open: bool,
}

impl SettingsDialogState {
    /// Open the settings dialog
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
    }

    /// Close the settings dialog
    pub fn close_dialog(&mut self) {
        self.dialog_open = false;
    }
}

/// Show the settings/preferences dialog
pub fn show(ctx: &egui::Context, state: &mut SettingsDialogState, settings: &mut AppSettings) {
    if !state.dialog_open {
        return;
    }

    let mut close_dialog = false;
    let mut clear_recent = false;

    egui::Window::new("Preferences")
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // Editing section
            ui.heading("Editing");
            ui.add_space(4.0);

            ui.checkbox(
                &mut settings.default_header_protection,
                "Enable header protection by default",
            )
            .on_hover_text(
                "When enabled, new files will have header protection turned on, \
                 preventing accidental edits to critical file structure regions",
            );

            ui.checkbox(
                &mut settings.show_high_risk_warnings,
                "Show warnings for high-risk edits",
            )
            .on_hover_text(
                "Display a warning dialog when editing regions that are likely \
                 to corrupt the file (e.g., JPEG scan data headers)",
            );

            ui.add_space(16.0);

            // Recent files section
            ui.heading("Recent Files");
            ui.add_space(4.0);

            let recent_count = settings.recent_files().len();
            ui.horizontal(|ui| {
                ui.label(format!("{} recent file(s) stored", recent_count));
                if ui
                    .add_enabled(recent_count > 0, egui::Button::new("Clear"))
                    .clicked()
                {
                    clear_recent = true;
                }
            });

            ui.add_space(16.0);

            // Info section
            ui.heading("About");
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Settings are saved automatically when the application closes.")
                    .small()
                    .color(egui::Color32::GRAY),
            );

            if let Some(path) = dirs::config_dir() {
                let settings_path = path.join("bend-rs").join("settings.json");
                ui.label(
                    egui::RichText::new(format!("Settings file: {}", settings_path.display()))
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }

            ui.add_space(16.0);

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

    // Process actions after UI scope
    if clear_recent {
        settings.clear_recent_files();
    }

    if close_dialog {
        state.close_dialog();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_dialog_state_default() {
        let state = SettingsDialogState::default();
        assert!(!state.dialog_open);
    }

    #[test]
    fn test_settings_dialog_state_open_close() {
        let mut state = SettingsDialogState::default();

        state.open_dialog();
        assert!(state.dialog_open);

        state.close_dialog();
        assert!(!state.dialog_open);
    }
}
