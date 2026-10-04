# Task 509C — Backend model-source snapshot and freshness contract

Reference-only follow-up: [Task 509D](2026-10-04-task-509d-codex-effective-provider-model-discovery-identity.md)
audits the actual certified 0.157.1 schemas and isolated startup/config reads.
Its classification C establishes no authoritative catalog/provider association;
Task 509C snapshot/picker implementation remains blocked. This note changes no
Task 509C implementation, classification or historical validation claim.

Starting HEAD: `570e93319382343b1fa3d7996c413736d9e13770`.
Version remains 0.33.0; current certified Codex remains 0.157.1.

## Plan and scope

1. Preserve the Task 509B report and Task 509/509A reference notes.
2. Inspect Desktop composition, executable admission, discovery/preflight,
   runtime identity, presentation, and accepted ADRs 0015/0030/0032/0033.
3. Establish whether existing discovery evidence identifies its effective
   runtime/provider configuration before assigning authoritative ownership.
4. If identifiable, implement Desktop-private snapshots, configuration
   generations and request ordering, with deterministic source/order tests.
5. Only after focused PASS, run the requested full deterministic gates,
   Desktop/frontend/static/Tauri/metadata/HostExplicit checks. Only A permits
   commit, normal master push and natural exact-head CI closure.

No picker implementation, runtime refresh, live OpenAI, preference migration,
version bump, tag or release. No product source changes in this disposition.

## Final classification

**C — CODEX DISCOVERY API CANNOT IDENTIFY ITS EFFECTIVE CONFIGURATION**

This is a stopped prerequisite audit, not a delivered snapshot contract.
The current adapter API cannot bind a preconnection catalog to its actual
effective upstream configuration. A Desktop generation alone identifies the
host's requested configuration, not configuration actually used by discovery.
No claim is made that the certified CLI can never support an adapter extension;
that extension has not been audited or implemented here.

## Exact blocking evidence

- `rah-runtime-codex/src/process.rs::ProcessTransport::start` resolves the
  executable privately, verifies version/schema, and starts
  `app-server --stdio`. It does not receive the factory's upstream provider or
  workspace. The resolved admission identity is not returned to Desktop.
- `experimental.rs::CodexFactory` retains executable/provider/workspace, but
  passes only the executable to process startup. `RuntimeInstance` exposes
  capabilities, liveness, discovery, conversation creation and shutdown; it
  exposes no admitted artifact or effective configuration observation.
- `catalog.rs::discover` sends exactly
  `model/list { limit: 100, includeHidden: true }`. No upstream provider,
  endpoint, workspace or configuration identity accompanies that request.
  Parsing retains bounded model selectors and completeness, not source identity.
- `experimental.rs::Instance::discover_models` checks the locally stored
  requested provider. Non-OpenAI providers return `Unsupported`. For OpenAI,
  it returns the default process catalog without observing which effective
  provider configuration produced it. Local requested-provider equality is
  not effective-provider evidence.
- `runtime.rs::restricted_thread_params` applies explicit provider/model and
  endpoint configuration at thread creation. `verify_effective_model_config`
  checks the returned model/provider then. This occurs after preconnection
  discovery; opening a thread to manufacture preconnection identity would
  change the discovery boundary.
- `codex_composition::configured_codex_factory` translates `Inherit` to a
  factory with local `OpenAi` plus `ModelSelection::RuntimeDefault`. This
  translation does not prove the inherited effective upstream is OpenAI.
  `create_and_preflight` deliberately skips discovery for RuntimeDefault.
- The pinned schema admission contract does not check model-list source
  identity or an effective-configuration read. No adapter `config/read` path
  exists. Absence from this contract is not proof of CLI schema impossibility.

Consequently neither a certified-version string nor a host executable selector
can substitute for the exact admitted artifact/configuration identity required
by this task. Labeling default metadata with a desired provider/generation
would fabricate ownership; this audit does not do that.

## Proposed snapshot contract — not implemented

A future Desktop-private owner should return one coherent snapshot containing
Task 509A `RuntimeAdapterIdentity`, an opaque admitted-runtime/configuration
identity, bounded effective upstream configuration, configuration generation,
request identity, resolution state and discriminated model evidence.

Source kinds: Codex discovered catalog; Codex discovery unsupported; native
OpenAI configured model; native OpenAI unconfigured; no runtime; resolution
error. Pending resolution should be explicit and withdraw earlier evidence.
Supported empty catalog remains a discovered catalog with zero entries.
Unsupported never validates a saved selector; no default-catalog fallback.
Errors serialize only bounded `RuntimeDiagnostic` under ADR 0032; internal
typed causes remain process-local. Snapshots carry no executable paths,
credentials, Tool handles or authority.

