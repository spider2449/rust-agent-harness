# Task 499 — Codex model preflight salvage and validation

## Checkpoint and scope

Starting HEAD is exactly `1ccb1b39bccef36b1a780d29b188c95b878c501a`.
GitHub master was read back at this SHA. New clean worktree
`F:/temp/rah-task499`, branch `task-499-model-preflight`, was created from it.
Task 498 CI `37024248085` PASS is supplied checkpoint evidence.
The original main worktree and historical `F:/temp/rah-task495` are preserved.
Version remains 0.33.0; preferred/current certified Codex remains exactly
0.157.1; v0.34 capability remains NONE SELECTED. No release/tag/version change.

## Read-only salvage review

Task 497 S2 is retained: Task 495 is partially salvageable. Read its catalog,
runtime method, Desktop status/gating and frontend presentation changes and
Task 495/496 historical reports without editing the old worktree. Salvaged:
bounded selector parser, catalog request, explicit membership classification,
alternatives, Inherit/provider-context no-probe behavior, connection-factory
ordering, generation-scoped status and deterministic fake-peer ideas.

Discarded: diagnostic-only RequestFailed, lossy agent_start_error, error string
conversion, old DiagnosticStage/Kind, completion conversion and direct consumed
adapter-error presentation. No old error representation is copied into product.

## Task 498 integration and implementation

ADR 0032 remains authoritative. Catalog errors convert through the existing
adapter `into_runtime_failure`, retaining RuntimeDiagnostic and the original
Arc-backed typed Error. Connection actor SharedFailure conversion retains the
original cause. Error::source/downcast remains available locally. Desktop stores
the envelope alongside a separately serializable presentation. Source Debug,
Display, arbitrary provider text, response bodies and stderr are never serialized.
Add neutral ModelDiscovery operation; no second error carrier or AgentRuntime
trait change. No dependency edge or ADR change.

Neutral ModelCatalog/ModelPreflight are descriptive runtime data, without a new
factory or discovery trait. Codex alone owns literal model/list RPC, five-second
deadline, 100-model/256-byte selector bounds and rejection of incomplete pages.
Successful membership proves advertisement in this current context only, never
entitlement, inference success, permanence or future compatibility.

Existing executable selection, exact version admission and schema validation
finish before initialized runtime preflight. 0.160.0 remains fail-closed.
Explicit OpenAI selection proceeds only when advertised; absence stops the
factory before ready publication and shuts down its runtime. Alternatives remain
visible. Inherit does not invent a concrete model. Non-OpenAI providers remain
NotChecked because the default Codex catalog cannot establish absence in those
contexts; no local/custom provider is blocked by unrelated default metadata.

Desktop keeps connection/model generations on the observation; stale or
disconnected results are hidden. Frontend adds a bounded alternatives datalist
and compatibility text to existing model validation controls. Selection is never
ranked, replaced or switched. No refresh/updater/credential action is added.
Chat presentation uses Task 498 local events until explicit sanitized projection,
so runtime/provider request failure is distinguishable from catalog failure.

## Tests and validation protocol

Fake app-server tests cover model/list request and selector parsing, present and
absent selection, alternatives, malformed/incomplete/oversized catalog, RPC
failure, correlation, no thread/turn probe, typed source and sanitized diagnostics.
Desktop tests cover gating, Inherit, retained typed failure, presentation and
generation currentness. Existing effective-model/provider mismatch, malformed
response and unexpected-exit tests remain unchanged and mandatory.

Source freezes before each serial sequence. Isolated target:
`F:/temp/rah-task499-target-isolated`; external logs preserve exact commands and
exit codes. A: complete Codex package tests including doctests. B: affected
protocol/runtime/session tests. C: Desktop connection/model/preflight coverage.
Stop on first real failure, without editing during any running validation.
Only after focused passes: fmt/check/workspace test/Clippy/diff; canonical
Desktop gate; frontend/static/Tauri/metadata and static/executable HostExplicit.

## Completed deterministic validation

Every command below fully exited 0. No product source edits occurred during
validation. The initial freeze contains 212 crate source/configuration hashes;
all 212 remained unchanged after the deterministic sequence.

| Gate | Actual result |
| --- | --- |
| A: cargo test -p rah-runtime-codex | 96 unit + 6 architecture + 11 live-contract passed; 113/0/1 total; doctests 0/0/0; complete exit 0 |
| B: cargo test -p rah-protocol -p rah-runtime -p rah-session | 30/0/0; all doctests exit 0 |
| Desktop fixture preparation, canonical script -PrepareOnly | PASS, exit 0 |
| C: cargo test -p rah-desktop --bin rah-desktop model_preflight | 4/0/0 |
| C: same Desktop target, connection filter | 8/0/0 |
| C: same Desktop target, model filter | 20/0/0 |
| cargo fmt --check | PASS |
| cargo check --workspace | PASS |
| cargo test --workspace | 1,029/0/24, including Desktop 329/0/20 and complete doctests |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS |
| git diff --check | PASS |
| Canonical Windows Desktop gate | PASS, 329/0/20, helper build and test exits 0, no watchdog timeout |
| node --check frontend/status.js | PASS |
| Four existing frontend/static suites | PASS |
| New model_preflight_test.js renderer behavior/redaction suite | PASS |
| Tauri permission inventory | PASS: 47 runtime/manifest/generated/default allows/frontend commands |
| cargo metadata --no-deps --format-version 1 | PASS: 13 packages/members, all version 0.33.0 |
| Static HostExplicit inventory | Exactly 11 expected variants |
| Executable HostExplicit inventories | Both host_allowlist_is_exact and snapshot_composition_keeps_the_eleven_host_explicit_kinds PASS in workspace and canonical Desktop runs |

