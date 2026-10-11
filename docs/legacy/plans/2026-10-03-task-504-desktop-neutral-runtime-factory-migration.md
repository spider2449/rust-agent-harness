# Task 504 ? Desktop neutral runtime factory migration

## 1. Starting checkpoint

Starting and final HEAD: 3ab73e32d33b37acb8027d7ad4b4c7b8e078d077.
The worktree was clean before edits. Previous exact-head CI 37097578196 is
PASS according to the task checkpoint; no new CI is claimed.

## 2. ADR decision

ADR-A ? create ADR now. ADR 0033 records durable neutral ownership, host Tool
admission/revocation, identity separation, effect accounting and typed errors.
It refines ADRs 0001/0005/0006 without granting authority or stabilizing the API.
The implementation and production adoption remain unvalidated at this stop.

## 3. Previous direct Codex coupling

ConnectionState, ActiveChat and publication structures stored Arc<CodexRuntime>.
Connect directly constructed the bridge and invoked preflight_selected_model.
run_chat started concrete runtime turns; cancellation, disconnect, exit and hard
recovery called concrete lifecycle methods. Desktop had no explicit private
app-server or thread ID fields.

## 4. New composition root (WIP)

runtime_composition::configured_codex_factory configures only Codex.
create_and_preflight consumes dyn ConfiguredRuntimeFactory and yields a neutral
instance. After the existing presentation gate succeeds, bind_conversation adds
host-owned Tool scope and a neutral conversation. Ready publication retains
existing generation/currentness checks. Adapter configuration gains canonical
host workspace binding and effective thread cwd verification using existing
adapter helpers. Executable version/schema admission still uses ProcessTransport.

## 5. Runtime storage (WIP)

Connection/publication/ActiveChat store Arc<DesktopRuntime>, a host owner whose
fields are Arc<dyn RuntimeInstance>, Arc<dyn RuntimeConversation>, HostToolScope
and neutral TurnControl. No adapter-private lifecycle type is stored.
Desktop production compilation has NOT been executed; this is source evidence.

## 6. Conversation and turn ownership (WIP)

One host-assigned ConversationId is distinct from each host-issued SessionId.
Provider thread/turn identities remain adapter-private. Desktop continues full
text replay. Neutral send returns the stream and control; the existing Desktop
terminal coordinator and model-turn exclusivity remain in place. No provider
identity is added to UI persistence. Native continuation is not enabled by UI.

## 7. Model preflight (WIP)

Explicit default-provider selection calls neutral discovery and reuses the
unchanged model_preflight::present policy. Complete catalog membership proceeds;
absence retains alternatives and stops before conversation/inference/ready.
RuntimeDefault is NotChecked. Adapter reports discovery Unsupported for existing
non-default Codex provider configurations, preserving the old NotChecked behavior.
The adapter translates neutral RuntimeDefault to Codex Inherit. Legacy config
conversion remains composition-only. Existing Task 499 tests are unchanged.

## 8. Host Tool port (WIP)

HostToolScope receives the same registry and allowed permissions as the prior
bridge. Its live port calls existing authorize_tool_dispatch and
 authorized_tool_dispatch. There is no alternate dispatch primitive, new Tool,
permission or registry interface. Distinct Tool requests remain allowed within
one lease. Desktop's host coordinator still controls model-turn admission.

## 9. Lifecycle and revocation (WIP)

Disconnect performs existing active-chat/index/host-operation policy checks,
then revokes under coordination before replacing Connected with Disconnecting.
Exit and hard recovery also revoke before state withdrawal. Neutral shutdown
owns provider cleanup; host scope accounts for already admitted effects.
Host owner Drop revokes retained weak ports. Dead-runtime status observation
revokes the scope before withdrawing ready state. No validation of this new
production binding has run.

## 10. Disconnect semantics

Repository membership, selected repository and conversation persistence are not
removed. Existing commit-context withdrawal and provider activation cleanup stay
host-owned. Error-only Disconnect semantics remain unchanged. No history deletion.