The runtime fingerprint must come from actual adapter admission and source
configuration, not preferred version, persisted provider preference or a
frontend reconstruction. `provider = openai` retains its historical Codex
upstream meaning. Native OpenAI uses only the startup configured model; missing
model is explicitly unconfigured. None has neither active model nor catalog.
Inactive Codex preferences remain persisted but do not enter either active source.

Proposed freshness: increment a backend configuration generation on active
adapter, admitted runtime identity, effective upstream configuration (including
endpoint) or native configured-model changes. Unrelated UI/readiness state
does not increment it. Every request captures that generation. Under the
owner's publication lock, accept only current-generation/current-request results,
including errors and Unsupported. Same-generation policy: newest request ID
wins. These are proposed semantics, not existing validated behavior.

Required order tests remain unimplemented: late A success after B success;
late A error after B success; late A catalog after B Unsupported; stale
Unsupported/error after B success; and same-generation request replacement.
Snapshot identity, source-kind, generation-change and serialization tests also
remain pending. The Task 509B reproduced stale overwrite is not corrected.

## Bounded prerequisite and Connect boundary

Before Desktop implementation, audit an adapter-local, preconversation source
resolution seam for the unchanged certified CLI. It must either obtain a
bounded effective configuration observation or apply and verify a supported
configuration before discovery, and return an opaque identity for the actual
admitted artifact and that configuration. Inherit must remain explicit and
unvalidated unless its effective configuration can be established. Unsupported
providers must stay Unsupported. Do not add raw config/error serialization or
provider types to neutral core APIs. No CLI refresh is prescribed.

Desktop can then bind this identity to its generation/request owner, expose
one DTO read, and test publication ordering with deferred local fixtures.

Existing Connect re-resolves a factory, performs fresh preflight, and checks
repository/model/profile/connection/identity generations before runtime
publication, including a final lifecycle-coordinated publication gate.
It does not consume a preconnection catalog today. Its model-generation check
does not observe an external admitted-artifact or ambient upstream change.
Therefore this audit cannot certify Connect against the proposed new source
generation. Once source identity exists, capture/revalidate that same identity
and generation at the final publication gate. No Connect redesign or change
was made here; classification C is the prerequisite, not a second classification.

## Validation actually executed

`CARGO_TARGET_DIR=F:/temp/rah-task509a2-target-run2`:

`cargo test -p rah-runtime-codex catalog::tests -- --nocapture`

Result: **5 passed, 0 failed, 0 ignored**. Integration test groups matched zero
tests. Local fake transports only; no Codex process or inference. The tests
check the exact provider-free model-list request, bounded selector parsing,
incomplete/malformed rejection, sanitized RPC failure, and no catalog/inference
probe for Inherit/Ollama. These verify existing behavior, not the new snapshot.

Full workspace fmt/check/test/clippy, canonical Windows Desktop, complete
frontend/static, Tauri inventory, metadata and executable HostExplicit gates
were not run: the source-identity prerequisite stopped implementation before
the requested focused snapshot PASS. No workspace/Desktop counts are claimed.
No deterministic failure or Windows corruption occurred in the executed tests.
Closure includes `git status --short`, `git diff --stat`, and `git diff --check`.

## Authority, ADR and publication

HostExplicit is statically exactly 11 in unchanged `HostInvocationKind`.
No executable HostExplicit certification is claimed in this audit. No change
to ToolRegistry authorization, repositories/switching, leases, Trusted Profiles,
permissions, mutation uncertainty, remembered workspaces or persistence.
No dependency, public/core contract or product serialization change.

ADR-B: no architectural decision adopted. Proposed Desktop freshness ownership
fits existing host ownership; adapter source observation needs evidence before
implementation, not a new ADR automatically. No authority/security expansion.

No commit or push: only A authorizes completion/publication. HEAD remains the
starting SHA. No new exact-head CI. Worktree intentionally retains the existing
docs plus this report and a reference-only Task 509B note; it is not clean.

Exact Task 509B-resume scope: **not authorized by this C result**. First resolve
and validate the adapter source-identity prerequisite, then complete Task 509C
snapshot ownership and all requested deterministic gates to A. Only then resume
509B catalog picker/presentation, invalidation and Connect UX/policy, native
configured-model P1 policy, and its acceptance tests. Task 510 and Task 508L
remain deferred; no live OpenAI, runtime refresh or picker work began.
