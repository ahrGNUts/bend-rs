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

/// Fill gaps between parsed sections with "Unknown" sections.
///
/// Sorts sections by start offset, then inserts `RiskLevel::Unknown` sections
/// for any byte ranges not covered by existing sections.
pub fn fill_gaps(sections: &mut Vec<FileSection>, data_len: usize) {
    // Sort by start offset
    sections.sort_by_key(|s| s.start);

    let mut gap_sections = Vec::new();
    let mut covered_up_to: usize = 0;

    for section in sections.iter() {
        if section.start > covered_up_to {
            gap_sections.push(FileSection::new(
                "Unknown",
                covered_up_to,
                section.start,
                RiskLevel::Unknown,
            ));
        }
        if section.end > covered_up_to {
            covered_up_to = section.end;
        }
    }

    // Handle trailing gap
    if covered_up_to < data_len {
        gap_sections.push(FileSection::new(
            "Unknown",
            covered_up_to,
            data_len,
            RiskLevel::Unknown,
        ));
    }

    sections.append(&mut gap_sections);
    sections.sort_by_key(|s| s.start);
}

/// Parse a file and return its sections
pub fn parse_file(data: &[u8]) -> Option<Vec<FileSection>> {
    let parser = detect_format(data)?;
    match parser.parse(data) {
        Ok(mut sections) => {
            fill_gaps(&mut sections, data.len());
            Some(sections)
        }
        Err(_) => {
            // Parser recognized the format but couldn't parse it;
            // return a single Unknown section covering the entire file
            Some(vec![FileSection::new(
                "Unknown",
                0,
                data.len(),
                RiskLevel::Unknown,
            )])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_gaps_no_gaps() {
        let mut sections = vec![
            FileSection::new("A", 0, 10, RiskLevel::Safe),
            FileSection::new("B", 10, 20, RiskLevel::Safe),
        ];
        fill_gaps(&mut sections, 20);
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_fill_gaps_leading_gap() {
        let mut sections = vec![FileSection::new("A", 5, 10, RiskLevel::Safe)];
        fill_gaps(&mut sections, 10);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Unknown");
        assert_eq!(sections[0].start, 0);
        assert_eq!(sections[0].end, 5);
        assert_eq!(sections[0].risk, RiskLevel::Unknown);
    }

    #[test]
    fn test_fill_gaps_trailing_gap() {
        let mut sections = vec![FileSection::new("A", 0, 10, RiskLevel::Safe)];
        fill_gaps(&mut sections, 20);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[1].name, "Unknown");
        assert_eq!(sections[1].start, 10);
        assert_eq!(sections[1].end, 20);
    }

    #[test]
    fn test_fill_gaps_middle_gap() {
        let mut sections = vec![
            FileSection::new("A", 0, 5, RiskLevel::Safe),
            FileSection::new("B", 15, 20, RiskLevel::Safe),
        ];
        fill_gaps(&mut sections, 20);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[1].name, "Unknown");
        assert_eq!(sections[1].start, 5);
        assert_eq!(sections[1].end, 15);
    }

    #[test]
    fn test_fill_gaps_empty_sections() {
        let mut sections = Vec::new();
        fill_gaps(&mut sections, 100);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Unknown");
        assert_eq!(sections[0].start, 0);
        assert_eq!(sections[0].end, 100);
    }

    #[test]
    fn test_fill_gaps_multiple_gaps() {
        let mut sections = vec![
            FileSection::new("A", 5, 10, RiskLevel::Safe),
            FileSection::new("B", 20, 30, RiskLevel::Safe),
        ];
        fill_gaps(&mut sections, 40);
        // Should have: Unknown(0-5), A(5-10), Unknown(10-20), B(20-30), Unknown(30-40)
        assert_eq!(sections.len(), 5);
        assert_eq!(sections[0].name, "Unknown");
        assert_eq!(sections[2].name, "Unknown");
        assert_eq!(sections[4].name, "Unknown");
    }

    #[test]
    fn test_parse_file_unrecognized_format_returns_none() {
        // Random bytes that don't match BMP or JPEG
        let data = vec![0x00, 0x01, 0x02, 0x03];
        assert!(parse_file(&data).is_none());
    }

    #[test]
    fn test_parse_file_bmp_truncated_returns_partial_with_unknown() {
        // Valid BMP signature but truncated (only 10 bytes — not enough for full header)
        let mut data = vec![0u8; 10];
        data[0] = b'B';
        data[1] = b'M';
        let sections = parse_file(&data);
        assert!(sections.is_some());
        let sections = sections.unwrap();
        // Should have at least an Unknown section covering the file
        assert!(sections.iter().any(|s| s.risk == RiskLevel::Unknown));
    }

    #[test]
    fn test_parse_file_bmp_truncated_dib_returns_partial() {
        // BMP with valid file header but DIB header extends beyond file
        let mut data = vec![0u8; 20];
        data[0] = b'B';
        data[1] = b'M';
        // File size
        data[2] = 20;
        // Pixel data offset = 54
        data[10] = 54;
        // DIB header size = 40 (but file is only 20 bytes, so DIB extends beyond)
        data[14] = 40;

        let sections = parse_file(&data);
        assert!(sections.is_some());
        let sections = sections.unwrap();
        // Should have File Header (0-14) + Unknown (14-20) from fill_gaps
        assert!(sections.iter().any(|s| s.name == "File Header"));
        assert!(sections.iter().any(|s| s.name == "Unknown"));
    }
}
