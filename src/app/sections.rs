use crate::formats::{FileSection, RiskLevel};
use eframe::egui;

use super::BendApp;

impl BendApp {
    /// Find the section containing a byte offset
    pub fn section_at_offset(&self, offset: usize) -> Option<&FileSection> {
        fn find_in_sections(sections: &[FileSection], offset: usize) -> Option<&FileSection> {
            for section in sections {
                if offset >= section.start && offset < section.end {
                    // Check children first for more specific match
                    if let Some(child) = find_in_sections(&section.children, offset) {
                        return Some(child);
                    }
                    return Some(section);
                }
            }
            None
        }

        self.cached_sections
            .as_ref()
            .and_then(|sections| find_in_sections(sections, offset))
    }

    /// Get the background color for a byte based on its section's risk level
    pub fn section_color_for_offset(&self, offset: usize) -> Option<egui::Color32> {
        self.section_at_offset(offset)
            .map(|section| section.risk.background_color())
    }

    /// Check if an offset is in a protected region (header protection enabled + High/Critical risk)
    pub fn is_offset_protected(&self, offset: usize) -> bool {
        if !self.header_protection {
            return false;
        }

        self.section_at_offset(offset)
            .map(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .unwrap_or(false)
    }

    /// Check if any byte in a range overlaps a protected region
    pub fn is_range_protected(&self, start: usize, len: usize) -> bool {
        if !self.header_protection || len == 0 {
            return false;
        }
        (start..start + len).any(|offset| self.is_offset_protected(offset))
    }

    /// Check if an offset is in a high-risk region that should show a warning
    /// Returns the risk level if it's High or Critical, None otherwise
    pub fn get_high_risk_level(&self, offset: usize) -> Option<RiskLevel> {
        self.section_at_offset(offset)
            .filter(|section| matches!(section.risk, RiskLevel::High | RiskLevel::Critical))
            .map(|section| section.risk)
    }

    /// Check if a warning should be shown for editing at this offset
    pub fn should_warn_for_edit(&self, offset: usize) -> bool {
        if self.dialogs.suppress_high_risk_warnings {
            return false;
        }
        self.get_high_risk_level(offset).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test app with cached sections
    fn create_test_app_with_sections(sections: Vec<FileSection>) -> BendApp {
        BendApp {
            cached_sections: Some(sections),
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

        // Test offset in first section
        let section = app.section_at_offset(5);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Header");

        // Test offset in second section
        let section = app.section_at_offset(50);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Data");

        // Test offset beyond all sections
        let section = app.section_at_offset(200);
        assert!(section.is_none());
    }

    #[test]
    fn test_section_at_offset_nested() {
        let parent = FileSection::new("Header", 0, 54, RiskLevel::Caution)
            .with_child(FileSection::new("Magic", 0, 2, RiskLevel::Critical))
            .with_child(FileSection::new("Size", 2, 6, RiskLevel::High));

        let sections = vec![parent, FileSection::new("Data", 54, 100, RiskLevel::Safe)];
        let app = create_test_app_with_sections(sections);

        // Test offset in nested child (should return most specific match)
        let section = app.section_at_offset(0);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Magic");

        let section = app.section_at_offset(4);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Size");

        // Test offset in parent but not in any child
        let section = app.section_at_offset(10);
        assert!(section.is_some());
        assert_eq!(section.unwrap().name, "Header");
    }

    #[test]
    fn test_section_at_offset_boundary() {
        let sections = vec![
            FileSection::new("First", 0, 10, RiskLevel::Safe),
            FileSection::new("Second", 10, 20, RiskLevel::Caution),
        ];
        let app = create_test_app_with_sections(sections);

        // Test at exact boundary (end is exclusive)
        let section = app.section_at_offset(9);
        assert_eq!(section.unwrap().name, "First");

        let section = app.section_at_offset(10);
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

        // Verify colors are returned for each risk level
        let color = app.section_color_for_offset(5);
        assert!(color.is_some());
        // Green-ish for Safe
        let c = color.unwrap();
        assert!(c.g() > c.r()); // Green channel should be highest

        let color = app.section_color_for_offset(25);
        assert!(color.is_some());
        // Orange-ish for High
        let c = color.unwrap();
        assert!(c.r() > c.b()); // Red channel higher than blue

        // No color for offset outside sections
        let color = app.section_color_for_offset(100);
        assert!(color.is_none());
    }

    #[test]
    fn test_section_at_offset_no_sections() {
        let app = BendApp::default();

        // Should return None when no sections cached
        assert!(app.section_at_offset(0).is_none());
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
        assert!(!app.header_protection);
        assert!(!app.is_offset_protected(5)); // Safe
        assert!(!app.is_offset_protected(15)); // Caution
        assert!(!app.is_offset_protected(25)); // High
        assert!(!app.is_offset_protected(35)); // Critical

        // Enable header protection
        app.header_protection = true;

        // Safe and Caution still not protected
        assert!(!app.is_offset_protected(5));
        assert!(!app.is_offset_protected(15));

        // High and Critical are now protected
        assert!(app.is_offset_protected(25));
        assert!(app.is_offset_protected(35));

        // Offset outside any section is not protected
        assert!(!app.is_offset_protected(100));
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

        // Warnings not suppressed by default
        assert!(!app.dialogs.suppress_high_risk_warnings);

        // Safe and Caution should not trigger warnings
        assert!(!app.should_warn_for_edit(5));
        assert!(!app.should_warn_for_edit(15));

        // High and Critical should trigger warnings
        assert!(app.should_warn_for_edit(25));
        assert!(app.should_warn_for_edit(35));

        // get_high_risk_level returns correct levels
        assert!(app.get_high_risk_level(5).is_none());
        assert!(app.get_high_risk_level(15).is_none());
        assert_eq!(app.get_high_risk_level(25), Some(RiskLevel::High));
        assert_eq!(app.get_high_risk_level(35), Some(RiskLevel::Critical));

        // Suppress warnings
        app.dialogs.suppress_high_risk_warnings = true;

        // No warnings when suppressed
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
        app.header_protection = true;

        // Unknown region should NOT be protected even with header protection on
        assert!(!app.is_offset_protected(10));
        assert!(!app.is_offset_protected(40));

        // Critical region is still protected
        assert!(app.is_offset_protected(55));
    }

    #[test]
    fn test_unknown_risk_no_warnings() {
        let sections = vec![
            FileSection::new("Unknown", 0, 50, RiskLevel::Unknown),
            FileSection::new("High", 50, 60, RiskLevel::High),
        ];
        let app = create_test_app_with_sections(sections);

        // Unknown should NOT trigger a warning
        assert!(!app.should_warn_for_edit(10));
        assert!(app.get_high_risk_level(10).is_none());

        // High still triggers a warning
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
        app.header_protection = true;

        // Entirely in safe region — not protected
        assert!(!app.is_range_protected(0, 10));

        // Entirely in protected region
        assert!(app.is_range_protected(10, 5));
        assert!(app.is_range_protected(20, 5));

        // Spanning safe-to-protected boundary
        assert!(app.is_range_protected(8, 4)); // bytes 8..12, crosses into High at 10

        // Zero length — never protected
        assert!(!app.is_range_protected(15, 0));

        // Protection disabled — nothing protected
        app.header_protection = false;
        assert!(!app.is_range_protected(10, 5));
        assert!(!app.is_range_protected(20, 5));
    }
}
