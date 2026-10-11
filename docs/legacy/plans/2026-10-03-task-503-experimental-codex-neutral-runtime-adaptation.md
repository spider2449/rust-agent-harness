# Task 503 — experimental Codex neutral runtime adaptation

## Checkpoint and scope

Starting HEAD: `904a781f928bc8fa5d592f5db92bd58cdcede400`.
Clean local checkout and GitHub master independently matched this SHA. Starting
exact-head CI supplied by the task: `37082592253`, PASS. The deleted mirror is
not contacted; its existing local remote configuration is left unchanged.
RAH remains 0.33.0; certified/preferred Codex remains 0.157.1; v0.34 capability
is NONE SELECTED. No release, tag, version, baseline or Desktop migration.

ADR decision: **ADR-B**. Accepted ADRs 0001/0002 own neutral runtime and private
adapter translation; 0003/0006 preserve Tool authority and private dynamic
routing; 0021 supplies existing authorized dispatch; 0032 owns typed failures.
This experimental, parallel implementation does not replace production
composition or promote the seam. No new ADR or crate dependency edge.

## Implementation and conformance plan

CONTRACT-MINOR additions: an explicit live-only host request mode, host-issued
turn admission with SessionId/event receiver/lifetime, and an experimental
connection-scoped weak port. The host owner reserves execution atomically with
revocation checks, rejects overlapping turns, allows distinct calls within a
turn, and waits for admitted effects. Dropped response futures do not replay
or roll back host-owned execution. Buffered legacy fake ports remain supported
only through their existing mode; they cannot silently masquerade as live ports.

Changed files: `rah-runtime/src/experimental.rs`, `lib.rs`,
`experimental_host.rs`; `rah-runtime-codex/src/experimental.rs`,
`experimental/tests.rs`, `lib.rs`, plus crate-private helper visibility in
`runtime.rs` and definition-snapshot/output helpers in `bridge.rs`; this report.
No Desktop source change.

- Factory: adapter-owned executable/provider configuration; neutral factory
  validates and creates an instance using existing certified process startup.
  Fixture injection is private and test-only.
- Runtime: closed capabilities, existing bounded model/list catalog mapping,
  conversation opening, liveness and owned router shutdown.
- Discovery: inference selector maps to descriptor ID; untrusted display data
  stays excluded; complete catalog means advertised, not inference guaranteed.
  Malformed/incomplete discovery preserves the existing typed failure.
- Default: RuntimeDefault maps privately to CodexModelConfig::Inherit. Explicit
  selection is validated and verified against the effective thread response.
- Identity: host ConversationId, host-issued operation SessionId, and private
  provider thread/turn strings remain distinct. No provider identifier appears
  in a neutral identifier or event variant.
- Continuation: native send resumes the retained private thread and starts a
  new turn; replay starts a fresh thread using the supplied complete text.
  Native continuation before any thread fails explicitly.
- Turn/events: reuse existing restricted production translation into neutral
  events; merge host live events without exposing RPC sequencing. Stream guard
  invalidates routes and initiates interruption on premature drop.
- Tool port: adapter receives only immutable definitions and request interface,
  never registry, repository, permission or HostExplicit mutation handles.
- Live delivery: host constructs Requested/Started/Finished or typed failure
  before/after actual admission/execution, not synthetic provider notifications.
- Revocation: host owner withdraws before teardown waits; retained weak ports
  reject before authorized dispatch. Already admitted effects retain ownership.
- Correlation/concurrency: private session/call keys deduplicate identical
  provider requests, reject conflicting replay and route replies to each waiting
  RPC ID. Distinct calls use owned concurrent workers, not artificial serialism.
- Cancellation: stop new submissions before interrupt; Stopped requires provider
  interrupted terminal confirmation. No rollback or replay claim.
- Shutdown: mark instance unavailable, invalidate routes/leases, stop and join
  router workers, clean existing app-server transport; retained handles reject.
- Failures: existing sanitized RuntimeDiagnostic and typed RuntimeFailure sources;
  Codex errors remain downcastable, host dispatch errors remain separately typed.
- Parity: existing restricted thread/input/event and Tool output helpers are
  shared; production constructors/bridge/model preflight stay in use unchanged.
  Existing production tests must run alongside new neutral fixture tests.

## Serial validation

