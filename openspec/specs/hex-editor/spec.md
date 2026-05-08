# hex-editor Specification

## Purpose
TBD - created by archiving change refactor-split-app-state. Update Purpose after archive.
## Requirements
### Requirement: Application State Separation
The application SHALL organize top-level state into cohesive substates grouped by concern (document, UI, I/O, configuration) rather than a single monolithic struct. UI rendering functions SHALL NOT require a reference to the full application state.

#### Scenario: UI rendering function accepts only required substates
- **WHEN** a UI rendering function in `src/ui/` is called
- **THEN** it receives only the substates whose fields it reads or mutates
- **AND** it does not receive a reference to the full `BendApp`

#### Scenario: Substates are accessed through named fields
- **WHEN** a function needs document, UI, I/O, or configuration state
- **THEN** it accesses them through named substate fields on `BendApp` (e.g. `app.doc`, `app.ui`, `app.io`, `app.config`)
- **AND** no top-level field of `BendApp` exists outside those substates

#### Scenario: Document-level predicates live on the document
- **WHEN** a caller queries whether a byte offset or range is protected
- **THEN** the predicate is called on `DocumentState` rather than `BendApp`
- **AND** no UI file depends on methods defined on `BendApp` for protection checks

### Requirement: Refactor Preserves User-Visible Behavior
The state-separation refactor SHALL NOT change any user-visible behavior, on-disk settings schema, file format support, or keyboard shortcut.

#### Scenario: Behavior parity after refactor
- **WHEN** the refactor is complete
- **THEN** opening, editing, undoing, save points, search/replace, drag-select, copy/paste, header protection, export, and settings persistence all behave identically to the pre-refactor baseline

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

### Requirement: Shared Highlight Helpers
The hex editor SHALL share the cursor color palette decision and the non-cursor background priority chain between the hex and ASCII column renderers, rather than duplicating those decisions inline in each renderer.

Out of scope: full visual parity between columns (e.g. painting search-match and risk-level tints in the ASCII column). The hex column today paints a richer set of highlights than the ASCII column; this refactor preserves that asymmetry. Aligning the column visuals would be a behavior change tracked separately.

#### Scenario: Cursor color palette is shared
- **WHEN** the hex column or the ASCII column needs the bright/dim cursor color pair for the current write mode
- **THEN** both renderers obtain it from the same helper rather than duplicating the insert-vs-overwrite branch

#### Scenario: Non-cursor background priority is shared
- **WHEN** a byte has any non-cursor highlight (selection, current-match, search-match, bookmark, or section tint)
- **THEN** the priority chain that picks the winning background color is computed by a single helper used by the hex column renderer (and available for any future renderer that needs the same decision)

#### Scenario: Selection and cursor are visually present in both columns
- **WHEN** a byte is selected or under the cursor
- **THEN** both its hex cell and its ASCII cell display the corresponding indicator (preserving pre-refactor behavior)

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

### Requirement: Save Points
The application SHALL allow users to create explicit save points (snapshots) that can be restored. Save points SHALL be self-contained snapshots of the working buffer at the moment of creation, stored independently of one another. Save points SHALL persist across all subsequent edits to the working buffer, including length-changing operations (insert and delete) and undo/redo of those operations. Any save point MAY be deleted at any time, regardless of its position in the list.

#### Scenario: Create save point
- **WHEN** user triggers "Create Save Point"
- **THEN** the current buffer state is stored as a named, self-contained snapshot
- **AND** the save point appears in a list of available restore points

#### Scenario: Save points persist across in-place byte edits
- **WHEN** the user edits bytes in overwrite mode after creating a save point
- **THEN** the save point remains in the list
- **AND** the save point remains restorable to its captured state

#### Scenario: Save points persist across length-changing edits
- **WHEN** the user inserts or deletes bytes after creating a save point, whether through Insert-mode typing, Backspace, Delete, paste in Insert mode, or any other length-changing operation
- **THEN** the save point remains in the list
- **AND** the save point remains restorable to its captured state and length

#### Scenario: Save points persist across undo and redo
- **WHEN** the user undoes or redoes any operation, including length-changing operations
- **THEN** all save points remain in the list
- **AND** all save points remain restorable to their captured states

#### Scenario: Restore save point with matching length
- **WHEN** user selects a save point to restore and the working buffer length equals the snapshot length
- **THEN** the working buffer contents are replaced with the snapshot contents
- **AND** the restore is recorded as a single undoable history entry

#### Scenario: Restore save point with different length
- **WHEN** user selects a save point to restore and the working buffer length differs from the snapshot length
- **THEN** the working buffer is replaced entirely with the snapshot, including its length
- **AND** the cursor is clamped to a valid offset within the new buffer
- **AND** the restore is recorded as a single undoable history entry that round-trips correctly through undo and redo

#### Scenario: Delete any save point
- **WHEN** user deletes a save point at any position in the list
- **THEN** the deleted save point is removed
- **AND** all other save points remain in their captured states
- **AND** all other save points remain restorable

