# Save points survive length changes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Save points must survive every kind of buffer edit, including length-changing operations and undo/redo of those operations. Any save point — not just the most recent — must be deletable.

**Architecture:** Replace the diff-chain `SavePoint { id, name, diff: Vec<ByteChange> }` model with self-contained `SavePoint { id, name, bytes: Vec<u8> }` snapshots. Add a new length-aware `EditOperation::Replace` history variant so that restoring to a snapshot of a different length round-trips through undo/redo as a single atomic step. Drop the three `save_points.clear_all(...)` calls in `EditorState`'s length-changing paths (`on_length_changed`, `apply_insert`, `apply_delete`).

**Tech Stack:** Rust 2021. `eframe` / `egui` 0.29 (UI). `image` crate (decode only — not touched by this change). No new dependencies.

**Reference docs:**
- `openspec/changes/refactor-save-points-to-snapshots/proposal.md` — what & why
- `openspec/changes/refactor-save-points-to-snapshots/design.md` — option analysis, restore semantics, memory cost analysis
- `openspec/changes/refactor-save-points-to-snapshots/specs/hex-editor/spec.md` — MODIFIED Save Points requirement and scenarios

---

## File Map

| File | Role | Change |
|---|---|---|
| `src/editor/history.rs` | `EditOperation` enum + coalescing + `History` ring buffer | **Modify** — add `Replace` variant, add no-coalesce arm in `try_coalesce` |
| `src/editor/buffer.rs` | `EditorState`: dual buffer, history dispatch, save-point integration | **Modify** — handle `Replace` in `apply_undo_op`/`apply_redo_op`; drop `clear_all` calls in 3 spots; rewrite `restore_save_point`; remove `can_delete_save_point`; rename/invert and add tests |
| `src/editor/savepoints.rs` | `SavePoint` and `SavePointManager` | **Rewrite** — bytes-based snapshots; delete-any-save-point; remove `ByteChange`/`compute_diff`/`last_save_point_state`; remove `can_delete`; clean up `new` / `restore` / `clear_all` signatures |
| `src/ui/savepoints.rs` | Save points side-panel UI | **Modify** — drop `can_delete: Vec<bool>` collection and the trash-button gate so trash is always shown |

No new files. No tests outside the touched modules.

---

## Task 1: Add `EditOperation::Replace` variant

**Files:**
- Modify: `src/editor/history.rs` — add enum variant, add `try_coalesce` arm
- Modify: `src/editor/buffer.rs` — add `apply_undo_op` / `apply_redo_op` arms, add 3 round-trip tests

**Why this task is first:** `restore_save_point` (in Task 3) needs `Replace` to record a length-changing restore as a single undoable history entry. Building this primitive first lets Task 3 use it without a forward reference.

- [ ] **Step 1.1: Add 3 failing tests to `src/editor/buffer.rs::tests` mod**

Add the following tests inside `#[cfg(test)] mod tests { ... }`. Place them after `test_undo_redo` (search for `fn test_undo_redo` to find the spot).

```rust
#[test]
fn test_replace_op_round_trips_for_equal_length() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data.clone());

    let op = EditOperation::Replace {
        offset: 0,
        old_values: vec![0x00, 0x01, 0x02, 0x03],
        new_values: vec![0xAA, 0xBB, 0xCC, 0xDD],
    };

    editor.apply_redo_op(&op);
    assert_eq!(editor.working(), &[0xAA, 0xBB, 0xCC, 0xDD]);

    editor.apply_undo_op(&op);
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn test_replace_op_round_trips_for_grow() {
    let data = vec![0x00, 0x01, 0x02];
    let mut editor = EditorState::new(data);

    let op = EditOperation::Replace {
        offset: 0,
        old_values: vec![0x00, 0x01, 0x02],
        new_values: vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
    };

    editor.apply_redo_op(&op);
    assert_eq!(editor.working(), &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);

    editor.apply_undo_op(&op);
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02]);
}

#[test]
fn test_replace_op_round_trips_for_shrink() {
    let data = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
    let mut editor = EditorState::new(data);

    let op = EditOperation::Replace {
        offset: 0,
        old_values: vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
        new_values: vec![0x00, 0x01],
    };

    editor.apply_redo_op(&op);
    assert_eq!(editor.working(), &[0x00, 0x01]);

    editor.apply_undo_op(&op);
    assert_eq!(editor.working(), &[0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
}
```

- [ ] **Step 1.2: Run the new tests; confirm compile failure**

```bash
cargo test --lib editor::buffer::tests::test_replace_op 2>&1 | tail -20
```

Expected: compilation error mentioning `no variant or associated item named 'Replace'` on `EditOperation`. We have not yet added the variant.

- [ ] **Step 1.3: Add the `Replace` variant in `src/editor/history.rs`**

Find the `pub enum EditOperation` block. Append a new variant after `DeleteBytes`:

```rust
/// Replace a contiguous range of bytes; old and new ranges may have different lengths.
/// Used by save-point restoration where the buffer length may differ between the
/// snapshot and the current working buffer. For equal-length in-place edits, prefer
/// `Range`, which carries the equal-length invariant in the type.
Replace {
    offset: usize,
    old_values: Vec<u8>,
    new_values: Vec<u8>,
},
```

