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
- [ ] 2.1 Define `struct RowResult { cursor_move: Option<(usize, EditMode)>, start_drag: bool, drag_current_offset: Option<usize>, context_menu_offset: Option<usize> }` with `Default`
- [ ] 2.2 Write `fn render_row(ui, row_idx, state: &DisplayState, colors: &AppColors, highlights: &HighlightLookup, ...) -> RowResult`
- [ ] 2.3 Move hex-column rendering from the `show()` closure into `render_row`
- [ ] 2.4 Move ASCII-column rendering from the `show()` closure into `render_row`
- [ ] 2.5 Ensure `render_row` performs NO editor state mutations — only reads and event collection
- [ ] 2.6 `cargo build` + visual smoke test

## 3. Extract interaction handler
- [ ] 3.1 Write `fn handle_row_interactions(result: RowResult, editor: &mut EditorState, ui_state: &mut UiState, shift_held: bool)`
- [ ] 3.2 Move click, drag-start, drag-continue, secondary-click, and context-menu logic from the `show()` closure into this function
- [ ] 3.3 Preserve drag-select behavior from commit 2cef50a (highlights both columns)
- [ ] 3.4 Preserve edit-mode-aware copy/paste from commit 2cef50a
- [ ] 3.5 Preserve non-selectable ASCII pipe borders from commit 6b4fdaf
- [ ] 3.6 Preserve ASCII alignment on incomplete last row from commit 37e8e65

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
