# Task Coordination System

This directory contains task files organized by domain. Each file tracks tasks in a specific area.

## Task Files

| File | Domain | Description |
|------|--------|-------------|
| [backend.md](backend.md) | Core Logic | Data structures, parsing, algorithms |
| [frontend.md](frontend.md) | UI/UX | egui components, user interaction |
| [infrastructure.md](infrastructure.md) | DevOps | Build, test, CI/CD, docs, release |

## Task Format

Each task file has three sections:

```markdown
## In Progress
- [ ] Task currently being worked on

## Pending
- [ ] Task waiting to be started

## Completed
- [x] Finished task
```

## How to Use

### Claim a task
Move it from `## Pending` to `## In Progress`

### Complete a task
1. Change `- [ ]` to `- [x]`
2. Move it to `## Completed`

### Add a new task

1. Choose the correct file based on the task's domain:
   - **backend.md** — Core logic, data structures, parsing, algorithms, non-UI functionality
   - **frontend.md** — UI components, user interaction, visual presentation, egui widgets
   - **infrastructure.md** — Build system, testing, CI/CD, documentation, release

2. Choose the correct section within the file:
   - Add under an existing `### From Phase N:` heading if it fits an existing phase
   - Create a new heading if the task belongs to a new category (e.g., `### From Phase 19: New Feature`)

3. Use the task format:
   ```markdown
   - [ ] PHASE.NUMBER Description of task
   ```
   - `PHASE` is the phase number the task belongs to
   - `NUMBER` is a sequential index within that phase
   - For tasks not tied to a phase, use a category prefix (e.g., `T.1` for testing tasks)

4. If unsure which file a task belongs to, ask: "Is this about what the app *does* (backend), what the user *sees* (frontend), or how the app is *built/shipped* (infrastructure)?"

#### Examples

```markdown
# Backend task (new parser feature)
- [ ] 7.7 Parse BMP ICC color profile section

# Frontend task (new UI element)
- [ ] 9.6 Add expand/collapse all button to structure tree

# Infrastructure task (new CI step)
- [ ] 18.6 Add automated screenshot tests to CI pipeline

# Testing task (category prefix)
- [ ] T.4 Add unit tests for undo/redo stack operations
```

#### Numbering rules

- Check the last number used in the target phase before adding a new task
- If Phase 7 has tasks up to 7.6, the next task is 7.7
- For category-prefixed tasks (e.g., `T.1`, `T.2`), increment from the last used number
- Never reuse a number, even if a task was deleted

## Quick Commands

```bash
# See all pending tasks
grep -h "^\- \[ \]" tasks/*.md

# See all in-progress tasks
grep -h "^\- \[ \]" tasks/*.md | head -20

# Count remaining tasks per file
for f in tasks/*.md; do echo "$f: $(grep -c '^\- \[ \]' $f)"; done
```
