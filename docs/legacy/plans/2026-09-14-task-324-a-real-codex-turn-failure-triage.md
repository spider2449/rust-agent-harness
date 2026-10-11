# Task 324-A - RAH v0.26 Real Codex Turn Failure Triage

Date: 2026-09-14
Outcome: A - EXTERNAL / GENERIC CODEX LIVE PATH BLOCKED

This is a live-failure triage and test-harness diagnostic correction. It is
not v0.26 certification, production implementation, release preparation, or
Task 325 authorization.

## Authoritative checkpoint

- Task 324 was blocked at source `3d4e645ca1019a9e0ac19f5e3a23cbe503329ec1`.
- Current master at task start: `61be071dbd5c18334c9226374231f7699ac91ea1`.
- Task 324 documentation commits: `9a0afa8426eaa346534fabc500de408d25bdddec`
  and `61be071dbd5c18334c9226374231f7699ac91ea1`.
- Task 324 exact-head CI: `34841216237`, PASS.
- Task 323 remains READY for Windows live certification.
- Task 325 was and remains NOT AUTHORIZED.
- Published release remains RAH v0.25.0: source
  `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, annotated tag object
  `ea3c31aaf5190b632d7ef86387f7aff6004ae664`, GitHub Release `387406579`.

## Phase 1 - unchanged generic baseline probes

Both probes used the exact approved gate values: Codex `0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, reasoning effort `medium`, isolated temporary Codex home,
MCP/plugins/apps disabled, code mode disabled, and
`ephemeral-auth-file-copy` authentication. No credentials, auth contents,
Codex home path, or native repository path is recorded here.

The primary command was:

```powershell
& .\scripts\codex-live-gate.ps1 -Version '0.149.0' -ExpectedSha256 '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00' -Model 'gpt-5.6-terra' -ReasoningEffort 'medium' -Command { cargo run -p rah-runtime-codex --example live_fs_read_bridge }
```

Result: FAIL, exit code 1. The binary and SHA matched. The real runtime
started and the model request started. The safe Agent event sequence was:

```text
Started -> ModelRequestStarted -> ModelDelta x5 -> Completed
```

The five model deltas combined to `RAH_FS_READ_OK`. No `ToolRequested`,
`ToolStarted`, or `ToolFinished` occurred. App-server shutdown completed.
The example failed with `missing ToolFinished output`.

The existing optional probe was then run once with the same gate:

```powershell
& .\scripts\codex-live-gate.ps1 -Version '0.149.0' -ExpectedSha256 '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00' -Model 'gpt-5.6-terra' -ReasoningEffort 'medium' -Command { cargo run -p rah-runtime-codex --example live_echo_bridge }
```

Result: FAIL, exit code 1. Its safe sequence was:

```text
Started -> ModelRequestStarted -> ModelDelta(safe refusal text) -> Completed
```

The model response said the echo call could not run because code-mode host was
disabled. Requested, started, and finished echo counts were all zero.

Classification: Outcome A. The directly proven condition is that the approved
live model/tool environment completes ordinary model turns without selecting
the advertised dynamic tool. This is an external/approved-live
configuration/model-tool availability block. The evidence does not prove an
authentication rejection, a RAH runtime dispatch defect, a Desktop defect, or
an event-observation defect. No full Task 324 rerun and no Desktop A-turn
diagnostic were run after this failed prerequisite.

## Phase 2 - Windows harness lint correction

The initial Windows Desktop clippy command identified exactly four test-only
Task 324 findings: three `needless_borrow` calls and one `drop_non_drop` on a
Tauri `State`. The correction removes the three redundant borrows and removes
the non-semantic `drop(state)` statement. No broad lint suppression was added.

The corrected harness passes:

```text
cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings: PASS
```

## Phase 3 - terminal-aware observer correction

The old sequential activity-first wait was replaced inside the ignored
Windows test module with a shared ordered event log for `chat_event` and
`activity_event`, plus one global 180-second deadline. It processes both
streams as they arrive and immediately handles Chat Started, Chat Failed,
Chat Cancelled, Chat Completed, and each required tool lifecycle event.

The observer now reports safe bounded stages including send-chat rejection,
thread/session start failure, runtime failure or disconnect, completion without
the required tool, wrong or invalid tool lifecycle, and terminal timeout with
the last observed stage. Existing private `append_live_evidence` records are
read only through an allowlisted failure-stage mapping. A test-owned temporary
evidence path is removed after each live turn; no credential-bearing data is
printed or persisted.

The required `fs.read` lifecycle and marker checks remain strict. A prose-only
completion cannot pass. The full certification test remains ignored by default
and still requires `RAH_RUN_V026_MULTI_REPO_LIVE=1`.

Deterministic observer tests cover immediate Chat Completed failure without a
tool, immediate Chat Failed handling, and acceptance of exactly the complete
`fs.read` lifecycle. All three pass.

## Phase 5 - Desktop A-turn diagnostic

Not executed. Phase 1 did not pass, so no Desktop Connect, repository
admission/activation, or real Desktop model turn was attempted. No Stage or
Unstage operation was performed.

## Cleanup and production boundary

Both generic bridge processes completed app-server shutdown. The live gate
removed its isolated temporary Codex home. The corrected test observer removes
its temporary private evidence file and restores the evidence environment
variable after each turn. No disposable repository was created by this task.

Production runtime changed: no. All source changes are inside Windows test/live
certification test code. No Cargo, dependency, frontend, permission, ADR,
production runtime, or release file changed. No production defect was found;
the generic prerequisite was blocked before Desktop-specific diagnosis.

## Validation and closure evidence

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test --workspace -- --test-threads=1`: PASS. All executed tests
  passed; the run included Desktop `244` passed and `11` ignored, Codex
  runtime `83` passed and `1` ignored, and all other workspace suites passed.
  The three new observer tests passed in the Desktop count.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; 13 packages, version
  `0.25.0`, Rust edition `2024`.
- `cargo build -p rah-desktop --release`: PASS.
- `node --check crates/rah-desktop/frontend/status.js`: PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js`: PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js`: PASS.
- `node crates/rah-desktop/tauri_permission_test.js`: PASS.
- Focused reruns: Task 321-C/E/I filter `8 passed`; Task 320 `3 passed`;
  Task 318 `1 passed`.
- `git diff --check`: PASS.
- No Cargo.lock or dependency drift; no new dependency edge.
- The full Task 324 ignored live test was not run. No
  `RAH_V026_MULTI_REPOSITORY_LIVE_OK` marker was emitted.

## Authorization and closure

The result is:

```text
OUTCOME A - EXTERNAL / GENERIC CODEX LIVE PATH BLOCKED
```

Task 324 remains blocked and a fresh full Task 324 rerun is not authorized by
this task. The next supported action is to resolve the approved environment,
service, or model-tool availability condition and then obtain separate
authority for a fresh certification using the corrected source SHA. Task 325
remains unauthorized in all cases.

The final changed-file list is exactly:

```text
crates/rah-desktop/src/main.rs
docs/plans/2026-09-14-task-324-a-real-codex-turn-failure-triage.md
```

The source change is test-only. Final Git head/remote alignment and exact-head
CI are recorded by the completion report after the correction is committed.
