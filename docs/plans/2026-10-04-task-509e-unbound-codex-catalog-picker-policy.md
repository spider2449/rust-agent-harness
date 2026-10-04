# Task 509E — Provider-unbound Codex catalog picker policy

Starting HEAD: `5cddd77dac3893f461b8c8e607c5ec8aa553dcdb`; clean master.
Documentation-only product/contract decision. No picker implementation.

## Plan and disposition

1. Inspect authoritative task reports, accepted ADRs and production connection,
   discovery, model configuration, conversation and turn boundaries.
2. Run focused existing deterministic lifecycle/configuration checks.
3. Decide suggestion, custom, Inherit, freshness and Connect contracts; add only
   reference notes to Tasks 509B/510A; publish documentation with exact-head CI.

**A — RUNTIME-ADVERTISED PICKER POLICY SELECTED**

P1 is selected. Connect may publish a ready runtime with provider compatibility
unverified. This is an explicit product acceptance of possible first-turn failure,
not a compatibility certification. No further policy decision is needed to resume
509B within the scope below. Implementation is not automatically resumed here.

## Established evidence and terminology

Task 510A is authoritative for the artifact/protocol observations. Both exact
0.157.1 and 0.160.0 catalogs have no provider/config ownership input or response
identity. Distinct explicit OpenAI/Ollama config readback can yield identical
catalogs; thread metadata does not bind discovery; Inherit remains unobservable.
This is not a Desktop discovery bug.

- Runtime-advertised model: returned by this admitted Codex runtime's catalog.
- Provider-validated model: compatibility with the effective upstream provider
  independently established for the exact relevant context.

Catalog membership establishes only the first. Neither structural configuration
checks, successful initialization, matching thread metadata nor Ready establish
the second. Validation evidence is contextual and time-bounded, never entitlement
or permanent support. No current picker path produces ProviderValidated.