Then find `fn try_coalesce`. Its inner `match existing { ... }` covers `Single`, `InsertBytes | DeleteBytes | Group`, and `Range`. Add a `Replace` no-coalesce arm. The simplest is to fold it into the existing `InsertBytes | DeleteBytes | Group` arm:

```rust
EditOperation::InsertBytes { .. }
| EditOperation::DeleteBytes { .. }
| EditOperation::Group(_)
| EditOperation::Replace { .. } => false,
```

- [ ] **Step 1.4: Add match arms in `apply_undo_op` and `apply_redo_op` in `src/editor/buffer.rs`**

In `fn apply_undo_op` (find with `grep -n 'fn apply_undo_op' src/editor/buffer.rs`), add a new arm before the closing brace of the `match op { ... }`:

```rust
EditOperation::Replace {
    offset,
    old_values,
    new_values,
} => {
    let len_before = self.working.len();
    self.working.splice(
        *offset..*offset + new_values.len(),
        old_values.iter().copied(),
    );
    if self.working.len() != len_before {
        self.length_changed = true;
    }
}
```

In `fn apply_redo_op` (just below `apply_undo_op`), add a parallel arm:

```rust
EditOperation::Replace {
    offset,
    old_values,
    new_values,
} => {
    let len_before = self.working.len();
    self.working.splice(
        *offset..*offset + old_values.len(),
        new_values.iter().copied(),
    );
    if self.working.len() != len_before {
        self.length_changed = true;
    }
}
```

The two arms are mirrored: undo uses `new_values.len()` for the splice range and writes `old_values`; redo uses `old_values.len()` and writes `new_values`. This is correct because the operation describes the *forward* direction (`old_values` → `new_values`), so undo reverses it.

- [ ] **Step 1.5: Run the 3 new tests; confirm they pass**

```bash
cargo test --lib editor::buffer::tests::test_replace_op 2>&1 | tail -10
```

Expected: `test result: ok. 3 passed`.

- [ ] **Step 1.6: Run the full test suite + clippy + fmt**

```bash
cargo fmt
cargo build --release
cargo test 2>&1 | tail -3
cargo clippy --all-targets 2>&1 | tail -5
```

Expected:
- `cargo fmt`: no diff to apply on a re-run.
- `cargo build --release`: succeeds.
- `cargo test`: 210 passed (207 existing + 3 new). 0 failed.
- `cargo clippy`: no new warnings.

- [ ] **Step 1.7: Commit**

```bash
git add src/editor/history.rs src/editor/buffer.rs
git commit -m "$(cat <<'EOF'
feat(editor/history): add EditOperation::Replace for length-changing edits

Range enforces equal-length old/new vectors. The upcoming
save-point-restoration path needs to record a swap between buffers
of different lengths as a single undoable history entry, so it
needs a length-aware variant.

Replace is added as a sibling, not a relaxation of Range, so
existing fixed-length call sites keep their typing invariant.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Rewrite `SavePointManager` to bytes-based snapshots

**Files:**
- Modify: `src/editor/savepoints.rs` — full rewrite of data model, methods, and tests

**Why before Task 3:** This task changes `SavePoint`'s public field shape (`diff` → `bytes`) and `SavePointManager`'s internals. Method *signatures* are deliberately preserved so `EditorState` (in `buffer.rs`) keeps compiling. Cleanup of vestigial signature args happens atomically in Task 3 alongside the buffer.rs changes.

- [ ] **Step 2.1: Replace the contents of `src/editor/savepoints.rs`**

Overwrite the file with:

```rust
//! Save points: named, self-contained snapshots of editing state
//!
//! Each save point holds a full `Vec<u8>` snapshot of the working buffer at the
//! moment of creation. Snapshots are independent — there is no chain of diffs —
//! so save points survive any subsequent edit (including length changes) and
//! can be deleted in any order.
//!
//! Memory cost is `O(N × buffer_size)` for N save points. For typical workloads
//! (single-digit MB files, modest save-point counts) this is a few tens of MB.
//! See `openspec/changes/refactor-save-points-to-snapshots/design.md` for the
//! detailed cost analysis and the comparison with diff-based and compressed
//! alternatives.

use std::collections::HashMap;

/// A named snapshot of the editing state.
#[derive(Clone, Debug)]
pub struct SavePoint {
    pub id: u64,
    pub name: String,
    /// Snapshot of the working buffer at creation time. Owned, uncompressed.
    bytes: Vec<u8>,
}

impl SavePoint {
    pub fn new(id: u64, name: String, bytes: Vec<u8>) -> Self {
        Self { id, name, bytes }
    }
}

/// Manages save points for an editor session.
pub struct SavePointManager {
    save_points: Vec<SavePoint>,
    id_to_index: HashMap<u64, usize>,
    next_id: u64,
}

impl SavePointManager {
    /// Create a new save point manager.
    ///
    /// The `_original_bytes` argument is unused and retained only for source
    /// compatibility while `EditorState` is still calling the old signature.
    /// Task 3 of the refactor drops this argument.
    pub fn new(_original_bytes: &[u8]) -> Self {
        Self {
            save_points: Vec::new(),
            id_to_index: HashMap::new(),
            next_id: 1,
        }
    }

