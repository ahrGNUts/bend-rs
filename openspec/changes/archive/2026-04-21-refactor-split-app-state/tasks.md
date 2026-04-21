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
- [x] 6.1 Moved `mark_preview_dirty`, `set_animation_frame`, `toggle_animation_playback`, `pause_animation` from `impl BendApp` onto `impl PreviewState` (they only touch `preview.*`). Narrowed 5 UI `show()` functions to take substates directly instead of `&mut BendApp`:
  - `image_preview::show(ui, &mut PreviewState, &AppColors)` — was `&mut BendApp`
  - `savepoints::show(ui, &mut DocumentState, &mut SavePointsPanelState)` — was `&mut BendApp`
  - `bookmarks::show(ui, &mut DocumentState, &mut UiState, &mut BookmarksPanelState)` — was `&mut BendApp`
  - `structure_tree::show(ui, &mut DocumentState, &mut UiState)` — was `&mut BendApp`
  - `go_to_offset_dialog::show(ctx, &mut DocumentState, &mut UiState)` — was `&mut BendApp`
  - `shortcuts_dialog::show(ctx, &mut ShortcutsDialogState)` — already narrow
  - `settings_dialog::show(ctx, &mut SettingsDialogState, &mut AppSettings)` — already narrow
- [x] 6.2 Two UI files — `search_dialog.rs` and `hex_editor.rs` — still take `&mut BendApp`. Both call BendApp-level cross-cutters (`navigate_to_search_match`, `refresh_search`, `section_color_for_offset`, `should_warn_for_edit`) that genuinely span three+ substates. Per design.md's "3+ substates OK" clause, these keep `&mut BendApp`. Added inline doc comments on each narrowed function noting the substates it touches.
- [x] 6.3 Partial: 7 of 9 UI `show()` functions no longer import `BendApp` (the two exceptions are documented in 6.2). Full elimination is deferred to the companion proposal `refactor-flatten-hex-editor-show` (which rewrites hex_editor internals) — at that point, search_dialog's remaining BendApp usage can be revisited without blocking this change.

## 7. Verification
- [x] 7.1 `cargo fmt` — clean (no diff)
- [x] 7.2 `cargo build --release` — succeeds (28.8s)
- [x] 7.3 `cargo test` — 204/204 pass
- [x] 7.4 `cargo clippy --all-targets` — no new warnings introduced by this refactor (the remaining warnings are pre-existing: `field_reassign_with_default` in test setup in `settings.rs` / `search.rs` / `preview.rs` tests, plus a pre-existing `collapsible_if` in `bookmarks.rs:162` and `needless_borrow` in `hex_editor.rs:589`)
- [x] 7.5 Manual smoke test passed — no user-visible regressions vs. pre-refactor baseline
