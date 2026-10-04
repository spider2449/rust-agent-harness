# Task 509B — Provider-aware model picker, freshness, and Connect gating

Starting HEAD: `570e93319382343b1fa3d7996c413736d9e13770`; clean worktree.
Version 0.33.0; certified Codex baseline 0.157.1.

## Implementation and validation plan

1. Inspect host composition, catalog/preflight, picker, persistence, lifecycle,
   accepted ADRs 0030/0033, architecture and security boundaries.
2. Implement backend-owned preselection discovery using the admitted runtime's
   existing `model/list` source. Scope requests to host configuration and explicit
   replacement generations; discard stale success and failure. Own discovery
   instance teardown without opening a conversation or granting Tool authority.
3. Replace default free text with catalog choices. Fail closed while loading,
   empty, failed, unavailable, or awaiting a catalog-valid applied selection.
   Preserve fresh Connect-time preflight and connected lifecycle.
4. Native OpenAI: configured backend snapshot only. No presets; custom policy P1
   (no custom IDs). No live discovery, API request, or new persistence.
5. Deterministic tests: catalog-relative absent `gpt-6.1-sol`, runtime/provider
   switches, out-of-order success/error, loading/empty/error and Connect gates.
6. Bounded Desktop acceptance, focused tests, then full requested deterministic
   gates, canonical Windows Desktop, frontend/static, Tauri inventory, metadata,
   and HostExplicit exactly 11. Preserve failure evidence and stop on gate failure.
7. Record final classification, evidence, authority/ADR impact and Task 510 scope.
   Only A permits commit, normal master push, and natural exact-head CI closure.

Persisted Codex preferences remain historical desired state. Native OpenAI never
writes them. ADR-B expected; no new public runtime API or authority boundary.
No runtime refresh, live OpenAI work, UI redesign, version bump, tag or release.

Task 510, only after A and publication closure: exact newer Codex CLI schema,
catalog and artifact audit, deterministic regressions and required direct/Desktop
certification under ADR 0030; update exact admission/baseline only from evidence.
Keep Task 508L paused and HostExplicit 11; do not begin Task 510 here.

## Final disposition

**B — CODEX CATALOG FRESHNESS REQUIRES A BACKEND CONTRACT CHANGE**

This is a stopped contract audit, not a completed picker. The implementation
steps above were proposed, not executed. Only documentation changes. Task 509A's
classification and checkpoint remain intact. Task 509 is not complete and Task
510 is not authorized by this result. No deterministic gate failed.

## Picker state and source contract findings

`runtime_model_state.rs` preserves the established adapter/upstream-provider
distinction. Native OpenAI presents the backend's startup `RAH_OPENAI_MODEL`
snapshot, Codex presents an inactive persisted preference, and None has no model.
All three presentations correctly avoid claiming model validation. Persistence
continues to mean Codex upstream preference; no native OpenAI persistence exists.

`desktop_model_configuration_commands::model_configuration` returns preferences
and composition identity. It provides neither a preconnection catalog nor the
identity of the resolved/admitted executable and effective catalog configuration.
`ScopedModelPreflight` carries adapter, connection generation and model generation;
`DesktopAppState::status` only exposes it in Connected/Error states. That protects
Connect-time observations, not a disconnected picker discovery lifecycle.

The existing Model control is still a text input/datalist. `renderModelPreflight`
fills the datalist from connection evidence. Connect enables from runtime and
lifecycle status, not from resolved catalog membership. These are known remaining
defects; no picker or gating behavior was changed in this disposition.

### Codex catalog source and provider configuration gap

`rah-runtime-codex/src/catalog.rs::discover` already implements bounded
`model/list`: five-second request deadline, at most 100 models, bounded selectors,
complete page requirement, and safe typed failures. It needs no hard-coded list.

However, `experimental.rs::Instance::discover_models` only calls it for
`CodexModelProvider::OpenAi`. Ollama, LM Studio, llama.cpp and other provider
configurations return `ModelDiscovery::Unsupported` instead of a catalog.
`ProcessTransport::start` starts `app-server --stdio`; the selected upstream
provider configuration is applied later through thread parameters, not passed
to the catalog request. `runtime.rs::preflight_selected_model` explicitly says:
"Default catalog metadata cannot validate a custom provider."

`codex_composition::configured_codex_factory` maps Inherit to an OpenAI factory
plus RuntimeDefault. `runtime_composition::create_and_preflight` skips discovery
for RuntimeDefault and maps Unsupported discovery to NotChecked. Therefore
NotChecked cannot be promoted into current selected-provider catalog validity.
Using the OpenAI catalog for a newly selected local provider would invent
compatibility. Broadening adapter discovery without source evidence would erase
the existing deliberate unsupported distinction.

### Required bounded backend correction

Provide a Desktop-private preselection model-source snapshot, with ownership of:

- Host-selected admitted executable identity and effective catalog configuration;
  adapter, upstream-provider/endpoint context, workspace context where relevant,
  and replacement request generation.
- Resolved catalog, loading, empty, sanitized error, and explicit unsupported
  source states. Unsupported must never imply arbitrary catalog membership.
- A distinct selection-validity result, with catalog membership kept separate
  from historical preferences and from remote entitlement/inference claims.
- Invalidation on replacement, source change, Disconnect/recomposition and
  teardown; cancellation/cleanup ownership of each discovery instance. Discovery
  must not open a conversation or activate external Tool providers.
- Publication that rechecks request/context identity and discards both stale
  success and stale error. Frontend generation checks supplement this ownership.
