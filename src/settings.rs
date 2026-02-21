//! Application settings persistence
//!
//! Settings are stored in a JSON file at the platform-appropriate config location:
//! - macOS: ~/Library/Application Support/bend-rs/settings.json
//! - Windows: %APPDATA%/bend-rs/settings.json
//! - Linux: ~/.config/bend-rs/settings.json

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// User preference for application theme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemePreference {
    Dark,
    Light,
    #[default]
    System,
}

impl fmt::Display for ThemePreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemePreference::Dark => write!(f, "Dark"),
            ThemePreference::Light => write!(f, "Light"),
            ThemePreference::System => write!(f, "System"),
        }
    }
}

impl From<ThemePreference> for egui::ThemePreference {
    fn from(pref: ThemePreference) -> Self {
        match pref {
            ThemePreference::Dark => egui::ThemePreference::Dark,
            ThemePreference::Light => egui::ThemePreference::Light,
            ThemePreference::System => egui::ThemePreference::System,
        }
    }
}

impl From<egui::ThemePreference> for ThemePreference {
    fn from(pref: egui::ThemePreference) -> Self {
        match pref {
            egui::ThemePreference::Dark => ThemePreference::Dark,
            egui::ThemePreference::Light => ThemePreference::Light,
            egui::ThemePreference::System => ThemePreference::System,
        }
    }
}

impl ThemePreference {
    /// Apply this theme preference to the given egui context
    pub fn apply(self, ctx: &egui::Context) {
        ctx.set_theme(egui::ThemePreference::from(self));
    }
}

/// Maximum number of recent files to track
const MAX_RECENT_FILES: usize = 10;

/// Application settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    /// Window width in logical pixels
    pub window_width: f32,

    /// Window height in logical pixels
    pub window_height: f32,

    /// List of recently opened files (most recent first)
    pub recent_files: Vec<PathBuf>,

    /// Whether header protection is enabled by default
    pub default_header_protection: bool,

    /// Whether to show high-risk edit warnings
    pub show_high_risk_warnings: bool,

    /// Theme preference (Dark, Light, or System)
    #[serde(default)]
    pub theme: ThemePreference,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            window_width: 1200.0,
            window_height: 800.0,
            recent_files: Vec::new(),
            default_header_protection: false,
            show_high_risk_warnings: true,
            theme: ThemePreference::default(),
        }
    }
}

impl AppSettings {
    /// Get the path to the settings file
    fn settings_path() -> Option<PathBuf> {
        dirs::config_dir().map(|mut path| {
            path.push("bend-rs");
            path.push("settings.json");
            path
        })
    }

