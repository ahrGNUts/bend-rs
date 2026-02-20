//! BMP file format parser
//!
//! BMP structure:
//! - File Header (14 bytes): signature, file size, reserved, pixel data offset
//! - DIB Header (variable): image dimensions, color depth, compression, etc.
//! - Optional Color Table: palette for indexed color images
//! - Pixel Data: the actual image pixels

use super::traits::{FileSection, ImageFormat, RiskLevel};

/// BMP format parser
pub struct BmpParser;

impl BmpParser {
    /// Read a little-endian u32 from data
    fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
        if offset + 4 > data.len() {
            return None;
        }
        Some(u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }
}

impl ImageFormat for BmpParser {
    fn can_parse(&self, data: &[u8]) -> bool {
        // BMP files start with "BM"
        data.len() >= 2 && data[0] == b'B' && data[1] == b'M'
    }

    fn parse(&self, data: &[u8]) -> Result<Vec<FileSection>, String> {
        if !self.can_parse(data) {
            return Err("Not a valid BMP file".to_string());
        }

        let mut sections = Vec::new();

        // Need at least 14 bytes for the file header
        if data.len() < 14 {
            return Ok(sections);
        }

        // File Header (14 bytes)
        let file_header = FileSection::new("File Header", 0, 14, RiskLevel::Critical)
            .with_description("BMP file header - editing will likely corrupt the file")
            .with_child(
                FileSection::new("Signature", 0, 2, RiskLevel::Critical)
                    .with_description("'BM' magic bytes"),
            )
            .with_child(FileSection::new("File Size", 2, 6, RiskLevel::Critical))
            .with_child(
                FileSection::new("Reserved", 6, 10, RiskLevel::Safe)
                    .with_description("Reserved bytes, usually zero"),
            )
            .with_child(FileSection::new(
                "Pixel Data Offset",
                10,
                14,
                RiskLevel::Critical,
            ));

        sections.push(file_header);

        // Read pixel data offset — return partial on failure
        let Some(pixel_offset) = Self::read_u32(data, 10).map(|v| v as usize) else {
            return Ok(sections);
        };

        // DIB Header — need at least 4 bytes for the size field
        if data.len() < 18 {
            return Ok(sections);
        }

        let Some(dib_header_size) = Self::read_u32(data, 14).map(|v| v as usize) else {
            return Ok(sections);
        };

        let dib_header_end = 14 + dib_header_size;
        if dib_header_end > data.len() {
            // DIB header extends beyond file — return partial results
            return Ok(sections);
        }

        let dib_header_name = match dib_header_size {
            12 => "BITMAPCOREHEADER",
            40 => "BITMAPINFOHEADER",
            52 => "BITMAPV2INFOHEADER",
            56 => "BITMAPV3INFOHEADER",
            108 => "BITMAPV4HEADER",
            124 => "BITMAPV5HEADER",
            _ => "DIB Header",
        };

        let mut dib_header = FileSection::new(
            format!("DIB Header ({})", dib_header_name),
            14,
            dib_header_end,
            RiskLevel::Critical,
        )
        .with_description("Image metadata - editing will likely corrupt the image");

        // Add child sections for common BITMAPINFOHEADER fields
        if dib_header_size >= 40 {
            dib_header = dib_header
                .with_child(FileSection::new("Header Size", 14, 18, RiskLevel::Critical))
                .with_child(FileSection::new("Image Width", 18, 22, RiskLevel::Critical))
                .with_child(FileSection::new(
                    "Image Height",
                    22,
                    26,
                    RiskLevel::Critical,
                ))
                .with_child(FileSection::new(
                    "Color Planes",
                    26,
                    28,
                    RiskLevel::Critical,
                ))
                .with_child(FileSection::new(
                    "Bits Per Pixel",
                    28,
                    30,
                    RiskLevel::Critical,
                ))
                .with_child(FileSection::new("Compression", 30, 34, RiskLevel::Critical))
                .with_child(FileSection::new("Image Size", 34, 38, RiskLevel::Caution))
                .with_child(FileSection::new(
                    "X Pixels Per Meter",
                    38,
                    42,
                    RiskLevel::Safe,
                ))
                .with_child(FileSection::new(
                    "Y Pixels Per Meter",
                    42,
                    46,
                    RiskLevel::Safe,
                ))
                .with_child(FileSection::new("Colors Used", 46, 50, RiskLevel::Caution))
                .with_child(FileSection::new(
                    "Important Colors",
                    50,
                    54,
                    RiskLevel::Safe,
                ));
        }

        sections.push(dib_header);

        // Color Table (if present)
        let color_table_start = dib_header_end;
        if color_table_start < pixel_offset {
            let color_table = FileSection::new(
                "Color Table",
                color_table_start,
                pixel_offset,
                RiskLevel::Caution,
            )
            .with_description("Palette for indexed color images - editing changes colors");
            sections.push(color_table);
        }

        // Pixel Data
        if pixel_offset < data.len() {
            let pixel_data =
                FileSection::new("Pixel Data", pixel_offset, data.len(), RiskLevel::Safe)
                    .with_description("Image pixel data - the fun part to glitch!");
            sections.push(pixel_data);
        }

        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_parse_bmp() {
        let parser = BmpParser;

        // Valid BMP signature
        assert!(parser.can_parse(b"BM\x00\x00\x00\x00"));

        // Invalid signatures
        assert!(!parser.can_parse(b""));
        assert!(!parser.can_parse(b"B"));
        assert!(!parser.can_parse(b"PNG"));
    }

    #[test]
    fn test_parse_minimal_bmp() {
        let parser = BmpParser;

        // Minimal BMP header (14 bytes file header + 40 bytes DIB header)
        let mut bmp = vec![0u8; 54 + 4]; // Header + 1 pixel

        // File header
        bmp[0] = b'B';
        bmp[1] = b'M';
        // File size
        bmp[2] = 58;
        bmp[3] = 0;
        bmp[4] = 0;
        bmp[5] = 0;
        // Pixel data offset
        bmp[10] = 54;
        bmp[11] = 0;
        bmp[12] = 0;
        bmp[13] = 0;

        // DIB header
        bmp[14] = 40;
        bmp[15] = 0;
        bmp[16] = 0;
        bmp[17] = 0; // Header size

        let sections = parser.parse(&bmp).unwrap();

        assert_eq!(sections.len(), 3); // File header, DIB header, pixel data
        assert_eq!(sections[0].name, "File Header");
        assert!(sections[1].name.contains("BITMAPINFOHEADER"));
        assert_eq!(sections[2].name, "Pixel Data");
    }
}
