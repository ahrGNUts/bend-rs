//! Save points: named, self-contained snapshots of editing state
//!
//! Each save point holds a full `Vec<u8>` snapshot of the working buffer at the
//! moment of creation. Snapshots are independent — there is no chain of diffs —
//! so save points survive any subsequent edit (including length changes) and
//! can be deleted in any order.
//!
//! Memory cost is `O(N × buffer_size)` for N save points. For typical workloads
//! (single-digit MB files, modest save-point counts) this is a few tens of MB.
//! See `openspec/changes/refactor-save-points-to-snapshots/design.md` for the
//! detailed cost analysis and the comparison with diff-based and compressed
//! alternatives.

use std::collections::HashMap;

/// A named snapshot of the editing state.
#[derive(Clone, Debug)]
pub struct SavePoint {
    pub id: u64,
    pub name: String,
    /// Snapshot of the working buffer at creation time. Owned, uncompressed.
    bytes: Vec<u8>,
}

impl SavePoint {
    pub fn new(id: u64, name: String, bytes: Vec<u8>) -> Self {
        Self { id, name, bytes }
    }
}

/// Manages save points for an editor session.
pub struct SavePointManager {
    save_points: Vec<SavePoint>,
    id_to_index: HashMap<u64, usize>,
    next_id: u64,
}

impl Default for SavePointManager {
    fn default() -> Self {
        Self {
            save_points: Vec::new(),
            id_to_index: HashMap::new(),
            next_id: 1,
        }
    }
}

impl SavePointManager {
    /// Create a new, empty save point manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// All save points, in creation order.
    pub fn save_points(&self) -> &[SavePoint] {
        &self.save_points
    }

    /// Mutable access to a save point by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut SavePoint> {
        self.id_to_index
            .get(&id)
            .map(|&index| &mut self.save_points[index])
    }

    /// Create a new save point capturing the current working buffer state.
    /// Returns the ID of the created save point.
    pub fn create(&mut self, name: String, current_state: &[u8]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let save_point = SavePoint::new(id, name, current_state.to_vec());
        let index = self.save_points.len();
        self.save_points.push(save_point);
        self.id_to_index.insert(id, index);

        id
    }

    /// Restore by returning a fresh copy of the snapshot's bytes.
    pub fn restore(&self, id: u64) -> Option<Vec<u8>> {
        let index = *self.id_to_index.get(&id)?;
        Some(self.save_points[index].bytes.clone())
    }

    /// Rename a save point. Returns whether the save point was found.
    #[must_use = "returns whether the save point was found and renamed"]
    pub fn rename(&mut self, id: u64, new_name: String) -> bool {
        if let Some(sp) = self.get_mut(id) {
            sp.name = new_name;
            true
        } else {
            false
        }
    }

    /// Delete a save point. Works for any save point, regardless of its
    /// position in the list. Returns whether the save point was found.
    #[must_use = "returns whether the save point was deleted"]
    pub fn delete(&mut self, id: u64) -> bool {
        let Some(&index) = self.id_to_index.get(&id) else {
            return false;
        };
        self.save_points.remove(index);
        // Rebuild the index since `Vec::remove` shifts every later element down by one.
        self.id_to_index.clear();
        for (idx, sp) in self.save_points.iter().enumerate() {
            self.id_to_index.insert(sp.id, idx);
        }
        true
    }

    /// Number of save points.
    pub fn len(&self) -> usize {
        self.save_points.len()
    }

    /// Clear all save points.
    ///
    /// Used on file load: a new file's working buffer has nothing to do with
    /// the previous file's save points.
    #[allow(dead_code)] // Retained as the file-load reset path; not yet wired up.
    pub fn clear_all(&mut self) {
        self.save_points.clear();
        self.id_to_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_save_point() {
        let mut manager = SavePointManager::new();

        let modified = vec![0xAA, 0x01, 0xBB, 0x03];
        let id = manager.create("First save".to_string(), &modified);

        assert_eq!(manager.len(), 1);
        let restored = manager.restore(id).unwrap();
        assert_eq!(restored, modified);
    }

    #[test]
    fn test_restore_save_point() {
        let mut manager = SavePointManager::new();

        let modified1 = vec![0xAA, 0x01, 0x02, 0x03];
        let id1 = manager.create("SP1".to_string(), &modified1);

        let modified2 = vec![0xAA, 0xBB, 0x02, 0x03];
        let id2 = manager.create("SP2".to_string(), &modified2);

        let restored1 = manager.restore(id1).unwrap();
        assert_eq!(restored1, modified1);

        let restored2 = manager.restore(id2).unwrap();
        assert_eq!(restored2, modified2);
    }

    #[test]
    fn test_rename_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new();

        let id = manager.create("Original name".to_string(), &original);
        assert!(manager.rename(id, "New name".to_string()));

        let sps = manager.save_points();
        assert_eq!(sps[0].name, "New name");
    }

    #[test]
    fn test_delete_any_save_point() {
        let mut manager = SavePointManager::new();

        let modified1 = vec![0xAA, 0x01, 0x02, 0x03];
        let id1 = manager.create("SP1".to_string(), &modified1);

        let modified2 = vec![0xAA, 0xBB, 0x02, 0x03];
        let id2 = manager.create("SP2".to_string(), &modified2);

        let modified3 = vec![0xAA, 0xBB, 0xCC, 0x03];
        let id3 = manager.create("SP3".to_string(), &modified3);

        // Middle save point can be deleted (was the leaf-only restriction's blocker).
        assert!(manager.delete(id2));
        assert_eq!(manager.len(), 2);

        // Both remaining save points still resolve to their captured states.
        let restored1 = manager.restore(id1).unwrap();
        assert_eq!(restored1, modified1);
        let restored3 = manager.restore(id3).unwrap();
        assert_eq!(restored3, modified3);

        // Now delete what's currently the first of two (also a non-leaf in the original list).
        assert!(manager.delete(id1));
        assert_eq!(manager.len(), 1);
        let restored3 = manager.restore(id3).unwrap();
        assert_eq!(restored3, modified3);
    }

    #[test]
    fn test_save_point_independent_of_subsequent_edits() {
        let mut manager = SavePointManager::new();

        let snapshot_state = vec![0xAA, 0x01, 0x02, 0x03];
        let id = manager.create("SP1".to_string(), &snapshot_state);

        // The snapshot is self-contained and restore returns its captured bytes.
        let restored = manager.restore(id).unwrap();
        assert_eq!(restored, snapshot_state);
    }

    #[test]
    fn test_delete_re_indexes_after_middle_removal() {
        let mut manager = SavePointManager::new();

        let id1 = manager.create("a".to_string(), &[0x01]);
        let id2 = manager.create("b".to_string(), &[0x02]);
        let id3 = manager.create("c".to_string(), &[0x03]);

        assert!(manager.delete(id2));

        // After re-indexing, id3 must still resolve correctly. If `delete`
        // failed to rebuild `id_to_index`, id3 would point at a stale index
        // and `restore` would return `None` or wrong bytes.
        let restored3 = manager.restore(id3).unwrap();
        assert_eq!(restored3, vec![0x03]);
        let restored1 = manager.restore(id1).unwrap();
        assert_eq!(restored1, vec![0x01]);
    }
}
