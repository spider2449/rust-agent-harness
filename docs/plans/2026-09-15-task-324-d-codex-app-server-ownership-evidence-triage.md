# Task 324-D — Codex App-Server Ownership Evidence Triage

Status: in progress. This document is the task evidence record and will be
closed only after the zero-model diagnostics, cleanup, validation, and exact
head CI are complete.

## Authoritative checkpoint

- Current master: `6b28e31fbb870167cc62f5c1cd4218dec9ea2238`
- Direct parent: `6563a581d7ab6fafddb9c45d3175d31971bb522c`
- Task 324-C source and final head: `6b28e31fbb870167cc62f5c1cd4218dec9ea2238`
- Task 324-C exact-head CI: `34918680794` — PASS
- Task 324-C result: BLOCKED — host-driven Windows live certification failed
- Failure: Desktop `connect_codex` returned success for active repository A,
  while the external `Win32_Process` probe observed zero newly created Codex
  app-server PIDs. No Stage, Unstage, Commit, branch/ref, push, fetch, pull,
  or model effect was reached.
- Task 325: NOT AUTHORIZED.
- Published release: RAH v0.25.0, source
  `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag object
  `ea3c31aaf5190b632d7ef86387f7aff6004ae664`, GitHub Release `387406579`.

## Scope and safety boundary

This is process-ownership evidence triage, not a Task 324-C rerun. The
diagnostic uses a fresh disposable fixture only for Desktop repository
admission/activation, one Desktop Connect/Disconnect, and one direct
`CodexRuntime::connect`/shutdown control. It sends no model request and does
not call Stage, Unstage, Commit, branch/ref, patch, edit, create, delete,
rename, push, fetch, or pull paths. It records no absolute paths or full
command lines and never kills a process found by global enumeration.

## Production source audit

`ProcessTransport::start` in `crates/rah-runtime-codex/src/process.rs`:

1. Resolves the executable, including Windows native executable resolution.
2. Verifies `--version` against the pinned `codex-cli 0.149.0` value.
3. Generates and validates the captured app-server schema.
4. Spawns the resolved executable with exactly `app-server --stdio`, piped
   stdin/stdout/stderr, and `kill_on_drop(true)`.
5. Stores the `tokio::process::Child` in `ProcessTransport` together with the
   pipes and stderr task.

The transport is moved into the `AppServerConnection` task. The connection is
stored in `CodexRuntime`; `CodexRuntime::shutdown` sends the owned transport's
shutdown command and awaits the connection and bridge lifecycle tasks.

Desktop `connect_codex` wraps the successful runtime in `Arc<CodexRuntime>`,
places that `Arc` in `PendingConnectedPublication`, and atomically publishes
it in `ConnectionState::Connected { runtime, ... }` while holding lifecycle
coordination. `disconnect_codex` extracts that same `Arc`, calls
`runtime.shutdown().await`, then publishes `NotConnected`. Therefore a
successful normal Desktop Connect cannot return after the published runtime
has already been dropped. `ConnectionState::Connected` is not itself a native
PID proof, but it is retained runtime ownership; successful initialization is
also an app-server protocol liveness proof.

## Observer under audit

The old Task 324-C helper queried `Get-CimInstance Win32_Process` and required
both `ExecutablePath -eq [IO.Path]::GetFullPath($selected)` and
`CommandLine -match 'app-server\s+--stdio'`. It returned only matching PIDs.
Task 324-D adds a test-only sanitized census that retains only PID, parent PID,
executable basename, path-present, certified-binary identity, app-server,
stdio, and old-regex booleans. It does not persist native paths or command
lines. It also compares the process identity after filesystem item
normalization and case-insensitive Windows spelling normalization.

## Live evidence

To be completed after the diagnostic commit is tested through the certified
Codex gate:

- Windows edition/build/architecture:
- Codex version/SHA-256:
- Desktop before/after census:
- Old observer exact failure:
- Predicate that failed:
- Parent relationship:
- Standalone runtime census:
- Desktop Connect/Disconnect:
- Corrected ownership proof:
- Cleanup:

## Validation and closure

To be completed:

- deterministic classifier tests;
- `cargo fmt --check`;
- `cargo check --workspace`;
- `cargo test --workspace -- --test-threads=1`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `git diff --check`;
- `cargo metadata --no-deps --format-version 1`;
- `cargo build -p rah-desktop --release`;
- frontend syntax/authority/membership tests;
- Tauri permission test;
- focused Task 321/320/318/315 regressions;
- exact-head CI and clean `HEAD == origin/master`.

Final outcome must be exactly one of Outcome A, B, C, or D. Task 325 remains
NOT AUTHORIZED in every outcome.
