# Task 331-A — Remembered Workspace Staged-File and Reparse-Ancestor Hardening

## Scope

Correct the two accepted Task 331 persistence defects without changing the
schema, authority boundary, startup behavior, dependencies, or frontend:

- reread and validate the closed temporary file after close, including exact
  equality with the requested serialized catalog;
- validate every existing component of the supported local storage-root chain,
  with fresh checks before load, save, and delete effects and before native
  replacement.

## Tests

- tamper the closed temporary file with invalid bytes and with a valid but
  different catalog, proving the old final catalog remains unchanged;
- accept an ordinary local ancestor chain and reject unsupported prefixes;
- use a test-only ancestor metadata classification seam to prove an ordinary
  final directory is rejected when an ancestor is reparse-marked, with no
  catalog, temporary, or coordination mutation.

## Validation and delivery

Run the requested formatting, check, focused remembered-workspace tests,
clippy, and diff checks. Commit with
`fix: harden remembered workspace persistence staging`, push `origin master`,
and verify a clean worktree, `HEAD == origin/master`, and passing exact-head
CI.
