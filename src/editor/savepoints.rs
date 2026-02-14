//! Save points for creating named snapshots of editing state
//!
//! Save points store incremental diffs between states, allowing efficient
//! memory usage while supporting arbitrary state restoration.
//!
//! ## Architecture
//!
//! Save points form a chain where each stores only the diff from the previous:
//!
//! ```text
//! [Original File] -> [SavePoint 1] -> [SavePoint 2] -> [SavePoint 3]
//!                    (diff vs orig)   (diff vs SP1)   (diff vs SP2)
//! ```
//!
//! To restore to a save point, we:
//! 1. Reset to the original file bytes
//! 2. Apply diffs in order up to the target save point
//!
//! This approach:
//! - Uses minimal memory (only stores changes, not full copies)
//! - Allows restoring to any point in the chain
//! - Requires careful handling when deleting non-leaf save points

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single byte change in a diff
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteChange {
    pub offset: usize,
    pub old_value: u8,
    pub new_value: u8,
}

/// A named snapshot of the editing state
#[derive(Clone, Debug)]
pub struct SavePoint {
    /// Unique identifier for this save point
    pub id: u64,

    /// User-provided name for this save point
    pub name: String,

    /// Unix timestamp when this save point was created
    pub timestamp: u64,

    /// Incremental diff from the previous save point (or original file if first)
    /// This represents changes that were made AFTER the previous save point
    pub diff: Vec<ByteChange>,
}

impl SavePoint {
    /// Create a new save point with the given name and diff
    pub fn new(id: u64, name: String, diff: Vec<ByteChange>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id,
            name,
            timestamp,
            diff,
        }
    }
}

/// Manages save points for an editor session
pub struct SavePointManager {
    /// List of save points in chronological order
    /// Index 0 is the first save point (diff from original)
    save_points: Vec<SavePoint>,

    /// Index lookup by ID for O(1) access
    id_to_index: HashMap<u64, usize>,

    /// Counter for generating unique save point IDs
    next_id: u64,

    /// The state of the working buffer at the last save point
    /// Used to compute diffs for new save points
    last_save_point_state: Vec<u8>,
}

impl SavePointManager {
    /// Create a new save point manager
    pub fn new(original_bytes: &[u8]) -> Self {
        Self {
            save_points: Vec::new(),
            id_to_index: HashMap::new(),
            next_id: 1,
            last_save_point_state: original_bytes.to_vec(),
        }
    }

    /// Get all save points
    pub fn save_points(&self) -> &[SavePoint] {
        &self.save_points
    }

    /// Get a save point by ID
    pub fn get(&self, id: u64) -> Option<&SavePoint> {
        self.id_to_index.get(&id).map(|&i| &self.save_points[i])
    }

    /// Get a mutable reference to a save point by ID
    pub fn get_mut(&mut self, id: u64) -> Option<&mut SavePoint> {
        if let Some(&index) = self.id_to_index.get(&id) {
            Some(&mut self.save_points[index])
        } else {
            None
        }
    }

    /// Create a new save point from the current working buffer state
    ///
    /// Returns the ID of the created save point
    pub fn create(&mut self, name: String, current_state: &[u8]) -> u64 {
        // Compute diff from last save point state to current state
        let diff = compute_diff(&self.last_save_point_state, current_state);

        let id = self.next_id;
        self.next_id += 1;

        let save_point = SavePoint::new(id, name, diff);
        let index = self.save_points.len();
        self.save_points.push(save_point);
        self.id_to_index.insert(id, index);

        // Update last save point state to current
        self.last_save_point_state = current_state.to_vec();

        id
    }

    /// Restore the working buffer to a specific save point
    ///
    /// Returns the bytes that should be in the working buffer after restoration
    pub fn restore(&self, id: u64, original: &[u8]) -> Option<Vec<u8>> {
        // Find the index of the target save point
        let target_idx = self.save_points.iter().position(|sp| sp.id == id)?;

        // Start with original bytes
        let mut result = original.to_vec();

        // Apply diffs up to and including the target save point
        for save_point in &self.save_points[..=target_idx] {
            for change in &save_point.diff {
                if change.offset < result.len() {
                    result[change.offset] = change.new_value;
                }
            }
        }

        Some(result)
    }

    /// Rename a save point
    #[must_use = "returns whether the save point was found and renamed"]
    pub fn rename(&mut self, id: u64, new_name: String) -> bool {
        if let Some(sp) = self.get_mut(id) {
            sp.name = new_name;
            true
        } else {
            false
        }
    }

