# Task 225B — Local-branch observation scope correction

Date: 2026-09-06
Status: COMPLETE

Task 225B is a narrow post-Task-225 ADR 0020 conformance correction. Task 225
was already committed at `ae06ad9ace22018ea3e64e845228871bd37d880c` and its
exact-head CI had passed before this independent source audit.

The correction is limited to the private `RepositoryBranchCreationPolicy`
observation used by branch admission and post-effect verification:

- Git-owned `for-each-ref` is explicitly restricted to `refs/heads/`.
- Internal snapshot terminology describes local heads rather than all refs.
- A finite `MAX_LOCAL_HEADS` parsed-count bound is enforced alongside the
  existing finite `HostExecutionPolicy` output bound of 512 KiB stdout.
- Odd fields, malformed local-head ref/OID encodings, count overflow, and
  output overflow fail closed before mutation.
- Exact, prefix, ASCII-case-fold, packed-head, tag, and remote-tracking
  semantics remain covered by deterministic tests.

The fixed `update-ref` mutation, CAS, hooks, reflog, authority boundaries, and
private/no-Tool status are unchanged. Task 226 remains not started.

## Final evidence

- Implementation commit: `7c64a7ed3db567418d3a8dba0a369d744eb50c09`
- Exact-head CI: `34021544398` PASS
- Focused branch tests: 15 PASS
- `rah-tools`: 163 PASS
- Workspace: PASS
- Formatting: PASS
- Clippy: PASS
- Diff-check: PASS
- Metadata: 13 packages, all 0.18.0, edition 2024
- Cargo changes: none
- Dependency drift: none
- Task 226: not started