- An explicit Inherit/runtime-default source policy and a source-supported policy
  for Codex upstream providers with Unsupported discovery. Do not silently use
  OpenAI catalog evidence to validate those configurations.

No new neutral public API is prescribed. Keep this at the host/adapter edges and
preserve current admission policy. This correction should precede completing the
Task 509B picker; it is not a recommendation to refresh Codex or perform live
OpenAI discovery.

## Freshness reproduction and teardown limits

Executed the actual extracted `renderModelConfiguration` and
`refreshModelConfiguration` functions in a Node VM with deferred local responses:
request A (Codex) starts; request B (native OpenAI) starts; B completes first;
A completes later. The final rendered adapter becomes Codex and the text input
again contains stale `gpt-6.1-sol`. This deterministically reproduces the existing
unconditional late-response overwrite, with zero network calls and inference.
It is defect evidence, not a passed stale-result-rejection test.

Evidence: `F:/temp/task509b-evidence/configuration-freshness-audit.cjs`, exit 0.
No preselection catalog command exists to test catalog-specific stale success,
stale error, stale loading or discovery replacement teardown. Those requested
acceptance cases remain unimplemented/unvalidated. Existing Connect observation
filtering is not claimed as their substitute.

## Required behavior remaining

Loading must immediately withdraw prior selection validity; empty catalog and
bounded source error must block Connect. Runtime A-to-B and upstream-provider
changes must invalidate A choices and restore a preference only if B's fresh
source validates it. Late A success/error/loading must never alter B's catalog,
selection, eligibility or presentation. None remains fail closed. Connected
controls must disable or explicitly stage changes, without hot recomposition;
Disconnect must restore editability and require current-source revalidation.
These picker/lifecycle acceptance cases were not implemented or manually run.

Native OpenAI policy for resumed implementation: configured snapshot labeled
`Configured model`, with local structural validation only. No presets in this
task; custom-model policy P1, no custom IDs. Current free text has not yet been
replaced, so this is a selected implementation policy, not delivered behavior.
Never use Codex discovery or write native state into Codex preferences.

The existing Task 509A catalog-relative fixture omits `gpt-6.1-sol` and rejects it
at preflight. Its state test was rerun successfully. The existing composition
fixture also proves an absent explicit model opens no conversation and sends
zero inference. The requested new picker-specific `gpt-6.1-sol` regression,
fresh catalog selection and zero-inference invalid-UI acceptance remain pending.
No permanent assertion about future catalog membership is introduced.

## Validation actually executed

All executed checks passed against unchanged implementation source:

| Check | Result |
| --- | --- |
| `cargo test -p rah-desktop --all-features runtime_model_state::tests` | 3/0/0 |
| `cargo test -p rah-desktop --all-features model_preflight::tests` | 4/0/0 |
| Current all-feature test executable: `runtime_composition::tests::task504_factory_preflight_gates_open_and_ready --exact` | 1/0/0 |
| Same executable: `host_invocation::tests::host_allowlist_is_exact --exact` | 1/0/0 |
| Existing frontend JavaScript syntax, all six `*test.js` scripts | PASS |
| `node crates/rah-desktop/tauri_permission_test.js` | PASS, 47 matching commands |
| Deferred-response configuration defect reproduction | Expected existing overwrite reproduced |

Focused Rust total: 9 passed, 0 failed, 0 ignored. Not new picker certification.
Target: `F:/temp/rah-task509a2-target-run2`; evidence logs:
`F:/temp/task509b-evidence/`. No corruption-style linker failure occurred.
Final whitespace and Git-state checks are recorded at closure.

Full workspace fmt/check/test/clippy, canonical Windows Desktop and metadata
were not rerun for this documentation-only stopped disposition. Task 509A's
supplied workspace 1065/0/24 and Desktop 338/0/20 are historical checkpoint
evidence, not Task 509B results. Bounded GUI/manual acceptance was not performed;
no actual Codex process/catalog request, native OpenAI request, real credential
use, inference, or runtime refresh occurred. Only deterministic local fixtures.

## Authority, ADR and publication

HostExplicit is statically exactly 11 and its executable exact-set test passed.
No ToolRegistry, repository authority/switching, lease, permission, Trusted Profile,
mutation-uncertainty, remembered-workspace or runtime-lifecycle source changed.
No dependency, manifest, lockfile, version, public/core API or ADR changes.
ADR-B: this is an implementation contract gap under existing ownership; no new
durable architectural decision is proposed or adopted.

No commit or push: only classification A authorizes them. HEAD stays at the
starting checkpoint; no new exact-head CI exists. Worktree intentionally retains
the new untracked report and two tracked reference-only notes. It is not clean.
No tag/release/version bump, Task 510 or full UI redesign.

Next work: bounded backend model-source snapshot/ownership contract above, then
resume Task 509B picker, freshness, provenance, Connect gates, deterministic and
manual acceptance. Only after A, normal publication and natural exact-head CI
PASS should Task 510 perform the exact-runtime refresh/certification scope stated
above. Do not start Task 510 from this B disposition.

## Task 509C reference-only follow-up

[Task 509C](2026-10-04-task-509c-backend-model-source-snapshot-and-freshness-contract.md)
records classification C: the current Codex discovery API does not identify
its effective preconnection configuration/admitted artifact. No snapshot or
picker implementation is delivered; Task 509B remains stopped at B. Resolve
the bounded adapter prerequisite and validate Task 509C to A before resuming.
