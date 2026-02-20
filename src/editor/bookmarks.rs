//! Bookmarks and annotations for the hex editor

use std::collections::HashMap;

/// A bookmark marking a specific offset in the file with an optional annotation
#[derive(Debug, Clone)]
pub struct Bookmark {
    /// Unique identifier for this bookmark
    pub id: u64,
    /// Byte offset in the file
    pub offset: usize,
    /// User-defined name for the bookmark
    pub name: String,
    /// Optional annotation/note about this location
    pub annotation: String,
}

impl Bookmark {
    /// Create a new bookmark at the given offset
    pub fn new(id: u64, offset: usize, name: String) -> Self {
        Self {
            id,
            offset,
            name,
            annotation: String::new(),
        }
    }
}

/// Manager for bookmarks in the current file
#[derive(Default)]
pub struct BookmarkManager {
    /// All bookmarks, sorted by offset
    bookmarks: Vec<Bookmark>,
    /// Index lookup by ID for O(1) access
    id_to_index: HashMap<u64, usize>,
    /// Next ID to assign
    next_id: u64,
}

impl BookmarkManager {
    /// Create a new bookmark manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a bookmark at the given offset
    pub fn add(&mut self, offset: usize, name: String) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let bookmark = Bookmark::new(id, offset, name);

        // Binary search for sorted insertion position — O(log n) instead of full sort
        let pos = self.bookmarks.partition_point(|b| b.offset < offset);
        self.bookmarks.insert(pos, bookmark);

        // Rebuild indices for items at and after the insertion point
        for (index, bm) in self.bookmarks.iter().enumerate().skip(pos) {
            self.id_to_index.insert(bm.id, index);
        }