    /// Load settings from disk, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = Self::settings_path() else {
            log::warn!("Could not determine config directory, using defaults");
            return Self::default();
        };

        match std::fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(settings) => {
                    log::info!("Loaded settings from {}", path.display());
                    settings
                }
                Err(e) => {
                    log::warn!("Failed to parse settings file: {}, using defaults", e);
                    Self::default()
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to read settings file: {}", e);
                }
                Self::default()
            }
        }
    }

    /// Save settings to disk
    pub fn save(&self) {
        let Some(path) = Self::settings_path() else {
            log::warn!("Could not determine config directory, settings not saved");
            return;
        };

        // Ensure the config directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("Failed to create config directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(self) {
            Ok(contents) => {
                if let Err(e) = std::fs::write(&path, contents) {
                    log::warn!("Failed to write settings file: {}", e);
                } else {
                    log::info!("Saved settings to {}", path.display());
                }
            }
            Err(e) => {
                log::warn!("Failed to serialize settings: {}", e);
            }
        }
    }

    /// Add a file to the recent files list
    pub fn add_recent_file(&mut self, path: PathBuf) {
        // Remove if already in list (to move to front)
        self.recent_files.retain(|p| p != &path);

        // Add to front
        self.recent_files.insert(0, path);

        // Trim to max size
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    /// Get the recent files list
    pub fn recent_files(&self) -> &[PathBuf] {
        &self.recent_files
    }

    /// Clear the recent files list
    pub fn clear_recent_files(&mut self) {
        self.recent_files.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.window_width, 1200.0);
        assert_eq!(settings.window_height, 800.0);
        assert!(settings.recent_files.is_empty());
        assert!(!settings.default_header_protection);
        assert!(settings.show_high_risk_warnings);
        assert_eq!(settings.theme, ThemePreference::System);
    }

    #[test]
    fn test_add_recent_file() {
        let mut settings = AppSettings::default();

        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file2.bmp"));

        assert_eq!(settings.recent_files.len(), 2);
        assert_eq!(
            settings.recent_files[0],
            PathBuf::from("/path/to/file2.bmp")
        );
        assert_eq!(
            settings.recent_files[1],
            PathBuf::from("/path/to/file1.bmp")
        );
    }

    #[test]
    fn test_add_recent_file_moves_to_front() {
        let mut settings = AppSettings::default();

        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file2.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp")); // Re-add file1

        assert_eq!(settings.recent_files.len(), 2);
        assert_eq!(
            settings.recent_files[0],
            PathBuf::from("/path/to/file1.bmp")
        );
        assert_eq!(
            settings.recent_files[1],
            PathBuf::from("/path/to/file2.bmp")
        );
    }

    #[test]
    fn test_recent_files_max_limit() {
        let mut settings = AppSettings::default();

        for i in 0..15 {
            settings.add_recent_file(PathBuf::from(format!("/path/to/file{}.bmp", i)));
        }

        assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
        // Most recent should be at front
        assert_eq!(
            settings.recent_files[0],
            PathBuf::from("/path/to/file14.bmp")
        );
    }

    #[test]
    fn test_clear_recent_files() {
        let mut settings = AppSettings::default();
        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp"));
        settings.clear_recent_files();
        assert!(settings.recent_files.is_empty());
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut settings = AppSettings::default();
        settings.window_width = 1000.0;
        settings.add_recent_file(PathBuf::from("/path/to/file.bmp"));

        let json = serde_json::to_string(&settings).unwrap();
        let loaded: AppSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.window_width, 1000.0);
        assert_eq!(loaded.recent_files.len(), 1);
    }

    #[test]
    fn test_theme_preference_default_is_system() {
        assert_eq!(ThemePreference::default(), ThemePreference::System);
    }

    #[test]
    fn test_theme_preference_to_egui() {
        let dark: egui::ThemePreference = ThemePreference::Dark.into();
        assert_eq!(dark, egui::ThemePreference::Dark);
        let light: egui::ThemePreference = ThemePreference::Light.into();
        assert_eq!(light, egui::ThemePreference::Light);
        let system: egui::ThemePreference = ThemePreference::System.into();
        assert_eq!(system, egui::ThemePreference::System);
    }

    #[test]
    fn test_egui_theme_to_theme_preference() {
        let dark: ThemePreference = egui::ThemePreference::Dark.into();
        assert_eq!(dark, ThemePreference::Dark);
        let light: ThemePreference = egui::ThemePreference::Light.into();
        assert_eq!(light, ThemePreference::Light);
        let system: ThemePreference = egui::ThemePreference::System.into();
        assert_eq!(system, ThemePreference::System);
    }

    #[test]
    fn test_theme_preference_display() {
        assert_eq!(ThemePreference::Dark.to_string(), "Dark");
        assert_eq!(ThemePreference::Light.to_string(), "Light");
        assert_eq!(ThemePreference::System.to_string(), "System");
    }

    #[test]
    fn test_app_settings_partial_eq() {
        let a = AppSettings::default();
        let b = AppSettings::default();
        assert_eq!(a, b);

        let mut c = AppSettings::default();
        c.window_width = 999.0;
        assert_ne!(a, c);
    }

    #[test]
    fn test_theme_round_trip_serialization() {
        for theme in [
            ThemePreference::Dark,
            ThemePreference::Light,
            ThemePreference::System,
        ] {
            let mut settings = AppSettings::default();
            settings.theme = theme;
            let json = serde_json::to_string(&settings).unwrap();
            let loaded: AppSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded.theme, theme);
        }
    }

    #[test]
    fn test_theme_backward_compat_missing_key() {
        // Simulate a settings.json from before theme was added
        let json = r#"{
            "window_width": 1200.0,
            "window_height": 800.0,
            "recent_files": [],
            "default_header_protection": false,
            "show_high_risk_warnings": true
        }"#;
        let loaded: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.theme, ThemePreference::System);
    }
}