Certified/preferred stays 0.157.1, SHA-256
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
Candidate 0.160.0, SHA-256
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`,
is unadmitted/uncertified. Host snapshot provenance must describe the actual
resolved admitted runtime, not infer it from PATH or requested version. Current
admission does not return an executable fingerprint: 509B must not claim measured
artifact identity without host measurement, or a running-image/TOCTOU guarantee.

## P1/P2/P3/P4 evaluation

| Policy | Decision | Reason |
| --- | --- | --- |
| P1 runtime suggestions | Select | Prevents ordinary typing errors; honest runtime provenance; permits usable configurations despite unverified compatibility. |
| P2 provider-bound only | Reject | No current ownership proof; removes dropdown usability and can exclude usable local configurations without establishing incompatibility. |
| P3 Connect compatibility validation | Not available as central contract | Existing lifecycle does not validate remote model compatibility before inference. Retain actual local checks without upgrading their meaning. |
| P4 RAH provider allowlists | Reject | No evidence justifies completeness/authority; duplicates changing provider knowledge and creates stale false claims. |

## Actual Connect and inference boundaries

Source audited at starting HEAD:
`crates/rah-desktop/src/production_composition.rs::connect_with_configuration`,
`runtime_composition.rs::{create_and_preflight,bind_conversation}`,
`crates/rah-runtime-codex/src/experimental.rs::{CodexFactory,Instance,Conversation}`,
`model_config.rs`, `runtime.rs::{restricted_thread_params,verify_effective_model_config}`,
and Desktop `model_preflight.rs`.

| Stage | Actual guarantee / rejection timing |
| --- | --- |
| Provider configuration | Local selection checks validate model text, URL shape and credential variable names. Factory validates executable path; process startup validates admitted version/schema and initialize. Explicit provider and endpoint are applied through thread parameters later, not to discovery. This does not probe endpoint, credentials or remote support at Connect. Codex startup errors may reject Connect; later provider configuration errors can occur at thread/start or inference. |
| Model preflight | Explicit OpenAI currently calls model/list, gates absent IDs and catalog errors; other providers return Unsupported/NotChecked. RuntimeDefault skips discovery. These are advertisement/availability checks only. |
| Conversation binding / Ready | Instance.open builds a local conversation and structurally checks explicit model configuration; it sends no thread/start. Host publication checks current lifecycle generations. Ready means runtime connected, not model/provider verified. |
| Thread creation | Conversation.send obtains host turn admission, sends thread/start, checks returned explicit model/provider and workspace before turn/start. Matching fields prove effective selection, not remote compatibility. Inherit bypasses explicit model/provider equality; it cannot be labeled validated. |
| First inference | turn/start acceptance is not execution success. Observe terminal turn/completed/error. Model, account, endpoint or provider failure can first surface here; retain sanitized typed diagnostics and existing failure lifecycle. No retry or silent model/provider fallback. |

Task 510A's historical exact-0.157.1 gpt-6.1-sol control passed thread/start and
turn/start, then failed with upstream 400/account model unsupported. Its exact
0.160.0 explicit-OpenAI control completed. These are prior documented live
observations, not new 509E runs. The counterexample rules out B and proves that
thread creation cannot be promoted to a universal no-inference compatibility gate.
Failures may be known earlier, but successful compatibility is not guaranteed
before a real turn. C's further-decision requirement is resolved by accepting P1
here, so final classification is A. No new thread probe or inference is needed.

## Catalog completeness and Codex custom policy

Pagination completion means the returned runtime list was fully read, not that
all models usable by every upstream provider are enumerated. Adapter model text
validation has no membership requirement. Thread parameters accept an explicit
ID independently of model/list. The historical absent gpt-6.1-sol request reached
thread/start and turn/start; it failed inference. This proves acceptance outside
the list, not successful absent-model inference. No general completeness or
successful custom-provider claim is made.

Select **C2 — explicit `Custom model ID…`**, not unrestricted normal text. C1
would remove supported explicit-ID routing based on an unproven exhaustive list,
especially for local/custom providers whose identifiers need not be advertised.
Custom text uses existing bounded structural validation (trim, nonempty, 256-byte
maximum, no NUL); display `Not runtime-advertised; provider compatibility
unverified`. Choosing custom is deliberate and context-scoped; no additional
permission dialog or model-authority grant. A known compatibility rejection
cannot be bypassed by relabeling the same ID as custom.

Missing saved IDs are `Invalid for current runtime catalog` in normal mode and
block Connect. Never silently convert them to custom, substitute another model,
or preserve ordinary validity. User may deliberately choose Custom and apply the
ID, if no independently known incompatibility blocks that exact context.

## State model and Connect eligibility

Use separate bounded axes, not one valid boolean:

- Source: Loading, Ready, Empty, Unavailable (sanitized error/unsupported).
- Selection: None, RuntimeAdvertised, CustomUnverified, InheritedUnverified,
  ConfiguredSnapshot (native OpenAI), Invalid with reason.
- Compatibility: Unverified, ProviderValidated with scoped evidence, Rejected
  with scoped evidence. 509B never synthesizes ProviderValidated.

Unknown is represented by pending source/selection and unverified compatibility.
Catalog absence is a selection-mode failure, not proof of remote incompatibility.

| State | Connect eligibility |
| --- | --- |
| None / malformed explicit ID or provider configuration | Blocked |
| Loading / context replacement pending | Blocked until current request settles; prior validity withdrawn |
| RuntimeAdvertised + Unverified | Allowed after fresh Connect advertisement check; Ready retains unverified label |
| ProviderValidated | Allowed only for exact current context and otherwise eligible selection/source; not produced by 509B |
| Invalid normal saved selection / absent normal ID | Blocked; choose current suggestion or explicitly apply Custom |
| Compatibility Rejected | Blocked for same context, including custom; never infer global rejection from generic failure |
| Empty / Unavailable + ordinary catalog mode | Blocked; no stale list fallback |
| Empty / Unavailable + explicitly applied CustomUnverified | Allowed after source resolution and local checks; catalog availability/membership not a custom-mode gate |
| CustomUnverified | Allowed after local checks and fresh current-context preflight; advertisement may be observed but absence is not a custom-mode rejection |
| InheritedUnverified / effective provider unobservable | Allowed after current context resolves and normal startup checks; no provider-specific validation claim |
| Native ConfiguredSnapshot | Allowed with existing local configuration checks; missing/invalid snapshot blocked |

All allowed rows remain subject to existing runtime admission, host lifecycle,
repository and permission checks. Discovery error never bypasses admission.
Fresh Connect preflight must use selection mode: normal catalog selections retain
missing/error gates; Custom and Inherit use local checks and descriptive discovery
only. Do not disable the gate globally. Connect performs zero inference.

## Inherit, switching and provenance

Inherit uses I1/I3: runtime suggestions may be shown descriptively, with `Provider
compatibility unverified`; provider-specific validation is unavailable. Preserve
existing runtime-default semantics using a `Runtime default (Inherit)` selection,
which sends neither explicit model nor provider. Saved explicit IDs are historical
preferences, not the effective inherited model. In this bounded 509B scope,
applying a suggestion/custom ID requires choosing an explicit upstream provider;
do not silently map Inherit to OpenAI or add a model-only inherited override.

Host source context includes adapter, resolved runtime/admission identity,
upstream selection/endpoint configuration, relevant workspace and replacement
generation. This is the validation/request context, not catalog ownership.

On runtime or upstream-provider/endpoint change, withdraw old eligibility, clear
compatibility evidence, re-evaluate saved selection, refresh/rebind source and
provenance, and discard late success AND error using host context/generation
checks. Identical returned catalogs do not preserve prior compatibility state.
Disconnect/recomposition/teardown owns cancellation and cleanup; connected
controls follow existing disable/staging semantics, without hot recomposition.

0.157.1 to any subsequently admitted 0.160.0 must resolve a new runtime catalog.
OpenAI to Ollama must bind a new active context even if model arrays match. Labels
are `Advertised by Codex runtime`, `Provider compatibility unverified`, and
native `Configured for native OpenAI`. Never `Ollama models` or
`provider-supported` on the strength of Codex discovery. Minimal terminology only.

## Native OpenAI and gpt-6.1-sol

Task 509B native policy is confirmed: configured backend model snapshot only,
no presets, P1 no custom IDs, local structural checks, no live discovery, no
Codex-preference writes. ConfiguredSnapshot is not ProviderValidated.

Under the observed 0.157.1 catalog, gpt-6.1-sol is absent: no ordinary suggestion,
saved normal ID invalid and Connect blocked. Explicit Custom is unverified; the
historically failing account/provider context cannot be called compatible.
Under observed 0.160.0 it is runtime-advertised; the prior direct successful
control is specific artifact/account/provider evidence, not catalog ownership.
Current admission still rejects 0.160.0. Future tests use catalog-relative fixtures
and distinguish absence, custom selection, advertisement and compatibility.

## Exact Task 509B resume scope

1. Implement host-owned preselection source snapshot with resolved runtime
   provenance, provider/endpoint context, generation, source and selection axes.
   Discovery instances own teardown; no conversation, external Tool activation,
   provider probe or inference during discovery.
2. At the private Codex adapter/host edge expose runtime-global suggestions also
   for non-OpenAI and Inherit contexts. Current OpenAI-only discovery guard must
   be deliberately revised as advertisement access, never provider validation.
   Do not invent provider binding or a new provider-specific neutral API.
3. Replace normal free text with suggestions plus explicit Custom entry; preserve
   native snapshot-only and Inherit runtime-default policies above. Add minimal
   provenance/unverified status before and after Connect.
4. Implement mode-aware fresh Connect gates and host publication checks. Do not
   add thread creation at Connect, compatibility inference, provider fallback,
   silent saved-ID conversion or persistent validated flags.
5. Deterministically cover stale success/error, source replacement/cleanup,
   runtime/provider switches with identical arrays, cleared evidence, normal
   missing/empty/error blocking, custom exception, known rejection blocking,
   Inherit/default routing, native snapshot and both gpt-6.1-sol fixtures. Verify
   zero inference at discovery/Connect and current-context publication.
6. Perform the separately requested 509B implementation checks/acceptance before
   claiming delivery. This document is policy authorization, not picker completion.

This supersedes only the earlier provider-bound prerequisite and catalog-only
Connect assumptions in 509B/509C; their historical stopped findings remain intact.
Task 510B is a separate ADR 0030 certification task, including required exact
artifact/schema, deterministic and direct/Desktop evidence. No certification,
baseline/admission change, tag, release, version bump or automatic 509B work here.

## ADR, authority and executed validation

ADR-B: product provenance/selection contract under accepted ADRs 0030/0033 and
existing host/adapter ownership. No new or amended ADR, dependency, public/core
boundary, permission, ToolRegistry, repository authority/switching, Trusted Profile
or HostExplicit change. HostExplicit remains exactly 11.

Focused deterministic commands used `F:/temp/rah-task509a2-target-run2` for
CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR, serially against unchanged source:

| Command/filter | Result |
| --- | --- |
| cargo test -p rah-runtime-codex --lib model_config::tests | 0 selected; not evidence (module has no tests) |
| Same package: experimental::tests::default_catalog_identity_native_continuation_and_replay | 1 passed |
| Same package: model_provider_configuration | 1 passed |
| Same package: explicit_effective | 2 passed |
| cargo test -p rah-desktop --all-features runtime_composition::tests::task504_factory_preflight_gates_open_and_ready | 1 passed |
| Same Desktop command: model_preflight::tests | 4 passed |
| Same Desktop command: host_invocation::tests::host_allowlist_is_exact | 1 passed |

Total 10 passed, 0 failed, 0 ignored; no new tests/product code. Source inspection
establishes lazy thread timing; fixtures establish local gates and matching-field
checks, not real provider compatibility. Task 510A supplies prior runtime evidence.
No live Codex/OpenAI requests, OPENAI_API_KEY use, UI acceptance, broad workspace
gate or runtime certification. Task 508L remains paused. Remaining practical risk
is first-turn incompatibility; accepted explicitly with truthful presentation.

Closure: documentation diff/status/whitespace review, normal GitHub master push,
and natural exact-head CI are reported in the final return. No mirror push.
