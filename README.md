# Bend -- Databending Studio

A cross-platform desktop application for databending -- the art of deliberately manipulating raw binary data of image files to produce glitch art.

![Main View](screenshots/main-view.png)

Databending is a creative process where you edit the raw bytes of an image file to create intentional visual glitches and artifacts. The results depend on which bytes you modify and how the file format interprets the corrupted data. Traditionally, this meant juggling multiple copies of files across a generic hex editor and a separate image viewer, with no safety net if you went too far.

Bend combines a hex editor with a live image preview in a single application. It provides non-destructive editing, undo/redo, named save points, format-aware structure visualization, and header protection -- so you can experiment freely without fear of losing your work.

## Features

### Editing

- Hex editor with virtual scrolling (16 bytes per row, handles large files)
- Hex and ASCII editing modes (Ctrl/Cmd+M to toggle)
- Insert and Overwrite write modes (Ctrl/Cmd+I to toggle)
- Non-destructive dual-buffer architecture (original file data never modified)
- Undo/redo with automatic operation coalescing (adjacent edits merged)

### Preview

- Live image preview with debounced updates
- Animated GIF playback with play/pause and frame-by-frame controls
- Comparison view showing original and current images side-by-side
- Graceful corruption handling (last valid preview preserved when edits break decoding)

### Organization

- Named save points with incremental diffs (only changed bytes stored)
- Bookmarks with custom names and annotations
- Recent files list in the File menu

### Search

- Hex pattern search with wildcard support (e.g., `FF ?? FF`)
- ASCII string search with case sensitivity toggle
- Find and Replace with single and Replace All operations

### Format Awareness

- File structure visualization in a collapsible sidebar tree
- Color-coded risk levels for file sections (Safe, Caution, High, Critical)
- Header protection toggle to block edits to critical regions
- High-risk edit warnings with option to suppress

### Platform

- Cross-platform: macOS, Windows, Linux
- Dark, Light, and System theme options
- Persistent settings and window state across sessions
- Drag-and-drop file opening
- Native file dialogs

## Screenshots

![Comparison Mode](screenshots/comparison-mode.png)

![Structure Tree](screenshots/structure-tree.png)

![GIF Playback](screenshots/gif-playback.png)

![Search and Replace](screenshots/search-replace.png)

## Supported Formats

| Format | Extensions     | Notes                                                  |
|--------|----------------|--------------------------------------------------------|
| BMP    | .bmp           | Full structure parsing, beginner-friendly               |
| JPEG   | .jpg, .jpeg    | Marker segment parsing, scan data glitch effects        |
| GIF    | .gif           | Animated playback with frame controls                   |

## Building from Source

### Prerequisites

