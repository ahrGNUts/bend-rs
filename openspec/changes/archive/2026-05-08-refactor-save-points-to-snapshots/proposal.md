# Change: Save points survive length changes (uncompressed snapshot model)

## Why

Save points today are silently destroyed by any length-changing edit. The trigger paths in `src/editor/buffer.rs` — `on_length_changed` (`:276`), `apply_insert` (`:338`), `apply_delete` (`:346`) — all call `self.save_points.clear_all(&self.original)`. As a result, switching the editor into Insert mode (status bar shows "INS") and typing a single character wipes every save point in the session. Backspace/Delete in Insert mode and paste in Insert mode behave the same way. There is no warning, no indicator — the save point list just empties.

The root cause is the data model: each `SavePoint` stores `Vec<ByteChange>` where `ByteChange { offset, old_value, new_value }` uses **absolute offsets**. An insert or delete shifts every following offset, so the chain of diffs becomes meaningless. The original implementation chose the simple-but-blunt fix — clear everything — over the harder fix of making diffs length-aware.

A second limitation falls out of the same chain design: only the most recent (leaf) save point can be deleted. Non-leaf save points are locked because successor diffs are computed against them. The chain dependency forces this.

Both limitations have the same root, and a single change resolves both.

## What Changes

- Replace the diff-chain model with **per-save-point uncompressed snapshots**. Each `SavePoint` becomes self-contained: `bytes: Vec<u8>` holds a full copy of the working buffer at creation time. No chain, no shared base state, no cross-save-point dependencies.
- Compression was investigated and ruled out (see `design.md`): on this app's supported formats (BMP, JPEG, GIF), the only format that compresses meaningfully is BMP (~1.21–1.29×). JPEG and GIF gain effectively nothing because their bulk is already compressed by their own encodings. The complexity, the new dependency, and the per-save-point CPU cost did not earn their keep on representative workloads.
- Add a new `EditOperation::Replace { offset, old_values, new_values }` variant to `src/editor/history.rs`. Unlike `EditOperation::Range`, the two value vectors may differ in length. `restore_save_point` always emits `Replace { offset: 0, .. }` so that restoring to a snapshot of a different length is correctly recorded as a single undoable history entry. `Range` stays for fixed-length in-place edits.
- Drop `self.save_points.clear_all(...)` calls from `on_length_changed`, `apply_insert`, `apply_delete`. `clear_all` itself remains as the file-load reset path but takes no `base_state` argument anymore.
- Drop the leaf-only restriction. `SavePointManager::delete(id)` removes any save point and re-indexes the `HashMap`. `can_delete` becomes "does it exist?" — likely deleted entirely so the UI always shows the trash button.
- `SavePointManager::restore(&self, id) -> Option<Vec<u8>>` no longer needs an `original` argument; each save point is fully self-contained.
- Removed: `ByteChange`, `compute_diff`, `SavePointManager::last_save_point_state`, the diff-chain doc comment block.
- `restore_save_point` clamps the cursor to `working.len().saturating_sub(1)` after restore (since the new length may be shorter), and sets `length_changed = true` when the length actually changes (so virtual-scroll caches in the hex editor invalidate).
- UI cleanup in `src/ui/savepoints.rs`: remove the `can_delete: Vec<bool>` collection (`:48-56`) and the `can_delete.get(idx)` gate around the trash button (`:137`).

**No behavior change for users beyond the fix:** create, name, restore, and rename all work identically. The visible additions are: save points stay alive after Insert-mode keystrokes, and the trash button is always available.

## Impact

- **Affected specs:** `hex-editor` capability — modifies the existing "Save Points" requirement to add explicit persistence-across-length-changes scenarios and remove the implicit leaf-only deletion model.
- **Affected code:**
  - `src/editor/savepoints.rs` — primary rewrite. Data model change, API change, removed helpers.
  - `src/editor/buffer.rs` — drop `clear_all` calls in 3 locations; update `restore_save_point` to emit `Replace` and clamp cursor; drop `original` arg from manager calls.
  - `src/editor/history.rs` — add `EditOperation::Replace` variant; update apply-undo / apply-redo dispatch.
  - `src/ui/savepoints.rs` — drop `can_delete` collection; trash button always shown.
  - Existing tests update: `test_insert_clears_save_points` in `buffer.rs` becomes `test_insert_preserves_save_points`. New tests for length-change persistence, non-leaf delete, length-change restore round-trip, `Replace` op round-trip.
- **No new dependencies.** No on-disk format change. Save points are in-memory only; no migration.
- **Memory cost:** ~1× buffer per save point. Bounded and predictable. Worked examples for a 50 MB file: 5 save points = 250 MB; 10 save points = 500 MB; 20 save points = 1 GB. The dual-buffer architecture (immutable `original` + editable `working`) already accounts for 2× the file size before any save points exist, so save points are an additive cost on top of that baseline. For typical workloads (single-digit MB files, ~5–15 save points per session), additional cost is in the low tens of MB. Users routinely working with files in the 50+ MB range and many save points will want to keep save-point counts modest. On-disk save-point persistence is a possible future change if that workload turns out to be common — out of scope here.
- **CPU cost:** Save-point creation = a single `Vec<u8>::clone()`. Restore = another clone. Both are memory-bandwidth-bound and effectively free at the file sizes this app handles.
- **Risk:** The new `Replace` variant must round-trip correctly through undo/redo, including for length-change cases. Mitigated by direct round-trip tests on `Replace` and on the full restore-undo-redo cycle.
- **No ripple into** effects, format parsers, image rendering, settings, or persistence.
