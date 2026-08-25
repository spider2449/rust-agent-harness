# Task 094C — Multi-file policy integration and ADR acceptance

Status: Complete

## Implemented public boundary

`rah-tools` exports `REPOSITORY_EDIT_FILES_TOOL_NAME` (`repo.edit-files`) and
`RepositoryMultiFileEditTool`. The constructor accepts only host-selected Git
executable and repository root and creates the crate-private
`RepositoryMultiFileMutationPolicy`; model input cannot choose authority,
absolute paths, or native temporary paths.

The Tool definition has `PermissionLevel::Execute` as its outer runtime gate.
Its closed `targets` schema accepts one through four targets with logical path,
SHA-256, byte-length, and one through sixteen replacements. JSON Schema keeps
logical-character limits while the private parser remains authoritative for all
UTF-8 byte, aggregate, and postimage bounds.

Tool output is intentionally closed: preflight failures produce only
`{"status":"invalid_target"}` or `{"status":"precondition_failed"}`. Native
outcomes produce `status` plus deterministic host-ordered logical effects with
only `path` and `committed_verified`, `unchanged_verified`, `not_attempted`, or
`uncertain` state. No reason, host path, temporary, Git metadata, OS detail,
retry, or rollback field is exposed.

## Integration and audit basis

The wrapper delegates parsing, preflight, ordering, replacement planning,
native commit attempts, and classification to the retained private engine. It
does not duplicate or reinterpret those rules. Direct tests prove ToolRegistry
registration, definition lookup, and ordinary dispatch without registry changes.

The implementation audit confirms host-bound repository authority; closed
request parsing; original-snapshot exact replacement semantics; complete
preflight/revalidation; lexical host order; one native attempt per target;
known-no-effect, partial, and uncertain stopping classifications; no rollback,
retry, replay, staging, history/ref, or network Git; shared repository lease;
and index/HEAD/ref preservation. It makes no cross-file atomicity claim.

`repo.patch` remains the released one-file backward-compatible capability;
`repo.edit-files` is separate wider existing-file authority. `repo.create-file`
remains separate exclusive creation authority and no missing target is created.

## Deferred work

Trusted Profile production behavior, profile schema/version, symbolic resource
binding, effective composition, static inventory, and CLI support remain
unchanged (`profile_version = 1`). Generic Tool Bridge production code is also
unchanged: aliases, permission handling, registry dispatch, dedupe,
cancellation/disconnect, and no replay remain generic. Task 095 owns full
profile-to-bridge composition evidence. Task 094C runs no Codex live gate;
live certification remains separately gated on Windows `codex-cli 0.149.0`.

## Evidence

Windows direct Tool tests cover construction, Execute definition, one/four
target success, host order, redaction, known no-effect, partial, uncertain,
and no retry. Existing Task 094A/094B deterministic evidence covers native
Windows `MoveFileExW`, Unix same-directory rename/mode preservation, Git
fixtures, unmerged/gitlink rejection, and no ambient Git identity. Ubuntu CI
will exercise the complete workspace validation on the pushed exact head.
