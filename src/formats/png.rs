//! PNG file format parser
//!
//! PNG structure:
//! - Signature: 8 bytes (89 50 4E 47 0D 0A 1A 0A)
//! - Chunks: each chunk has Length (4 bytes) + Type (4 bytes) + Data (Length bytes) + CRC (4 bytes)
//! - Critical chunks: IHDR (must be first), PLTE, IDAT, IEND (must be last)
//! - Ancillary chunks: tEXt, zTXt, iTXt, gAMA, cHRM, sRGB, iCCP, bKGD, pHYs, tIME, tRNS

use super::traits::{FileSection, ImageFormat, RiskLevel};

/// PNG signature bytes
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// PNG format parser
pub struct PngParser;

impl PngParser {
    /// Read a big-endian u32 from data
    fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }

    /// Convert 4 bytes to a chunk type string
    fn chunk_type_str(data: &[u8], offset: usize) -> Option<String> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(String::from_utf8_lossy(&data[offset..offset + 4]).to_string())
    }

    /// Get human-readable name for a chunk type
    fn chunk_name(chunk_type: &str) -> &'static str {
        match chunk_type {
            "IHDR" => "IHDR (Image Header)",
            "PLTE" => "PLTE (Palette)",
            "IDAT" => "IDAT (Image Data)",
            "IEND" => "IEND (Image End)",
            "tEXt" => "tEXt (Text)",
            "zTXt" => "zTXt (Compressed Text)",
            "iTXt" => "iTXt (International Text)",
            "gAMA" => "gAMA (Gamma)",
            "cHRM" => "cHRM (Chromaticities)",
            "sRGB" => "sRGB (Standard RGB)",
            "iCCP" => "iCCP (ICC Profile)",
            "bKGD" => "bKGD (Background)",
            "pHYs" => "pHYs (Physical Dimensions)",
            "tIME" => "tIME (Timestamp)",
            "tRNS" => "tRNS (Transparency)",
            _ => "Unknown Chunk",
        }
    }

    /// Get risk level for a chunk type
    fn chunk_risk(chunk_type: &str) -> RiskLevel {
        match chunk_type {
            "IHDR" | "IEND" => RiskLevel::Critical,
            "PLTE" => RiskLevel::High,
            "IDAT" => RiskLevel::Caution,
            "gAMA" | "cHRM" | "sRGB" | "iCCP" | "bKGD" | "tRNS" => RiskLevel::Caution,
            "tEXt" | "zTXt" | "iTXt" | "tIME" | "pHYs" => RiskLevel::Safe,
            _ => RiskLevel::Unknown,
        }
    }

    /// Get description for a chunk type
    fn chunk_description(chunk_type: &str) -> &'static str {
        match chunk_type {
            "IHDR" => "Image dimensions, bit depth, and color type",
            "PLTE" => "Palette data for indexed-color images",
            "IDAT" => "Compressed image data - the fun part to glitch",
            "IEND" => "End of PNG data stream",
            "tEXt" => "Uncompressed text metadata - safe to edit",
            "zTXt" => "Compressed text metadata",
            "iTXt" => "International text metadata",
            "gAMA" => "Image gamma value",
            "cHRM" => "Primary chromaticities and white point",
            "sRGB" => "Standard RGB color space indicator",
            "iCCP" => "Embedded ICC profile",
            "bKGD" => "Default background color",
            "pHYs" => "Physical pixel dimensions",
            "tIME" => "Last modification time",
            "tRNS" => "Transparency information",
            _ => "",
        }
    }

    /// Build child sections for IHDR chunk data (13 bytes)
    fn ihdr_children(data_start: usize, data_len: usize) -> Vec<FileSection> {
        if data_len < 13 {
            return Vec::new();
        }
        vec![
            FileSection::new("Width", data_start, data_start + 4, RiskLevel::Critical),
            FileSection::new(
                "Height",
                data_start + 4,
                data_start + 8,
                RiskLevel::Critical,
            ),
            FileSection::new(
                "Bit Depth",
                data_start + 8,
                data_start + 9,
                RiskLevel::Critical,
            ),
            FileSection::new(
                "Color Type",
                data_start + 9,
                data_start + 10,
                RiskLevel::Critical,
            ),
            FileSection::new(
                "Compression",
                data_start + 10,
                data_start + 11,
                RiskLevel::Critical,
            ),
            FileSection::new(
                "Filter Method",
                data_start + 11,
                data_start + 12,
                RiskLevel::Critical,
            ),
            FileSection::new(
                "Interlace",
                data_start + 12,
                data_start + 13,
                RiskLevel::Critical,
            ),
        ]
    }
}

