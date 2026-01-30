//! Undo/redo history management

/// Maximum number of operations to keep in history
const MAX_HISTORY_SIZE: usize = 1000;

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
}

/// Linear undo/redo history
pub struct History {
    /// Stack of operations that can be undone
    undo_stack: Vec<EditOperation>,

    /// Stack of operations that can be redone
    redo_stack: Vec<EditOperation>,
}

impl History {
    /// Create a new empty history
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Push a new operation onto the history
    ///
    /// This clears the redo stack (branching history not supported)
    /// and enforces the maximum history size.
    pub fn push(&mut self, op: EditOperation) {
        // Clear redo stack - we're creating new history
        self.redo_stack.clear();

        // Add to undo stack
        self.undo_stack.push(op);

        // Enforce maximum size - drop oldest operations
        while self.undo_stack.len() > MAX_HISTORY_SIZE {
            self.undo_stack.remove(0);
        }
    }

    /// Undo the last operation, returning it for application
    pub fn undo(&mut self) -> Option<EditOperation> {
        if let Some(op) = self.undo_stack.pop() {
            self.redo_stack.push(op.clone());
            Some(op)
        } else {
            None
        }
    }

    /// Redo the last undone operation, returning it for application
    pub fn redo(&mut self) -> Option<EditOperation> {
        if let Some(op) = self.redo_stack.pop() {
            self.undo_stack.push(op.clone());
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

        // Push more than MAX_HISTORY_SIZE operations
        for i in 0..MAX_HISTORY_SIZE + 100 {
            history.push(EditOperation::Single {
                offset: i,
                old_value: 0x00,
                new_value: 0xFF,
            });
        }

        assert_eq!(history.undo_count(), MAX_HISTORY_SIZE);
    }
}
