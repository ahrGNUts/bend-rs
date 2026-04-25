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
