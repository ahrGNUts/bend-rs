//! Structure tree UI component for visualizing file sections

use crate::app::BendApp;
use crate::formats::{FileSection, RiskLevel};
use eframe::egui::{self, RichText};

/// Show a single section in the tree
fn show_section(
    ui: &mut egui::Ui,
    section: &FileSection,
    clicked_offset: &mut Option<usize>,
    current_cursor: usize,
) {
    let is_cursor_in_section = current_cursor >= section.start && current_cursor < section.end;

    // Color the section name based on risk level
    let color = section.risk.color();
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
    // Get cursor position and check if editor exists
    let current_cursor = match &app.editor {
        Some(editor) => editor.cursor(),
        None => {
            ui.label("No file loaded");
            return;
        }
    };

    // Early return for missing or empty sections
    match &app.cached_sections {
        None => {
            ui.label("Unable to parse file structure");
            return;
        }
        Some(sections) if sections.is_empty() => {
            ui.label("No structure detected");
            return;
        }
        _ => {}
    }

    // Track clicked offset for navigation
    let mut clicked_offset: Option<usize> = None;

    // Scope the immutable borrow of sections for UI rendering
    if let Some(sections) = &app.cached_sections {
        // Legend
        ui.horizontal(|ui| {
            ui.label("Risk:");
            for risk in [RiskLevel::Safe, RiskLevel::Caution, RiskLevel::High, RiskLevel::Critical] {
                ui.colored_label(risk.color(), risk.label());
            }
        });

        ui.separator();

        // Show sections
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for section in sections {
                    show_section(ui, section, &mut clicked_offset, current_cursor);
                }
            });
    }

    // Handle navigation - borrow of cached_sections has ended
    if let Some(offset) = clicked_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_cursor(offset);
        }
    }
}