- Rust toolchain (edition 2021) -- install via [rustup](https://rustup.rs)

### Build

```sh
git clone https://github.com/your-username/bend-rs.git
cd bend-rs
cargo build --release
```

The compiled binary will be at `target/release/bend-rs`.

### Linux

On some Linux distributions, eframe/egui may require system packages for graphics and windowing support. See the [eframe documentation](https://github.com/emilk/egui/tree/master/crates/eframe) for platform-specific dependencies.

## Usage Guide

### Opening a File

Open a file with **File > Open** or **Ctrl/Cmd+O**. You can also drag and drop an image file onto the application window. Previously opened files appear in the **File > Recent Files** menu.

### The Interface

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

The left sidebar shows the file's structure tree, save points, and bookmarks. The center panel is the hex editor. The right panel shows the image preview. The status bar at the bottom displays cursor position, edit mode, and file information.

### Editing Bytes

Click any byte in the hex editor to position the cursor. In **Hex mode**, type `0-9` or `A-F` to edit nibble by nibble (two keystrokes per byte). In **ASCII mode**, type any printable character to replace the byte at the cursor. Toggle between modes with **Ctrl/Cmd+M**.

**Overwrite mode** replaces existing bytes. **Insert mode** shifts bytes right and grows the buffer. Toggle with **Ctrl/Cmd+I**. The current mode is shown in the status bar and toolbar.

Navigate with arrow keys, Page Up/Down (16 rows), and Home/End. Hold Shift to extend a selection.

### Save Points

Save points are the core safety feature for databending. After making edits you want to preserve, create a save point with **Ctrl/Cmd+S**. Each save point is named and timestamped, and only stores the byte-level differences from the previous state.

Restore any save point from the sidebar to return to that state. Save points can be renamed, deleted, and the restore operation itself is undoable.

### Exporting Your Work

The original file is never modified. To save your changes, use **File > Export** or **Ctrl/Cmd+E**. This opens a save dialog with a default filename of `<original>_glitched.<ext>`. You can export at any time and continue editing.

### Comparison View

Toggle comparison mode from the toolbar to see the original and current images side-by-side. Both images scale together. For animated GIFs, frame playback stays synchronized between the two views.

### Search and Replace

Open the search dialog with **Ctrl/Cmd+F**. Search for hex patterns (space-separated bytes, e.g., `FF D8 FF`) with `??` as a wildcard for any byte, or search for ASCII strings with an optional case sensitivity toggle.

Navigate matches with **Enter** (next) and **Shift+Enter** (previous), or use the Next/Previous buttons. Replace individual matches or use Replace All. Replace All respects header protection when enabled.

### Header Protection

Toggle the **Protect** button in the toolbar to prevent edits to format header and metadata regions. Protected bytes are displayed with strikethrough styling. This prevents accidental corruption of file structure that would make the image completely unreadable.

### Bookmarks

Add a bookmark at the cursor position with **Ctrl/Cmd+D**. Bookmarks appear in the sidebar with customizable names and annotations. Click a bookmark to jump to that offset. Useful for marking interesting locations in the file for repeated experimentation.

## Keyboard Shortcuts

A complete list is also available in the application via **F1** or **Help > Keyboard Shortcuts**.

### File Operations

| Shortcut               | Action      |
|------------------------|-------------|
| Ctrl+O / Cmd+O         | Open file   |
| Ctrl+E / Cmd+E         | Export file |

### Edit Operations

| Shortcut                     | Action                |
|------------------------------|-----------------------|
| Ctrl+Z / Cmd+Z              | Undo                  |
| Ctrl+Shift+Z / Cmd+Shift+Z  | Redo                  |
| Ctrl+Y / Cmd+Y              | Redo (alternative)    |
| Ctrl+F / Cmd+F              | Find & Replace        |
| Ctrl+G / Cmd+G              | Go to offset          |
| Ctrl+S / Cmd+S              | Create save point     |
| Ctrl+D / Cmd+D              | Add bookmark at cursor|
| Ctrl+R / Cmd+R              | Refresh preview       |

### Navigation

| Shortcut          | Action                  |
|-------------------|-------------------------|
| Arrow Keys        | Move cursor             |
| Page Up / Down    | Move cursor by 16 rows  |
| Home              | Go to start of file     |
| End               | Go to end of file       |

### Selection

| Shortcut              | Action                    |
|-----------------------|---------------------------|
| Shift + Arrow Keys    | Extend selection          |
| Shift + Page Up/Down  | Extend selection by 16 rows|
| Shift + Home          | Select to start           |
| Shift + End           | Select to end             |
| Shift + Click         | Select range              |

### Hex Editing

| Shortcut         | Action                                              |
|------------------|-----------------------------------------------------|
| 0-9, A-F         | Edit hex value at cursor (Hex mode)                  |
| Any printable char| Edit ASCII value at cursor (ASCII mode)             |
| Ctrl+M / Cmd+M   | Toggle between Hex and ASCII editing mode            |
| Ctrl+I / Cmd+I   | Toggle Insert/Overwrite mode                         |
| Backspace        | Delete byte before cursor (Insert) / Move left (Overwrite)|
| Delete           | Delete byte at cursor (Insert mode)                  |
| Right-click      | Context menu (copy, paste, bookmark)                 |

### View

| Shortcut | Action               |
|----------|----------------------|
| F1       | Show keyboard shortcuts|

## Configuration

Settings are stored as JSON at the platform-appropriate config directory:

| Platform | Path                                              |
|----------|---------------------------------------------------|
| macOS    | ~/Library/Application Support/bend-rs/settings.json|
| Windows  | %APPDATA%/bend-rs/settings.json                    |
| Linux    | ~/.config/bend-rs/settings.json                    |

Configurable options include window dimensions, recent files, header protection default, high-risk edit warning preference, and theme.

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).
