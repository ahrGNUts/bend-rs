# Design: Save points as self-contained uncompressed snapshots

## Decision context

Save points were reported as silently disappearing after byte edits. Investigation traced this to `SavePointManager::clear_all` being invoked from three length-changing paths in `EditorState`:

- `on_length_changed` — called by `insert_byte` / `insert_bytes` / `delete_byte`
- `apply_insert` — undo of delete, redo of insert
- `apply_delete` — undo of insert, redo of delete

The clearing is *intentional* under the current data model: each `SavePoint` stores `Vec<ByteChange>` keyed on absolute offsets, and any insert/delete shifts every following offset, invalidating the chain. The simple-but-blunt clear was the original design choice. Documented in `savepoints.rs::clear_all` and pinned by `test_insert_clears_save_points`.

Three approaches were considered for making save points robust to length changes.

### Option A — Full uncompressed snapshots (chosen)

Each `SavePoint` stores a full `Vec<u8>` of the buffer at creation time. Restore = swap working buffer for the snapshot. No chain, no diff math, delete-any-save-point trivially supported. Memory: ~1× buffer per save point.

### Option B — Length-aware edit-script diffs

Keep the chain, but make each diff a sequence of `Replace` / `Insert` / `Delete` operations rather than fixed-offset byte substitutions. Computing a diff between two byte buffers of different lengths requires a real diff algorithm (Myers or similar). Deleting a non-leaf save point requires *rebasing* the next save point's diff onto the new predecessor. Lower memory than A for small edits; comparable for large edits. Substantial implementation surface (diff algorithm + rebase logic + edge cases).

### Option C — Compressed full snapshots

Same data model as A, but each snapshot is zlib-compressed via `flate2`. Adds a compress/decompress step on save-point create/restore. Considered but ruled out after empirical measurement (see "Empirical evidence" below).

### Empirical evidence: why A and not C

Compression was initially recommended on the assumption of 5–10× ratio for "image-shaped data" and ~30–50 ms compression on a 5 MB buffer. A one-off benchmark (committed as `examples/bench_compression.rs`, then removed once it had served its purpose) measured those numbers on this machine for the formats this app actually edits — BMP, JPEG, GIF (PNG is read-only and was excluded from the test set):

| Format / size | level 1 ratio · compress time | level 6 ratio · compress time |
|---|---|---|
| BMP, 0.75 MB (glitched) | 1.21× · 5 ms | 1.29× · 17 ms |
| JPEG, 3.0 MB | 1.00× · 12 ms | 1.00× · 63 ms |
| JPEG, 4.7 MB | 1.02× · 20 ms | 1.02× · 107 ms |
| GIF, 0.81 MB | 1.00× · 3 ms | 1.00× · 16 ms |

JPEG and GIF compress to essentially 1.00× because their bulk is already compressed by their own encodings (DCT, LZW). Only BMP shows a real win, and only in the 20–30% range. Level 6 (default) costs roughly 5× the CPU of level 1 for a small additional ratio gain, and at 5 MB the per-save-point hitch (107 ms) is approaching perceptible.

At the 50 MB scale that motivated keeping the design simple, level-6 compression would cost ~1 second per save-point creation — a clear UI hitch — for a memory saving that's 0–30% depending on format. Level 1 is faster (~250 ms at 50 MB) but still gains nothing measurable on JPEG/GIF.

Conclusion: compression doesn't earn its keep on the actual supported formats. **Option A (uncompressed snapshots)** wins on:

- Predictable, format-independent memory cost (`N × buffer_size`).
- Zero per-save-point CPU hitch — creation and restore are bandwidth-bound clones.
- No new dependency. (`flate2` is not currently a direct dependency.)
- Simplest possible implementation: a save point is "a labeled `Vec<u8>`."

Option B's complexity (diff algorithm + rebase logic) was never justified once the simpler options were on the table.

## Data model

```rust
pub struct SavePoint {
    pub id: u64,
    pub name: String,
    /// Snapshot of the working buffer at creation time. Owned, uncompressed.
    bytes: Vec<u8>,
}

pub struct SavePointManager {
    save_points: Vec<SavePoint>,
    id_to_index: HashMap<u64, usize>,
    next_id: u64,
    // last_save_point_state is gone — no chain to maintain a base for.
}
```

Removed types: `ByteChange`, `compute_diff`, `last_save_point_state`.

## API surface

`SavePointManager`:

| Method | Old | New |
|---|---|---|
| `new` | `new(original_bytes: &[u8])` | `new()` — no chain base needed |
| `create` | `(name, current_state) -> id` (computes diff internally) | `(name, current_state) -> id` — clones `current_state` into `bytes` |
| `restore` | `(&self, id, original) -> Option<Vec<u8>>` — replays diffs from original | `(&self, id) -> Option<Vec<u8>>` — clones the snapshot's `bytes` |
| `delete` | `(id) -> bool` — leaf only | `(id) -> bool` — any save point; re-indexes `id_to_index` |
| `can_delete` | `(id) -> bool` — leaf only | **removed** (UI always shows trash button) |
| `clear_all` | `(base_state: &[u8])` — for length changes; resets chain base | `clear_all()` — only used on file load; no base to track |

`EditorState` (in `src/editor/buffer.rs`):

| Method | Change |
|---|---|
| `on_length_changed` | drop `self.save_points.clear_all(...)` line; bookmark adjustment and `length_changed` flag remain |
| `apply_insert` | drop `self.save_points.clear_all(...)` line |
| `apply_delete` | drop `self.save_points.clear_all(...)` line |
| `restore_save_point` | drop `original` arg from `save_points.restore(...)`; emit `EditOperation::Replace` instead of `Range`; clamp cursor; set `length_changed = true` when length differs |
| `can_delete_save_point` | remove (or have it return `id_to_index.contains_key(&id)`) |

