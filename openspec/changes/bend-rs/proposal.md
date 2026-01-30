# Change: bend-rs - Cross-Platform Databending Application

## Why

Databending - the practice of manipulating raw image data to create glitch art - currently requires a tedious multi-tool workflow: copy the source image, open the copy in a hex editor, make edits, save, open in an image viewer, hope it didn't corrupt, repeat. There's no easy way to see changes in real-time, undo breaking edits, or preserve good intermediate states without manual file management. This friction discourages experimentation and makes the creative process unnecessarily painful.

## Business Value Analysis

### User Personas and Benefits

| Persona | Pain Today | Value Delivered | Frequency of Use |
|---------|-----------|-----------------|------------------|
| **Practicing Glitch Artist** | Juggles 3+ apps (file manager, hex editor, image viewer). Loses good intermediate states. Spends more time on workflow than art. | Single unified tool. Never lose work. 10x faster iteration. | Daily/Weekly |
| **Curious Beginner** | Intimidated by hex editors. No idea which bytes to edit. High risk of total file corruption discourages experimentation. | Format-aware guidance. Safe defaults. Learn by doing without fear. | Occasional, then converts to regular |
| **Creative Coder / Generative Artist** | Wants programmatic glitch effects but needs to understand manual process first. No good way to explore cause-and-effect. | Visual feedback loop for learning. Bookmarks document discoveries. | Project-based bursts |
| **Educator / Workshop Leader** | Difficult to demonstrate binary-to-visual relationship. Students break files and give up. | Live demo tool. Students can experiment safely. Comparison view shows cause/effect. | Workshops, courses |

### Problem Statement

**The core problem**: Databending is a legitimate art form with a 20+ year history, but the tooling hasn't evolved. Artists still use the same painful workflow from 2005: copy file, open in generic hex editor, save, check in separate viewer, repeat. This workflow:

1. **Destroys creative flow** - Context switching between apps breaks concentration
2. **Punishes experimentation** - One wrong edit can corrupt the file with no way back
3. **Wastes artist time** - Manual file management instead of making art
4. **Has a steep learning curve** - No guidance on file structure means beginners corrupt headers and give up

**Why now**: No dedicated databending tool exists. Generic hex editors don't understand images. Image editors don't expose raw bytes. This is an underserved niche with passionate practitioners.

### Feature Priority by Value

| Priority | Feature | Value Rationale |
|----------|---------|-----------------|
| **P0 - Essential** | Split view (hex + preview) | The entire value proposition. Without this, it's just another hex editor. |
| **P0 - Essential** | Live preview updates | Immediate feedback is the core workflow improvement. |
| **P0 - Essential** | Non-destructive editing | Eliminates the #1 fear (destroying the original file). |
| **P0 - Essential** | Undo/redo | Basic safety net. Users expect this. |
| **P1 - Important** | Save points | Enables branching experiments from known-good states. |
| **P1 - Important** | Format structure visualization | Guides users to "interesting" regions, reduces header corruption. |
| **P1 - Important** | Section highlighting | Visual learning aid, reduces accidental header edits. |
| **P1 - Important** | Graceful corruption handling | Keeps users in flow instead of showing cryptic errors. |
| **P2 - Valuable** | Comparison view | Helps users understand what changed. Educational value. |
| **P2 - Valuable** | Search/replace | Power user feature for targeted edits. |
| **P2 - Valuable** | Bookmarks/annotations | Supports learning and documentation of discoveries. |
| **P2 - Valuable** | Header protection toggle | Training wheels for beginners; power users can disable. |
| **P3 - Nice to Have** | High-risk edit warnings | Extra safety; some users will disable immediately. |

### What If We Don't Build This?

**For users**:
- Continue using fragmented, frustrating workflow
- Beginners continue to bounce off the learning curve
- The art form remains niche and inaccessible

**For the project**:
- Miss opportunity to own an underserved niche
- No dedicated databending tool exists - first mover advantage available

**Competitive landscape**:
- **Generic hex editors** (HxD, Hex Fiend, ImHex): No image preview, no format awareness for images
- **Image editors** (GIMP, Photoshop): No raw byte access
- **Glitch-specific tools** (mostly web-based): Pre-canned effects only, no manual byte editing
- **Gap**: Zero tools combine hex editing + live image preview + format awareness

### Success Metrics

| Metric | Target | How to Measure |
|--------|--------|----------------|
| **Core workflow works** | User can open image, edit bytes, see preview update, export result | Manual testing, user feedback |
| **Iteration speed** | < 1 second from edit to preview update | Performance profiling |
| **Learning curve** | Beginner can make first successful glitch in < 5 minutes | User testing with new users |
| **Stability** | < 1% crash rate during editing sessions | Crash reporting (if implemented) |
| **File safety** | 0 cases of original file corruption | Automated tests, user feedback |
| **Cross-platform** | Works on macOS, Windows, Linux | CI builds and manual testing |

