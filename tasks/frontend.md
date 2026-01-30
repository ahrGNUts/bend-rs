# Frontend Tasks

Tasks related to UI components, user interaction, and visual presentation.

## In Progress

## Pending

### From Phase 10: Section Highlighting
- [ ] 10.5 Test section highlighting with screen reader / accessibility tools

### From Phase 17: Polish
- [ ] 17.4 Add keyboard shortcut help / cheat sheet (depends on phases 3, 4, 13)
- [ ] 17.5 Implement Go to offset dialog
- [ ] 17.6 Add toolbar with common actions
- [ ] 17.7 Add right-click context menu in hex view
  - [ ] 17.7a Create context menu UI component
  - [ ] 17.7b Implement Copy (hex and ASCII formats)
  - [ ] 17.7c Implement Paste from clipboard (depends on 3.x editing)
  - [ ] 17.7d Add "Bookmark here" action (depends on 14.x)
  - [ ] 17.7e Add "Go to offset..." action (depends on 17.5)

### From Phase 18: Settings and Persistence
- [ ] 18.3 Restore window size and position on launch
- [ ] 18.4 Implement recent files list (File menu)
- [ ] 18.5 Add Settings/Preferences dialog

## Completed

### From Phase 1: Project Foundation
- [x] 1.3 Create basic egui application window with eframe
- [x] 1.4 Implement native file open dialog (rfd)
- [x] 1.5 Implement drag-and-drop file open

### From Phase 2: Split View Layout
- [x] 2.1 Create two-panel split layout in egui
- [x] 2.2 Implement hex view layout structure
  - [x] 2.2a Define row structure: offset column (8 chars) + hex bytes + ASCII column
  - [x] 2.2b Implement byte grouping (16 bytes per row, space every 8 bytes)
  - [x] 2.2c Render rows with monospace font and proper column alignment
- [x] 2.3 Add ASCII column alongside hex display
- [x] 2.4 Implement virtual scrolling for hex view
- [x] 2.5 Render image from buffer bytes using image crate
- [x] 2.6 Display rendered image in right panel with scaling
- [x] 2.7 Handle image decode errors gracefully (show placeholder)

### From Phase 3: Byte Editing with Live Preview
- [x] 3.1 Implement cursor/selection in hex view
- [x] 3.2 Handle keyboard input for hex editing
  - [x] 3.2a Capture and filter keyboard input (accept only 0-9, A-F, a-f)
  - [x] 3.2b Implement nibble-level editing (track high/low nibble state)
  - [x] 3.2c Auto-advance cursor after completing byte (two nibbles entered)
- [x] 3.3 Update buffer when user types valid hex
- [x] 3.4 Trigger image re-render on buffer change
- [x] 3.5 Show broken image indicator when decode fails
- [x] 3.6 Implement keyboard navigation (arrow keys, Page Up/Down, Home/End)
- [x] 3.7 Implement range selection (Shift+click, Shift+arrow keys)

### From Phase 4: Undo/Redo System
- [x] 4.4 Wire up Ctrl+Z / Cmd+Z for undo
- [x] 4.5 Wire up Ctrl+Shift+Z / Cmd+Shift+Z for redo
- [x] 4.6 Update hex view and image preview on undo/redo

### From Phase 5: Non-Destructive Workflow
- [x] 5.2 Implement Export / Save As with file dialog
- [x] 5.4 Add unsaved changes indicator in UI
- [x] 5.5 Prompt before closing with unsaved changes

### From Phase 6: Save Points
- [x] 6.2 Implement Create Save Point action
- [x] 6.3 Add save points list UI (sidebar or dropdown)
- [x] 6.4 Implement Restore Save Point action
- [x] 6.5 Make restore operation undoable
- [x] 6.6 Allow naming/renaming save points
- [x] 6.7 Allow deleting save points
- [x] 6.8 Handle save point deletion in chain (UI)

### From Phase 9: Structure Visualization
- [x] 9.1 Create collapsible tree UI component
- [x] 9.2 Populate tree from parsed format sections
- [x] 9.3 Show section name, offset, and size
- [x] 9.4 Click section to scroll hex view to that offset
- [x] 9.5 Highlight current section based on cursor position

### From Phase 10: Section Highlighting
- [x] 10.1 Define color scheme for different section types
- [x] 10.2 Apply background colors to hex view based on sections
- [x] 10.3 Add legend or tooltip explaining colors
- [x] 10.4 Ensure colors are accessible (contrast, colorblind-friendly)

### From Phase 11: Graceful Corruption Handling
- [x] 11.1 Implement last valid state caching
- [x] 11.2 Show last valid image when current buffer fails to decode
- [x] 11.3 Add visual indicator that preview is stale/cached
- [x] 11.4 Provide clear broken image icon as fallback
- [x] 11.5 Display decode error message in status area

### From Phase 12: Comparison View
- [x] 12.1 Add toggle for comparison mode
- [x] 12.2 Render original image from preserved bytes
- [x] 12.3 Display original and current side-by-side
- [x] 12.4 Ensure both images scale together
- [x] 12.5 Add labels (Original / Current)

### From Phase 13: Search and Replace
- [x] 13.1 Create search dialog UI
- [x] 13.5 Add case-sensitive toggle for ASCII search
- [x] 13.6 Highlight all matches in hex view
- [x] 13.7 Add Next / Previous match navigation
- [x] 13.8 Implement single-occurrence replace
- [x] 13.9 Implement Replace All as single undoable operation
- [x] 13.10 Show no matches found feedback

### From Phase 14: Bookmarks and Annotations
- [x] 14.2 Implement Add Bookmark action at cursor
- [x] 14.3 Create bookmarks list UI panel
- [x] 14.4 Click bookmark to navigate to offset
- [x] 14.5 Highlight bookmarked positions in hex view
- [x] 14.6 Allow editing bookmark name and annotation
- [x] 14.7 Allow deleting bookmarks

### From Phase 15: Header Protection Toggle
- [x] 15.1 Add Protect Headers toggle in toolbar
- [x] 15.2 Mark header/metadata sections as protected when enabled
- [x] 15.3 Block edits to protected regions with visual feedback
- [x] 15.4 Show protection status indicator in hex view
- [x] 15.5 Persist setting per-file (session only)

### From Phase 16: High-Risk Edit Warnings
- [x] 16.1 Implement warning dialog component
- [x] 16.2 Trigger warning when editing high-risk regions
- [x] 16.3 Add Proceed and Cancel options
- [x] 16.4 Add Don't show again checkbox
- [x] 16.5 Persist warning preference for session
- [x] 16.6 Add setting to re-enable warnings
