# Tasks: bend-rs Implementation

## 1. Project Foundation
- [ ] 1.1 Initialize Rust project with Cargo
- [ ] 1.2 Add core dependencies (eframe, egui, image, rfd, serde)
- [ ] 1.3 Create basic egui application window with eframe
- [ ] 1.4 Implement native file open dialog (rfd)
- [ ] 1.5 Load file bytes into memory buffer
- [ ] 1.6 Create app state structure to hold loaded file data

## 2. Split View Layout (P0)
- [ ] 2.1 Create two-panel split layout in egui
- [ ] 2.2 Implement basic hex view (display bytes as hex with offsets)
- [ ] 2.3 Add ASCII column alongside hex display
- [ ] 2.4 Implement virtual scrolling for hex view (handle large files)
- [ ] 2.5 Render image from buffer bytes using image crate
- [ ] 2.6 Display rendered image in right panel with scaling
- [ ] 2.7 Handle image decode errors gracefully (show placeholder)

## 3. Byte Editing with Live Preview (P0)
- [ ] 3.1 Implement cursor/selection in hex view
- [ ] 3.2 Handle keyboard input for hex editing (0-9, A-F)
- [ ] 3.3 Update buffer when user types valid hex
- [ ] 3.4 Trigger image re-render on buffer change (debounced)
- [ ] 3.5 Show broken image indicator when decode fails
- [ ] 3.6 Preserve last valid image state for corrupted data option

## 4. Undo/Redo System (P0)
- [ ] 4.1 Design edit operation structure (offset, old value, new value)
- [ ] 4.2 Implement history stack for undo
- [ ] 4.3 Implement redo stack
- [ ] 4.4 Wire up Ctrl+Z / Cmd+Z for undo
- [ ] 4.5 Wire up Ctrl+Shift+Z / Cmd+Shift+Z for redo
- [ ] 4.6 Update hex view and image preview on undo/redo
- [ ] 4.7 Implement history size limit to bound memory

## 5. Non-Destructive Workflow (P0)
- [ ] 5.1 Store original bytes separately (never modify)
- [ ] 5.2 Implement Export / Save As with file dialog
- [ ] 5.3 Write modified buffer to chosen destination
- [ ] 5.4 Add unsaved changes indicator in UI
- [ ] 5.5 Prompt before closing with unsaved changes

## 6. Save Points (P1)
- [ ] 6.1 Design save point data structure (name, timestamp, buffer snapshot or diff)
- [ ] 6.2 Implement Create Save Point action
- [ ] 6.3 Add save points list UI (sidebar or dropdown)
- [ ] 6.4 Implement Restore Save Point action
- [ ] 6.5 Make restore operation undoable
- [ ] 6.6 Allow naming/renaming save points
- [ ] 6.7 Allow deleting save points

## 7. Format Parsing - BMP (P1)
- [ ] 7.1 Define ImageFormat trait (sections, labels, risk levels)
- [ ] 7.2 Implement BMP header parsing (file header, DIB header)
- [ ] 7.3 Identify pixel data offset and size
- [ ] 7.4 Identify optional color table region
- [ ] 7.5 Return structured section list with offsets and labels
- [ ] 7.6 Handle malformed BMP files gracefully

## 8. Format Parsing - JPEG (P1)
- [ ] 8.1 Implement JPEG marker segment parsing
- [ ] 8.2 Identify SOI, EOI markers
- [ ] 8.3 Identify APP markers (APP0, APP1 for EXIF)
- [ ] 8.4 Identify DQT, DHT, SOF segments
- [ ] 8.5 Identify SOS and scan data region
- [ ] 8.6 Mark scan data as high risk for warnings
- [ ] 8.7 Handle malformed JPEG files gracefully

## 9. Structure Visualization (P1)
- [ ] 9.1 Create collapsible tree UI component
- [ ] 9.2 Populate tree from parsed format sections
- [ ] 9.3 Show section name, offset, and size
- [ ] 9.4 Click section to scroll hex view to that offset
- [ ] 9.5 Highlight current section based on cursor position

## 10. Section Highlighting (P1)
- [ ] 10.1 Define color scheme for different section types
- [ ] 10.2 Apply background colors to hex view based on sections
- [ ] 10.3 Add legend or tooltip explaining colors
- [ ] 10.4 Ensure colors are accessible (contrast, colorblind-friendly)

## 11. Graceful Corruption Handling (P1)
- [ ] 11.1 Implement last valid state caching
- [ ] 11.2 Show last valid image when current buffer fails to decode
- [ ] 11.3 Add visual indicator that preview is stale/cached
- [ ] 11.4 Provide clear broken image icon as fallback
- [ ] 11.5 Display decode error message in status area

## 12. Comparison View (P2)
- [ ] 12.1 Add toggle for comparison mode
- [ ] 12.2 Render original image from preserved bytes
- [ ] 12.3 Display original and current side-by-side
- [ ] 12.4 Ensure both images scale together
- [ ] 12.5 Add labels (Original / Current)

## 13. Search and Replace (P2)
- [ ] 13.1 Create search dialog UI
- [ ] 13.2 Implement hex pattern search (e.g., FF D8 FF)
- [ ] 13.3 Implement ASCII string search
- [ ] 13.4 Highlight all matches in hex view
- [ ] 13.5 Add Next / Previous match navigation
- [ ] 13.6 Implement single-occurrence replace
- [ ] 13.7 Implement Replace All as single undoable operation
- [ ] 13.8 Show no matches found feedback

## 14. Bookmarks and Annotations (P2)
- [ ] 14.1 Design bookmark data structure (offset, name, annotation)
- [ ] 14.2 Implement Add Bookmark action at cursor
- [ ] 14.3 Create bookmarks list UI panel
- [ ] 14.4 Click bookmark to navigate to offset
- [ ] 14.5 Highlight bookmarked positions in hex view
- [ ] 14.6 Allow editing bookmark name and annotation
- [ ] 14.7 Allow deleting bookmarks

## 15. Header Protection Toggle (P2)
- [ ] 15.1 Add Protect Headers toggle in toolbar
- [ ] 15.2 Mark header/metadata sections as protected when enabled
- [ ] 15.3 Block edits to protected regions with visual feedback
- [ ] 15.4 Show protection status indicator in hex view
- [ ] 15.5 Persist setting per-file (session only)

## 16. High-Risk Edit Warnings (P3)
- [ ] 16.1 Implement warning dialog component
- [ ] 16.2 Trigger warning when editing high-risk regions (e.g., JPEG scan data)
- [ ] 16.3 Add Proceed and Cancel options
- [ ] 16.4 Add Don't show again checkbox
- [ ] 16.5 Persist warning preference for session
- [ ] 16.6 Add setting to re-enable warnings

## 17. Polish and Platform Testing
- [ ] 17.1 Test on macOS, verify native look and feel
- [ ] 17.2 Test on Windows, verify native look and feel
- [ ] 17.3 Test on Linux, verify native look and feel
- [ ] 17.4 Add keyboard shortcut help / cheat sheet
- [ ] 17.5 Implement Go to offset dialog
- [ ] 17.6 Add toolbar with common actions
- [ ] 17.7 Performance testing with large files (10MB+)
- [ ] 17.8 Memory profiling and optimization if needed

## 18. Documentation and Release Prep
- [ ] 18.1 Write README with screenshots and usage guide
- [ ] 18.2 Create GitHub releases workflow
- [ ] 18.3 Build binaries for macOS, Windows, Linux
- [ ] 18.4 Add LICENSE file
- [ ] 18.5 Create CONTRIBUTING guide if accepting contributions
