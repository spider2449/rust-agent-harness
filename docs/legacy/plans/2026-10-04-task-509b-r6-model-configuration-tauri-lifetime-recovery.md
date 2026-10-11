# Task 509B-R6 — model_configuration Tauri lifetime recovery

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`.
Preserved dirty, unstaged Task 509 WIP (14 tracked modifications, six untracked files).
Evidence directory: `F:/temp/task509br6-evidence/`.

## Plan and audit

Correct only the existing command boundary; run the exact R5 presentation gate,
immediate 47-surface inventory, focused source suite, nine JS syntax checks and
seven frontend suites, static/metadata/HostExplicit, serial workspace gates,
canonical Windows Desktop and bounded manual acceptance. Stop on the first
stable-source failure. Only A permits commit, normal master push and exact-head CI.
No Task 510B, live OpenAI, runtime-baseline change or 0.160.0 certification.

R5 E0277 points to desktop_model_configuration_commands.rs:11 (direct DTO return).
State input at line 10 contains a lifetime; the macro explicitly requires Result
for asynchronous reference/lifetime inputs. E0597 points to the command macro at
line 8, invoked by main.rs:9459–9507: __tauri_message__ is borrowed for 'static
but dropped by the wrapper. tauri-2.11.5/src/ipc/command.rs:286 requires
Future + Send + 'static. The State input derives from that invoke message and
was used across helper await at line 12; the helper refresh awaits at line 18.
Complete original diagnostics are reproduced below and preserved verbatim in
`r5-complete-diagnostics.log`.

Existing patterns: synchronous desktop_status_commands::app_status uses State
and an infallible DTO. trusted_profile_selection (main.rs:4845 vicinity) does
likewise. Async host_invoke_read, host_prepare_repo_patch and edit/delete
preparation use State and Result<_, FrontendError>. Owned AppHandle is used by
run_host_tool (main.rs:3017; managed state at 3047), run_chat (8750 vicinity),
and the endpoint readiness future (5216–5224). These own their handle before
obtaining managed state. Local tauri-macros-2.6.3/src/command/wrapper.rs:178–250
checks only reference/lifetime inputs; body_async:361–397 owns the invoke in
an async move and serializes the returned future's output.

L2 selected: `async fn model_configuration(app: AppHandle) -> ModelConfigurationPresentation`.
The owned injected AppHandle supplies `app.state::<DesktopAppState>().inner()`
to the unchanged helper. No borrowed State command parameter escapes the invoke.
A local managed-state borrow spans the await, backed by the owned handle inside
the future; it is not an invalid invoke-message lifetime. No detached task added.
L3 unnecessary: the owned input permits the existing infallible DTO. No error
contract or provider diagnostic is added. Compilation proves the macro contract.

L1 audited and rejected for this narrow task: model_source::refresh performs
asynchronous artifact measurement and catalog discovery on cache miss. Only this
presentation path and production Connect composition invoke refresh. A sync read
alone would leave the initial picker unavailable; relocating discovery would
change lifecycle/trigger semantics. Refresh remains in the host-owned source
module, not implemented in the wrapper. A future pure snapshot read would need
an independently owned discovery lifecycle; this task does not introduce one.
The folded response, lifecycle lock, context rebinding, generation/request owner,
newest-request-wins, stale success/error checks and provenance are unchanged.

## Validation

Exact R5 command: `cargo test -p rah-desktop model_configuration_presentation_is_closed_and_sanitized -- --test-threads=1`.
Both target variables: `F:/temp/rah-task509a2-target-run2`.
Compilation PASS, **1 passed / 0 failed / 0 ignored**, 377 filtered; exit 0.
No linker corruption or fresh-target retry. `focused-presentation.log` and `.exit`.
Immediate inventory PASS: **47 / 47 / 47 / 47 / 47**; `inventory.log`, exit 0.
Only the boundary file was formatted after these gates; no behavioral change.

Focused command: `cargo test -p rah-desktop model_source::tests -- --test-threads=1`.
PASS **17 / 0 / 0**, 361 filtered, exit 0 (`focused-source.log`). Snapshot
ownership, context/artifact generation, newest-request-wins, stale success/error,
loading withdrawal, Advertised membership, explicit/restored Custom, malformed
IDs, Inherit, native configured-only/None, provider switching, compatibility
reset, Disconnect revocation and absent `gpt-6.1-sol` all passed deterministically.

Frontend: all **9** `node --check` checks and all **7** frontend suites PASS,
exit 0, including three Edge layout fixtures. Suites report success rather than
numeric assertion totals. `Custom · unverified` remains exact UTF-8 provenance.
The status-authority suite provides the remaining frontend static checks.
No standalone model_source_snapshot path was restored; no inventory expectation
changed. Backend source/freshness logic remains byte-identical during the gates.

Metadata: `cargo metadata --no-deps --format-version 1` PASS; exactly **14**
members/packages, all **0.33.0**. Production `rah-runtime-codex -> sha2` direct
dependency remains intentional from R3; only its prior dev-to-production placement
diff exists. Cargo.lock has no HEAD diff. No R6 dependency or version change.
HostExplicit static closed host_kind match exactly **11**; executable
`cargo test -p rah-desktop host_allowlist_is_exact -- --test-threads=1` PASS
**1 / 0 / 0**, exit 0. No Tool authority correction was required.

Serial full gates PASS, exit 0: `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`git diff --check`. Workspace sum **1085 passed / 0 failed / 24 ignored**;
Desktop segment **358 / 0 / 20**. Logs/exit files: `workspace-*.log[.exit]`.
Frozen implementation hashes: `source-freeze.txt`; no edits while gates ran.