### Adoption Signals (Post-Launch)

- GitHub stars and forks
- Community sharing of glitch art created with the tool
- Feature requests (indicates engaged users)
- Workshop/tutorial content created by others
- Mentions in glitch art communities (Reddit r/glitch_art, etc.)

## What We're Building

A cross-platform desktop application (built with Rust and egui) that provides:

### Core Features
- **Commander-style split view** - Hex editor on the left, live image preview on the right
- **Non-destructive editing** - Original file preserved; all edits tracked in memory
- **Explicit save points** - User-controlled checkpoints to preserve good states
- **Linear undo/redo** - Navigate edit history to recover from breaking changes
- **Live preview** - Image updates as you edit hex data (when still parseable)
- **Graceful corruption handling** - Shows last valid state or broken-image indicator
- **Comparison view** - Side-by-side original vs. current state
- **Bookmarks and annotations** - Mark and label interesting byte offsets
- **Search and replace** - Find byte patterns or ASCII strings, with replace capability

### Format Support (Initial)
- **BMP** - Simple header structure, predictable results
- **JPEG** - More complex, interesting compression artifacts

### Format Awareness
- **Structure visualization** - Sidebar showing file sections (header, metadata, pixel data)
- **Section highlighting** - Color-coded regions in hex view
- **"Jump to data" navigation** - Quick access to editable regions
- **Optional header locking** - Toggle "safe zones" to prevent total corruption

### Future Considerations (Not MVP)
- Session export/import
- Branching history (tree of states)
- Batch operations
- Randomization tools and presets
- Additional format support (PNG, etc.)
- Minimap visualization
- Heatmap overlay for recent changes

## Technical Approach

### Architecture
```
bend-rs/
├── src/
│   ├── main.rs              # Entry point, egui app setup
│   ├── app.rs               # Main application state
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── hex_editor.rs    # Left panel - hex view with editing
│   │   ├── image_preview.rs # Right panel - rendered image
│   │   ├── comparison.rs    # Side-by-side original vs. current
│   │   ├── structure_tree.rs# File structure sidebar
│   │   ├── bookmarks.rs     # Bookmarks list and management
│   │   ├── search.rs        # Search/replace dialog
│   │   └── toolbar.rs       # Actions, save points, etc.
│   ├── formats/
│   │   ├── mod.rs
│   │   ├── traits.rs        # ImageFormat trait
│   │   ├── bmp.rs           # BMP parser
│   │   └── jpeg.rs          # JPEG parser
│   ├── editor/
│   │   ├── mod.rs
│   │   ├── buffer.rs        # Byte buffer with edit tracking
│   │   ├── history.rs       # Undo/redo, save points
│   │   ├── bookmarks.rs     # Bookmark data and operations
│   │   ├── search.rs        # Search/replace logic
│   │   └── operations.rs    # Edit operations
│   └── session/
│       ├── mod.rs
│       └── export.rs        # Session save/load (future)
```

### Key Dependencies
- `eframe` / `egui` - Cross-platform GUI framework
- `image` - Image decoding with graceful error handling
- `rfd` - Native file dialogs
- `serde` - Session serialization

### Format Parsing Strategy
- Trait-based design (`ImageFormat`) for extensibility
- BMP: Parse fixed-size header to locate pixel data offset
- JPEG: Parse marker segments (SOI, APP0, DQT, SOF, DHT, SOS) to identify structure
- Each format provides: section boundaries, human-readable labels, safe/unsafe zones

### Performance Considerations
- Lazy hex rendering (virtual scrolling for large files)
- Debounced image re-rendering on edit
- Memory-efficient history (store diffs, not full copies)

## Impact

- **New application** - No existing code affected
- **Cross-platform** - macOS, Windows, Linux support via egui
- **Self-contained** - No external runtime dependencies for end users

## Design Decisions

1. **Maximum file size**: To be determined experimentally during development. We'll test with various file sizes and optimize as needed.
2. **Keyboard shortcuts**: Mirror common hex editor conventions (HxD, Hex Fiend) where they overlap. Custom shortcuts only where no convention exists.
3. **JPEG entropy-coded data**: Warn users when editing scan data (high corruption risk), but allow the edit. Users can dismiss and disable these warnings.

## Success Criteria

- User can open a BMP or JPEG file
- Hex data displayed on left, image preview on right
- Edits to hex data reflected in preview (or graceful error shown)
- Undo/redo works across edits
- Save points can be created and restored
- File structure visible and navigable
- Original file never modified; explicit "export" required to save changes
- Comparison view shows original vs. current side-by-side
- Bookmarks can be created, named, and navigated to
- Search finds byte patterns and ASCII strings; replace modifies matches
