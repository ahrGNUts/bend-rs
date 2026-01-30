# Backend Tasks

Tasks related to core logic, data structures, and non-UI functionality.

## In Progress

## Pending

### Testing
- [ ] T.1 Add unit tests for BMP header parsing
- [ ] T.2 Add unit tests for JPEG marker parsing
- [ ] T.3 Add integration test for file load -> edit -> export workflow

### From Phase 1: Project Foundation
- [ ] 1.1 Initialize Rust project with Cargo
- [ ] 1.2 Add core dependencies (eframe, egui, image, rfd, serde)
- [ ] 1.6 Load file bytes into memory buffer
- [ ] 1.7 Create app state structure to hold loaded file data
- [ ] 1.8 Implement dual-buffer architecture
  - `original: Vec<u8>` - immutable after load, used for comparison and save point base
  - `working: Vec<u8>` - all edits apply here, undo/redo operates on this
  - Document this architecture in code comments

### From Phase 4: Undo/Redo System
- [ ] 4.1 Design edit operation structure (offset, old value, new value)
- [ ] 4.2 Implement history stack for undo
- [ ] 4.3 Implement redo stack
- [ ] 4.7 Implement history management
  - Coalesce adjacent single-byte edits within 500ms into single operations
  - Cap history at 1000 operations
  - Silently drop oldest operations when limit reached
  - Consider: add UI indicator when history is truncated

### From Phase 5: Non-Destructive Workflow
- [ ] 5.1 Store original bytes separately (never modify)
- [ ] 5.3 Write modified buffer to chosen destination

### From Phase 6: Save Points
- [ ] 6.1 Design save point data structure
  - Store: name, timestamp, Vec<(offset, old_byte, new_byte)> as diff from previous save point
  - First save point diffs from original file
  - Restore by: reset to original, apply chain of diffs up to target
  - Memory: only stores incremental changes between save points
  - Note: deletion must either recompute successor's diff or prevent deleting non-leaf save points
- [ ] 6.8 Handle save point deletion in chain
  - Either recompute successor's diff or restrict to leaf-only deletion

### From Phase 7: Format Parsing - BMP
- [ ] 7.1 Define ImageFormat trait (sections, labels, risk levels)
- [ ] 7.2 Implement BMP header parsing (file header, DIB header)
- [ ] 7.3 Identify pixel data offset and size
- [ ] 7.4 Identify optional color table region
- [ ] 7.5 Return structured section list with offsets and labels
- [ ] 7.6 Handle malformed BMP files gracefully

### From Phase 8: Format Parsing - JPEG
- [ ] 8.1 Implement JPEG marker segment parsing
- [ ] 8.2 Identify SOI, EOI markers
- [ ] 8.3 Identify APP markers (APP0, APP1 for EXIF)
- [ ] 8.4 Identify DQT, DHT, SOF segments
- [ ] 8.5 Identify SOS and scan data region
- [ ] 8.6 Mark scan data as high risk for warnings
- [ ] 8.7 Handle malformed JPEG files gracefully

### From Phase 13: Search and Replace
- [ ] 13.2 Implement hex pattern search (e.g., FF D8 FF)
- [ ] 13.3 Add wildcard support for hex search (e.g., FF ?? FF)
- [ ] 13.4 Implement ASCII string search

### From Phase 18: Settings and Persistence
- [ ] 18.1 Design settings data structure (window size, recent files, preferences)
- [ ] 18.2 Persist settings to disk (platform-appropriate location)

### From Phase 14: Bookmarks and Annotations
- [ ] 14.1 Design bookmark data structure (offset, name, annotation)

## Completed
