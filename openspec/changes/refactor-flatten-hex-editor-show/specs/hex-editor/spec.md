# Hex Editor Capability — Rendering Architecture Delta

## ADDED Requirements

### Requirement: Hex Editor Render/Interact Separation
The hex editor SHALL separate per-row rendering from input handling. Rendering functions SHALL produce visual output and a description of detected input events; interaction handling SHALL apply those events to editor state afterward.

#### Scenario: Row rendering does not mutate editor state
- **WHEN** a visible row is rendered during a frame
- **THEN** the row-rendering function does not mutate `EditorState`
- **AND** it returns a `RowResult` describing any detected input events (cursor move, drag start, drag continue, context menu request)

#### Scenario: Interaction handling applies collected events in one pass
- **WHEN** all visible rows have been rendered and their `RowResult`s collected
- **THEN** a single interaction-handling function applies the collected events to editor and UI state
- **AND** no editor state mutation occurs inside the per-row render loop

#### Scenario: show() orchestrates but does not paint bytes directly
- **WHEN** `hex_editor::show()` executes
- **THEN** it delegates per-row rendering to the row-rendering function
- **AND** its body is an orchestrator (scroll-target resolution, viewport loop, interaction dispatch) rather than inline byte painting

### Requirement: Shared Byte Highlight Painter
The hex editor SHALL paint byte backgrounds (cursor, selection, search highlight, risk-level tint) through a single shared routine used by both the hex and ASCII columns.

#### Scenario: Selection visual parity across columns
- **WHEN** a byte is selected and both its hex and ASCII cells are visible
- **THEN** both cells display the same selection background, painted by the same routine

#### Scenario: Cursor visual parity across columns
- **WHEN** the cursor is on a byte and the cursor is visible in both columns
- **THEN** both cells display the same cursor indicator, painted by the same routine

#### Scenario: Search highlight and risk tint parity
- **WHEN** a byte is a search match or sits in a risk-classified file section
- **THEN** hex and ASCII cells receive the same highlight/tint, painted by the same routine

### Requirement: Named Struct for Edit Input Results
Functions that collect multiple optional results from hex editor input SHALL return a named struct rather than a tuple of options.

#### Scenario: Edit input returns a named result
- **WHEN** `handle_edit_input` processes a frame's keyboard input
- **THEN** it returns a named struct (e.g. `EditInputResult`) with explicitly named fields for each collected result
- **AND** the call site destructures by field name rather than by tuple position

### Requirement: Refactor Preserves Hex Editor Behavior
This refactor SHALL NOT change any user-visible hex editor behavior.

#### Scenario: Selection, scrolling, and edit behavior preserved
- **WHEN** the refactor is complete
- **THEN** scrolling, clicking, drag-selecting, shift-click, secondary-click, context menu, copy/paste (in hex and ASCII modes), overwrite/insert editing, search highlighting, risk tinting, non-selectable ASCII pipe borders, and alignment of an incomplete last row all behave identically to the pre-refactor baseline
