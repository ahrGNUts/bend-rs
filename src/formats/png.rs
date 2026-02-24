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

use std::borrow::Cow;

use super::bytes;
use super::traits::{FileSection, ImageFormat, ParseError, RiskLevel};

/// PNG signature bytes
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Metadata for a known PNG chunk type (one source of truth).
struct ChunkInfo {
    chunk_type: &'static [u8; 4],
    name: &'static str,
    risk: RiskLevel,
    description: &'static str,
}

/// All known PNG chunk types with their name, risk level, and description.
const KNOWN_CHUNKS: &[ChunkInfo] = &[
    // Critical chunks
    ChunkInfo {
        chunk_type: b"IHDR",
        name: "IHDR (Image Header)",
        risk: RiskLevel::Critical,
        description: "Image dimensions, bit depth, and color type",
    },
    ChunkInfo {
        chunk_type: b"PLTE",
        name: "PLTE (Palette)",
        risk: RiskLevel::High,
        description: "Palette data for indexed-color images",
    },
    ChunkInfo {
        chunk_type: b"IDAT",
        name: "IDAT (Image Data)",
        risk: RiskLevel::Caution,
        description: "Compressed image data - the fun part to glitch",
    },
    ChunkInfo {
        chunk_type: b"IEND",
        name: "IEND (Image End)",
        risk: RiskLevel::Critical,
        description: "End of PNG data stream",
    },
    // Ancillary chunks
    ChunkInfo {
        chunk_type: b"tEXt",
        name: "tEXt (Text)",
        risk: RiskLevel::Safe,
        description: "Uncompressed text metadata - safe to edit",
    },
    ChunkInfo {
        chunk_type: b"zTXt",
        name: "zTXt (Compressed Text)",
        risk: RiskLevel::Safe,
        description: "Compressed text metadata",
    },
    ChunkInfo {
        chunk_type: b"iTXt",
        name: "iTXt (International Text)",
        risk: RiskLevel::Safe,
        description: "International text metadata",
    },
    ChunkInfo {
        chunk_type: b"gAMA",
        name: "gAMA (Gamma)",
        risk: RiskLevel::Caution,
        description: "Image gamma value",
    },
    ChunkInfo {
        chunk_type: b"cHRM",
        name: "cHRM (Chromaticities)",
        risk: RiskLevel::Caution,
        description: "Primary chromaticities and white point",
    },
    ChunkInfo {
        chunk_type: b"sRGB",
        name: "sRGB (Standard RGB)",
        risk: RiskLevel::Caution,
        description: "Standard RGB color space indicator",
    },
    ChunkInfo {
        chunk_type: b"iCCP",
        name: "iCCP (ICC Profile)",
        risk: RiskLevel::Caution,
        description: "Embedded ICC profile",
    },
    ChunkInfo {
        chunk_type: b"bKGD",
        name: "bKGD (Background)",
        risk: RiskLevel::Caution,
        description: "Default background color",
    },
    ChunkInfo {
        chunk_type: b"pHYs",
        name: "pHYs (Physical Dimensions)",
        risk: RiskLevel::Safe,
        description: "Physical pixel dimensions",
    },
    ChunkInfo {
        chunk_type: b"tIME",
        name: "tIME (Timestamp)",
        risk: RiskLevel::Safe,
        description: "Last modification time",
    },
    ChunkInfo {
        chunk_type: b"tRNS",
        name: "tRNS (Transparency)",
        risk: RiskLevel::Caution,
        description: "Transparency information",
    },
    ChunkInfo {
        chunk_type: b"sBIT",
        name: "sBIT (Significant Bits)",
        risk: RiskLevel::Caution,
        description: "Number of significant bits per channel",
    },
    ChunkInfo {
        chunk_type: b"hIST",
        name: "hIST (Histogram)",
        risk: RiskLevel::Safe,
        description: "Frequency of each palette color",
    },
    ChunkInfo {
        chunk_type: b"sPLT",
        name: "sPLT (Suggested Palette)",
        risk: RiskLevel::Safe,
        description: "Suggested palette for display",
    },
    ChunkInfo {
        chunk_type: b"eXIf",
        name: "eXIf (Exif Metadata)",
        risk: RiskLevel::Safe,
        description: "Exchangeable image file (Exif) metadata",
    },
    // APNG chunks
    ChunkInfo {
        chunk_type: b"acTL",
        name: "acTL (Animation Control)",
        risk: RiskLevel::High,
        description: "APNG animation control - frame count and loop info",
    },
    ChunkInfo {
        chunk_type: b"fcTL",
        name: "fcTL (Frame Control)",
        risk: RiskLevel::High,
        description: "APNG frame control - dimensions, offsets, timing",
    },
    ChunkInfo {
        chunk_type: b"fdAT",
        name: "fdAT (Frame Data)",
        risk: RiskLevel::Caution,
        description: "APNG frame data - compressed frame image data",
    },
    // PNG Third/Fourth Edition
    ChunkInfo {
        chunk_type: b"cICP",
        name: "cICP (Coding-Independent Code Points)",
        risk: RiskLevel::Caution,
        description: "HDR color space via coding-independent code points",
    },
    ChunkInfo {
        chunk_type: b"mDCv",
        name: "mDCv (Mastering Display Color Volume)",
        risk: RiskLevel::Caution,
        description: "HDR mastering display color volume",
    },
    ChunkInfo {
        chunk_type: b"cLLi",
        name: "cLLi (Content Light Level)",
        risk: RiskLevel::Caution,
        description: "HDR content light level information",
    },
    ChunkInfo {
        chunk_type: b"caBX",
        name: "caBX (Content Credentials Box)",
        risk: RiskLevel::Safe,
        description: "Content credentials (C2PA) provenance box",
    },
    // Registered extensions
    ChunkInfo {
        chunk_type: b"oFFs",
        name: "oFFs (Image Offset)",
        risk: RiskLevel::Safe,
        description: "Image offset from page origin",
    },
    ChunkInfo {
        chunk_type: b"pCAL",
        name: "pCAL (Pixel Calibration)",
        risk: RiskLevel::Safe,
        description: "Calibration of pixel values to physical units",
    },
    ChunkInfo {
        chunk_type: b"sCAL",
        name: "sCAL (Physical Scale)",
        risk: RiskLevel::Safe,
        description: "Physical scale of image subject",
    },
    ChunkInfo {
        chunk_type: b"gIFg",
        name: "gIFg (GIF Graphic Control)",
        risk: RiskLevel::Safe,
        description: "GIF graphic control extension data",
    },
    ChunkInfo {
        chunk_type: b"gIFx",
        name: "gIFx (GIF Application Extension)",
        risk: RiskLevel::Safe,
        description: "GIF application extension data",
    },
    ChunkInfo {
        chunk_type: b"sTER",
        name: "sTER (Stereo Image)",
        risk: RiskLevel::Safe,
        description: "Stereo image indicator",
    },
    ChunkInfo {
        chunk_type: b"dSIG",
        name: "dSIG (Digital Signature)",
        risk: RiskLevel::High,
        description: "Digital signature for integrity verification",
    },
    // Common private chunks (Apple, Adobe, ImageMagick)
    ChunkInfo {
        chunk_type: b"iDOT",
        name: "iDOT (Apple Optimization Data)",
        risk: RiskLevel::Safe,
        description: "Apple PNG optimization hints",
    },
    ChunkInfo {
        chunk_type: b"CgBI",
        name: "CgBI (Apple CoreGraphics)",
        risk: RiskLevel::High,
        description: "Apple CoreGraphics premultiplied BGR format marker",
    },
    ChunkInfo {
        chunk_type: b"vpAg",
        name: "vpAg (Virtual Page)",
        risk: RiskLevel::Safe,
        description: "Virtual page size information",
    },
    ChunkInfo {
        chunk_type: b"orNT",
        name: "orNT (Orientation)",
        risk: RiskLevel::Safe,
        description: "Image orientation metadata",
    },
];

