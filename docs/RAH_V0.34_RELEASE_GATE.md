# RAH v0.34.0 completed historical release record

Status: **RELEASED SOURCE-ONLY BY TASK 518; TASK 519 POST-RELEASE DOCUMENTATION CLOSURE**.

## Immutable publication record

- Release source and peeled tag target: `7b972988f2f116b87b3770d66be1ef970e20b726`.
- Annotated tag: `v0.34.0`; tag object: `504c298947f81f531364d99c1189b8281ddf5a6f`.
- Release-source [master CI 37889995880](https://github.com/spider2449/rust-agent-harness/actions/runs/37889995880): PASS.
- [Tag CI 37890500633](https://github.com/spider2449/rust-agent-harness/actions/runs/37890500633): PASS for the exact release source.
- [GitHub Release](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.34.0)
  ID: `407568690`; published `2026-10-09T05:55:25Z`;
  `draft=false`; `prerelease=false`; uploaded assets=`0`.
- Artifacts: GitHub source archives only; no executable or installer uploads.

Task 516 performed preparation. Task 517 recorded the explicit human OpenAI
waiver without publication authority. Task 518 separately received authorization
and published the exact source above. Task 519 independently verified publication
and corrected historical documentation; its later documentation commit is not
the release source. See the [Task 519 closure record](plans/2026-10-09-task-519-v0.34-post-release-closure.md).

## Historical preparation and waiver decision

Task 517 verified Task 516 closure at
`041daae9cc448d0eaaef71dc15dfdce7a4aac6ef`: local/origin/live master matched and
[exact-head CI 37888938589](https://github.com/spider2449/rust-agent-harness/actions/runs/37888938589)
passed. The backend execution environment lacks OPENAI_API_KEY; model availability
and explicit paid-call authorization are not established. No live request ran.
The human explicitly approved the documented source-only OpenAI waiver on
2026-10-09; see the
[Task 517 decision report](plans/2026-10-09-task-517-release-decision-report.md).
**NSIS_INSTALLER_NOT_VERIFIED** remains separate. Task 518 publication was
source only, following the prior release's zero uploaded assets; no executable
upload or installer certification was included.

## Scope and identity

Task 516 starts from `141f4d7966f6e67d9f308b40adcb7053bd5c0d8b`.
Local HEAD, fetched origin/master and live GitHub master matched; exact-head
[CI 37882152540](https://github.com/spider2449/rust-agent-harness/actions/runs/37882152540)
passed before preparation. All 14 workspace packages and internal lock records,
and the Desktop Tauri bundle version, are aligned to 0.34.0 (edition 2024).
External dependency/checksum drift is forbidden. Prior v0.33 publication history
and its immutable release gate are preserved.

Scope includes native OpenAI/llama.cpp, neutral runtime and optional legacy Codex,
native Desktop lifecycle/cancellation, model-selected host Tool integration,
redesigned workspace, persistent/resizable panels and Workspace/Inspector
section navigation. Repository and security authority boundaries are unchanged.

## OpenAI public-release blocker

**OPENAI_LIVE_NOT_VERIFIED - EXPLICIT HUMAN WAIVER APPROVED**.

Actual official OpenAI Responses API acceptance has not been established.
Deterministic API fixtures and missing-key recovery are not substitutes.
The human explicitly approved Task 517's documented source-only waiver. That
resolves the OpenAI publication blocker under the stated scope without certifying
live behavior. No waiver is inferred from preparation, tests or CI. Task 516/517
authorized no tag or GitHub Release; Task 518 subsequently supplied separate
publication authorization and completed source-only publication. The waiver
is not OpenAI live PASS.

## Runtime, authority and recovery

Default Desktop compiles native OpenAI/llama.cpp; selection and configuration
are host-owned and changed while disconnected. OpenAI uses a fixed official
Responses origin and backend-only OPENAI_API_KEY. llama.cpp uses loopback-only
HTTP(S), bounded health/model discovery, and separate optional
RAH_LLAMA_CPP_API_KEY. Empty local model selection resolves only a sole listed
model. There is no downloaded model, launched server, implicit Codex fallback,
frontend credential store or automatic replay. Credential changes need restart.
Optional legacy Codex retains its admission rules; it is not a native prerequisite.

MODEL REQUEST IS NOT AUTHORIZATION. Model Tool requests use revocable host
leases, ToolRegistry and permission dispatch. HostExplicit remains exactly 11;
Tauri command inventory remains 49. Observation, bounded worktree mutation,
index mutation and reviewed commit/history mutation stay separate. Persisted
layout/navigation is presentation only; remembered repository candidates do
not restore authority after restart.

Wait for the cancelled terminal before the next prompt; disconnect/reconnect
creates fresh handles and revokes old Tool leases. Missing keys fail closed;
restart with backend credentials or explicitly recover to a configured local
provider while disconnected. Restore local server health before reconnecting.
Cancel, timeout, disconnect and response loss do not roll back admitted effects.
Inspect uncertain effects before another action. Retained known-good executable
and compatible configuration restoration is operational recovery, not external
effect or repository-history rollback.

## Evidence reuse and limits

[Task 514F](plans/2026-10-09-task-514f-native-acceptance-closure.md) consolidates
native actual Windows provider, multi-turn, model-selected repo.status and
cancellation evidence. [Task 515B](plans/2026-10-09-task-515b-windows-acceptance.md)
records layout, persistence, navigation and model-selected Tool UI acceptance.
[Task 515C](plans/2026-10-09-task-515c-final-integration.md) records final corrected
UI, Cancel/next prompt, reconnect, missing-key and llama.cpp recovery acceptance.
Its accepted executable SHA-256 is
`CC149596E17B3D971C1AE7F19021A672977C313229F7FE7346465D66B80A48F1`.
Those results are reused only for unchanged source behavior; version metadata
and the new production executable are independently checked in Task 516.
Original failures, diagnostics and historical untracked reports remain preserved.
Historical staged-diff investigations are not reopened absent fresh evidence.

Local model/template support is limited to the accepted sessions and retained
configuration; arbitrary llama.cpp models/templates are not certified. OpenAI
model discovery and provider-native continuation are unsupported; host-owned
text replay remains supported. Runtime APIs remain experimental. Windows
executable/UI proof does not certify Linux/macOS live behavior or NSIS installer
creation/execution. Bundle configuration is audited separately from installer proof.

## Required preparation quality gates

Formatting; locked workspace check/tests; strict workspace all-target/all-feature
Clippy; native/neutral suites; deterministic production OpenAI Tool fixture;
fixture-helper build and canonical default-parallel Windows Desktop suite;
all ten frontend suites including both Edge browser suites; JavaScript syntax;
49-command Tauri inventory; exact HostExplicit 11 tests; production Windows
release build; Cargo metadata, lock integrity and emitted executable version;
diff integrity; reviewed commit and normal push; local/origin/live master equality
and exact-head CI success. Stop at the first required failure; retain evidence.

Executed results and final classification belong to the
[Task 516 preparation report](plans/2026-10-09-task-516-release-preparation-report.md).
Commit/publication identifiers are retained under `target/task516`; this document
does not claim its own future commit identity or future CI result.

Task 516 local gates all passed: workspace 1,078/0/18 (passed/failed/ignored),
canonical Desktop 339/0/13, native 23, neutral 18, production fixture 1,
all ten frontend suites/both Edge suites, Tauri 49 and HostExplicit 11.
Production Windows executable: 21,177,344 bytes; file/product version 0.34.0;
SHA-256 `A15C0D0E0802E2D2150B088E401D636870D7234E3A17D6968F9A2C98E15785F5`.
All 14 packages and internal lock records use 0.34.0 with no external drift.
Bundle configuration/version and executable resources PASS; NSIS installer was
not built or tested (Tauri packaging CLI unavailable). Exact-head CI and master
equality after the normal push are required for classification A. That classification
means READY FOR RELEASE DECISION; OPENAI GATE EXPLICITLY RECORDED, never final
public-release readiness by itself. Task 517's explicit waiver subsequently
resolves the OpenAI decision for source-only scope, retaining live noncertification.