## 11. Repository switching

Existing disconnected-runtime policy remains unchanged. Existing deterministic
repository-switch fixtures now construct their runtime through the new binding.
These Desktop fixtures have NOT been executed. Stale-handle tests are added but
unexecuted; repository authority isolation is not newly certified.

## 12. Cancellation and recovery

Desktop retains graceful timeout, terminal arbitration and hard-shutdown policy.
Neutral TurnControl performs provider cancellation. Admitted effects remain
host-owned; completed effects are not rolled back and no replay is added.
Startup, preflight and conversation-binding failure paths invoke neutral cleanup.
Task 503 adapter cancellation tests pass in Phase A; new Desktop tests are unrun.

## 13. Failure envelope

Neutral construction/send/discovery/shutdown retain RuntimeFailure and typed
process-local sources. Frontend turn errors use sanitized diagnostics. The
composition presentation mapping downcasts locally only to preserve existing
closed Codex UI error codes. New typed-source/sanitization Desktop tests are
unrun. Existing adapter typed-failure tests pass; no envelope flattening is added.

## 14. Remaining direct Desktop Codex references

| Location | Classification | Purpose |
| --- | --- | --- |
| main.rs model/provider types, PreparedCodexConnection | composition/configuration only | Existing host model/provider configuration |
| runtime_composition.rs factory/config imports | composition/configuration only | Fixed Codex factory configuration |
| codex_baseline.rs preferred version | artifact certification only | Existing certified selection |
| main.rs preferred version and baseline presentation | artifact certification only | Existing certified status |
| main.rs CodexAdapterError and closed UI mapping | temporary migration debt | Preserve exact error presentation at adapter composition edge |
| main_tests.rs historical standalone CodexRuntime probes | temporary migration debt, test-only | Existing ignored adapter/process ownership controls |
| model_preflight.rs and runtime_composition.rs test Codex errors | composition/configuration only, test-only | Typed-source and configuration regressions |

No concrete Codex lifecycle storage or ordinary production start/cancel/shutdown
call remains in main.rs. Source coupling is reduced, but production compilation
and behavior are not certified. No unexpected direct lifecycle coupling was found
in the source audit; that does not substitute for Desktop validation.

## 15. Focused validation ? exact stop evidence

Stable-source command, isolated previously nonexistent target:

    CARGO_TARGET_DIR=F:/temp/rah-task504-target
    RAH_TEST_TARGET_DIR=F:/temp/rah-task504-target
    cargo test -p rah-runtime -p rah-runtime-codex

Exit 101. Log: F:/temp/rah-task504-evidence/phase-a.log.
rah-runtime: 18 passed, 0 failed (8 unit, 6 cancellation, 1 echo, 3 model-failure).
rah-runtime-codex library: 99 passed, 3 failed, 1 ignored (103 tests).
Aggregate executed: 117 passed, 3 failed, 1 ignored. Later Codex integration and
doctests were not reached. All six Task 503 experimental adapter tests passed;
legacy runtime_tests and catalog tests also passed where executed.

Failures:

- bridge_tests::repository_search_dispatches_through_the_generic_bridge,
  bridge_tests.rs:4257: expected success=true; response id 914 has success=false
  and content text "RAH tool execution failed".
- bridge_tests::repository_list_dispatches_through_the_generic_bridge,
  bridge_tests.rs:4257: expected success=true; response id 915 has success=false
  and content text "RAH tool execution failed".
- bridge_tests::host_composed_repo_create_directory_uses_generic_bridge_once,
  bridge_tests.rs:2060: expected success=true; response id 1840 has success=false,
  path "existing/new-directory", status "precondition_failed", uncertain=false.

These are failed behavioral assertions, not a linker or toolchain failure.
The captured response alone does not identify the underlying observer/Tool
cause. No timeout, scheduling, bridge-race or migration-causation claim is made.
No source correction, retry or passing rerun followed.