        id
    }

    /// Rebuild the ID -> index map after structural changes
    fn rebuild_index(&mut self) {
        self.id_to_index.clear();
        for (index, bookmark) in self.bookmarks.iter().enumerate() {
            self.id_to_index.insert(bookmark.id, index);
        }
    }

    /// Remove a bookmark by ID
    #[must_use = "returns whether the bookmark was found and removed"]
    pub fn remove(&mut self, id: u64) -> bool {
        if let Some(&index) = self.id_to_index.get(&id) {
            self.bookmarks.remove(index);
            // Rebuild index after removal (indices shift)
            self.rebuild_index();
            true
        } else {
            false
        }
    }

    /// Get a mutable reference to a bookmark by ID
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Bookmark> {
        if let Some(&index) = self.id_to_index.get(&id) {
            Some(&mut self.bookmarks[index])
        } else {
            None
        }
    }

    /// Get all bookmarks
    pub fn all(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Rename a bookmark
    #[must_use = "returns whether the bookmark was found and renamed"]
    pub fn rename(&mut self, id: u64, new_name: String) -> bool {
        if let Some(bookmark) = self.get_mut(id) {
            bookmark.name = new_name;
            true
        } else {
            false
        }
    }

    /// Set the annotation for a bookmark
    #[must_use = "returns whether the bookmark was found and annotation was set"]
    pub fn set_annotation(&mut self, id: u64, annotation: String) -> bool {
        if let Some(bookmark) = self.get_mut(id) {
            bookmark.annotation = annotation;
            true
        } else {
            false
        }
    }

    /// Check if there's a bookmark at the given offset (binary search on sorted vec)
    pub fn at_offset(&self, offset: usize) -> Option<&Bookmark> {
        let idx = self.bookmarks.partition_point(|b| b.offset < offset);
        self.bookmarks.get(idx).filter(|b| b.offset == offset)
    }

    /// Check if offset has a bookmark
    pub fn has_bookmark(&self, offset: usize) -> bool {
        self.at_offset(offset).is_some()
    }

    /// Adjust bookmark offsets after bytes were inserted at `offset`
    ///
    /// Bookmarks at or after `offset` are shifted right by `count`.
    pub fn adjust_offsets_after_insert(&mut self, offset: usize, count: usize) {
        for bookmark in &mut self.bookmarks {
            if bookmark.offset >= offset {
                bookmark.offset += count;
            }
        }
    }

    /// Adjust bookmark offsets after bytes were deleted starting at `offset`
    ///
    /// Bookmarks within the deleted range `[offset, offset+count)` are removed.
    /// Bookmarks after the deleted range are shifted left by `count`.
    pub fn adjust_offsets_after_delete(&mut self, offset: usize, count: usize) {
        let delete_end = offset + count;
        self.bookmarks
            .retain(|b| b.offset < offset || b.offset >= delete_end);
        for bookmark in &mut self.bookmarks {
            if bookmark.offset >= delete_end {
                bookmark.offset -= count;
            }
        }
        self.rebuild_index();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_bookmark() {
        let mut manager = BookmarkManager::new();
        manager.add(100, "Test".to_string());

        let bookmarks = manager.all();
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].offset, 100);
        assert_eq!(bookmarks[0].name, "Test");
    }

    #[test]
    fn test_bookmarks_sorted_by_offset() {
        let mut manager = BookmarkManager::new();
        manager.add(300, "Third".to_string());
        manager.add(100, "First".to_string());
        manager.add(200, "Second".to_string());

        let bookmarks = manager.all();
        assert_eq!(bookmarks[0].offset, 100);
        assert_eq!(bookmarks[1].offset, 200);
        assert_eq!(bookmarks[2].offset, 300);
    }

    #[test]
    fn test_remove_bookmark() {
        let mut manager = BookmarkManager::new();
        let id1 = manager.add(100, "First".to_string());
        manager.add(200, "Second".to_string());

        assert!(manager.remove(id1));
        assert_eq!(manager.all().len(), 1);
        assert!(!manager.has_bookmark(100));
        assert!(manager.has_bookmark(200));
    }

    #[test]
    fn test_rename_bookmark() {
        let mut manager = BookmarkManager::new();
        let id = manager.add(100, "Old Name".to_string());

        assert!(manager.rename(id, "New Name".to_string()));
        assert_eq!(manager.at_offset(100).unwrap().name, "New Name");
    }

    #[test]
    fn test_set_annotation() {
        let mut manager = BookmarkManager::new();
        let id = manager.add(100, "Test".to_string());

        assert!(manager.set_annotation(id, "This is a note".to_string()));
        assert_eq!(manager.at_offset(100).unwrap().annotation, "This is a note");
    }

    #[test]
    fn test_at_offset() {
        let mut manager = BookmarkManager::new();
        manager.add(100, "Test".to_string());

        assert!(manager.at_offset(100).is_some());
        assert!(manager.at_offset(200).is_none());
        assert!(manager.has_bookmark(100));
        assert!(!manager.has_bookmark(200));
    }

    #[test]
    fn test_adjust_offsets_after_insert() {
        let mut manager = BookmarkManager::new();
        manager.add(100, "Before".to_string());
        manager.add(200, "At".to_string());
        manager.add(300, "After".to_string());

        // Insert 10 bytes at offset 200
        manager.adjust_offsets_after_insert(200, 10);

        let bookmarks = manager.all();
        assert_eq!(bookmarks[0].offset, 100); // Before: unchanged
        assert_eq!(bookmarks[1].offset, 210); // At insert point: shifted +10
        assert_eq!(bookmarks[2].offset, 310); // After: shifted +10
    }

    #[test]
    fn test_adjust_offsets_after_delete() {
        let mut manager = BookmarkManager::new();
        manager.add(100, "Before".to_string());
        manager.add(150, "In range".to_string());
        manager.add(300, "After".to_string());

        // Delete 100 bytes starting at offset 120 (range 120..220)
        manager.adjust_offsets_after_delete(120, 100);

        let bookmarks = manager.all();
        assert_eq!(bookmarks.len(), 2); // Bookmark at 150 removed
        assert_eq!(bookmarks[0].offset, 100); // Before: unchanged
        assert_eq!(bookmarks[1].offset, 200); // After: shifted -100
    }

    #[test]
    fn test_adjust_offsets_delete_at_boundary() {
        let mut manager = BookmarkManager::new();
        manager.add(10, "Start".to_string());
        manager.add(14, "End in range".to_string());

        // Delete 5 bytes at offset 10 (range 10..15)
        manager.adjust_offsets_after_delete(10, 5);

        let bookmarks = manager.all();
        assert_eq!(bookmarks.len(), 0); // Both removed (both in range [10, 15))
    }
}
