# Task 511 — Native OpenAI Production Runtime Cutover

## Scope and checkpoint

Starting SHA: e680f689129aa430b9865cb5d662682e43f9136a. Independently
verified local HEAD, origin/master and live GitHub master before edits.
Primary checkout is dirty with preserved Task 510C2 WIP. Work is isolated on
task511-native-openai, created from origin/master. No primary WIP is changed.
Task 510C2 and R26–R29 certification/admission work is stopped.

## Phase 1 audit

Before: Desktop default -> provider-codex -> optional rah-runtime-codex ->
Codex discovery/certified baseline/model catalog -> app-server.
OpenAI is already an independent optional provider-openai dependency.
Neither neutral rah-runtime nor rah-cli depends on rah-runtime-codex.
Workspace membership, legacy examples/dev tests and certification-harness
retain Codex; workspace membership does not create a Desktop dependency.

Shared production path: backend runtime selector -> configured factory ->
RuntimeInstance -> RuntimeConversation -> owned TurnHandle stream/control.
Connect captures adapter/configuration before provider-specific model source
and preflight. OpenAI model source reads configuration only; its factory
never enters executable discovery, catalog, baseline, version or companion code.
Disconnect revokes HostToolScope before withdrawing composition/shutdown.
Reconnect constructs fresh runtime and conversation ownership.

Existing native adapter: fixed Responses HTTPS endpoint, HTTP/SSE, streaming
text, fragmented/multiple events, assembled function calls, serialized host
Tool requests, call_id correlation, Tool-result continuation, local cancellation,
owned shutdown, typed OpenAiAdapterError under RuntimeFailure, closed sanitized
diagnostics, redaction and eight Tool rounds. Host supplies registry/permissions
and active-turn leases. Adapter performs no repository operations.
Host-owned text replay is supported; NativeContinuation and model discovery
are unsupported. Explicit models are configured, never inferred from Codex.
Provider rejection is observed on the model request, separately from build
compatibility. No live API validation or credentials are required for tests.

Backend configuration: existing RAH_RUNTIME_PROVIDER, RAH_OPENAI_MODEL and
OPENAI_API_KEY. No key IPC, project persistence, arbitrary endpoint or new store.
Desktop's validation UI already presents runtime identity separately from
Codex upstream provider preference and supports this composition; no redesign.
Legacy command names are retained for IPC compatibility.

Existing adapter tests cover all twelve requested controls; Task 508 production
fixture covers shared Connect/conversation/text/authorized repo.status/continuation/
completion/Disconnect/stale retained port/repository isolation. Additional
controls will cover new default selection and Desktop cancel/reconnect.
Canonical Windows harness prepares sibling MCP/plugin fixture executables.
Cargo feature matrix already supports OpenAI, Codex, both and no-provider.
Release history and Codex certification records remain untouched.

## Decision

Desktop default becomes provider-openai; provider-codex stays optional legacy.
The existing selector is reused: no override prefers OpenAI when compiled,
Codex-only keeps its legacy default, explicit unavailable/unknown selection
fails closed. No silent provider fallback. No-provider fails closed.
Deterministic tests use local fixtures or explicit legacy factories rather
than production credentials. No new dependency, lockfile/version change.

ADR-B: ADRs 0001/0002/0005 permit a replaceable optional Codex adapter;
ADRs 0032/0033 already define neutral diagnostics/composition/host Tool lifetime.
ADR 0034 remains accepted legacy-adapter design; its unfinished implementation
is no longer a blocker for ordinary native OpenAI production.

## Validation plan

Serial stop-on-first-failure: fmt, workspace check, prepared fixture helpers,
workspace tests, all-target/all-feature warnings-denied Clippy, diff check;
then focused OpenAI/neutral suites, canonical Desktop tests, OpenAI-only build,
feature matrix/trees, local production acceptance and baseline-delta process
census, frontend static checks and Tauri permission matrix.
HostExplicit must remain exactly 11 statically and in registry composition.
Review authority against existing active repo, worktree, permission, lease,
Trusted Profile, remembered workspace and mutation-uncertainty boundaries.
No live provider calls, CLI certification, version bump, tag or release.
Commit/push only after outcome A and all required checks pass; require live
master equality and natural exact-head CI success.

## Original results before Task 511A

**F — DETERMINISTIC / WINDOWS CLOSURE FAILED.**