    /// All save points, in creation order.
    pub fn save_points(&self) -> &[SavePoint] {
        &self.save_points
    }

    /// Mutable access to a save point by ID.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut SavePoint> {
        self.id_to_index
            .get(&id)
            .map(|&index| &mut self.save_points[index])
    }

    /// Create a new save point capturing the current working buffer state.
    /// Returns the ID of the created save point.
    pub fn create(&mut self, name: String, current_state: &[u8]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let save_point = SavePoint::new(id, name, current_state.to_vec());
        let index = self.save_points.len();
        self.save_points.push(save_point);
        self.id_to_index.insert(id, index);

        id
    }

    /// Restore by returning a fresh copy of the snapshot's bytes.
    ///
    /// The `_original` argument is unused and retained only for source
    /// compatibility while `EditorState` is still calling the old signature.
    /// Task 3 of the refactor drops this argument.
    pub fn restore(&self, id: u64, _original: &[u8]) -> Option<Vec<u8>> {
        let index = *self.id_to_index.get(&id)?;
        Some(self.save_points[index].bytes.clone())
    }

    /// Rename a save point. Returns whether the save point was found.
    #[must_use = "returns whether the save point was found and renamed"]
    pub fn rename(&mut self, id: u64, new_name: String) -> bool {
        if let Some(sp) = self.get_mut(id) {
            sp.name = new_name;
            true
        } else {
            false
        }
    }

    /// Returns whether a save point with the given id currently exists.
    ///
    /// Retained for caller compatibility; deletion is now permitted for any
    /// existing save point. Task 3 of the refactor removes this method and
    /// updates the UI to always show the trash button.
    pub fn can_delete(&self, id: u64) -> bool {
        self.id_to_index.contains_key(&id)
    }

    /// Delete a save point. Works for any save point, regardless of its
    /// position in the list. Returns whether the save point was found.
    #[must_use = "returns whether the save point was deleted"]
    pub fn delete(&mut self, id: u64) -> bool {
        let Some(&index) = self.id_to_index.get(&id) else {
            return false;
        };
        self.save_points.remove(index);
        // Rebuild the index since `Vec::remove` shifts every later element down by one.
        self.id_to_index.clear();
        for (idx, sp) in self.save_points.iter().enumerate() {
            self.id_to_index.insert(sp.id, idx);
        }
        true
    }

    /// Number of save points.
    pub fn len(&self) -> usize {
        self.save_points.len()
    }

    /// Clear all save points.
    ///
    /// Used on file load (a new file's working buffer has nothing to do with
    /// the previous file's save points). The `_base_state` argument is unused
    /// and retained only for source compatibility while `EditorState` is
    /// still calling the old signature. Task 3 drops this argument.
    pub fn clear_all(&mut self, _base_state: &[u8]) {
        self.save_points.clear();
        self.id_to_index.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let modified = vec![0xAA, 0x01, 0xBB, 0x03];
        let id = manager.create("First save".to_string(), &modified);

        assert_eq!(manager.len(), 1);
        let restored = manager.restore(id, &original).unwrap();
        assert_eq!(restored, modified);
    }

    #[test]
    fn test_restore_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let modified1 = vec![0xAA, 0x01, 0x02, 0x03];
        let id1 = manager.create("SP1".to_string(), &modified1);

        let modified2 = vec![0xAA, 0xBB, 0x02, 0x03];
        let id2 = manager.create("SP2".to_string(), &modified2);

        let restored1 = manager.restore(id1, &original).unwrap();
        assert_eq!(restored1, modified1);

        let restored2 = manager.restore(id2, &original).unwrap();
        assert_eq!(restored2, modified2);
    }

    #[test]
    fn test_rename_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let id = manager.create("Original name".to_string(), &original);
        assert!(manager.rename(id, "New name".to_string()));

        let sps = manager.save_points();
        assert_eq!(sps[0].name, "New name");
    }

    #[test]
    fn test_delete_any_save_point() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let modified1 = vec![0xAA, 0x01, 0x02, 0x03];
        let id1 = manager.create("SP1".to_string(), &modified1);

        let modified2 = vec![0xAA, 0xBB, 0x02, 0x03];
        let id2 = manager.create("SP2".to_string(), &modified2);

        let modified3 = vec![0xAA, 0xBB, 0xCC, 0x03];
        let id3 = manager.create("SP3".to_string(), &modified3);

        // Middle save point can be deleted (was the leaf-only restriction's blocker).
        assert!(manager.delete(id2));
        assert_eq!(manager.len(), 2);

        // Both remaining save points still resolve to their captured states.
        let restored1 = manager.restore(id1, &original).unwrap();
        assert_eq!(restored1, modified1);
        let restored3 = manager.restore(id3, &original).unwrap();
        assert_eq!(restored3, modified3);

        // Now delete what's currently the first of two (also a non-leaf in the original list).
        assert!(manager.delete(id1));
        assert_eq!(manager.len(), 1);
        let restored3 = manager.restore(id3, &original).unwrap();
        assert_eq!(restored3, modified3);
    }

    #[test]
    fn test_save_point_independent_of_subsequent_edits() {
        let original = vec![0x00, 0x01, 0x02, 0x03];
        let mut manager = SavePointManager::new(&original);

        let snapshot_state = vec![0xAA, 0x01, 0x02, 0x03];
        let id = manager.create("SP1".to_string(), &snapshot_state);

        // Even if subsequent calls pass arbitrary `original` arguments, the
        // snapshot is self-contained and restore returns its captured bytes.
        let restored = manager.restore(id, &[0xFF, 0xEE, 0xDD]).unwrap();
        assert_eq!(restored, snapshot_state);
    }

    #[test]
    fn test_delete_re_indexes_after_middle_removal() {
        let original = vec![0x00];
        let mut manager = SavePointManager::new(&original);

        let id1 = manager.create("a".to_string(), &[0x01]);
        let id2 = manager.create("b".to_string(), &[0x02]);
        let id3 = manager.create("c".to_string(), &[0x03]);

        assert!(manager.delete(id2));

        // After re-indexing, id3 must still resolve correctly. If `delete`
        // failed to rebuild `id_to_index`, id3 would point at a stale index
        // and `restore` would return `None` or wrong bytes.
        let restored3 = manager.restore(id3, &original).unwrap();
        assert_eq!(restored3, vec![0x03]);
        let restored1 = manager.restore(id1, &original).unwrap();
        assert_eq!(restored1, vec![0x01]);
    }
}
```

- [ ] **Step 2.2: Run savepoints tests; confirm they pass**

```bash
cargo test --lib editor::savepoints 2>&1 | tail -10
```

Expected: `test result: ok. 6 passed`. The 6 tests are: `test_create_save_point`, `test_restore_save_point`, `test_rename_save_point`, `test_delete_any_save_point`, `test_save_point_independent_of_subsequent_edits`, `test_delete_re_indexes_after_middle_removal`.

Note that `test_compute_diff` is gone (function removed) and `test_delete_leaf_save_point` is gone (replaced by `test_delete_any_save_point` with broader coverage).

- [ ] **Step 2.3: Run the full test suite to confirm nothing else broke**

```bash
cargo test 2>&1 | tail -3
```

Expected: still all green. The existing `test_insert_clears_save_points` in `buffer.rs::tests` will still pass at this point because the `clear_all` call in `on_length_changed` is unchanged — Task 3 inverts that behavior. The save-point integration tests `test_save_point_create_and_restore` and `test_save_point_rename` in `buffer.rs` should also still pass because the rewrite preserves the external contract.

> **Errata** (added after Task 2 implementation): `test_save_point_delete` in `buffer.rs::tests` was originally listed here as "should still pass," but it actually fails under the new semantics — its assertion `!editor.can_delete_save_point(sp1)` (leaf-only) is invalidated by `SavePointManager::can_delete` becoming an exists-check. The pre-commit hook would block on it. The test is scheduled for deletion in Task 3 anyway (Step 3.1, "delete it"), so it must be deleted here in Task 2 to keep the tree green at the commit boundary. Add `src/editor/buffer.rs` to the Step 2.5 `git add` and remove the `fn test_save_point_delete` block when applying this task.

- [ ] **Step 2.4: cargo fmt + clippy**

```bash
cargo fmt
cargo clippy --all-targets 2>&1 | tail -5
```

Expected: no diff after fmt; no new clippy warnings.

- [ ] **Step 2.5: Commit**

```bash
git add src/editor/savepoints.rs
git commit -m "$(cat <<'EOF'
refactor(editor/savepoints): rewrite SavePointManager to bytes-based snapshots

