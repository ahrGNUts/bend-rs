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
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.working.len().saturating_sub(1));
    }

    /// Move cursor by offset, clamping to valid range
    pub fn move_cursor(&mut self, offset: isize) {
        let new_pos = if offset < 0 {
            self.cursor.saturating_sub((-offset) as usize)
        } else {
            self.cursor.saturating_add(offset as usize)
        };
        self.set_cursor(new_pos);
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
}
