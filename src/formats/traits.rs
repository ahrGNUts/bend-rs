//! Traits for image format parsing

use eframe::egui;

/// Risk level for editing a section
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe to edit - won't break file structure
    Safe,
    /// Caution - may affect image appearance but won't corrupt file
    Caution,
    /// High risk - likely to corrupt the file or make it unreadable
    High,
    /// Critical - editing will almost certainly break the file
    Critical,
    /// Unknown - unrecognized data region (not a header, no special protection)
    Unknown,
}

impl RiskLevel {
    /// Get the solid color for this risk level (for UI elements like tree view)
    pub fn color(self) -> egui::Color32 {
        match self {
            RiskLevel::Safe => egui::Color32::from_rgb(100, 200, 100), // Green
            RiskLevel::Caution => egui::Color32::from_rgb(200, 180, 80), // Yellow
            RiskLevel::High => egui::Color32::from_rgb(200, 130, 80),  // Orange
            RiskLevel::Critical => egui::Color32::from_rgb(200, 80, 80), // Red
            RiskLevel::Unknown => egui::Color32::from_rgb(150, 150, 150), // Gray
        }
    }

    /// Get the background color for this risk level (with alpha for hex view)
    pub fn background_color(self) -> egui::Color32 {
        match self {
            RiskLevel::Safe => egui::Color32::from_rgba_unmultiplied(100, 200, 100, 50),
            RiskLevel::Caution => egui::Color32::from_rgba_unmultiplied(200, 180, 80, 50),
            RiskLevel::High => egui::Color32::from_rgba_unmultiplied(200, 130, 80, 50),
            RiskLevel::Critical => egui::Color32::from_rgba_unmultiplied(200, 80, 80, 50),
            RiskLevel::Unknown => egui::Color32::from_rgba_unmultiplied(150, 150, 150, 50),
        }
    }

    /// Get a human-readable label for this risk level
    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::Safe => "Safe",
            RiskLevel::Caution => "Caution",
            RiskLevel::High => "High Risk",
            RiskLevel::Critical => "Critical",
            RiskLevel::Unknown => "Unknown",
        }
    }
}

/// A section of the file with metadata
#[derive(Clone, Debug)]
pub struct FileSection {
    /// Human-readable name for this section
    pub name: String,
    /// Start offset in bytes
    pub start: usize,
    /// End offset in bytes (exclusive)
    pub end: usize,
    /// Risk level for editing this section
    pub risk: RiskLevel,
    /// Optional description
    pub description: Option<String>,
    /// Child sections (for nested structures)
    pub children: Vec<FileSection>,
}

impl FileSection {
    /// Create a new section
    pub fn new(name: impl Into<String>, start: usize, end: usize, risk: RiskLevel) -> Self {
        Self {
            name: name.into(),
            start,
            end,
            risk,
            description: None,
            children: Vec::new(),
        }
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a child section
    pub fn with_child(mut self, child: FileSection) -> Self {
        self.children.push(child);
        self
    }
}

/// Trait for image format parsers
pub trait ImageFormat {
    /// Parse the file structure and return sections
    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, String>;

    /// Check if this parser can handle the given data
    fn can_parse(&self, data: &[u8]) -> bool;
}
