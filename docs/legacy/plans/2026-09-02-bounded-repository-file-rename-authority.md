# Task 170 — Bounded Repository File Rename/Move Authority

## Scope

Accept ADR 0018 as the new orthogonal, host-owned authority for moving exactly
one eligible clean HEAD-tracked regular file between two explicit logical
paths in one selected repository. Preserve the existing worktree, creation,
deletion, index, and commit/history boundaries.

## Work

1. Verify the supplied clean exact-head baseline and read the v0.14 roadmap,
   Task 169 research, ADRs 0010–0014, 0016, and 0017, plus relevant mutation
   implementations.
2. Add and accept `docs/adr/0018-bounded-repository-file-rename-move-authority.md`.
3. Keep the change documentation-only: no Rust, Cargo, dependency, version,
   Desktop, Trusted Profile, bridge, Git, or live-validation work.
4. Validate, commit one coherent docs-only change, push normally, and require
   exact-head CI success before Task 171 implementation begins.

## Contract preserved

The accepted ADR must require host-owned repository and generation binding,
exact source preimage validation, destination nonexistence and parent/path
security checks, immediate independent revalidation, one native same-volume
no-replace rename attempt, deterministic postconditions, conservative outcome
classification, unchanged Git/index state, and no replay or rollback after an
uncertain effect. Case-only Windows renames, directories, untracked sources,
cross-repository movement, overwrite, and copy/delete fallback remain out of
scope.

## Validation and completion gates

- `git diff --check` passes.
- Only this plan and ADR 0018 are changed.
- No Rust, Cargo.toml, Cargo.lock, dependency, version, or release files are
  changed.
- One docs-only Task 170 commit is created and pushed normally.
- Local `HEAD` equals the pushed branch head.
- CI passes for that exact head before Task 171 starts.
