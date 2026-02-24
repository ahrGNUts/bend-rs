//! Traits for image format parsing

use std::borrow::Cow;
use std::fmt;

/// Error returned when a parser cannot parse the given data.
#[derive(Debug, Clone)]
pub enum ParseError {
    /// The file signature does not match the expected format.
    InvalidSignature,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidSignature => write!(f, "Invalid file signature"),
        }
    }
}

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
    pub name: Cow<'static, str>,
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
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        start: usize,
        end: usize,
        risk: RiskLevel,
    ) -> Self {
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
    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, ParseError>;

    /// Check if this parser can handle the given data
    fn can_parse(&self, data: &[u8]) -> bool;
}
