# Task 171 — Bounded repository file rename/move implementation

ADR 0018 is implemented as a separate private host-owned
`RepositoryFileRenamePolicy` and the narrow `repo.rename-file` Tool.

The closed request is `source_path`, `destination_path`,
`expected_source_file_sha256`, and `expected_source_file_byte_length`.
The authority accepts one clean HEAD-tracked regular file and an absent file
under an existing validated parent in the same selected repository.

On Windows the effect uses one `MoveFileExW` call with no flags. On Linux it
uses one `renameat2` call with `RENAME_NOREPLACE`; unsupported Unix platforms
fail closed because ordinary `rename` cannot guarantee no replacement.

Deterministic coverage includes same-directory rename, cross-directory move,
malformed and collision requests, changed-source revalidation, exact bytes,
and one-attempt uncertain outcomes. The shared per-repository lease is reused.

Trusted Profile composition, Generic Bridge integration, Desktop integration,
automatic staging/commit, live validation, directory/untracked movement,
overwrite, case-only Windows rename, and copy/delete fallback are deferred.

Platform-gated path/reparse cases continue to rely on the shared logical-path
and filesystem validation helpers; Windows live certification is not part of
this task.
