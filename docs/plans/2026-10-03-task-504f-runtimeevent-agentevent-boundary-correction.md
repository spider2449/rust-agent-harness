# Task 504F — RuntimeEvent / AgentEvent boundary correction

Starting HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077` (master).
Dirty Task 504 WIP was inspected and preserved. Source stayed frozen during
validation. Evidence: `F:/temp/rah-task504f-evidence`, `F:/temp/rah-task504f-live`.

## Event audit

Task 501 gives TurnHandle the existing RuntimeEventStream and separate control.
Task 503 translates provider events and merges host live Tool events. ADR 0033
places neutral conversation/turn ownership in the host. DesktopRuntime::start
returns that neutral handle; production run_chat consumes handle.events.

RuntimeEvent in rah-runtime/src/failure.rs is a local **struct, not an enum**.
It owns AgentEvent and optional RuntimeFailure. event() borrows the protocol
projection; failure() borrows the retained envelope; into_event() consumes the
presentation projection. ADR 0032 deliberately established this distinction.
There are no separate RuntimeEvent variants to pattern-match directly.

AgentEvent belongs to rah-protocol/src/events.rs. It is generic, serializable
RAH protocol, not Codex-specific. Production consumers include rah-cli, legacy
runtime producers and Desktop activity/chat presentation. Frontend receives
ChatEvent/ActivityEvent. AgentEvent has no semantics absent from RuntimeEvent;
the latter wraps the entire former and adds a non-serializable typed source.

| Wrapped protocol variant | Meaning and production Desktop behavior |
| --- | --- |
| Started | Accepted request/session; Desktop emits Started after currentness registration |
| ModelRequestStarted | Model-request correlation; internal to Desktop presentation |
| ModelDelta | Incremental text -> current ChatEvent::Delta |
| ToolRequested | Untrusted call with host ID/name/input -> activity Requested |
| ToolStarted | Approved execution with same ID -> activity Started |
| ToolFinished | Correlated output -> activity Finished and existing uncertainty/review/refresh handling |
| ApprovalRequired | Approval request; migration introduces no approval UI |
| Completed | Final AgentOutput -> existing transcript/persistence and terminal ownership |
| Failed | Stable code/sanitized detail; safe diagnostic copied from local envelope |
| Cancelled | Distinct cancellation -> existing uncertainty accounting and ChatEvent::Cancelled |

Production main.rs already copies the safe diagnostic then uses
local_event.event().clone() at its presentation boundary. Its exhaustive match
and activity helper cover all production-used events. Typed sources stay local;
no CodexAdapterError enters protocol data. Cancellation does not imply rollback
or replay. Tool IDs/requested/started/finished and host-owned authority remain.

## All eleven E0308 sites

Original Task 504E coordinates in main_tests.rs are recorded below. Each is in
windows_live_desktop_repo_create_branch, which consumes the neutral stream and
also tests Desktop activity presentation. These are E3 mixed-layer sites,
redirected to the existing borrowed presentation seam. They are not an E1-only
neutral conformance test or evidence of a missing E2 production mapping.

| Original line | Site | Class | Corrected boundary |
| --- | --- | --- | --- |
| 6502 | activity_event_with_composition argument | E3 | Borrow local_event.event() for presentation helper |
| 6511 | ToolRequested | E3 | Match borrowed protocol projection, retaining ID/name |
| 6516 | ToolStarted | E3 | Same projection; requested-call lookup unchanged |
| 6531 | ToolFinished | E3 | Same projection; correlated output unchanged |
| 6545 | Completed | E3 | Same projection; final message unchanged |
| 6549 | Failed | E3 | Same projection; sanitized message, local envelope retained |
| 6553 | Cancelled | E3 | Same projection; separate cancellation branch |
| 6557 | Started | E3 | Same projection; existing fixture no-op |
| 6558 | ModelRequestStarted | E3 | Same projection; existing fixture no-op |
| 6559 | ModelDelta | E3 | Same projection; fixture asserts final output |
| 6560 | ApprovalRequired | E3 | Same projection; existing fixture no-op |

Counts: E1 0 / E2 0 / E3 11 / E4 0. No semantic gap.
Exact corrections: rename loop item to local_event; add
`let event = local_event.event();`; pass event instead of &event to activity
helper; match event instead of &event. Assertions remain intact. Local envelope
ownership survives inspection. Task 504F changed **no production source** and
added no mapping, alias, neutral API, adapter change or dependency.

## Frozen validation

CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR: `F:/temp/rah-task504-target`.
First exact Phase B command:

```powershell
cargo test -p rah-desktop --bin rah-desktop -- task504 model_preflight::tests:: desktop_model_selection_maps_only_closed_provider_choices connected_repository_selection_rejects_direct_activation_without_changing_authority local_runtime_failures_project_closed_frontend_codes activation_publication_requires_repository_model_profile_and_connection_currentness
```

PASS: **12 passed / 0 failed / 0 ignored**, exit **0**, 341 filtered out.
phase-b.log/phase-b.exit retain output. All eleven mismatches resolved; no new
compile blocker. The ignored mutating live branch fixture compiled but was not
executed; production validation used read-only Tools.

| Gate | Result |
| --- | --- |
| Canonical windows-desktop-test-gate.ps1, target above, output F:/temp/rah-task504f-evidence/desktop | PASS exit 0; 333 passed / 0 failed / 20 ignored |
| cargo fmt --check | PASS exit 0 |
| cargo check --workspace | PASS exit 0 |
| cargo test --workspace | PASS exit 0; 1046 passed / 0 failed / 24 ignored; 56 result groups including doctests |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS exit 0 |
| git diff --check | PASS exit 0, line-ending warnings only |
| Five node frontend scripts | PASS: model_preflight, remembered_workspace_layout, remembered_workspace, repository_membership, status_authority |
| node crates/rah-desktop/tauri_permission_test.js | PASS: 47 matching runtime/manifest/generated/default allows/frontend commands |
| cargo metadata --no-deps --format-version 1 | PASS; all 13 workspace packages v0.33.0 |
| HostExplicit | Exactly 11 statically; host_allowlist_is_exact passed in canonical and workspace suites |
| Task 499 | Preflight/publication/currentness tests pass; production advertised/absent cases pass |
| Task 503 | Six experimental conformance tests pass; Codex library 102/0/1, architecture 6/0/0, live-contract 11/0/0 |

The three historical bridge tests passed naturally. No recurrence, retry or
matrix. Task 504D Phase A 137/0/1 remains separate retained evidence. All nine
frozen hashes (eight source files plus ADR 0033) stayed equal; see frozen-hashes.json
and source-stability.json. No passing focused suite was rerun for activity.

## Production validation

cargo build -p rah-desktop: PASS exit 0. Actual production Desktop:
`F:/temp/rah-task504-target/debug/rah-desktop.exe`, PID 5688, SHA256
`8441B5B5BA3F96E68F27F06106FD53B493C003ABAB5A927ADC38CB6F7F8D8468`.
Stored codex-cli 0.157.1 passed precheck and normal adapter admission.
Owned app-server PID 2348, parent 5688, executable SHA256
`8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.
Ownership and hash are separate evidence; no new file-object certification.

