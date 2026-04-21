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
- [ ] 4.1 Rewrite `show()` as: prepare state → compute scroll target → build `ScrollArea` → in `show_viewport`, loop visible rows calling `render_row`, accumulating a `Vec<RowResult>` (or folding a single `RowResult`) → after the loop, call `handle_row_interactions`
- [ ] 4.2 Verify `show()` is ≤100 lines after refactor
- [ ] 4.3 Replace `handle_edit_input`'s `(Option<_>, Option<_>)` tuple return with `struct EditInputResult { pending_high_risk_edit: Option<...>, paste_text: Option<String> }`
- [ ] 4.4 Update the call site in `src/app/mod.rs` to destructure the named struct

## 5. Verification
- [ ] 5.1 `cargo fmt`
- [ ] 5.2 `cargo build --release`
- [ ] 5.3 `cargo test`
- [ ] 5.4 Manual test matrix:
  - Scroll a large file (>10MB) — no lag, no rendering artifacts
  - Click to move cursor in hex column
  - Click to move cursor in ASCII column
  - Drag-select across rows — both columns highlight together
  - Shift+click to extend selection
  - Secondary-click to open context menu
  - Copy selection in hex mode (copies hex bytes)
  - Copy selection in ASCII mode (copies text)
  - Paste in hex mode
  - Paste in ASCII mode
  - Overwrite edit
  - Insert edit
  - ESC cancels pending edit
  - Search highlight visible in both columns
  - Risk-level tint visible in both columns
  - Incomplete last row (file length not multiple of 16) renders correctly
- [ ] 5.5 Verify no visible behavior change vs. `main` baseline
