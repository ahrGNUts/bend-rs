use crate::formats::RiskLevel;
use eframe::egui;

use super::BendApp;

/// State for close confirmation and high-risk edit warning dialogs
#[derive(Default)]
pub struct DialogState {
    /// Whether the close confirmation dialog is showing
    pub show_close: bool,
    /// Pending close action (true = confirmed close)
    pub pending_close: bool,
    /// Whether high-risk edit warnings are suppressed for this session
    pub suppress_high_risk_warnings: bool,
    /// Pending high-risk edit waiting for user confirmation
    pub pending_high_risk_edit: Option<PendingEdit>,
    /// Checkbox state for "don't warn again" in high-risk dialog
    pub high_risk_dont_show: bool,
}

/// Type of pending edit (hex nibble or ASCII character)
#[derive(Clone, Copy)]
pub enum PendingEditType {
    /// Nibble edit (hex mode): nibble value 0-15
    Nibble(u8),
    /// ASCII edit: character to write
    Ascii(char),
    /// Backspace key (insert mode delete-previous)
    Backspace,
    /// Delete key (insert mode delete-at-cursor)
    Delete,
}

/// A pending edit awaiting user confirmation
#[derive(Clone, Copy)]
pub struct PendingEdit {
    /// The type of edit (nibble or ASCII)
    pub edit_type: PendingEditType,
    /// The byte offset being edited
    pub offset: usize,
    /// Risk level of the section being edited
    pub risk_level: RiskLevel,
}

impl BendApp {
    /// Show the high-risk edit warning dialog and handle user response
    pub(super) fn show_high_risk_warning_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.dialogs.pending_high_risk_edit else {
            return;
        };

        let mut should_proceed = false;
        let mut should_cancel = false;

        egui::Window::new("High-Risk Edit Warning")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Warning icon and message
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("\u{26A0}")
                                .size(32.0)
                                .color(egui::Color32::YELLOW),
                        );
                        ui.vertical(|ui| {
                            let risk_name = match pending.risk_level {
                                RiskLevel::High => "high-risk",
                                RiskLevel::Critical => "critical",
                                _ => "sensitive",
                            };
                            ui.label(format!("You are about to edit a {} region.", risk_name));
                            ui.label(format!("Offset: 0x{:08X}", pending.offset));
                        });
                    });

                    ui.add_space(10.0);

                    ui.label("Editing this region may corrupt the file or make it unreadable.");
                    ui.label("The image preview may fail to render after this edit.");

                    ui.add_space(10.0);

                    // Don't show again checkbox
                    ui.checkbox(
                        &mut self.dialogs.high_risk_dont_show,
                        "Don't warn me again this session",
                    );

                    ui.add_space(10.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Proceed").clicked() {
                            should_proceed = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_cancel = true;
                        }
                    });
                });
            });

        // Handle user response
        if should_proceed {
            // Apply the edit based on type
            if let Some(editor) = &mut self.editor {
                match pending.edit_type {
                    PendingEditType::Nibble(nibble_value) => {
                        let _ = editor.edit_nibble_with_mode(nibble_value);
                    }
                    PendingEditType::Ascii(ch) => {
                        let _ = editor.edit_ascii_with_mode(ch);
                    }
                    PendingEditType::Backspace => {
                        editor.handle_backspace();
                    }
                    PendingEditType::Delete => {
                        editor.handle_delete();
                    }
                }
            }
            if self.dialogs.high_risk_dont_show {
                self.dialogs.suppress_high_risk_warnings = true;
            }
            self.dialogs.high_risk_dont_show = false;
            self.dialogs.pending_high_risk_edit = None;
        } else if should_cancel {
            self.dialogs.high_risk_dont_show = false;
            self.dialogs.pending_high_risk_edit = None;
        }
    }

    /// Show the close confirmation dialog
    pub(super) fn show_close_dialog(&mut self, ctx: &egui::Context) {
        if !self.dialogs.show_close {
            return;
        }

        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("You have unsaved changes. Are you sure you want to exit?");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Export First").clicked() {
                        self.export_file();
                        self.dialogs.show_close = false;
                    }
                    if ui.button("Discard & Exit").clicked() {
                        self.dialogs.pending_close = true;
                        self.dialogs.show_close = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.dialogs.show_close = false;
                    }
                });
            });
    }
}
