# Frontend Tasks

Tasks related to UI components, user interaction, and visual presentation.

## In Progress

## Pending

### From Phase 1: Project Foundation
- [ ] 1.3 Create basic egui application window with eframe
- [ ] 1.4 Implement native file open dialog (rfd)
- [ ] 1.5 Implement drag-and-drop file open

### From Phase 2: Split View Layout
- [ ] 2.1 Create two-panel split layout in egui
- [ ] 2.2 Implement hex view layout structure
  - [ ] 2.2a Define row structure: offset column (8 chars) + hex bytes + ASCII column
  - [ ] 2.2b Implement byte grouping (16 bytes per row, space every 8 bytes)
  - [ ] 2.2c Render rows with monospace font and proper column alignment
- [ ] 2.3 Add ASCII column alongside hex display
- [ ] 2.4 Implement virtual scrolling for hex view
  - Use row-based virtualization (16 bytes per row)
  - Calculate visible row range from scroll position
  - Only render rows in viewport + small buffer above/below
  - Target: handle files up to 100MB without UI lag
- [ ] 2.5 Render image from buffer bytes using image crate
- [ ] 2.6 Display rendered image in right panel with scaling
- [ ] 2.7 Handle image decode errors gracefully (show placeholder)

### From Phase 3: Byte Editing with Live Preview
- [ ] 3.1 Implement cursor/selection in hex view
- [ ] 3.2 Handle keyboard input for hex editing
  - [ ] 3.2a Capture and filter keyboard input (accept only 0-9, A-F, a-f)
  - [ ] 3.2b Implement nibble-level editing (track high/low nibble state)
  - [ ] 3.2c Auto-advance cursor after completing byte (two nibbles entered)
- [ ] 3.3 Update buffer when user types valid hex
- [ ] 3.4 Trigger image re-render on buffer change
  - Debounce re-renders (100-200ms after last edit)
  - On decode failure, trigger graceful corruption handling (see 11.1-11.3)
- [ ] 3.5 Show broken image indicator when decode fails
- [ ] 3.6 Implement keyboard navigation (arrow keys, Page Up/Down, Home/End)
- [ ] 3.7 Implement range selection (Shift+click, Shift+arrow keys)

### From Phase 4: Undo/Redo System
- [ ] 4.4 Wire up Ctrl+Z / Cmd+Z for undo
- [ ] 4.5 Wire up Ctrl+Shift+Z / Cmd+Shift+Z for redo
- [ ] 4.6 Update hex view and image preview on undo/redo

### From Phase 5: Non-Destructive Workflow
- [ ] 5.2 Implement Export / Save As with file dialog
- [ ] 5.4 Add unsaved changes indicator in UI
- [ ] 5.5 Prompt before closing with unsaved changes

### From Phase 6: Save Points
- [ ] 6.2 Implement Create Save Point action
- [ ] 6.3 Add save points list UI (sidebar or dropdown)
- [ ] 6.4 Implement Restore Save Point action
- [ ] 6.5 Make restore operation undoable
- [ ] 6.6 Allow naming/renaming save points
- [ ] 6.7 Allow deleting save points
- [ ] 6.8 Handle save point deletion in chain (UI)
  - Update UI to reflect deletion constraints

### From Phase 9: Structure Visualization
Prerequisites: Sections 7-8 (Format Parsing)

- [ ] 9.1 Create collapsible tree UI component
- [ ] 9.2 Populate tree from parsed format sections
- [ ] 9.3 Show section name, offset, and size
- [ ] 9.4 Click section to scroll hex view to that offset
- [ ] 9.5 Highlight current section based on cursor position

### From Phase 10: Section Highlighting
Prerequisites: 2.2-2.3 (hex view must support background colors)

- [ ] 10.1 Define color scheme for different section types
- [ ] 10.2 Apply background colors to hex view based on sections
- [ ] 10.3 Add legend or tooltip explaining colors
- [ ] 10.4 Ensure colors are accessible (contrast, colorblind-friendly)
- [ ] 10.5 Test section highlighting with screen reader / accessibility tools

### From Phase 11: Graceful Corruption Handling
- [ ] 11.1 Implement last valid state caching
- [ ] 11.2 Show last valid image when current buffer fails to decode
- [ ] 11.3 Add visual indicator that preview is stale/cached
- [ ] 11.4 Provide clear broken image icon as fallback
- [ ] 11.5 Display decode error message in status area

### From Phase 12: Comparison View
- [ ] 12.1 Add toggle for comparison mode
- [ ] 12.2 Render original image from preserved bytes
- [ ] 12.3 Display original and current side-by-side
- [ ] 12.4 Ensure both images scale together
- [ ] 12.5 Add labels (Original / Current)

### From Phase 13: Search and Replace
- [ ] 13.1 Create search dialog UI
- [ ] 13.5 Add case-sensitive toggle for ASCII search
- [ ] 13.6 Highlight all matches in hex view
- [ ] 13.7 Add Next / Previous match navigation
- [ ] 13.8 Implement single-occurrence replace
- [ ] 13.9 Implement Replace All as single undoable operation
- [ ] 13.10 Show no matches found feedback

### From Phase 14: Bookmarks and Annotations
- [ ] 14.2 Implement Add Bookmark action at cursor
- [ ] 14.3 Create bookmarks list UI panel
- [ ] 14.4 Click bookmark to navigate to offset
- [ ] 14.5 Highlight bookmarked positions in hex view
- [ ] 14.6 Allow editing bookmark name and annotation
- [ ] 14.7 Allow deleting bookmarks

### From Phase 15: Header Protection Toggle
Prerequisites: Sections 7-8 (format parsing to identify header regions)

- [ ] 15.1 Add Protect Headers toggle in toolbar
- [ ] 15.2 Mark header/metadata sections as protected when enabled
- [ ] 15.3 Block edits to protected regions with visual feedback
- [ ] 15.4 Show protection status indicator in hex view
- [ ] 15.5 Persist setting per-file (session only)

### From Phase 16: High-Risk Edit Warnings
- [ ] 16.1 Implement warning dialog component
- [ ] 16.2 Trigger warning when editing high-risk regions
- [ ] 16.3 Add Proceed and Cancel options
- [ ] 16.4 Add Don't show again checkbox
- [ ] 16.5 Persist warning preference for session
- [ ] 16.6 Add setting to re-enable warnings

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
