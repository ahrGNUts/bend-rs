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

        // Edit low nibble
        editor.edit_nibble(0xE);
        assert_eq!(editor.working()[0], 0xFE);

        // Undo should restore previous state
        editor.undo();
        assert_eq!(editor.working()[0], 0xF0);

        editor.undo();
        assert_eq!(editor.working()[0], 0x00);
    }
}
