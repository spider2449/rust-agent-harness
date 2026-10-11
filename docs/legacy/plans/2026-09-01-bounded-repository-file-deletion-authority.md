# Task 159 — Bounded Repository File Deletion Authority

## Scope

Accept ADR 0017 as the authoritative, docs-only contract for one bounded
repository file deletion authority. Preserve the separate worktree, index, and
history/ref authority boundaries established by ADRs 0010 through 0016.

## Work

1. Verify the clean starting worktree and required baseline commit.
2. Read Task 158 research and the relevant accepted ADRs and guardrails.
3. Add and accept `docs/adr/0017-bounded-repository-file-deletion-authority.md`.
4. Validate documentation-only scope, commit one coherent change, push it, and
   require CI success for the exact pushed head.

## Explicit exclusions

No Rust or Desktop implementation, Cargo/dependency/version/release change,
Trusted Profile implementation, generic filesystem or process authority,
rename/move, staging, commit, or network Git work is included.

## Validation and completion gates

- `git diff --check` passes.
- Only the task plan and ADR 0017 are changed.
- No Rust, Cargo, dependency, version, or release files are changed.
- One docs-only commit is created and pushed normally.
- The local head equals the pushed branch head.
- CI passes for that exact head before implementation work begins.
