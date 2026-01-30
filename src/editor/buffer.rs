//! Byte buffer management with edit tracking
//!
//! ## Dual-Buffer Architecture
//!
//! The editor maintains two separate byte vectors:
//!
//! - `original`: Loaded once when a file is opened, never modified afterward.
//!   This serves as the reference point for comparison views and as the base
//!   state for save point restoration.
//!
//! - `working`: All user edits apply to this buffer. Undo/redo operations
//!   manipulate this buffer. The image preview renders from this buffer.
//!
//! This separation ensures:
//! 1. The original file is never accidentally modified
//! 2. Comparison view always shows the true original
//! 3. Save points can efficiently diff against a known base
//! 4. Export writes working buffer to a new location

use super::history::{EditOperation, History};

/// Which nibble (half-byte) is currently being edited
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NibblePosition {
    /// High nibble (first hex digit, bits 7-4)
    High,
    /// Low nibble (second hex digit, bits 3-0)
    Low,
}

/// Editor state containing buffers and edit history
pub struct EditorState {
    /// Original bytes loaded from file (immutable after load)
    original: Vec<u8>,

    /// Working buffer where all edits are applied
    working: Vec<u8>,

    /// Edit history for undo/redo
    history: History,

    /// Current cursor position in the buffer
    cursor: usize,

    /// Which nibble is being edited at the cursor
    nibble: NibblePosition,

    /// Selection range (start, end) - None if no selection
    selection: Option<(usize, usize)>,

    /// Selection anchor point - where selection started (for Shift+click/arrow)
    selection_anchor: Option<usize>,

    /// Whether the working buffer has unsaved changes
    modified: bool,
}