impl ImageFormat for PngParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        data.len() >= 8 && data[..8] == PNG_SIGNATURE
    }

    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, String> {
        if !self.can_parse(data) {
            return Err("Not a valid PNG file".to_string());
        }

        let mut sections = Vec::new();

        // PNG Signature (8 bytes)
        sections.push(
            FileSection::new("PNG Signature", 0, 8, RiskLevel::Critical)
                .with_description("PNG magic bytes"),
        );

        let mut pos = 8;

        // Parse chunks
        while pos < data.len() {
            let chunk_start = pos;

            // Read chunk length (4 bytes)
            let Some(data_length) = Self::read_u32_be(data, pos) else {
                // Truncated: can't read length
                break;
            };
            let data_length = data_length as usize;
            pos += 4;

            // Read chunk type (4 bytes)
            let Some(chunk_type) = Self::chunk_type_str(data, pos) else {
                // Truncated: can't read type
                break;
            };
            pos += 4;

            let data_start = pos;
            let chunk_end_expected = data_start + data_length + 4; // data + CRC

            // Determine if the chunk is truncated
            let is_truncated = chunk_end_expected > data.len();
            let chunk_end = if is_truncated {
                data.len()
            } else {
                chunk_end_expected
            };

            let name = Self::chunk_name(&chunk_type);
            let risk = Self::chunk_risk(&chunk_type);
            let desc = Self::chunk_description(&chunk_type);

            let mut section = FileSection::new(name, chunk_start, chunk_end, risk);
            if !desc.is_empty() {
                section = section.with_description(desc);
            }
            if is_truncated {
                section = section
                    .with_description(format!("{} (truncated)", desc).trim_start().to_string());
            }

            // Add structural child sections: Length, Type, Data, CRC
            section = section
                .with_child(FileSection::new(
                    "Length",
                    chunk_start,
                    chunk_start + 4,
                    RiskLevel::Critical,
                ))
                .with_child(FileSection::new(
                    "Type",
                    chunk_start + 4,
                    chunk_start + 8,
                    RiskLevel::Critical,
                ));

            // Data child (if there's data)
            let actual_data_len = if is_truncated {
                // All remaining bytes after type are data (no CRC)
                chunk_end.saturating_sub(data_start)
            } else {
                data_length
            };

            if actual_data_len > 0 {
                let mut data_section = FileSection::new(
                    "Data",
                    data_start,
                    data_start + actual_data_len,
                    risk, // inherits parent risk
                );

                // For IHDR, add parsed fields as children of the Data section
                if chunk_type == "IHDR" {
                    for child in Self::ihdr_children(data_start, actual_data_len) {
                        data_section = data_section.with_child(child);
                    }
                }

                section = section.with_child(data_section);
            }

            // CRC child (only if not truncated)
            if !is_truncated {
                let crc_start = data_start + data_length;
                section = section.with_child(FileSection::new(
                    "CRC",
                    crc_start,
                    crc_start + 4,
                    RiskLevel::High,
                ));
            }

            sections.push(section);

            if is_truncated {
                break;
            }

            pos = chunk_end;

            // Stop after IEND
            if chunk_type == "IEND" {
                break;
            }
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PNG with signature + given chunks
    fn build_png(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut data = PNG_SIGNATURE.to_vec();
        for (chunk_type, chunk_data) in chunks {
            let len = (chunk_data.len() as u32).to_be_bytes();
            data.extend_from_slice(&len);
            data.extend_from_slice(*chunk_type);
            data.extend_from_slice(chunk_data);
            // CRC placeholder (4 bytes of zeros)
            data.extend_from_slice(&[0u8; 4]);
        }
        data
    }

    /// Standard 13-byte IHDR data
    fn ihdr_data() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&100u32.to_be_bytes()); // width
        d.extend_from_slice(&80u32.to_be_bytes()); // height
        d.push(8); // bit depth
        d.push(2); // color type (truecolor)
        d.push(0); // compression
        d.push(0); // filter
        d.push(0); // interlace
        d
    }

    #[test]
    fn test_can_parse_png() {
        let parser = PngParser;

        // Valid PNG signature
        assert!(parser.can_parse(&PNG_SIGNATURE));
        assert!(parser.can_parse(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00]));

        // Invalid signatures
        assert!(!parser.can_parse(&[]));
        assert!(!parser.can_parse(&[0x89, 0x50]));
        assert!(!parser.can_parse(b"BM\x00\x00\x00\x00"));
        assert!(!parser.can_parse(&[0xFF, 0xD8, 0xFF, 0xE0]));
    }

    #[test]
    fn test_parse_minimal_png() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let png = build_png(&[(b"IHDR", &ihdr), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();

        assert!(sections.len() >= 3); // Signature + IHDR + IEND
        assert_eq!(sections[0].name, "PNG Signature");
        assert_eq!(sections[0].risk, RiskLevel::Critical);
        assert!(sections[1].name.contains("IHDR"));
        assert_eq!(sections[1].risk, RiskLevel::Critical);
        assert!(sections[2].name.contains("IEND"));
        assert_eq!(sections[2].risk, RiskLevel::Critical);
    }

    #[test]
    fn test_ihdr_has_children() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let png = build_png(&[(b"IHDR", &ihdr), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();
        let ihdr_section = &sections[1];

        // IHDR should have children: Length, Type, Data, CRC
        assert!(ihdr_section.children.len() >= 4);
        assert_eq!(ihdr_section.children[0].name, "Length");
        assert_eq!(ihdr_section.children[1].name, "Type");
        assert_eq!(ihdr_section.children[2].name, "Data");
        assert_eq!(ihdr_section.children[3].name, "CRC");

        // Data child should have IHDR-specific children
        let data_child = &ihdr_section.children[2];
        assert!(data_child.children.len() >= 7);
        let child_names: Vec<&str> = data_child
            .children
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(child_names.contains(&"Width"));
        assert!(child_names.contains(&"Height"));
        assert!(child_names.contains(&"Bit Depth"));
        assert!(child_names.contains(&"Color Type"));
        assert!(child_names.contains(&"Compression"));
        assert!(child_names.contains(&"Filter Method"));
        assert!(child_names.contains(&"Interlace"));
    }

    #[test]
    fn test_parse_not_png_returns_error() {
        let parser = PngParser;
        assert!(parser.parse(&[0x00, 0x01, 0x02, 0x03]).is_err());
        assert!(parser.parse(b"BM\x00\x00\x00\x00").is_err());
        assert!(parser.parse(&[0xFF, 0xD8, 0xFF]).is_err());
    }

    #[test]
    fn test_parse_truncated_png() {
        let parser = PngParser;
        // Only the signature, no chunks
        let png = PNG_SIGNATURE.to_vec();
        let sections = parser.parse(&png).unwrap();

        // Should have just the signature section
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "PNG Signature");
    }

    #[test]
    fn test_chunk_risk_levels() {
        assert_eq!(PngParser::chunk_risk("IHDR"), RiskLevel::Critical);
        assert_eq!(PngParser::chunk_risk("IEND"), RiskLevel::Critical);
        assert_eq!(PngParser::chunk_risk("PLTE"), RiskLevel::High);
        assert_eq!(PngParser::chunk_risk("IDAT"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("gAMA"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("cHRM"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("sRGB"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("iCCP"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("bKGD"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("tRNS"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk("tEXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk("zTXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk("iTXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk("tIME"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk("pHYs"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk("xYzW"), RiskLevel::Unknown);
    }

    #[test]
    fn test_parse_png_with_idat() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let idat_data = vec![0x78, 0x9C, 0x01, 0x02]; // fake compressed data
        let png = build_png(&[(b"IHDR", &ihdr), (b"IDAT", &idat_data), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();

        // Signature + IHDR + IDAT + IEND
        assert_eq!(sections.len(), 4);
        assert!(sections[2].name.contains("IDAT"));
        assert_eq!(sections[2].risk, RiskLevel::Caution);

        // IDAT should have children: Length, Type, Data, CRC
        assert_eq!(sections[2].children.len(), 4);
        assert_eq!(sections[2].children[0].name, "Length");
        assert_eq!(sections[2].children[1].name, "Type");
        assert_eq!(sections[2].children[2].name, "Data");
        assert_eq!(sections[2].children[3].name, "CRC");
    }

    #[test]
    fn test_parse_truncated_chunk() {
        let parser = PngParser;
        // Signature + partial chunk (length says 100 bytes but we only provide 4)
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&100u32.to_be_bytes()); // length = 100
        png.extend_from_slice(b"IHDR"); // type
        png.extend_from_slice(&[0u8; 4]); // only 4 bytes of data (not 100)

        let sections = parser.parse(&png).unwrap();

        // Signature + truncated IHDR
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "PNG Signature");
        assert!(sections[1].name.contains("IHDR"));
        // Truncated chunk end should be clamped to file size
        assert_eq!(sections[1].end, png.len());
    }
}
