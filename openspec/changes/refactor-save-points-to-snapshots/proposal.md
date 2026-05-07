# Change: Save points survive length changes (compressed snapshot model)

## Why

Save points today are silently destroyed by any length-changing edit. The trigger paths in `src/editor/buffer.rs` — `on_length_changed` (`:276`), `apply_insert` (`:338`), `apply_delete` (`:346`) — all call `self.save_points.clear_all(&self.original)`. As a result, switching the editor into Insert mode (status bar shows "INS") and typing a single character wipes every save point in the session. Backspace/Delete in Insert mode and paste in Insert mode behave the same way. There is no warning, no indicator — the save point list just empties.

The root cause is the data model: each `SavePoint` stores `Vec<ByteChange>` where `ByteChange { offset, old_value, new_value }` uses **absolute offsets**. An insert or delete shifts every following offset, so the chain of diffs becomes meaningless. The original implementation chose the simple-but-blunt fix — clear everything — over the harder fix of making diffs length-aware.

A second limitation falls out of the same chain design: only the most recent (leaf) save point can be deleted. Non-leaf save points are locked because successor diffs are computed against them. The chain dependency forces this.

Both limitations have the same root, and a single change resolves both.

## What Changes

- Replace the diff-chain model with **per-save-point compressed snapshots**. Each `SavePoint` becomes self-contained: `compressed: Vec<u8>`, `uncompressed_len: usize`. No chain, no shared base state, no cross-save-point dependencies.
- Use `flate2` (already a project dependency for PNG IDAT handling) at default compression level. Image-shaped data typically compresses 5–10×; for the few-MB buffers this app handles, snapshot creation is in the tens of milliseconds and restore is in the low tens.
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
- **No on-disk format change.** Save points are in-memory only; no migration.
- **Memory cost:** ~1× compressed buffer per save point. For a 5 MB image and 10 save points, raw cost is ~50 MB; with typical 5–10× compression on image data, real cost is 5–10 MB. Bounded by user behavior (number of save points × file size).
- **CPU cost:** Compression on save-point creation (~30–50 ms for 5 MB), decompression on restore (~10 ms). Both are user-triggered and rare; no impact on the per-frame render path.
- **Risk:** The new `Replace` variant must round-trip correctly through undo/redo, including for length-change cases. Mitigated by direct round-trip tests on `Replace` and on the full restore-undo-redo cycle.
- **No ripple into** effects, format parsers, image rendering, settings, or persistence.
