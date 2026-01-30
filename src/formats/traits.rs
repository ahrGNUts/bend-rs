//! Traits for image format parsing

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

    /// Size of this section in bytes
    pub fn size(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Trait for image format parsers
pub trait ImageFormat {
    /// Get the format name (e.g., "BMP", "JPEG")
    fn name(&self) -> &'static str;

    /// Parse the file structure and return sections
    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, String>;

    /// Check if this parser can handle the given data
    fn can_parse(&self, data: &[u8]) -> bool;
}
