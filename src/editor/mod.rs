//! Editor module: buffer management, history, and edit operations

pub mod bookmarks;
pub mod buffer;
mod cursor;
pub mod go_to_offset;
mod history;
mod modes;
pub mod savepoints;
pub mod search;
mod selection;

pub use buffer::EditorState;
pub use go_to_offset::GoToOffsetState;
pub use search::SearchState;

/// Check if a byte is printable ASCII (space 0x20 through tilde 0x7E).
///
/// This covers the same range as `is_ascii_graphic() || b == b' '`.
#[inline]
pub fn is_printable_ascii(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

/// Check if a char is printable ASCII (space 0x20 through tilde 0x7E).
///
/// Char variant of [`is_printable_ascii`] for call sites that work with `char`.
#[inline]
pub fn is_printable_ascii_char(ch: char) -> bool {
    (0x20..=0x7E).contains(&(ch as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_printable_ascii() {
        assert!(is_printable_ascii(b' ')); // 0x20 lower bound
        assert!(is_printable_ascii(b'~')); // 0x7E upper bound
        assert!(is_printable_ascii(b'A'));
        assert!(!is_printable_ascii(0x19)); // below range
        assert!(!is_printable_ascii(0x7F)); // DEL
        assert!(!is_printable_ascii(0x00)); // NUL
    }

    #[test]
    fn test_is_printable_ascii_char() {
        assert!(is_printable_ascii_char(' '));
        assert!(is_printable_ascii_char('~'));
        assert!(is_printable_ascii_char('A'));
        assert!(!is_printable_ascii_char('\t'));
        assert!(!is_printable_ascii_char('\n'));
        assert!(!is_printable_ascii_char('\x7F'));
    }
}
