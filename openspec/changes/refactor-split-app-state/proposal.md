# Change: Split BendApp god struct into cohesive substates

## Why

`BendApp` in `src/app/mod.rs:73-138` holds 20 mixed-concern fields: editor state, UI panel state (savepoints_state, bookmarks_state, shortcuts_dialog_state, settings_dialog_state, context_menu_state), I/O plumbing (open_dialog_rx, export_dialog_rx, pending_open_path, pending_hex_scroll, window_resize_timer, last_window_size), rendering caches (colors, cached_sections), and persistent configuration (settings, header_protection).

The single struct forces every UI `show()` function to take `&mut BendApp`, which couples UI rendering to every subsystem. Adding features means adding fields; callers reach across concerns trivially; reasoning about lifetimes and mutation paths is hard. The project has grown organically and BendApp is now the bottleneck for further structural cleanups (e.g. #2: flatten `hex_editor::show()`).

## What Changes

- Introduce four cohesive substates inside `src/app/` and make `BendApp` a thin container that owns them:
  - `DocumentState` — `editor: Option<EditorState>`, `current_file: Option<PathBuf>`, `cached_sections: Option<Vec<FileSection>>`, `preview: PreviewState`, `header_protection: bool`.
  - `UiState` — `colors`, `dialogs`, `context_menu_state`, `search_state`, `go_to_offset_state`, `savepoints_state`, `bookmarks_state`, `shortcuts_dialog_state`, `settings_dialog_state`, `pending_hex_scroll`, `last_window_size`.
  - `IoState` — `open_dialog_rx`, `export_dialog_rx`, `pending_open_path`, `window_resize_timer`.
  - `AppConfig` — `settings: AppSettings`.
- `BendApp` becomes `{ doc: DocumentState, ui: UiState, io: IoState, config: AppConfig }`.
- Migrate every UI `show()` signature from `fn show(ui, app: &mut BendApp)` to accept only the substates it reads/mutates. For example:
  - `ui::search_dialog::show(ui, &mut ui_state.search_state, &mut doc.editor, &ui_state.colors, &config.settings)`.
  - `ui::hex_editor::show(ui, &mut doc, &mut ui_state, &config)`.
- Move `is_offset_protected`/`is_range_protected` (currently `BendApp` methods in `src/app/sections.rs:34-50`) onto `DocumentState` so they no longer require the full app.
- **Non-functional:** no user-visible behavior change. This is a pure internal refactor.
- **Breaking (internal only):** all `BendApp` field accesses in `src/app/*.rs` and `src/ui/*.rs` must go through the new substates.

## Impact

- **Affected specs:** `hex-editor` capability — adds an architectural requirement about state separation. No behavioral delta.
- **Affected code:**
  - `src/app/mod.rs` (BendApp definition, `new`, `update`, helper methods).
  - `src/app/preview.rs`, `src/app/menu_bar.rs`, `src/app/toolbar.rs`, `src/app/dialogs.rs`, `src/app/input.rs`, `src/app/sections.rs` — all take `&mut BendApp` today.
  - `src/ui/hex_editor.rs`, `src/ui/search_dialog.rs`, `src/ui/settings_dialog.rs`, `src/ui/shortcuts_dialog.rs`, `src/ui/structure_tree.rs`, `src/ui/bookmarks.rs`, `src/ui/savepoints.rs`, `src/ui/image_preview.rs`, `src/ui/go_to_offset_dialog.rs` — migrate to narrower signatures.
- **Enables:** #2 (`refactor-flatten-hex-editor-show`) lands more cleanly because `hex_editor::show` already won't need the full `BendApp`. Future work to extract a pure-logic `Editor` API that's testable without egui is unblocked.
- **Risk:** large mechanical patch (~15 files). Mitigated by making the split one field-group at a time with compile-check between each extraction.
