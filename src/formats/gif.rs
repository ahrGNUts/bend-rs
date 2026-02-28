//! GIF file format parser (GIF87a / GIF89a)
//!
//! GIF structure:
//! - Header (6 bytes): signature "GIF" + version "87a" or "89a"
//! - Logical Screen Descriptor (7 bytes): canvas dimensions, packed flags, BG color, aspect ratio
//! - Global Color Table (optional): 3 × 2^(N+1) RGB triplets
//! - Blocks: extension blocks and image descriptors (repeated)
//!   - Graphics Control Extension (0x21 0xF9): delay, disposal, transparency
//!   - Comment Extension (0x21 0xFE): metadata text
//!   - Application Extension (0x21 0xFF): e.g. NETSCAPE looping
//!   - Plain Text Extension (0x21 0x01): text overlay (rare)
//!   - Image Descriptor (0x2C): per-frame position, dimensions, flags
//!     - Optional Local Color Table
//!     - Image Data: LZW minimum code size + sub-blocks
//! - Trailer (0x3B): end-of-file marker

use super::traits::{FileSection, ImageFormat, ParseError, RiskLevel};

/// GIF format parser
pub struct GifParser;

/// Skip a sequence of GIF sub-blocks (size-prefixed chunks terminated by a 0x00 byte).
/// Returns the position after the 0x00 terminator, or None if data is truncated.
fn skip_sub_blocks(data: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let block_size = *data.get(pos)? as usize;
        pos += 1;
        if block_size == 0 {
            return Some(pos);
        }
        pos = pos.checked_add(block_size)?;
        if pos > data.len() {
            return None;
        }
    }
}

/// Parse a Graphics Control Extension block.
/// `pos` points to the 0x21 introducer byte.
fn parse_graphics_control_ext(data: &[u8], pos: usize) -> (FileSection, usize) {
    // Standard GCE is 8 bytes: 21 F9 04 <packed> <delay_lo> <delay_hi> <trans_idx> 00
    let block_end = (pos + 8).min(data.len());

    let mut section = FileSection::new(
        "Graphics Control Extension",
        pos,
        block_end,
        RiskLevel::Caution,
    )
    .with_description("Controls frame delay, disposal method, and transparency");

    // Introducer + label (2 bytes)
    section = section.with_child(FileSection::new(
        "Extension Introducer",
        pos,
        (pos + 2).min(data.len()),
        RiskLevel::Critical,
    ));

    if data.len() >= pos + 8 {
        // Block size byte (should be 0x04)
        section = section.with_child(FileSection::new(
            "Block Size",
            pos + 2,
            pos + 3,
            RiskLevel::Critical,
        ));
        // Packed byte: disposal method, user input flag, transparent flag
        section = section.with_child(
            FileSection::new("Packed Byte", pos + 3, pos + 4, RiskLevel::Caution)
                .with_description("Disposal method, user input flag, transparent color flag"),
        );
        // Delay time (2 bytes LE, in centiseconds)
        section = section.with_child(
            FileSection::new("Delay Time", pos + 4, pos + 6, RiskLevel::Caution)
                .with_description("Frame delay in centiseconds"),
        );
        // Transparent color index
        section = section.with_child(FileSection::new(
            "Transparent Color Index",
            pos + 6,
            pos + 7,
            RiskLevel::Caution,
        ));
        // Block terminator
        section = section.with_child(FileSection::new(
            "Block Terminator",
            pos + 7,
            pos + 8,
            RiskLevel::Critical,
        ));
    }

    (section, block_end)
}

/// Parse a Comment Extension block.
/// `pos` points to the 0x21 introducer byte.
fn parse_comment_ext(data: &[u8], pos: usize) -> (FileSection, usize) {
    // 21 FE <sub-blocks> 00
    let sub_block_start = pos + 2;
    let end = skip_sub_blocks(data, sub_block_start).unwrap_or(data.len());

    let section = FileSection::new("Comment Extension", pos, end, RiskLevel::Safe)
        .with_description("Metadata comment — safe to edit");

    (section, end)
}

