//! Settings/Preferences dialog UI component

use crate::settings::{AppSettings, ThemePreference};
use eframe::egui;

/// State for the settings dialog
#[derive(Default)]
pub struct SettingsDialogState {
    /// Whether the dialog is visible
    pub dialog_open: bool,
    /// Snapshot of settings when dialog opened (for change detection)
    initial_settings: Option<AppSettings>,
}

impl SettingsDialogState {
    /// Open the settings dialog and snapshot current settings for change detection
    pub fn open(&mut self, settings: &AppSettings) {
        self.dialog_open = true;
        self.initial_settings = Some(settings.clone());
    }

    /// Close the settings dialog
    pub fn close(&mut self) {
        self.dialog_open = false;
        self.initial_settings = None;
    }
}

/// Actions that can be triggered by the settings dialog
enum SettingsAction {
    Close,
    ClearRecent,
}

/// Show the settings/preferences dialog
/// Returns true if settings were changed and should be saved
pub fn show(
    ctx: &egui::Context,
    state: &mut SettingsDialogState,
    settings: &mut AppSettings,
) -> bool {
    if !state.dialog_open {
        return false;
    }

    let mut actions: Vec<SettingsAction> = Vec::new();

    egui::Window::new("Preferences")
        .collapsible(false)
        .resizable(true)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // Appearance section
            ui.heading("Appearance");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Theme:");
                let prev = settings.theme;
                ui.selectable_value(&mut settings.theme, ThemePreference::Light, "Light");
                ui.selectable_value(&mut settings.theme, ThemePreference::Dark, "Dark");
                ui.selectable_value(&mut settings.theme, ThemePreference::System, "System");
                // Live preview: apply immediately on change
                if settings.theme != prev {
                    settings.theme.apply(ctx);
                }
            });

            ui.add_space(16.0);

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
                    actions.push(SettingsAction::ClearRecent);
                }
            });

            ui.add_space(16.0);

            // Info section
            ui.heading("About");
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Settings are saved when this dialog is closed.")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );

            if let Some(path) = dirs::config_dir() {
                let settings_path = path.join("bend-rs").join("settings.json");
                ui.label(
                    egui::RichText::new(format!("Settings file: {}", settings_path.display()))
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }

            ui.add_space(16.0);

            // Close button
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    actions.push(SettingsAction::Close);
                }
            });

            // Handle Escape to close
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                actions.push(SettingsAction::Close);
            }
        });

    // Process actions after UI scope
    let mut should_save = false;

    for action in &actions {
        match action {
            SettingsAction::ClearRecent => {
                settings.clear_recent_files();
                should_save = true;
            }
            SettingsAction::Close => {
                // Check if settings changed
                if state.initial_settings.as_ref() != Some(settings) {
                    should_save = true;
                }
                state.close();
            }
        }
    }

    // Save immediately if needed
    if should_save {
        settings.save();
    }

    should_save
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_dialog_state_default() {
        let state = SettingsDialogState::default();
        assert!(!state.dialog_open);
        assert!(state.initial_settings.is_none());
    }

    #[test]
    fn test_settings_dialog_state_open_close() {
        let mut state = SettingsDialogState::default();
        let settings = AppSettings::default();

        state.open(&settings);
        assert!(state.dialog_open);
        assert!(state.initial_settings.is_some());

        state.close();
        assert!(!state.dialog_open);
        assert!(state.initial_settings.is_none());
    }
}
