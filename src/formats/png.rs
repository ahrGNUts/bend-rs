//! PNG file format parser
//!
//! PNG structure:
//! - Signature: 8 bytes (89 50 4E 47 0D 0A 1A 0A)
//! - Chunks: each chunk has Length (4 bytes) + Type (4 bytes) + Data (Length bytes) + CRC (4 bytes)
//! - Critical chunks: IHDR (must be first), PLTE, IDAT, IEND (must be last)
//! - Ancillary chunks: tEXt, zTXt, iTXt, gAMA, cHRM, sRGB, iCCP, bKGD, pHYs, tIME, tRNS,
//!   sBIT, hIST, sPLT, eXIf, cICP, mDCv, cLLi, caBX
//! - APNG chunks: acTL, fcTL, fdAT
//! - Registered extensions: oFFs, pCAL, sCAL, gIFg, gIFx, sTER, dSIG

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

    /// Extract 4-byte chunk type from data at offset
    fn chunk_type_bytes(data: &[u8], offset: usize) -> Option<[u8; 4]> {
        if offset + 4 > data.len() {
            return None;
        }
        Some([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    }

    /// Get human-readable name for a chunk type.
    ///
    /// For unrecognized chunk types, uses PNG's chunk type byte conventions
    /// (bit 5 of each byte encodes ancillary/critical, public/private, safe-to-copy)
    /// to produce a descriptive fallback label.
    fn chunk_name(chunk_type: &[u8; 4]) -> String {
        match chunk_type {
            b"IHDR" => "IHDR (Image Header)",
            b"PLTE" => "PLTE (Palette)",
            b"IDAT" => "IDAT (Image Data)",
            b"IEND" => "IEND (Image End)",
            b"tEXt" => "tEXt (Text)",
            b"zTXt" => "zTXt (Compressed Text)",
            b"iTXt" => "iTXt (International Text)",
            b"gAMA" => "gAMA (Gamma)",
            b"cHRM" => "cHRM (Chromaticities)",
            b"sRGB" => "sRGB (Standard RGB)",
            b"iCCP" => "iCCP (ICC Profile)",
            b"bKGD" => "bKGD (Background)",
            b"pHYs" => "pHYs (Physical Dimensions)",
            b"tIME" => "tIME (Timestamp)",
            b"tRNS" => "tRNS (Transparency)",
            b"sBIT" => "sBIT (Significant Bits)",
            b"hIST" => "hIST (Histogram)",
            b"sPLT" => "sPLT (Suggested Palette)",
            b"eXIf" => "eXIf (Exif Metadata)",
            b"acTL" => "acTL (Animation Control)",
            b"fcTL" => "fcTL (Frame Control)",
            b"fdAT" => "fdAT (Frame Data)",
            // PNG Third/Fourth Edition
            b"cICP" => "cICP (Coding-Independent Code Points)",
            b"mDCv" => "mDCv (Mastering Display Color Volume)",
            b"cLLi" => "cLLi (Content Light Level)",
            b"caBX" => "caBX (Content Credentials Box)",
            // Registered extensions
            b"oFFs" => "oFFs (Image Offset)",
            b"pCAL" => "pCAL (Pixel Calibration)",
            b"sCAL" => "sCAL (Physical Scale)",
            b"gIFg" => "gIFg (GIF Graphic Control)",
            b"gIFx" => "gIFx (GIF Application Extension)",
            b"sTER" => "sTER (Stereo Image)",
            b"dSIG" => "dSIG (Digital Signature)",
            // Common private chunks (Apple, Adobe, ImageMagick)
            b"iDOT" => "iDOT (Apple Optimization Data)",
            b"CgBI" => "CgBI (Apple CoreGraphics)",
            b"vpAg" => "vpAg (Virtual Page)",
            b"orNT" => "orNT (Orientation)",
            _ => return Self::fallback_chunk_name(chunk_type),
        }
        .into()
    }

    /// Produce a descriptive name for an unrecognized chunk type using PNG byte conventions.
    /// Bit 5 of byte 0: 0 = critical, 1 = ancillary
    /// Bit 5 of byte 1: 0 = public, 1 = private
    fn fallback_chunk_name(chunk_type: &[u8; 4]) -> String {
        let is_ancillary = chunk_type[0] & 0x20 != 0;
        let is_private = chunk_type[1] & 0x20 != 0;

        let type_str = chunk_type
            .iter()
            .map(|&b| if b.is_ascii_graphic() { b as char } else { '?' })
            .collect::<String>();

        let kind = match (is_ancillary, is_private) {
            (true, true) => "Private Ancillary",
            (true, false) => "Public Ancillary",
            (false, true) => "Private Critical",
            (false, false) => "Public Critical",
        };
        format!("{type_str} ({kind} Chunk)")
    }

    /// Get risk level for a chunk type.
    ///
    /// For unrecognized chunks, uses PNG byte conventions: ancillary chunks default
    /// to Safe (decoders can skip them), critical chunks default to High.
    fn chunk_risk(chunk_type: &[u8; 4]) -> RiskLevel {
        match chunk_type {
            b"IHDR" | b"IEND" => RiskLevel::Critical,
            b"PLTE" | b"acTL" | b"fcTL" | b"dSIG" | b"CgBI" => RiskLevel::High,
            b"IDAT" | b"fdAT" | b"gAMA" | b"cHRM" | b"sRGB" | b"iCCP" | b"bKGD" | b"tRNS"
            | b"sBIT" | b"cICP" | b"mDCv" | b"cLLi" => RiskLevel::Caution,
            b"tEXt" | b"zTXt" | b"iTXt" | b"tIME" | b"pHYs" | b"eXIf" | b"hIST" | b"sPLT"
            | b"oFFs" | b"pCAL" | b"sCAL" | b"gIFg" | b"gIFx" | b"sTER" | b"caBX" | b"iDOT"
            | b"vpAg" | b"orNT" => RiskLevel::Safe,
            _ => {
                // Ancillary chunks (bit 5 of first byte set) are safe to skip
                if chunk_type[0] & 0x20 != 0 {
                    RiskLevel::Safe
                } else {
                    RiskLevel::High
                }
            }
        }
    }

    /// Get description for a chunk type
    fn chunk_description(chunk_type: &[u8; 4]) -> &'static str {
        match chunk_type {
            b"IHDR" => "Image dimensions, bit depth, and color type",
            b"PLTE" => "Palette data for indexed-color images",
            b"IDAT" => "Compressed image data - the fun part to glitch",
            b"IEND" => "End of PNG data stream",
            b"tEXt" => "Uncompressed text metadata - safe to edit",
            b"zTXt" => "Compressed text metadata",
            b"iTXt" => "International text metadata",
            b"gAMA" => "Image gamma value",
            b"cHRM" => "Primary chromaticities and white point",
            b"sRGB" => "Standard RGB color space indicator",
            b"iCCP" => "Embedded ICC profile",
            b"bKGD" => "Default background color",
            b"pHYs" => "Physical pixel dimensions",
            b"tIME" => "Last modification time",
            b"tRNS" => "Transparency information",
            b"sBIT" => "Number of significant bits per channel",
            b"hIST" => "Frequency of each palette color",
            b"sPLT" => "Suggested palette for display",
            b"eXIf" => "Exchangeable image file (Exif) metadata",
            b"acTL" => "APNG animation control - frame count and loop info",
            b"fcTL" => "APNG frame control - dimensions, offsets, timing",
            b"fdAT" => "APNG frame data - compressed frame image data",
            b"cICP" => "HDR color space via coding-independent code points",
            b"mDCv" => "HDR mastering display color volume",
            b"cLLi" => "HDR content light level information",
            b"caBX" => "Content credentials (C2PA) provenance box",
            b"oFFs" => "Image offset from page origin",
            b"pCAL" => "Calibration of pixel values to physical units",
            b"sCAL" => "Physical scale of image subject",
            b"gIFg" => "GIF graphic control extension data",
            b"gIFx" => "GIF application extension data",
            b"sTER" => "Stereo image indicator",
            b"dSIG" => "Digital signature for integrity verification",
            b"iDOT" => "Apple PNG optimization hints",
            b"CgBI" => "Apple CoreGraphics premultiplied BGR format marker",
            b"vpAg" => "Virtual page size information",
            b"orNT" => "Image orientation metadata",
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
                RiskLevel::High,
            ),
            FileSection::new(
                "Filter Method",
                data_start + 11,
                data_start + 12,
                RiskLevel::High,
            ),
            FileSection::new(
                "Interlace",
                data_start + 12,
                data_start + 13,
                RiskLevel::High,
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
            let Some(chunk_type) = Self::chunk_type_bytes(data, pos) else {
                // Truncated: can't read type
                break;
            };
            pos += 4;

            let data_start = pos;
            let Some(chunk_end_expected) = data_start
                .checked_add(data_length)
                .and_then(|v| v.checked_add(4))
            else {
                break;
            };

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
            if is_truncated {
                let msg = if desc.is_empty() {
                    "(truncated)".into()
                } else {
                    format!("{desc} (truncated)")
                };
                section = section.with_description(msg);
            } else if !desc.is_empty() {
                section = section.with_description(desc);
            }

            // Add structural child sections: Length, Type, Data, CRC
            // Note: Length and Type children always fit within parent bounds because
            // the parser breaks out of the loop if it can't read both the 4-byte length
            // and 4-byte type fields, guaranteeing chunk_end >= chunk_start + 8.
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
                if chunk_type == *b"IHDR" {
                    for child in Self::ihdr_children(data_start, actual_data_len) {
                        data_section = data_section.with_child(child);
                    }
                }

                section = section.with_child(data_section);
            }

            // CRC child (only if not truncated)
            // Note: CRC bytes are read but not validated. This is intentional — as a
            // glitch art tool, we want to display and allow editing of broken files
            // without rejecting them for checksum mismatches.
            if !is_truncated {
                let crc_start = data_start + data_length;
                section = section.with_child(FileSection::new(
                    "CRC",
                    crc_start,
                    crc_start + 4,
                    risk, // CRC risk matches the chunk's risk level
                ));
            }

            sections.push(section);

            if is_truncated {
                break;
            }

            pos = chunk_end;

            // Stop after IEND
            if chunk_type == *b"IEND" {
                break;
            }
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PNG with signature + given chunks.
    /// Uses zero-filled CRC placeholders (not real checksums). This is acceptable
    /// because the parser intentionally skips CRC validation for glitch art use.
    fn build_png(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut data = PNG_SIGNATURE.to_vec();
        for (chunk_type, chunk_data) in chunks {
            let len = (chunk_data.len() as u32).to_be_bytes();
            data.extend_from_slice(&len);
            data.extend_from_slice(*chunk_type);
            data.extend_from_slice(chunk_data);
            data.extend_from_slice(&[0u8; 4]); // CRC placeholder
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
    fn test_ihdr_child_risk_levels() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let png = build_png(&[(b"IHDR", &ihdr), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();
        let data_child = &sections[1].children[2]; // IHDR -> Data

        // Width, Height, Bit Depth, Color Type should be Critical
        assert_eq!(data_child.children[0].risk, RiskLevel::Critical); // Width
        assert_eq!(data_child.children[1].risk, RiskLevel::Critical); // Height
        assert_eq!(data_child.children[2].risk, RiskLevel::Critical); // Bit Depth
        assert_eq!(data_child.children[3].risk, RiskLevel::Critical); // Color Type

        // Compression, Filter, Interlace should be High (interesting glitch targets)
        assert_eq!(data_child.children[4].risk, RiskLevel::High); // Compression
        assert_eq!(data_child.children[5].risk, RiskLevel::High); // Filter Method
        assert_eq!(data_child.children[6].risk, RiskLevel::High); // Interlace
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
    fn test_parse_truncated_mid_length() {
        let parser = PngParser;
        // Signature + only 2 bytes of a chunk length field (truncated mid-length)
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&[0x00, 0x00]);

        let sections = parser.parse(&png).unwrap();

        // Should have just the signature — can't read 4-byte length, so loop breaks
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "PNG Signature");
    }

    #[test]
    fn test_parse_truncated_mid_type() {
        let parser = PngParser;
        // Signature + 4 bytes length + only 2 bytes of type (truncated mid-type)
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&0u32.to_be_bytes()); // length = 0
        png.extend_from_slice(&[b'I', b'H']); // partial type

        let sections = parser.parse(&png).unwrap();

        // Should have just the signature — can't read 4-byte type, so loop breaks
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "PNG Signature");
    }

    #[test]
    fn test_chunk_risk_levels() {
        // Critical
        assert_eq!(PngParser::chunk_risk(b"IHDR"), RiskLevel::Critical);
        assert_eq!(PngParser::chunk_risk(b"IEND"), RiskLevel::Critical);
        // High
        assert_eq!(PngParser::chunk_risk(b"PLTE"), RiskLevel::High);
        assert_eq!(PngParser::chunk_risk(b"acTL"), RiskLevel::High);
        assert_eq!(PngParser::chunk_risk(b"fcTL"), RiskLevel::High);
        assert_eq!(PngParser::chunk_risk(b"dSIG"), RiskLevel::High);
        // Caution
        assert_eq!(PngParser::chunk_risk(b"IDAT"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"fdAT"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"gAMA"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"cHRM"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"sRGB"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"iCCP"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"bKGD"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"tRNS"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"sBIT"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"cICP"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"mDCv"), RiskLevel::Caution);
        assert_eq!(PngParser::chunk_risk(b"cLLi"), RiskLevel::Caution);
        // Safe
        assert_eq!(PngParser::chunk_risk(b"tEXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"zTXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"iTXt"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"tIME"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"pHYs"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"eXIf"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"hIST"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"sPLT"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"oFFs"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"pCAL"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"sCAL"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"gIFg"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"gIFx"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"sTER"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"caBX"), RiskLevel::Safe);
        // Fallback: ancillary (lowercase first byte) defaults to Safe
        assert_eq!(PngParser::chunk_risk(b"xYzW"), RiskLevel::Safe);
        // Fallback: critical (uppercase first byte) defaults to High
        assert_eq!(PngParser::chunk_risk(b"XYZW"), RiskLevel::High);
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

    #[test]
    fn test_parse_png_with_ancillary_chunk() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let text_data = b"Comment\x00Hello, world!";
        let png = build_png(&[(b"IHDR", &ihdr), (b"tEXt", text_data), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();

        // Signature + IHDR + tEXt + IEND
        assert_eq!(sections.len(), 4);

        let text_section = &sections[2];
        assert!(text_section.name.contains("tEXt"));
        assert_eq!(text_section.risk, RiskLevel::Safe);

        // Should have children: Length, Type, Data, CRC
        assert_eq!(text_section.children.len(), 4);
        assert_eq!(text_section.children[0].name, "Length");
        assert_eq!(text_section.children[1].name, "Type");
        assert_eq!(text_section.children[2].name, "Data");
        assert_eq!(text_section.children[3].name, "CRC");
    }

    #[test]
    fn test_crc_risk_varies_by_chunk_type() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let text_data = b"Comment\x00Hello!";
        let png = build_png(&[(b"IHDR", &ihdr), (b"tEXt", text_data), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();

        // IHDR CRC should be Critical (matches IHDR risk)
        let ihdr_crc = &sections[1].children[3];
        assert_eq!(ihdr_crc.name, "CRC");
        assert_eq!(ihdr_crc.risk, RiskLevel::Critical);

        // tEXt CRC should be Safe (matches tEXt risk)
        let text_crc = &sections[2].children[3];
        assert_eq!(text_crc.name, "CRC");
        assert_eq!(text_crc.risk, RiskLevel::Safe);

        // IEND CRC should be Critical (matches IEND risk)
        // IEND has no Data child (zero-length), so CRC is at index 2
        let iend_crc = sections[3]
            .children
            .iter()
            .find(|c| c.name == "CRC")
            .expect("IEND should have a CRC child");
        assert_eq!(iend_crc.risk, RiskLevel::Critical);
    }

    #[test]
    fn test_iend_zero_length_has_no_data_child() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let png = build_png(&[(b"IHDR", &ihdr), (b"IEND", &[])]);

        let sections = parser.parse(&png).unwrap();
        let iend_section = &sections[2];

        assert!(iend_section.name.contains("IEND"));
        // IEND (zero-length data) should have: Length, Type, CRC — but no Data child
        assert_eq!(iend_section.children.len(), 3);
        assert_eq!(iend_section.children[0].name, "Length");
        assert_eq!(iend_section.children[1].name, "Type");
        assert_eq!(iend_section.children[2].name, "CRC");
    }

    #[test]
    fn test_trailing_data_after_iend() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let mut png = build_png(&[(b"IHDR", &ihdr), (b"IEND", &[])]);
        // Append trailing garbage after IEND (simulates metadata appended by tools)
        png.extend_from_slice(b"TRAILING GARBAGE DATA");

        let sections = parser.parse(&png).unwrap();

        // Parser stops at IEND — should have Signature + IHDR + IEND
        assert_eq!(sections.len(), 3);
        assert!(sections[2].name.contains("IEND"));

        // The IEND section should end before the trailing data
        let iend_end = sections[2].end;
        assert!(iend_end < png.len());

        // When used via parse_file, fill_gaps should label the trailing bytes as Unknown
        let mut all_sections = sections;
        super::super::fill_gaps(&mut all_sections, png.len());
        let unknown_sections: Vec<_> = all_sections
            .iter()
            .filter(|s| s.name == "Unknown")
            .collect();
        assert_eq!(unknown_sections.len(), 1);
        assert_eq!(unknown_sections[0].start, iend_end);
        assert_eq!(unknown_sections[0].end, png.len());
    }

    #[test]
    fn test_non_ascii_chunk_type() {
        let parser = PngParser;
        // Build a PNG with a non-ASCII chunk type (0xFF bytes)
        let mut png = PNG_SIGNATURE.to_vec();
        let chunk_data = [0x01, 0x02];
        let len = (chunk_data.len() as u32).to_be_bytes();
        png.extend_from_slice(&len);
        png.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // invalid chunk type
        png.extend_from_slice(&chunk_data);
        png.extend_from_slice(&[0u8; 4]); // CRC

        let sections = parser.parse(&png).unwrap();

        // Should use fallback labeling; 0xFF has bit 5 set = ancillary + private
        assert_eq!(sections.len(), 2); // Signature + unknown chunk
        assert_eq!(sections[1].name, "???? (Private Ancillary Chunk)");
        assert_eq!(sections[1].risk, RiskLevel::Safe);
    }

    #[test]
    fn test_fallback_chunk_name_conventions() {
        // Ancillary (lowercase byte 0) + private (lowercase byte 1)
        assert_eq!(
            PngParser::chunk_name(b"xyzw"),
            "xyzw (Private Ancillary Chunk)"
        );
        // Critical (uppercase byte 0) + public (uppercase byte 1)
        assert_eq!(
            PngParser::chunk_name(b"XYZW"),
            "XYZW (Public Critical Chunk)"
        );
        // Ancillary + public (uppercase byte 1)
        assert_eq!(
            PngParser::chunk_name(b"xYZW"),
            "xYZW (Public Ancillary Chunk)"
        );
        // Critical + private (lowercase byte 1)
        assert_eq!(
            PngParser::chunk_name(b"XyZW"),
            "XyZW (Private Critical Chunk)"
        );
    }

    #[test]
    fn test_common_private_chunks() {
        // Apple iDOT — most common private chunk on macOS
        assert!(PngParser::chunk_name(b"iDOT").contains("Apple"));
        assert_eq!(PngParser::chunk_risk(b"iDOT"), RiskLevel::Safe);

        // Apple CgBI — iOS optimized PNGs
        assert!(PngParser::chunk_name(b"CgBI").contains("Apple"));
        assert_eq!(PngParser::chunk_risk(b"CgBI"), RiskLevel::High);

        // vpAg, orNT — ImageMagick and other tools
        assert_eq!(PngParser::chunk_risk(b"vpAg"), RiskLevel::Safe);
        assert_eq!(PngParser::chunk_risk(b"orNT"), RiskLevel::Safe);
    }
}
