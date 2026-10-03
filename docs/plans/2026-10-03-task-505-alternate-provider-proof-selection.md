# Task 505 — Alternate-provider proof selection and Codex-optional cutover plan

## 1. Checkpoint and scope

Starting HEAD: `7f97f7189c9218d01117b12b982bea32eea9b42e` on clean `master`.
GitHub master matched by `git ls-remote`; natural push CI
[37106052884](https://github.com/spider2449/rust-agent-harness/actions/runs/37106052884)
was independently verified successful for that exact SHA.

RAH remains 0.33.0; HostExplicit remains exactly 11; preferred and certified
Codex runtime remains 0.157.1. No v0.34 capability is selected. This task changes
only this report: no adapter, dependency, feature, configuration, default,
version, tag, release, credential setup, API request or live validation.
Future task descriptions are recommendations, not automatic authorization.

## 2. Post-Task-504 architecture and evidence

The initial Task 504 report is historical stopped WIP. Its later Task 504F
report records completed validation and classification A; do not mistake the
earlier stop for the checkpoint's present state.

Production composition is Desktop -> configured neutral factory -> runtime
instance -> conversation -> owned turn stream/control -> Codex adapter.
`runtime_composition::create_and_preflight` and `bind_conversation` are the
production neutral seams. `DesktopRuntime` owns neutral handles and host Tool
scope. The Task 504F evidence covers Connect, preflight, ordinary turn,
read-only Tool, Disconnect, absent model and repository switching.

Authoritative material reviewed: README, ARCHITECTURE, ARCHITECTURE_GUARDRAILS,
SECURITY; ADRs 0001/0002/0005/0006/0015/0030/0032/0033; Task 498 failure envelope,
Task 500 removal criterion, Task 501 fake-adapter distinction, Task 504/504F;
current manifests, neutral contracts, Desktop composition/configuration,
baseline resolver, frontend, Tauri build/bundle configuration and CI scripts.

ADR 0033 governs neutral ownership, host Tool admission/revocation and effect
accounting. It deliberately leaves provider registration and alternate adapters
outside its specific decision. That exclusion does not prohibit a private
composition choice under the already accepted adapter boundaries.

## 3. Remaining Codex dependency inventory

Classes can overlap: a configuration reference can also prevent compilation.
Paths below are current source evidence, not claims of future validation.

| Location | Class | Current constraint and required treatment |
| --- | --- | --- |
| `crates/rah-desktop/Cargo.toml`, Windows dependency on `rah-runtime-codex` | build-time hard | Unconditional, nonoptional. Make optional and gate all consuming source. |
| `src/main.rs` imports and `runtime_composition.rs::configured_codex_factory` | build-time hard; runtime hard | Only configured production adapter; concrete model configuration/factory imports. Isolate behind Codex feature and private selection. |
| `main.rs::connect_codex`, `prepare_codex_connection`, `resolve_prepare_and_connect_codex` | runtime hard | Connect always converts to Codex config, resolves executable and prepares Codex before neutral creation. Branch adapter preparation before resolution while retaining common host publication gates. |
| `main.rs::ConnectionState`, publication/source metadata | build-time hard; presentation/config | Neutral runtime still carries mandatory `CodexExecutableSource`; use private adapter presentation metadata with optional Codex artifact information. |
| `main.rs::DesktopModelSelection::codex_model_config`, `set_model_configuration` validation | build-time hard; presentation/config | Existing OpenAI/Ollama/LM Studio/llama.cpp choices configure Codex, not alternate runtimes. Preserve these settings for Codex; alternate explicit model comes from separate proof config. |
| `main.rs::runtime_frontend_error`, `codex_frontend_error` | build-time hard; presentation/config | Codex typed downcast and fallback to Codex error code. Gate local Codex mapping; alternate projects closed neutral diagnostics without concrete provider errors in lifecycle modules. |
| `src/codex_baseline.rs`, preferred version status | build-time hard; runtime hard on Codex Connect | Imports adapter constant; override/baseline/PATH resolution and executable validation. Compile and execute only for selected Codex. Do not relax ADR 0030 admission. |
| `rah-runtime-codex` factory, transport, certification/model config | runtime hard within selected adapter | Version/schema admission and owned app-server are correctly Codex-specific. Keep intact; disabled Desktop must not depend on this crate. |
| Root Cargo workspace membership and lockfile | default build scope, not Desktop transitive dependency | `--workspace` selects Codex even if Desktop feature is off. Keep source/member/lock entries; use package-scoped disabled build and separately selected non-Codex workspace checks. |
| `main_tests.rs`, `model_preflight.rs` tests, `runtime_composition.rs` tests, baseline tests | test-only; disabled test compilation blocker | Direct Codex imports/fixtures and ignored live probes require cfg gating. Retain neutral tests in all configurations, with neutral error fixtures instead of gating whole suites. |
| `rah-runtime-codex` tests/examples; tools-plugin cross-boundary source audits | test-only | Adapter-specific suites inspect/use Codex; not executable prerequisites for alternate Desktop. Preserve default gates; source-presence audit is not linking. |
| `frontend/index.html`, `status.js`, frontend tests | presentation/config; default-only | Connect Codex labels, inherit-Codex setting, codexStatus/error/version fields and commands. Minimal truthful adapter label/status and unavailable Codex settings are needed in proof build; no provider selector. |
| `build.rs`, capabilities/default.json, generated permissions | presentation/config | `connect_codex`/`disconnect_codex` names are IPC contracts, not themselves executable dependencies. Keep compatibility names delegating to selected runtime initially; preserve full matching permission inventory. |
| `tauri.conf.json`, frontendDist, NSIS bundle | packaging/config | No Codex externalBin/resource declared. Existing Tauri version string is unrelated historical configuration; do not alter it. Later alternate packaging must not add Codex prerequisite/install step. |
| `scripts/codex-baseline.ps1`, `test-codex-baseline.ps1`, `codex-live-gate.ps1` | adapter-specific runtime/test setup | Only Codex certification/install/live workflows. Never invoke for alternate proof. Existing baseline store need not be modified or removed on development host. |
| `scripts/windows-desktop-test-gate.ps1` | default-only test/build configuration | Prepares MCP/plugin fixtures and invokes default Desktop tests; does not install Codex. Add narrow explicit feature arguments or separate disabled gate, preserving default behavior. |
| `.github/workflows/ci.yml` | default-only workspace/test configuration | Ubuntu fmt/check/test/all-features Clippy and Tauri inventory; cannot certify Windows disabled Desktop because Windows source is cfg excluded. Add targeted Windows evidence later, not a combinatorial matrix. |
| README/config documentation and historical release gates | presentation/config; default-only | Current baseline instructions describe existing default. Document alternate proof invocation later; preserve immutable historical certification/version evidence. |

`rah-cli` has no production Codex edge. `rah-model` exposes ModelBackend and
MockBackend; `rah-runtime` exports MinimalTestRuntime and its experimental fake
is test-only. MCP/process-plugin providers are Tool providers, not alternate
inference runtimes. `rah-profile-composition` composes Tools, not runtime auth.
No existing usable native inference adapter was found in the workspace.

## 4. Candidates and selection matrix

Y = credible within bounded proposal; L = limited proof; W = implementation or
environment work; N = absent. These assess proposed proofs, not implemented
capabilities. B means a new explicitly named production reference runtime,
not relabeling Task 501's fixture.

| Criterion | A: native OpenAI Responses | B: local deterministic reference | C: narrow reference HTTP | D: direct existing local-service API |
| --- | --- | --- | --- | --- |
| No Codex CLI | Y | Y | Y | Y if implemented directly |
| Production Desktop neutral path | W, same seam | W, same seam | W, same seam | W, same seam |
| Conversation/turn lifecycle | Y | Y | Y | Y |
| Natural streaming | Y, SSE | N; complete response valid | Protocol would need design | Service/model dependent |
| Host Tool port | Y, function loop required | Y, scripted request, limited realism | Must invent function semantics | Service/model dependent |
| Neutral RuntimeFailure | Y | Y | Y | Y |
| Authority change | None | None | None if endpoint host-owned | None if endpoint host-owned |
| New library families | Existing reqwest/TLS | None | Existing reqwest/TLS | Existing reqwest/TLS |
| Deterministic tests | Fixture HTTP plus conformance | Native | Fixture HTTP | Fixture API |
| Bounded scope | One endpoint, text/functions | Smallest code | Extra protocol/server ownership | Endpoint/model/compatibility work |
| Real provider semantics | Strong | Weak: no inference provider | Weak unless real service adopted | Strong once exact service/model proven |
| Removable without distortion | Y | Y, if isolated | Risk of architecture for invented protocol | Y, if isolated |

### A — preferred adapter

Select **native OpenAI Responses over HTTPS**, adapter-owned reqwest client,
text-only replay, SSE output, custom function calls and explicit model. No SDK,
Agents product, OpenAI CLI, Codex CLI, hosted executable Tools or provider registry
framework. This is the smallest credible *real provider* alternative found:
it adds wire translation but reuses the already present transport family and
the exact production neutral interfaces. Credentials/live expense are bounded
validation prerequisites, not a reason to fake inference.

Official API research on 2026-10-03 establishes the required semantics, but
does not establish this user's credentials, entitlement, quota or eventual model
compatibility. No authenticated endpoint was contacted. Choose the exact live
model at the future certification checkpoint; do not assume the Codex model
catalog or subscription conveys API access.

### B — fallback

One production-selectable **local deterministic reference adapter**, isolated in
`rah-runtime-reference`, with fixed documented local response semantics and an
explicit host Tool exercise mode. It must implement the neutral factory,
instance/conversation/control, carry typed failures, own cancellation/shutdown,
and operate through normal Desktop Connect/chat/Disconnect in a disabled build.
It must be compiled as ordinary production code and use the real revocable host
port; neither cfg(test) replacement nor an alternate test-only Connect command
counts. This is materially beyond Task 501, which never crossed production
composition or actual dispatch.

Advertise discovery/native continuation/streaming unsupported; replay and
cancellation supported, Tool requests supported only for its explicit reference
mode. It is not a language model. If used, certify architecture/build/lifetime
optionality with explicit synthetic-provider nonclaim; it cannot close the
selected real-provider proof. Switching to fallback requires a later decision.

### C — not selected

A minimal HTTP reference adapter/server would add transport realism but require
inventing, versioning and maintaining a protocol plus endpoint just for this
proof. No existing service was found to give that protocol independent meaning.
Fixture HTTP is useful *testing for A*, not another production provider product.

### D — repository/ecosystem audit

Existing Ollama, LM Studio and llama.cpp model choices are CodexModelProvider
configuration; their selection still starts Codex. The llama.cpp readiness
client checks health and is not an inference/conversation/Tool runtime. Current
ModelBackend mock and MinimalTestRuntime are deterministic demonstrations.
There is no already implemented native adapter to register cheaply.

A direct llama.cpp/Ollama adapter is plausible ecosystem work, but exact API,
server version, compatible model/template, Tool support, weights and local
hardware evidence would still need selection. This audit makes no unsupported
claim about current external service APIs. A is supported by checked official
semantics without requiring a model installation; D offers no demonstrated
cheaper implementation or certification path today. MCP and Process Plugin
infrastructure cannot be used as an inference-provider shortcut.

## 5. Production selection and Codex-disabled definition

Choose both minimal compile-time exclusion and **private static selection at
the Desktop composition root**. Conceptual features: default `provider-codex`;
optional `provider-openai` activates only `dep:rah-runtime-openai`. No plugins,
dynamic registration, public registry API or new neutral provider enum.

An immutable host startup setting `RAH_RUNTIME_ADAPTER=codex|openai` selects a
compiled factory. Omitted selection preserves Codex in normal default builds;
an openai-only build may infer its sole compiled adapter. Explicit unknown or
uncompiled choice fails closed; a no-provider build can launch disconnected and
returns a closed configuration failure on Connect. A both-enabled development
build defaults to Codex. Selection does not change midconnection and is never
model supplied. An internal selected-factory value yields
`Box<dyn ConfiguredRuntimeFactory>` and neutral ModelSelection, then uses the
same preflight/binding/currentness/publication pipeline. Repository authority
and Tool composition remain common host code.

**Codex disabled means Windows rah-desktop's normal dependency graph contains
no rah-runtime-codex, and its executable is built without compiling/linking that
adapter.** No Codex executable discovery, baseline access, version/schema probe
or process spawn is required or attempted. Keeping optional source/workspace
membership/lock entries is allowed. Merely compiling Codex and not invoking it
is an intermediate result and does not satisfy this target.

Features alone cannot achieve this: concrete config/source/error/test references
must be separated first. Runtime selection alone cannot exclude linked code.
Use only default Codex, OpenAI-only and one both-enabled selection check; avoid
a general feature combination framework. No user-facing provider selector is
required. Existing Connect/Disconnect controls remain; minimal adapter-aware
labels and config availability must tell the truth. Existing IPC aliases may
remain compatibility names, but their implementation must be adapter-neutral.

## 6. Cargo, dependencies and packaging

Proposed `rah-runtime-openai` depends toward rah-runtime and rah-protocol, never
the reverse; no concrete ToolRegistry dependency is needed. Desktop -> optional
adapter is justified only for factory configuration. Reuse workspace async-trait,
async-stream/futures, serde/serde_json, thiserror, Tokio and tokio-util cancellation.
Reuse the currently present reqwest 0.13.4 default-features=false rustls/stream
family; enable JSON in that adapter if needed. Implement a bounded small SSE
decoder rather than requiring a provider SDK/SSE framework. New package edges
are proposed, no crates or lockfile edits occur in 505.

| Candidate | Dependency/build/license/Windows implications |
| --- | --- |
| A | New adapter package and HTTP edge; existing reqwest/rustls transitive family, possible feature union/lock changes to inspect. No Rust provider SDK or executable installation. Review resolved licenses and advisory/maintenance status when implementing; reuse does not certify them. Static transport code increases build work; existing Tauri/NSIS remains. |
| B | New reference package, existing async/serde/cancellation crates; no HTTP/TLS, provider license or external binary. Smallest build/packaging burden but weak provider claim. |
| C | Same HTTP family plus new reference endpoint/protocol owner and its maintenance/license/hosting obligations; no proven benefit. |
| D | Likely existing HTTP family plus adapter package; external server/model licensing, Windows install, memory/hardware and version/template maintenance require audit. No RAH weight loading or model bundling. |

Do not move HTTP/provider types into rah-protocol or rah-runtime. Package-scoped
builds can exclude an optional workspace member. `--all-features` is deliberately
not Codex-free evidence. Root `--workspace --no-default-features` still selects
the Codex member and is not proof of absence in Desktop's dependency closure.

## 7. Minimal native API surface and auth/config

Implement only configured factory, instance/liveness, capability absence,
conversation/replay, owned turn/control, bounded HTTPS/SSE translation,
function-loop translation, typed failures and shutdown. Factory creation and
Connect validate local configuration and prepare client/conversation; they do
not prove network authentication or invoke inference. First ordinary turn proves
remote usability. Missing auth/model fails before ready; rejected credentials
on the first request fail through the ordinary neutral failure envelope.

Proof configuration: backend-only `RAH_OPENAI_API_KEY` and `RAH_OPENAI_MODEL`,
read once by trusted startup code into adapter-owned config. Fixed production
origin `https://api.openai.com`, Responses path only; no arbitrary endpoint or
model-supplied URL. Deterministic tests inject loopback transport privately,
without a production endpoint override. Disable redirects and automatic retry;
do not silently inherit arbitrary proxy routing for this bounded proof client.
Explicit connect/turn deadlines, byte/frame/argument/output bounds and shutdown
ownership are required. This is provider transport, not a new model Tool/network
authority and not a claim of network isolation.

ADR 0015's configurable llama.cpp endpoint authority stays unchanged. A's fixed
OpenAI origin is selected by the trusted host as an alternate route to the same
hosted provider, not a generic endpoint grant. Explicit alternate startup
selection must disclose hosted prompt/Tool-output transmission; no automatic
fallback from a local provider to OpenAI is permitted.

The [official authentication reference](https://developers.openai.com/api/reference/overview)
specifies bearer credentials. This design keeps them in the native backend,
never renderer, Tool definitions/arguments, IPC, transcript, profile persistence,
diagnostics or serialized adapter config. Redact config Debug; mark auth headers
sensitive; never log request/body/raw error source. Do not read Codex credentials
or equate API billing with Codex login. No key manager, OAuth, UI or persistent
credential product in the proof. Environment storage is a temporary host input,
not a security guarantee against local processes.

Trusted Profiles retain their existing Tool-provider authority-composition role;
they do not currently supply runtime API credentials. Do not extend their schema
or route credentials through Tool permission machinery. B needs no credentials;
C/D auth must be explicitly service-scoped if later chosen, not borrowed from
Tool authority.

## 8. Model and conversation semantics

Selected A advertises discovery=false; discover_models returns Unsupported.
Require explicit nonempty model config; RuntimeDefault is rejected as invalid
configuration because this adapter has no honest implicit default. No catalog
request or invented one-entry complete catalog; Desktop shows NotChecked, not
advertised/entitled. Unsupported capability is a first-class result. B may use
a documented fixed local default; C/D need endpoint-specific choices, not a
Codex discovery assumption.

Selected A: text_replay=true, native_continuation=false. Each neutral
ConversationId is host-assigned; each turn receives the host lease SessionId.
No durable remote conversation or previous_response_id is exposed. Desktop's
complete text snapshot begins each turn; adapter-private function/output items
exist only within that turn. Reject NativeContinuation explicitly. Transcript
storage remains completed text and does not imply native provider resumption.

The [official conversation-state guide](https://developers.openai.com/api/docs/guides/conversation-state)
documents stateless request history with `store:false` and separately durable
Conversations. Select the stateless route. Within a Tool loop retain required
provider output items, including reasoning items when present, for subsequent
requests; do not treat final text as sufficient midturn history. Include encrypted
reasoning data when the chosen model requires it for stateless function loops;
keep it adapter-private and bounded. `store:false` is not a zero-retention claim.
Cross-turn replay of text honestly omits provider-private reasoning/Tool state.

## 9. Streaming and Tool mediation

Selected A: streaming=true. The [official streaming guide](https://developers.openai.com/api/docs/guides/streaming-responses)
documents SSE and text/terminal events. Translate actual text deltas to
ModelDelta and final output to Completed; detect failure/incomplete/refusal,
malformed data and premature EOF without claiming completion. The neutral
contract also permits Started followed by one terminal complete response with
no deltas; B need not manufacture streaming. Streaming is natural for A, so it
is in selected proof scope. Unknown optional events may be ignored only without
losing terminal or Tool semantics; unsupported executable item types fail closed.

**Tool support is required for A acceptance**, not an optional unit-test claim.
The [official function-calling guide](https://developers.openai.com/api/docs/guides/function-calling)
supports custom function requests and correlated outputs. Translate only host
snapshots, with collision-free private wire aliases and reverse lookup to
original ToolName. Set strict=false to preserve host schemas, disable parallel
calls for the first proof, and wait for complete validated arguments. Preserve
required reasoning/output items alongside correlated function outputs. No built-in
hosted executable Tools are advertised.

Use HostToolPort::admit_turn once, retain the lease, use request_live, and merge
the lease's host-produced lifecycle stream with model events. Host allocates
ToolCallId; provider call IDs remain private. Host authorization/dispatch remains
the sole execution path. Return bounded ToolOutput to the provider for one final
answer. Do not synthesize ToolStarted/Finished from provider notifications.
Bound the loop initially to four provider submissions and three sequential Tool
requests per host turn; budget exhaustion fails closed with no retry. Exact
byte/time limits must be recorded and tested before implementation closure.

Duplicate provider call identity must never re-execute an admitted Tool. Reject
ambiguous duplicates, unknown aliases, malformed arguments and unadvertised
Tools safely. Permission denial and uncertain host effects remain host facts;
no automatic compensation or replay. First live proof permits only one harmless
read-only Tool such as repo.status. B's scripted Tool exercise tests the real
port but is explicitly not model function-calling realism; C/D would need equally
honest provider support before a comparable claim.

## 10. Error, cancellation and shutdown mapping

Adapter-private `OpenAiAdapterError` variants: InvalidConfig, HttpTransport,
HttpRejection(status), Decode/Protocol, StreamInterrupted, LimitExceeded and
Closed. Keep original reqwest/decode typed causes process-local as appropriate;
never use raw provider body, key, URL query, prompt or source Display/Debug in
serialized diagnostics. Attach them via RuntimeFailure under ADR 0032. No
OpenAI error import/downcast in neutral Desktop lifecycle code.

| Observation | Neutral diagnostic |
| --- | --- |
| Missing key/model or unsupported RuntimeDefault | Connection/InvalidConfiguration |
| HTTP rejection including auth/model/rate limit | SessionStart or Turn/ProviderRejection |
| Request/stream network failure | SessionStart or Turn/Transport |
| Invalid SSE/JSON, unknown executable output, premature termination | Turn/Protocol |
| Bound exhaustion | Turn/Operation |
| Retained handle after close/shutdown | appropriate operation/Unavailable |
| Unsupported native continuation | SessionResume/Operation |

Use actual stage for operation, not arbitrary relabeling. rpc_code stays None;
HTTP status is not an RPC code. Closed neutral templates suffice even where
failure kinds lack a provider-specific distinction. Tests downcast local sources
and prove IPC/event projection/transcript/log paths omit secret sentinels.

Selected A: cancellation=true means local request/stream cancellation, not
remote rollback. Turn control interrupts active client I/O/loop, stops new Tool
submissions and produces one Cancelled terminal with no later Completed after
local stop wins; AlreadyTerminal is distinct. If local stop cannot be confirmed,
return a failure rather than invent Stopped. Completion/cancel races have one
owned terminal decision. Drop/close/shutdown cancel and await owned work, bounded
by host policy; no detached request worker or Codex process assumption.

Disconnect revokes host scope before adapter shutdown, and the host drains
already admitted effects as today. A submitted HTTPS request or admitted Tool
can have effects despite lost response. Do not call remote cancel/rollback APIs
for this foreground proof, infer rollback, replay requests, or claim remote
generation ceased merely because the local stream closed. B can confirm its
local worker stopped; C/D require their transport-specific honest semantics.

## 11. Authority review and neutral-contract fit

Factory owns provider transport/config only. Conversation receives neutral seed
and revocable request-only Tool port. No registry mutation, permission control,
repository selection, HostExplicit control, shell, process or filesystem access
is supplied to A. Host-selected repository remains a Tool boundary; A needs no
execution cwd and no raw repository root as inference authority. Existing profile
composition/publication/currentness and effect accounting remain unchanged.

The six capabilities, ModelSelection/Unsupported discovery, TextReplay,
TurnHandle/control, live Tool lease and RuntimeFailure cover A without redesign.
No identified contract gap justifies classification C. An implementation-discovered
gap must be reported with provider evidence, not patched into core silently.
HostExplicit remains exactly 11. Network credentials do not expand Tool authority.

## 12. Deterministic validation design

Tests must exercise observable neutral contracts and production selection,
without paid credentials, Internet, live model or GPU. Adapter wire tests use a
loopback fixture speaking the documented API; fixtures do not become a provider.

| Group | Required evidence |
| --- | --- |
| Factory/config | Validate/create; missing key/model; fixed endpoint; no inference during Connect; invalid adapter and disabled selection fail closed. |
| Capabilities/models | Exact bits; Unsupported discovery; explicit selection; rejected RuntimeDefault/native continuation; NotChecked publication. |
| Conversation/turn | Host conversation identity vs lease SessionId; text snapshot replay across two turns; ordered actual deltas and exactly one terminal; nonstreaming contract fixture. |
| Wire bounds | Fragmented SSE/UTF-8, failure/incomplete/refusal/EOF, malformed/oversized frames and args, response and turn deadlines, submission budget, no auto-retry. |
| Failure envelope | Original typed source survives; sanitized diagnostic/event projection; sentinel key/body/prompt absent from IPC, persisted text and diagnostic logs. |
| Tool round trip | Wire alias collision/unknown rejection; complete JSON request -> production host authorization/dispatch -> host lifecycle -> function output -> final text. Denial, duplicate ID, uncertain result and bounded loop fail safely. |
| Revocation | Retained conversation/port after Disconnect, shutdown, and repository A->B cannot dispatch; already admitted effect remains accounted for. |
| Cancel/close/shutdown | Delayed headers/body/Tool result; cancellation/terminal race; no later completion or detached task; later send fails; liveness and close/shutdown idempotence. |
| Desktop selection | Normal production Connect path chooses configured factory; default Codex behavior unchanged; alternate never calls executable resolver/certifier; all repository/model/profile/connection generation gates retained. |
| Build/IPC | Windows Codex-only, OpenAI-only dependency/build/test evidence, one both-enabled selection test; matching Tauri manifest/capabilities/frontend command inventory and truthful status. |

Use focused checks for implementation, canonical Windows Desktop gate with its
provider fixtures for composition, and applicable milestone fmt/check/test/Clippy
and diff gates once the integrated proof is ready. Keep source fixed during
validation. Do not use passing adapter tests or Ubuntu CI as a substitute for
real Windows Desktop evidence.

## 13. Bounded production/live validation

Later explicit live authorization and usable API credential/model are required;
none is inferred as authorization to spend during 505. In a clean Windows VM
or equivalent dedicated host with no installed Codex CLI/baseline artifacts,
launch the exact OpenAI-only production Desktop executable and inspect its
WebView/Tauri flow directly. Connect locally, one short ordinary streaming turn,
one separate harmless read-only Tool turn, Disconnect and normal exit. Record
exact model, endpoint origin, deadlines/output/turn budget, actual request count,
provider result, owned process evidence and screenshots without credentials.
Maximum two top-level successful chat turns; the Tool turn has one Tool and a
bounded continuation request. Do not repeatedly spend until green. A failed or
unavailable API attempt retains evidence and cannot certify remote success.

Connect readiness proves local runtime composition, not credential entitlement;
ordinary turn must prove remote inference. Tool turn must show actual authorized
requested/started/finished lifecycle and provider use of the output. Verify host
scope withdrawal and no active owned request on Disconnect. Cancellation/error,
missing-model config, unsupported discovery/native continuation, stale handles
and repository-switch isolation are primarily deterministic gates; supplement
with bounded production observation where necessary, not paid exhaustive cases.

This proves an alternate adapter, not full provider UX, credential product,
broad model selection, packaging parity, reliability, entitlement or rollback.

## 14. Exact future Codex-free build and executable evidence

These are proposed commands after the named features/packages exist, not
commands run in Task 505. Execute on Windows using a previously nonexistent
target directory; set CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR to the same path.

```powershell
$env:CARGO_TARGET_DIR = 'F:\temp\rah-task508-openai-only-target'
$env:RAH_TEST_TARGET_DIR = $env:CARGO_TARGET_DIR
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc --no-default-features --features provider-openai -e normal,build
cargo build -p rah-desktop --bin rah-desktop --target x86_64-pc-windows-msvc --no-default-features --features provider-openai --message-format=json
cargo test -p rah-desktop --bin rah-desktop --target x86_64-pc-windows-msvc --no-default-features --features provider-openai
```

Capture tree, compiler artifact messages, exit codes and executable path/hash.
Require no rah-runtime-codex in resolved normal/build graph or compiler artifacts
for that isolated build. Inspect dev/test graph separately and keep it Codex-free
for alternate Desktop tests. A source grep, lockfile entry or string search alone
is not linking evidence. Do not build `--workspace` or `--all-features` as the
absence proof. Run selected non-Codex workspace packages separately as appropriate.

Move/build the artifact in the dedicated Codex-free environment, record absence
of installed CLI, PATH resolution and baseline store/overrides, and launch the
same identified binary with host adapter/model/key configuration. `Get-Command`
alone or hiding Codex on PATH is insufficient machine-absence evidence; use a
clean environment inventory. Do not uninstall or move the developer's Codex.
Record actual executable identity/size/SHA256 separately from path spelling.
Process ownership/launch monitoring must show no Codex invocation throughout
Connect, both turns, Disconnect and exit. Absence of a child alone misses probes;
also assert resolver/certification non-entry deterministically. Existing runtime
config prefs and profiles must not cause fallback to Codex.

For a later packaging claim, build/install the same feature-selected NSIS
artifact in that clean environment and repeat launch/lifecycle; raw executable
success alone is not an installer certificate. This task selects executable
build/operation proof first; no release packaging/default cutover is implied.

## 15. Acceptance and implementation slices

The selected real-provider proof is complete only when all of these hold:

1. A implements existing experimental neutral contracts without provider types
   in neutral/core APIs and passes deterministic conformance/negative gates.
2. Normal production Desktop selects A through the shared factory/preflight,
   conversation, turn/currentness and teardown path.
3. Windows OpenAI-only build excludes compiling/linking rah-runtime-codex;
   alternate Desktop test graph also excludes it.
4. Identified production executable launches/operates with no Codex CLI installed
   and no baseline dependency or Codex invocation.
5. Connect, one ordinary streamed response, one authorized read-only host Tool
   round trip and Disconnect succeed with preserved lifecycle evidence.
6. Unsupported discovery/default/native continuation, failure sanitization,
   cancellation/shutdown, stale-handle revocation and repository binding pass
   their applicable deterministic/production gates.
7. HostExplicit remains exactly 11; no authority expansion or ToolRegistry bypass.
8. Default Codex build/admission/lifecycle remains passing and unchanged as default.

| Task | Recommended bounded scope and exit |
| --- | --- |
| 506 | Codex-optional Desktop build/composition seam FIRST. Optional provider-codex dependency; cfg all Codex config/artifact/error/test references; private selected factory boundary; neutral source/status/config shell; preserve default behavior. Build/test Windows no-provider configuration (disconnected; Connect fails closed), prove no Codex dependency, and use existing injected fixture only for deterministic selection checks. No new production fake or alternate API calls. |
| 507 | Native OpenAI adapter core and deterministic conformance. New isolated adapter package, existing transport family, explicit model/key config, SSE, bounded function loop/live host port, typed failures and owned cancellation. No Desktop UX, paid inference or default change. |
| 508 | Production OpenAI composition selection plus Codex-free Desktop proof. Add provider-openai feature/startup setting, minimal truthful renderer/config integration, shared production path tests; run isolated Windows disabled build and separately authorized bounded live proof on clean Codex-free host. This is the first task that may establish the full selected criterion. |
| 509 | Optionality cleanup only after 508 evidence. Update supported invocation/docs and narrowly identified build/packaging gates; remove remaining misleading presentation/setup prerequisites. Preserve historical Codex evidence/default. No general provider productization or automatic default switch. |

The prerequisite seam is concrete build/composition work, not another neutral
contract redesign. Task 506's disconnected executable is deliberately not the
alternate proof and cannot close Task 500's removal criterion.

## 16. ADR decision, classification and exact next task

**ADR-B — ADR 0033/current ADRs sufficient.** ADRs 0001/0002 establish replaceable
RAH-owned contracts and adapter separation; ADR 0005's process requirement
governs Codex, not every provider; ADRs 0032/0033 own failures and lifetime.
Optional dependency features, fixed known-adapter selection and adapter-owned
HTTP translation are private composition details, not a new public extension
model, authority rule, protocol or core dependency direction. No new ADR is
created. A future dynamic registry, new authority or neutral API revision would
require separate review. The OpenAI edge is outside core, directed inward as
required, with task-owned reason documented above.

**B — CODEX CANNOT YET BE MADE OPTIONAL WITHOUT BUILD-COMPOSITION WORK**

One real proof candidate A is selected, with B as explicitly weaker fallback.
Hard dependency/configuration/admission edges are identified; strong disabled
definition, neutral fit, authority and deterministic/live acceptance are bounded.
The current source cannot build the intended Windows Desktop without Codex
adapter imports/dependency; remove that prerequisite first rather than pretending
runtime neutrality has already made the build optional.

Exact next task: **Task 506 — Codex-optional Desktop build and composition seam**,
with precisely the first slice above. Do not implement A automatically, change
current provider default, select a v0.34 capability or bump any version.

## 17. Task 505 validation and publication

Research-only required check: `git diff --check`. No local workspace suite or live
API call is required or claimed. Only this report is staged for
`docs: select alternate runtime proof`; normal push to GitHub master only, no
mirror modification, tag or release. Task closure records commit SHA, push
result, natural exact-head CI (if triggered), remote equality and final clean
worktree. This report cannot embed its own future commit identity. A later
exact-head CI pass validates its configured gates, not alternate-provider proof.