/// Parse an Application Extension block.
/// `pos` points to the 0x21 introducer byte.
fn parse_application_ext(data: &[u8], pos: usize) -> (FileSection, usize) {
    // 21 FF 0B <8-byte app id> <3-byte auth code> <sub-blocks> 00
    let _header_end = (pos + 14).min(data.len()); // 2 + 1 + 8 + 3 = 14 bytes for the fixed part
    let sub_block_start = if data.len() >= pos + 14 {
        pos + 14
    } else {
        data.len()
    };
    let end = skip_sub_blocks(data, sub_block_start).unwrap_or(data.len());

    let mut section = FileSection::new("Application Extension", pos, end, RiskLevel::Caution)
        .with_description("Application-specific data (e.g. NETSCAPE loop count)");

    if data.len() >= pos + 14 {
        section = section.with_child(FileSection::new(
            "Application Identifier",
            pos + 3,
            pos + 11,
            RiskLevel::Caution,
        ));
        section = section.with_child(FileSection::new(
            "Authentication Code",
            pos + 11,
            pos + 14,
            RiskLevel::Caution,
        ));
        if end > pos + 14 {
            section = section.with_child(FileSection::new(
                "Application Data",
                pos + 14,
                end,
                RiskLevel::Caution,
            ));
        }
    }

    (section, end)
}

/// Parse a Plain Text Extension block.
/// `pos` points to the 0x21 introducer byte.
fn parse_plain_text_ext(data: &[u8], pos: usize) -> (FileSection, usize) {
    // 21 01 0C <12 bytes fixed> <sub-blocks> 00
    let _fixed_end = (pos + 15).min(data.len()); // 2 + 1 + 12 = 15
    let sub_block_start = if data.len() >= pos + 15 {
        pos + 15
    } else {
        data.len()
    };
    let end = skip_sub_blocks(data, sub_block_start).unwrap_or(data.len());

    let section = FileSection::new("Plain Text Extension", pos, end, RiskLevel::Caution)
        .with_description("Text overlay (rare GIF89a feature)");

    (section, end)
}

/// Parse an Image Descriptor and its associated data (local color table + image data).
/// `pos` points to the 0x2C image separator byte.
/// Returns the section and the position after the image data.
fn parse_image_block(data: &[u8], pos: usize) -> (Vec<FileSection>, usize) {
    let mut children = Vec::new();

    // Image Descriptor is 10 bytes: 2C <left:2> <top:2> <width:2> <height:2> <packed:1>
    let desc_end = (pos + 10).min(data.len());
    let mut desc_section = FileSection::new("Image Descriptor", pos, desc_end, RiskLevel::Critical)
        .with_description("Frame position and dimensions");

    if data.len() >= pos + 10 {
        desc_section = desc_section
            .with_child(FileSection::new(
                "Image Separator",
                pos,
                pos + 1,
                RiskLevel::Critical,
            ))
            .with_child(
                FileSection::new("Left Position", pos + 1, pos + 3, RiskLevel::Critical)
                    .with_description("X offset within canvas"),
            )
            .with_child(
                FileSection::new("Top Position", pos + 3, pos + 5, RiskLevel::Critical)
                    .with_description("Y offset within canvas"),
            )
            .with_child(FileSection::new(
                "Frame Width",
                pos + 5,
                pos + 7,
                RiskLevel::Critical,
            ))
            .with_child(FileSection::new(
                "Frame Height",
                pos + 7,
                pos + 9,
                RiskLevel::Critical,
            ))
            .with_child(
                FileSection::new("Packed Byte", pos + 9, pos + 10, RiskLevel::Critical)
                    .with_description("Local color table flag, interlace, sort, LCT size"),
            );
    }

    children.push(desc_section);

    let mut current_pos = desc_end;

    // Local Color Table (if present)
    if data.len() >= pos + 10 {
        let packed = data[pos + 9];
        let has_lct = packed & 0x80 != 0;
        if has_lct {
            let lct_size_bits = (packed & 0x07) as usize;
            let lct_entries = 1 << (lct_size_bits + 1);
            let lct_bytes = lct_entries * 3;
            let lct_end = (current_pos + lct_bytes).min(data.len());

            children.push(
                FileSection::new("Local Color Table", current_pos, lct_end, RiskLevel::Safe)
                    .with_description("Per-frame palette override — great for glitch art"),
            );
            current_pos = lct_end;
        }
    }

    // Image Data: LZW minimum code size (1 byte) + sub-blocks
    if current_pos < data.len() {
        let img_data_start = current_pos;
        current_pos += 1; // LZW minimum code size byte
        let img_data_end = skip_sub_blocks(data, current_pos).unwrap_or(data.len());

        children.push(
            FileSection::new(
                "Image Data",
                img_data_start,
                img_data_end,
                RiskLevel::Caution,
            )
            .with_description("LZW-compressed pixel data"),
        );
        current_pos = img_data_end;
    }

    (children, current_pos)
}