Drops the absolute-offset diff chain and the leaf-only delete
restriction. Each save point is now a self-contained Vec<u8>
snapshot, independent of every other save point.

Public method signatures on SavePointManager are unchanged in this
commit so EditorState keeps compiling. The vestigial _original_bytes,
_original, and _base_state args are removed in the next commit
together with the call-site updates.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Wire integration changes (`buffer.rs` + `ui/savepoints.rs`)

**Files:**
- Modify: `src/editor/buffer.rs` — drop `clear_all` calls in length-changing paths; rewrite `restore_save_point`; remove `can_delete_save_point`; rename/invert one test; add 7 new tests
- Modify: `src/editor/savepoints.rs` — drop the now-vestigial `_original_bytes` / `_original` / `_base_state` args; remove `can_delete`
- Modify: `src/ui/savepoints.rs` — drop `can_delete: Vec<bool>` collection; remove the gate around the trash button

**Why these are bundled:** Removing the vestigial args from `SavePointManager` API breaks the call sites in `buffer.rs` until the call sites are updated. Similarly, removing `can_delete_save_point` from `EditorState` breaks the UI gate until the gate is removed. Bundling all three keeps the codebase compilable at the commit boundary.

- [ ] **Step 3.1: Add 7 new failing tests + invert 1 existing test in `src/editor/buffer.rs::tests`**

First, find and **delete** the existing `test_insert_clears_save_points` test (search for `fn test_insert_clears_save_points`). It will be replaced.

Then add the following tests inside `#[cfg(test)] mod tests { ... }`. Place them grouped together near the existing save-point tests (search for `fn test_save_point_create_and_restore`).