Freeze source after formatting and fixture completion. Use an isolated target
where practical and retain command logs. Do not edit during Cargo execution.

1. Phase A: rah-runtime experimental contract tests.
2. Phase B: rah-runtime-codex production and neutral adapter conformance tests.
3. Phase C: host live-port lifecycle/revocation tests.
4. Only after zero focused failures: fmt, workspace check/test, warnings-denied
   full-feature/all-target Clippy, diff check; canonical Windows Desktop gate;
   frontend/static suites, permission inventory, metadata, static/executable
   HostExplicit verification (required count exactly 11).

Source was formatted with `cargo fmt --all`, then frozen before Phase A.
No source edits occurred during either validation command. Both Cargo processes
have exited; the final process census found no cargo.exe/rustc.exe process.
Isolated build target: `F:/temp/rah-task503-target` (previously nonexistent).

| Gate | Exact command / outcome |
| --- | --- |
| Phase A | `cargo test -p rah-runtime experimental::`: PASS, 4 passed, 0 failed, 0 ignored; filtered integration harnesses ran zero tests |
| Phase B | `cargo test -p rah-runtime-codex`: FAIL, exit 101, compiler E0282; zero adapter tests executed |
| Phase C | NOT RUN: stopped after Phase B |
| Full workspace / canonical Desktop / frontend / permission inventory / executable HostExplicit | NOT RUN: focused prerequisites did not pass |
| Final `git diff --check` | PASS |

Exact failure: `crates/rah-runtime-codex/src/experimental.rs:548:9`,
`let mut workers = JoinSet::new();`. Rust cannot infer the response type of
`JoinSet<((SessionId, String), _)>` at the earlier `response.clone()` use in
the completion branch. The probable narrow correction is an explicit worker
result type; it was **not applied** under the requested stable-source stop rule.
This is an implementation compile failure, not evidence of a larger contract,
provider identity, authority, or production behavior failure.

Preserved logs:

- `F:/temp/rah-task503-phase-a.log`; SHA256
  `ED80B45F8D48B775B8D0239BC1FFFD86BECC30F48485A1E0773DCFFD85389D8D`.
- `F:/temp/rah-task503-phase-b.log`; SHA256
  `62F4312A1EAB799095E0BC5C8D18C9F7BF7FFB2D7B45F4C350327510F3BA1BC9`.
- Source SHA256 inventory: `F:/temp/rah-task503-stopped-source-sha256.json`.

All Codex factory/runtime/conversation/turn, model selection/discovery,
concurrent-call, live-event, retained-handle, cancellation/shutdown, typed-error
and production-parity conformance claims remain **UNVERIFIED**. The new fixture
tests exist but did not execute. Phase A proves only the existing four neutral
fake-adapter contract tests on the changed rah-runtime source, not the new host
scope tests (reserved for Phase C).

HostExplicit static source inspection found exactly the established 11
HostInvocationKind variants and 11 host_kind entries. No Desktop source changed.
Executable HostExplicit verification and production regression gates were not
run; passing production behavior is not claimed.

**E — DETERMINISTIC VALIDATION FAILED**

## Authority/security and next task

Production composition and authority source are unchanged. The proposed
experimental host scope uses existing Tool authorization and policy and exposes
no registry/repository/permission/HostExplicit mutation handle to adapters.
Its execution/revocation guarantees are not certified: the required tests remain
unexecuted. No new crate dependency, ADR, baseline or authority expansion.

Recommended next task: authorize a narrow Task 503 compile correction and
continue its deterministic conformance/validation, preserving this failed
snapshot and evidence. Review lifecycle/event-ordering and early-provider-request
races before claiming conformance. No Desktop migration is authorized here.

No commit, GitHub push or new exact-head CI: classification E does not authorize
publication. HEAD remains the starting SHA. Final worktree is intentionally
dirty, preserving five modified tracked source files and four new files (two
adapter implementation/test files, host scope implementation, and this report).
No staging, reset, clean, stash, tag, release or mirror operation occurred.

## Task 503A continuation — final current disposition

Historical Task 503 failure evidence above is retained. Task 503A verified all
eight preserved source hashes before correcting only experimental.rs:548 to
`let mut workers: JoinSet<((SessionId, String), Value)> = JoinSet::new();`.
The single spawn future already returns this tuple on every branch. The earlier
response.clone() in tokio::select! needs its unresolved Value type before later
spawn inference. The outer join result uses JoinError; the worker does not
return a Result. Typed host/runtime failures and existing response mapping were
unchanged. E0282 is resolved without a neutral contract or semantic change.

