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

