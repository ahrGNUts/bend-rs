# Change: Flatten hex_editor::show() by extracting row rendering and interaction

## Why

`src/ui/hex_editor.rs::show()` is ~230 lines of deeply nested code (`src/ui/hex_editor.rs:462-692`) that mixes four concerns: virtual-scroll row iteration, per-byte visual painting, input handling (click, drag, secondary, context menu), and selection/cursor mutation. Per-byte rendering is additionally duplicated: `render_hex_byte()` (`:160-~210`) and `render_ascii_row()` (`:~210-262`) share ~100 lines of parallel cursor/selection/highlight/risk-tint painting logic.

Recent commits all land in this function and compound the nesting:

- 2cef50a — copy/paste respects edit mode; drag-select highlights both columns
- 6b4fdaf — ASCII column pipe borders non-selectable
- 37e8e65 — ASCII column alignment on incomplete last row

Every new selection or edit-mode feature makes `show()` harder to reason about. The nesting also makes it impossible to unit-test rendering logic separately from input handling.

## What Changes

- Introduce a `RowResult` struct describing input events detected while rendering one row: `cursor_move`, `start_drag`, `drag_current_offset`, `context_menu_offset`.
- Extract `render_row(ui, row_idx, ctx: &RowRenderContext) -> RowResult`: builds one row's visuals (offset column, hex column, ASCII column) and returns detected events without mutating editor state. `RowRenderContext` bundles the per-frame read state (display state, editor, colors, highlight lookup, pointer snapshot) that the renderer needs.
- Extract two narrow shared helpers in place of a single `paint_byte_highlight`:
  - `cursor_color_pair(write_mode, colors) -> (bright, dim)` — used by both hex and ASCII renderers for the insert-vs-overwrite cursor palette decision.
  - `byte_background_color(highlight, colors) -> Option<Color32>` — the priority chain (selection > current_match > search_match > bookmark > section tint) used by the hex column renderer.
  - **Why two helpers, not one painter:** the hex column paints a split-nibble cursor (left half / right half intensities) while the ASCII column paints a single full-cell cursor. The actual "paint a byte cell" operations differ enough that a unified painter would mostly be a switch. Extracting the genuinely duplicated decisions (palette + priority) gives the deduplication win without forcing a fake abstraction. The ASCII column also paints a strict subset of highlights today (cursor + selection only); fully unifying the painter would imply painting search/risk in ASCII too, which is a behavior change explicitly out of scope.
- Extract `handle_row_interactions(ui, app, result, ctx: &RowInteractionContext)`: applies collected `RowResult` events to editor + UI state in one pass. `RowInteractionContext` bundles the per-frame snapshot (`shift_held`, `primary_down`, `drag_id`) the handler needs.
- `show()` shrinks to an orchestrator: resolve scroll target, compute visible range, loop `render_row` over visible rows, fold results, call `handle_row_interactions`. Target: ≤100 lines.
- Replace `handle_edit_input`'s `(Option<_>, Option<_>)` return with a named `EditInputResult` struct.
- **Non-functional:** no user-visible behavior change. All recent selection/edit-mode/ASCII-alignment work preserved.

## Impact

- **Affected specs:** `hex-editor` capability — adds architectural requirements for render/interact separation. No behavioral delta.
- **Affected code:**
  - `src/ui/hex_editor.rs` — primary target.
  - `src/app/mod.rs` — minor ripple where `EditInputResult` is consumed (currently destructures the tuple at the call site).
- **No ripple into** `src/editor/` — this refactor stops at the UI boundary.
- **Enables:** future row-level features (inline annotations, per-row diff markers, structure-aware backgrounds) without deeper nesting. Also unblocks snapshot-style visual parity tests on `render_row` in isolation.
- **Risk:** refactor of input handling is the most error-prone part — off-by-one errors in drag-select across hex/ASCII columns are easy to reintroduce. **Mitigated** by landing the byte-highlight helpers first (pure visual), then `render_row` (collects events, no mutations), then `handle_row_interactions` last.
- **Dependency:** lands cleaner after `refactor-split-app-state` (so `show()` can take `&mut DocumentState, &mut UiState` instead of `&mut BendApp`), but is not blocked on it.
