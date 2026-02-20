# Tasks: bend-rs Implementation

## 1. Project Foundation
- [x] 1.1 Initialize Rust project with Cargo
- [x] 1.2 Add core dependencies (eframe, egui, image, rfd, serde)
- [x] 1.3 Create basic egui application window with eframe
- [x] 1.4 Implement native file open dialog (rfd)
- [x] 1.5 Implement drag-and-drop file open
- [x] 1.6 Load file bytes into memory buffer
- [x] 1.7 Create app state structure to hold loaded file data
- [x] 1.8 Implement dual-buffer architecture
  - `original: Vec<u8>` - immutable after load, used for comparison and save point base
  - `working: Vec<u8>` - all edits apply here, undo/redo operates on this
  - Document this architecture in code comments

## 2. Split View Layout (P0)
- [x] 2.1 Create two-panel split layout in egui
- [x] 2.2 Implement hex view layout structure
  - [x] 2.2a Define row structure: offset column (8 chars) + hex bytes + ASCII column
  - [x] 2.2b Implement byte grouping (16 bytes per row, space every 8 bytes)
  - [x] 2.2c Render rows with monospace font and proper column alignment
- [x] 2.3 Add ASCII column alongside hex display
- [x] 2.4 Implement virtual scrolling for hex view
  - Use row-based virtualization (16 bytes per row)
  - Calculate visible row range from scroll position
  - Only render rows in viewport + small buffer above/below
  - Target: handle files up to 100MB without UI lag
- [x] 2.5 Render image from buffer bytes using image crate
- [x] 2.6 Display rendered image in right panel with scaling
- [x] 2.7 Handle image decode errors gracefully (show placeholder)

## 3. Byte Editing with Live Preview (P0)
- [x] 3.1 Implement cursor/selection in hex view
- [x] 3.2 Handle keyboard input for hex editing
  - [x] 3.2a Capture and filter keyboard input (accept only 0-9, A-F, a-f)
  - [x] 3.2b Implement nibble-level editing (track high/low nibble state)
  - [x] 3.2c Auto-advance cursor after completing byte (two nibbles entered)
- [x] 3.3 Update buffer when user types valid hex
- [x] 3.4 Trigger image re-render on buffer change
  - Debounce re-renders (100-200ms after last edit)
  - On decode failure, trigger graceful corruption handling (see 11.1-11.3)
- [x] 3.5 Show broken image indicator when decode fails
- [x] 3.6 Implement keyboard navigation (arrow keys, Page Up/Down, Home/End)
- [x] 3.7 Implement range selection (Shift+click, Shift+arrow keys)

## 4. Undo/Redo System (P0)
- [x] 4.1 Design edit operation structure (offset, old value, new value)
- [x] 4.2 Implement history stack for undo
- [x] 4.3 Implement redo stack
- [x] 4.4 Wire up Ctrl+Z / Cmd+Z for undo
- [x] 4.5 Wire up Ctrl+Shift+Z / Cmd+Shift+Z for redo
- [x] 4.6 Update hex view and image preview on undo/redo
- [x] 4.7 Implement history management
  - Coalesce adjacent single-byte edits within 500ms into single operations
  - Cap history at 1000 operations
  - Silently drop oldest operations when limit reached
  - Consider: add UI indicator when history is truncated

## 5. Non-Destructive Workflow (P0)
- [x] 5.1 Store original bytes separately (never modify)
- [x] 5.2 Implement Export / Save As with file dialog
- [x] 5.3 Write modified buffer to chosen destination
- [x] 5.4 Add unsaved changes indicator in UI
- [x] 5.5 Prompt before closing with unsaved changes

## 6. Save Points (P1)
- [x] 6.1 Design save point data structure
  - Store: name, timestamp, Vec<(offset, old_byte, new_byte)> as diff from previous save point
  - First save point diffs from original file
  - Restore by: reset to original, apply chain of diffs up to target
  - Memory: only stores incremental changes between save points
  - Note: deletion must either recompute successor's diff or prevent deleting non-leaf save points
- [x] 6.2 Implement Create Save Point action
- [x] 6.3 Add save points list UI (sidebar or dropdown)
- [x] 6.4 Implement Restore Save Point action
- [x] 6.5 Make restore operation undoable
- [x] 6.6 Allow naming/renaming save points
- [x] 6.7 Allow deleting save points
- [x] 6.8 Handle save point deletion in chain
  - Either recompute successor's diff or restrict to leaf-only deletion
  - Update UI to reflect deletion constraints
