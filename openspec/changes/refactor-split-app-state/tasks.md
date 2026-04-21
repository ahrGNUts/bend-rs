# Tasks: Split BendApp into cohesive substates

## 1. Define substates
- [x] 1.1 Create `src/app/state.rs` (substates added incrementally per tasks 2–5 rather than all at once, so unused-field warnings don't accumulate)
- [x] 1.2 Each substate gets `#[derive(Default)]`
- [x] 1.3 Re-export from `src/app/mod.rs` as substates land (`pub use state::IoState;` done)

## 2. Extract `IoState` (smallest, most isolated)
- [x] 2.1 Move `open_dialog_rx`, `export_dialog_rx`, `pending_open_path`, `window_resize_timer`, `last_window_size` off `BendApp` into `IoState`; moved `FileDialogResult` enum alongside it.
- [x] 2.2 Add `io: IoState` field to `BendApp`
- [x] 2.3 Update callers in `src/app/mod.rs` (dialog polling, resize debounce, `is_dialog_pending` → `IoState::is_dialog_pending`) and `src/app/menu_bar.rs:113`. `src/app/dialogs.rs` did not touch these fields.
- [x] 2.4 `cargo build` + `cargo test` (204 tests pass)

## 3. Extract `AppConfig`
- [x] 3.1 Move `settings: AppSettings` off `BendApp` into `AppConfig`
- [x] 3.2 Add `config: AppConfig` field to `BendApp`
- [x] 3.3 Update `BendApp::new()` to construct `AppConfig`
- [x] 3.4 Update every read of `self.settings` → `self.config.settings` across `src/app/mod.rs`, `src/app/menu_bar.rs`, `src/app/dialogs.rs` (also the `test_settings_sync_suppress_warnings` test)
- [x] 3.5 `cargo build` + `cargo test` (204 tests pass)

## 4. Extract `UiState`
- [x] 4.1 Move `colors`, `dialogs`, `context_menu_state`, `search_state`, `go_to_offset_state`, `savepoints_state`, `bookmarks_state`, `shortcuts_dialog_state`, `settings_dialog_state`, `pending_hex_scroll` into `UiState`. `last_window_size` stayed in `IoState` alongside `window_resize_timer` — they implement the same debounce and split better there (design doc updated).
- [x] 4.2 Add `ui: UiState` field to `BendApp`
- [x] 4.3 Update UI files: `src/ui/hex_editor.rs`, `src/ui/search_dialog.rs`, `src/ui/structure_tree.rs`, `src/ui/bookmarks.rs`, `src/ui/go_to_offset_dialog.rs`, `src/ui/image_preview.rs`. `src/ui/settings_dialog.rs`, `shortcuts_dialog.rs`, `savepoints.rs` take dialog-state directly, not through `BendApp`.
- [x] 4.4 Update `src/app/input.rs`, `src/app/toolbar.rs`, `src/app/menu_bar.rs`, `src/app/sections.rs` (+ tests), `src/app/dialogs.rs`. `src/app/preview.rs` did not reference moved fields.
- [x] 4.5 `cargo build` + `cargo test` (204 tests pass)

## 5. Extract `DocumentState`
- [x] 5.1 Move `editor`, `current_file`, `cached_sections`, `preview`, `header_protection` into `DocumentState` in `src/app/state.rs`
- [x] 5.2 Add `doc: DocumentState` field to `BendApp`; remove the 5 fields from `BendApp`
- [x] 5.3 Move pure helpers (`section_at_offset`, `is_offset_protected`, `is_range_protected`, `get_high_risk_level`) onto `DocumentState`. `section_color_for_offset` and `should_warn_for_edit` remain on `BendApp` because they combine `doc` with `ui.colors`/`ui.dialogs`.
- [x] 5.4 Update app-layer: `src/app/mod.rs`, `src/app/dialogs.rs`, `src/app/input.rs`, `src/app/preview.rs`, `src/app/toolbar.rs`, `src/app/menu_bar.rs`, `src/app/sections.rs`
- [x] 5.5 Deferred to Task 6 — `hex_editor::show` still takes `&mut BendApp`; this task only moves field locations and updates accesses
- [x] 5.6 Update `app.is_offset_protected` / `app.is_range_protected` / `app.get_high_risk_level` / `app.section_at_offset` → `app.doc.<method>` across `src/ui/hex_editor.rs`, `src/ui/search_dialog.rs` (including tests), plus test modules in `src/app/mod.rs` and `src/app/sections.rs`
- [x] 5.7 `cargo build` + `cargo test` (204 tests pass)

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
