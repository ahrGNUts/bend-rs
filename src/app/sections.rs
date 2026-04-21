//! Cross-cutting helpers that combine `DocumentState` structure lookups with
//! `UiState` (colors, warning-suppression) concerns. Pure structure lookups
//! (`section_at_offset`, `is_offset_protected`, `is_range_protected`,
//! `get_high_risk_level`) live on `DocumentState` in `src/app/state.rs`.

use eframe::egui;

use super::BendApp;

impl BendApp {
    /// Get the background color for a byte based on its section's risk level.
    /// Combines document-structure lookup with UI color palette.
    pub fn section_color_for_offset(&self, offset: usize) -> Option<egui::Color32> {
        self.doc
            .section_at_offset(offset)
            .map(|section| self.ui.colors.risk_bg_color(section.risk))
    }

    /// Check if a warning should be shown for editing at this offset.
    /// Respects the session-level warning-suppression flag on `UiState`.
    pub fn should_warn_for_edit(&self, offset: usize) -> bool {
        if self.ui.dialogs.suppress_high_risk_warnings {
            return false;
        }
        self.doc.get_high_risk_level(offset).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::DocumentState;
    use crate::formats::{FileSection, RiskLevel};

    /// Helper to create a test app with cached sections
    fn create_test_app_with_sections(sections: Vec<FileSection>) -> BendApp {
        BendApp {
            doc: DocumentState {
                cached_sections: Some(sections),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_section_at_offset_simple() {
        let sections = vec![
            FileSection::new("Header", 0, 14, RiskLevel::Critical),
            FileSection::new("Data", 14, 100, RiskLevel::Safe),
        ];
        let app = create_test_app_with_sections(sections);

        let section = app.doc.section_at_offset(5);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Header");

        let section = app.doc.section_at_offset(50);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Data");

        let section = app.doc.section_at_offset(200);
        assert!(section.is_none());
    }

    #[test]
    fn test_section_at_offset_nested() {
        let parent = FileSection::new("Header", 0, 54, RiskLevel::Caution)
            .with_child(FileSection::new("Magic", 0, 2, RiskLevel::Critical))
            .with_child(FileSection::new("Size", 2, 6, RiskLevel::High));

        let sections = vec![parent, FileSection::new("Data", 54, 100, RiskLevel::Safe)];
        let app = create_test_app_with_sections(sections);

        let section = app.doc.section_at_offset(0);
        assert_eq!(section.unwrap().name, "Magic");

        let section = app.doc.section_at_offset(4);
        assert_eq!(section.unwrap().name, "Size");

        let section = app.doc.section_at_offset(10);
        assert_eq!(section.unwrap().name, "Header");
    }

    #[test]
    fn test_section_at_offset_boundary() {
        let sections = vec![
            FileSection::new("First", 0, 10, RiskLevel::Safe),
            FileSection::new("Second", 10, 20, RiskLevel::Caution),
        ];
        let app = create_test_app_with_sections(sections);

        let section = app.doc.section_at_offset(9);
        assert_eq!(section.unwrap().name, "First");

        let section = app.doc.section_at_offset(10);
        assert_eq!(section.unwrap().name, "Second");
    }

    #[test]
    fn test_section_color_for_offset() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let app = create_test_app_with_sections(sections);

        let color = app.section_color_for_offset(5);
        assert!(color.is_some());
        let c = color.unwrap();
        assert!(c.g() > c.r());

        let color = app.section_color_for_offset(25);
        assert!(color.is_some());
        let c = color.unwrap();
        assert!(c.r() > c.b());

        let color = app.section_color_for_offset(100);
        assert!(color.is_none());
    }

    #[test]
    fn test_section_at_offset_no_sections() {
        let app = BendApp::default();

        assert!(app.doc.section_at_offset(0).is_none());
        assert!(app.section_color_for_offset(0).is_none());
    }

    #[test]
    fn test_header_protection() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);

        // Header protection disabled - nothing protected
        assert!(!app.doc.header_protection);
        assert!(!app.doc.is_offset_protected(5));
        assert!(!app.doc.is_offset_protected(15));
        assert!(!app.doc.is_offset_protected(25));
        assert!(!app.doc.is_offset_protected(35));

        // Enable header protection
        app.doc.header_protection = true;

        assert!(!app.doc.is_offset_protected(5));
        assert!(!app.doc.is_offset_protected(15));
        assert!(app.doc.is_offset_protected(25));
        assert!(app.doc.is_offset_protected(35));
        assert!(!app.doc.is_offset_protected(100));
    }

    #[test]
    fn test_high_risk_warnings() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("Caution", 10, 20, RiskLevel::Caution),
            FileSection::new("High", 20, 30, RiskLevel::High),
            FileSection::new("Critical", 30, 40, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);

        assert!(!app.ui.dialogs.suppress_high_risk_warnings);

        assert!(!app.should_warn_for_edit(5));
        assert!(!app.should_warn_for_edit(15));

        assert!(app.should_warn_for_edit(25));
        assert!(app.should_warn_for_edit(35));

        assert!(app.doc.get_high_risk_level(5).is_none());
        assert!(app.doc.get_high_risk_level(15).is_none());
        assert_eq!(app.doc.get_high_risk_level(25), Some(RiskLevel::High));
        assert_eq!(app.doc.get_high_risk_level(35), Some(RiskLevel::Critical));

        // Suppress warnings
        app.ui.dialogs.suppress_high_risk_warnings = true;

        assert!(!app.should_warn_for_edit(25));
        assert!(!app.should_warn_for_edit(35));
    }

    #[test]
    fn test_unknown_risk_not_protected() {
        let sections = vec![
            FileSection::new("Unknown", 0, 50, RiskLevel::Unknown),
            FileSection::new("Header", 50, 60, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);
        app.doc.header_protection = true;

        assert!(!app.doc.is_offset_protected(10));
        assert!(!app.doc.is_offset_protected(40));
        assert!(app.doc.is_offset_protected(55));
    }

    #[test]
    fn test_unknown_risk_no_warnings() {
        let sections = vec![
            FileSection::new("Unknown", 0, 50, RiskLevel::Unknown),
            FileSection::new("High", 50, 60, RiskLevel::High),
        ];
        let app = create_test_app_with_sections(sections);

        assert!(!app.should_warn_for_edit(10));
        assert!(app.doc.get_high_risk_level(10).is_none());

        assert!(app.should_warn_for_edit(55));
    }

    #[test]
    fn test_is_range_protected() {
        let sections = vec![
            FileSection::new("Safe", 0, 10, RiskLevel::Safe),
            FileSection::new("High", 10, 20, RiskLevel::High),
            FileSection::new("Critical", 20, 30, RiskLevel::Critical),
        ];
        let mut app = create_test_app_with_sections(sections);
        app.doc.header_protection = true;

        assert!(!app.doc.is_range_protected(0, 10));
        assert!(app.doc.is_range_protected(10, 5));
        assert!(app.doc.is_range_protected(20, 5));
        assert!(app.doc.is_range_protected(8, 4));
        assert!(!app.doc.is_range_protected(15, 0));

        app.doc.header_protection = false;
        assert!(!app.doc.is_range_protected(10, 5));
        assert!(!app.doc.is_range_protected(20, 5));
    }
}
