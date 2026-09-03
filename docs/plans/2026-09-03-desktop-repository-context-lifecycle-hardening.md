# Task 175C — Desktop Repository Context Lifecycle Hardening

## Scope and historical boundary

Task 175B's historical Desktop attempt remains unclassified. The observed
`Chat failed` result does not prove why the post-start turn failed, and this
task does not claim to repair that attempt. No destructive live validation is
run, and `repo.rename-file` authority semantics, schemas, permissions, and ADR
0018 are unchanged.

## Architecture facts established by Task 175B

- Desktop repository observers and mutation tools are built from one immutable
  `DesktopRepository` snapshot and canonical root.
- Codex receives that root as its verified thread/start working directory.
- Selecting a repository rotates `repository_generation` and clears
  conversation state, but does not reconnect an already connected runtime.
- `send_chat` rejects stale repository or model generations before a model turn.
- A connection that began for an old repository could previously publish after
  selection changed, exposing the old runtime and registry as connected state.

## Lifecycle correction

Repository selection is rejected while connection setup is active, including a
second check after the folder dialog returns. Connection publication also
revalidates the captured repository and connection generations. A stale
runtime is shut down and is never published; the current state remains
disconnected and requires a fresh connection. A connected runtime is never
rebound to a new repository generation.

## Frontend alignment and diagnostics

The frontend disables repository selection while connecting and disables Send
when the connected runtime reports reconnect-required repository or model
state. Backend generation guards remain authoritative.

When `RAH_LIVE_EVIDENCE_PATH` is unset, no evidence file is touched. When it
is set, bounded Desktop records correlate repository selection, connection
start/publication/rejection, thread start, tool request/start/finish, and
terminal completion/failure. Records contain generations and a process-local
salted SHA-256 repository fingerprint, never the absolute path, prompt,
credentials, environment, authority objects, executable paths, or raw backend
errors. The fingerprint is process-stable correlation metadata only; it is not
a durable repository identity or authority token. Closed failure stages
distinguish pre-turn stale rejection, thread/turn start failure, model/runtime
failure, tool dispatch failure, and terminal/disconnect failure.

## Deterministic regressions

Coverage proves the connecting-selection rule, publication generation guard,
opaque fingerprint behavior, stale send guard, and existing tool lifecycle
contracts. Runtime publication rejects an old captured repository generation,
shuts down the newly constructed runtime, and leaves the current repository
requiring a fresh connection. Desktop turns continue to start a fresh Codex
thread/session while the long-lived runtime retains its immutable bridge/tool
snapshot.

## Validation boundary

This task does not certify the historical failed run, perform a destructive
live rerun, alter authority, or begin the v0.14 milestone audit. After exact
head CI passes, the next validation is Task 175D's non-mutating two-repository
context test.
