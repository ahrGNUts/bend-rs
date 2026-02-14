//! JPEG file format parser
//!
//! JPEG structure:
//! - SOI (Start of Image): FF D8
//! - APP segments: metadata (JFIF, EXIF, etc.)
//! - DQT: Quantization tables
//! - DHT: Huffman tables
//! - SOF: Start of Frame (image dimensions)
//! - SOS: Start of Scan (compressed data follows)
//! - Entropy-coded data
//! - EOI (End of Image): FF D9

use super::traits::{FileSection, ImageFormat, RiskLevel};

/// JPEG format parser
pub struct JpegParser;

impl JpegParser {
    /// Read a big-endian u16 from data (JPEG uses big-endian)
    fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
        if offset + 2 > data.len() {
            return None;
        }
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }

    /// Get human-readable name for a marker
    fn marker_name(marker: u8) -> &'static str {
        match marker {
            0xD8 => "SOI (Start of Image)",
            0xD9 => "EOI (End of Image)",
            0xE0 => "APP0 (JFIF)",
            0xE1 => "APP1 (EXIF/XMP)",
            0xE2..=0xEF => "APPn (Application Data)",
            0xDB => "DQT (Quantization Table)",
            0xC4 => "DHT (Huffman Table)",
            0xC0 => "SOF0 (Baseline DCT)",
            0xC1 => "SOF1 (Extended Sequential)",
            0xC2 => "SOF2 (Progressive DCT)",
            0xC3..=0xCF => "SOFn (Start of Frame)",
            0xDA => "SOS (Start of Scan)",
            0xDD => "DRI (Restart Interval)",
            0xD0..=0xD7 => "RSTn (Restart Marker)",
            0xFE => "COM (Comment)",
            _ => "Unknown Marker",
        }
    }

    /// Get risk level for a marker type
    fn marker_risk(marker: u8) -> RiskLevel {
        match marker {
            0xD8 | 0xD9 => RiskLevel::Critical, // SOI/EOI
            0xDB => RiskLevel::High,            // Quantization tables
            0xC4 => RiskLevel::High,            // Huffman tables (before SOF range)
            0xC0..=0xCF => RiskLevel::Critical, // SOF markers
            0xDA => RiskLevel::High,            // SOS
            0xE0..=0xEF => RiskLevel::Caution,  // APP markers
            0xFE => RiskLevel::Safe,            // Comments
            0xDD => RiskLevel::Caution,         // DRI
            _ => RiskLevel::Caution,
        }
    }
}

impl ImageFormat for JpegParser {
    fn name(&self) -> &'static str {
        "JPEG"
    }

    fn can_parse(&self, data: &[u8]) -> bool {
        // JPEG files start with FF D8 FF
        data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF
    }

    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, String> {
        if !self.can_parse(data) {
            return Err("Not a valid JPEG file".to_string());
        }

        let mut sections = Vec::new();
        // SOI marker
        sections.push(
            FileSection::new("SOI (Start of Image)", 0, 2, RiskLevel::Critical)
                .with_description("JPEG magic bytes FF D8"),
        );
        let mut pos = 2;

        // Parse markers until we hit the entropy-coded data or EOF
        while pos < data.len() {
            // Look for marker (FF xx)
            if data[pos] != 0xFF {
                // We're in entropy-coded data or error
                break;
            }

            // Skip any padding FF bytes
            while pos < data.len() && data[pos] == 0xFF {
                pos += 1;
            }

            if pos >= data.len() {
                break;
            }

            let marker = data[pos];
            pos += 1;

            // Handle different marker types
            match marker {
                0x00 => {
                    // Escaped FF in data, skip
                    continue;
                }
                0xD8 => {
                    // SOI (shouldn't appear again)
                    continue;
                }
                0xD9 => {
                    // EOI
                    let start = pos - 2;
                    sections.push(
                        FileSection::new("EOI (End of Image)", start, pos, RiskLevel::Critical)
                            .with_description("End of JPEG data"),
                    );
                    break;
                }
                0xD0..=0xD7 => {
                    // Restart markers (no length)
                    continue;
                }
                0xDA => {
                    // SOS - Start of Scan
                    if pos + 2 > data.len() {
                        break;
                    }
                    let segment_len = Self::read_u16_be(data, pos).unwrap_or(0) as usize;
                    let segment_start = pos - 2;
                    let segment_end = pos + segment_len;

                    if segment_end > data.len() {
                        break;
                    }

                    sections.push(
                        FileSection::new(
                            "SOS (Start of Scan)",
                            segment_start,
                            segment_end,
                            RiskLevel::High,
                        )
                        .with_description("Scan header - marks start of compressed data"),
                    );

                    pos = segment_end;

                    // Everything after SOS until EOI is entropy-coded data
                    let entropy_start = pos;

                    // Find EOI
                    let mut entropy_end = data.len();
                    for i in pos..data.len().saturating_sub(1) {
                        if data[i] == 0xFF && data[i + 1] == 0xD9 {
                            entropy_end = i;
                            break;
                        }
                    }

                    if entropy_end > entropy_start {
                        sections.push(
                            FileSection::new(
                                "Entropy-Coded Data",
                                entropy_start,
                                entropy_end,
                                RiskLevel::High,
                            )
                            .with_description(
                                "Compressed image data - editing here creates glitch effects but often corrupts the image",
                            ),
                        );
                    }

                    pos = entropy_end;
                }
                _ => {
                    // Markers with length field
                    if pos + 2 > data.len() {
                        break;
                    }

                    let segment_len = Self::read_u16_be(data, pos).unwrap_or(0) as usize;
                    let segment_start = pos - 2;
                    let segment_end = pos + segment_len;

                    if segment_end > data.len() {
                        break;
                    }

                    let name = Self::marker_name(marker);
                    let risk = Self::marker_risk(marker);

                    let mut section = FileSection::new(name, segment_start, segment_end, risk);

                    // Add descriptions for common markers
                    section.description = Some(match marker {
                        0xE0 => "JFIF application data",
                        0xE1 => "EXIF metadata or XMP data",
                        0xDB => "Quantization tables - affects image quality",
                        0xC4 => "Huffman tables - required for decoding",
                        0xC0 => "Image dimensions and color components",
                        0xFE => "Comment - safe to edit",
                        _ => "",
                    }.to_string());

                    sections.push(section);
                    pos = segment_end;
                }
            }
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_parse_jpeg() {
        let parser = JpegParser;

        // Valid JPEG signature
        assert!(parser.can_parse(&[0xFF, 0xD8, 0xFF, 0xE0]));

        // Invalid signatures
        assert!(!parser.can_parse(&[]));
        assert!(!parser.can_parse(&[0xFF]));
        assert!(!parser.can_parse(&[0xFF, 0xD8]));
        assert!(!parser.can_parse(b"BMP"));
    }

    #[test]
    fn test_parse_minimal_jpeg() {
        let parser = JpegParser;

        // Minimal JPEG: SOI + APP0 + EOI
        let jpeg = vec![
            0xFF, 0xD8,             // SOI
            0xFF, 0xE0, 0x00, 0x10, // APP0 marker with length 16
            0x4A, 0x46, 0x49, 0x46, 0x00, // "JFIF\0"
            0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, // JFIF data
            0xFF, 0xD9,             // EOI
        ];

        let sections = parser.parse(&jpeg).unwrap();

        assert!(sections.len() >= 2); // At least SOI and APP0
        assert_eq!(sections[0].name, "SOI (Start of Image)");
        assert!(sections[1].name.contains("APP0"));
    }
}
