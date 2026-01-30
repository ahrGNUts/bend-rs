# Hex Editor Capability

## ADDED Requirements

### Requirement: File Loading
The application SHALL allow users to open BMP and JPEG image files through a native file dialog.

#### Scenario: Open valid image file
- **WHEN** user selects "Open File" and chooses a valid BMP or JPEG file
- **THEN** the file's raw bytes are loaded into the editor buffer
- **AND** the hex view displays the byte data
- **AND** the image preview renders the file

#### Scenario: Open unsupported file type
- **WHEN** user attempts to open a non-BMP/non-JPEG file
- **THEN** an error message is displayed explaining supported formats

### Requirement: Hex Data Display
The application SHALL display file contents as hexadecimal bytes in a scrollable editor view on the left panel.

#### Scenario: View hex data
- **WHEN** a file is loaded
- **THEN** bytes are displayed in hexadecimal format with offset addresses
- **AND** an ASCII representation is shown alongside the hex values
- **AND** the view supports scrolling for files larger than the viewport

#### Scenario: Navigate to offset
- **WHEN** user requests navigation to a specific byte offset
- **THEN** the hex view scrolls to display that offset

### Requirement: Byte-Level Editing
The application SHALL allow users to edit individual bytes by typing hexadecimal values.

#### Scenario: Edit single byte
- **WHEN** user selects a byte position and types a valid hex value (0-9, A-F)
- **THEN** the byte at that position is updated
- **AND** the change is recorded in edit history

#### Scenario: Invalid hex input rejected
- **WHEN** user types a non-hexadecimal character
- **THEN** the input is ignored and the byte remains unchanged

### Requirement: Live Image Preview
The application SHALL display a rendered image preview on the right panel that updates when hex data changes.

#### Scenario: Preview updates on edit
- **WHEN** user modifies hex data
- **THEN** the image preview attempts to re-render from the modified buffer
- **AND** the update occurs within a reasonable time (debounced for rapid edits)

#### Scenario: Corrupted data handling
- **WHEN** hex edits result in data that cannot be parsed as a valid image
- **THEN** the preview shows a "broken image" indicator or the last valid state
- **AND** the user can continue editing to potentially fix the corruption

### Requirement: Edit History with Undo/Redo
The application SHALL maintain a linear edit history allowing users to undo and redo changes.

#### Scenario: Undo edit
- **WHEN** user triggers undo (Ctrl+Z / Cmd+Z)
- **THEN** the most recent edit is reverted
- **AND** the hex view and image preview update accordingly

#### Scenario: Redo edit
- **WHEN** user triggers redo (Ctrl+Shift+Z / Cmd+Shift+Z) after an undo
- **THEN** the previously undone edit is reapplied

#### Scenario: History limit
- **WHEN** edit history exceeds a configured maximum
- **THEN** the oldest entries are discarded to maintain memory bounds

### Requirement: Save Points
The application SHALL allow users to create explicit save points (snapshots) that can be restored.

#### Scenario: Create save point
- **WHEN** user triggers "Create Save Point"
- **THEN** the current buffer state is stored as a named checkpoint
- **AND** the save point appears in a list of available restore points

#### Scenario: Restore save point
- **WHEN** user selects a save point to restore
- **THEN** the buffer is reset to that saved state
- **AND** a new history entry is created (restore is itself undoable)

### Requirement: Non-Destructive Workflow
The application SHALL preserve the original file and require explicit export to save changes.

#### Scenario: Original file preserved
- **WHEN** user makes edits to a loaded file
- **THEN** the original file on disk remains unchanged

#### Scenario: Export modified file
- **WHEN** user triggers "Export" or "Save As"
- **THEN** a file dialog allows choosing the destination
- **AND** the modified buffer is written to the chosen location

### Requirement: Comparison View
The application SHALL provide a side-by-side comparison of the original file state and current edited state.

#### Scenario: View comparison
- **WHEN** user activates comparison view
- **THEN** the original image is displayed alongside the current edited image
- **AND** differences are visually apparent

#### Scenario: Toggle comparison mode
- **WHEN** user toggles comparison view off
- **THEN** the display returns to the standard single-preview layout

### Requirement: Bookmarks and Annotations
The application SHALL allow users to bookmark byte offsets and add text annotations.

#### Scenario: Create bookmark
- **WHEN** user selects a byte offset and triggers "Add Bookmark"
- **THEN** the offset is saved to a bookmarks list
- **AND** user can optionally provide a name/label for the bookmark

#### Scenario: Navigate to bookmark
- **WHEN** user selects a bookmark from the list
- **THEN** the hex editor scrolls to display that bookmarked offset
- **AND** the bookmarked position is visually highlighted

#### Scenario: Delete bookmark
- **WHEN** user removes a bookmark
- **THEN** the bookmark is deleted from the list
- **AND** the underlying data is unaffected

#### Scenario: Annotate bookmark
- **WHEN** user adds or edits an annotation on a bookmark
- **THEN** the annotation text is saved with the bookmark
- **AND** the annotation is displayed when viewing bookmark details

### Requirement: Search and Replace
The application SHALL provide search functionality for byte patterns and ASCII strings, with optional replace capability.

#### Scenario: Search for byte pattern
- **WHEN** user enters a hexadecimal byte pattern (e.g., "FF D8 FF")
- **THEN** all occurrences in the file are found
- **AND** the user can navigate between matches

#### Scenario: Search for ASCII string
- **WHEN** user enters an ASCII string to search for
- **THEN** all occurrences of that string in the file are found
- **AND** the user can navigate between matches

#### Scenario: Replace single occurrence
- **WHEN** user is at a search match and triggers "Replace"
- **THEN** the matched bytes are replaced with the specified replacement
- **AND** the change is recorded in edit history

#### Scenario: Replace all occurrences
- **WHEN** user triggers "Replace All"
- **THEN** all matches are replaced with the specified replacement
- **AND** all changes are recorded in edit history (as a single undoable operation)

#### Scenario: No matches found
- **WHEN** user searches for a pattern that does not exist in the file
- **THEN** a message indicates no matches were found
