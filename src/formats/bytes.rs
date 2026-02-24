//! Shared byte-reading helpers for format parsers

/// Read a big-endian u16 from `data` at `offset`.
pub fn read_u16_be(data: &[u8], offset: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(offset..offset + 2)?.try_into().ok()?;
    Some(u16::from_be_bytes(bytes))
}

/// Read a big-endian u32 from `data` at `offset`.
pub fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

/// Read a little-endian u32 from `data` at `offset`.
pub fn read_u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_u16_be() {
        let data = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(read_u16_be(&data, 0), Some(0x0102));
        assert_eq!(read_u16_be(&data, 2), Some(0x0304));
        assert_eq!(read_u16_be(&data, 3), None); // not enough bytes
        assert_eq!(read_u16_be(&data, 4), None); // out of bounds
    }

    #[test]
    fn test_read_u32_be() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(read_u32_be(&data, 0), Some(0x01020304));
        assert_eq!(read_u32_be(&data, 1), Some(0x02030405));
        assert_eq!(read_u32_be(&data, 2), None); // not enough bytes
        assert_eq!(read_u32_be(&data, 5), None); // out of bounds
    }

    #[test]
    fn test_read_u32_le() {
        let data = [0x04, 0x03, 0x02, 0x01, 0xFF];
        assert_eq!(read_u32_le(&data, 0), Some(0x01020304));
        assert_eq!(read_u32_le(&data, 1), Some(0xFF010203));
        assert_eq!(read_u32_le(&data, 2), None); // not enough bytes
    }

    #[test]
    fn test_empty_data() {
        let data: [u8; 0] = [];
        assert_eq!(read_u16_be(&data, 0), None);
        assert_eq!(read_u32_be(&data, 0), None);
        assert_eq!(read_u32_le(&data, 0), None);
    }
}
