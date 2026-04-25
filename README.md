# Bend — Databending Studio

Bend is a cross-platform desktop app for **databending** — manipulating raw image bytes to create glitch art. It pairs a hex editor with a live image preview so you can see your edits as they happen, while keeping the original file safe and your edit history reversible.

> [!SCREENSHOT — hero]
> Capture the full application window with an interesting glitched image loaded. Both the hex editor (left) and image preview (right) should be visible, ideally with a few bytes selected and some visible corruption in the preview. This is the headline image for the project.

This application was inspired by a .NET Framework tool someone posted on Reddit many years ago. That tool was a good start at a glitch image studio, but it had some limitations and rough edges. Databending is inherently destructive, so the process can be a little risky — Bend's goal is to take the friction out of the workflow without taking away the chaos.

In the old days, glitching images meant making lots of copies of a file and opening each one in a hex editor. If you forgot to make a copy at the right moment, or got too daring with your edits, you could lose something you liked. Bend lets you create save points so you can experiment with confidence, tracks every edit so breaking changes can be undone, and shows your edits side-by-side with the original whenever you want.

This is nowhere near the only tool for glitching stuff, but I wanted to take a crack at it anyway. It's still being polished, and prebuilt binaries are coming.

Happy glitching!

## Table of Contents
- [Features](#features)
- [Supported Formats](#supported-formats)
- [Installation](#installation)
- [Usage Guide](#usage-guide)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Settings & Persistence](#settings--persistence)
- [Building from Source](#building-from-source)
- [License](#license)

## Features

- **Split view** — hex editor on the left, image preview on the right
- **Non-destructive editing** — the original file is never touched; you explicitly export to save changes
- **Undo / redo** — every edit can be reversed, with adjacent edits coalesced into single operations
- **Save points** — checkpoint your work and jump back to any previous state
- **Bookmarks & annotations** — mark interesting offsets and attach notes
- **Format-aware structure tree** — see and jump to BMP, JPEG, and GIF sections
- **Section highlighting** — color-coded byte regions in the hex view
- **Header protection** — toggle a "safe zone" that prevents edits to header bytes
- **High-risk warnings** — optional prompts before editing JPEG scan data and other risky regions
- **Comparison mode** — view the original and current image side-by-side
- **Animated GIF playback** — frame-by-frame controls in the preview pane
- **Search & replace** — find hex patterns (with `??` wildcards) or ASCII strings
- **Go to offset** — jump straight to any byte offset
- **HEX / ASCII edit modes** — type hex digits or ASCII characters directly into the buffer
- **Insert / overwrite modes** — choose whether typing replaces or grows the buffer
- **Recent files** — quick access to your last 10 opens
- **Theme** — Dark, Light, or follow the system

> [!SCREENSHOT — feature overview]
> Annotated screenshot of the full UI calling out the major regions: menu bar, toolbar, structure tree (left sidebar), hex editor (middle), image preview (right), save points / bookmarks panel. A simple labeled overlay (numbers + a legend) is fine.

## Supported Formats

Bend currently parses and previews:

- **BMP** — full header, DIB header, and pixel data sections
- **JPEG / JPG** — SOI, EOI, APP markers, DQT, DHT, SOF, SOS, and scan data
- **GIF** — including Logical Screen Descriptor, Global / Local Color Tables, extension blocks, image data, and **animated GIF playback**

Other formats may load if the underlying decoder accepts them, but structure visualization and section-level features are only available for the formats above.

## Installation

> [!NOTE]
> Prebuilt binaries are not yet published. For now, see [Building from Source](#building-from-source).

### Linux desktop integration

If you've built or installed the `bend-rs` binary into your `$PATH`, you can register the `.desktop` file and icon set so Bend appears in your application launcher:

```sh
./scripts/install-linux-desktop.sh
```

To remove the desktop entry later:

```sh
./scripts/uninstall-linux-desktop.sh
```

The installer drops files into `$XDG_DATA_HOME` (or `~/.local/share` if unset) and refreshes the GTK icon cache and desktop database where available. Tested on KDE Plasma (X11 and Wayland), GNOME (Wayland), Cinnamon, and XFCE.

## Usage Guide

### Opening a file

You can open an image in three ways:

1. **File → Open…** (or `Ctrl/Cmd+O`)
2. **Drag and drop** a file onto the application window
3. **File → Recent Files** to reopen a recent image

Only `.bmp`, `.jpg`, `.jpeg`, and `.gif` files are recognized when dropped or chosen via the dialog.

> [!SCREENSHOT — opening a file]
> The File menu open, showing Open…, Export…, Recent Files (with at least 2 entries to demonstrate the submenu), and Exit.

### Editing bytes

The hex editor has two columns: the hex bytes themselves and an ASCII rendering on the right. The cursor is shared between both columns — what you click in one is reflected in the other.

- **HEX mode** (default): type `0–9` and `A–F` to overwrite the byte under the cursor a nibble at a time
- **ASCII mode**: type any printable character to overwrite the byte under the cursor

Toggle modes with `Ctrl/Cmd+M`, or click the **HEX** / **ASCII** buttons in the toolbar.

- **Overwrite mode** (default): typing replaces the existing byte
- **Insert mode**: typing inserts a new byte at the cursor; `Backspace` and `Delete` remove bytes

Toggle insert/overwrite with `Ctrl/Cmd+I`.

> [!SCREENSHOT — hex editor with cursor and selection]
> Close-up of the hex editor showing: the cursor on a specific byte, a multi-byte selection (Shift+drag or Shift+arrow), and the ASCII column reflecting the same selection. The structure tree color-coded sections in the background should be visible.

### Refreshing the preview

Bend uses an explicit **refresh** model rather than re-rendering on every keystroke — that lets you make a series of edits without each one fighting the decoder. When the buffer differs from what the preview is showing, an "unsaved changes" indicator appears.

To update the preview:

- Click the **Refresh** button in the toolbar, or
- Press `Ctrl/Cmd+R`, or
- Choose **Edit → Refresh Preview**

If the new bytes can't be decoded, Bend keeps showing the last successfully decoded image with a stale-preview indicator and surfaces a decode error in the status area, so a single bad edit never wipes your view.

> [!SCREENSHOT — refresh preview workflow]
> Two-shot composite (or two side-by-side captures): first showing the unsaved-changes indicator after several edits, second showing the refreshed preview with visible glitch artifacts.

### Save points

Save points are named, in-memory snapshots you can return to at any time. They're great for branching: get the file into a state you like, save a point, then keep glitching — if you go too far, jump back.

- **Create a save point**: `Ctrl/Cmd+S` or **Edit → Create Save Point**
- **Restore** a save point from the sidebar — restoring is itself undoable
- **Rename** or **delete** save points from the sidebar list

> [!SCREENSHOT — save points panel]
> Left sidebar showing the Save Points list with at least 3 named save points (e.g., "Initial," "First glitch," "Header swapped"). Show the Restore / Rename / Delete buttons on one entry.

### Bookmarks

Bookmarks mark a specific byte offset with a name and an optional note — useful for documenting "the byte that controls X" while you're exploring a format.

- **Add bookmark at cursor**: `Ctrl/Cmd+D` or **Edit → Add Bookmark**
- **Click** a bookmark in the sidebar to jump to its offset
- **Add Note** / **Edit Note** / **Delete Note** to document what each bookmark does
- Bookmarked bytes are highlighted in the hex view

> [!SCREENSHOT — bookmarks panel and hex view]
> Sidebar with 2-3 bookmarks (one with a note visible) and the corresponding bookmarked bytes highlighted in the hex view.

### Search & replace

Open with `Ctrl/Cmd+F` or **Edit → Find & Replace…**.

- **Hex mode**: search for byte patterns like `FF D8 FF`. Use `??` as a wildcard for any single byte.
- **ASCII mode**: search for text strings, with an optional case-sensitive toggle.
- **Enter** jumps to the next match; **Shift+Enter** jumps to the previous match.
- **Replace** rewrites the current match; **Replace All** is a single atomic undo operation.
- When **Protect Headers** is enabled, replaces inside protected regions are skipped and reported in the status line.

> [!SCREENSHOT — search dialog]
> The Search & Replace dialog with a hex pattern containing a wildcard (e.g., `FF ?? FF`) and a couple of matches highlighted in the hex view behind the dialog.

### Structure tree

The structure tree on the left shows the parsed sections of the file (header, color table, pixel data, scan data, etc.). Click a section to scroll the hex view to that offset. The currently-active section based on cursor position is highlighted automatically.

> [!SCREENSHOT — structure tree]
> Structure tree expanded for a JPEG, showing SOI, APP0, DQT, SOF, DHT, SOS / scan data branches. The hex view should reflect a selection inside the SOS region with that branch highlighted as the active section.

### Header protection

Toggle **Protect** in the toolbar (or **Edit → Protect Headers**) to prevent edits to header / metadata regions. Protected bytes render with a strikethrough so you can still see and select them, and edit attempts inside protected regions are silently blocked. This is a per-session, per-file setting; the default state is configurable in Preferences.

> [!SCREENSHOT — header protection]
> Hex view with Protect mode on, showing strikethrough-styled bytes in the header region and the toolbar's Protect button highlighted as active.

### Comparison mode

Toggle **Compare** in the toolbar to see the original and current image side-by-side. Both images scale together. For animated GIFs, frame indices are synced and clamped to the smaller frame count if the two differ.

> [!SCREENSHOT — comparison mode]
> Two-pane preview with the original (clean) image on one side and a glitched version on the other, with "Original" and "Current" labels visible.

### Animated GIF playback

When you load a multi-frame GIF, the preview panel shows playback controls: play/pause, step forward/back, jump to first/last frame, and a frame counter. Stepping pauses playback automatically. Re-decode of an edited GIF preserves the current frame index and play state when possible.

> [!SCREENSHOT — animated GIF preview]
> Preview panel showing an animated GIF with playback controls visible and the frame counter (e.g., "Frame 5 / 24").

### Exporting

When you're happy with the result, **File → Export…** (or `Ctrl/Cmd+E`) writes the current buffer to a file you choose. The original file is never modified — Bend always writes to a destination you pick. An unsaved-changes indicator in the title bar (and a confirmation prompt on close) help you avoid losing work.

## Keyboard Shortcuts

The full list is also available in the app via **Help → Keyboard Shortcuts** (or press `F1`).

| Category | Shortcut | Action |
|---|---|---|
| **File** | `Ctrl/Cmd+O` | Open file |
| | `Ctrl/Cmd+E` | Export file |
| **Edit** | `Ctrl/Cmd+Z` | Undo |
| | `Ctrl/Cmd+Shift+Z` | Redo |
| | `Ctrl/Cmd+Y` | Redo (alternative) |
| | `Ctrl/Cmd+F` | Find & Replace |
| | `Ctrl/Cmd+G` | Go to offset |
| | `Ctrl/Cmd+S` | Create save point |
| | `Ctrl/Cmd+D` | Add bookmark at cursor |
| | `Ctrl/Cmd+R` | Refresh preview |
| **Navigation** | Arrow keys | Move cursor |
| | Page Up / Page Down | Move cursor by 16 rows |
| | Home / End | Go to start / end of file |
| **Selection** | Shift + Arrow keys | Extend selection |
| | Shift + Page Up / Down | Extend selection by 16 rows |
| | Shift + Home / End | Extend selection to start / end |
| | Shift + Click | Select range |
| **Editing** | `0-9`, `A-F` | Edit hex value (HEX mode) |
| | Any printable char | Edit byte (ASCII mode) |
| | `Ctrl/Cmd+M` | Toggle HEX / ASCII edit mode |
| | `Ctrl/Cmd+I` | Toggle insert / overwrite mode |
| | Backspace | Delete byte before cursor (Insert) / move left (Overwrite) |
| | Delete | Delete byte at cursor (Insert mode) |
| | Right-click | Context menu (copy, paste, bookmark, go to offset) |
| **Help** | `F1` | Show keyboard shortcuts dialog |

> [!SCREENSHOT — keyboard shortcuts dialog]
> The Keyboard Shortcuts dialog open and scrolled to show several sections (File Operations, Edit Operations, Navigation, etc.).

## Settings & Persistence

Bend remembers your window size, recent files, header-protection default, high-risk-warning preference, and theme between launches. The settings file lives at:

- **macOS**: `~/Library/Application Support/bend-rs/settings.json`
- **Windows**: `%APPDATA%/bend-rs/settings.json`
- **Linux**: `~/.config/bend-rs/settings.json`

Open **Edit → Preferences…** to change settings interactively (theme, default header protection, high-risk warnings). Changes are saved automatically.

> [!SCREENSHOT — preferences dialog]
> The Preferences dialog open, showing the Theme selector (Dark / Light / System), default header protection checkbox, and high-risk warnings toggle.

## Building from Source

Bend is built with Rust. You'll need a recent stable Rust toolchain (install via [rustup](https://rustup.rs/) if you don't have one).

```sh
git clone https://github.com/<your-username>/bend-rs.git
cd bend-rs
cargo build --release
```

The resulting binary will be at `target/release/bend-rs`. To run from source:

```sh
cargo run --release
```

### Platform notes

- **macOS**: tested and working.
- **Windows**: a Windows resource file (icon) is compiled into the binary automatically via `build.rs`.
- **Linux**: the binary works on the desktop environments listed above. Use `scripts/install-linux-desktop.sh` to add a launcher entry and icons.

### Running tests

```sh
cargo test
```

## License

Bend is licensed under the **GNU GPL v3.0**. See [LICENSE](LICENSE) for the full text.
