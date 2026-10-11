# Task 182 — Bounded Repository Directory Creation Authority ADR

## Scope

Accept ADR 0019 for the new narrow, host-owned `repo.create-directory`
authority selected by Task 180 and established by Task 181. The authority is
limited to exactly one absent ordinary directory leaf at one validated logical
repository-relative path under one already-existing validated parent.

## Starting state

- Starting `HEAD`: `53b039f84c78990b341bf461bbef45d3739b624a`
- Starting `origin/master`: `53b039f84c78990b341bf461bbef45d3739b624a`
- Working tree: clean
- Release state preserved: v0.14.0 release commit, annotated tag, and tag
  object remain immutable.

## Authority decision

Task 181 is authoritative. ADR 0019 records its decision for a private,
opaque, host-constructed `RepositoryDirectoryCreationPolicy` and the future
`repo.create-directory` Tool. The closed request is only `path`. The operation
requires an existing ordinary parent, rejects every existing final object,
creates one ordinary directory with one bounded native attempt, preserves Git
metadata without creating placeholders, and classifies verified success,
known no-effect, precondition failure, and uncertainty conservatively.

The ADR preserves separate file creation, deletion, rename/move, content,
index, commit/history, read, and Execute authorities; the shared repository
mutation lease; immediate identity revalidation; no replay; no rollback or
delete compensation; host-owned repository/runtime generation; future Desktop
refresh and reviewed-commit revocation; and Trusted Profile composition only
from host-supplied authority.

## Work completed

- Created `docs/adr/0019-bounded-repository-directory-creation-authority.md`.
- Kept the change documentation-only.
- Did not implement `repo.create-directory` or
  `RepositoryDirectoryCreationPolicy`.
- Did not modify Rust, frontend source, Cargo files, dependencies, versions,
  profile schema, ToolRegistry, Desktop behavior, release metadata, tags, or
  GitHub Releases.

## Validation and completion gates

- `git diff --check` passes.
- Only the ADR and this Task 182 plan are changed.
- No source, frontend, Cargo, dependency, version, or release changes exist.
- One documentation-only commit is created and pushed normally.
- Local `HEAD` equals `origin/master` after the push.
- Exact-head CI passes before Task 182 is declared complete.

## Proposed next task

Task 183 — Core bounded `repo.create-directory` implementation: implement the
narrow core authority and deterministic low-level contract using accepted ADR
0019 as the source of truth. Do not begin it automatically from Task 182.
