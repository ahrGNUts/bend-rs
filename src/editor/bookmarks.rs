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
pub struct BookmarkManager {
    /// All bookmarks, sorted by offset
    bookmarks: Vec<Bookmark>,
    /// Index lookup by ID for O(1) access
    id_to_index: HashMap<u64, usize>,
    /// Next ID to assign
    next_id: u64,
}

impl Default for BookmarkManager {
    fn default() -> Self {
        Self {
            bookmarks: Vec::new(),
            id_to_index: HashMap::new(),
            next_id: 0,
        }
    }
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
        self.bookmarks.push(bookmark);

        // Keep bookmarks sorted by offset
        self.bookmarks.sort_by_key(|b| b.offset);

        // Rebuild the index map after sorting
        self.rebuild_index();

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

    /// Get a bookmark by ID
    pub fn get(&self, id: u64) -> Option<&Bookmark> {
        self.id_to_index.get(&id).map(|&i| &self.bookmarks[i])
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

    /// Check if there's a bookmark at the given offset
    pub fn at_offset(&self, offset: usize) -> Option<&Bookmark> {
        self.bookmarks.iter().find(|b| b.offset == offset)
    }

    /// Check if offset has a bookmark
    pub fn has_bookmark(&self, offset: usize) -> bool {
        self.at_offset(offset).is_some()
    }

    /// Get the number of bookmarks
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// Check if there are no bookmarks
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }

    /// Clear all bookmarks
    pub fn clear(&mut self) {
        self.bookmarks.clear();
        self.id_to_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_bookmark() {
        let mut manager = BookmarkManager::new();
        let id = manager.add(100, "Test".to_string());

        assert_eq!(manager.len(), 1);

        let bookmark = manager.get(id).unwrap();
        assert_eq!(bookmark.offset, 100);
        assert_eq!(bookmark.name, "Test");
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
        let id2 = manager.add(200, "Second".to_string());

        assert!(manager.remove(id1));
        assert_eq!(manager.len(), 1);
        assert!(manager.get(id1).is_none());
        assert!(manager.get(id2).is_some());
    }

    #[test]
    fn test_rename_bookmark() {
        let mut manager = BookmarkManager::new();
        let id = manager.add(100, "Old Name".to_string());

        assert!(manager.rename(id, "New Name".to_string()));
        assert_eq!(manager.get(id).unwrap().name, "New Name");
    }

    #[test]
    fn test_set_annotation() {
        let mut manager = BookmarkManager::new();
        let id = manager.add(100, "Test".to_string());

        assert!(manager.set_annotation(id, "This is a note".to_string()));
        assert_eq!(manager.get(id).unwrap().annotation, "This is a note");
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
}
