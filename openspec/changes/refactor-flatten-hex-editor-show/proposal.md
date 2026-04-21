# Change: Flatten hex_editor::show() by extracting row rendering and interaction

## Why

`src/ui/hex_editor.rs::show()` is ~230 lines of deeply nested code (`src/ui/hex_editor.rs:462-692`) that mixes four concerns: virtual-scroll row iteration, per-byte visual painting, input handling (click, drag, secondary, context menu), and selection/cursor mutation. Per-byte rendering is additionally duplicated: `render_hex_byte()` (`:160-~210`) and `render_ascii_row()` (`:~210-262`) share ~100 lines of parallel cursor/selection/highlight/risk-tint painting logic.

Recent commits all land in this function and compound the nesting:

- 2cef50a — copy/paste respects edit mode; drag-select highlights both columns
- 6b4fdaf — ASCII column pipe borders non-selectable
- 37e8e65 — ASCII column alignment on incomplete last row

Every new selection or edit-mode feature makes `show()` harder to reason about. The nesting also makes it impossible to unit-test rendering logic separately from input handling.

## What Changes

- Introduce a `RowResult` struct describing input events detected while rendering one row: `cursor_move`, `start_drag`, `drag_current_offset`, `secondary_click`, `context_menu_offset`.
- Extract `render_row(ui, row_idx, state, colors, highlights, ...) -> RowResult`: builds one row's visuals (offset column, hex column, ASCII column) and returns detected events without mutating editor state.
- Extract `paint_byte_highlight(ui, rect, flags, colors)`: shared painter for cursor background, selection background, search highlight, and risk-level tint. Called from both hex and ASCII column rendering inside `render_row`.
- Extract `handle_row_interactions(row_result, editor, ui_state)`: applies collected `RowResult` events to editor state in one pass.
- `show()` shrinks to an orchestrator: resolve scroll target, compute visible range, loop `render_row` over visible rows, collect results, call `handle_row_interactions`. Target: ≤100 lines.
- Replace `handle_edit_input`'s `(Option<_>, Option<_>)` return (`src/ui/hex_editor.rs:720-756`) with a named `EditInputResult` struct.
- **Non-functional:** no user-visible behavior change. All recent selection/edit-mode/ASCII-alignment work preserved.

## Impact

- **Affected specs:** `hex-editor` capability — adds architectural requirements for render/interact separation. No behavioral delta.
- **Affected code:**
  - `src/ui/hex_editor.rs` — primary target.
  - `src/app/mod.rs` — minor ripple where `EditInputResult` is consumed (currently destructures the tuple at the call site).
- **No ripple into** `src/editor/` — this refactor stops at the UI boundary.
- **Enables:** future row-level features (inline annotations, per-row diff markers, structure-aware backgrounds) without deeper nesting. Also unblocks snapshot-style visual parity tests on `render_row` in isolation.
- **Risk:** refactor of input handling is the most error-prone part — off-by-one errors in drag-select across hex/ASCII columns are easy to reintroduce. **Mitigated** by landing `paint_byte_highlight` extraction first (pure visual), then `render_row` (collects events, no mutations), then `handle_row_interactions` last.
- **Dependency:** lands cleaner after `refactor-split-app-state` (so `show()` can take `&mut DocumentState, &mut UiState` instead of `&mut BendApp`), but is not blocked on it.