- [x] 6.9 Add keyboard shortcut of command/ctrl + S to create new save point

## 7. Format Parsing - BMP (P1)
- [x] 7.1 Define ImageFormat trait (sections, labels, risk levels)
- [x] 7.2 Implement BMP header parsing (file header, DIB header)
- [x] 7.3 Identify pixel data offset and size
- [x] 7.4 Identify optional color table region
- [x] 7.5 Return structured section list with offsets and labels
- [x] 7.6 Handle malformed BMP files gracefully

## 8. Format Parsing - JPEG (P1)
- [x] 8.1 Implement JPEG marker segment parsing
- [x] 8.2 Identify SOI, EOI markers
- [x] 8.3 Identify APP markers (APP0, APP1 for EXIF)
- [x] 8.4 Identify DQT, DHT, SOF segments
- [x] 8.5 Identify SOS and scan data region
- [x] 8.6 Mark scan data as high risk for warnings
- [x] 8.7 Handle malformed JPEG files gracefully

## 9. Structure Visualization (P1)
Prerequisites: Sections 7-8 (Format Parsing)

- [x] 9.1 Create collapsible tree UI component
- [x] 9.2 Populate tree from parsed format sections
- [x] 9.3 Show section name, offset, and size
- [x] 9.4 Click section to scroll hex view to that offset
- [x] 9.5 Highlight current section based on cursor position

## 10. Section Highlighting (P1)
Prerequisites: 2.2-2.3 (hex view must support background colors)

- [x] 10.1 Define color scheme for different section types
- [x] 10.2 Apply background colors to hex view based on sections
- [x] 10.3 Add legend or tooltip explaining colors
- [x] 10.4 Ensure colors are accessible (contrast, colorblind-friendly)
- [ ] 10.5 Test section highlighting with screen reader / accessibility tools

## 11. Graceful Corruption Handling (P1)
- [x] 11.1 Implement last valid state caching
- [x] 11.2 Show last valid image when current buffer fails to decode
- [x] 11.3 Add visual indicator that preview is stale/cached
- [x] 11.4 Provide clear broken image icon as fallback
- [x] 11.5 Display decode error message in status area

## 12. Comparison View (P2)
- [x] 12.1 Add toggle for comparison mode
- [x] 12.2 Render original image from preserved bytes
- [x] 12.3 Display original and current side-by-side
- [x] 12.4 Ensure both images scale together
- [x] 12.5 Add labels (Original / Current)

## 13. Search and Replace (P2)
- [x] 13.1 Create search dialog UI
- [x] 13.2 Implement hex pattern search (e.g., FF D8 FF)
- [x] 13.3 Add wildcard support for hex search (e.g., FF ?? FF)
- [x] 13.4 Implement ASCII string search
- [x] 13.5 Add case-sensitive toggle for ASCII search
- [x] 13.6 Highlight all matches in hex view
- [x] 13.7 Add Next / Previous match navigation
- [x] 13.8 Implement single-occurrence replace
- [x] 13.9 Implement Replace All as single undoable operation
- [x] 13.10 Show no matches found feedback

## 14. Bookmarks and Annotations (P2)
- [x] 14.1 Design bookmark data structure (offset, name, annotation)
- [x] 14.2 Implement Add Bookmark action at cursor
- [x] 14.3 Create bookmarks list UI panel
- [x] 14.4 Click bookmark to navigate to offset
- [x] 14.5 Highlight bookmarked positions in hex view
- [x] 14.6 Allow editing bookmark name and annotation
- [x] 14.7 Allow deleting bookmarks
- [x] 14.8 Create keyboard shortcut of command/ctrl + D to add a new bookmark in the hex editor

## 15. Header Protection Toggle (P2)
Prerequisites: Sections 7-8 (format parsing to identify header regions)

- [x] 15.1 Add Protect Headers toggle in toolbar
- [x] 15.2 Mark header/metadata sections as protected when enabled
- [x] 15.3 Block edits to protected regions with visual feedback
- [x] 15.4 Show protection status indicator in hex view
- [x] 15.5 Persist setting per-file (session only)

## 16. High-Risk Edit Warnings (P3)
- [x] 16.1 Implement warning dialog component
- [x] 16.2 Trigger warning when editing high-risk regions (e.g., JPEG scan data)
- [x] 16.3 Add Proceed and Cancel options
- [x] 16.4 Add Don't show again checkbox
- [x] 16.5 Persist warning preference for session
- [x] 16.6 Add setting to re-enable warnings