Production patch is frozen and uncommitted at the first required failure.
Default feature is provider-openai. Selection prefers OpenAI when compiled,
retains Codex-only legacy default and rejects unknown/uncompiled overrides.
No production UI/IPC redesign, public runtime API or authority implementation
was changed. Existing Task 508 fixture was extended with fresh reconnect and
held-open native request cancellation, but that extension has not executed.
Current README/architecture/guardrails/security documentation was updated;
historical tasks and ADR 0034 were not edited.

| Gate | Result |
| --- | --- |
| cargo fmt --check | PASS, exit 0 |
| cargo check --workspace | PASS, exit 0; fresh target, 1m38s; three default Desktop dead-code warnings |
| canonical Windows harness PrepareOnly | PASS, exit 0; sibling fixture helpers prepared |
| cargo test --workspace | FAIL, exit 101; Desktop test compilation; no executable test results |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | NOT RUN after failure |
| git diff --check | PASS as final non-executing patch inspection |
| focused OpenAI and neutral suites | NOT RUN after failure |
| canonical Desktop executable tests | NOT RUN after failure |
| OpenAI-only Desktop build / dependency-tree matrix / smoke | NOT RUN after failure |
| production local text/Tool/continuation/cancel/reconnect/shutdown acceptance | NOT RUN after failure |
| model-preflight and diagnostic/redaction executable controls | NOT RUN after failure |
| process census / no-Codex-launch acceptance proof | NOT RUN after failure |
| feature-matrix checks / frontend static tests / Tauri permission matrix | NOT RUN after failure |

Exact first blocker: unguarded Codex-specific assertions in
crates/rah-desktop/src/desktop_preferences.rs:810 and :838, inside
v4_explicit_modes_round_trip_and_same_id_remains_distinct and
legacy_ids_absent_from_catalog_remain_advertised_and_inherit_has_no_mode.
Four E0433 diagnostics reference the absent optional rah_runtime_codex crate;
two E0599 diagnostics reference feature-excluded codex_model_config().
These are test-only compilation dependencies, not evidence of a production
runtime dependency or a missing native capability. No retry, test weakening,
restoration of the Codex default or correction was attempted after this failure.

Evidence is preserved in target/task511/evidence/workspace-check.log,
workspace-test.log and the timestamped canonical PrepareOnly directory.
Static HostExplicit mapping remains exactly 11; executable registry proof
did not run. Authority review found no change to registry dispatch,
HostToolScope, active repo/worktree, leases, permissions, Trusted Profiles,
remembered-workspace semantics, provider/model authority or mutation uncertainty.
Native endpoint/origin restrictions and externally configured credentials
remain unchanged. No live provider requests were performed.

Cargo.lock and dependencies are unchanged; no version bump, tag or release.
HEAD and origin/master remain e680f689129aa430b9865cb5d662682e43f9136a.
No commit/push occurred, so no new exact-head CI claim is made.
Final task worktree has eight modified tracked files and this untracked report;
primary preserved Task 510C2 WIP remains untouched.

Suggested next bounded correction: separate the two preference tests' neutral
persistence assertions from their optional Codex translation assertions, keeping
both behaviors tested in their applicable feature modes. Then restart the
required serial closure; do not resume CLI certification/admission work.

## Task 511A — preference test composition correction

Verified before editing: branch `task511-native-openai`, starting and current
HEAD `e680f689129aa430b9865cb5d662682e43f9136a`, all eight modified tracked
cutover files and this untracked report preserved. Primary 510C2 WIP untouched.

Assertion audit before editing:

| Original test | N — neutral assertions retained unconditionally | C — Codex assertion moved |
| --- | --- | --- |
| `v4_explicit_modes_round_trip_and_same_id_remains_distinct` | Exact canonical bytes for all four ID/mode cases; restored selection equality; advertised/custom selection inequality; canonical encoding inequality | Restored selection converts to explicit Codex configuration with OpenAi provider, for all four cases |
| `legacy_ids_absent_from_catalog_remain_advertised_and_inherit_has_no_mode` | Catalog absence; stored legacy ID; Advertised mode for versions 1–3; inherited default selection, absent model/mode, and canonical inherited bytes for versions 1–4 | Parsed legacy selection converts to explicit Codex configuration with OpenAi provider, for versions 1–3 |

