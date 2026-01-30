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
- [ ] 1.5 Load file bytes into memory buffer
- [ ] 1.6 Create app state structure to hold loaded file data

### From Phase 4: Undo/Redo System
- [ ] 4.1 Design edit operation structure (offset, old value, new value)
- [ ] 4.2 Implement history stack for undo
- [ ] 4.3 Implement redo stack
- [ ] 4.7 Implement history size limit to bound memory

### From Phase 5: Non-Destructive Workflow
- [ ] 5.1 Store original bytes separately (never modify)
- [ ] 5.3 Write modified buffer to chosen destination

### From Phase 6: Save Points
- [ ] 6.1 Design save point data structure (name, timestamp, buffer snapshot or diff)

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
- [ ] 13.3 Implement ASCII string search

### From Phase 14: Bookmarks and Annotations
- [ ] 14.1 Design bookmark data structure (offset, name, annotation)

## Completed
