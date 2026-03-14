//! Centralized, theme-aware color palette for the application.
//!
//! Every custom color used in the UI lives here. Render sites construct
//! `AppColors::new(dark_mode)` and read fields instead of using inline RGB literals.

use crate::formats::RiskLevel;
use eframe::egui;
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
    pub cursor_text: Color32,
    /// Text color for hex bytes that sit on a tinted background
    pub hex_byte_text: Color32,

    // -- Menu shortcut text --
    pub shortcut_normal: Color32,
    pub shortcut_hover: Color32,

    // -- Surface & accent --
    pub accent: Color32,
    pub accent_muted: Color32,
    pub bg_base: Color32,
    pub bg_surface: Color32,
    pub bg_elevated: Color32,
    pub border: Color32,
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
            risk_safe: Color32::from_rgb(110, 207, 128),
            risk_caution: Color32::from_rgb(224, 184, 64),
            risk_high: Color32::from_rgb(224, 128, 64),
            risk_critical: Color32::from_rgb(224, 85, 85),
            risk_unknown: Color32::from_rgb(128, 136, 152),
            risk_bg_alpha: 30,

            cursor_bright_overwrite: Color32::from_rgb(96, 96, 192),
            cursor_dim_overwrite: Color32::from_rgb(48, 48, 96),
            cursor_bright_insert: Color32::from_rgb(80, 176, 80),
            cursor_dim_insert: Color32::from_rgb(40, 80, 40),

            selection_bg: Color32::from_rgb(30, 72, 112),
            current_match_bg: Color32::from_rgb(212, 144, 48),
            search_match_bg: Color32::from_rgb(160, 160, 64),
            bookmark_bg: Color32::from_rgb(64, 168, 192),

            modified_indicator: Color32::from_rgb(224, 168, 48),
            warning_text: Color32::from_rgb(240, 200, 80),
            error_text: Color32::from_rgb(224, 96, 96),

            cursor_text: Color32::WHITE,
            hex_byte_text: Color32::from_rgb(200, 206, 216),

            shortcut_normal: Color32::from_rgb(85, 93, 110),
            shortcut_hover: Color32::from_rgb(200, 206, 216),

            accent: Color32::from_rgb(78, 201, 176),
            accent_muted: Color32::from_rgb(36, 50, 56),
            bg_base: Color32::from_rgb(19, 22, 27),
            bg_surface: Color32::from_rgb(26, 30, 37),
            bg_elevated: Color32::from_rgb(34, 39, 47),
            border: Color32::from_rgb(46, 52, 64),
        }
    }

    /// Light-mode palette.
    pub fn light() -> Self {
        Self {
            risk_safe: Color32::from_rgb(24, 136, 56),
            risk_caution: Color32::from_rgb(154, 120, 0),
            risk_high: Color32::from_rgb(192, 80, 16),
            risk_critical: Color32::from_rgb(184, 32, 32),
            risk_unknown: Color32::from_rgb(96, 104, 120),
            risk_bg_alpha: 70,

            cursor_bright_overwrite: Color32::from_rgb(88, 88, 192),
            cursor_dim_overwrite: Color32::from_rgb(160, 160, 208),
            cursor_bright_insert: Color32::from_rgb(56, 160, 56),
            cursor_dim_insert: Color32::from_rgb(144, 200, 144),

            selection_bg: Color32::from_rgb(104, 168, 224),
            current_match_bg: Color32::from_rgb(232, 168, 64),
            search_match_bg: Color32::from_rgb(216, 208, 80),
            bookmark_bg: Color32::from_rgb(96, 192, 216),

            modified_indicator: Color32::from_rgb(192, 136, 0),
            warning_text: Color32::from_rgb(168, 128, 0),
            error_text: Color32::from_rgb(184, 40, 40),

            cursor_text: Color32::WHITE,
            hex_byte_text: Color32::from_rgb(40, 44, 52),

            shortcut_normal: Color32::from_rgb(139, 142, 150),
            shortcut_hover: Color32::from_rgb(64, 69, 80),

            accent: Color32::from_rgb(59, 130, 246),
            accent_muted: Color32::from_rgb(213, 228, 250),
            bg_base: Color32::from_rgb(248, 246, 242),
            bg_surface: Color32::from_rgb(255, 255, 255),
            bg_elevated: Color32::from_rgb(255, 255, 255),
            border: Color32::from_rgb(224, 221, 216),
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

    /// Apply surface/accent colors to egui's built-in Visuals.
    pub fn apply_to_visuals(&self, visuals: &mut egui::Visuals) {
        visuals.panel_fill = self.bg_surface;
        visuals.window_fill = self.bg_elevated;
        visuals.extreme_bg_color = self.bg_base;
        visuals.faint_bg_color = self.bg_surface;
        visuals.code_bg_color = self.bg_base;

        visuals.window_stroke = egui::Stroke::new(1.0, self.border);

        // Widget backgrounds and borders matched to our surfaces
        let w = &mut visuals.widgets;
        w.noninteractive.weak_bg_fill = self.bg_surface;
        w.noninteractive.bg_fill = self.bg_surface;
        w.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.border);

        // Adjust noninteractive text (weak_text_color / default labels) for
        // contrast against our custom surfaces. egui defaults are gray(140)
        // for dark and gray(80) for light.
        if visuals.dark_mode {
            w.noninteractive.fg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(155, 162, 176));
        } else {
            w.noninteractive.fg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(50, 54, 62));
        }

        visuals.selection.bg_fill = self.accent_muted;
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.hyperlink_color = self.accent;

        visuals.warn_fg_color = self.warning_text;
        visuals.error_fg_color = self.error_text;
    }
}

/// Apply the custom palette to both dark and light egui styles.
/// Call once at startup; survives subsequent `set_theme()` calls.
pub fn apply_custom_visuals(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        // Bump the small text size from egui's default 10.0 to 12.0
        // for better readability in the structure tree and elsewhere.
        style
            .text_styles
            .entry(egui::TextStyle::Small)
            .and_modify(|fd| fd.size = 12.0);
    });
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        AppColors::dark().apply_to_visuals(&mut style.visuals);
    });
    ctx.style_mut_of(egui::Theme::Light, |style| {
        AppColors::light().apply_to_visuals(&mut style.visuals);
    });
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

    #[test]
    fn dark_surface_colors_are_set() {
        let c = AppColors::dark();
        assert_ne!(c.bg_base, Color32::BLACK);
        assert_ne!(c.bg_surface, Color32::BLACK);
        assert_ne!(c.bg_elevated, Color32::BLACK);
        assert_ne!(c.accent, Color32::BLACK);
        assert_ne!(c.border, Color32::BLACK);
    }

    #[test]
    fn light_surface_colors_are_set() {
        let c = AppColors::light();
        assert_ne!(c.bg_base, Color32::BLACK);
        assert_ne!(c.bg_surface, Color32::BLACK);
        assert_ne!(c.accent, Color32::BLACK);
        assert_ne!(c.border, Color32::BLACK);
    }

    #[test]
    fn apply_to_visuals_sets_panel_fill() {
        let c = AppColors::dark();
        let mut visuals = egui::Visuals::dark();
        let before = visuals.panel_fill;
        c.apply_to_visuals(&mut visuals);
        assert_eq!(visuals.panel_fill, c.bg_surface);
        assert_ne!(visuals.panel_fill, before);
    }

    #[test]
    fn dark_and_light_accents_differ() {
        let dark = AppColors::dark();
        let light = AppColors::light();
        assert_ne!(dark.accent, light.accent);
        assert_ne!(dark.bg_base, light.bg_base);
    }
}