The two original tests keep their names and all N assertions. C assertions retain
their original predicates and input cases in two separate tests:
`v4_explicit_modes_translate_to_codex_openai_config` and
`legacy_ids_absent_from_catalog_translate_to_codex_openai_config`, each gated
with `#[cfg(feature = "provider-codex")]`. No production API, shim, OpenAI
translation assertion, dependency or feature wiring change was added.
Desktop default remains `provider-openai`; `provider-codex` remains optional.

Focused gates, using the preserved `target/task511` for both CARGO_TARGET_DIR
and RAH_TEST_TARGET_DIR:

- `cargo test -p rah-desktop --bin rah-desktop desktop_preferences::tests`:
  PASS, exit 0; 27 passed, 0 failed, 0 ignored, 315 filtered out.
- `cargo test -p rah-desktop --bin rah-desktop --features provider-codex desktop_preferences::tests`:
  PASS, exit 0; 29 passed, 0 failed, 0 ignored, 353 filtered out. Both new
  Codex translation tests explicitly executed and passed.

Execution notes: the initial Python edit command failed because Python was
unavailable; the actual correction used apply_patch. The first Codex-enabled
launch used PowerShell ErrorActionPreference=Stop, which terminated on Cargo's
duplicate-target warning before a gate result. That log was preserved; the
valid launch without that wrapper setting completed successfully. No failed
test result was retried or discarded.

Evidence: `target/task511/evidence/task511a-preferences-openai.log`,
`task511a-preferences-codex.log` (wrapper failure), and
`task511a-preferences-codex-validation.log`.

Resumed serial gates:

- `cargo test --workspace`: PASS, exit 0; 1,061 passed, 0 failed,
  16 ignored across 60 result groups. Desktop: 331 passed, 0 failed,
  11 ignored. Original compilation blocker is passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  PASS, exit 0.
- `cargo test -p rah-runtime`: PASS, exit 0; 18 passed, 0 failed, 0 ignored.
- `cargo test -p rah-runtime-openai`: PASS, exit 0; 14 passed, 0 failed,
  0 ignored, plus successful empty doctest group.
- Canonical `scripts/windows-desktop-test-gate.ps1`, using the same isolated
  target and `target/task511/evidence/task511a-canonical` output: PASS, exit 0;
  331 passed, 0 failed, 11 ignored. Evidence directory:
  `20261008-194940-956-5706d1838eba4feda8e483395498cf35`.

Task 511A classification: **A — CODEX-SPECIFIC TEST ASSUMPTION CORRECTED;
TASK 511 VALIDATION RESUMED**. Overall Task 511 closure continues serially;
this is not yet complete cutover acceptance or authorization to commit.

## Latest Task 511 closure result

**F — DETERMINISTIC / WINDOWS CLOSURE FAILED.** Task 511A itself remains
**A — CODEX-SPECIFIC TEST ASSUMPTION CORRECTED; TASK 511 VALIDATION RESUMED**.
No test failure was encountered after the preference correction. The first
new required failure is the Windows process-proof observer prerequisite.

Additional gates completed in required serial order:

| Gate | Result |
| --- | --- |
| Explicit OpenAI-only selection tests (`--no-default-features --features provider-openai runtime_selection::tests`) | PASS, exit 0; 2 passed, 0 failed |
| Codex-only selection tests (`--no-default-features --features provider-codex runtime_selection::tests`) | PASS, exit 0; 2 passed, 0 failed |
| Combined selection tests (`--no-default-features --features provider-openai,provider-codex runtime_selection::tests`) | PASS, exit 0; 3 passed, 0 failed |
| No-provider selection tests (`--no-default-features runtime_selection::tests`) | PASS, exit 0; 2 passed, 0 failed |
| `cargo tree -p rah-desktop --no-default-features -e normal`, with each corresponding feature set | PASS for all four; native only has OpenAI/no Codex, legacy only has Codex/no OpenAI, both has both, none has neither |
| `cargo build -p rah-desktop --bin rah-desktop --no-default-features --features provider-openai` | PASS, exit 0; three pre-existing default-composition dead-code warnings |
| OpenAI-only executable `--runtime-composition-smoke` with empty PATH and backend selection openai | PASS, exit 0; adapter=openai, codex_compiled=false, http_requests=0, passed=true |
| `cargo test -p rah-desktop --bin rah-desktop --no-default-features --features openai-fixture tests::task508_production_openai_connect_turn_tool_disconnect_repository_authority -- --exact --nocapture` | PASS, exit 0; 1 passed, 0 failed, 342 filtered out; local text/Tool/continuation/Disconnect/revocation/repository-switch/fresh reconnect/active-request cancellation/final shutdown assertions execute |
| Baseline-delta no-Codex product process proof | FAIL at observer registration; measured product acceptance process was not launched |
| Frontend/static and Tauri permission checks | NOT RUN; frontend, command inventory and permissions were not touched; serial execution stopped at observer failure |
| Final formatting closure | NOT RUN after new failure; original pre-correction fmt passed, resumed Clippy passed |
| Final `git diff --check` | PASS, exit 0; non-executing patch inspection after stopping at observer failure |

