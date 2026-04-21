# Tasks: Flatten hex_editor::show()

## 1. Extract shared byte-highlight helpers
Scope note: the real duplication between hex and ASCII rendering is narrower than the original tasks.md claimed. Today the hex column paints cursor/selection/current_match/search_match/bookmark/section_bg/protected; the ASCII column paints only cursor/selection. Full parity (search/risk tints in ASCII too) would be a user-visible behavior change and is intentionally out of scope for this refactor. Instead we extract the genuinely duplicated pieces.

- [x] 1.1 Use the existing `ByteHighlight` struct rather than introducing a parallel `ByteHighlightFlags` (they would be identical).
- [x] 1.2 Extract `cursor_color_pair(write_mode, colors) -> (bright, dim)` — the insert-vs-overwrite tuple destructuring duplicated in both renderers.
- [x] 1.3 Extract `byte_background_color(highlight, colors) -> Option<Color32>` for the priority chain (selection > current_match > search_match > bookmark > section). Used in `render_hex_byte`; available for future ASCII use.
- [x] 1.4 Wire both helpers into `render_hex_byte` and `render_ascii_row`.
- [x] 1.5 Visual parity preserved (same colors, same painting order).
- [x] 1.6 `cargo build` + `cargo test` (204 tests pass)

## 2. Introduce RowResult and pure row rendering
- [x] 2.1 Defined `RowResult { cursor_move, start_drag, drag_current_offset, context_menu_offset }` with `Default` and `merge()` (last-value-wins — matches the original loop's plain-assignment semantics)
- [x] 2.2 Added `PointerContext { pointer_pos, primary_down, drag_active }` so `render_row` sees a consistent snapshot
- [x] 2.3 Wrote `render_row(ui, row_idx, state, editor, colors, highlights, pointer, scroll_to_me) -> RowResult` containing the hex-column rendering
- [x] 2.4 Same function renders the ASCII column (pipes, padding for incomplete last rows, pointer-to-byte mapping)
- [x] 2.5 `render_row` takes `&EditorState` (immutable) — no editor mutations in the render path; events are collected into `RowResult`
- [x] 2.6 `cargo build` + `cargo test` pass

## 3. Extract interaction handler
- [x] 3.1 Wrote `handle_row_interactions(ui, app, result, shift_held, drag_id, primary_down)` (takes `&mut BendApp` because it mutates both `doc.editor` and `ui.context_menu_state`; threading those as narrow substate refs is a job for a follow-up once we decide whether drag_id management belongs in `UiState`)
- [x] 3.2 Moved cursor-move, drag-start, drag-extend, drag-release, and context-menu logic out of the `show()` closure body
- [x] 3.3 Drag-select behavior (commit 2cef50a) preserved — `drag_current_offset` is still set from both columns using press_origin for ASCII
- [x] 3.4 Edit-mode-aware copy/paste (commit 2cef50a) preserved — unchanged; lives in `handle_keyboard_input`
- [x] 3.5 Non-selectable ASCII pipe borders (commit 6b4fdaf) preserved — `render_row` emits the same bracketing labels with `.selectable(false)`
- [x] 3.6 ASCII alignment on incomplete last row (commit 37e8e65) preserved — `render_row` still pads hex with transparent labels when `row_bytes.len() < BYTES_PER_ROW`

## 4. Shrink show() to orchestrator
- [x] 4.1 `show()` is now: prepare state → snapshot pointer → compute scroll target → ScrollArea → loop `render_row` folding results via `RowResult::merge` → `handle_row_interactions` → keyboard input → context menu
- [x] 4.2 `show()` is 86 lines (target ≤100)
- [x] 4.3 Replaced `handle_edit_input`'s `(Option<_>, Option<_>)` tuple with `struct EditInputResult { pending_high_risk_edit, paste_text }`; caller in `handle_keyboard_input` destructures by field name
- [x] 4.4 `src/app/mod.rs` needed no change — `handle_edit_input` is only called from `handle_keyboard_input` inside `hex_editor.rs`. The proposal overstated the ripple; the call site is internal.

## 5. Verification
- [x] 5.1 `cargo fmt` — clean
- [x] 5.2 `cargo build --release` — succeeds
- [x] 5.3 `cargo test` — 204/204 pass
- [x] 5.4 `cargo clippy --all-targets` — no new warnings from this refactor. Bundled `render_row`'s args into `RowRenderContext<'a>` to avoid a `too_many_arguments` warning.
- [ ] 5.5 Manual smoke test (user to perform) — test matrix: scroll a large file, click in both columns, drag-select across rows, shift+click, secondary-click, copy/paste in hex and ASCII modes, overwrite + insert edits, ESC cancel, search highlight, risk tint, incomplete last row. Confirm no user-visible behavior change vs. `main` baseline.

Scope note on matrix items:
- "Search highlight visible in both columns" and "Risk-level tint visible in both columns" — the pre-refactor code only paints these in the hex column; ASCII parity would be a behavior change and is not part of this refactor (see Task 1 scope note). Expect hex-column-only here.
