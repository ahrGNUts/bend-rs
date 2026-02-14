//! Undo/redo history management

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of operations to keep in history
const MAX_HISTORY_SIZE: usize = 1000;

/// Time window for coalescing adjacent single-byte edits (500ms)
const COALESCE_WINDOW: Duration = Duration::from_millis(500);

/// Represents a single edit operation that can be undone/redone
#[derive(Clone, Debug)]
pub enum EditOperation {
    /// Single byte edit
    Single {
        offset: usize,
        old_value: u8,
        new_value: u8,
    },
    /// Range of bytes edit
    Range {
        offset: usize,
        old_values: Vec<u8>,
        new_values: Vec<u8>,
    },
    /// Insert bytes at an offset (buffer grows)
    InsertBytes {
        offset: usize,
        values: Vec<u8>,
    },
    /// Delete bytes at an offset (buffer shrinks)
    DeleteBytes {
        offset: usize,
        values: Vec<u8>,
    },
}

/// Try to coalesce a new operation with an existing one
///
/// Returns true if the operations were coalesced, false otherwise.
/// Coalescing happens when:
/// - Both operations are adjacent single-byte edits (consecutive offsets)
/// - Or one is a Range and the other is an adjacent Single
fn try_coalesce(existing: &mut EditOperation, new: &EditOperation) -> bool {
    // Extract info from new operation first
    let EditOperation::Single {
        offset: new_offset,
        old_value: new_old,
        new_value: new_new,
    } = new
    else {
        return false; // Only coalesce with Single operations
    };

    match existing {
        // Coalesce two adjacent single-byte edits into a range
        EditOperation::Single {
            offset: existing_offset,
            old_value: existing_old,
            new_value: existing_new,
        } => {
            // Check if adjacent (new edit is right after existing)
            if *new_offset == *existing_offset + 1 {
                *existing = EditOperation::Range {
                    offset: *existing_offset,
                    old_values: vec![*existing_old, *new_old],
                    new_values: vec![*existing_new, *new_new],
                };
                return true;
            }
            // Check if adjacent (new edit is right before existing)
            if *existing_offset == *new_offset + 1 {
                *existing = EditOperation::Range {
                    offset: *new_offset,
                    old_values: vec![*new_old, *existing_old],
                    new_values: vec![*new_new, *existing_new],
                };
                return true;
            }
            // Check if same offset (re-editing same byte)
            if *existing_offset == *new_offset {
                // Keep the original old_value, use the new new_value
                *existing_new = *new_new;
                return true;
            }
            false
        }

        // InsertBytes and DeleteBytes never coalesce
        EditOperation::InsertBytes { .. } | EditOperation::DeleteBytes { .. } => false,

        // Extend a range with an adjacent single-byte edit
        EditOperation::Range {
            offset: range_offset,
            old_values,
            new_values,
        } => {
            let range_end = *range_offset + old_values.len();
            // Check if new edit is right after the range
            if *new_offset == range_end {
                old_values.push(*new_old);
                new_values.push(*new_new);
                return true;
            }
            // Check if new edit is right before the range
            if *new_offset + 1 == *range_offset {
                old_values.insert(0, *new_old);
                new_values.insert(0, *new_new);
                *range_offset = *new_offset;
                return true;
            }
            false
        }
    }
}

/// Linear undo/redo history
pub struct History {
    /// Stack of operations that can be undone (VecDeque for O(1) front removal)
    undo_stack: VecDeque<EditOperation>,

    /// Stack of operations that can be redone
    redo_stack: Vec<EditOperation>,

    /// Timestamp of the last pushed operation (for coalescing)
    last_push_time: Option<Instant>,
}

