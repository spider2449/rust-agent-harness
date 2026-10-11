# Task 510B-R5E — Certification capture wire opacity and R5 closure

Starting HEAD: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
Evidence directory: `F:/Temp/rah-task510br5e-evidence`.

1. Preserve R5 WIP, historical STOP reports and all successful live gates.
2. Audit the unchanged architecture guard and every capture caller before editing.
3. Restore wire opacity; run the exact architecture test first, then deterministic
   capture regressions, affected checks/Clippy, formatting and diff checks.
4. Resume workspace tests once, workspace check, full Clippy, canonical Windows
   Desktop suite and established frontend/static closure. Stop on first failure.
5. Validate Tauri, metadata, lockfile identity, HostExplicit, authority and final
   frozen bundle identity/process census. Only A permits durable WIP review,
   staging, certification commit, normal master push and exact-head natural CI.
6. Task 510C is not begun. Authorization requires A and certification CI PASS.

## Guard and caller audit

`architecture.rs:98-123` scans every Rust source in adapter src, trims each
line and selects lines beginning `pub ` or `pub use`. It forbids substrings
AppServerTransport, ProcessTransport, ConnectionEvent, Incoming and
serde_json::Value. It does not evaluate cfg. Its mechanical-release-gate
module rationale implements accepted ADRs 0001/0005 and architecture guardrails:
provider wire/process types stay adapter-private. No guard changes or exceptions.
Existing public types are RAH-facing runtime/error/model configuration types;
certification support exposes artifact identity and construction, not wire DTOs.

U1: internal readiness and capture tests in certification_support.rs.
U2: no records()/timeline() callers elsewhere inside the adapter crate.
U3: task510b_snapshot integration crate uses records for Tool evidence and timeline
for cancellation evidence; Desktop certification crate uses records for catalog
assertions and timeline for persisted evidence. Public access is required.

Option B chosen. Before:
`pub fn records(&self) -> Vec<(bool, serde_json::Value)>`
and `pub fn timeline(&self) -> Vec<(u128, bool, serde_json::Value)>`.
After: `Vec<(bool, String)>` and `Vec<(u128, bool, String)>` respectively.
Private raw_records retains values for readiness and internal tests. Capture,
notifications and transport behavior remain unchanged. External harnesses parse
strings locally, retaining existing JSON evidence layout. No public Codex DTO.
The existing certification-harness feature and doc-hidden module remain required.

First required architecture gate PASS: 1 passed / 0 failed / 0 ignored.
Historical R5D partial workspace result remains 520 passed / 1 failed / 21 ignored;
its Desktop component remains 358 / 0 / 20, not canonical final closure.
Cargo.lock starting SHA256:
`4e9819aa68daa52fc6deb834edfd8a5b82e19d6d95f8d0f11a5e9d0a8211c8d8`.
Pre-existing HEAD diff is preserved. ADR-B: restore existing encapsulation; no ADR.

## Final certification result

**A — CODEX 0.160.0 EXACT RUNTIME BUNDLE CERTIFIED**

This certification does not change production admission or the preferred
`codex-cli 0.157.1` baseline. Task 510C remains a separate task, never automatically
started. Its authorization additionally requires natural exact-head certification
commit CI PASS; publication results are recorded in the final return and external
evidence directory rather than embedding a commit's own SHA in its source.

| Required gate | Fresh R5E result |
| --- | --- |
| Exact architecture guard, first gate | 1 passed / 0 failed / 0 ignored |
| Certification support regressions | 12 passed / 0 failed / 0 ignored; 103 filtered |
| Affected all-target checks, certification features | PASS |
| Affected all-target Clippy, certification features, -D warnings | PASS |
| cargo fmt --check / git diff --check | PASS / PASS |
| cargo test --workspace | 1088 passed / 0 failed / 25 ignored; 60 summaries |
| cargo check --workspace | PASS |
| Full workspace all-target/all-feature Clippy, -D warnings | PASS |
| Canonical Windows Desktop | 358 passed / 0 failed / 20 ignored |
| Frontend syntax | All 8 actual frontend JavaScript files PASS |
| Frontend suites | All 7 PASS, first execution; Edge layout PASS |
| Tauri inventory | 47 / 47 / 47 / 47 / 47 |
| Metadata | 14 packages/members; all 0.33.0; all edition 2024 |
| Cargo.lock | Starting R5 hash unchanged; existing HEAD diff retained |
| HostExplicit | Static mapping exactly 11; executable host_allowlist_is_exact PASS |
| Final bundle | Both SHA/length/file IDs unchanged; residual processes 0 |

