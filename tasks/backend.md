# Backend Tasks

Tasks related to core logic, data structures, and non-UI functionality.

## In Progress

## Pending

### Testing
- [ ] T.3 Add integration test for file load -> edit -> export workflow

### From Phase 18: Settings and Persistence
- [ ] 18.1 Design settings data structure (window size, recent files, preferences)
- [ ] 18.2 Persist settings to disk (platform-appropriate location)

## Completed

### Testing
- [x] T.1 Add unit tests for BMP header parsing
- [x] T.2 Add unit tests for JPEG marker parsing

### From Phase 1: Project Foundation
- [x] 1.1 Initialize Rust project with Cargo
- [x] 1.2 Add core dependencies (eframe, egui, image, rfd, serde)
- [x] 1.6 Load file bytes into memory buffer
- [x] 1.7 Create app state structure to hold loaded file data
- [x] 1.8 Implement dual-buffer architecture
  - `original: Vec<u8>` - immutable after load, used for comparison and save point base
  - `working: Vec<u8>` - all edits apply here, undo/redo operates on this

### From Phase 4: Undo/Redo System
- [x] 4.1 Design edit operation structure (offset, old value, new value)
- [x] 4.2 Implement history stack for undo
- [x] 4.3 Implement redo stack
- [x] 4.7 Implement history management
  - Coalesce adjacent single-byte edits within 500ms into single operations
  - Cap history at 1000 operations
  - Silently drop oldest operations when limit reached

### From Phase 5: Non-Destructive Workflow
- [x] 5.1 Store original bytes separately (never modify)
- [x] 5.3 Write modified buffer to chosen destination

### From Phase 6: Save Points
- [x] 6.1 Design save point data structure
  - Store: name, timestamp, Vec<(offset, old_byte, new_byte)> as diff from previous save point
  - First save point diffs from original file
  - Restore by: reset to original, apply chain of diffs up to target
  - Memory: only stores incremental changes between save points
- [x] 6.8 Handle save point deletion in chain
  - Recompute successor's diff when deleting non-leaf save points

### From Phase 7: Format Parsing - BMP
- [x] 7.1 Define ImageFormat trait (sections, labels, risk levels)
- [x] 7.2 Implement BMP header parsing (file header, DIB header)
- [x] 7.3 Identify pixel data offset and size
- [x] 7.4 Identify optional color table region
- [x] 7.5 Return structured section list with offsets and labels
- [x] 7.6 Handle malformed BMP files gracefully

### From Phase 8: Format Parsing - JPEG
- [x] 8.1 Implement JPEG marker segment parsing
- [x] 8.2 Identify SOI, EOI markers
- [x] 8.3 Identify APP markers (APP0, APP1 for EXIF)
- [x] 8.4 Identify DQT, DHT, SOF segments
- [x] 8.5 Identify SOS and scan data region
- [x] 8.6 Mark scan data as high risk for warnings
- [x] 8.7 Handle malformed JPEG files gracefully

### From Phase 13: Search and Replace
- [x] 13.2 Implement hex pattern search (e.g., FF D8 FF)
- [x] 13.3 Add wildcard support for hex search (e.g., FF ?? FF)
- [x] 13.4 Implement ASCII string search

### From Phase 14: Bookmarks and Annotations
- [x] 14.1 Design bookmark data structure (offset, name, annotation)
