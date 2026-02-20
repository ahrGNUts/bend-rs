# Project Context

## Purpose

Bend is a cross-platform desktop application for **databending** -- the art of deliberately manipulating raw binary data of image files to produce glitch art effects. It replaces the tedious manual workflow of copying files, opening them in a generic hex editor, and viewing results in a separate image viewer. Bend provides a unified hex editor + live image preview with non-destructive editing, undo/redo, format awareness, and safety features that encourage creative experimentation without fear of data loss.

**Target Users:**
- Practicing glitch artists seeking faster iteration
- Curious beginners intimidated by raw hex editors
- Creative coders/generative artists learning the manual process
- Educators demonstrating binary-to-visual relationships

## Tech Stack

- **Language:** Rust (Edition 2021)
- **GUI Framework:** egui 0.29 + eframe 0.29 (immediate-mode, cross-platform)
- **Image Decoding:** image 0.25 (BMP, JPEG, PNG, ICO)
- **Native File Dialogs:** rfd 0.15
- **Serialization:** serde 1.0 + serde_json 1.0
- **Clipboard:** arboard 3.4
- **Platform Directories:** dirs 5.0
- **Logging:** log 0.4 + env_logger 0.11
- **Build (Windows):** winresource 0.1 (icon embedding)

## Project Conventions

### Code Style

- Run `cargo fmt` before every commit
- Standard Rust naming: `snake_case` for functions/variables, `PascalCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants
- Modules organized as directory modules (`mod.rs` pattern) when they contain multiple submodules
- Inline `#[cfg(test)]` modules preferred over separate test files
- Avoid over-engineering: prefer direct, minimal solutions; no premature abstractions

### Architecture Patterns

**Key Architectural Decisions:**

1. **Dual-Buffer Design:** `original` buffer (immutable after load) + `working` buffer (all edits). Ensures non-destructive editing with comparison capability.
2. **Immediate-Mode GUI:** egui renders the entire UI each frame; state lives in `BendApp`.
3. **Virtual Scrolling:** Hex editor renders only visible rows + small buffer for performance with large files.
4. **Operation Coalescing:** Adjacent single-byte edits within 500ms are merged into one undo operation.
5. **Incremental Save Point Diffs:** Save points store only byte-level changes from previous state, not full copies.
6. **Trait-Based Format Support:** `ImageFormat` trait allows extensible format parsers (currently BMP and JPEG).
7. **Risk-Level Coloring:** File sections are classified as Safe/Caution/High/Critical/Unknown and color-coded in the hex view.
8. **Debounced Preview Updates:** Image preview re-renders after a short delay to avoid lag during rapid editing.

### Testing Strategy

- Unit tests embedded in source files using `#[cfg(test)]` modules
- Tests cover: file extension validation, settings save/load, format parsing (BMP, JPEG), gap filling, search functionality, edit operations, printable ASCII detection, unsupported format handling, settings sync
- All tests must pass before committing (`cargo test`)
- Run `cargo fmt` before staging

### Git Workflow

- Single `main` branch for primary development
- Commit messages should reference the change name and task when working from OpenSpec tasks
- Never commit failing tests
- Always run `cargo fmt` before staging files
- OpenSpec-driven development: proposals in `openspec/changes/`, specs in `openspec/specs/`

## Domain Context

### What is Databending?

Databending is a creative art form where artists deliberately edit the raw binary data of image files to produce glitch effects. The visual artifacts depend on which bytes are modified and the file format's structure.

### Key Concepts

| Term | Definition |
|------|-----------|
| **Databending** | Art of editing raw binary data to create intentional glitch effects |
| **Nibble** | Half a byte (4 bits); hex digits are edited one nibble at a time |
| **Hex Editing** | Viewing/modifying file bytes as hexadecimal values |
| **Glitch Art** | Visual corruption artifacts created intentionally for aesthetic effect |
| **Non-Destructive Editing** | Original file data never modified; changes apply to a working copy only, explicit "Export" required to save |
| **Save Point** | Named snapshot of working buffer state; restorable and undo-able |
| **Risk Level** | Classification of file sections by edit danger: Safe (green), Caution (yellow), High (orange), Critical (red), Unknown (gray) |
| **Header Protection** | Toggle that blocks edits to critical/high-risk file sections |
| **Write Mode** | Overwrite (replace bytes) vs. Insert (shift bytes right, grow buffer) |
| **Edit Mode** | Hex (nibble-level, 0-9/A-F) vs. ASCII (character-level) editing |
| **Entropy-Coded Data** | JPEG compressed data; editing produces interesting visual artifacts |

### Supported File Formats

**BMP (Bitmap):** Simple structure -- 14-byte file header + DIB header + optional color table + pixel data. Predictable and beginner-friendly.

**JPEG:** Marker-based structure -- SOI (FF D8) + marker segments (APP, DQT, SOF, DHT, SOS) + entropy-coded data + EOI (FF D9). Editing entropy-coded data produces interesting glitch effects.

### UI Layout

```
+---------------------------------------------------+
|              Menu Bar (File/Edit/Help)             |
+---------------------------------------------------+
|                 Toolbar (Buttons)                  |
+-------------+------------------+------------------+
|             |                  |                  |
|  Sidebar    |    Hex Editor    |  Image Preview   |
| (Structure, |  (16 bytes/row,  | (Live rendering  |
|  SavePoints,|   virtual scroll)|  of working      |
|  Bookmarks) |                  |  buffer)         |
|             |                  |                  |
+-------------+------------------+------------------+
|       Status Bar (Cursor, Mode, File Info)        |
+---------------------------------------------------+
```

## Important Constraints

- **Non-destructive by design:** The original buffer must never be modified after load. All edits go to the working buffer.
- **Export-only save:** Changes are written to a new file only on explicit user action (Export), never overwriting the source.
- **Cross-platform:** Must work on macOS, Windows, and Linux. Platform-specific paths handled via `dirs` crate.
- **Performance:** Virtual scrolling in hex editor for large files. Debounced preview updates. Operation coalescing for undo history. Undo stack capped at 1000 operations.
- **Safety features:** Header protection toggle, risk-level warnings for high/critical sections, close confirmation on unsaved changes.
- **No CI/CD yet:** No GitHub Actions workflows configured. Testing is manual (`cargo test`).

## External Dependencies

- **egui/eframe:** Cross-platform immediate-mode GUI framework. No external runtime required.
- **image crate:** Used only for decoding (BMP, JPEG, PNG, ICO) to render previews, not for encoding.
- **rfd:** Native OS file dialogs (open/save).
- **dirs:** Platform-appropriate config directory resolution (macOS: `~/Library/Application Support/bend-rs/`, Windows: `%APPDATA%/bend-rs/`, Linux: `~/.config/bend-rs/`).
- **arboard:** System clipboard access for copy/paste.
- No external services, APIs, or network calls. Fully offline desktop application.