    /// Check if a save point can be deleted
    ///
    /// Currently, only the last (leaf) save point can be deleted to avoid
    /// having to recompute diffs for successor save points.
    pub fn can_delete(&self, id: u64) -> bool {
        // Only allow deleting the last save point (leaf)
        self.save_points.last().map(|sp| sp.id == id).unwrap_or(false)
    }

    /// Delete a save point (only leaf save points can be deleted)
    ///
    /// Returns true if the save point was deleted, false otherwise
    #[must_use = "returns whether the save point was deleted"]
    pub fn delete(&mut self, id: u64) -> bool {
        if !self.can_delete(id) {
            return false;
        }

        if let Some(deleted) = self.save_points.pop() {
            // Remove from index
            self.id_to_index.remove(&deleted.id);

            // Revert last_save_point_state by undoing the deleted save point's diff
            for change in deleted.diff.iter().rev() {
                if change.offset < self.last_save_point_state.len() {
                    self.last_save_point_state[change.offset] = change.old_value;
                }
            }
            true
        } else {
            false
        }
    }

    /// Get the number of save points
    pub fn len(&self) -> usize {
        self.save_points.len()
    }

    /// Check if there are no save points
    pub fn is_empty(&self) -> bool {
        self.save_points.is_empty()
    }

    /// Clear all save points and reset diff base state
    ///
    /// Used when buffer length changes (insert/delete) since absolute-offset
    /// diffs become invalid.
    pub fn clear_all(&mut self, base_state: &[u8]) {
        self.save_points.clear();
        self.id_to_index.clear();
        self.last_save_point_state = base_state.to_vec();
    }
}

/// Compute the diff between two byte slices
fn compute_diff(old: &[u8], new: &[u8]) -> Vec<ByteChange> {
    let mut changes = Vec::new();
    let max_len = old.len().max(new.len());

    for offset in 0..max_len {
        let old_value = old.get(offset).copied().unwrap_or(0);
        let new_value = new.get(offset).copied().unwrap_or(0);

        if old_value != new_value {
            changes.push(ByteChange {
                offset,
                old_value,
                new_value,
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        // Make some changes
        let modified = vec![0xAA, 0x01, 0xBB, 0x03];
        let id = manager.create("First save".to_string(), &modified);

        assert_eq!(manager.len(), 1);
        let sp = manager.get(id).unwrap();
        assert_eq!(sp.name, "First save");
        assert_eq!(sp.diff.len(), 2); // Two bytes changed
    }

    #[test]
    fn test_restore_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        // Create first save point
        let modified1 = vec![0xAA, 0x01, 0x02, 0x03];
        let id1 = manager.create("SP1".to_string(), &modified1);

        // Create second save point
        let modified2 = vec![0xAA, 0xBB, 0x02, 0x03];
        let id2 = manager.create("SP2".to_string(), &modified2);

        // Restore to first save point
        let restored1 = manager.restore(id1, &original).unwrap();
        assert_eq!(restored1, modified1);

        // Restore to second save point
        let restored2 = manager.restore(id2, &original).unwrap();
        assert_eq!(restored2, modified2);
    }

    #[test]
    fn test_rename_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let id = manager.create("Original name".to_string(), &original);
        assert!(manager.rename(id, "New name".to_string()));

        let sp = manager.get(id).unwrap();
        assert_eq!(sp.name, "New name");
    }

    #[test]
    fn test_delete_leaf_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let id1 = manager.create("SP1".to_string(), &[0xAA, 0x01, 0x02, 0x03]);
        let id2 = manager.create("SP2".to_string(), &[0xAA, 0xBB, 0x02, 0x03]);

        // Can only delete the last one
        assert!(!manager.can_delete(id1));
        assert!(manager.can_delete(id2));

        // Delete the leaf
        assert!(manager.delete(id2));
        assert_eq!(manager.len(), 1);

        // Now id1 is the leaf
        assert!(manager.can_delete(id1));
        assert!(manager.delete(id1));
        assert!(manager.is_empty());
    }

    #[test]
    fn test_compute_diff() {
        let old = vec![0x00, 0x01, 0x02, 0x03];
        let new = vec![0xAA, 0x01, 0xBB, 0x03];

        let diff = compute_diff(&old, &new);

        assert_eq!(diff.len(), 2);
        assert_eq!(
            diff[0],
            ByteChange {
                offset: 0,
                old_value: 0x00,
                new_value: 0xAA
            }
        );
        assert_eq!(
            diff[1],
            ByteChange {
                offset: 2,
                old_value: 0x02,
                new_value: 0xBB
            }
        );
    }
}