Phase B exact command `cargo test -p rah-runtime-codex`: compiled, exit 0;
119 passed, 0 failed, 1 ignored. All six new adapter tests executed and passed:
configured factory/instance, identity/discovery/default, continuation/replay,
owned stream/control, Tool correlation/live events, retained handles,
cancellation/shutdown and typed error propagation. Concurrent distinct calls
produce two effects and correctly correlated duplicate replies. Cancellation
preserves completed uncertain Tool output without replay. Stale adapter handles
produce no Tool effect. Existing production regressions also passed.

Phase C `cargo test -p rah-runtime experimental_host::`: exit 0;
3 passed, 0 failed, 0 ignored. Retained ports reject requests/admission after
revocation and owner drop before dispatch; no extra effect occurs. Already
admitted host effects survive response-future drop, with typed dispatch sources.

Resumed full gates: fmt/check PASS; workspace tests PASS (1042 passed,
0 failed, 24 ignored, exit 0); Clippy FAIL, exit 101. Exact command:
`cargo clippy --workspace --all-targets --all-features -- -D warnings`.
Two `clippy::collapsible_if` errors at experimental.rs:435:9 and :564:17.
These pre-existing WIP nested conditionals are a deterministic lint/build gate
failure. Source stayed stable, proven by all eight validation hashes. No patch
or retry followed. Closure git diff --check passed.

Canonical Desktop test gate, frontend/static suites, Tauri inventory, metadata,
later explicit executable HostExplicit checks and separately planned post-gate
production parity: NOT RUN after stop. The canonical PrepareOnly helper build
passed before workspace tests; it does not certify the Desktop test gate.
Static HostExplicit remains exactly 11 variants and 11 routing entries.
Workspace Desktop tests passed, without substituting for remaining gates.

No Desktop composition/model-preflight/baseline change, 0.160.0 admission,
authority expansion, new dependency/ADR or Desktop migration. Task 503A changes
only one source annotation and these two reports. No commit/push/new CI; HEAD
remains `904a781f928bc8fa5d592f5db92bd58cdcede400`. WIP remains dirty and
unstaged. Source hashes, historical failure logs and patches remain preserved.
No cargo/rustc process remained after Clippy exited.

Final current Task 503 classification:
**E — LATER DETERMINISTIC VALIDATION FAILED**.

Exact new log paths, hashes and next-task boundary are recorded in
[Task 503A](2026-10-03-task-503a-joinset-type-inference-correction.md).
Next task requires separate authorization for the two lint diagnostics and
validation resume. No Desktop migration.

## Task 503B continuation — final current disposition

Historical stop classifications above remain preserved evidence. Task 503B
corrected only the two collapsible_if sites with Clippy's canonical let chains;
the accepted JoinSet annotation remains unchanged. Evaluation order, state
removal, mutable lookup, cancellation, response retention and typed errors are
preserved. No neutral redesign or Desktop migration occurred.

Clippy PASS (exit 0). Canonical Windows Desktop gate PASS: 329 passed,
0 failed, 20 ignored, exit 0. Five frontend suites and all actual frontend
JavaScript syntax checks PASS. Tauri inventory matches at 47 throughout.
Metadata PASS (13 packages). HostExplicit static variants/routes = 11/11;
canonical and explicit executable host_allowlist_is_exact PASS (1 passed).
Post-gate production/neutral parity: 33 passed, 0 failed, 1 ignored, all exit 0.
The previously successful Phase B/C/workspace evidence remains preserved;
workspace tests were not rerun. Formatting and diff checks pass after correction.

**A — CODEX ADAPTER CONFORMS TO NEUTRAL RUNTIME CONTRACTS**

Task 503 implementation is complete. No dependency, ADR, baseline, model
preflight, production composition or authority expansion. Classification A
authorizes the coherent Task 503 commit and normal GitHub master push; exact
commit/push/CI identity will be recorded in the final publication return and
external evidence, avoiding a second commit solely to record its own SHA.
Full corrections, commands and evidence paths:
[Task 503B](2026-10-03-task-503b-clippy-correction-and-final-validation.md).
Next task, if separately authorized: assess Desktop migration. It is not begun.