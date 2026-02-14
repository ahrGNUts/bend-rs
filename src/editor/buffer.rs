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

use super::bookmarks::BookmarkManager;
use super::history::{EditOperation, History};
use super::savepoints::{SavePoint, SavePointManager};

/// Which nibble (half-byte) is currently being edited
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NibblePosition {
    /// High nibble (first hex digit, bits 7-4)
    High,
    /// Low nibble (second hex digit, bits 3-0)
    Low,
}

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

/// Editor state containing buffers and edit history
pub struct EditorState {
    /// Original bytes loaded from file (immutable after load)
    original: Vec<u8>,

    /// Working buffer where all edits are applied
    working: Vec<u8>,

    /// Edit history for undo/redo
    history: History,

    /// Save point manager for named snapshots
    save_points: SavePointManager,

    /// Bookmark manager for annotated locations
    bookmarks: BookmarkManager,

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

    /// Current editing mode (hex nibble vs ASCII character)
    edit_mode: EditMode,

    /// Current write mode (insert vs overwrite)
    write_mode: WriteMode,

    /// Whether buffer length changed since last check (for UI cache invalidation)
    length_changed: bool,
}

impl EditorState {
    /// Create a new editor state from file bytes
    pub fn new(bytes: Vec<u8>) -> Self {
        let save_points = SavePointManager::new(&bytes);
        Self {
            working: bytes.clone(),
            original: bytes,
            history: History::new(),
            save_points,
            bookmarks: BookmarkManager::new(),
            cursor: 0,
            nibble: NibblePosition::High,
            selection: None,
            selection_anchor: None,
            modified: false,
            edit_mode: EditMode::default(),
            write_mode: WriteMode::default(),
            length_changed: false,
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

    /// Get the current edit mode
    pub fn edit_mode(&self) -> EditMode {
        self.edit_mode
    }

    /// Set the edit mode
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        // Reset nibble to High when switching modes
        self.nibble = NibblePosition::High;
    }

    /// Toggle between Hex and ASCII editing modes
    pub fn toggle_edit_mode(&mut self) {
        self.edit_mode = match self.edit_mode {
            EditMode::Hex => EditMode::Ascii,
            EditMode::Ascii => EditMode::Hex,
        };
        // Reset nibble to High when switching modes
        self.nibble = NibblePosition::High;
    }

    /// Get the current write mode
    pub fn write_mode(&self) -> WriteMode {
        self.write_mode
    }

    /// Set the write mode
    pub fn set_write_mode(&mut self, mode: WriteMode) {
        self.write_mode = mode;
    }

    /// Toggle between Insert and Overwrite write modes
    pub fn toggle_write_mode(&mut self) {
        self.write_mode = match self.write_mode {
            WriteMode::Overwrite => WriteMode::Insert,
            WriteMode::Insert => WriteMode::Overwrite,
        };
    }

    /// Check and reset the length_changed flag (returns true if length changed since last call)
    pub fn take_length_changed(&mut self) -> bool {
        let changed = self.length_changed;
        self.length_changed = false;
        changed
    }

    /// Edit the current byte with an ASCII character
    /// Only accepts printable ASCII (0x20-0x7E)
    /// Returns true if the character was accepted, false if rejected
    #[must_use = "returns whether the character was accepted"]
    pub fn edit_ascii(&mut self, ch: char) -> bool {
        // Only accept printable ASCII characters (space through tilde)
        let byte_value = ch as u32;
        if byte_value < 0x20 || byte_value > 0x7E {
            return false;
        }

        if self.cursor >= self.working.len() {
            return false;
        }

        let new_value = byte_value as u8;
        let current = self.working[self.cursor];

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

        // Advance cursor to next byte
        if self.cursor + 1 < self.working.len() {
            self.cursor += 1;
        }

        true
    }

    /// Edit the current nibble with a hex digit (0-15)
    /// Returns true if cursor should advance to next byte
    #[must_use = "returns whether the cursor advanced to the next byte"]
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

    // ========== Mode-Aware Editing ==========

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
            WriteMode::Insert => {
                match self.nibble {
                    NibblePosition::High => {
                        // Insert a new byte with the high nibble set
                        let value = nibble_value << 4;
                        self.insert_byte(self.cursor, value);
                        // Stay on this byte, move to low nibble
                        self.nibble = NibblePosition::Low;
                        false
                    }
                    NibblePosition::Low => {
                        // Overwrite the low nibble of the just-inserted byte
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
                        // Advance cursor to next byte
                        if self.cursor + 1 < self.working.len() {
                            self.cursor += 1;
                        }
                        true
                    }
                }
            }
        }
    }

    /// Edit an ASCII character respecting the current write mode
    ///
    /// In Overwrite mode, delegates to `edit_ascii()`.
    /// In Insert mode, inserts a new byte and advances cursor.
    #[must_use = "returns whether the character was accepted"]
    pub fn edit_ascii_with_mode(&mut self, ch: char) -> bool {
        let byte_value = ch as u32;
        if byte_value < 0x20 || byte_value > 0x7E {
            return false;
        }
        match self.write_mode {
            WriteMode::Overwrite => self.edit_ascii(ch),
            WriteMode::Insert => {
                let value = byte_value as u8;
                self.insert_byte(self.cursor, value);
                // Advance cursor past the inserted byte
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

    // ========== Insert/Delete Operations ==========

    /// Called after any operation that changes buffer length.
    /// Clears save points, adjusts bookmarks, and sets the length_changed flag.
    fn on_length_changed(&mut self, offset: usize, count: usize, is_insert: bool) {
        // Save points use absolute offsets — invalidate them all
        self.save_points.clear_all(&self.original);
        // Adjust bookmark offsets
        if is_insert {
            self.bookmarks.adjust_offsets_after_insert(offset, count);
        } else {
            self.bookmarks.adjust_offsets_after_delete(offset, count);
        }
        self.length_changed = true;
    }

    /// Insert a single byte at the given offset
    pub fn insert_byte(&mut self, offset: usize, value: u8) {
        let offset = offset.min(self.working.len());
        self.working.insert(offset, value);
        self.history.push(EditOperation::InsertBytes {
            offset,
            values: vec![value],
        });
        self.modified = true;
        self.on_length_changed(offset, 1, true);
    }

    /// Insert multiple bytes at the given offset
    pub fn insert_bytes(&mut self, offset: usize, values: &[u8]) {
        if values.is_empty() {
            return;
        }
        let offset = offset.min(self.working.len());
        self.working.splice(offset..offset, values.iter().copied());
        self.history.push(EditOperation::InsertBytes {
            offset,
            values: values.to_vec(),
        });
        self.modified = true;
        self.on_length_changed(offset, values.len(), true);
    }

    /// Delete the byte at the given offset, returning the deleted value
    pub fn delete_byte(&mut self, offset: usize) -> Option<u8> {
        if offset >= self.working.len() {
            return None;
        }
        let value = self.working.remove(offset);
        self.history.push(EditOperation::DeleteBytes {
            offset,
            values: vec![value],
        });
        self.modified = true;
        // Clamp cursor if it now points past the end
        if !self.working.is_empty() {
            self.cursor = self.cursor.min(self.working.len() - 1);
        }
        self.on_length_changed(offset, 1, false);
        Some(value)
    }

    /// Delete a range of bytes starting at offset
    pub fn delete_bytes_range(&mut self, offset: usize, count: usize) {
        if offset >= self.working.len() || count == 0 {
            return;
        }
        let end = (offset + count).min(self.working.len());
        let actual_count = end - offset;
        let values: Vec<u8> = self.working.drain(offset..end).collect();
        self.history.push(EditOperation::DeleteBytes {
            offset,
            values,
        });
        self.modified = true;
        if !self.working.is_empty() {
            self.cursor = self.cursor.min(self.working.len() - 1);
        }
        self.on_length_changed(offset, actual_count, false);
    }

    /// Undo the last edit operation
    #[must_use = "returns whether an operation was undone"]
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
                EditOperation::InsertBytes { offset, ref values } => {
                    // Undo insert: remove the inserted bytes
                    let count = values.len();
                    self.working.drain(offset..offset + count);
                    self.bookmarks.adjust_offsets_after_delete(offset, count);
                    self.save_points.clear_all(&self.original);
                    self.length_changed = true;
                    if !self.working.is_empty() {
                        self.cursor = self.cursor.min(self.working.len() - 1);
                    }
                }
                EditOperation::DeleteBytes { offset, ref values } => {
                    // Undo delete: re-insert the deleted bytes
                    let count = values.len();
                    self.working.splice(offset..offset, values.iter().copied());
                    self.bookmarks.adjust_offsets_after_insert(offset, count);
                    self.save_points.clear_all(&self.original);
                    self.length_changed = true;
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
    #[must_use = "returns whether an operation was redone"]
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
                EditOperation::InsertBytes { offset, ref values } => {
                    // Redo insert: splice bytes back in
                    let count = values.len();
                    self.working.splice(offset..offset, values.iter().copied());
                    self.bookmarks.adjust_offsets_after_insert(offset, count);
                    self.save_points.clear_all(&self.original);
                    self.length_changed = true;
                }
                EditOperation::DeleteBytes { offset, ref values } => {
                    // Redo delete: drain bytes again
                    let count = values.len();
                    self.working.drain(offset..offset + count);
                    self.bookmarks.adjust_offsets_after_delete(offset, count);
                    self.save_points.clear_all(&self.original);
                    self.length_changed = true;
                    if !self.working.is_empty() {
                        self.cursor = self.cursor.min(self.working.len() - 1);
                    }
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

    // ========== Save Points ==========

    /// Create a new save point with the current state
    pub fn create_save_point(&mut self, name: String) -> u64 {
        self.save_points.create(name, &self.working)
    }

    /// Get all save points
    pub fn save_points(&self) -> &[SavePoint] {
        self.save_points.save_points()
    }

    /// Restore the buffer to a specific save point
    ///
    /// This operation is undoable - the entire restoration is recorded as a
    /// single edit operation.
    ///
    /// Returns true if restoration was successful
    #[must_use = "returns whether the restore was successful"]
    pub fn restore_save_point(&mut self, id: u64) -> bool {
        let Some(restored) = self.save_points.restore(id, &self.original) else {
            return false;
        };

        // Record the restoration as a range edit for undo support
        let old_values = self.working.clone();
        let new_values = restored.clone();

        // Only record if there's actually a change
        if old_values != new_values {
            self.history.push(super::history::EditOperation::Range {
                offset: 0,
                old_values,
                new_values: new_values.clone(),
            });
        }

        self.working = restored;
        self.modified = self.working != self.original;
        true
    }

    /// Rename a save point
    #[must_use = "returns whether the save point was found and renamed"]
    pub fn rename_save_point(&mut self, id: u64, new_name: String) -> bool {
        self.save_points.rename(id, new_name)
    }

    /// Check if a save point can be deleted
    pub fn can_delete_save_point(&self, id: u64) -> bool {
        self.save_points.can_delete(id)
    }

    /// Delete a save point (only leaf save points can be deleted)
    #[must_use = "returns whether the save point was deleted"]
    pub fn delete_save_point(&mut self, id: u64) -> bool {
        self.save_points.delete(id)
    }

    /// Get the number of save points
    pub fn save_point_count(&self) -> usize {
        self.save_points.len()
    }

    // ========== Bookmarks ==========

    /// Add a bookmark at the given offset
    pub fn add_bookmark(&mut self, offset: usize, name: String) -> u64 {
        self.bookmarks.add(offset, name)
    }

    /// Remove a bookmark by ID
    #[must_use = "returns whether the bookmark was found and removed"]
    pub fn remove_bookmark(&mut self, id: u64) -> bool {
        self.bookmarks.remove(id)
    }

    /// Get all bookmarks
    pub fn bookmarks(&self) -> &super::bookmarks::BookmarkManager {
        &self.bookmarks
    }

    /// Get mutable access to bookmarks
    pub fn bookmarks_mut(&mut self) -> &mut super::bookmarks::BookmarkManager {
        &mut self.bookmarks
    }

    /// Check if there's a bookmark at the given offset
    pub fn has_bookmark_at(&self, offset: usize) -> bool {
        self.bookmarks.has_bookmark(offset)
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

    #[test]
    fn test_save_point_create_and_restore() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        // Make some edits
        editor.edit_nibble(0xA);
        editor.edit_nibble(0xB);
        assert_eq!(editor.working()[0], 0xAB);

        // Create a save point
        let sp_id = editor.create_save_point("First checkpoint".to_string());
        assert_eq!(editor.save_point_count(), 1);

        // Make more edits
        editor.edit_nibble(0xC);
        editor.edit_nibble(0xD);
        assert_eq!(editor.working()[1], 0xCD);

        // Restore to save point
        assert!(editor.restore_save_point(sp_id));
        assert_eq!(editor.working()[0], 0xAB);
        assert_eq!(editor.working()[1], 0x01); // Should be back to original

        // Undo should restore the state before the save point restoration
        assert!(editor.undo());
        assert_eq!(editor.working()[1], 0xCD);
    }

    #[test]
    fn test_save_point_rename() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let sp_id = editor.create_save_point("Original name".to_string());
        assert!(editor.rename_save_point(sp_id, "New name".to_string()));

        let sps = editor.save_points();
        assert_eq!(sps[0].name, "New name");
    }

    #[test]
    fn test_save_point_delete() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let sp1 = editor.create_save_point("SP1".to_string());
        let sp2 = editor.create_save_point("SP2".to_string());

        // Can only delete leaf (sp2)
        assert!(!editor.can_delete_save_point(sp1));
        assert!(editor.can_delete_save_point(sp2));

        assert!(editor.delete_save_point(sp2));
        assert_eq!(editor.save_point_count(), 1);

        // Now sp1 is the leaf
        assert!(editor.delete_save_point(sp1));
        assert_eq!(editor.save_point_count(), 0);
    }

    #[test]
    fn test_edit_mode_default() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let editor = EditorState::new(data);

        // Default should be Hex mode
        assert_eq!(editor.edit_mode(), EditMode::Hex);
    }

    #[test]
    fn test_edit_mode_toggle() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        assert_eq!(editor.edit_mode(), EditMode::Hex);

        editor.toggle_edit_mode();
        assert_eq!(editor.edit_mode(), EditMode::Ascii);

        editor.toggle_edit_mode();
        assert_eq!(editor.edit_mode(), EditMode::Hex);
    }

    #[test]
    fn test_edit_mode_toggle_resets_nibble() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Edit high nibble to move to low nibble
        editor.edit_nibble(0xA);
        assert_eq!(editor.nibble(), NibblePosition::Low);

        // Toggle should reset nibble to High
        editor.toggle_edit_mode();
        assert_eq!(editor.nibble(), NibblePosition::High);
    }

    #[test]
    fn test_edit_ascii_printable() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Edit with 'A' (0x41)
        assert!(editor.edit_ascii('A'));
        assert_eq!(editor.working()[0], 0x41);
        assert_eq!(editor.cursor(), 1); // Cursor should advance

        // Edit with space (0x20) - lower bound of printable
        assert!(editor.edit_ascii(' '));
        assert_eq!(editor.working()[1], 0x20);
        assert_eq!(editor.cursor(), 2);

        // Edit with '~' (0x7E) - upper bound of printable
        assert!(editor.edit_ascii('~'));
        assert_eq!(editor.working()[2], 0x7E);
        assert_eq!(editor.cursor(), 3);
    }

    #[test]
    fn test_edit_ascii_rejects_non_printable() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Try tab (0x09) - should be rejected
        assert!(!editor.edit_ascii('\t'));
        assert_eq!(editor.working()[0], 0x00); // Unchanged
        assert_eq!(editor.cursor(), 0); // Cursor should not advance

        // Try newline (0x0A) - should be rejected
        assert!(!editor.edit_ascii('\n'));
        assert_eq!(editor.working()[0], 0x00);

        // Try DEL (0x7F) - should be rejected
        assert!(!editor.edit_ascii('\x7F'));
        assert_eq!(editor.working()[0], 0x00);
    }

    #[test]
    fn test_edit_ascii_undo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.edit_ascii('A');
        assert_eq!(editor.working()[0], 0x41);

        editor.undo();
        assert_eq!(editor.working()[0], 0x00);
        assert!(!editor.is_modified());
    }

    #[test]
    fn test_edit_ascii_at_end() {
        let data = vec![0x00, 0x01];
        let mut editor = EditorState::new(data);

        // Move cursor to last byte
        editor.set_cursor(1);

        // Edit - should work but cursor stays at last position
        assert!(editor.edit_ascii('Z'));
        assert_eq!(editor.working()[1], b'Z');
        assert_eq!(editor.cursor(), 1); // Cursor stays at last byte (can't advance past end)
    }

    #[test]
    fn test_set_edit_mode() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Edit high nibble to move to low nibble
        editor.edit_nibble(0xA);
        assert_eq!(editor.nibble(), NibblePosition::Low);

        // Set mode should reset nibble
        editor.set_edit_mode(EditMode::Ascii);
        assert_eq!(editor.edit_mode(), EditMode::Ascii);
        assert_eq!(editor.nibble(), NibblePosition::High);

        editor.set_edit_mode(EditMode::Hex);
        assert_eq!(editor.edit_mode(), EditMode::Hex);
        assert_eq!(editor.nibble(), NibblePosition::High);
    }

    // ========== WriteMode Tests ==========

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

    // ========== Insert/Delete Tests ==========

    #[test]
    fn test_insert_byte() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        editor.insert_byte(1, 0xFF);
        assert_eq!(editor.working(), &[0x00, 0xFF, 0x01, 0x02, 0x03]);
        assert_eq!(editor.len(), 5);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_insert_byte_at_start() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data);

        editor.insert_byte(0, 0xFF);
        assert_eq!(editor.working(), &[0xFF, 0x00, 0x01, 0x02]);
    }

    #[test]
    fn test_insert_byte_at_end() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data);

        editor.insert_byte(3, 0xFF);
        assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0xFF]);
    }

    #[test]
    fn test_insert_bytes() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        editor.insert_bytes(2, &[0xAA, 0xBB]);
        assert_eq!(editor.working(), &[0x00, 0x01, 0xAA, 0xBB, 0x02, 0x03]);
        assert_eq!(editor.len(), 6);
    }

    #[test]
    fn test_delete_byte() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        let deleted = editor.delete_byte(1);
        assert_eq!(deleted, Some(0x01));
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
        assert_eq!(editor.len(), 3);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_delete_byte_out_of_bounds() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data);

        let deleted = editor.delete_byte(10);
        assert_eq!(deleted, None);
        assert_eq!(editor.len(), 3);
    }

    #[test]
    fn test_delete_bytes_range() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04];
        let mut editor = EditorState::new(data);

        editor.delete_bytes_range(1, 2);
        assert_eq!(editor.working(), &[0x00, 0x03, 0x04]);
        assert_eq!(editor.len(), 3);
    }

    #[test]
    fn test_insert_byte_undo_redo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.insert_byte(1, 0xFF);
        assert_eq!(editor.working(), &[0x00, 0xFF, 0x01, 0x02, 0x03]);

        // Undo should remove the inserted byte
        assert!(editor.undo());
        assert_eq!(editor.working(), &data);
        assert!(!editor.is_modified());

        // Redo should re-insert
        assert!(editor.redo());
        assert_eq!(editor.working(), &[0x00, 0xFF, 0x01, 0x02, 0x03]);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_delete_byte_undo_redo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.delete_byte(1);
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);

        // Undo should re-insert the deleted byte
        assert!(editor.undo());
        assert_eq!(editor.working(), &data);
        assert!(!editor.is_modified());

        // Redo should delete again
        assert!(editor.redo());
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
    }

    #[test]
    fn test_insert_bytes_undo() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data.clone());

        editor.insert_bytes(1, &[0xAA, 0xBB]);
        assert_eq!(editor.working(), &[0x00, 0xAA, 0xBB, 0x01, 0x02]);

        assert!(editor.undo());
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_length_changed_flag() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Flag should be false initially
        assert!(!editor.take_length_changed());

        // Insert should set the flag
        editor.insert_byte(0, 0xFF);
        assert!(editor.take_length_changed());

        // Should be cleared after take
        assert!(!editor.take_length_changed());

        // Delete should also set the flag
        editor.delete_byte(0);
        assert!(editor.take_length_changed());
    }

    // ========== Mode-Aware Editing Tests ==========

    #[test]
    fn test_edit_nibble_with_mode_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // In overwrite mode, should behave like edit_nibble
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
        editor.set_write_mode(WriteMode::Insert);

        // High nibble: inserts a new byte
        let advanced = editor.edit_nibble_with_mode(0xA);
        assert!(!advanced);
        assert_eq!(editor.len(), 5); // Byte inserted
        assert_eq!(editor.working()[0], 0xA0); // High nibble set
        assert_eq!(editor.nibble(), NibblePosition::Low);
        assert_eq!(editor.cursor(), 0); // Still on the inserted byte

        // Low nibble: overwrites low nibble of inserted byte
        let advanced = editor.edit_nibble_with_mode(0xB);
        assert!(advanced);
        assert_eq!(editor.working()[0], 0xAB); // Full byte
        assert_eq!(editor.cursor(), 1); // Advanced past inserted byte
        assert_eq!(editor.nibble(), NibblePosition::High);

        // Original bytes shifted right
        assert_eq!(editor.working(), &[0xAB, 0x00, 0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_edit_nibble_insert_undo() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());
        editor.set_write_mode(WriteMode::Insert);

        // Insert 0xAB: two operations (InsertBytes + Single overwrite)
        editor.edit_nibble_with_mode(0xA);
        editor.edit_nibble_with_mode(0xB);
        assert_eq!(editor.working(), &[0xAB, 0x00, 0x01, 0x02, 0x03]);

        // Undo low nibble overwrite
        editor.undo();
        assert_eq!(editor.working()[0], 0xA0); // Back to just high nibble

        // Undo the insert
        editor.undo();
        assert_eq!(editor.working(), &data); // Back to original
    }

    #[test]
    fn test_edit_ascii_with_mode_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Overwrite mode: should behave like edit_ascii
        assert!(editor.edit_ascii_with_mode('A'));
        assert_eq!(editor.working()[0], 0x41);
        assert_eq!(editor.cursor(), 1);
    }

    #[test]
    fn test_edit_ascii_with_mode_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.set_write_mode(WriteMode::Insert);

        assert!(editor.edit_ascii_with_mode('H'));
        assert_eq!(editor.len(), 5);
        assert_eq!(editor.working()[0], b'H');
        assert_eq!(editor.cursor(), 1);
        // Original bytes shifted right
        assert_eq!(editor.working(), &[b'H', 0x00, 0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_handle_backspace_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.set_cursor(2);
        editor.handle_backspace();
        // Overwrite mode: just moves cursor left
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.working(), &data); // No modification
    }

    #[test]
    fn test_handle_backspace_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.set_write_mode(WriteMode::Insert);

        editor.set_cursor(2);
        editor.handle_backspace();
        // Insert mode: moves cursor left and deletes byte
        assert_eq!(editor.cursor(), 1);
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
    }

    #[test]
    fn test_handle_backspace_insert_at_start() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data.clone());
        editor.set_write_mode(WriteMode::Insert);

        editor.set_cursor(0);
        editor.handle_backspace();
        // At start, nothing should happen
        assert_eq!(editor.cursor(), 0);
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_handle_delete_overwrite() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data.clone());

        editor.set_cursor(1);
        editor.handle_delete();
        // Overwrite mode: no action
        assert_eq!(editor.working(), &data);
    }

    #[test]
    fn test_handle_delete_insert() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);
        editor.set_write_mode(WriteMode::Insert);

        editor.set_cursor(1);
        editor.handle_delete();
        // Insert mode: deletes byte at cursor
        assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);
        assert_eq!(editor.cursor(), 1);
    }

    // ========== Bookmark Offset Adjustment Tests ==========

    #[test]
    fn test_insert_adjusts_bookmarks() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04];
        let mut editor = EditorState::new(data);

        // Add bookmarks at various positions
        editor.add_bookmark(1, "Before".to_string());
        editor.add_bookmark(3, "After".to_string());

        // Insert byte at position 2 — bookmark at 3 should shift to 4
        editor.insert_byte(2, 0xFF);

        let bookmarks = editor.bookmarks().all();
        assert_eq!(bookmarks[0].offset, 1); // Before insert point: unchanged
        assert_eq!(bookmarks[1].offset, 4); // After insert point: shifted +1
    }

    #[test]
    fn test_delete_adjusts_bookmarks() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04];
        let mut editor = EditorState::new(data);

        editor.add_bookmark(1, "Before".to_string());
        editor.add_bookmark(2, "Deleted".to_string());
        editor.add_bookmark(4, "After".to_string());

        // Delete byte at position 2 — bookmark at 2 removed, bookmark at 4 shifts to 3
        editor.delete_byte(2);

        let bookmarks = editor.bookmarks().all();
        assert_eq!(bookmarks.len(), 2); // One removed
        assert_eq!(bookmarks[0].offset, 1); // Before: unchanged
        assert_eq!(bookmarks[1].offset, 3); // After: shifted -1
    }

    #[test]
    fn test_insert_byte_on_empty_buffer() {
        let mut editor = EditorState::new(vec![]);

        editor.insert_byte(0, 0xAA);
        assert_eq!(editor.working(), &[0xAA]);
        assert_eq!(editor.len(), 1);

        // Undo should return to empty
        assert!(editor.undo());
        assert_eq!(editor.working(), &[] as &[u8]);
        assert_eq!(editor.len(), 0);
    }

    #[test]
    fn test_delete_byte_on_empty_buffer() {
        let mut editor = EditorState::new(vec![]);

        // Should be a no-op
        let deleted = editor.delete_byte(0);
        assert_eq!(deleted, None);
        assert_eq!(editor.len(), 0);
    }

    #[test]
    fn test_insert_clears_save_points() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Create a save point
        editor.edit_byte(0, 0xFF);
        editor.create_save_point("SP1".to_string());
        assert_eq!(editor.save_point_count(), 1);

        // Insert should clear save points
        editor.insert_byte(0, 0xAA);
        assert_eq!(editor.save_point_count(), 0);
    }
}
