# Task 324-D — Codex App-Server Ownership Evidence Triage

Status: in progress. The zero-model ownership diagnostic has passed on the
current candidate; closure remains pending the final documentation-source
rerun, standard validation, push, and exact-head CI.

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

The diagnostic was run through `scripts/codex-live-gate.ps1` with an
ephemeral-auth-file copy, isolated temporary Codex home, MCP `0`, plugins
disabled, apps disabled, and no model request. The required baseline was
`codex-cli 0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

The host was Windows 10 Professional build `19045`, `64-bit`; Git was
`2.54.0.windows.1`. The selected source was the host override to the exact
certified binary. The Desktop diagnostic used a fresh two-repository fixture,
admitted and activated repository A, then performed one real Desktop Connect
and normal Disconnect. It performed one direct `CodexRuntime::connect` and
normal `shutdown` control afterward. No model, Stage, Unstage, Commit, or
other repository effect path was called.

The sanitized Desktop census was:

| census | certified executable identity | app-server | `--stdio` | old regex | old observer intersection |
| --- | ---: | ---: | ---: | ---: | ---: |
| before Connect | 0 | 0 | 1 unrelated candidate | 0 | 0 |
| after Connect | 0 | 1 new candidate | 2 total candidates | 1 | 0 |

The final candidate run used source SHA `68bd9c9` and diagnostic process PID
`1408`. The one new candidate had executable basename `codex.exe`,
`executable_path_present=true`,
`executable_identity_matches_certified_binary=false`,
`command_line_contains_app_server=true`,
`command_line_contains_stdio=true`, and
`command_line_matches_old_task_324_regex=true`. Thus the old observer lost
the child on the certified full executable identity predicate; command-line
matching was not the failing predicate. The corrected observer found exactly
one new app-server candidate. Its parent PID was `1408`, the diagnostic test
process PID, establishing direct spawn by the test process for this production
path; no launcher parent was observed. The unrelated pre-existing candidates were
not treated as owned and were not touched.

The Desktop result was `connected`, and the test observed
`Connected { runtime: Arc<CodexRuntime>, ... }` with retained runtime ownership
before Disconnect. Normal Disconnect published `NotConnected`. The direct
runtime control produced the same one-new-candidate shape: basename `codex.exe`,
path present, certified identity false, app-server true, stdio true, and old
regex true. This independently rules out a Desktop-only integration failure.

In that final run, Desktop PID `39628` and standalone PID `25488` were the new
app-server candidates; both had parent PID `1408`. After each Disconnect/shutdown,
the attributable new PID set was empty. The
fresh fixture root was deleted successfully after the test-only cleanup
released the Desktop persistence connection; no unrelated Codex process was
killed or modified.

The current evidence does not support Outcome C or D. Successful protocol
initialization plus retained Desktop `Arc<CodexRuntime>` proves the runtime
was live and published. It also does not support an OS-wide limitation:
Win32/CIM returned all required sanitized fields and consistently identified
the app-server by command tokens. The relevant Windows environment difference
from Task 324-C is therefore not implicated; Windows 10 is not newly claimed
as a full certification environment by this triage.

- Windows edition/build/architecture: Windows 10 Professional / `19045` / `64-bit`.
- Codex version/SHA-256: `0.149.0` /
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Desktop before/after census: certified identity `0 -> 0`; app-server `0 -> 1`;
  stdio `1 -> 2`; old regex `0 -> 1`; old observer intersection `0 -> 0`.
- Old observer exact failure: zero new PID because the certified executable
  identity predicate was false.
- Predicate that failed: full certified `ExecutablePath` identity; app-server
  and exact `app-server --stdio` command predicates were true.
- Parent relationship: Desktop and standalone app-server candidates were direct
  children of diagnostic process PID `1408`.
- Standalone runtime census: same one-new-candidate shape and same old observer
  miss; direct `CodexRuntime` initialization succeeded.
- Desktop Connect/Disconnect: `connected`, retained runtime `1`, then
  `NotConnected`.
- Corrected ownership proof: one new sanitized app-server plus successful
  owned runtime initialization and shutdown.
- Cleanup: both attributable PID sets empty and fresh Task 324-D root absent.

## Validation and closure

The first live attempt exposed only a test-fixture cleanup issue: Git object
files were read-only. A test-only bounded attribute/deletion cleanup was added;
the next attempts narrowed a remaining SQLite lock to the managed Desktop
`Persistence` connection, which was released through a test-only replacement
before app teardown. The final documentation-source diagnostic rerun and the
validation results below remain to be recorded.

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

## Outcome

OUTCOME A — PROCESS EVIDENCE OBSERVER DEFECT CONFIRMED AND CLOSED, pending the
final documentation-source rerun and CI closure. The defect is the old observer's
assumption that CIM `ExecutablePath`, after string normalization, must equal
the selected certified executable path. The app-server command identity and
owned runtime lifecycle are real and stable; the test-only evidence method
uses identity-aware sanitized census plus successful owned runtime
initialization/shutdown.

Task 325 remains NOT AUTHORIZED in every outcome.