Exact first new failure:

```text
Register-WmiEvent -Query 'SELECT * FROM Win32_ProcessStartTrace' -SourceIdentifier ...
Register-WmiEvent : Access denied
CategoryInfo: NotSpecified: (:) [Register-WmiEvent], ManagementException
FullyQualifiedErrorId: System.Management.ManagementException,Microsoft.PowerShell.Commands.RegisterWmiEventCommand
```

The process-proof command returned exit 1. Registration preceded baseline capture
and the intended measured acceptance launch, so neither baseline-delta ownership
nor no-Codex launch proof exists. The earlier unobserved deterministic native
acceptance passed; it is not substituted for process proof. No observer retry,
fallback census, elevation, host configuration change or live provider request
was attempted. Error evidence is retained at
`target/task511/evidence/task511a-process-observer-error.log`.

Other additional evidence: `task511a-matrix-{openai,codex,both,none}.log`,
`task511a-tree-{openai,codex,both,none}.log`,
`task511a-openai-only-build.log`, `task511a-openai-empty-path-smoke.log`,
and `task511a-native-production-acceptance.log`, all under the same evidence root.

Cargo.lock remains unchanged; no new dependency or dependency edge was added.
HostExplicit remains exactly 11: unchanged static HostInvocationKind/mapping;
`rename_file_is_the_eleventh_host_tool_and_requires_rename_authority` passed
in workspace and canonical Desktop tests, including its eligible-count assertion.
Authority/security review: only test composition was corrected in Task 511A.
The preserved cutover continues to use existing provider-neutral composition;
no ToolRegistry, HostToolScope, repo/worktree authority, lease, permission,
Trusted Profile, remembered-workspace, provider/model-authority or replay policy
implementation changed. No authority/security regression was observed.
ADR disposition remains ADR-B; historical ADR 0034 and Codex evidence untouched.

Starting and current HEAD remain `e680f689129aa430b9865cb5d662682e43f9136a`
on `task511-native-openai`. All original eight modified files and report are
preserved, plus the single preference-test correction (nine modified tracked
files total). Primary Task 510C2 WIP remains untouched. No commit, push, tag,
release or version bump; no new exact-head CI claim.

Next bounded prerequisite: establish an authorized, reviewable Windows
process-start observation mechanism that can prove product process ownership
without conflating controlling development Codex infrastructure. Do not treat
the denied observer registration as evidence against native OpenAI runtime
behavior or restore Codex as a default dependency.

## Task 511B — ordinary-user process proof attempt

Overall: **F — DETERMINISTIC / WINDOWS CLOSURE FAILED**.
Starting/current isolated HEAD: e680f689129aa430b9865cb5d662682e43f9136a.
Branch task511-native-openai; original nine modified tracked files preserved.
No product source was changed by Task 511B. Primary 510C2 WIP untouched.
Historical Register-WmiEvent Access denied evidence remains unchanged.

P1 reconfirmation: selected normal dependency tree with --no-default-features
--features openai-fixture contains rah-runtime-openai and no rah-runtime-codex.
Measured Cargo fingerprint features are exactly openai-fixture, provider-openai;
provider-codex is disabled. Selected provider is openai, both explicitly in the
acceptance production connection and RAH_RUNTIME_PROVIDER in its environment.
Production executable target/task511/debug/rah-desktop.exe SHA256:
B2BC1C992954F82239E79DE39C02986B9096AABE2414B99E6B8616D7965765A3.
Measured preserved acceptance executable:
target/task511/debug/deps/rah_desktop-d14297d5ac73a8a5.exe SHA256:
D81CF8FCE458848E5F84CF63D29726483025AF66606A346FB80A51D620B57783.
No rebuild/configuration change or full feature-matrix rerun.