/// Look up metadata for a known chunk type.
fn chunk_info(chunk_type: &[u8; 4]) -> Option<&'static ChunkInfo> {
    KNOWN_CHUNKS
        .iter()
        .find(|info| info.chunk_type == chunk_type)
}

/// PNG format parser
pub struct PngParser;

impl PngParser {
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
    /// Known chunks return a borrowed `&'static str` (zero allocation).
    /// Unknown chunks produce an owned `String` via fallback labeling.
    fn chunk_name(chunk_type: &[u8; 4]) -> Cow<'static, str> {
        if let Some(info) = chunk_info(chunk_type) {
            Cow::Borrowed(info.name)
        } else {
            Cow::Owned(Self::fallback_chunk_name(chunk_type))
        }
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
        if let Some(info) = chunk_info(chunk_type) {
            info.risk
        } else if chunk_type[0] & 0x20 != 0 {
            // Ancillary chunks are safe to skip
            RiskLevel::Safe
        } else {
            RiskLevel::High
        }
    }

    /// Get description for a chunk type
    fn chunk_description(chunk_type: &[u8; 4]) -> &'static str {
        chunk_info(chunk_type).map_or("", |info| info.description)
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

    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, ParseError> {
        if !self.can_parse(data) {
            return Err(ParseError::InvalidSignature);
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
            let Some(data_length) = bytes::read_u32_be(data, pos) else {
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
            .map(|c| c.name.as_ref())
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

    #[test]
    fn test_parse_multiple_idat_chunks() {
        let parser = PngParser;
        let ihdr = ihdr_data();
        let idat1 = vec![0x78, 0x9C]; // first IDAT
        let idat2 = vec![0x01, 0x02, 0x03]; // second IDAT
        let png = build_png(&[
            (b"IHDR", &ihdr),
            (b"IDAT", &idat1),
            (b"IDAT", &idat2),
            (b"IEND", &[]),
        ]);

        let sections = parser.parse(&png).unwrap();

        // Signature + IHDR + IDAT + IDAT + IEND
        assert_eq!(sections.len(), 5);

        // Both IDAT sections should be present with correct names and contiguous ranges
        let idat_sections: Vec<_> = sections
            .iter()
            .filter(|s| s.name.contains("IDAT"))
            .collect();
        assert_eq!(idat_sections.len(), 2);
        assert_eq!(idat_sections[0].risk, RiskLevel::Caution);
        assert_eq!(idat_sections[1].risk, RiskLevel::Caution);

        // Second IDAT starts where first IDAT ends
        assert_eq!(idat_sections[1].start, idat_sections[0].end);
    }

    #[test]
    fn test_overflow_chunk_length_handled() {
        let parser = PngParser;
        // Build a PNG with a chunk whose data_length is u32::MAX.
        // The parser should not panic — checked_add detects the overflow and breaks.
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&u32::MAX.to_be_bytes()); // length = u32::MAX
        png.extend_from_slice(b"IDAT"); // type
        png.extend_from_slice(&[0xAA; 8]); // some data (nowhere near u32::MAX)

        let sections = parser.parse(&png).unwrap();

        // Parser should produce the Signature section. The IDAT chunk's
        // data_start + data_length + 4 overflows, so the loop breaks cleanly.
        assert!(sections.len() >= 1);
        assert_eq!(sections[0].name, "PNG Signature");
    }
}