impl EditorState {
    /// Create a new editor state from file bytes
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            working: bytes.clone(),
            original: bytes,
            history: History::new(),
            cursor: 0,
            nibble: NibblePosition::High,
            selection: None,
            selection_anchor: None,
            modified: false,
        }
    }

    /// Get a reference to the original (unmodified) bytes
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// Get a reference to the working (edited) bytes
    pub fn working(&self) -> &[u8] {
        &self.working
    }

    /// Get a mutable reference to the working buffer
    pub fn working_mut(&mut self) -> &mut Vec<u8> {
        &mut self.working
    }

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

    /// Edit the current nibble with a hex digit (0-15)
    /// Returns true if cursor should advance to next byte
    pub fn edit_nibble(&mut self, nibble_value: u8) -> bool {
        if self.cursor >= self.working.len() || nibble_value > 15 {
            return false;
        }

        let current = self.working[self.cursor];
        let new_value = match self.nibble {
            NibblePosition::High => (nibble_value << 4) | (current & 0x0F),
            NibblePosition::Low => (current & 0xF0) | nibble_value,
        };

        if current != new_value {
            // Record the edit for undo
            self.history.push(EditOperation::Single {
                offset: self.cursor,
                old_value: current,
                new_value,
            });
            self.working[self.cursor] = new_value;
            self.modified = true;
        }

        // Toggle nibble position
        match self.nibble {
            NibblePosition::High => {
                self.nibble = NibblePosition::Low;
                false // Don't advance cursor yet
            }
            NibblePosition::Low => {
                self.nibble = NibblePosition::High;
                // Advance cursor to next byte
                if self.cursor + 1 < self.working.len() {
                    self.cursor += 1;
                }
                true // Cursor advanced
            }
        }
    }

    /// Get the current selection range
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// Set selection range
    pub fn set_selection(&mut self, start: usize, end: usize) {
        let start = start.min(self.working.len());
        let end = end.min(self.working.len());
        self.selection = Some((start.min(end), start.max(end)));
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.selection_anchor = None;
    }

    /// Start a new selection at the current cursor position
    pub fn start_selection(&mut self) {
        self.selection_anchor = Some(self.cursor);
        self.selection = Some((self.cursor, self.cursor + 1));
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

    /// Check if buffer has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Edit a single byte at the given offset
    pub fn edit_byte(&mut self, offset: usize, new_value: u8) {
        if offset >= self.working.len() {
            return;
        }

        let old_value = self.working[offset];
        if old_value == new_value {
            return;
        }

        // Record the edit for undo
        self.history.push(EditOperation::Single {
            offset,
            old_value,
            new_value,
        });

        // Apply the edit
        self.working[offset] = new_value;
        self.modified = true;
    }

    /// Edit multiple bytes starting at offset
    pub fn edit_bytes(&mut self, offset: usize, new_values: &[u8]) {
        if offset >= self.working.len() {
            return;
        }

        let end = (offset + new_values.len()).min(self.working.len());
        let old_values: Vec<u8> = self.working[offset..end].to_vec();
        let actual_new: Vec<u8> = new_values[..end - offset].to_vec();

        if old_values == actual_new {
            return;
        }

        // Record the edit for undo
        self.history.push(EditOperation::Range {
            offset,
            old_values,
            new_values: actual_new.clone(),
        });

        // Apply the edit
        self.working[offset..end].copy_from_slice(&actual_new);
        self.modified = true;
    }

    /// Undo the last edit operation
    pub fn undo(&mut self) -> bool {
        if let Some(op) = self.history.undo() {
            match op {
                EditOperation::Single {
                    offset, old_value, ..
                } => {
                    self.working[offset] = old_value;
                }
                EditOperation::Range {
                    offset, old_values, ..
                } => {
                    let end = offset + old_values.len();
                    self.working[offset..end].copy_from_slice(&old_values);
                }
            }
            // Check if we're back to original state
            self.modified = self.working != self.original;
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) -> bool {
        if let Some(op) = self.history.redo() {
            match op {
                EditOperation::Single {
                    offset, new_value, ..
                } => {
                    self.working[offset] = new_value;
                }
                EditOperation::Range {
                    offset, new_values, ..
                } => {
                    let end = offset + new_values.len();
                    self.working[offset..end].copy_from_slice(&new_values);
                }
            }
            self.modified = self.working != self.original;
            true
        } else {
            false
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Get the byte at the cursor position
    pub fn byte_at_cursor(&self) -> Option<u8> {
        self.working.get(self.cursor).copied()
    }

    /// Get a slice of bytes for display
    pub fn bytes_in_range(&self, start: usize, end: usize) -> &[u8] {
        let start = start.min(self.working.len());
        let end = end.min(self.working.len());
        &self.working[start..end]
    }

    /// Total number of bytes
    pub fn len(&self) -> usize {
        self.working.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.working.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor_state() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let editor = EditorState::new(data.clone());

        assert_eq!(editor.original(), &data);
        assert_eq!(editor.working(), &data);
        assert!(!editor.is_modified());
    }

    #[test]
    fn test_edit_byte() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.edit_byte(1, 0xFF);

        assert_eq!(editor.original(), &data);
        assert_eq!(editor.working(), &[0x00, 0xFF, 0x02, 0x03]);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_undo_redo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.edit_byte(1, 0xFF);
        assert_eq!(editor.working()[1], 0xFF);

        assert!(editor.undo());
        assert_eq!(editor.working()[1], 0x01);
        assert!(!editor.is_modified());

        assert!(editor.redo());
        assert_eq!(editor.working()[1], 0xFF);
        assert!(editor.is_modified());
    }

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

    #[test]
    fn test_nibble_editing() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Should start at high nibble
        assert_eq!(editor.nibble(), NibblePosition::High);
        assert_eq!(editor.cursor(), 0);

        // Edit high nibble with 'A' (10)
        let advanced = editor.edit_nibble(0xA);
        assert!(!advanced); // Should not advance after high nibble
        assert_eq!(editor.nibble(), NibblePosition::Low);
        assert_eq!(editor.working()[0], 0xA0); // High nibble changed, low stayed

        // Edit low nibble with 'B' (11)
        let advanced = editor.edit_nibble(0xB);
        assert!(advanced); // Should advance after completing byte
        assert_eq!(editor.nibble(), NibblePosition::High);
        assert_eq!(editor.working()[0], 0xAB); // Full byte is now 0xAB
        assert_eq!(editor.cursor(), 1); // Cursor moved to next byte
    }

    #[test]
    fn test_nibble_editing_undo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        // Edit high nibble
        editor.edit_nibble(0xF);
        assert_eq!(editor.working()[0], 0xF0);

        // Edit low nibble (same byte - will be coalesced with high nibble edit)
        editor.edit_nibble(0xE);
        assert_eq!(editor.working()[0], 0xFE);

        // Undo should restore to original state (edits are coalesced)
        // Since both nibble edits are to the same byte and within 500ms,
        // they are coalesced into a single operation
        editor.undo();
        assert_eq!(editor.working()[0], 0x00);

        // No more undo available since both edits were coalesced
        assert!(!editor.can_undo());
    }

    #[test]
    fn test_selection_with_shift_arrow() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        // Start at position 2
        editor.set_cursor(2);
        assert_eq!(editor.cursor(), 2);
        assert!(editor.selection().is_none());

        // Shift+right should select from 2 to 3
        editor.move_cursor_with_selection(1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((2, 4))); // bytes 2, 3 selected

        // Continue extending right
        editor.move_cursor_with_selection(1);
        assert_eq!(editor.cursor(), 4);
        assert_eq!(editor.selection(), Some((2, 5))); // bytes 2, 3, 4 selected

        // Shift+left should shrink selection
        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((2, 4))); // bytes 2, 3 selected
    }

    #[test]
    fn test_selection_backwards() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        // Start at position 4
        editor.set_cursor(4);

        // Shift+left should select from 4 backwards
        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 3);
        assert_eq!(editor.selection(), Some((3, 5))); // bytes 3, 4 selected

        // Continue extending left
        editor.move_cursor_with_selection(-1);
        assert_eq!(editor.cursor(), 2);
        assert_eq!(editor.selection(), Some((2, 5))); // bytes 2, 3, 4 selected
    }

    #[test]
    fn test_set_cursor_with_selection() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        // Start at position 1
        editor.set_cursor(1);

        // Shift+click at position 4 (extend = true)
        editor.set_cursor_with_selection(4, true);
        assert_eq!(editor.cursor(), 4);
        assert_eq!(editor.selection(), Some((1, 5))); // bytes 1, 2, 3, 4 selected

        // Regular click at position 2 (extend = false)
        editor.set_cursor_with_selection(2, false);
        assert_eq!(editor.cursor(), 2);
        assert!(editor.selection().is_none()); // selection cleared
    }

    #[test]
    fn test_selection_with_extend_to() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut editor = EditorState::new(data);

        // Start at position 2
        editor.set_cursor(2);

        // Extend to position 5 (like Shift+End)
        editor.extend_selection_to(5);
        assert_eq!(editor.cursor(), 5);
        assert_eq!(editor.selection(), Some((2, 6))); // bytes 2-5 selected

        // Extend back to position 0 (like Shift+Home)
        editor.extend_selection_to(0);
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.selection(), Some((0, 3))); // bytes 0, 1, 2 selected (anchor at 2)
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

        // After clearing, new selection should start fresh
        editor.move_cursor_with_selection(1);
        // New anchor should be at current cursor (2), selecting to 3
        assert_eq!(editor.selection(), Some((2, 4)));
    }
}
