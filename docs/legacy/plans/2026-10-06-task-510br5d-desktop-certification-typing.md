# Task 510B-R5D — Desktop certification typing and final gates

Starting HEAD: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
Preserve existing R5 WIP, empty staging, and R5C failed evidence.

1. Audit existing Connect/Disconnect assertions; correct only certification
   test typing using status fields and Debug-free result matching.
2. Compile the exact codex-certification target, then execute the exact ignored
   Desktop bundle test once. Stop on live failure, without retry.
3. On Desktop PASS, run production rejection, baseline, protocol/identity,
   workspace, canonical Desktop, frontend/static, inventories and authority review.
4. Only complete classification A permits reviewed durable WIP cleanup,
   certification commit, normal push and exact-head CI. Do not begin Task 510C.

Evidence: `F:/Temp/rah-task510br5d-evidence`.

## Result

**D — FULL DETERMINISTIC / DESKTOP / FRONTEND CLOSURE FAILED**

Starting and final HEAD: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
R4L CI `37317133407` remains historical PASS, not certification CI.

Audited main_tests.rs existing real Connect result handling and field/status
assertions (including Task 324 and connected-close coverage). Corrected only
certification_tests.rs typing: match Connect success and assert status connected;
format only FrontendError on failure; assert published connected state without
formatting ConnectionResult. Disconnect matches its actual ConnectionResult
and requires status not connected. No production ConnectionResult trait,
Disconnect return type, IPC, composition or behavior changed.

Exact compile command: `cargo test -p rah-desktop --features certification-harness
--test codex-certification certification_tests::task510br5c_bundle_desktop_turn
--no-run`. PASS, exit 0, 6m23s including editor Cargo package-cache contention.
Exact execution adds `-- --exact --ignored --test-threads=1 --nocapture`.
PASS, exit 0, 1 passed / 0 failed / 0 ignored / 381 filtered, 53.10s.
Used existing two-file bundle and real backend connect_with_configuration/run_chat/
disconnect_codex. Connect status connected; model/list response includes
gpt-6.1-sol; real fresh turn commits one conversation pair and presents events;
chat returns Idle; Disconnect status not connected; runtime alive false.
Both artifact measurements unchanged. No item/tool/call. No repo.status rerun.
Final independent bundle process census 0. Catalog remains runtime-advertised /
provider-unverified; fresh successful interaction is separate evidence, not a
blanket provider validation claim. No catalog entry was added or hardcoded.

Fresh ordinary production rejection: added ignored r5d_production_rejection
to the existing certification integration harness. Uses ordinary CodexFactory::new,
not candidate construction, against exact frozen bundle. Command:
`cargo test -p rah-runtime-codex --features certification-harness --test
task510b_snapshot r5d_production_rejection -- --exact --ignored --nocapture`.
PASS 1/0, 0.03s. Typed VersionMismatch actual codex-cli 0.160.0. ProcessTransport
start verifies version before schema and app-server spawn, so rejection precedes
app-server startup; inference 0, Tool 0. Exact admission list and preferred
baseline asserted unchanged at codex-cli 0.157.1.

Fresh baseline: scripts/codex-baseline.ps1 verify 0.157.1 PASS; independent
Get-FileHash SHA256 `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`
PASS; actual --version codex-cli 0.157.1. No demotion/removal or fallback change.

## Protocol matrix

