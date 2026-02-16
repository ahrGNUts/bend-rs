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

// Re-export types that were originally defined here for API stability
pub use super::cursor::NibblePosition;
pub use super::modes::{EditMode, WriteMode};

/// Editor state containing buffers and edit history
pub struct EditorState {
    /// Original bytes loaded from file (immutable after load)
    pub(super) original: Vec<u8>,

    /// Working buffer where all edits are applied
    pub(super) working: Vec<u8>,

    /// Edit history for undo/redo
    pub(super) history: History,

    /// Save point manager for named snapshots
    pub(super) save_points: SavePointManager,

    /// Bookmark manager for annotated locations
    pub(super) bookmarks: BookmarkManager,

    /// Current cursor position in the buffer
    pub(super) cursor: usize,

    /// Which nibble is being edited at the cursor
    pub(super) nibble: NibblePosition,

    /// Selection range (start, end) - None if no selection
    pub(super) selection: Option<(usize, usize)>,

    /// Selection anchor point - where selection started (for Shift+click/arrow)
    pub(super) selection_anchor: Option<usize>,

    /// Whether the working buffer has unsaved changes
    pub(super) modified: bool,

    /// Current editing mode (hex nibble vs ASCII character)
    pub(super) edit_mode: EditMode,

    /// Current write mode (insert vs overwrite)
    pub(super) write_mode: WriteMode,

    /// Whether buffer length changed since last check (for UI cache invalidation)
    pub(super) length_changed: bool,

    /// Monotonically increasing counter, bumped on every edit/undo/redo
    edit_generation: u64,
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
            edit_generation: 0,
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

    /// Get the current edit generation counter (incremented on every edit/undo/redo)
    pub fn edit_generation(&self) -> u64 {
        self.edit_generation
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
        if !(0x20..=0x7E).contains(&byte_value) {
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
            self.edit_generation += 1;
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
            self.edit_generation += 1;
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

    /// Check if buffer has been modified
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Replace a range of bytes as a single undoable operation
    pub fn replace_bytes(&mut self, offset: usize, new_values: &[u8]) {
        let end = (offset + new_values.len()).min(self.working.len());
        let actual_len = end - offset;
        if actual_len == 0 {
            return;
        }

        let old_values = self.working[offset..end].to_vec();
        let new_values = new_values[..actual_len].to_vec();

        if old_values == new_values {
            return; // No change
        }

        self.working[offset..end].copy_from_slice(&new_values);
        self.history.push(EditOperation::Range {
            offset,
            old_values,
            new_values,
        });
        self.modified = true;
        self.edit_generation += 1;
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
        self.edit_generation += 1;
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
        self.edit_generation += 1;
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
        self.edit_generation += 1;
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
        self.edit_generation += 1;
        // Clamp cursor if it now points past the end
        if !self.working.is_empty() {
            self.cursor = self.cursor.min(self.working.len() - 1);
        }
        self.on_length_changed(offset, 1, false);
        Some(value)
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
            self.edit_generation += 1;
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
            self.edit_generation += 1;
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

        // Swap working with restored, getting old values without cloning
        let old_values = std::mem::replace(&mut self.working, restored);

        // Only record if there's actually a change
        if old_values != self.working {
            self.history.push(super::history::EditOperation::Range {
                offset: 0,
                old_values,
                new_values: self.working.clone(),
            });
        }

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

    // ========== Replace Bytes Tests ==========

    #[test]
    fn test_replace_bytes() {
        let data = vec![0x00, 0x01, 0x02, 0x03, 0x04];
        let mut editor = EditorState::new(data);

        editor.replace_bytes(1, &[0xAA, 0xBB]);
        assert_eq!(editor.working(), &[0x00, 0xAA, 0xBB, 0x03, 0x04]);
        assert!(editor.is_modified());

        // Single undo should revert the entire replacement
        assert!(editor.undo());
        assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03, 0x04]);
        assert!(!editor.is_modified());

        // Redo should re-apply the entire replacement
        assert!(editor.redo());
        assert_eq!(editor.working(), &[0x00, 0xAA, 0xBB, 0x03, 0x04]);
    }

    #[test]
    fn test_replace_bytes_no_change() {
        let data = vec![0x00, 0x01, 0x02, 0x03];
        let mut editor = EditorState::new(data);

        // Replacing with same values should be a no-op
        editor.replace_bytes(1, &[0x01, 0x02]);
        assert!(!editor.is_modified());
        assert!(!editor.can_undo());
    }

    #[test]
    fn test_replace_bytes_at_end() {
        let data = vec![0x00, 0x01, 0x02];
        let mut editor = EditorState::new(data);

        // Replace that would extend past buffer should be clamped
        editor.replace_bytes(2, &[0xAA, 0xBB, 0xCC]);
        assert_eq!(editor.working(), &[0x00, 0x01, 0xAA]);
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
