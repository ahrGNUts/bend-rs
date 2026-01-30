//! Image format parsing for structure visualization
//!
//! This module provides format-specific parsing to identify file structure
//! (headers, metadata, pixel data) for visualization and safe editing zones.

mod bmp;
mod jpeg;
pub mod traits;

pub use bmp::BmpParser;
pub use jpeg::JpegParser;
pub use traits::{FileSection, ImageFormat, RiskLevel};

/// Detect the format of a file and return the appropriate parser
pub fn detect_format(data: &[u8]) -> Option<Box<dyn ImageFormat>> {
    let bmp = BmpParser;
    if bmp.can_parse(data) {
        return Some(Box::new(bmp));
    }

    let jpeg = JpegParser;
    if jpeg.can_parse(data) {
        return Some(Box::new(jpeg));
    }

    None
}

/// Parse a file and return its sections
pub fn parse_file(data: &[u8]) -> Option<Vec<FileSection>> {
    let parser = detect_format(data)?;
    parser.parse(data).ok()
}
