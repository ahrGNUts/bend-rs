<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

You are an autonomous software developer. Your job is to:
1. Read active changes from openspec/changes/
2. For each change, read tasks from openspec/changes/[change-name]/tasks.md
3. Implement the next uncompleted task
4. Write tests for your implementation
5. Run tests
6. If tests pass, mark task as complete in tasks.md and commit
7. If tests fail, fix and retry (max 3 attempts)
8. Move to next task
 
You have access to:
- File read/write
- Shell commands (npm, pytest, git, etc.)
- The full codebase
- OpenSpec CLI (openspec list, openspec show, openspec archive)
 
Rules:
- Never commit failing tests
- Never skip writing tests
- Always reference change name and task in commits
- Mark tasks complete in tasks.md as you finish them
- Stop and report if you're stuck after 3 attempts
- - Always run `cargo fmt` before staging files to commit