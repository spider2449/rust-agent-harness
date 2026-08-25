# ADR 0014 — Bounded multi-file repository edit authority

Status: Accepted

## Context

ADR 0012 authorizes one clean tracked-file replacement through private `RepositoryWorktreeMutationPolicy`. ADR 0013 authorizes one exclusive new-file creation. v0.9 needs a coherent small edit over existing files without granting generic filesystem write or collapsing distinct commit/failure models.

## Decision

`repo.edit-files`, backed by separate private host-owned `RepositoryMultiFileMutationPolicy`, binds one canonical non-bare repository and may replace complete postimages for one through four existing clean HEAD-tracked regular strict-UTF-8 files. Host owns repository, Git executable, lease, limits, canonical identities, temporary names, and native calls. `PermissionLevel::Execute` remains outer gate only.

Each target supplies logical relative path, expected complete SHA-256, expected byte length, and one through sixteen exact literal replacements. Every match resolves exactly once against original snapshot; duplicate/overlap reject, adjacency is permitted, one deterministic postimage results. There is no legacy single-replacement form, regex, glob, line/range, or unified-diff semantics.

Before any persistent target mutation, acquire existing shared lease; validate complete request and host repository; safely resolve/reject aliases; prove target tracked-clean status; capture images/identities/raw index/HEAD/refs/Git observations; check all preconditions; compute bounded postimages; prepare host temporaries; and immediately revalidate every repository, target, temporary, and Git invariant. Failure before first native replacement causes zero repository-content mutations by this call.

Host commits ascending lexicographic UTF-8 byte order of canonical logical repository-relative paths; model cannot choose order. Native replacement is each target's sole commit point and is never retried. Per-file native behavior does not make multiple files atomic or transactional.

Outcomes are `ok`, `invalid_target`, `precondition_failed`, `failed_known_no_effect`, `partial_effect`, and `uncertain`. `partial_effect` requires a verified committed prefix and verified original/not-attempted state for every remaining target, with redacted inventory values `committed_verified`, `unchanged_verified`, and `not_attempted`. Any unproven effect or observation is `uncertain`.

Cancellation/disconnect before first commit may be known zero effect only when proven. Afterwards it never implies rollback: safe bounded observation yields `partial_effect` only with full proof, otherwise `uncertain`. No automatic rollback, retry, replay, prefix continuation, or recovery journal is authorized.

Preserve/check raw index bytes, HEAD, and refs before, during, and after operation. No staging or Git history/ref/network action occurs. Share existing lease with `repo.patch`, `repo.create-file`, `host.git.stage`, and `host.git.unstage`.

Trusted Profile remains version 1: additive closed `repo.edit-files` symbolic executable/repository binding is statically non-effectful and effectively publishes into a fresh registry only on complete success. No root path is model-visible. Generic Tool Bridge changes are unnecessary: existing aliases, Execute enforcement, registry dispatch, dedupe, cancellation/disconnect, translation, and no replay are capability-agnostic.

Windows retains canonical identity checks, reparse/junction rejection, and existing one-attempt `MoveFileExW` replacement. Sharing, filter-driver, and failed post-observation cases are uncertain; `WRITE_THROUGH` is not durability. Unix uses descriptor-relative no-follow validation, same-directory same-filesystem temporaries, mode preservation, and per-file rename. Failed observation after rename is uncertain; no cross-file atomicity or durability claim exists.

## Authority distinction

This does not widen ADR 0012: one-file replacement has a simpler authority/failure model, while several targets add deterministic ordering and partial-prefix semantics. It is not ADR 0013: exclusive creation can retain a partial file and has heterogeneous commit/cleanup behavior.

## Consequences

Exclude creation, deletion, rename, directories, chmod/mode changes, binary edits, generic write, shell/process authority, staging, commit/history/refs/network, rollback, replay, journal, OS sandboxing, and cross-file transaction semantics. `repo.patch` stays supported for smaller one-file authority; `repo.create-file` is not a target form.

Acceptance is supported by Task 094A deterministic all-target preflight and shared-lease evidence, Task 094B native fault, partial/uncertain, Git-invariant, Windows, and Unix evidence, and Task 094C direct host-constructed Tool, output-redaction, and generic ToolRegistry evidence. Task 095 completed Trusted Profile v1 and Generic Tool Bridge composition; Task 096 completed Windows live Codex certification using exactly `codex-cli 0.149.0` and emitted `RAH_REPO_EDIT_FILES_LIVE_OK`. The implementation remains non-transactional: it has no rollback, retry, or replay.

## Alternatives rejected

### Widen `RepositoryWorktreeMutationPolicy`

Rejected: it silently grants multi-target/partial-prefix authority under a policy designed for one target.

### Call it a transaction or add automatic rollback

Rejected: cross-file filesystem atomicity is not proven; rollback is new mutation authority that can overwrite concurrent work or fail.

### Mix `repo.create-file` targets

Rejected: exclusive creation adds partial-write residue, heterogeneous commit points, and cleanup ambiguity.

### Generic patch, diff, range, or regex input

Rejected: it increases content-selection ambiguity without improving bounded native replacement.

### New PermissionLevel or bridge branch

Rejected: private host policy is the authority and generic Execute bridge transport already applies.
