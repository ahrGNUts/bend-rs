//! Cursor and nibble position management for the hex editor

use super::buffer::EditorState;

/// Which nibble (half-byte) is currently being edited
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NibblePosition {
    /// High nibble (first hex digit, bits 7-4)
    High,
    /// Low nibble (second hex digit, bits 3-0)
    Low,
}

impl EditorState {
    /// Get the current cursor position
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Set the cursor position, clamping to valid range
    /// Also resets nibble position to High
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.working.len().saturating_sub(1));
        self.nibble = NibblePosition::High;
    }

    /// Move cursor by offset, clamping to valid range
    /// Also resets nibble position to High
    pub fn move_cursor(&mut self, offset: isize) {
        let new_pos = if offset < 0 {
            self.cursor.saturating_sub((-offset) as usize)
        } else {
            self.cursor.saturating_add(offset as usize)
        };
        self.set_cursor(new_pos);
    }

    /// Get the current nibble position
    pub fn nibble(&self) -> NibblePosition {
        self.nibble
    }
}

#[cfg(test)]
mod tests {
    use super::super::buffer::EditorState;

    #[test]
    fn test_cursor_movement() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        assert_eq!(editor.cursor(), 0);

        editor.set_cursor(2);
        assert_eq!(editor.cursor(), 2);

        editor.move_cursor(1);
        assert_eq!(editor.cursor(), 3);

        // Should clamp at max
        editor.move_cursor(10);
        assert_eq!(editor.cursor(), 3);

        // Should clamp at 0
        editor.move_cursor(-10);
        assert_eq!(editor.cursor(), 0);
    }
}
