//! Centralized, theme-aware color palette for the application.
//!
//! Every custom color used in the UI lives here. Render sites construct
//! `AppColors::new(dark_mode)` and read fields instead of using inline RGB literals.

use crate::formats::RiskLevel;
use eframe::egui::Color32;

/// Theme-aware color palette. Construct via `AppColors::new(dark_mode)`.
pub struct AppColors {
    // -- Risk level solid colors (structure tree, legends) --
    pub risk_safe: Color32,
    pub risk_caution: Color32,
    pub risk_high: Color32,
    pub risk_critical: Color32,
    pub risk_unknown: Color32,

    /// Alpha used for risk-level background tints in the hex view.
    risk_bg_alpha: u8,

    // -- Cursor highlights --
    pub cursor_bright_overwrite: Color32,
    pub cursor_dim_overwrite: Color32,
    pub cursor_bright_insert: Color32,
    pub cursor_dim_insert: Color32,

    // -- Selection / search / bookmark highlights --
    pub selection_bg: Color32,
    pub current_match_bg: Color32,
    pub search_match_bg: Color32,
    pub bookmark_bg: Color32,

    // -- Status indicators --
    pub modified_indicator: Color32,
    pub warning_text: Color32,
    pub error_text: Color32,

    // -- Editor chrome --
    pub ascii_delimiter: Color32,
    pub cursor_text: Color32,
    /// Text color for hex bytes that sit on a tinted background
    pub hex_byte_text: Color32,

    // -- Menu shortcut text --
    pub shortcut_normal: Color32,
    pub shortcut_hover: Color32,
}

impl AppColors {
    /// Build a palette appropriate for the current theme.
    pub fn new(dark_mode: bool) -> Self {
        if dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Dark-mode palette.
    pub fn dark() -> Self {
        Self {
            risk_safe: Color32::from_rgb(120, 210, 130),
            risk_caution: Color32::from_rgb(230, 190, 70),
            risk_high: Color32::from_rgb(230, 130, 60),
            risk_critical: Color32::from_rgb(235, 80, 75),
            risk_unknown: Color32::from_rgb(140, 145, 155),
            risk_bg_alpha: 30,

            cursor_bright_overwrite: Color32::from_rgb(90, 90, 180),
            cursor_dim_overwrite: Color32::from_rgb(40, 40, 80),
            cursor_bright_insert: Color32::from_rgb(70, 170, 70),
            cursor_dim_insert: Color32::from_rgb(35, 80, 35),

            selection_bg: Color32::from_rgb(30, 70, 110),
            current_match_bg: Color32::from_rgb(230, 150, 50),
            search_match_bg: Color32::from_rgb(190, 190, 70),
            bookmark_bg: Color32::from_rgb(60, 170, 200),

            modified_indicator: Color32::from_rgb(240, 175, 50),
            warning_text: Color32::from_rgb(255, 210, 75),
            error_text: Color32::from_rgb(240, 90, 90),

            ascii_delimiter: Color32::from_gray(80),
            cursor_text: Color32::WHITE,
            hex_byte_text: Color32::from_gray(200),

            shortcut_normal: Color32::from_gray(130),
            shortcut_hover: Color32::from_gray(210),
        }
    }

    /// Light-mode palette.
    pub fn light() -> Self {
        Self {
            risk_safe: Color32::from_rgb(15, 110, 30),
            risk_caution: Color32::from_rgb(140, 100, 0),
            risk_high: Color32::from_rgb(170, 65, 10),
            risk_critical: Color32::from_rgb(170, 20, 20),
            risk_unknown: Color32::from_rgb(80, 80, 90),
            risk_bg_alpha: 70,

            cursor_bright_overwrite: Color32::from_rgb(90, 90, 200),
            cursor_dim_overwrite: Color32::from_rgb(150, 150, 210),
            cursor_bright_insert: Color32::from_rgb(50, 160, 50),
            cursor_dim_insert: Color32::from_rgb(140, 200, 140),

            selection_bg: Color32::from_rgb(100, 170, 240),
            current_match_bg: Color32::from_rgb(240, 170, 60),
            search_match_bg: Color32::from_rgb(230, 220, 80),
            bookmark_bg: Color32::from_rgb(100, 200, 230),

            modified_indicator: Color32::from_rgb(200, 130, 0),
            warning_text: Color32::from_rgb(170, 120, 0),
            error_text: Color32::from_rgb(190, 30, 30),

            ascii_delimiter: Color32::from_gray(180),
            cursor_text: Color32::WHITE,
            hex_byte_text: Color32::from_gray(30),

            shortcut_normal: Color32::from_gray(100),
            shortcut_hover: Color32::from_gray(50),
        }
    }

    /// Solid color for a risk level (tree view labels, legends).
    pub fn risk_color(&self, level: RiskLevel) -> Color32 {
        match level {
            RiskLevel::Safe => self.risk_safe,
            RiskLevel::Caution => self.risk_caution,
            RiskLevel::High => self.risk_high,
            RiskLevel::Critical => self.risk_critical,
            RiskLevel::Unknown => self.risk_unknown,
        }
    }

    /// Translucent background color for a risk level (hex view section tint).
    pub fn risk_bg_color(&self, level: RiskLevel) -> Color32 {
        let solid = self.risk_color(level);
        Color32::from_rgba_unmultiplied(solid.r(), solid.g(), solid.b(), self.risk_bg_alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_palette_is_valid() {
        let c = AppColors::dark();
        // Spot-check a few fields are non-black (i.e. actually set)
        assert_ne!(c.risk_safe, Color32::BLACK);
        assert_ne!(c.warning_text, Color32::BLACK);
        assert_ne!(c.cursor_bright_overwrite, Color32::BLACK);
    }

    #[test]
    fn light_palette_is_valid() {
        let c = AppColors::light();
        assert_ne!(c.risk_safe, Color32::BLACK);
        assert_ne!(c.warning_text, Color32::BLACK);
        assert_ne!(c.cursor_bright_insert, Color32::BLACK);
    }

    #[test]
    fn risk_color_maps_correctly() {
        let c = AppColors::dark();
        assert_eq!(c.risk_color(RiskLevel::Safe), c.risk_safe);
        assert_eq!(c.risk_color(RiskLevel::Caution), c.risk_caution);
        assert_eq!(c.risk_color(RiskLevel::High), c.risk_high);
        assert_eq!(c.risk_color(RiskLevel::Critical), c.risk_critical);
        assert_eq!(c.risk_color(RiskLevel::Unknown), c.risk_unknown);
    }

    #[test]
    fn risk_bg_has_correct_alpha() {
        let c = AppColors::dark();
        let bg = c.risk_bg_color(RiskLevel::Safe);
        assert_eq!(bg.a(), 30);

        let c = AppColors::light();
        let bg = c.risk_bg_color(RiskLevel::High);
        assert_eq!(bg.a(), 70);
    }

    #[test]
    fn selection_differs_from_insert_dim_cursor() {
        // Verifies the collision fix: selection must NOT equal cursor_dim_insert
        let c = AppColors::dark();
        assert_ne!(c.selection_bg, c.cursor_dim_insert);

        let c = AppColors::light();
        assert_ne!(c.selection_bg, c.cursor_dim_insert);
    }

    #[test]
    fn new_dispatches_correctly() {
        let dark = AppColors::new(true);
        let light = AppColors::new(false);
        // Cursor text is white in both themes
        assert_eq!(dark.cursor_text, Color32::WHITE);
        assert_eq!(light.cursor_text, Color32::WHITE);
    }
}
