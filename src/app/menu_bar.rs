use eframe::egui;

use super::BendApp;

/// Returns the platform-appropriate modifier key text for shortcuts
fn modifier_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘ " // space to give the character that follows more breathing room
    } else {
        "Ctrl+"
    }
}

/// Menu item with shortcut hint that has better contrast than egui's default.
/// Uses a horizontal layout with the shortcut text aligned right.
/// Shortcut text is dimmer when not hovered, brighter when hovered.
fn menu_item_with_shortcut(ui: &mut egui::Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    // Calculate label and shortcut widths for proper sizing
    let label_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
    let shortcut_galley = ui.painter().layout_no_wrap(
        shortcut.to_string(),
        egui::FontId::default(),
        egui::Color32::WHITE,
    );

    // Width = label + gap + shortcut + padding
    let desired_width = label_galley.size().x + 40.0 + shortcut_galley.size().x + 8.0;

    let response = ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(egui::vec2(desired_width, 0.0)),
    );

    // Paint shortcut with brightness based on hover state
    if !shortcut.is_empty() {
        let shortcut_color = if response.hovered() {
            egui::Color32::from_gray(200) // Brighter when hovered
        } else {
            egui::Color32::from_gray(120) // Dimmer when not hovered
        };

        let shortcut_galley = ui.painter().layout_no_wrap(
            shortcut.to_string(),
            egui::FontId::default(),
            shortcut_color,
        );

        let pos = egui::pos2(
            response.rect.right() - shortcut_galley.size().x - 8.0,
            response.rect.center().y - shortcut_galley.size().y / 2.0,
        );
        ui.painter().galley(pos, shortcut_galley, shortcut_color);
    }

    response.clicked()
}

impl BendApp {
    /// Render the top menu bar
    pub(super) fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| self.render_file_menu(ui, ctx));
                ui.menu_button("Edit", |ui| self.render_edit_menu(ui));
                ui.menu_button("Help", |ui| self.render_help_menu(ui));
            });
        });
    }

    /// Render the File menu contents
    fn render_file_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mod_str = modifier_key();
        let open_shortcut = format!("{}O", mod_str);
        let export_shortcut = format!("{}E", mod_str);

        if menu_item_with_shortcut(ui, "Open...", &open_shortcut, true) {
            self.open_file_dialog();
            ui.close_menu();
        }
        let has_file = self.editor.is_some();
        if menu_item_with_shortcut(ui, "Export...", &export_shortcut, has_file) {
            self.export_file();
            ui.close_menu();
        }
        ui.separator();

        // Recent files submenu
        let recent_files = self.settings.recent_files().to_vec();
        let has_recent = !recent_files.is_empty();
        ui.menu_button("Recent Files", |ui| {
            if has_recent {
                for path in &recent_files {
                    let display_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.to_string_lossy().into_owned());

                    if ui
                        .button(&display_name)
                        .on_hover_text(path.to_string_lossy())
                        .clicked()
                    {
                        self.pending_open_path = Some(path.clone());
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Clear Recent Files").clicked() {
                    self.settings.clear_recent_files();
                    self.settings.save();
                    ui.close_menu();
                }
            } else {
                ui.label("No recent files");
            }
        });

        ui.separator();
        if ui.button("Exit").clicked() {
            if self.has_unsaved_changes() {
                self.dialogs.show_close = true;
            } else {
                self.settings.save();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ui.close_menu();
        }
    }

    /// Render the Edit menu contents
    fn render_edit_menu(&mut self, ui: &mut egui::Ui) {
        let mod_str = modifier_key();
        let has_file = self.editor.is_some();
        let can_undo = self.editor.as_ref().is_some_and(|e| e.can_undo());
        let can_redo = self.editor.as_ref().is_some_and(|e| e.can_redo());
        let undo_shortcut = format!("{}Z", mod_str);
        let redo_shortcut = format!("{}Shift+Z", mod_str);
        let find_shortcut = format!("{}F", mod_str);
        let goto_shortcut = format!("{}G", mod_str);
        let refresh_shortcut = format!("{}R", mod_str);

        if menu_item_with_shortcut(ui, "Undo", &undo_shortcut, can_undo) {
            self.do_undo();
            ui.close_menu();
        }
        if menu_item_with_shortcut(ui, "Redo", &redo_shortcut, can_redo) {
            self.do_redo();
            ui.close_menu();
        }
        ui.separator();

        if menu_item_with_shortcut(ui, "Find & Replace...", &find_shortcut, has_file) {
            self.search_state.open_dialog();
            ui.close_menu();
        }
        if menu_item_with_shortcut(ui, "Go to Offset...", &goto_shortcut, has_file) {
            self.go_to_offset_state.open_dialog();
            ui.close_menu();
        }
        ui.separator();
        if menu_item_with_shortcut(ui, "Refresh Preview", &refresh_shortcut, has_file) {
            self.mark_preview_dirty();
            ui.close_menu();
        }
        ui.separator();
        if ui
            .add_enabled(
                has_file,
                egui::Checkbox::new(&mut self.header_protection, "Protect Headers"),
            )
            .changed()
        {
            // Checkbox already updates the value
        }
        ui.separator();
        // Re-enable warnings option (only shown when warnings are suppressed)
        if self.dialogs.suppress_high_risk_warnings {
            if ui.button("Re-enable High-Risk Warnings").clicked() {
                self.dialogs.suppress_high_risk_warnings = false;
                ui.close_menu();
            }
        } else {
            ui.add_enabled(false, egui::Button::new("High-Risk Warnings: Enabled"));
        }
        ui.separator();
        if ui.button("Preferences...").clicked() {
            self.settings_dialog_state.open(&self.settings);
            ui.close_menu();
        }
    }

    /// Render the Help menu contents
    fn render_help_menu(&mut self, ui: &mut egui::Ui) {
        if menu_item_with_shortcut(ui, "Keyboard Shortcuts", "F1", true) {
            self.shortcuts_dialog_state.open_dialog();
            ui.close_menu();
        }
    }
}