| Required row | Explicit evidence |
| --- | --- |
| startup | successful real Desktop Connect and runtime session |
| initialize | desktop-wire.json initialize ID 1 request and result |
| model/list | Desktop wire request/result; gpt-6.1-sol catalog assertion PASS |
| thread/start | Desktop wire request/result and thread/started |
| turn/start | Desktop wire request/result |
| provider turn/started | Desktop notification; retained R5C exact matching inProgress readiness |
| streaming | Desktop wire 7 item/agentMessage/delta messages |
| completion | Desktop turn/completed plus committed assistant marker |
| Tool advertisement | preserved R5A thread/start single repo.status dynamicTools mapping |
| Tool call | preserved R5A raw calls 1 / neutral requests 1 / authorization 1 accepted |
| Tool result continuation | preserved R5A execution 1 / success 1 / reply 1 / completed turn PASS |
| cancellation | preserved R5C acknowledgement → local start → matching provider inProgress → ID 4 interrupt → result {} → interrupted → Cancelled → fresh lease |
| diagnostics | preserved R5C process-local typed cause and sanitized public provider_rejection envelope |
| shutdown | Desktop real Disconnect PASS / runtime dead / final process census 0; R5C explicit shutdown retained |

No successful Tool/cancellation/diagnostic live gate repeated. S1 logical
ModelRequestStarted semantics unchanged; no sleep-based readiness.
Desktop capture includes provider warning/configWarning notifications; successful
completion is established by assertions, not a claim of warning-free operation.

## Identity and deterministic closure

Final independent PowerShell/fsutil identities (final-identities.json):

| Member | SHA256 | Length | File ID |
| --- | --- | ---: | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 |

Targets: CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR F:/Temp/rah-task510br2-target.
cargo fmt --check PASS. cargo check --workspace PASS, 1m31s.
cargo test --workspace FAIL exit 101: aggregate completed targets 520 passed /
1 failed / 21 ignored across 17 targets; incomplete workspace execution.
Desktop component 358 passed / 0 failed / 20 ignored, 217.51s.
Adapter architecture suite 5 passed / 1 failed / 0 ignored, 0.75s.
Exact failure: codex_wire_types_are_not_public, architecture.rs:113, flags
certification_support.rs:721 `pub fn records(&self) -> Vec<(bool, serde_json::Value)>`.
This guard scans pub source lines without interpreting cfg; certification_support
is doc-hidden and certification-harness-feature-gated. Existing capture timeline
also exposes Value. This is the first deterministic closure failure; no repair,
retry or production API change made. No authority regression established.
Preserve workspace-test.log/exit and source evidence for a bounded follow-up.

Full Clippy NOT RUN after STOP. Final git diff --check PASS.
Canonical Windows Desktop suite NOT RUN; workspace component counts above are
not canonical certification. Frontend/static NOT RUN; no Edge retry.
Tauri 47/47/47/47/47 and metadata 14 packages / 0.33.0 / edition 2024 NOT freshly
validated after STOP. No new IPC route or manifest/version change in R5D.
Cargo.lock unchanged from expected R5 start, SHA256
4e9819aa68daa52fc6deb834edfd8a5b82e19d6d95f8d0f11a5e9d0a8211c8d8.
Its pre-existing HEAD diff is preserved. HostExplicit host_allowlist_is_exact
executed and passed within workspace Desktop; static inventory closure NOT RUN.
Do not claim complete exactly-11 closure from this partial run.

R5D changes only certification_tests.rs, task510b_snapshot.rs and this report.
No R5D change to ToolRegistry ownership, active repository authority, switching,
HostToolScope, leases, permissions, Trusted Profiles, remembered workspaces,
mutation uncertainty, provider/model authority or production Codex admission.
Accumulated prior diagnostic WIP remains unreviewed for final publication;
full authority/security closure NOT RUN after STOP. Certification grants no
production authority. ADR-B; no new ADR, dependency, release or version change.

All starting WIP kept; none removed. A-only durable cleanup/staging not reached.
Staging empty; no certification commit SHA/message, no push/new CI/tag/release.
Final HEAD unchanged and worktree dirty with preserved R5 WIP and R5D report.
Task 510C NOT AUTHORIZED: classification A plus exact-head certification CI
not achieved. No Task 510C work begun. Suggested next bounded task: resolve
the certification capture/public-wire architecture guard conflict, preserving
this failed evidence and accepted provider-neutral boundaries, then resume gates.