Logs are `F:/temp/rah-task499-phase-a.log`, `-phase-b.log`,
`-phase-c-preflight.log`, `-phase-c-connection.log`, `-phase-c-model.log`,
`-full-fmt.log`, `-full-check.log`, `-full-test.log`, `-full-clippy.log` and
`-full-diff-check.log`. Metadata is `F:/temp/rah-task499-metadata.json`.
Canonical Desktop evidence directory:
`F:/temp/rah-task499-desktop-gate/20261003-064705-068-3e6df43916fd42cbaa6b9350b1fe732c`.
Its status.json records PASS and elapsed 184.441 seconds. Frontend suite and
inventory outputs are preserved in the task tool results. Existing mismatch,
malformed-response and unexpected-exit tests passed unchanged relative to
Task 498; typed sources recover mismatch fields and exit status/stderr.

## Production validation and catalog observations

Production build `cargo build -p rah-desktop` passed, exit 0. Actual production
Desktop executable (not a test executable):
`F:/temp/rah-task499-target-isolated/debug/rah-desktop.exe`, PID 8096,
SHA256 `56442A207B478FED05A88BCB127B1E8AE5663ACB3A6D6E0CE65D37BF700C4744`.
Only stored `codex-cli 0.157.1` was used, with version precheck and normal
adapter version/schema admission. Actual Desktop-owned app-server PID 2596,
parent 8096, ran the stored 0.157.1 executable, SHA256
`8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.
Ownership and hashes are separate evidence; no new file-object identity
certification or broader release certification is claimed.

Live evidence is `F:/temp/rah-task499-live`: launch.json, ownership.json,
initial.json, discover.json/png, advertised.json/png, chat.json/png,
absent.json/png, restored.json and evidence.jsonl. WebView CDP exercised the
production Tauri commands and existing renderer; screenshots were directly
inspected. No product test hook or synthetic app-server supplied these results.

First, a harmless invalid selection obtained the catalog without inference
(connection generation 1). It produced MODEL_NOT_ADVERTISED, zero publications
and zero thread starts, with nine visible alternatives. This observation supplied
the validation choice for Case A; product selection never changes automatically.

Case A (generation 2): explicitly selected advertised `gpt-6-astra`, catalog
MODEL_ADVERTISED, normal connection publication and visible membership limitation.
Exactly one neutral turn completed with `RAH_TASK499_LIVE_OK`; one thread_start,
one desktop_completed with marker_observed=true. No Tool execution was requested
or observed. This is bounded current-context inference evidence for that turn.

Case B (generation 3): `rah-task499-harmless-invalid-model`, successful catalog,
MODEL_NOT_ADVERTISED, error/non-ready connection state, selected identifier and
all nine alternatives visibly displayed. Zero connection publications, zero
thread starts, zero completed turns for this generation. No inference turn sent.

Observed catalog: gpt-6-astra, gpt-6-sol, gpt-6-luna, gpt-reserve, gpt-5.6-sol,
gpt-5.6-terra, gpt-5.6-luna, gpt-5.5, codex-auto-review. `gpt-6.1-sol` was not
advertised in these observations. No retirement, entitlement, runtime-age or
future-availability conclusion follows. No 0.160.0 executable was used.

Cleanup evidence: Windows native app-data resolution ignored temporary child
APPDATA/LOCALAPPDATA overrides; Desktop used its existing application store.
The captured initial Inherit/model-null selection was restored and verified.
The single authorized neutral turn remains in ordinary transcript persistence;
existing history was not cleared. No provider credentials were changed.

An external cleanup harness assertion expected codexStatus="not connected" after
disconnecting the absent-model error state. Actual runtimeStatus was "not connected"
but codexStatus retained "error". Inspection of unchanged starting-HEAD disconnect
code proves it restores the previous state when no runtime exists. Model reset
succeeded; this was an erroneous cleanup assertion, not failed Case A/B gating.
No live case was rerun to erase this evidence and no product patch followed.
WebView window.close was denied by existing core:window:allow-close policy;
no permission was added. Native Process.CloseMainWindow on the owned Desktop
returned true and closed it normally. No app-server remained after rejection.

## Authority and final disposition

HostExplicit static and executable count: 11. Repository
authority, ToolRegistry authorization, Trusted Profiles, remembered authority,
leases, permissions and mutation uncertainty are unchanged. Catalog data grants
no authority. No downloading, baseline promotion, dynamic allowlist, certification
lifecycle or runtime abstraction migration.

## Final classification and publication boundary

**A — MODEL PREFLIGHT SALVAGED AND VALIDATED ON NEUTRAL ERROR BOUNDARY**

Useful S2 catalog/preflight behavior is restored, without the lossy error model.
Advertised explicit default-provider selection proceeds; absent selection stops
before ready publication with alternatives retained. Inherit and unsupported
provider-context nonclaims are preserved. Typed adapter causes remain recoverable;
sanitized diagnostics reach frontend. All deterministic gates and both bounded
production cases pass. HostExplicit remains 11, with no authority expansion.

This completed classification authorizes the intended Task 499 commit and normal
GitHub master/internal-mirror publication, without force push, tag, release or
version change. Final commit SHA, remote SHA readbacks and natural exact-head CI
run/result are recorded in the final task response and external publication
receipt, avoiding a self-referential commit hash in this report.