Static fallback/lookup audit: runtime_selection::select rejects unknown or
uncompiled explicit providers; configured_factory matches a single selected
adapter. Its native arm validates OpenAiFactory and returns native diagnostics
on failure. production_composition propagates factory/create errors; no retry
through Codex. model_source::resolve OpenAi reads configured native model only.
Codex discovery, version/schema/baseline and companion paths require the
provider-codex feature, excluded from this artifact. Measured fixture asserts
codex_resolver=0 and codex_runtime_construction=0 after final shutdown.
No automatic Codex fallback is reachable in the measured composition.

Observer: ordinary-user System.Diagnostics.Process enumeration on a dedicated
.NET thread, exact case-insensitive executable-name matching. Identity includes
PID, UTC start time and name; executable paths recorded where accessible.
No WMI subscription, elevation, security policy, injection or host modification.
Requested sleep interval 10 ms; 407 completed enumeration samples; maximum
observed sample-start gap 46.8303 ms. Polling is not mathematically lossless for
arbitrarily short processes; its evidence is combined with P1.
Parent PID not collected, as optional supplemental evidence.

Frozen B0-511B (all times UTC):
- codex.exe PID 8860, start 2026-10-08T11:16:04.0645201Z;
  path C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe.
- codex-code-mode-host.exe PID 5472, start 2026-10-08T11:16:36.6561165Z;
  same vendor bin directory, codex-code-mode-host.exe.
Baseline frozen before polling; no identities added later or killed.
Poller active 2026-10-08T12:16:02.8323970Z. One-second prelaunch stability
interval PASS; baseline remained stable through disposable control completion.
Self-test PASS: PING.exe PID 16716, start 2026-10-08T12:16:03.8805257Z,
first observed 2026-10-08T12:16:03.9017960Z, C:\WINDOWS\System32\PING.EXE.
Launched after monitoring, absent from initial snapshot, observed, exited,
no exact-identity residual. Control was unrelated to Codex.

Measured acceptance launched 2026-10-08T12:16:06.6806050Z; process exit observed
2026-10-08T12:16:13.5064459Z. Stdout reports the exact production acceptance
PASS: 1 passed / 0 failed / 342 filtered out, 6.51 seconds; stderr empty.
It executes Connect/native runtime/conversation/streaming/authorized Tool/
continuation/completion/Disconnect/reconnect/cancellation/final shutdown.
Established acceptance PATH retained (full exact PATH in process-proof JSON),
CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR point at target/task511.
Earlier empty-PATH smoke PASS preserved; empty PATH was not an established
configuration for the full repository/Tool acceptance and was not introduced.

Observed matching Codex identities: exactly the two B0 identities above.
ObservedDelta=0 for both names across the complete monitoring interval.
Two-second bounded post-shutdown observation completed with zero new identity;
monitor stopped at 2026-10-08T12:16:15.5311665Z, no observer error.
The prior report did not specify a post-shutdown interval; this attempt explicitly
used two seconds rather than claiming an undocumented historical duration.

Closure blocker: Start-Process -PassThru process object's ExitCode was null
after HasExited/WaitForExit. The wrapper compared null against zero, recorded
"measured acceptance failed" and returned exit 1. This is an observer-wrapper
exit-status capture failure, not a failing Rust test. Preserve the original JSON
and logs unchanged. No retry or substitution of the previous acceptance PASS.
No-Codex launch observation is zero-delta with valid P1, but full process-proof
closure is not accepted because the measured launch exit status was not captured.
No commit/push; do not classify overall Task 511 A.

Evidence under target/task511/evidence:
- task511b-observer.ps1 (reviewable observer, outside product source)
- task511b-B0.json, task511b-process-proof.json
- task511b-control.log
- task511b-acceptance.stdout.log, task511b-acceptance.stderr.log
- task511b-measured-tree.log, task511b-measured-fingerprint.json
- task511b-start-status.txt

Cargo.lock unchanged; no dependency added. HostExplicit remains 11 with preserved
executable coverage. No ToolRegistry, HostToolScope, active repo/worktree authority,
leases, permissions, Trusted Profiles, remembered workspace semantics, mutation
uncertainty or provider/model authority implementation changed. Native OpenAI
gains no authority beyond the neutral runtime port. ADR-B retained; ADR 0034 and
historical Codex evidence untouched. Previously green suites preserved; no frontend
or Tauri suites rerun for this outside-product observer.
Next bounded correction: capture the measured process exit code reliably while
retaining process-handle ownership, then conduct a separately recorded attempt.