```rust
#[test]
fn test_insert_preserves_save_points() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data);

    editor.edit_byte(0, 0xFF);
    let id = editor.create_save_point("SP1".to_string());
    assert_eq!(editor.save_point_count(), 1);

    editor.insert_byte(0, 0xAA);
    assert_eq!(
        editor.save_point_count(),
        1,
        "save point must persist across insert"
    );

    // The save point still restores to its captured state.
    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0xFF, 0x01, 0x02, 0x03]);
}

#[test]
fn test_save_point_survives_byte_edit() {
    let data = vec![0x00, 0x01, 0x02];
    let mut editor = EditorState::new(data);

    let id = editor.create_save_point("SP1".to_string());
    editor.edit_byte(0, 0xAA);
    editor.edit_byte(1, 0xBB);

    assert_eq!(editor.save_point_count(), 1);
    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02]);
}

#[test]
fn test_save_point_survives_delete() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data);

    let id = editor.create_save_point("SP1".to_string());
    let _ = editor.delete_byte(1);
    assert_eq!(editor.working(), &[0x00, 0x02, 0x03]);

    assert_eq!(editor.save_point_count(), 1);
    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn test_save_point_survives_undo_redo_of_length_change() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data);

    let id = editor.create_save_point("SP1".to_string());

    editor.insert_byte(0, 0xFF);
    assert_eq!(editor.save_point_count(), 1);

    let _ = editor.undo();
    assert_eq!(editor.save_point_count(), 1);

    let _ = editor.redo();
    assert_eq!(editor.save_point_count(), 1);

    // Undo back to the snapshot's length and verify the save point still restores.
    let _ = editor.undo();
    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);
}

#[test]
fn test_restore_with_shorter_snapshot() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data);

    let id = editor.create_save_point("SP_short".to_string());

    editor.insert_bytes(2, &[0xAA, 0xBB, 0xCC]);
    assert_eq!(editor.len(), 7);

    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);
    assert_eq!(editor.len(), 4);
}

#[test]
fn test_restore_with_longer_snapshot() {
    let data = vec![0x00, 0x01, 0x02];
    let mut editor = EditorState::new(data);

    editor.insert_bytes(0, &[0xAA, 0xBB, 0xCC]);
    let id = editor.create_save_point("SP_long".to_string());
    assert_eq!(editor.len(), 6);

    let _ = editor.delete_byte(0);
    let _ = editor.delete_byte(0);
    assert_eq!(editor.len(), 4);

    assert!(editor.restore_save_point(id));
    assert_eq!(editor.len(), 6);
    assert_eq!(editor.working(), &[0xAA, 0xBB, 0xCC, 0x00, 0x01, 0x02]);
}

#[test]
fn test_restore_clamps_cursor_into_shorter_buffer() {
    let data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
    let mut editor = EditorState::new(data);

    // Snapshot at length 10.
    let id = editor.create_save_point("SP10".to_string());

    // Grow the buffer and move the cursor beyond the snapshot's length.
    editor.insert_bytes(10, &[0xFF; 10]);
    editor.set_cursor(15);
    assert_eq!(editor.cursor(), 15);

    // Restore to the 10-byte snapshot.
    assert!(editor.restore_save_point(id));
    assert!(
        editor.cursor() < editor.len(),
        "cursor must be clamped into the new (shorter) buffer"
    );
}

#[test]
fn test_restore_undo_round_trips_through_length_change() {
    let data = vec![0x00, 0x01, 0x02, 0x03];
    let mut editor = EditorState::new(data);

    // Snapshot at length 4.
    let id = editor.create_save_point("SP4".to_string());

    // Grow to length 7.
    editor.insert_bytes(2, &[0xAA, 0xBB, 0xCC]);
    assert_eq!(editor.len(), 7);

    // Restore to length 4 — emits a Replace history op.
    assert!(editor.restore_save_point(id));
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);

    // Undo: should regrow back to length 7 with the inserted bytes intact.
    assert!(editor.undo());
    assert_eq!(editor.len(), 7);
    assert_eq!(editor.working(), &[0x00, 0x01, 0xAA, 0xBB, 0xCC, 0x02, 0x03]);

    // Redo: back to the snapshot state at length 4.
    assert!(editor.redo());
    assert_eq!(editor.working(), &[0x00, 0x01, 0x02, 0x03]);
}
```

Also locate the existing `test_save_point_delete` test (search for `fn test_save_point_delete`) and **delete it** — its assertion that `can_delete_save_point` returns false for non-leaf save points is no longer valid. The new `test_delete_any_save_point` in `savepoints.rs` (added in Task 2) covers this behavior at the manager layer; the new `EditorState`-layer tests above cover restore behavior.

- [ ] **Step 3.2: Run the new tests; confirm they fail in the expected ways**

```bash
cargo test --lib editor::buffer::tests::test_insert_preserves_save_points 2>&1 | tail -10
cargo test --lib editor::buffer::tests::test_save_point_survives 2>&1 | tail -20
cargo test --lib editor::buffer::tests::test_restore_with 2>&1 | tail -10
```