## New `EditOperation::Replace` variant

`EditOperation::Range` enforces equal-length old/new vectors. Once snapshots can have a different length than the current working buffer, `restore_save_point` cannot use `Range` to record the swap as a single undoable history entry.

```rust
EditOperation::Replace {
    offset: usize,
    old_values: Vec<u8>,
    new_values: Vec<u8>,
}
```

Semantics:

- **Apply forward (redo):** splice `working[offset..offset + old_values.len()]` with `new_values`.
- **Apply backward (undo):** splice `working[offset..offset + new_values.len()]` with `old_values`.

When `old_values.len() == new_values.len()`, `Replace` and `Range` are equivalent in effect. We add `Replace` rather than relax `Range`'s invariant because:

- `Range` carries useful intent: "in-place edit, no length change." Existing call sites (`replace_bytes`, `replace_all_bytes`, `apply_effect_batch`) all rely on equal length and benefit from the typing.
- A new variant has zero blast radius on existing edit paths.

`restore_save_point` always uses `Replace` (even when lengths happen to match), since the restore is conceptually a wholesale swap rather than an in-place edit.

## Restore semantics

When `restore_save_point(id)` is invoked:

1. Get a fresh `Vec<u8>` from the target snapshot (`new_values = save_point.bytes.clone()`).
2. Capture `length_will_change = new_values.len() != self.working.len()` before moving the vector.
3. `let old_values = std::mem::replace(&mut self.working, new_values)`.
4. If `old_values != self.working`, push `EditOperation::Replace { offset: 0, old_values, new_values: self.working.clone() }` to `self.history` (matching the existing direct-push pattern in `restore_save_point` rather than going through `record_operation`, to preserve current restore semantics around `edit_generation`).
5. If `length_will_change`, set `self.length_changed = true`.
6. Clamp cursor: `self.cursor = self.cursor.min(self.working.len().saturating_sub(1))`.
7. Update `self.modified = self.working != self.original`.

Undoing a restore that grew the buffer shrinks it back via the `Replace` op; undoing one that shrank it grows it back. No special-case logic needed — the `Replace` variant handles both directions uniformly.

Pre-existing behavior preserved: this code path does NOT bump `edit_generation` (the existing restore doesn't either). If consumer caches downstream of restore turn out to need invalidation, that's a separate change and applies symmetrically to the pre-existing restore behavior.

## Bookmarks and restore

Bookmarks store offsets. After restoring to a save point with a different length, some bookmark offsets may fall outside the new buffer.

**Decision:** Do NOT auto-adjust bookmarks on restore. Rationale:

- Bookmarks are user-curated annotations. Silently moving or deleting them on restore would be surprising.
- This matches the existing behavior for undo of a long edit sequence: bookmarks added partway through are not removed by undo.
- Bookmark rendering paths already need to tolerate offsets at or past the buffer end (e.g. during the brief window between an edit and the next frame). Out-of-range bookmarks become invisible until the buffer grows back; nothing crashes.

Out of scope for this change. If user feedback later asks for it, a follow-up change can add explicit "clamp bookmarks on restore" or "drop out-of-range bookmarks on restore" semantics.

## Memory cost at scale

Memory is `N × buffer_size` for N save points. Working examples:

- 1 MB file, 10 save points → 10 MB of save-point memory.
- 5 MB file, 10 save points → 50 MB.
- 50 MB file, 10 save points → 500 MB.
- 50 MB file, 20 save points → 1 GB.

The editor's dual-buffer architecture (`original` + `working`) already costs 2× the file size before any save points exist, so save-point cost is additive on top of that baseline. For typical workloads (single-digit MB files, modest save-point counts), additional cost is in the low tens of MB. Users routinely working with 50+ MB files and many save points will want to keep counts modest.

If the 50+ MB workload turns out to be common in practice, the next step would be **on-disk save-point persistence** — write each snapshot to a temp file, hold a path + checksum in memory, decompress/load on restore. That's a significant change (file lifecycle, error handling for I/O failures, cleanup on app exit) and is explicitly out of scope here.

## Migration

None. Save points have always been in-memory only; no on-disk format. Existing files load as before.

## Test changes

- `src/editor/buffer.rs::tests::test_insert_clears_save_points` — rename to `test_insert_preserves_save_points`, invert assertion.
- New: `test_save_point_survives_byte_edit`, `test_save_point_survives_insert`, `test_save_point_survives_delete`, `test_save_point_survives_undo_redo_of_length_change`, `test_delete_non_leaf_save_point`, `test_restore_with_shorter_snapshot`, `test_restore_with_longer_snapshot`, `test_restore_clamps_cursor`.
- New in `src/editor/history.rs` (or wherever EditOperation tests live): `test_replace_round_trips_for_equal_length`, `test_replace_round_trips_for_grow`, `test_replace_round_trips_for_shrink`.
- New in `src/editor/savepoints.rs`: `test_delete_middle_save_point_re_indexes`, `test_save_point_independent_of_subsequent_edits`.

## Out of scope

- Persisting save points across application restarts (no on-disk format).
- On-disk save-point storage to handle very large files (see "Memory cost at scale").
- Compression of snapshots (investigated, ruled out — see "Empirical evidence").
- Bookmark adjustment on restore (see "Bookmarks and restore" above).
- Restoring as a *forking* history operation (today, restore is recorded linearly into history as a `Replace`; this preserves the existing model).
