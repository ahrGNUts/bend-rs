# Tasks: Save points as self-contained uncompressed snapshots

## 1. Add `EditOperation::Replace` variant

- [x] 1.1 Add `Replace { offset: usize, old_values: Vec<u8>, new_values: Vec<u8> }` variant to `EditOperation` in `src/editor/history.rs`.
- [x] 1.2 Wire `Replace` into `apply_undo_op` and `apply_redo_op` (or whichever dispatch the buffer uses) in `src/editor/buffer.rs`. Forward = splice `working[offset..offset + old_values.len()]` with `new_values`; backward = splice `working[offset..offset + new_values.len()]` with `old_values`.
- [x] 1.3 Verify `Range` is untouched (still equal-length only). Existing call sites — `replace_bytes`, `replace_all_bytes`, `apply_effect_batch` — keep using `Range`.
- [x] 1.4 Tests (in `src/editor/history.rs` or wherever `EditOperation` tests live):
  - `test_replace_round_trips_for_equal_length`
  - `test_replace_round_trips_for_grow` (new longer than old)
  - `test_replace_round_trips_for_shrink` (new shorter than old)
- [x] 1.5 `cargo build` clean.

## 2. Rewrite `SavePoint` and `SavePointManager` to uncompressed snapshots

- [x] 2.1 In `src/editor/savepoints.rs`, change `SavePoint` to `{ id, name, bytes: Vec<u8> }`. Remove `diff` field.
- [x] 2.2 Remove `ByteChange` struct, `compute_diff` fn, `last_save_point_state` field, and the diff-chain doc-comment block at the top of the file.
- [x] 2.3 `SavePointManager::new()` takes no args.
- [x] 2.4 `create(name, current_state)` clones `current_state` into the new save point's `bytes` field. No diff computation.
- [x] 2.5 `restore(&self, id) -> Option<Vec<u8>>` returns `save_point.bytes.clone()`. Drop the `original` parameter.
- [x] 2.6 `delete(id) -> bool` works for any id, not just leaf. Removes from `save_points` (e.g. `swap_remove` or `remove`) AND rebuilds `id_to_index` to reflect the new positions. Returns whether the id existed.
- [x] 2.7 Remove `can_delete` method (deletion is always allowed if the save point exists).
- [x] 2.8 `clear_all()` takes no args (no base state to track). Used only on file load.
- [x] 2.9 Update existing tests:
  - `test_create_save_point` — assertion becomes "snapshot bytes match input".
  - `test_restore_save_point` — drop the `original` argument from the call.
  - `test_delete_leaf_save_point` — rename to `test_delete_any_save_point`; cover middle deletion.
  - Drop `test_compute_diff` (function gone).
- [x] 2.10 New tests in `src/editor/savepoints.rs`:
  - `test_delete_middle_save_point_re_indexes` (delete index 1 of 3, verify ids 0 and 2 still resolve and restore correctly)
  - `test_save_point_independent_of_subsequent_edits` (snapshot is unaffected by edits made after creation)
- [x] 2.11 `cargo build` clean.

## 3. Buffer integration

- [x] 3.1 In `src/editor/buffer.rs`, update `EditorState::new` to call `SavePointManager::new()` (no args).
- [x] 3.2 In `on_length_changed` (line 276), drop `self.save_points.clear_all(&self.original)`. Keep bookmark adjustment and `length_changed = true`.
- [x] 3.3 In `apply_insert` (line 338), drop `self.save_points.clear_all(&self.original)`.
- [x] 3.4 In `apply_delete` (line 346), drop `self.save_points.clear_all(&self.original)`.
- [x] 3.5 In `restore_save_point` (line 474):
  - Drop `&self.original` arg from `save_points.restore(id)` (now `save_points.restore(id)`).
  - Capture `length_will_change = restored.len() != self.working.len()` BEFORE moving the vector.
  - `let old_values = std::mem::replace(&mut self.working, restored)`.
  - If `old_values != self.working`, push `EditOperation::Replace { offset: 0, old_values, new_values: self.working.clone() }` directly to `self.history` (preserve existing direct-push pattern; do not switch to `record_operation`).
  - If `length_will_change`, set `self.length_changed = true`.
  - Clamp cursor: `self.cursor = self.cursor.min(self.working.len().saturating_sub(1))`.
  - Update `self.modified = self.working != self.original`.
- [x] 3.6 Remove `EditorState::can_delete_save_point` and the corresponding wiring; or have it forward to a manager method that returns "exists".
- [x] 3.7 Tests:
  - `test_insert_clears_save_points` → rename to `test_insert_preserves_save_points`; invert assertion.
  - New: `test_save_point_survives_byte_edit`, `test_save_point_survives_insert`, `test_save_point_survives_delete`, `test_save_point_survives_undo_redo_of_length_change`.
  - New: `test_restore_with_shorter_snapshot` — buffer grows to N+10, restore to length-N save point, verify length and contents.
  - New: `test_restore_with_longer_snapshot` — buffer shrinks to N-5, restore to length-N save point, verify length and contents.
  - New: `test_restore_clamps_cursor` — cursor at offset 100, restore to a 50-byte snapshot, cursor lands at 49.
  - New: `test_restore_undo_round_trips_through_length_change` — restore (length changes), undo (back to original length and contents), redo (length changes again).
- [x] 3.8 `cargo build` clean.

## 4. UI cleanup

- [x] 4.1 In `src/ui/savepoints.rs`, remove the `can_delete: Vec<bool>` collection (lines 47–56).
- [x] 4.2 Remove the `can_delete.get(idx).copied().unwrap_or(false) &&` gate around the trash button (line 137). Trash button is always shown for every save point.
- [x] 4.3 If `EditorState::can_delete_save_point` was kept as an "exists?" wrapper, remove this UI's call to it; otherwise update the call site to drop the now-removed method.
- [x] 4.4 Visual smoke test: open file, create 3 save points, click trash on the middle one — it disappears, others remain.

## 5. Verification

- [x] 5.1 `cargo fmt` — clean.
- [x] 5.2 `cargo build --release` — succeeds.
- [x] 5.3 `cargo test` — all pass, including the renamed/inverted/new tests.
- [x] 5.4 `cargo clippy --all-targets` — no new warnings.
- [x] 5.5 Manual smoke test:
  - Open an image. Create "Save A". Toggle to Insert mode (status bar "INS"). Type a few hex characters. **Expect:** "Save A" still in the panel.
  - Backspace several times in Insert mode. **Expect:** "Save A" still in the panel.
  - Paste bytes via Cmd/Ctrl+V in Insert mode. **Expect:** "Save A" still in the panel.
  - Create "Save B" (now with longer buffer than "Save A"). Restore to "Save A". **Expect:** buffer length and contents revert to Save A's. Cursor is in range. Undo → back to Save B's state. Redo → back to Save A's state.
  - Create "Save C" so the panel shows A, B, C. Delete "Save B" (the middle one). **Expect:** A and C remain, both still restorable.
  - Run an effect that mutates many bytes (no length change). **Expect:** all save points remain.
- [x] 5.6 `openspec validate refactor-save-points-to-snapshots --strict --no-interactive` — passes.
