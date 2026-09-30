# ADR 0031 — Bounded untracked-file correction authority

Status: Accepted for Task 484 implementation

## Context

Task 483 selected the first v0.33 product capability after repeated ordinary development evidence: a newly created untracked file cannot be corrected before staging through the existing repository Tools. ADR 0012 makes `repo.patch` HEAD-tracked-only, ADR 0014 makes `repo.edit-files` HEAD-tracked-only, and ADR 0013 makes `repo.create-file` create-only. Those contracts remain unchanged.

## Decision

Introduce one ordinary Tool, `repo.edit-untracked-file`, bound by the trusted host to the currently active admitted repository and its fixed Git executable. It uses the existing one-file replacement engine under a distinct untracked target class. `PermissionLevel::Execute` is an outer dispatch gate; trusted profile composition and the Tool's private policy supply the repository authority. The Tool is not HostExplicit; the exact HostExplicit set remains 11.

One request names one repository-relative logical path, an expected complete-file SHA-256 and byte length, and one or more bounded unique exact literal replacements. The expected digest and length are the whole-content optimistic-concurrency precondition. Replacement matching is strict UTF-8, byte-preserving outside the requested literals, and rejects NUL, invalid UTF-8, duplicate/overlapping matches, and oversized material. No arbitrary full-file write, patch language, or binary mutation is authorized.

The target must already be a regular file inside the selected repository, absent from current HEAD and every index stage, and non-ignored. The host verifies an exact `git ls-files --others --exclude-standard` observation of the literal target path. Staged additions, intent-to-add, paths deleted from index but present in HEAD, tracked files, ignored files, nested repositories, links/reparse points, hard-linked aliases, and path escapes are rejected. Linked worktrees use the active member's own HEAD, index, root, and identity.

The Tool shares the repository mutation lease, root/path/file-object identity checks, bounded preimage and postimage checks, same-parent exclusive temporary, immediate target and Git-state revalidation, and one native replacement attempt of the existing single-file engine. It verifies the exact postimage and repository state afterward. A proven pre-commit refusal has no target effect; a failed replacement is known no effect only after proof; lost post-observation is uncertain. The Tool never retries, stages, commits, changes refs, chooses a repository, or grants general filesystem or process authority. Temporary cleanup follows the same conservative policy as `repo.patch`.

## Consequences

The file remains untracked after success. The result reports `status`, `changed`, `uncertain`, a redacted reason, and the logical target path. `repo.patch`, `repo.edit-files`, and `repo.create-file` keep their accepted target classes. This decision adds no version bump, release, provider-specific path, or HostExplicit route.