Expected:
- `test_insert_preserves_save_points` fails — `save_point_count() == 0` after the insert (because `on_length_changed` still calls `clear_all`).
- `test_save_point_survives_undo_redo_of_length_change` fails — `apply_insert`/`apply_delete` still call `clear_all`.
- `test_restore_with_shorter_snapshot` and `test_restore_with_longer_snapshot` fail — `restore_save_point` still uses `EditOperation::Range` which can panic or yield wrong-length working buffer when lengths differ. Specifically: `Range`'s undo path is `self.working[*offset..*offset + old_values.len()].copy_from_slice(old_values)`, which requires `working.len() >= offset + old_values.len()`. In the shorter-snapshot test the buffer is 7 bytes after restore but the recorded `Range` op tries to restore the original 7-byte state into a 4-byte `working` — panic. Or, if the post-restore buffer is the 4-byte snapshot and `Range` records `old_values: 7-byte original` and `new_values: 4-byte snapshot`, the restore itself succeeds (writes 4 bytes) but undo trips on the length mismatch.
- `test_restore_undo_round_trips_through_length_change` fails — same root cause; the undo of a length-changing restore can't be expressed by `Range`.

Some tests may even panic instead of returning a fail assertion; that's fine — both indicate the implementation isn't done yet.

- [ ] **Step 3.3: Drop `clear_all` calls from length-changing paths in `src/editor/buffer.rs`**

Locate `fn on_length_changed`:

```rust
fn on_length_changed(&mut self, offset: usize, count: usize, is_insert: bool) {
    // Save points use absolute offsets — invalidate them all
    self.save_points.clear_all(&self.original);
    // Adjust bookmark offsets
    if is_insert {
        self.bookmarks.adjust_offsets_after_insert(offset, count);
    } else {
        self.bookmarks.adjust_offsets_after_delete(offset, count);
    }
    self.length_changed = true;
}
```

Remove the first two lines (the comment and the `clear_all` call), so the body becomes:

```rust
fn on_length_changed(&mut self, offset: usize, count: usize, is_insert: bool) {
    if is_insert {
        self.bookmarks.adjust_offsets_after_insert(offset, count);
    } else {
        self.bookmarks.adjust_offsets_after_delete(offset, count);
    }
    self.length_changed = true;
}
```

Locate `fn apply_insert`:

```rust
fn apply_insert(&mut self, offset: usize, values: &[u8]) {
    let count = values.len();
    self.working.splice(offset..offset, values.iter().copied());
    self.bookmarks.adjust_offsets_after_insert(offset, count);
    self.save_points.clear_all(&self.original);
    self.length_changed = true;
}
```

Remove the `self.save_points.clear_all(...)` line:

```rust
fn apply_insert(&mut self, offset: usize, values: &[u8]) {
    let count = values.len();
    self.working.splice(offset..offset, values.iter().copied());
    self.bookmarks.adjust_offsets_after_insert(offset, count);
    self.length_changed = true;
}
```

Locate `fn apply_delete` and do the same — remove only the `self.save_points.clear_all(...)` line:

```rust
fn apply_delete(&mut self, offset: usize, count: usize) {
    self.working.drain(offset..offset + count);
    self.bookmarks.adjust_offsets_after_delete(offset, count);
    self.length_changed = true;
    if !self.working.is_empty() {
        self.cursor = self.cursor.min(self.working.len() - 1);
    }
}
```

- [ ] **Step 3.4: Rewrite `restore_save_point` in `src/editor/buffer.rs`**

Locate `fn restore_save_point` and replace its body. The `EditOperation` variants are imported via `use super::history::EditOperation;` — confirm that's at the top of the file (it should already be there since `EditOperation::Range` is used in the existing body). Replace the function with:

```rust
/// Restore the buffer to a specific save point.
///
/// This operation is undoable — the entire restoration (including any
/// buffer-length change) is recorded as a single `EditOperation::Replace`
/// history entry.
///
/// Returns true if restoration was successful.
#[must_use = "returns whether the restore was successful"]
pub fn restore_save_point(&mut self, id: u64) -> bool {
    let Some(restored) = self.save_points.restore(id) else {
        return false;
    };

    let length_will_change = restored.len() != self.working.len();
    let old_values = std::mem::replace(&mut self.working, restored);

    if old_values != self.working {
        self.history.push(EditOperation::Replace {
            offset: 0,
            old_values,
            new_values: self.working.clone(),
        });
    }

    if length_will_change {
        self.length_changed = true;
    }

    self.cursor = self.cursor.min(self.working.len().saturating_sub(1));
    self.modified = self.working != self.original;
    true
}
```

Note the changes vs. the previous body:
- `self.save_points.restore(id, &self.original)` → `self.save_points.restore(id)` (drops the unused arg — Step 3.6 cleans up the manager signature to match).
- `EditOperation::Range` → `EditOperation::Replace`.
- New `length_will_change` capture before the swap (avoids using-after-move on `old_values`).
- New `length_changed = true` flag set when length differs.
- New cursor clamp.

- [ ] **Step 3.5: Remove `can_delete_save_point` from `EditorState`**

Locate `fn can_delete_save_point` in `src/editor/buffer.rs` (search for `fn can_delete_save_point`):

```rust
/// Check if a save point can be deleted
pub fn can_delete_save_point(&self, id: u64) -> bool {
    self.save_points.can_delete(id)
}
```

Delete the entire function. Step 3.7 removes the only UI caller; if there are no other callers (`grep -rn 'can_delete_save_point' src/`), the removal is safe.

