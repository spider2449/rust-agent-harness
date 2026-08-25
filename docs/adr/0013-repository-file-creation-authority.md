# ADR 0013 — Repository File Creation Authority

Status: Accepted

## Context

ADR 0012 grants bounded replacement authority only for one existing clean
HEAD-tracked worktree file and explicitly defers new files. Coding workflows
need a safe new source/test/configuration-file path without generic filesystem
writing or Git-state authority.

## Proposed decision

Authorize a private host-owned `RepositoryFileCreationPolicy` and
canonical `repo.create-file`, subject to the complete Task 084 contract in
`docs/RAH_REPOSITORY_FILE_CREATION_CONTRACT.md`. It is one exclusive
create-new attempt for one bounded UTF-8 non-executable regular file at one
validated repository-relative path in a symbolic host-authorized repository.

The target is absent from HEAD, index, and worktree. The existing parent is a
real, identity-verified non-link/non-reparse directory under the canonical root.
The capability rejects ignored targets, Git metadata, submodules, sparse
unsupported paths, all link/reparse traversal, and Windows namespace aliases.
It never stages, overwrites, appends, creates directories, deletes, renames, or
changes permissions. A repository lease, immediate revalidation, exclusive
native creation as commit point, exact post-observation, redacted outcomes, and
no replay after a possible effect are mandatory.

`PermissionLevel::Execute` remains an outer gate only. The trusted profile
additively binds this implemented capability to the existing symbolic repository
resource under `profile_version = 1`; it cannot create authority.
Generic Tool Bridge production behavior remains unchanged.

## Implementation evidence

Task 085C accepted this ADR after deterministic implementation and audit
coverage in `crates/rah-tools/src/repository_create_file.rs`,
`crates/rah-tools/src/native_repository_create.rs`, and their tests. The audit
record is `docs/plans/2026-08-25-repository-file-creation-integration-audit.md`.
Task 086 completed trusted-profile and Generic Tool Bridge composition
validation; Task 087 completed the certified live Codex invocation. Neither
enlarges this ADR.

## Consequences

This differs from ADR 0012: replacement changes bytes at an existing tracked
pathname; creation allocates a persistent new pathname and can leave a partial
file after commit. Therefore this ADR does not amend ADR 0012. It authorizes no
implementation until Accepted and separately tasked with deterministic Windows/
Ubuntu coverage.

## Deferred scope

Multiple files, mkdir, overwrite, append, binary files, chmod/executable
creation, deletion, rename, Git add/commit/ref operations, arbitrary workspace
writes, generic shell/process/network authority, transactions, rollback, crash
recovery, and automatic replay remain deferred.
