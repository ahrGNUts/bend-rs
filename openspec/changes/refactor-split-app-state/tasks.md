# Tasks: Split BendApp into cohesive substates

## 1. Define substates
- [ ] 1.1 Create `src/app/state.rs` with `DocumentState`, `UiState`, `IoState`, `AppConfig` structs (fields moved from `BendApp`)
- [ ] 1.2 Each substate gets `#[derive(Default)]` or an explicit `impl Default`
- [ ] 1.3 Re-export the four types from `src/app/mod.rs`

## 2. Extract `IoState` (smallest, most isolated)
- [ ] 2.1 Move `open_dialog_rx`, `export_dialog_rx`, `pending_open_path`, `window_resize_timer` off `BendApp` into `IoState`
- [ ] 2.2 Add `io: IoState` field to `BendApp`
- [ ] 2.3 Update callers in `src/app/mod.rs` (dialog polling at ~:505-527, `is_dialog_pending`), `src/app/dialogs.rs`, `src/app/menu_bar.rs`
- [ ] 2.4 `cargo build` + manual smoke test (open file via dialog)

## 3. Extract `AppConfig`
- [ ] 3.1 Move `settings: AppSettings` off `BendApp` into `AppConfig`
- [ ] 3.2 Add `config: AppConfig` field to `BendApp`
- [ ] 3.3 Update `BendApp::new()` to construct `AppConfig`
- [ ] 3.4 Update every read of `app.settings` → `app.config.settings`
- [ ] 3.5 `cargo build` + smoke test (settings dialog opens, theme persists)

## 4. Extract `UiState`
- [ ] 4.1 Move `colors`, `dialogs`, `context_menu_state`, `search_state`, `go_to_offset_state`, `savepoints_state`, `bookmarks_state`, `shortcuts_dialog_state`, `settings_dialog_state`, `pending_hex_scroll`, `last_window_size` into `UiState`
- [ ] 4.2 Add `ui: UiState` field to `BendApp`
- [ ] 4.3 Update every UI file: `src/ui/search_dialog.rs`, `src/ui/settings_dialog.rs`, `src/ui/shortcuts_dialog.rs`, `src/ui/structure_tree.rs`, `src/ui/bookmarks.rs`, `src/ui/savepoints.rs`, `src/ui/go_to_offset_dialog.rs`, `src/ui/image_preview.rs`
- [ ] 4.4 Update `src/app/input.rs`, `src/app/preview.rs`, `src/app/toolbar.rs`, `src/app/menu_bar.rs`
- [ ] 4.5 `cargo build` + smoke test (all dialogs open, search works, bookmarks panel works)

## 5. Extract `DocumentState`
- [ ] 5.1 Move `editor`, `current_file`, `cached_sections`, `preview`, `header_protection` into `DocumentState`
- [ ] 5.2 Add `doc: DocumentState` field to `BendApp`
- [ ] 5.3 Move `is_offset_protected` and `is_range_protected` from `BendApp` onto `DocumentState` (in `src/app/sections.rs`)
- [ ] 5.4 Update `src/app/input.rs`, `src/app/preview.rs`, `src/app/toolbar.rs`, `src/app/menu_bar.rs`
- [ ] 5.5 Update `src/ui/hex_editor.rs` to take `&mut DocumentState, &mut UiState, &AppConfig` instead of `&mut BendApp`
- [ ] 5.6 Update callers of `app.is_offset_protected` in `src/ui/search_dialog.rs:256` and elsewhere
- [ ] 5.7 `cargo build` + smoke test (hex editor renders, edits apply, header protection works)

## 6. Narrow UI signatures
- [ ] 6.1 For each UI `show()` function, replace `&mut BendApp` with the narrowest substate set it actually uses
- [ ] 6.2 Where a function needs three+ substates, document why in a one-line comment; do NOT re-bundle into a god struct
- [ ] 6.3 Verify no UI file imports `BendApp` (only substates)

## 7. Verification
- [ ] 7.1 `cargo fmt`
- [ ] 7.2 `cargo build --release`
- [ ] 7.3 `cargo test`
- [ ] 7.4 Manual full smoke test: open file, edit in hex + ASCII, undo/redo, create save point, restore save point, search + replace, drag-select, copy/paste, context menu, export, reopen settings, theme toggle
- [ ] 7.5 Verify no visible behavior change vs. `main` baseline
