# Format Awareness Capability

## ADDED Requirements

### Requirement: File Structure Parsing
The application SHALL parse BMP and JPEG files to identify structural sections (headers, metadata, image data).

#### Scenario: Parse BMP structure
- **WHEN** a BMP file is loaded
- **THEN** the application identifies: file header, DIB header, optional color table, and pixel data regions
- **AND** each section's byte offset and size are determined

#### Scenario: Parse JPEG structure
- **WHEN** a JPEG file is loaded
- **THEN** the application identifies marker segments: SOI, APP markers, DQT, SOF, DHT, SOS, and image data
- **AND** each segment's byte offset and size are determined

#### Scenario: Handle malformed files
- **WHEN** a file cannot be fully parsed due to corruption or non-standard structure
- **THEN** the application parses what it can and marks unparseable regions as "unknown"
- **AND** editing is still permitted

### Requirement: Structure Visualization
The application SHALL display a navigable tree or list of file sections in a sidebar.

#### Scenario: View structure tree
- **WHEN** a file is loaded and parsed
- **THEN** a sidebar displays the file structure as a collapsible tree
- **AND** each section shows its name, offset, and size

#### Scenario: Navigate via structure tree
- **WHEN** user clicks on a section in the structure tree
- **THEN** the hex editor scrolls to display that section's starting offset

### Requirement: Section Highlighting
The application SHALL visually distinguish different file sections in the hex editor view.

#### Scenario: Color-coded sections
- **WHEN** a file with known structure is displayed
- **THEN** different sections are highlighted with distinct background colors
- **AND** a legend or tooltip explains the color coding

### Requirement: Header Protection Toggle
The application SHALL provide an optional toggle to prevent editing of header/metadata sections.

#### Scenario: Enable header protection
- **WHEN** user enables "Protect Headers" for the current file
- **THEN** bytes in header/metadata sections become read-only
- **AND** attempts to edit protected bytes are blocked with visual feedback

#### Scenario: Disable header protection
- **WHEN** user disables "Protect Headers"
- **THEN** all bytes become editable
- **AND** a warning may be shown about corruption risk

#### Scenario: Per-file setting
- **WHEN** user toggles header protection
- **THEN** the setting applies only to the current file session
- **AND** does not affect other open files or persist across sessions

### Requirement: High-Risk Edit Warnings
The application SHALL warn users when editing regions with high corruption risk (e.g., JPEG entropy-coded scan data).

#### Scenario: Warn on risky edit
- **WHEN** user attempts to edit bytes in a high-risk region (e.g., JPEG scan data)
- **THEN** a warning dialog is displayed explaining the corruption risk
- **AND** the user can proceed with the edit or cancel

#### Scenario: Dismiss warning permanently
- **WHEN** user dismisses a high-risk warning and selects "Don't show again"
- **THEN** future edits to high-risk regions proceed without warning
- **AND** the preference persists for the session

#### Scenario: Re-enable warnings
- **WHEN** user re-enables high-risk warnings via settings or preferences
- **THEN** warnings are shown again for high-risk edits