Commands use CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR
`F:/Temp/rah-task510br2-target`. Affected checks/Clippy select
`-p rah-runtime-codex -p rah-desktop --all-targets --features
rah-runtime-codex/certification-harness,rah-desktop/certification-harness`.
Capture command is `cargo test -p rah-runtime-codex --features
certification-harness --lib certification_support::`. No ignored live tests run.

Focused breakdown: 5 identity/digest/construction tests; 5 provider readiness
tests; 1 transport direction test; 1 opaque snapshot fidelity test. The latter
round-trips Tool advertisement/request, provider turn/started, cancellation
request/result and diagnostic fixture, preserving direction, timestamps, order,
null/nested values, Unicode and escaped text. Private raw readiness remains intact.

Canonical command: `powershell -NoProfile -File
scripts/windows-desktop-test-gate.ps1 -TargetDirectory
F:/Temp/rah-task510br2-target -OutputDirectory
F:/Temp/rah-task510br5e-evidence/desktop`. Harness PASS exit 0; helper/test exits 0;
303.57s total, test 195.95s; watchdog false. Evidence subdirectory:
`20261006-194932-807-6a7c26d7a46d4d27898bb53b3f044e8d`.

Frontend suites: model_picker_test.js, model_preflight_test.js,
remembered_workspace_layout_test.js, remembered_workspace_test.js,
repository_membership_test.js, runtime_model_state_test.js, status_authority_test.js.
All actual frontend JS syntax checks and tauri_permission_test.js syntax/inventory
run successfully. Metadata uses offline Windows-filtered cargo metadata and checks
workspace_members against packages. No dependency changes made by R5E;
pre-existing runtime dev-dependency on rah-sandbox supports retained provenance
tests, not a new production dependency edge. No new certification route or IPC.

### Preserved live evidence and historical blockers

R5 Tool PASS remains raw calls 1 / neutral requests 1 / authorization 1 accepted /
execution starts 1 / successful results 1 / runtime replies 1 / turn completion
PASS. R5B's cancellation diagnosis is historical; R5C resolved readiness with
acknowledgement -> local ModelRequestStarted -> matching provider inProgress
turn/started -> interrupt -> {} -> terminal interrupted -> neutral Cancelled ->
fresh lease accepted -> shutdown. Diagnostic envelope PASS retains typed local
cause and sanitized public diagnostic. R5C's later Desktop compile STOP remains
historical; R5D fixed only test typing and achieved live Desktop 1/0, 53.10s.
Production rejection before app-server startup remains PASS: inference 0, Tool 0.
0.157.1 required SHA/baseline verification and complete R5D protocol matrix remain
PASS. None of these successful live gates were rerun in R5E.

R5D's 520/1/21 workspace STOP is preserved, including architecture.rs:113 and its
public records signature. Its 358/0/20 Desktop component was not canonical final
closure. R5E restores records and timeline wire opacity without weakening the
guard, then provides fresh complete deterministic/canonical/frontend closure.
Earlier R5/R5B/R5C/R5D classifications and reports are not rewritten.

### Authority and security closure

Review of the complete accumulated diff plus deterministic coverage confirms
unchanged ToolRegistry ownership, HostToolScope authority, active repository
authority, linked-worktree handling, repo switching, leases, permissions,
Trusted Profiles, remembered workspaces, mutation uncertainty and provider/model
authority. HostExplicit remains exactly 11. No new Tool or bypass, permission,
Tauri command, production admission, fallback, preferred baseline or version.
Accumulated instrumentation is opt-in process-local evidence; it does not compose
authority or serialize private provenance into Tool output. Process supervision,
fixed executable/argv/cwd/environment and existing uncertainty semantics remain.
Certification capture is feature-only and adapter-local. R5E changes inspection
representation only; no app-server handling, event translation or behavior change.
ADR-B, no new ADR. No Task 510C policy/admission work performed.

### Final frozen bundle remeasurement

Bundle: `F:/Temp/rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c`.
Independent PowerShell Get-FileHash / file length / fsutil file queryfileid:

| File | SHA256 | Length | File ID |
| --- | --- | ---: | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 |

Residual bundle process count 0. Both IDs refer to frozen files, not installed
source files. Identity and process evidence are final-identities.json and
final-process-count.txt. Certification applies to these exact bytes.

## A-only WIP review

All changed files classified below before staging. No historical STOP report is
deleted. No unrelated repository/workspace edits included.

| File | Decision and durable value |
| --- | --- |
| Cargo.lock | KEEP: pre-existing runtime test dependency; exact R5 starting bytes |
| crates/rah-desktop/src/certification_tests.rs | KEEP: exact-bundle real Desktop certification and private-source mapping |
| crates/rah-runtime-codex/src/certification_support.rs | KEEP: digest regression, opaque capture, provider-active readiness regressions |
| crates/rah-runtime-codex/src/experimental.rs | KEEP: feature-gated exact-bundle capture plumbing |
| crates/rah-runtime-codex/tests/task510b_snapshot.rs | KEEP retained exact-bundle/Tool/cancellation/diagnostic/rejection support; REMOVE two superseded probes described below |
| crates/rah-runtime/Cargo.toml | KEEP: opt-in provenance feature and test-only dependency |
| crates/rah-runtime/src/experimental_host.rs | KEEP: opt-in host lifecycle timing for retained provenance controls |
| crates/rah-runtime/tests/task510br4h_host.rs | KEEP: deterministic typed provenance regression, reusable explicit fault and real-root controls |
| crates/rah-sandbox/Cargo.toml | KEEP: opt-in local provenance feature |
| crates/rah-sandbox/src/lib.rs | KEEP: feature-gated provenance access for test controls |
| crates/rah-sandbox/src/supervised_process.rs | KEEP: process-local concrete OS/source evidence and blocked-pipe regression |
| crates/rah-tools/Cargo.toml | KEEP: propagation of opt-in provenance feature |
| crates/rah-tools/src/bin/rah_execute_fixture.rs | KEEP: descendant-owned pipe fault fixture |
| crates/rah-tools/src/host_execute.rs | KEEP: local source capture before conversion |
| crates/rah-tools/src/repository_boundary.rs | KEEP: opt-in boundary timing evidence |
| crates/rah-tools/src/repository_git_layout.rs | KEEP: local phase/argv/budget provenance |
| crates/rah-tools/src/repository_observer.rs | KEEP: observation timing and explicit forced-timeout regression |
| crates/rah-tools/src/repository_status.rs | KEEP: lease/observation/conversion timing |
| crates/rah-tools/tests/execute_policy.rs | KEEP: deterministic descendant-pipe reap regression |
| docs/plans/2026-10-04-task-510b-codex-0-160-0-runtime-certification.md | KEEP: complete historical campaign/STOP evidence |
| docs/plans/2026-10-05-task-510br5b-cancellation-protocol-diagnosis.md | KEEP: historical diagnosis/STOP |
| docs/plans/2026-10-05-task-510br5c-provider-active-cancellation-readiness.md | KEEP: historical correction/compile STOP |
| docs/plans/2026-10-06-task-510br5d-desktop-certification-typing.md | KEEP: live PASS/architecture STOP |
| docs/plans/2026-10-06-task-510br5e-certification-wire-opacity.md | KEEP: final closure and review |

Removed only r3_host_only_status and snapshot_catalog_and_direct_turn from the
integration harness: superseded one-off host/catalog/direct-turn probes. Prior
source saved as task510b_snapshot-before-cleanup.rs in external evidence;
historical reports and logs retained. Shared bounded Tool protocol harness stays
because it implements exact-bundle Tool certification and reusable raw capture.
Post-cleanup targeted integration Clippy -D warnings PASS; formatting PASS.
Only ignored test functions removed; production source and all executed workspace,
Desktop and focused regressions unchanged. No successful live gate rerun.

Publication uses `test: certify Codex 0.160.0 runtime bundle`, normal GitHub
origin/master push, no force/tag/release. After exact-head CI PASS, the next
authorized Task 510C must decide/implement known-good certified fallback plus an
automated runtime compatibility gate using runtime capability/protocol behavior
as evidence and version as recorded evidence rather than primary authority.
It must not be merely adding 0.160.0 to hardcoded allowed versions.
