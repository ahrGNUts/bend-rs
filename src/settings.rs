//! Application settings persistence
//!
//! Settings are stored in a JSON file at the platform-appropriate config location:
//! - macOS: ~/Library/Application Support/bend-rs/settings.json
//! - Windows: %APPDATA%/bend-rs/settings.json
//! - Linux: ~/.config/bend-rs/settings.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Maximum number of recent files to track
const MAX_RECENT_FILES: usize = 10;

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            window_width: 1200.0,
            window_height: 800.0,
            recent_files: Vec::new(),
            default_header_protection: false,
            show_high_risk_warnings: true,
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
            Ok(contents) => {
                match serde_json::from_str(&contents) {
                    Ok(settings) => {
                        log::info!("Loaded settings from {}", path.display());
                        settings
                    }
                    Err(e) => {
                        log::warn!("Failed to parse settings file: {}, using defaults", e);
                        Self::default()
                    }
                }
            }
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
    }

    #[test]
    fn test_add_recent_file() {
        let mut settings = AppSettings::default();

        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file2.bmp"));

        assert_eq!(settings.recent_files.len(), 2);
        assert_eq!(settings.recent_files[0], PathBuf::from("/path/to/file2.bmp"));
        assert_eq!(settings.recent_files[1], PathBuf::from("/path/to/file1.bmp"));
    }

    #[test]
    fn test_add_recent_file_moves_to_front() {
        let mut settings = AppSettings::default();

        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file2.bmp"));
        settings.add_recent_file(PathBuf::from("/path/to/file1.bmp")); // Re-add file1

        assert_eq!(settings.recent_files.len(), 2);
        assert_eq!(settings.recent_files[0], PathBuf::from("/path/to/file1.bmp"));
        assert_eq!(settings.recent_files[1], PathBuf::from("/path/to/file2.bmp"));
    }

    #[test]
    fn test_recent_files_max_limit() {
        let mut settings = AppSettings::default();

        for i in 0..15 {
            settings.add_recent_file(PathBuf::from(format!("/path/to/file{}.bmp", i)));
        }

        assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
        // Most recent should be at front
        assert_eq!(settings.recent_files[0], PathBuf::from("/path/to/file14.bmp"));
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
}