Canonical command:
`powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task509a2-target-run2 -OutputDirectory F:/temp/task509br6-evidence/desktop`.
Fresh run `20261004-154008-206-de01a7c8a1fa41f6a9a2cc2c693e2262`:
**358 passed / 0 failed / 20 ignored**, test and helper build exit **0**,
overall exit **0**, watchdog false. Status/stdout/stderr retained. No LNK1103,
LNK1223 or fresh-target retry occurred.

## Bounded actual-app acceptance and stop

After all deterministic gates, built `cargo build -p rah-desktop --features provider-openai`,
exit 0. Process-local TAURI_CONFIG overrides only the application identifier to
`org.rust-agent-harness.task509br6.acceptance`, isolating persistent preferences
from the normal Desktop profile; no repository config changed. Both provider
features were available; the launched process selected Codex, explicitly using
the saved exact 0.157.1 baseline. WebView2 debugging used local port 9509 and
an evidence-local WebView profile. App PID 8472; all inspection used CDP against
the actual `http://tauri.localhost/` page, not a fixture frontend. Harness sources,
app stdout/stderr and returned DTO/DOM snapshots remain in the evidence directory.

First inspection happened before Tauri globals initialized and reported
`Cannot read properties of undefined (reading 'core')`. A read-only DOM/global
diagnostic established the startup timing; the subsequent ready-state inspection
succeeded. This was an inspection-readiness issue, not a source correction or
replayed external effect. Original failure output remains `acceptance-initial.json`.

Initial ready-state result (`acceptance-initial-ready.json`): coherent real
command response, Codex/Inherit, generation 1/request 1, runtime_default,
inherited_unverified, truthful runtime-default provenance, Connect enabled.
The actual 0.157.1 catalog contained nine IDs: gpt-6-astra, gpt-6-sol,
gpt-6-luna, gpt-reserve, gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna, gpt-5.5,
codex-auto-review. `gpt-6.1-sol` was absent. No conversation or inference started.

First later failed acceptance gate: `acceptance-codex.js` selects OpenAI upstream
and the actual advertised `gpt-6-astra` via production DOM handlers, clicks Apply,
then polls the actual model_configuration command while awaiting backend
advertised_unverified **and** an enabled Connect button. After the bounded
30-second wait, exit **1**: `Error: acceptance wait timed out`, at wait line 8
and Advertised-Apply check line 23 (lines shifted by the evidence-results hook).
Complete CDP exception: `acceptance-codex.json`; exit: `acceptance-codex.exit`.
Only Inherit/catalog case completed; no Connect click occurred.

Captured failure UI (`acceptance-failure-state.json`): generation **2**, request
**2**, Codex upstream openai, Advertised gpt-6-astra; source and eligibility
**loading**, no artifact fingerprint, Connect disabled, no model/connection error.
Read-only backend capture (`acceptance-failure-backend.json`) subsequently returned
the same generation/request/context with the real nine-model **advertised_catalog**,
measured artifact, **advertised_unverified**, compatibility unverified. In that
same capture, DOM still showed Loading, Connect disabled, and gpt-6-astra as a
disabled saved option. The folded backend snapshot itself remained coherent.

Layer diagnosis: this acceptance harness deliberately introduced concurrent
configuration reads during Apply, so it does not prove single-reader Apply fails.
model_source.rs refresh returns an existing source (including Loading) at its
`!force && owner.source.is_some()` branch; another request can own resolution.
Frontend refreshModelSource/status.js:1368–1378 renders the response once, while
loadStatus:2151–2163 also reads/renders configuration. No publication subscription
or polling is added by R6. The observed ready backend/loading UI is a concurrent
presentation/publication case; exact request ordering was not instrumented.
No claim of a backend discovery failure, state incoherence, authority regression,
or proven permanent hang is made. No assertion was weakened or rerun to pass.

STOP after this stable-source acceptance failure. No second implementation fix.
Advertised Connect, stale Advertised live refusal, manual Custom/malformed/restore,
manual explicit-Inherit refusal, provider/runtime switching and native OpenAI
app acceptance were **not reached**. Their deterministic tests passed as above,
but those tests do not substitute for manual acceptance. Native configured-only,
no Codex catalog/no Custom/no-runtime branches are fixture-proven only in R6.
No native OpenAI request, credential use, chat prompt, turn or 0.160.0 work ran.
No runtime certification changed. App closed through CloseMainWindow successfully;
the process exited without force termination. Isolated acceptance artifacts remain.

