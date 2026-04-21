## Context

`BendApp` has grown to 20 fields across four concerns: document/editor, UI state, I/O plumbing, persistent config. Every UI and app-layer function takes `&mut BendApp` and reaches across concerns. The goal is to make the ownership boundaries explicit and narrow UI signatures without changing behavior.

## Goals / Non-Goals

- **Goals:**
  - Group fields by concern into named substates.
  - Narrow every UI `show()` signature to only what it needs.
  - Preserve all current behavior, including recent commits around selection (2cef50a), ASCII borders (6b4fdaf), and color caching (460e6bf).
  - Land in one PR (bounded blast radius).
- **Non-Goals:**
  - Not extracting an MVC layer or introducing message-passing.
  - Not making UI code egui-agnostic.
  - Not changing user-visible behavior, settings schema, or on-disk format.
  - Not breaking up `EditorState` — that's already cohesive.

## Decisions

- **Four substates, not two or ten.** Splitting `BendApp` into `{ doc, ui, io, config }` aligns with how the code actually reads/writes state. Two is too coarse (UI vs. everything else isn't a real boundary — I/O receivers belong neither to UI nor to the document). Ten is premature — we don't have evidence that `context_menu_state` and `search_state` need to be separated yet.
- **`header_protection` lives on `DocumentState`, not `AppConfig`.** It's a runtime toggle affecting edits, not a persisted preference. (The persisted default lives on `AppSettings` as `default_header_protection`.)
- **`preview` stays on `DocumentState`.** Image preview is derived from the document; treating it as UI would force UI functions to take doc too.
- **UI functions take the narrowest substate they need, not a bundle struct.** This makes the data dependencies visible in signatures and is how the borrow checker stays happy. A `Context<'a>` bundle would re-create the god-struct problem at a smaller scale.
- **`pending_hex_scroll` goes on `UiState`, not `DocumentState`.** It's a UI scroll command, not document content.
- **Alternatives considered:**
  - *Leave `BendApp` as-is; extract traits.* Rejected: traits don't shrink the struct or narrow UI signatures.
  - *Use `Rc<RefCell<_>>` substates.* Rejected: unnecessary runtime cost and borrow-panic risk when plain `&mut` works.
  - *Event-sourced architecture (commands + reducers).* Rejected as overkill for a ~6K-line desktop app; would be a rewrite, not a refactor.

## Risks / Trade-offs

- **Risk:** mechanical rename touches ~15 files; easy to introduce a silent logic change in a field move. **Mitigation:** migrate one substate at a time (`IoState` first — smallest and most isolated), compile and manually smoke-test between each.
- **Risk:** some UI functions genuinely need two or three substates (e.g. hex_editor needs doc + ui + config). Signatures grow wider parameter lists. **Trade-off accepted:** explicit parameter lists are better than hidden coupling; if a specific function gets 5+ params we'll bundle them per-function.
- **Risk:** `DocumentState::is_offset_protected` needs `cached_sections` and `header_protection`, both on `DocumentState` — fine. But callers currently in `search_dialog.rs:256` assume `&BendApp`. **Mitigation:** pass `&DocumentState` to those callers.

## Migration Plan

1. Add the four substate structs with `impl Default` (no `BendApp` changes yet).
2. Move fields from `BendApp` into `IoState`; update only the app-layer files that touch those fields.
3. Repeat for `AppConfig`, then `UiState`, then `DocumentState`.
4. At each step: `cargo build` + manual smoke test (open file, edit, undo, save point, export).
5. Final step: rename `BendApp` field accesses in every UI file in one pass.

**Rollback:** each substate extraction is an independent commit; revert any one if it breaks.

## Open Questions

- Should `AppColors` caching (currently on `BendApp` per commit 460e6bf) move with `UiState` or become a true per-frame local? **Leaning:** keep on `UiState` — it's a cache and has a natural home there.
- Does `pending_hex_scroll` need to persist across frames, or can it be consumed the frame it's set? **Current:** persists via `Option::take()` on the next `show()`. Keep behavior; UI owns the field.