All ten initial frozen files matched their exact SHA256 after Cargo exited;
source-stability.json records equality. No cargo/rustc process remained.
Preserved stopped-tracked.patch and stopped-untracked copies contain the tested
WIP. The only subsequent edit updates this report with stop evidence.

## 16. Desktop validation

Phase B and canonical Phase C NOT RUN after Phase A failure. Desktop compilation,
new construction/Tool/revocation/cancel/exit tests, existing model-preflight tests
and repository-switch fixtures are unverified. No assertions were weakened.

## 17. Full deterministic validation

Workspace fmt/check/test/Clippy, frontend/static tests, Tauri inventory, metadata
and executable HostExplicit gate NOT RUN after stop. cargo fmt was executed
before source freeze. Closure git diff --check passes. New target/logs preserved.

## 18. Production validation

Connect, ordinary turn, harmless Tool, Disconnect and absent-model cases NOT RUN.
No live Codex executable, paid model turn or alternate runtime was invoked.
Certified 0.157.1 behavior has not been re-certified on this WIP.

## 19. HostExplicit

Static enum and routing inspection: exactly 11, unchanged. host_invocation.rs,
provider_composition.rs, capability files and Cargo manifests/lockfile have zero
diff against starting HEAD. Executable verification NOT RUN; no new certificate.

## 20. Authority and security impact

Intended authority impact: none. Existing dispatch, registry, permissions,
repository selection, Trusted Profiles, remembered-workspace semantics and
mutation uncertainty remain the boundaries. No dependency edge, new provider,
v0.34 capability, baseline change, version bump, tag or release. Production
lifecycle correctness remains unproven until Desktop gates pass.

## 21. Final classification and Git state

E ? DETERMINISTIC VALIDATION FAILED

Work stopped at the first failing command. No commit, staging, push or new CI.
HEAD remains the authoritative checkpoint. Final worktree is dirty with Task 504
WIP only; it is deliberately preserved and is NOT reported clean. Seven tracked
source files, one new source module, ADR 0033 and this report comprise the patch.

## 22. Recommended next task

A separately scoped failure diagnosis should preserve this exact WIP and failed
logs, recover underlying Tool/observer evidence for the three legacy bridge
assertions, and distinguish source causation from environment/budget behavior
before any correction. Then resume required validation from a stable source.
Do not publish or add another provider before this migration is validated.

## 23. Task 504A diagnosis reference

[Task 504A](2026-10-03-task-504a-generic-bridge-failure-diagnosis.md) records
bounded diagnostic evidence and outcome D (root cause still incomplete).
The three tests exercise the unchanged legacy bridge rather than the new
neutral port; all individual diagnostics and one instrumented Phase A Codex
library run passed. Those passes do not explain the frozen failures or replace
the required migration gates. Instrumentation was removed with exact source
hash equality. No correction or Desktop validation resumption occurred.
This report's historical E classification and stop evidence remain unchanged.

## 24. Task 504D frozen validation restart

[Task 504D](2026-10-03-task-504d-frozen-source-validation-restart.md) resumed under
Task 504C G1 on unchanged frozen migration source. Phase A passed: 137 passed,
0 failed, 1 ignored; both doctest groups had zero tests. None of the historical
three bridge failures recurred. Desktop helper preparation passed.

Phase B exited 101 before any tests executed: E0308 OsString/PathBuf mismatches
at main.rs:8513 and main_tests.rs:6316/6812; E0599 missing neutral TurnHandle
into_events at main_tests.rs:6498. These are deterministic source compilation
errors. Task 504D classification: **B — DETERMINISTIC VALIDATION FAILED**.
No source correction, retry or later gate ran. Task 504 remains incomplete.
All eight source hashes and ADR 0033 matched the frozen checkpoint. No commit,
push or CI; HEAD unchanged and migration WIP preserved dirty/uncommitted.
See the linked report for exact command, logs, counts and unverified boundaries.
A separate narrow correction task is recommended; no correction is authorized
or performed by this validation-only task. Historical Task 504 evidence above
remains preserved.
