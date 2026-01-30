# Frontend Tasks

Tasks related to UI components, user interaction, and visual presentation.

## In Progress

## Pending

### From Phase 1: Project Foundation
- [ ] 1.3 Create basic egui application window with eframe
- [ ] 1.4 Implement native file open dialog (rfd)

### From Phase 2: Split View Layout
- [ ] 2.1 Create two-panel split layout in egui
- [ ] 2.2 Implement basic hex view (display bytes as hex with offsets)
- [ ] 2.3 Add ASCII column alongside hex display
- [ ] 2.4 Implement virtual scrolling for hex view (handle large files)
- [ ] 2.5 Render image from buffer bytes using image crate
- [ ] 2.6 Display rendered image in right panel with scaling
- [ ] 2.7 Handle image decode errors gracefully (show placeholder)

### From Phase 3: Byte Editing with Live Preview
- [ ] 3.1 Implement cursor/selection in hex view
- [ ] 3.2 Handle keyboard input for hex editing (0-9, A-F)
- [ ] 3.3 Update buffer when user types valid hex
- [ ] 3.4 Trigger image re-render on buffer change (debounced)
- [ ] 3.5 Show broken image indicator when decode fails
- [ ] 3.6 Preserve last valid image state for corrupted data option

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

### From Phase 9: Structure Visualization
- [ ] 9.1 Create collapsible tree UI component
- [ ] 9.2 Populate tree from parsed format sections
- [ ] 9.3 Show section name, offset, and size
- [ ] 9.4 Click section to scroll hex view to that offset
- [ ] 9.5 Highlight current section based on cursor position

### From Phase 10: Section Highlighting
- [ ] 10.1 Define color scheme for different section types
- [ ] 10.2 Apply background colors to hex view based on sections
- [ ] 10.3 Add legend or tooltip explaining colors
- [ ] 10.4 Ensure colors are accessible (contrast, colorblind-friendly)

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
- [ ] 13.4 Highlight all matches in hex view
- [ ] 13.5 Add Next / Previous match navigation
- [ ] 13.6 Implement single-occurrence replace
- [ ] 13.7 Implement Replace All as single undoable operation
- [ ] 13.8 Show no matches found feedback

### From Phase 14: Bookmarks and Annotations
- [ ] 14.2 Implement Add Bookmark action at cursor
- [ ] 14.3 Create bookmarks list UI panel
- [ ] 14.4 Click bookmark to navigate to offset
- [ ] 14.5 Highlight bookmarked positions in hex view
- [ ] 14.6 Allow editing bookmark name and annotation
- [ ] 14.7 Allow deleting bookmarks

### From Phase 15: Header Protection Toggle
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
- [ ] 17.4 Add keyboard shortcut help / cheat sheet
- [ ] 17.5 Implement Go to offset dialog
- [ ] 17.6 Add toolbar with common actions

## Completed
