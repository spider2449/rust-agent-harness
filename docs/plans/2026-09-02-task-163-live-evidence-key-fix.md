# Task 163 — Live evidence request-key correction

## Scope

Correct only the bounded live evidence extraction in the Codex bridge to read
the canonical `repo.delete-file` request fields. Add deterministic coverage and
record the second live attempt in the Task 163 evidence document.

## Boundaries

- Preserve `RepositoryFileDeletionPolicy`, ADR 0017, tool schemas, authority,
  execution semantics, Desktop authority, and deletion behavior.
- Keep live evidence opt-in, bounded, and redacted.
- Task 163 remains incomplete because the second live attempt failed its raw
  byte equality precondition before deletion.

## Validation and delivery

- Run the focused runtime test and the full workspace gates.
- Inspect the diff and Git state.
- Commit and push the coherent change.
- Verify CI succeeds for the exact pushed head.
