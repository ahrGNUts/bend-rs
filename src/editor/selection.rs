//! Selection management for the hex editor

use super::buffer::EditorState;
use super::cursor::NibblePosition;

impl EditorState {
    /// Get the current selection range
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    /// Extend selection from anchor to the given position
    /// If no anchor exists, sets anchor at current cursor before extending
    pub fn extend_selection_to(&mut self, pos: usize) {
        let pos = pos.min(self.working.len().saturating_sub(1));

        let anchor = self.selection_anchor.unwrap_or(self.cursor);

        let (start, end) = if pos >= anchor {
            (anchor, pos + 1)
        } else {
            (pos, anchor + 1)
        };

        self.selection = Some((start, end.min(self.working.len())));
        self.selection_anchor = Some(anchor);
        self.cursor = pos;
        self.nibble = NibblePosition::High;
    }

    /// Move cursor by offset while extending selection (for Shift+arrow)
    pub fn move_cursor_with_selection(&mut self, offset: isize) {
        // If no anchor, set it at current position first
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor);
        }

        // Calculate new cursor position
        let new_pos = if offset < 0 {
            self.cursor.saturating_sub((-offset) as usize)
        } else {
            self.cursor
                .saturating_add(offset as usize)
                .min(self.working.len().saturating_sub(1))
        };

        self.extend_selection_to(new_pos);
    }

    /// Set cursor with optional selection extension (for Shift+click)
    pub fn set_cursor_with_selection(&mut self, pos: usize, extend: bool) {
        if extend {
            self.extend_selection_to(pos);
        } else {
            self.clear_selection();
            self.set_cursor(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::buffer::EditorState;

    #[test]
    fn test_selection_with_shift_arrow() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        editor.set_cursor(2);
        assert_eq!(editor.cursor(), 2);
        assert!(editor.selection().is_none());

        editor.move_cursor_with_selection(1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((2, 4)));

        editor.move_cursor_with_selection(1);
        assert_eq!(editor.cursor(), 4);
        assert_eq!(editor.selection(), Some((2, 5)));

        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((2, 4)));
    }

    #[test]
    fn test_selection_backwards() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        editor.set_cursor(4);

        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((3, 5)));

        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 2);
        assert_eq!(editor.selection(), Some((2, 5)));
    }

    #[test]
    fn test_set_cursor_with_selection() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        editor.set_cursor(1);

        editor.set_cursor_with_selection(4, true);
        assert_eq!(editor.cursor(), 4);
        assert_eq!(editor.selection(), Some((1, 5)));

        editor.set_cursor_with_selection(2, false);
        assert_eq!(editor.cursor(), 2);
        assert!(editor.selection().is_none());
    }

    #[test]
    fn test_selection_with_extend_to() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        editor.set_cursor(2);

        editor.extend_selection_to(5);
        assert_eq!(editor.cursor(), 5);
        assert_eq!(editor.selection(), Some((2, 6)));

        editor.extend_selection_to(0);
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.selection(), Some((0, 3)));
    }

    #[test]
    fn test_clear_selection_clears_anchor() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        editor.set_cursor(1);
        editor.move_cursor_with_selection(1);
        assert!(editor.selection().is_some());

        editor.clear_selection();
        assert!(editor.selection().is_none());

        editor.move_cursor_with_selection(1);
        assert_eq!(editor.selection(), Some((2, 4)));
    }
}