## Final Task 509 disposition

**E — LATER VALIDATION FAILED**

The Tauri lifetime correction is proven by compilation, exact presentation test
and actual IPC execution. Folded snapshot coherence/freshness and picker policy
passed deterministic gates; actual Inherit/catalog passed. The later bounded
Advertised UI acceptance failed, preventing A and Task 509 completion.

No commit, push, exact-head CI, tag, release, version bump or Task 510B work.
HEAD remains `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`; worktree remains dirty
and unstaged, preserving all WIP. R6 edits only the command boundary, this plan
and a reference-only R5 note. Closure status/stat/diff-check retained; diff-check
PASS. Latest executed Tauri inventory remains **47 / 47 / 47 / 47 / 47**.

Next recovery requires separately scoped diagnosis of concurrent model-configuration
read/publication and UI loading completion, preserving the 47-command contract.
Deferred exact Task 510B scope remains runtime certification under ADR 0030:
exact artifact/schema audit, deterministic regression, direct and Desktop
certification evidence, and separately authorized admission/baseline update.
No general UI redesign or live native OpenAI work; E does not authorize starting it.

## ADR and authority

ADR-B; no new ADR. No new dependency edge, command, permission or capability.
No change to ToolRegistry, repository selection, host authorization or runtime
certification. No intended authority expansion. Cargo.lock untouched by R6.

## Complete R5 compiler diagnostics

~~~~text
cargo :    Compiling rah-desktop v0.33.0 (F:\coding\otherPrj\rust-agent-harness\crates\rah-desktop)
At line:49 char:1
+ cargo test -p rah-desktop model_configuration_presentation_is_closed_ ...
+ ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    + CategoryInfo          : NotSpecified: (   Compiling ra...es\rah-desktop):String) [], RemoteException
    + FullyQualifiedErrorId : NativeCommandError

error[E0277]: async commands that contain references as inputs must return a `Result`
    --> crates\rah-desktop\src\desktop_model_configuration_commands.rs:11:6
     |
  11 | ) -> ModelConfigurationPresentation {
     |      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unsatisfied trait bound
     |
help: the trait `desktop_model_configuration_commands::_::AsyncCommandMustReturnResult` is not implemented for
`ModelConfigurationPresentation`
    --> crates\rah-desktop\src\main.rs:1821:1
     |
1821 | struct ModelConfigurationPresentation {
     | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
help: the trait `desktop_model_configuration_commands::_::AsyncCommandMustReturnResult` is implemented for `Result<A,
B>`
    --> crates\rah-desktop\src\desktop_model_configuration_commands.rs:11:6
     |
  11 | ) -> ModelConfigurationPresentation {
     |      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     = note: required for the cast from `&ModelConfigurationPresentation` to `&dyn
desktop_model_configuration_commands::_::AsyncCommandMustReturnResult`

error[E0597]: `__tauri_message__` does not live long enough
    --> crates\rah-desktop\src\desktop_model_configuration_commands.rs:8:1
     |
   8 |   #[tauri::command]
     |   ^^^^^^^^^^^^^^^^-
     |   |               |
     |   |               `__tauri_message__` dropped here while still borrowed
     |   borrowed value does not live long enough
     |   argument requires that `__tauri_message__` is borrowed for `'static`
     |
    ::: crates\rah-desktop\src\main.rs:9459:25
     |
9459 |           .invoke_handler(tauri::generate_handler![
     |  _________________________-
9460 | |             desktop_status_commands::app_status,
9461 | |             trusted_profile_selection,
9462 | |             choose_trusted_profile,
...    |
9506 | |             host_cancel_tool_invocation
9507 | |         ])
     | |_________- in this macro invocation
     |
note: requirement that the value outlives `'static` introduced here
    --> F:\rust\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\tauri-2.11.5\src\ipc\command.rs:286:38
     |
 286 |       F: Future<Output = T> + Send + 'static,
     |                                      ^^^^^^^
     = note: this error originates in the macro `desktop_model_configuration_commands::__cmd__model_configuration`
which comes from the expansion of the macro `tauri::generate_handler` (in Nightly builds, run with -Z macro-backtrace
for more info)

Some errors have detailed explanations: E0277, E0597.
For more information about an error, try `rustc --explain E0277`.
error: could not compile `rah-desktop` (bin "rah-desktop" test) due to 2 previous errors

~~~~

## R7 reference

Concurrent-read acceptance isolation continues in
[Task 509B-R7](2026-10-04-task-509b-r7-model-configuration-concurrent-read-acceptance-isolation.md).
This reference does not rewrite the stopped R6 results or original acceptance logs.
