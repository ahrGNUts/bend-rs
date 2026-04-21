//! Structure tree UI component for visualizing file sections

use crate::app::BendApp;
use crate::formats::{FileSection, RiskLevel};
use crate::ui::theme::AppColors;
use eframe::egui::{self, RichText};

/// Show a single section in the tree
fn show_section(
    ui: &mut egui::Ui,
    section: &FileSection,
    clicked_offset: &mut Option<usize>,
    current_cursor: usize,
    colors: &AppColors,
) {
    let is_cursor_in_section = current_cursor >= section.start && current_cursor < section.end;

    // Color the section name with a background badge matching the hex editor style
    let bg = colors.risk_bg_color(section.risk);
    let mut name = RichText::new(&*section.name).color(colors.hex_byte_text);

    if is_cursor_in_section {
        name = name.strong();
    }

    // Create collapsible header if there are children
    if section.children.is_empty() {
        // Leaf node - just show as clickable label
        ui.horizontal(|ui| {
            let bg_idx = ui.painter().add(egui::Shape::Noop);
            let response = ui.selectable_label(false, name);
            let rounding = ui.visuals().widgets.inactive.rounding;
            ui.painter().set(
                bg_idx,
                egui::Shape::rect_filled(response.rect, rounding, bg),
            );
            if response.clicked() {
                *clicked_offset = Some(section.start);
            }
            // Draw a risk-colored outline around the selected section label
            if is_cursor_in_section {
                let rect = response.rect.expand(1.0);
                ui.painter().rect_stroke(
                    rect,
                    egui::Rounding::same(3.0),
                    egui::Stroke::new(1.5, colors.risk_color(section.risk)),
                );
            }

            // Show offset and size
            ui.label(
                RichText::new(format!(
                    " 0x{:X}..0x{:X} ({} bytes)",
                    section.start,
                    section.end,
                    section.end - section.start
                ))
                .small(),
            );
        });

        // Show description if present
        if let Some(desc) = &section.description {
            if !desc.is_empty() {
                ui.indent(section.start, |ui| {
                    ui.label(RichText::new(desc).small().italics());
                });
            }
        }
    } else {
        // Parent node with children
        let bg_idx = ui.painter().add(egui::Shape::Noop);
        let header = egui::CollapsingHeader::new(name)
            .id_salt(section.start)
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
                        .small(),
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
                    show_section(ui, child, clicked_offset, current_cursor, colors);
                }
            });

        let rounding = ui.visuals().widgets.inactive.rounding;
        ui.painter().set(
            bg_idx,
            egui::Shape::rect_filled(header.header_response.rect, rounding, bg),
        );

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
        let colors = app.ui.colors;

        // Legend
        ui.horizontal(|ui| {
            ui.label("Risk:");
            for risk in [
                RiskLevel::Safe,
                RiskLevel::Caution,
                RiskLevel::High,
                RiskLevel::Critical,
            ] {
                egui::Frame::none()
                    .fill(colors.risk_bg_color(risk))
                    .rounding(egui::Rounding::same(3.0))
                    .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(risk.label()).color(colors.hex_byte_text));
                    });
            }
        });

        ui.separator();

        // Show sections
        for section in sections {
            show_section(ui, section, &mut clicked_offset, current_cursor, &colors);
        }
    }

    // Handle navigation - borrow of cached_sections has ended
    if let Some(offset) = clicked_offset {
        if let Some(editor) = &mut app.editor {
            editor.set_cursor(offset);
        }
        app.scroll_hex_to_offset(offset);
    }
}
