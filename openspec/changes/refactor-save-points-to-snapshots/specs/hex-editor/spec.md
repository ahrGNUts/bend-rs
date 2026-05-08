# Hex Editor Capability — Save Points Delta

## ADDED Requirements

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