- [ ] **Step 3.6: Clean up vestigial argument signatures in `src/editor/savepoints.rs`**

Three signature changes:

1. `pub fn new(_original_bytes: &[u8]) -> Self` → `pub fn new() -> Self`. Drop the arg.
2. `pub fn restore(&self, id: u64, _original: &[u8]) -> Option<Vec<u8>>` → `pub fn restore(&self, id: u64) -> Option<Vec<u8>>`. Drop the arg.
3. `pub fn clear_all(&mut self, _base_state: &[u8])` → `pub fn clear_all(&mut self)`. Drop the arg.

Also remove the `pub fn can_delete(&self, id: u64) -> bool` method entirely (no longer needed after Step 3.5).

Then update the test calls in `src/editor/savepoints.rs::tests` to match the new signatures: every `SavePointManager::new(&original)` becomes `SavePointManager::new()`, every `manager.restore(id, &original)` becomes `manager.restore(id)`. The `&original` literal can be removed from the test bodies if it's no longer referenced anywhere else in the test (most tests still use `&original` for other purposes; just stop passing it to `new` and `restore`).

After this step, the call site in `EditorState::new` (in `buffer.rs`) needs the matching update too:

In `src/editor/buffer.rs::new`:
```rust
let save_points = SavePointManager::new(&bytes);
```
becomes
```rust
let save_points = SavePointManager::new();
```