## Task 511C — retained child exit-status closure

All section 28 gates PASS. Publication and exact-head CI are required separately
before final overall A. Starting HEAD e680f689129aa430b9865cb5d662682e43f9136a.
No product source changed in 511C: byte-equal before/after product diffs.
All prior green Task 511A validation and Task 511B evidence are preserved.

Correction: ProcessStartInfo with UseShellExecute=false, CreateNoWindow=true,
redirected streams; directly constructed Process.Start(), retained same object
and native handle, ReadToEndAsync for both streams, HasExited monitoring,
WaitForExit(), then ExitCode from that same object. Same handle confirmed.
External scripts remain in ignored target/task511/evidence, outside product.

X1: cmd.exe /d /c exit 0, PID 15964, start 2026-10-08T12:21:49.8435454Z,
WaitForExit completed, exact ExitCode 0. X2: cmd.exe /d /c exit 7, PID 11024,
start 2026-10-08T12:21:49.9382508Z, WaitForExit completed, exact ExitCode 7.
Both retain object/handle through completion; stdout/stderr empty.

Fresh B0-511C captured 2026-10-08T12:21:49.9733601Z onward, independently
of B0-511B. Its only Codex identities (UTC) and all observed identities:
- codex.exe PID 8860, start 2026-10-08T11:16:04.0645201Z.
- codex-code-mode-host.exe PID 5472, start 2026-10-08T11:16:36.6561165Z.
Paths accessible under the existing npm Codex vendor bin directory; full paths
in task511c-B0.json. These are controlling infrastructure, not product children.
One-second baseline stability PASS; stable through disposable poller control.
Poller active 2026-10-08T12:21:49.9980073Z before product launch.
Poller self-test PASS: PING.EXE PID 16576, start 2026-10-08T12:21:51.0230579Z,
first observed 2026-10-08T12:21:51.0321548Z; exit 0; no identity residual.

P1 remains valid: provider=openai; fingerprint exactly openai-fixture and
provider-openai; provider-codex disabled. Fresh normal dependency tree includes
rah-runtime-openai and excludes rah-runtime-codex. Source unchanged; audited
selector/factory/model-source path still has no reachable automatic Codex
fallback or discovery. No WMI, elevation or new Codex control process.

Measured child PID 1936, start 2026-10-08T12:21:53.2014716Z:
target/task511/debug/deps/rah_desktop-d14297d5ac73a8a5.exe
Arguments: tests::task508_production_openai_connect_turn_tool_disconnect_repository_authority --exact --nocapture.
Artifact SHA256 unchanged:
D81CF8FCE458848E5F84CF63D29726483025AF66606A346FB80A51D620B57783.
Retained handle 2520; WaitForExit completed; same object/handle; ExitCode=0.
Output coherent: 1 passed / 0 failed / 342 filtered out; stderr empty.
Full Connect/native runtime/conversation/stream/RAH Tool/continuation/completion/
shutdown plus reconnect/cancel controls executed. No live provider request.
Shutdown observed 2026-10-08T12:21:59.2196671Z; post-shutdown 2000 ms PASS,
zero new identity. Monitor stopped 2026-10-08T12:22:01.2570530Z; no error.
Requested interval 10 ms; 361 samples; maximum sample-start gap 47.0564 ms.
ObservedDelta codex.exe=0; codex-code-mode-host.exe=0; post-shutdown delta=0.
Combined P1 and sampled process proof supports no product Codex launch;
polling is not event-perfect for arbitrarily short-lived processes.

Final git diff --check PASS. Cargo.lock unchanged, no new dependency.
HostExplicit=11 with prior executable coverage preserved. ToolRegistry,
HostToolScope, active repository/worktree authority, leases, permissions,
Trusted Profiles, remembered workspace semantics, mutation uncertainty and
provider/model authority unchanged. ADR-B; ADR 0034 historical legacy design.
No full workspace rerun solely for external harness correction.

Evidence: task511c-child.ps1, task511c-observer.ps1, task511c-wrapper-tests.json,
task511c-B0.json, task511c-process-proof.json, task511c-measured-tree.log and
stdout/stderr logs under target/task511/evidence. Historical evidence untouched.
A preliminary Select-String invocation reversed positional path/pattern arguments;
its read-only check was corrected with named arguments. Tree confirmed unchanged;
this was not a measured acceptance or observer failure, and no measurement retry occurred.
