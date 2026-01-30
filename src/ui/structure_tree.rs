//! Structure tree UI component for visualizing file sections

use crate::app::BendApp;
use crate::formats::{parse_file, FileSection, RiskLevel};
use eframe::egui::{self, RichText};

/// Get the color for a risk level
fn risk_color(risk: RiskLevel) -> egui::Color32 {
    match risk {
        RiskLevel::Safe => egui::Color32::from_rgb(100, 200, 100),      // Green
        RiskLevel::Caution => egui::Color32::from_rgb(200, 180, 80),    // Yellow
        RiskLevel::High => egui::Color32::from_rgb(200, 130, 80),       // Orange
        RiskLevel::Critical => egui::Color32::from_rgb(200, 80, 80),    // Red
    }
}

/// Get a risk level label
fn risk_label(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Safe => "Safe",
        RiskLevel::Caution => "Caution",
        RiskLevel::High => "High Risk",
        RiskLevel::Critical => "Critical",
    }
}

/// Show a single section in the tree
fn show_section(
    ui: &mut egui::Ui,
    section: &FileSection,
    clicked_offset: &mut Option<usize>,
    current_cursor: usize,
) {
    let is_cursor_in_section = current_cursor >= section.start && current_cursor < section.end;

    // Color the section name based on risk level
    let color = risk_color(section.risk);
    let mut name = RichText::new(&section.name).color(color);

    if is_cursor_in_section {
        name = name.strong();
    }

    // Create collapsible header if there are children
    if section.children.is_empty() {
        // Leaf node - just show as clickable label
        ui.horizontal(|ui| {
            if ui.selectable_label(is_cursor_in_section, name).clicked() {
                *clicked_offset = Some(section.start);
            }

            // Show offset and size
            ui.label(
                RichText::new(format!(
                    " 0x{:X}..0x{:X} ({} bytes)",
                    section.start,
                    section.end,
                    section.end - section.start
                ))
                .small()
                .color(egui::Color32::GRAY),
            );
        });

        // Show description if present
        if let Some(desc) = &section.description {
            if !desc.is_empty() {
                ui.indent("desc", |ui| {
                    ui.label(RichText::new(desc).small().italics());
                });
            }
        }
    } else {
        // Parent node with children
        let header = egui::CollapsingHeader::new(name)
            .default_open(is_cursor_in_section)
            .show(ui, |ui| {
                // Show this section's info
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "0x{:X}..0x{:X} ({} bytes)",
                            section.start,
                            section.end,
                            section.end - section.start
                        ))
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                    if ui.small_button("Go").clicked() {
                        *clicked_offset = Some(section.start);
                    }
                });

                if let Some(desc) = &section.description {
                    if !desc.is_empty() {
                        ui.label(RichText::new(desc).small().italics());
                    }
                }

                // Show children
                for child in &section.children {
                    show_section(ui, child, clicked_offset, current_cursor);
                }
            });

        // Make header clickable too
        if header.header_response.clicked() {
            *clicked_offset = Some(section.start);
        }
    }
}

/// Show the structure tree panel
pub fn show(ui: &mut egui::Ui, app: &mut BendApp) {
    let Some(editor) = &app.editor else {
        ui.label("No file loaded");
        return;
    };

    // Parse the file structure
    let sections = parse_file(editor.working());

    let Some(sections) = sections else {
        ui.label("Unable to parse file structure");
        return;
    };

    if sections.is_empty() {
        ui.label("No structure detected");
        return;
    }

    // Track clicked offset for navigation
    let mut clicked_offset: Option<usize> = None;
    let current_cursor = editor.cursor();

    // Legend
    ui.horizontal(|ui| {
        ui.label("Risk:");
        for risk in [RiskLevel::Safe, RiskLevel::Caution, RiskLevel::High, RiskLevel::Critical] {
            ui.colored_label(risk_color(risk), risk_label(risk));
        }
    });

    ui.separator();

    // Show sections
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for section in &sections {
                show_section(ui, section, &mut clicked_offset, current_cursor);
            }
        });

    // Handle navigation
    if let Some(offset) = clicked_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_cursor(offset);
        }
    }
}