impl ImageFormat for GifParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        data.len() >= 6
            && data[0] == b'G'
            && data[1] == b'I'
            && data[2] == b'F'
            && (data[3..6] == *b"87a" || data[3..6] == *b"89a")
    }

    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, ParseError> {
        if !self.can_parse(data) {
            return Err(ParseError::InvalidSignature);
        }

        let mut sections = Vec::new();

        // --- Header (6 bytes) ---
        let header = FileSection::new("Header", 0, 6, RiskLevel::Critical)
            .with_description("GIF signature and version — must not be modified")
            .with_child(FileSection::new("Signature", 0, 3, RiskLevel::Critical))
            .with_child(FileSection::new("Version", 3, 6, RiskLevel::Critical));
        sections.push(header);

        // --- Logical Screen Descriptor (7 bytes, offset 6-13) ---
        if data.len() < 13 {
            return Ok(sections);
        }

        let mut lsd = FileSection::new("Logical Screen Descriptor", 6, 13, RiskLevel::Critical)
            .with_description("Canvas dimensions and color table configuration");

        lsd = lsd
            .with_child(FileSection::new("Canvas Width", 6, 8, RiskLevel::Critical))
            .with_child(FileSection::new(
                "Canvas Height",
                8,
                10,
                RiskLevel::Critical,
            ))
            .with_child(
                FileSection::new("Packed Byte", 10, 11, RiskLevel::Critical).with_description(
                    "Global color table flag, color resolution, sort flag, GCT size",
                ),
            )
            .with_child(
                FileSection::new("Background Color Index", 11, 12, RiskLevel::Caution)
                    .with_description("Index into GCT for background color"),
            )
            .with_child(FileSection::new(
                "Pixel Aspect Ratio",
                12,
                13,
                RiskLevel::Safe,
            ));

        sections.push(lsd);

        let mut pos: usize = 13;

        // --- Global Color Table (if present) ---
        let packed = data[10];
        let has_gct = packed & 0x80 != 0;
        if has_gct {
            let gct_size_bits = (packed & 0x07) as usize;
            let gct_entries = 1 << (gct_size_bits + 1);
            let gct_bytes = gct_entries * 3;
            let gct_end = (pos + gct_bytes).min(data.len());

            sections.push(
                FileSection::new("Global Color Table", pos, gct_end, RiskLevel::Safe)
                    .with_description(
                        "RGB palette entries — ideal for glitch art color manipulation",
                    ),
            );
            pos = gct_end;
        }

        // --- Parse blocks until trailer or end of data ---
        let mut frame_number: usize = 0;
        // Accumulate per-frame sections to group into "Frame N" parents
        let mut frame_children: Vec<FileSection> = Vec::new();
        let mut frame_start: Option<usize> = None;

        while pos < data.len() {
            let block_type = data[pos];

            match block_type {
                // Extension introducer
                0x21 => {
                    if pos + 1 >= data.len() {
                        break;
                    }
                    let label = data[pos + 1];
                    match label {
                        // Graphics Control Extension — starts a new frame
                        0xF9 => {
                            // If we have accumulated frame children, flush the previous frame
                            if !frame_children.is_empty() {
                                let fs = frame_start.unwrap_or(pos);
                                let prev_end = frame_children.last().map(|s| s.end).unwrap_or(pos);
                                frame_number += 1;
                                let mut frame_section = FileSection::new(
                                    format!("Frame {}", frame_number),
                                    fs,
                                    prev_end,
                                    RiskLevel::Caution,
                                );
                                for child in frame_children.drain(..) {
                                    frame_section = frame_section.with_child(child);
                                }
                                sections.push(frame_section);
                            }
                            frame_start = Some(pos);

                            let (gce_section, new_pos) = parse_graphics_control_ext(data, pos);
                            frame_children.push(gce_section);
                            pos = new_pos;
                        }
                        // Comment Extension
                        0xFE => {
                            let (section, new_pos) = parse_comment_ext(data, pos);
                            sections.push(section);
                            pos = new_pos;
                        }
                        // Application Extension
                        0xFF => {
                            let (section, new_pos) = parse_application_ext(data, pos);
                            sections.push(section);
                            pos = new_pos;
                        }
                        // Plain Text Extension
                        0x01 => {
                            let (section, new_pos) = parse_plain_text_ext(data, pos);
                            frame_children.push(section);
                            pos = new_pos;
                        }
                        // Unknown extension — skip sub-blocks
                        _ => {
                            let ext_start = pos;
                            pos += 2; // skip introducer + label
                            let end = skip_sub_blocks(data, pos).unwrap_or(data.len());
                            sections.push(FileSection::new(
                                format!("Unknown Extension (0x{:02X})", label),
                                ext_start,
                                end,
                                RiskLevel::Caution,
                            ));
                            pos = end;
                        }
                    }
                }

                // Image Descriptor
                0x2C => {
                    if frame_start.is_none() {
                        frame_start = Some(pos);
                    }

                    let (image_sections, new_pos) = parse_image_block(data, pos);
                    for s in image_sections {
                        frame_children.push(s);
                    }
                    pos = new_pos;

                    // If no GCE preceded this image, flush as a standalone frame
                    // (This handles GIF87a files and frames without GCE)
                    if frame_children
                        .iter()
                        .all(|s| s.name != "Graphics Control Extension")
                    {
                        let fs = frame_start.unwrap_or(pos);
                        let prev_end = frame_children.last().map(|s| s.end).unwrap_or(pos);
                        frame_number += 1;
                        let mut frame_section = FileSection::new(
                            format!("Frame {}", frame_number),
                            fs,
                            prev_end,
                            RiskLevel::Caution,
                        );
                        for child in frame_children.drain(..) {
                            frame_section = frame_section.with_child(child);
                        }
                        sections.push(frame_section);
                        frame_start = None;
                    }
                }

                // Trailer
                0x3B => {
                    // Flush any accumulated frame children
                    if !frame_children.is_empty() {
                        let fs = frame_start.unwrap_or(pos);
                        let prev_end = frame_children.last().map(|s| s.end).unwrap_or(pos);
                        frame_number += 1;
                        let mut frame_section = FileSection::new(
                            format!("Frame {}", frame_number),
                            fs,
                            prev_end,
                            RiskLevel::Caution,
                        );
                        for child in frame_children.drain(..) {
                            frame_section = frame_section.with_child(child);
                        }
                        sections.push(frame_section);
                    }

                    sections.push(
                        FileSection::new("Trailer", pos, pos + 1, RiskLevel::Critical)
                            .with_description("End of GIF file marker"),
                    );
                    pos += 1;
                    break;
                }

                // Unknown byte — treat as end of parseable data
                _ => {
                    break;
                }
            }
        }

        // Flush remaining frame children if trailer was missing
        if !frame_children.is_empty() {
            let fs = frame_start.unwrap_or(pos);
            let prev_end = frame_children.last().map(|s| s.end).unwrap_or(pos);
            frame_number += 1;
            let mut frame_section = FileSection::new(
                format!("Frame {}", frame_number),
                fs,
                prev_end,
                RiskLevel::Caution,
            );
            for child in frame_children.drain(..) {
                frame_section = frame_section.with_child(child);
            }
            sections.push(frame_section);
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid GIF89a with a single 1x1 frame.
    /// Returns a byte vector that is a valid, parseable GIF.
    fn minimal_gif89a() -> Vec<u8> {
        let mut gif = Vec::new();

        // Header
        gif.extend_from_slice(b"GIF89a");

        // Logical Screen Descriptor
        gif.extend_from_slice(&[
            0x01, 0x00, // width = 1
            0x01, 0x00, // height = 1
            0x80, // packed: GCT flag=1, color res=0, sort=0, GCT size=0 (2 entries)
            0x00, // background color index
            0x00, // pixel aspect ratio
        ]);

        // Global Color Table (2 entries × 3 bytes = 6 bytes)
        gif.extend_from_slice(&[
            0x00, 0x00, 0x00, // color 0: black
            0xFF, 0xFF, 0xFF, // color 1: white
        ]);

        // Image Descriptor
        gif.extend_from_slice(&[
            0x2C, // image separator
            0x00, 0x00, // left = 0
            0x00, 0x00, // top = 0
            0x01, 0x00, // width = 1
            0x01, 0x00, // height = 1
            0x00, // packed: no LCT, not interlaced
        ]);

        // Image Data
        gif.push(0x02); // LZW minimum code size
        gif.push(0x02); // sub-block size = 2
        gif.extend_from_slice(&[0x4C, 0x01]); // compressed data for 1 pixel
        gif.push(0x00); // sub-block terminator

        // Trailer
        gif.push(0x3B);

        gif
    }

    /// Build a minimal 2-frame animated GIF89a.
    fn minimal_animated_gif() -> Vec<u8> {
        let mut gif = Vec::new();

        // Header
        gif.extend_from_slice(b"GIF89a");

        // Logical Screen Descriptor
        gif.extend_from_slice(&[
            0x01, 0x00, // width = 1
            0x01, 0x00, // height = 1
            0x80, // packed: GCT flag=1, color res=0, sort=0, GCT size=0
            0x00, 0x00,
        ]);

        // Global Color Table (2 entries)
        gif.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF]);

        // Application Extension (NETSCAPE2.0 for looping)
        gif.extend_from_slice(&[
            0x21, 0xFF, 0x0B, // extension introducer, app label, block size
        ]);
        gif.extend_from_slice(b"NETSCAPE2.0");
        gif.extend_from_slice(&[
            0x03, // sub-block size
            0x01, // sub-block ID
            0x00, 0x00, // loop count = 0 (infinite)
            0x00, // terminator
        ]);

        // --- Frame 1 ---
        // Graphics Control Extension
        gif.extend_from_slice(&[
            0x21, 0xF9, 0x04, // introducer, label, block size
            0x00, // packed
            0x0A, 0x00, // delay = 10 centiseconds
            0x00, // transparent color index
            0x00, // terminator
        ]);

        // Image Descriptor
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        // Image Data
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);

        // --- Frame 2 ---
        // Graphics Control Extension
        gif.extend_from_slice(&[0x21, 0xF9, 0x04, 0x00, 0x14, 0x00, 0x00, 0x00]);

        // Image Descriptor
        gif.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        // Image Data
        gif.push(0x02);
        gif.push(0x02);
        gif.extend_from_slice(&[0x4C, 0x01]);
        gif.push(0x00);

        // Trailer
        gif.push(0x3B);

        gif
    }

    #[test]
    fn test_can_parse_gif87a() {
        let parser = GifParser;
        let mut data = b"GIF87a".to_vec();
        data.extend_from_slice(&[0; 20]);
        assert!(parser.can_parse(&data));
    }

    #[test]
    fn test_can_parse_gif89a() {
        let parser = GifParser;
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&[0; 20]);
        assert!(parser.can_parse(&data));
    }

    #[test]
    fn test_cannot_parse_invalid() {
        let parser = GifParser;
        assert!(!parser.can_parse(b"PNG"));
        assert!(!parser.can_parse(b"BM"));
        assert!(!parser.can_parse(b"GIF"));
        assert!(!parser.can_parse(b"GIF90a"));
        assert!(!parser.can_parse(&[]));
    }

    #[test]
    fn test_parse_minimal_gif() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        // Should have: Header, LSD, GCT, Frame 1 (with Image Descriptor + Image Data), Trailer
        let names: Vec<&str> = sections.iter().map(|s| s.name.as_ref()).collect();
        assert!(names.contains(&"Header"));
        assert!(names.contains(&"Logical Screen Descriptor"));
        assert!(names.contains(&"Global Color Table"));
        assert!(names.contains(&"Frame 1"));
        assert!(names.contains(&"Trailer"));
    }

    #[test]
    fn test_parse_header_children() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        let header = sections.iter().find(|s| s.name == "Header").unwrap();
        assert_eq!(header.start, 0);
        assert_eq!(header.end, 6);
        assert_eq!(header.children.len(), 2);
        assert_eq!(header.children[0].name, "Signature");
        assert_eq!(header.children[1].name, "Version");
    }

    #[test]
    fn test_parse_lsd_children() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        let lsd = sections
            .iter()
            .find(|s| s.name == "Logical Screen Descriptor")
            .unwrap();
        assert_eq!(lsd.start, 6);
        assert_eq!(lsd.end, 13);
        assert_eq!(lsd.children.len(), 5);

        let child_names: Vec<&str> = lsd.children.iter().map(|c| c.name.as_ref()).collect();
        assert!(child_names.contains(&"Canvas Width"));
        assert!(child_names.contains(&"Canvas Height"));
        assert!(child_names.contains(&"Packed Byte"));
        assert!(child_names.contains(&"Background Color Index"));
        assert!(child_names.contains(&"Pixel Aspect Ratio"));
    }

    #[test]
    fn test_parse_global_color_table() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        let gct = sections
            .iter()
            .find(|s| s.name == "Global Color Table")
            .unwrap();
        // 2 entries × 3 bytes = 6 bytes, starting after LSD at offset 13
        assert_eq!(gct.start, 13);
        assert_eq!(gct.end, 19);
        assert_eq!(gct.risk, RiskLevel::Safe);
    }

    #[test]
    fn test_parse_animated_gif_frames() {
        let parser = GifParser;
        let data = minimal_animated_gif();
        let sections = parser.parse(&data).unwrap();

        let frame_sections: Vec<&FileSection> = sections
            .iter()
            .filter(|s| s.name.starts_with("Frame"))
            .collect();
        assert_eq!(frame_sections.len(), 2);
        assert_eq!(frame_sections[0].name, "Frame 1");
        assert_eq!(frame_sections[1].name, "Frame 2");

        // Each frame should have GCE, Image Descriptor, and Image Data as children
        for frame in &frame_sections {
            let child_names: Vec<&str> = frame.children.iter().map(|c| c.name.as_ref()).collect();
            assert!(
                child_names.contains(&"Graphics Control Extension"),
                "Frame {} missing GCE",
                frame.name
            );
            assert!(
                child_names.contains(&"Image Descriptor"),
                "Frame {} missing Image Descriptor",
                frame.name
            );
            assert!(
                child_names.contains(&"Image Data"),
                "Frame {} missing Image Data",
                frame.name
            );
        }
    }

    #[test]
    fn test_parse_application_extension() {
        let parser = GifParser;
        let data = minimal_animated_gif();
        let sections = parser.parse(&data).unwrap();

        let app_ext = sections.iter().find(|s| s.name == "Application Extension");
        assert!(app_ext.is_some());
        let app_ext = app_ext.unwrap();
        assert_eq!(app_ext.risk, RiskLevel::Caution);

        // Should have children for app identifier and auth code
        let child_names: Vec<&str> = app_ext.children.iter().map(|c| c.name.as_ref()).collect();
        assert!(child_names.contains(&"Application Identifier"));
        assert!(child_names.contains(&"Authentication Code"));
    }

    #[test]
    fn test_parse_trailer() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        let trailer = sections.iter().find(|s| s.name == "Trailer").unwrap();
        assert_eq!(trailer.end - trailer.start, 1);
        assert_eq!(trailer.risk, RiskLevel::Critical);
    }

    #[test]
    fn test_truncated_header_only() {
        let parser = GifParser;
        // Only the 6-byte header, no LSD
        let data = b"GIF89a";
        let sections = parser.parse(data).unwrap();
        // Should get Header but nothing else
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Header");
    }

    #[test]
    fn test_truncated_after_lsd() {
        let parser = GifParser;
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]); // LSD, no GCT
                                                                             // Should parse Header + LSD, then stop
        let sections = parser.parse(&data).unwrap();
        assert_eq!(sections.len(), 2);
    }

    #[test]
    fn test_truncated_gct() {
        let parser = GifParser;
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&[
            0x01, 0x00, 0x01, 0x00, 0x80, // GCT flag set, size = 0 (2 entries, need 6 bytes)
            0x00, 0x00,
        ]);
        // Only 2 bytes of GCT instead of 6
        data.extend_from_slice(&[0x00, 0x00]);
        let sections = parser.parse(&data).unwrap();
        let gct = sections
            .iter()
            .find(|s| s.name == "Global Color Table")
            .unwrap();
        // Should clamp to data.len()
        assert_eq!(gct.end, data.len());
    }

    #[test]
    fn test_no_gct() {
        let parser = GifParser;
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&[
            0x01, 0x00, 0x01, 0x00, 0x00, // no GCT flag
            0x00, 0x00,
        ]);
        data.push(0x3B); // trailer
        let sections = parser.parse(&data).unwrap();
        assert!(sections.iter().all(|s| s.name != "Global Color Table"));
    }

    #[test]
    fn test_comment_extension() {
        let parser = GifParser;
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]); // LSD, no GCT

        // Comment extension
        data.push(0x21); // extension introducer
        data.push(0xFE); // comment label
        data.push(0x05); // sub-block size
        data.extend_from_slice(b"Hello");
        data.push(0x00); // terminator

        data.push(0x3B); // trailer

        let sections = parser.parse(&data).unwrap();
        let comment = sections.iter().find(|s| s.name == "Comment Extension");
        assert!(comment.is_some());
        assert_eq!(comment.unwrap().risk, RiskLevel::Safe);
    }

    #[test]
    fn test_skip_sub_blocks() {
        // Two sub-blocks followed by terminator
        let data = [
            0x03, 0xAA, 0xBB, 0xCC, // block 1: size=3, 3 data bytes
            0x02, 0xDD, 0xEE, // block 2: size=2, 2 data bytes
            0x00, // terminator
        ];
        assert_eq!(skip_sub_blocks(&data, 0), Some(8));

        // Empty (just terminator)
        let data2 = [0x00];
        assert_eq!(skip_sub_blocks(&data2, 0), Some(1));

        // Truncated
        let data3 = [0x05, 0xAA]; // says 5 bytes but only 1 available
        assert_eq!(skip_sub_blocks(&data3, 0), None);
    }

    #[test]
    fn test_frame_without_gce() {
        // GIF87a style: image descriptor without preceding GCE
        let parser = GifParser;
        let mut data = b"GIF87a".to_vec();
        data.extend_from_slice(&[0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]); // LSD

        // Image Descriptor (no GCE before it)
        data.extend_from_slice(&[0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        // Image Data
        data.push(0x02);
        data.push(0x02);
        data.extend_from_slice(&[0x4C, 0x01]);
        data.push(0x00);

        data.push(0x3B); // trailer

        let sections = parser.parse(&data).unwrap();
        let frames: Vec<_> = sections
            .iter()
            .filter(|s| s.name.starts_with("Frame"))
            .collect();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].name, "Frame 1");
    }

    #[test]
    fn test_image_descriptor_children() {
        let parser = GifParser;
        let data = minimal_gif89a();
        let sections = parser.parse(&data).unwrap();

        let frame = sections.iter().find(|s| s.name == "Frame 1").unwrap();
        let img_desc = frame
            .children
            .iter()
            .find(|c| c.name == "Image Descriptor")
            .unwrap();

        let child_names: Vec<&str> = img_desc.children.iter().map(|c| c.name.as_ref()).collect();
        assert!(child_names.contains(&"Image Separator"));
        assert!(child_names.contains(&"Left Position"));
        assert!(child_names.contains(&"Top Position"));
        assert!(child_names.contains(&"Frame Width"));
        assert!(child_names.contains(&"Frame Height"));
        assert!(child_names.contains(&"Packed Byte"));
    }

    #[test]
    fn test_gce_children() {
        let parser = GifParser;
        let data = minimal_animated_gif();
        let sections = parser.parse(&data).unwrap();

        let frame1 = sections.iter().find(|s| s.name == "Frame 1").unwrap();
        let gce = frame1
            .children
            .iter()
            .find(|c| c.name == "Graphics Control Extension")
            .unwrap();

        let child_names: Vec<&str> = gce.children.iter().map(|c| c.name.as_ref()).collect();
        assert!(child_names.contains(&"Delay Time"));
        assert!(child_names.contains(&"Packed Byte"));
        assert!(child_names.contains(&"Transparent Color Index"));
    }
}
