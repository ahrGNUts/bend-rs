//! Edit mode and write mode management for the hex editor

use super::buffer::EditorState;
use super::cursor::NibblePosition;
use super::history::EditOperation;

/// Whether typing inserts new bytes or overwrites existing ones
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WriteMode {
    /// Overwrite mode: typing replaces existing bytes in-place
    #[default]
    Overwrite,
    /// Insert mode: typing inserts new bytes, shifting subsequent bytes right
    Insert,
}

/// Which editing mode is active (hex nibble vs ASCII character)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EditMode {
    /// Hex editing: two keystrokes per byte (nibble-level)
    #[default]
    Hex,
    /// ASCII editing: one keystroke per byte (character-level)
    Ascii,
}

impl EditorState {
    /// Get the current edit mode
    pub fn edit_mode(&self) -> EditMode {
        self.edit_mode
    }

    /// Set the edit mode
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        self.nibble = NibblePosition::High;
    }

    /// Get the current write mode
    pub fn write_mode(&self) -> WriteMode {
        self.write_mode
    }

    /// Toggle between Insert and Overwrite write modes
    pub fn toggle_write_mode(&mut self) {
        self.write_mode = match self.write_mode {
            WriteMode::Overwrite => WriteMode::Insert,
            WriteMode::Insert => WriteMode::Overwrite,
        };
    }

    /// Edit a nibble respecting the current write mode
    ///
    /// In Overwrite mode, delegates to `edit_nibble()`.
    /// In Insert mode:
    /// - High nibble: inserts a new byte with `nibble << 4`, sets nibble to Low
    /// - Low nibble: overwrites the low nibble of the just-inserted byte, advances cursor
    #[must_use = "returns whether the cursor advanced to the next byte"]
    pub fn edit_nibble_with_mode(&mut self, nibble_value: u8) -> bool {
        if nibble_value > 15 {
            return false;
        }
        match self.write_mode {
            WriteMode::Overwrite => self.edit_nibble(nibble_value),
            WriteMode::Insert => match self.nibble {
                NibblePosition::High => {
                    let value = nibble_value << 4;
                    self.insert_byte(self.cursor, value);
                    self.nibble = NibblePosition::Low;
                    false
                }
                NibblePosition::Low => {
                    if self.cursor < self.working.len() {
                        let current = self.working[self.cursor];
                        let new_value = (current & 0xF0) | nibble_value;
                        if current != new_value {
                            self.history.push(EditOperation::Single {
                                offset: self.cursor,
                                old_value: current,
                                new_value,
                            });
                            self.working[self.cursor] = new_value;
                            self.modified = true;
                        }
                    }
                    self.nibble = NibblePosition::High;
                    if self.cursor + 1 < self.working.len() {
                        self.cursor += 1;
                    }
                    true
                }
            },
        }
    }

    /// Edit an ASCII character respecting the current write mode
    ///
    /// In Overwrite mode, delegates to `edit_ascii()`.
    /// In Insert mode, inserts a new byte and advances cursor.
    #[must_use = "returns whether the character was accepted"]
    pub fn edit_ascii_with_mode(&mut self, ch: char) -> bool {
        if !super::is_printable_ascii_char(ch) {
            return false;
        }
        match self.write_mode {
            WriteMode::Overwrite => self.edit_ascii(ch),
            WriteMode::Insert => {
                let value = ch as u8;
                self.insert_byte(self.cursor, value);
                if self.cursor + 1 < self.working.len() {
                    self.cursor += 1;
                }
                true
            }
        }
    }

    /// Handle Backspace key
    ///
    /// In Overwrite mode: moves cursor left.
    /// In Insert mode: moves cursor left, then deletes byte at cursor.
    pub fn handle_backspace(&mut self) {
        match self.write_mode {
            WriteMode::Overwrite => {
                self.move_cursor(-1);
            }
            WriteMode::Insert => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.nibble = NibblePosition::High;
                    self.delete_byte(self.cursor);
                }
            }
        }
    }

    /// Handle Delete key
    ///
    /// In Overwrite mode: no action.
    /// In Insert mode: deletes byte at cursor.
    pub fn handle_delete(&mut self) {
        if self.write_mode == WriteMode::Insert {
            self.delete_byte(self.cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::buffer::EditorState;
    use super::super::cursor::NibblePosition;
    use super::{EditMode, WriteMode};

    #[test]
    fn test_edit_mode_default() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let editor = EditorState::new(data);
        assert_eq!(editor.edit_mode(), EditMode::Hex);
    }

    #[test]
    fn test_edit_mode_set_toggles() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        assert_eq!(editor.edit_mode(), EditMode::Hex);
        editor.set_edit_mode(EditMode::Ascii);
        assert_eq!(editor.edit_mode(), EditMode::Ascii);
        editor.set_edit_mode(EditMode::Hex);
        assert_eq!(editor.edit_mode(), EditMode::Hex);
    }

    #[test]
    fn test_set_edit_mode_resets_nibble() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let _ = editor.edit_nibble(0xA);
        assert_eq!(editor.nibble(), NibblePosition::Low);

        editor.set_edit_mode(EditMode::Ascii);
        assert_eq!(editor.nibble(), NibblePosition::High);
    }

    #[test]
    fn test_set_edit_mode() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let _ = editor.edit_nibble(0xA);
        assert_eq!(editor.nibble(), NibblePosition::Low);

        editor.set_edit_mode(EditMode::Ascii);
        assert_eq!(editor.edit_mode(), EditMode::Ascii);
        assert_eq!(editor.nibble(), NibblePosition::High);

        editor.set_edit_mode(EditMode::Hex);
        assert_eq!(editor.edit_mode(), EditMode::Hex);
        assert_eq!(editor.nibble(), NibblePosition::High);
    }

    #[test]
    fn test_write_mode_default() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let editor = EditorState::new(data);
        assert_eq!(editor.write_mode(), WriteMode::Overwrite);
    }

    #[test]
    fn test_write_mode_toggle() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        editor.toggle_write_mode();
        assert_eq!(editor.write_mode(), WriteMode::Insert);

        editor.toggle_write_mode();
        assert_eq!(editor.write_mode(), WriteMode::Overwrite);
    }

    #[test]
    fn test_edit_nibble_with_mode_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let advanced = editor.edit_nibble_with_mode(0xA);
        assert!(!advanced);
        assert_eq!(editor.working()[0], 0xA0);
        assert_eq!(editor.nibble(), NibblePosition::Low);

        let advanced = editor.edit_nibble_with_mode(0xB);
        assert!(advanced);
        assert_eq!(editor.working()[0], 0xAB);
        assert_eq!(editor.cursor(), 1);
    }

    #[test]
    fn test_edit_nibble_with_mode_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.toggle_write_mode();

        let advanced = editor.edit_nibble_with_mode(0xA);
        assert!(!advanced);
        assert_eq!(editor.len(), 5);
        assert_eq!(editor.working()[0], 0xA0);
        assert_eq!(editor.nibble(), NibblePosition::Low);
        assert_eq!(editor.cursor(), 0);

        let advanced = editor.edit_nibble_with_mode(0xB);
        assert!(advanced);
        assert_eq!(editor.working()[0], 0xAB);
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.nibble(), NibblePosition::High);
        assert_eq!(editor.working(), &[0xAB, 0x00, 0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_edit_nibble_insert_undo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());
        editor.toggle_write_mode();

        let _ = editor.edit_nibble_with_mode(0xA);
        let _ = editor.edit_nibble_with_mode(0xB);
        assert_eq!(editor.working(), &[0xAB, 0x00, 0x01, 0x02, 0x03]);

        let _ = editor.undo();
        assert_eq!(editor.working()[0], 0xA0);

        let _ = editor.undo();
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_edit_ascii_with_mode_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        assert!(editor.edit_ascii_with_mode('A'));
        assert_eq!(editor.working()[0], 0x41);
        assert_eq!(editor.cursor(), 1);
    }

    #[test]
    fn test_edit_ascii_with_mode_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.toggle_write_mode();

        assert!(editor.edit_ascii_with_mode('H'));
        assert_eq!(editor.len(), 5);
        assert_eq!(editor.working()[0], b'H');
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.working(), &[b'H', 0x00, 0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_handle_backspace_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.set_cursor(2);
        editor.handle_backspace();
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_handle_backspace_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.toggle_write_mode();

        editor.set_cursor(2);
        editor.handle_backspace();
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
    }

    #[test]
    fn test_handle_backspace_insert_at_start() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data.clone());
        editor.toggle_write_mode();

        editor.set_cursor(0);
        editor.handle_backspace();
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_handle_delete_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.set_cursor(1);
        editor.handle_delete();
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_handle_delete_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.toggle_write_mode();

        editor.set_cursor(1);
        editor.handle_delete();
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
        assert_eq!(editor.cursor(), 1);
    }
}