(`bytes` is still used immediately below in the `Self { working: bytes.clone(), original: bytes, ... }` initializer, so don't delete it.)

- [ ] **Step 3.7: Drop the trash-button gate in `src/ui/savepoints.rs`**

Locate the `let can_delete: Vec<_> = ...` binding (search for `let can_delete: Vec`). Remove the entire binding block. In its current form it's:

```rust
let can_delete: Vec<_> = save_points
    .iter()
    .map(|(id, _)| {
        doc.editor
            .as_ref()
            .map(|e| e.can_delete_save_point(*id))
            .unwrap_or(false)
    })
    .collect();
```

Remove that.

Then locate the trash-button section (search for `// Delete button`):

```rust
// Delete button (only for leaf)
if can_delete.get(idx).copied().unwrap_or(false)
    && ui
        .button("🗑")
        .pointer_cursor()
        .on_hover_text("Delete")
        .clicked()
{
    action_delete = Some(*id);
}
```

Replace with:

```rust
// Delete button (any save point can be deleted)
if ui
    .button("🗑")
    .pointer_cursor()
    .on_hover_text("Delete")
    .clicked()
{
    action_delete = Some(*id);
}
```

This removes the `can_delete.get(idx).copied().unwrap_or(false) &&` gate. The comment is updated to match the new behavior.

If `idx` is no longer used anywhere else in the loop body after this change, the `for (idx, (id, name)) in save_points.iter().enumerate()` loop can be simplified to `for (id, name) in save_points.iter()`. Check by searching for other uses of `idx` in the function.

- [ ] **Step 3.8: Run all tests; confirm they pass**

```bash
cargo build --release 2>&1 | tail -5
cargo test 2>&1 | tail -3
```

Expected:
- `cargo build --release`: succeeds.
- `cargo test`: all green. Total count is 207 (existing) − 2 (deleted: `test_insert_clears_save_points`, `test_save_point_delete`) − 1 (`test_compute_diff` deleted in Task 2) − 1 (`test_delete_leaf_save_point` deleted in Task 2 / replaced) + 3 (Task 1's Replace tests) + 8 (Task 3's new tests) + 2 (Task 2's new tests: `test_save_point_independent_of_subsequent_edits`, `test_delete_re_indexes_after_middle_removal`) = 216 tests passing. (The exact total depends on how many `test_save_point_*` tests are kept; the key is `0 failed`.)

- [ ] **Step 3.9: cargo fmt + clippy**

```bash
cargo fmt
cargo clippy --all-targets 2>&1 | tail -10
```

Expected: no diff after fmt; no new clippy warnings.

- [ ] **Step 3.10: Commit**

```bash
git add src/editor/buffer.rs src/editor/savepoints.rs src/ui/savepoints.rs
git commit -m "$(cat <<'EOF'
feat(editor): save points survive length changes; allow deleting any save point

- Drop save_points.clear_all calls from on_length_changed,
  apply_insert, apply_delete in EditorState. Save points are now
  self-contained snapshots (Task 2 of this refactor) and no longer
  invalidated by length changes.
- Rewrite restore_save_point to emit EditOperation::Replace so a
  length-changing restore round-trips through undo/redo as a single
  history entry. Clamp the cursor into the post-restore buffer.
- Drop can_delete_save_point from EditorState and the leaf-only
  gate from the side-panel UI; the trash button is now always
  available for every save point.
- Clean up vestigial _original_bytes / _original / _base_state args
  on SavePointManager methods (preserved through Task 2 to keep
  buffer.rs compiling, removed now that all call sites are updated).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: End-to-end verification

**Files:** none (manual verification only).

This task confirms user-visible behavior on a real file. The automated tests cover the data model and state transitions; this catches anything UI-shaped that the unit tests wouldn't surface (e.g. a render-path panic from an out-of-range cursor that the unit tests didn't drive).

- [ ] **Step 4.1: Build a release binary**

```bash
cargo build --release
```

- [ ] **Step 4.2: Open the bundled BMP and run the smoke checklist**

```bash
target/release/bend-rs assets/base_converted_glitched.bmp
```

(If the binary doesn't accept a file path argument, open the file via the in-app File menu instead.)

Walk through each of these and tick when verified:

- [ ] **a. Save point survives in-place edits.** Click into the hex grid, create "Save A" via the side-panel ➕ New button. Type a few hex characters in **Overwrite mode** (status bar shows "OVR"). "Save A" should still appear in the panel.
- [ ] **b. Save point survives Insert-mode typing (the original bug).** Toggle to Insert mode (status bar shows "INS"). Type 5–10 hex characters. "Save A" should still be in the panel.
- [ ] **c. Save point survives Backspace and Delete in Insert mode.** Still in Insert mode, press Backspace several times, then Delete. "Save A" stays.
- [ ] **d. Save point survives paste in Insert mode.** Copy some bytes (Cmd-C/Ctrl-C on a selection), then paste (Cmd-V/Ctrl-V) in Insert mode. "Save A" stays.
- [ ] **e. Restore to a save point with a different length.** Note the byte count in the status bar. Create "Save B" at the current (longer) length. Click Restore (↩) on "Save A". The byte count should drop back to the original length and the contents should match the captured state.
- [ ] **f. Undo and redo of a length-changing restore round-trip.** Press Cmd-Z/Ctrl-Z. Buffer should re-grow to the pre-restore state. Press Cmd-Shift-Z (or the redo shortcut). Buffer should shrink back to the snapshot state.
- [ ] **g. Delete a non-leaf save point.** Create "Save C" so the panel shows A, B, C in order. Click trash (🗑) on the *middle* save point ("Save B"). It disappears; A and C remain. Restore to A and to C and verify each still matches its captured state.
- [ ] **h. Effect-driven byte changes preserve save points.** Apply any effect from the Effects menu that mutates many bytes without changing length (most do). Save points should remain in the panel.

If any item fails, the implementation has a bug. Don't archive the change until every box ticks.

- [ ] **Step 4.3: Update the OpenSpec change tasks.md to mark all sub-items complete**

```bash
# After verifying everything passes, mark the OpenSpec task list as done:
$EDITOR openspec/changes/refactor-save-points-to-snapshots/tasks.md
```

Tick every `- [ ]` in `tasks.md` to `- [x]`. (The OpenSpec tasks.md is the authoritative checklist for the change; the plan.md is the detailed implementation guide that produced it.)

- [ ] **Step 4.4: Validate the OpenSpec change**

```bash
openspec validate refactor-save-points-to-snapshots --strict --no-interactive
```

Expected: `Change 'refactor-save-points-to-snapshots' is valid`. Any "Error while flushing PostHog" lines are unrelated telemetry noise — ignore.

- [ ] **Step 4.5: Final commit (status + tasks update)**

```bash
git add openspec/changes/refactor-save-points-to-snapshots/tasks.md
git commit -m "$(cat <<'EOF'
chore(refactor-save-points-to-snapshots): mark tasks complete

Manual smoke checklist passed; all automated tests green.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

The change is now ready to be reviewed and (when approved) archived via:

```bash
openspec archive refactor-save-points-to-snapshots
```

(Archiving moves the change to `openspec/changes/archive/YYYY-MM-DD-refactor-save-points-to-snapshots/` and updates the canonical `openspec/specs/hex-editor/spec.md` with the modified Save Points requirement. Run only once the work has actually shipped — out of scope for this plan.)

---

## Self-review notes

**Spec coverage.** Each scenario in `specs/hex-editor/spec.md` maps to a task:
- *Create save point* — Task 2 `test_create_save_point`; Task 4 step 4.2(a).
- *Save points persist across in-place byte edits* — Task 3 `test_save_point_survives_byte_edit`; Task 4 step 4.2(a).
- *Save points persist across length-changing edits* — Task 3 `test_insert_preserves_save_points`, `test_save_point_survives_delete`; Task 4 steps 4.2(b), (c), (d).
- *Save points persist across undo and redo* — Task 3 `test_save_point_survives_undo_redo_of_length_change`; Task 4 step 4.2(f).
- *Restore save point with matching length* — covered by the existing-and-still-passing `test_save_point_create_and_restore`.
- *Restore save point with different length* — Task 3 `test_restore_with_shorter_snapshot`, `test_restore_with_longer_snapshot`, `test_restore_clamps_cursor_into_shorter_buffer`, `test_restore_undo_round_trips_through_length_change`; Task 4 steps 4.2(e), (f).
- *Delete any save point* — Task 2 `test_delete_any_save_point`, `test_delete_re_indexes_after_middle_removal`; Task 4 step 4.2(g).

**Type/name consistency.** `SavePoint` field renamed `diff` → `bytes`. `EditOperation::Replace` named consistently across history.rs, buffer.rs, and tests. `SavePointManager::new`/`restore`/`clear_all` signatures finalize in Task 3 and the same signatures are used by callers in `EditorState`.

**No placeholders.** Every step has the actual code, command, or test to run. No "TBD"/"TODO"/"see above"/"similar to Task N".