impl History {
    /// Create a new empty history
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            last_push_time: None,
        }
    }

    /// Push a new operation onto the history
    ///
    /// This clears the redo stack (branching history not supported)
    /// and enforces the maximum history size.
    ///
    /// Adjacent single-byte edits within 500ms are coalesced into Range operations.
    pub fn push(&mut self, op: EditOperation) {
        // Clear redo stack - we're creating new history
        self.redo_stack.clear();

        let now = Instant::now();

        // Try to coalesce with the previous operation if within time window
        if let Some(last_time) = self.last_push_time {
            if now.duration_since(last_time) <= COALESCE_WINDOW {
                if let Some(last_op) = self.undo_stack.back_mut() {
                    if try_coalesce(last_op, &op) {
                        self.last_push_time = Some(now);
                        return;
                    }
                }
            }
        }

        // No coalescing - add as new operation
        self.undo_stack.push_back(op);
        self.last_push_time = Some(now);

        // Enforce maximum size - drop oldest operations (O(1) with VecDeque)
        while self.undo_stack.len() > MAX_HISTORY_SIZE {
            self.undo_stack.pop_front();
        }
    }

    /// Undo the last operation, returning it for application
    pub fn undo(&mut self) -> Option<EditOperation> {
        if let Some(op) = self.undo_stack.pop_back() {
            self.redo_stack.push(op.clone());
            Some(op)
        } else {
            None
        }
    }

    /// Redo the last undone operation, returning it for application
    pub fn redo(&mut self) -> Option<EditOperation> {
        if let Some(op) = self.redo_stack.pop() {
            self.undo_stack.push_back(op.clone());
            Some(op)
        } else {
            None
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the number of operations in the undo stack
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of operations in the redo stack
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_undo() {
        let mut history = History::new();

        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xFF,
        });

        assert!(history.can_undo());
        assert!(!history.can_redo());

        let op = history.undo();
        assert!(op.is_some());
        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn test_redo() {
        let mut history = History::new();

        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xFF,
        });

        history.undo();
        assert!(history.can_redo());

        let op = history.redo();
        assert!(op.is_some());
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_new_operation_clears_redo() {
        let mut history = History::new();

        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xFF,
        });

        history.undo();
        assert!(history.can_redo());

        // Push a new operation
        history.push(EditOperation::Single {
            offset: 1,
            old_value: 0x01,
            new_value: 0xFE,
        });

        // Redo should now be empty
        assert!(!history.can_redo());
    }

    #[test]
    fn test_max_history_size() {
        let mut history = History::new();

        // Push more than MAX_HISTORY_SIZE operations with delays to prevent coalescing
        for i in 0..MAX_HISTORY_SIZE + 100 {
            // Use non-adjacent offsets to prevent coalescing
            history.push(EditOperation::Single {
                offset: i * 100, // Non-adjacent offsets
                old_value: 0x00,
                new_value: 0xFF,
            });
            // Reset the timestamp to prevent time-based coalescing
            history.last_push_time = None;
        }

        assert_eq!(history.undo_count(), MAX_HISTORY_SIZE);
    }

    #[test]
    fn test_coalesce_adjacent_singles() {
        let mut history = History::new();

        // Push first edit
        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xAA,
        });

        // Push adjacent edit (should coalesce)
        history.push(EditOperation::Single {
            offset: 1,
            old_value: 0x01,
            new_value: 0xBB,
        });

        // Should be coalesced into one Range operation
        assert_eq!(history.undo_count(), 1);

        // Verify it's a Range with both bytes
        if let Some(EditOperation::Range {
            offset,
            old_values,
            new_values,
        }) = history.undo()
        {
            assert_eq!(offset, 0);
            assert_eq!(old_values, vec![0x00, 0x01]);
            assert_eq!(new_values, vec![0xAA, 0xBB]);
        } else {
            panic!("Expected Range operation");
        }
    }

    #[test]
    fn test_coalesce_same_offset() {
        let mut history = History::new();

        // Push first edit
        history.push(EditOperation::Single {
            offset: 5,
            old_value: 0x00,
            new_value: 0xAA,
        });

        // Push edit at same offset (should coalesce, keeping original old_value)
        history.push(EditOperation::Single {
            offset: 5,
            old_value: 0xAA, // This is the "current" state
            new_value: 0xBB,
        });

        // Should be coalesced into one operation
        assert_eq!(history.undo_count(), 1);

        // Verify it keeps original old_value and final new_value
        if let Some(EditOperation::Single {
            offset,
            old_value,
            new_value,
        }) = history.undo()
        {
            assert_eq!(offset, 5);
            assert_eq!(old_value, 0x00); // Original value before any edits
            assert_eq!(new_value, 0xBB); // Final value after all edits
        } else {
            panic!("Expected Single operation");
        }
    }

    #[test]
    fn test_coalesce_extends_range() {
        let mut history = History::new();

        // Push first two edits (will coalesce into Range)
        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xAA,
        });
        history.push(EditOperation::Single {
            offset: 1,
            old_value: 0x01,
            new_value: 0xBB,
        });

        // Push third adjacent edit (should extend the Range)
        history.push(EditOperation::Single {
            offset: 2,
            old_value: 0x02,
            new_value: 0xCC,
        });

        // Should still be one operation
        assert_eq!(history.undo_count(), 1);

        if let Some(EditOperation::Range {
            offset,
            old_values,
            new_values,
        }) = history.undo()
        {
            assert_eq!(offset, 0);
            assert_eq!(old_values, vec![0x00, 0x01, 0x02]);
            assert_eq!(new_values, vec![0xAA, 0xBB, 0xCC]);
        } else {
            panic!("Expected Range operation");
        }
    }

    #[test]
    fn test_no_coalesce_non_adjacent() {
        let mut history = History::new();

        // Push first edit
        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xAA,
        });

        // Push non-adjacent edit (should NOT coalesce)
        history.push(EditOperation::Single {
            offset: 5, // Not adjacent to offset 0
            old_value: 0x05,
            new_value: 0xBB,
        });

        // Should be two separate operations
        assert_eq!(history.undo_count(), 2);
    }

    #[test]
    fn test_no_coalesce_after_timeout() {
        let mut history = History::new();

        // Push first edit
        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xAA,
        });

        // Simulate timeout by clearing last_push_time
        history.last_push_time = None;

        // Push adjacent edit (should NOT coalesce due to timeout)
        history.push(EditOperation::Single {
            offset: 1,
            old_value: 0x01,
            new_value: 0xBB,
        });

        // Should be two separate operations
        assert_eq!(history.undo_count(), 2);
    }

    #[test]
    fn test_insert_bytes_no_coalesce() {
        let mut history = History::new();

        // Push an InsertBytes operation
        history.push(EditOperation::InsertBytes {
            offset: 0,
            values: vec![0xAA],
        });

        // Push an adjacent Single edit — should NOT coalesce with InsertBytes
        history.push(EditOperation::Single {
            offset: 1,
            old_value: 0x00,
            new_value: 0xBB,
        });

        assert_eq!(history.undo_count(), 2);
    }

    #[test]
    fn test_delete_bytes_no_coalesce() {
        let mut history = History::new();

        // Push a DeleteBytes operation
        history.push(EditOperation::DeleteBytes {
            offset: 0,
            values: vec![0xAA],
        });

        // Push an adjacent Single edit — should NOT coalesce with DeleteBytes
        history.push(EditOperation::Single {
            offset: 0,
            old_value: 0x00,
            new_value: 0xBB,
        });

        assert_eq!(history.undo_count(), 2);
    }
}