Existing production WebView/Tauri commands and renderer were used, without
product test hooks. launch.json, ownership.json, result JSON, screenshots and
evidence.jsonl retain observations. Screenshots were directly inspected.

- Connect: advertised gpt-6-astra, ready publication.
- Ordinary turn: completed RAH_TASK504F_LIVE_OK, no Tool activity.
- Read-only Tool: exactly one repo.status requested/started/finished, success,
  completed RAH_TASK504F_TOOL_OK.
- Connected switch: repository_selection_requires_disconnect; active member and
  membership generation unchanged.
- Disconnect: runtime/codex not connected, repository Tools inactive; no owned
  app-server child remained.
- Absent model: rah-task499-harmless-invalid-model, visible identifier/nine
  alternatives, non-ready. Generations 1/3 had no publication, thread_start or
  completion. No rejected-case inference submitted.
- Disconnected switch: repo-a -> repo-b, repository generation 1 -> 2.
  Reconnect passed; exactly one fs.read lifecycle returned RAH_TASK504F_repo-b
  from readme.txt, proving fresh selected-repository binding. Deterministic
  stale-handle/revocation/lease/uncertainty regressions also passed.

Temporary fixtures/harness scripts stayed outside the worktree. Initial
Inherit/null preference and original remembered catalog were restored; only
temporary candidates removed, transcript not cleared. A cleanup harness call
omitted close_repository's required request and was rejected before closure;
corrected invocation used the existing member/generation guard. This argument
error was not a product gate failure and caused no source or validation retry.
Desktop closed normally via Process.CloseMainWindow.

## Authority and closure

HostExplicit exactly **11**. No authority expansion, new dependency, permission,
Trusted Profile change, ToolRegistry bypass, repository selection/lease change,
replay, uncertainty or remembered-workspace semantic change. ADR 0033 unchanged
from migration WIP. Neutral runtime contracts and adapter semantics untouched
by 504F. No alternate-provider work started. Live evidence is bounded and does
not claim universal reliability, rollback or stable experimental APIs.

**A — EVENT BOUNDARY ALIGNED; TASK 504 VALIDATION COMPLETED**

This authorizes the complete Task 504 commit and normal GitHub master push.
Publication identity follows after natural exact-head CI verification.