## 17. Polish and Platform Testing
- [x] 17.1 Test on macOS, verify native look and feel
- [ ] 17.2 Test on Windows, verify native look and feel
- [ ] 17.3 Test on Linux, verify native look and feel
- [x] 17.4 Add keyboard shortcut help / cheat sheet (depends on phases 3, 4, 13)
- [x] 17.5 Implement Go to offset dialog
- [x] 17.6 Add toolbar with common actions
- [x] 17.7 Add right-click context menu in hex view
  - [x] 17.7a Create context menu UI component
  - [x] 17.7b Implement Copy (hex and ASCII formats)
  - [x] 17.7c Implement Paste from clipboard (depends on 3.x editing)
  - [x] 17.7d Add "Bookmark here" action (depends on 14.x)
  - [x] 17.7e Add "Go to offset..." action (depends on 17.5)
- [x] 17.8 BUG: Settings should be persistent between application restarts
- [x] 17.9 ENHANCEMENT: Switch from updating image on keystroke in hex editor to an unsaved changes/reload bytes model
- [ ] 17.10 Switch from using placeholder app icon to databent base_converted.jpg
- [x] 17.11 ENHANCEMENT: Implement an insert/overwrite mode when editing bytes and ascii
- [x] 17.12 BUG: typing in the find and replace modal should capture the cursor and not cause input in the hex or ascii editors
- [x] 17.13 BUG: shortcuts dialog title should be visible at all times and not grow vertically out of view of the user
- [x] 17.14 BUG: clicking next/previous buttons in find & replace dialog should scroll to the matching string or byte
- [x] 17.15 ENHANCEMENT: Enter/Shift+Enter in search dialog for next/previous match navigation
- [x] 17.16 BUG: Remove unused `_pattern_len` parameter from `is_within_match`
- [x] 17.17 BUG: Closing search dialog should clear search highlights
- [x] 17.18 BUG: Replace creates per-byte undo operations instead of single atomic undo
- [x] 17.19 BUG: ASCII replace with different-length replacement corrupts data (no length validation)
- [x] 17.20 BUG: After replace, current_match resets to index 0 instead of staying near replaced position
- [x] 17.21 BUG: Enter navigates stale matches after query/mode/case change (regression from 17.15)
- [x] 17.22 BUG: Search results not invalidated when buffer changes via manual edits
- [x] 17.23 BUG: Shift+Enter on first search causes double-scroll (both do_search and do_prev fire)
- [x] 17.24 ENHANCEMENT: Use strikethrough instead of opaque red background for protected bytes
- [x] 17.25 BUG: vertical scrolling should be enabled for far left panel when contents extend beyond window height
- [x] 17.26 BUG: replace operations should ignore headers when 'Protect Headers' is enabled
- [x] 17.27 ENHANCEMENT: search_state.error field is overloaded for both errors and informational messages (e.g. "Replaced N of M, K skipped") — all render in red. Add a separate info message field or enum to distinguish severity and render informational messages in a neutral/yellow color.
- [x] 17.28 BUG: Replace All creates per-match undo operations instead of a single atomic undo (Ctrl+Z only undoes the last replacement, not all of them)

## 18. Settings and Persistence (P3)
- [x] 18.1 Design settings data structure (window size, recent files, preferences)
- [x] 18.2 Persist settings to disk (platform-appropriate location)
- [x] 18.3 Restore window size and position on launch
- [x] 18.4 Implement recent files list (File menu)
- [x] 18.5 Add Settings/Preferences dialog

## 19. Documentation and Release Prep
- [ ] 19.1 Write README with screenshots and usage guide
- [ ] 19.2 Create GitHub releases workflow
- [ ] 19.3 Build binaries for macOS, Windows, Linux
- [x] 19.4 Add LICENSE file
- [ ] 19.5 Create CONTRIBUTING guide if accepting contributions

## 20. Performance Validation
- [ ] 20.1 Create benchmark harness for file loading (validates 1.6)
- [ ] 20.2 Create benchmark for scroll performance (validates 2.4)
- [ ] 20.3 Verify: 10MB file loads in <2s
- [ ] 20.4 Verify: scroll latency <50ms at 60fps
- [ ] 20.5 Profile memory: working set <2x file size for files under 50MB
