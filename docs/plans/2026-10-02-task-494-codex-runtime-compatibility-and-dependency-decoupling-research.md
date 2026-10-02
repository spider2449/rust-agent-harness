# Task 494 — Codex runtime compatibility and dependency decoupling research

Date: 2026-10-02 (Asia/Taipei). Research only; no redesign implemented.

## 1. Starting checkpoint and evidence discipline

Primary checkout and dedicated `F:\Temp\rah-task494` worktree started clean at
`d5ef2e541d744f8070019878f6754d16a4b974a8`. GitHub master was independently
queried and matched. GitHub push CI `37006252679` reports success for that exact
SHA. Task 493's classification remains A — DESKTOP REGISTRATION DEFECT CORRECTED
AND PRODUCTION ACCEPTANCE PASSES. The removed Task 438 residue was not restored.

RAH remains 0.33.0, HostExplicit exactly 11, preferred/current certified Codex
0.157.1, and v0.34 capability NONE SELECTED. Published release source
`ad25355c81ed0a39499cd75e5b240a7594f3e7a5` and Release `400948501` are immutable.
Mirror synchronization is a transport concern, not architecture evidence.

Evidence labels used below:

- **Source**: current checkpoint source and accepted documentation.
- **Schema**: locally generated schema from the identified executable.
- **Live**: one direct app-server control per executable, outside production RAH.
- **Historical**: supplied Task 492 observation or earlier accepted records.
- **Inference / recommendation**: proposed interpretation or future work.

Authoritative reading included README, CHANGELOG, ARCHITECTURE,
ARCHITECTURE_GUARDRAILS, SECURITY, ADRs 0001, 0002, 0005, 0006, 0015 and 0030,
and Task 493's report. No authority ADR is changed by this report.

## 2. Current certification model: four separate concepts

| Concept | Current implementation / limitation |
| --- | --- |
| A. Runtime identity | Explicit executable selection; version; saved manifest and binary/sibling SHA-256; Windows file identity is separate live certification evidence. |
| B. Runtime certification | Exact source-defined admitted version set plus required generated schema checks. This is compatibility policy, not model availability. |
| C. Model catalog | Dynamic app-server catalog exists but RAH currently does not query it. Catalog is not immutable release evidence or guaranteed entitlement. |
| D. Selected-model compatibility | Actual selected provider/model can fail after thread and turn acceptance. Successful thread creation is insufficient; completed inference demonstrates request-time operation only. |

**Source:** `crates/rah-runtime-codex/src/lib.rs:27-34` defines
`CURRENT_CERTIFIED_CODEX_VERSIONS = ["codex-cli 0.157.1"]` and
`PREFERRED_CURRENT_CODEX_VERSION = "codex-cli 0.157.1"`. ADR 0030 already replaces
the conceptual permanent pin with an explicit exact certified set and a
deterministic preferred member. It requires a certification task, schema audit,
deterministic validation and live direct plus Desktop certification to add a
version. A set with one member is the present implementation, not an architectural
requirement that the set always have one member.

`crates/rah-desktop/src/codex_baseline.rs` selects host override
`RAH_CODEX_EXECUTABLE`, otherwise the preferred store entry under
`CODEX_BASELINE_HOME` or `%LOCALAPPDATA%\codex-baselines`, otherwise PATH if the
preferred directory is absent. An invalid present baseline fails; it does not
silently fall back. Store verification checks manifest shape/version/platform,
non-reparse regular files, canonical executable path, MZ bytes, SHA-256, reported
version and sibling `codex-code-mode-host.exe` hash. The override branch returns
the host path without those store hash checks.

`crates/rah-runtime-codex/src/process.rs` resolves the executable, runs
`--version`, rejects versions outside the exact set, generates and checks schemas,
then spawns `app-server --stdio`. This validation applies to override and PATH
as well. It does **not** authenticate arbitrary override/PATH bytes through a
certification-bound SHA allowlist. Version self-report and matching schema are
not cryptographic executable provenance. The baseline manifest is a local
integrity record, not a signature rooted in an external certification authority.
The distinction matters to future maintenance-artifact security; this report
does not claim a stronger existing guarantee.

Windows live certification separately compares child ownership, canonical/raw
paths, file-object identity and hash. Neither Desktop baseline resolution nor
adapter startup performs a general running-child file-object comparison. Hash
equality alone is not Windows executable identity, and these checks do not prove
race-free execution binding.

### Check timing

| Stage | What happens |
| --- | --- |
| Desktop application startup | Disconnected preferences/status initialization; no automatic runtime connection, restored authority or model probe. Displayed preferred version is policy, not observed executable identity. |
| Explicit Connect preparation | Host model/provider syntax and endpoint validation, composition/repository generation snapshot, executable resolution and preferred-store verification. |
| Runtime creation inside Connect | Adapter version admission, generated schema inspection, subprocess spawn, initialize/initialized; optional bridge responder installation. No `model/list`, inference, or thread validation. |
| `AgentRuntime::start` | Nonempty messages; bridge definitions snapshot; `thread/start`; verify returned effective model/provider and cwd; `turn/start`; private thread/turn mapping to new RAH SessionId. |
| Before `thread/start` | Model string syntax/length only, not existence, retirement, entitlement or successful generation. |

`process.rs` embeds `fixtures/schema_contract_0_157_1.json` through
`include_str!`. Checks cover required schema files/fields and experimental
dynamic Tools, not complete protocol semantic equivalence. Adding a version to
the set alone is insufficient certification.

## 3. Observed lifecycle problem

**Historical:** Task 492 selected `gpt-6.1-sol`, displayed `Chat failed`, and an
older model worked. Its original backend cause was not retained. It does not
prove 0.159.2 was required and cannot be retrospectively assigned this report's
new error. ADR 0030 independently records an earlier 0.149.0 versus 0.157.1 model
control difference. Historical certification cannot freeze server availability.

**Live:** today's controlled comparison demonstrates a runtime/model difference
with the current account. **Inference:** both new-model rollout and eventual
model removal can outlive RAH's release. A working runtime/model pair is a
time-bound operational fact. Release, CLI and model lifetimes remain separate.

## 4. Concrete Codex dependency and coupling map

```text
Desktop host: repository/profile/permission/lease/generation composition
  -> concrete Codex executable resolver + CodexModelConfig
  -> Arc<CodexRuntime> (Desktop connection/chat/cancel/recovery ownership)
      -> ProcessTransport: version/schema -> codex app-server -> stdio
      -> AppServerConnection: initialize, RPC correlation, notifications
      -> private SessionId <-> threadId/turnId map
      -> dynamic Tool schema/request/result translation
          -> host-supplied ToolRegistry and permission checks
      -> neutral AgentHandle / AgentEvent
  -> closed Desktop ChatEvent / FrontendError -> frontend/status.js
```

| Coupling category / concrete location | Classification and responsibility |
| --- | --- |
| Protocol: `protocol.rs`, `connection.rs`; JSON-RPC IDs, initialize/initialized, thread/start, thread/resume, turn/start, turn/interrupt, agentMessage deltas, turn/completed | Codex-specific adapter detail. No wire DTO should move to neutral core. |
| Protocol: `AgentRuntime`, `AgentHandle`, `AgentRequest`, `AgentEvent` | RAH-owned contract. Acceptance, stream, termination and cancellation semantics need explicit lifecycle design. |
| Process: `process.rs`, transport shutdown, bounded stderr, child ownership | Codex-specific invocation/transport. Host runtime lifetime and cancellation deadlines are RAH-owned. |
| Process: `codex_baseline.rs`, Desktop concrete runtime fields and recovery methods | Mixed boundary: host selection is correct, but lifecycle/controller types are concrete Codex. |
| Model/provider: `model_config.rs`, `runtime.rs:restricted_thread_params` | Codex-specific provider names, Responses config and `modelProvider` encoding. |
| Model/provider: Desktop `codex_model_config`, model generations, host endpoint validation | Mixed: host desired/effective configuration and authority are RAH-owned; producing Codex config directly couples composition. |
| Tool bridge: `bridge.rs` snapshots `ToolDefinition`, aliases names, emits `inputSchema`, correlates dynamic calls/results and deduplicates | Encoding/routing are Codex-specific; Tool definition, authorization, actual execution and truthful lifecycle events are RAH-owned. |
| UI: `frontend/index.html`, model/provider settings, Codex version/source status and `status.js:148` | Mixed: provider/model selection and useful errors are generic needs; Codex-only connection labels/status are adapter presentation. |
| Certification: lib.rs exact set, process.rs embedded contract, fixture JSON, baseline manifests, scripts/codex-baseline.ps1 and live gates | Codex adapter certification detail under host-owned admission policy. Historical releases are separate evidence. |
| Session: `runtime.rs:SessionRecord`, `rah-session`, Desktop bounded transcript replay | Mapping is correctly adapter-private; resume semantics and concrete Desktop lifecycle remain mixed. |

`rah-runtime` has no Codex version/model policy. `rah-protocol` has no Codex wire
types. `rah-desktop` directly imports adapter config/errors/runtime and stores
`Arc<CodexRuntime>` in connection/chat ownership; its lifecycle is operationally
Codex-dependent despite neutral library boundaries. `rah-core/src/lib.rs` is
minimal; it does not currently provide the replacement orchestration system.

## 5. Model catalog findings

**Schema:** saved 0.157.1 successfully generated its protocol bundle outside the
repository. `ClientRequest.json` contains `model/list`;
`v2/ModelListParams.json` exposes cursor, limit and includeHidden, with **no
per-request provider selector**. `ModelListResponse.json` describes data and
nextCursor, model ID/model string/display name, effort/default/visibility and
upgrade metadata. New candidate schema was also generated. New official methods
must not be assumed supported merely because current documentation lists them.

**Live:** both versions accepted `model/list` before `thread/start`, with
`{"limit":100,"includeHidden":true}`; one page returned, no continuation.
0.157.1 returned `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-reserve`,
`gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`, `codex-auto-review`.
0.160.0 returned these plus `gpt-6.1-sol`.

**Official documentation:** [App Server](https://learn.chatgpt.com/docs/app-server)
describes model/list as selector discovery with pagination and account/client
dependent results. [ChatGPT-plan app-server configuration](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
warns it may be a bundled client catalog and that successful inference verifies
access for the request. The [model page](https://developers.openai.com/api/docs/models/gpt-6.1-sol)
documents the exact API model, but API documentation does not establish Codex
account entitlement. Official pages were fetched on this date; exact local
version schema and live observations take precedence for this audit.

Conclusion: catalog discovery **exists and is usable**, but a reliable universal
server-side availability/entitlement check has **not** been established.
Authentication may influence results; both controls used the same ambient
ChatGPT authentication and equal account/updated metadata. No logged-out,
alternate-account or custom-provider matrix was run. Because provider selection
is currently applied at thread creation and model/list has no provider parameter,
a default catalog cannot certify Ollama, LM Studio, llama.cpp or custom endpoints.
Provider-scoped discovery must be tested/designed separately, with no new endpoint
authority inferred from a catalog.

IDs are opaque operational identifiers, not guaranteed permanent. No evidence
guarantees immediate removal of retired entries or completeness for custom
models. Hidden does not mean unavailable. A catalog can differ by client version,
as today's comparison demonstrates; absence may be stale client data rather
than actual server retirement. No retired model was intentionally exercised.

## 6. Compatibility errors: what can actually be distinguished

**Schema:** 0.157.1 `TurnCompletedNotification.json` defines optional
codexErrorInfo and additionalDetails. Categories include `unauthorized`,
`badRequest`, `other`, context/usage/rate limits, internal server errors and
transport variants with optional HTTP status. There is no dedicated
model-retired or CLI-too-old category. Local camelCase schema values, rather
than illustrative documentation spelling, are the wire evidence.

| Failure class | Existing evidence and reliable distinction |
| --- | --- |
| Model does not exist | May be provider bad-request/HTTP/message detail; no distinct guaranteed model-not-found code in inspected Codex contract. Preserve diagnostic; do not universally infer from 400/404. |
| Model retired | Same limitation; retirement versus typo or account restriction is not generally distinguishable. Requires explicit provider evidence. |
| Model exists but CLI too old | No direct typed category. Today's same-account A/B control supports compatibility difference; `other` alone does not diagnose age. |
| Invalid provider configuration | Host syntax errors are typed `InvalidModelProviderConfig`; Codex config rejection may be JSON-RPC or turn error. Valid but offline/configured incorrectly must not be called syntax-invalid. |
| Authentication failure | `unauthorized` or explicit upstream auth/status evidence can support an auth presentation; not every auth failure is guaranteed to use it. Never infer from generic failure. |
| Server/API failure | Typed transport/HTTP/server variants can support broader provider failure; exact subcause depends on retained payload. |
| Protocol/schema mismatch | `SchemaInspection`, `SchemaMismatch`, `MalformedFraming`, `ProtocolViolation` are explicit adapter errors; schema checks are bounded and do not prove all semantics. |
| Unsupported request field | Can surface as JSON-RPC invalid params or provider badRequest/message, or be ignored if additive parsing permits it. No universal dedicated field-error category. |
| Uncertified executable | `VersionMismatch` rejects an unknown reported version before app-server spawn. Store integrity failure maps separately to baseline invalid; no current universal override/PATH hash-admission policy. |

**Source loss points:** `protocol.rs:parse` retains RPC code/message but drops
arbitrary error data. `runtime.rs:agent_error` stringifies CodexAdapterError into
`AgentError::Runtime`; turn/completed failed extracts message only, dropping
codexErrorInfo/additionalDetails. Additive `error` notifications are not surfaced
as structured generic failures. `main.rs:frontend_error` has useful closed
connection categories but coalesces several adapter variants.
`main.rs` chat startup failure and `AgentEvent::Failed` paths privately trace
detail, then emit `ChatEvent::Failed { ChatRuntimeFailed }`.
`frontend/status.js` renders that as `Chat failed`. Private live evidence records
failure stage/generations, not the full provider error. Thus today's backend
contains more useful information than the frontend, but precise structured
information has already been lost before UI mapping.

**Recommendation:** first preserve stage, adapter failure kind and optional
observed provider status/category privately; provide a sanitized closed public
diagnostic with an unknown fallback. Never expose raw config, credentials,
stderr or arbitrary provider message to the UI without review. Do not invent
retirement/age categories or parse arbitrary prose into authoritative policy.

## 7. Controlled runtime/model observations

Environment: Windows NT 10.0.26100.0, x64, PowerShell host, Node v24.20.0;
research cwd `F:\Temp\rah-task494-evidence`, same inherited environment and
ambient Codex account/config, explicit provider `openai`, model `gpt-6.1-sol`.
Account/updated payload equality was checked without publishing account data.
No credentials, trusted store entries or host configuration were changed.

| Identity | Saved certified baseline | Installed candidate |
| --- | --- | --- |
| Reported version | codex-cli 0.157.1 | codex-cli 0.160.0 |
| Path | `C:\Users\morefunfun\AppData\Local\codex-baselines\0.157.1\codex.exe` | `C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe` |
| SHA-256 | `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574` | `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d` |
| Windows file ID (fsutil) | `0x0000000000000000002500000000e992` | `0x0000000000000000001d00000000b96d` |
| UTC interval | 2026-10-02T13:00:13.073Z to 13:00:15.916Z | 2026-10-02T13:00:15.917Z to 13:00:20.650Z |
| Exit | 0, no forced kill; stderr empty | 0, no forced kill; stderr empty |

File IDs are recorded independently of SHA and path; no running-image identity
comparison or TOCTOU guarantee is claimed. `scripts/codex-baseline.ps1 verify
0.157.1` passed. `inspect-installed 0.159.2` found no exact candidate; PATH's npm
package has moved to 0.160.0. Neither candidate was admitted or saved as trusted.

Each control sent initialize (experimentalApi false), initialized, model/list,
thread/start, then turn/start. No dynamic Tools were advertised; server requests
would receive an error. Thread parameters used explicit research cwd,
approvalPolicy never, read-only sandbox, shell/unified execution/memories disabled,
web/image/app defaults disabled and empty mcp_servers. This mirrors a restricted
neutral control, but ambient config generated warnings and MCP startup-status
notifications: it was **not hermetic provider isolation**. Actual item types were
userMessage only for baseline, userMessage and agentMessage for candidate; no
Tool execution item or server Tool request was observed.

Exact common substantive requests (JSON-RPC IDs 3 and 4):

```json
{"id":3,"method":"thread/start","params":{"model":"gpt-6.1-sol","modelProvider":"openai","cwd":"F:/Temp/rah-task494-evidence","approvalPolicy":"never","sandbox":"read-only","config":{"features":{"shell_tool":false,"unified_exec":false,"memories":false},"tools":{"web_search":false,"view_image":false},"apps":{"_default":{"enabled":false}},"mcp_servers":{}}}}
```

```json
{"id":4,"method":"turn/start","params":{"threadId":"<returned private thread id>","input":[{"type":"text","text":"Reply with exactly RAH494_OK. Do not use tools."}],"approvalPolicy":"never","sandboxPolicy":{"type":"readOnly"}}}
```

Both thread/start responses returned the requested model/provider and research
cwd; both turn/start calls returned a turn ID. Baseline thread ID:
`01a0fcb3-73b1-70c1-88af-3e99c8c09f92`; turn ID:
`01a0fcb3-7418-78f1-9697-c719075535dd`. Candidate thread ID:
`01a0fcb3-7d71-70f1-8a73-960e7d256af9`; turn ID:
`01a0fcb3-7dd2-7ab2-ac46-003e235d6d3b`.

Baseline event sequence included thread/started, turn/started, user item,
`error` with willRetry false, then turn/completed status failed. Exact error:

```json
{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT account.\"}}","codexErrorInfo":"other","additionalDetails":null,"misalignment":null}
```

Candidate emitted thread/started, turn/started, user item, assistant deltas,
completed assistant item `RAH494_OK`, then turn/completed status completed.
**R2:** baseline fails and newer candidate succeeds for the same control. This
supports an operational runtime/model compatibility difference; it does not
isolate the upstream internal mechanism, prove a minimum required CLI version,
prove 0.159.2 behavior, or certify Desktop/Tools on 0.160.0. One attempt each,
no retry or fallback. The model-dependent symptom was reproduced directly,
not as a new production Desktop reproduction of Task 492.

Raw full requests/results/event order and process exits are retained outside
the repository in `F:\Temp\rah-task494-evidence\01571.json` (SHA-256
`0cc3e3bb33efcea79bd61b02a9bdc5c0466defe3b32a4a6080b9ee6f6f00bf89`)
and `01600.json` (SHA-256
`d5d0c3dcc618f86b9acae15db0f9523d6a4d093ddf8b80460eda2ee30ba90d28`).
They may include private ambient metadata and are not publication artifacts.
`probe.cjs`, candidate path and generated schema directories are retained there.
Research process children exited; no production RAH runtime was started.

## 8. Retirement and reconnect behavior

**Inference from source:** an unavailable selected model remains an inactive
desired preference across reconnect. Connect repeats initialization without
availability validation; the next chat submits the same selected model and can
repeat the failure. No current code guarantees detection before chat or changes
the preference after failure. Server changes need not alter the executable or
schema. No intentional retired-model request was needed for this conclusion.

Thread/start effective equality prevents silent substitution; preserve it.
Do not silently choose another catalog entry, provider or runtime. A preflight
cannot guarantee future turns because availability may change after the check.

## 9. Certified runtime set and release independence

**Feasibility:** high for multiple exact versions under ADR 0030; current
singleton constant, preferred-only Desktop lookup and single embedded fixture
must be addressed in later design. Baseline storage/scripts already support
multiple version directories, but storage is not admission. Each admitted
version needs documented schema/Tool protocol compatibility and live Windows
identity, neutral chat and Tool controls, plus relevant deterministic CI.

Migration should preserve exact versions/bytes, immutable prior evidence,
explicit preferred selection, no highest-version/PATH auto-promotion, and
explicit rollback to a still-admitted artifact. UI selection must be from
host-certified identities, not directory enumeration or provider suggestions.
Revoking current support does not delete historical artifacts or claims.

**Post-release operational artifact:** conceptually feasible, **not supported
by the present binary**. Admission/preference constants and embedded schema
contract are compiled into RAH; Desktop store discovery is keyed to that
compiled preference. A new source policy currently requires rebuilding RAH,
even if the product version string were unchanged. Existing scripts can archive
bytes but cannot authorize the running binary to accept a new version.

Future design must specify a host-controlled certification artifact bound to
RAH adapter/contract version, platform, exact executable and sibling hashes,
schema contract digest, deterministic evidence and Windows live evidence.
Whether authenticated offline import or locally explicit installation is used
requires a later decision. Trust roots, tamper protection, expiry/revocation,
rollback and update ownership must be specified; a bare mutable manifest or
remote allowlist is insufficient. A changed adapter implementation still needs
a RAH build/release. Artifact maintenance can only extend already-supported
semantics, not introduce arbitrary new protocol interpretation.

CI should validate every supported contract and negative admission path;
paid/live controls remain separately authorized. Historical release certification
must never be rewritten to mean the current operational set. No mutable remote
allowlist, runtime download or certification policy change is implemented here.

## 10. Connect-time gate feasibility

The desired sequence is feasible **as bounded staged evidence**, not one
universal readiness boolean:

```text
host selects identity -> integrity/admission/schema -> initialized runtime
 -> catalog snapshot if meaningful for this provider
 -> selected-model preflight evidence -> publish connected state
```

| Proposed outcome | Evidence limit / justified use |
| --- | --- |
| READY | Certification plus successful selected-model inference can support ready-at-this-time. Catalog membership and thread creation alone cannot. Preserve later-turn failure handling. |
| MODEL_UNAVAILABLE | Use only explicit provider rejection establishing unavailability; catalog absence alone is insufficient. Unknown catalog completeness must remain visible. Cannot universally distinguish retired, typo and entitlement denial. |
| RUNTIME_MAY_BE_OUTDATED | Advisory inference from explicit evidence or controlled comparison; not a guaranteed Codex error category. No silent upgrade. Today's R2 supports recommending a separate certification task. |
| PROVIDER_ERROR | Supported where actual configuration/auth/HTTP/transport evidence exists; retain an unknown broader failure fallback. |
| RUNTIME_UNCERTIFIED | Existing exact-version admission rejects unknown versions. Future hash-bound admission must reject unverified bytes as well. |

A meaningful inference preflight sends a bounded fixed prompt, consumes account
usage, may persist provider conversation state and must have cancellation,
timeout and owned cleanup. It must exclude repository content/history and
Tools; no ToolRegistry authority is activated by successful preflight. Merely
moving thread/start into Connect would not have caught today's failure.

ADR 0015 prohibits automatic provider probing except its explicit bounded
readiness exception. Therefore a general inference/catalog probe requires a
later explicit design/ADR scope decision; it must not be smuggled into existing
Connect as an incidental refactor. Task 494 authorizes research, not that policy
change. This does not block a narrow error-preservation task or a later designed
explicit preflight. A catalog-only check can offer suggestions and warnings,
but cannot honestly implement strict READY/MODEL_UNAVAILABLE across providers.

Use immutable runtime/provider/model/generation snapshots. Discard stale results
when configuration or repository/runtime generation changes. No retry, automatic
fallback, auto-connect, provider-controlled executable choice or uncertain-effect
replay. Keep gate results separate from certification and authority.

## 11. Model selector research

The current UI is a provider select plus bounded **free-text model input**, not a
hard-coded list of model options. Replacing static strings is therefore not an
exact description of the current UI. Future dynamic suggestions should coexist
with explicit host model selection where catalogs are incomplete.

Refresh on explicit request or the approved connection lifecycle; bound requests
and pagination, timestamp and scope caches to executable identity, provider
configuration and account context without retaining secrets. Offline cached
entries must be labeled stale and cannot certify readiness. Keep a selected
missing/retired entry visible with its diagnostic; require the human to select
another model. Do not auto-replace it with catalog default/upgrade metadata.
Provider-specific catalogs and unknown enumeration capability must be supported;
an OpenAI catalog must not be shown as availability for a local/custom provider.

## 12. Existing neutral runtime abstractions

**Source:** `rah-runtime/src/lib.rs` has AgentRuntime start/resume/cancel,
AgentHandle(SessionId, event stream), and AgentError SessionNotFound,
InvalidRequest, Runtime(message). `rah-protocol/src/agent.rs` has messages,
request identity and empty AgentOptions. `rah-model/src/lib.rs` separately has
ModelBackend::complete(ModelRequest), neutral Tool definitions/calls, deltas,
usage and result/error stream. `rah-session` stores neutral SessionId, messages,
Tool outputs, status and host metadata. MinimalTestRuntime/MockBackend establish
that the libraries are not intrinsically Codex-only, but do not constitute a
production native runtime or selectable Desktop alternative.

The minimum semantic interface needed by Desktop includes host-owned runtime
construction/admission and shutdown, immutable effective configuration, operation
start returning identity/events, cancellation with terminal confirmation,
bounded recovery and liveness. Catalog/preflight are optional capabilities with
explicit unsupported/unknown states. Current native resume versus replay and
repeated-turn semantics must be resolved; simply adding model/list to AgentRuntime
would leave most Desktop lifecycle coupling intact.

## 13. Proposed neutral seam and responsibilities

**Design recommendation, not accepted API:** separate a host runtime factory /
owned connection lifetime from an agent operation interface. Describe semantics
before naming methods. The host selects an adapter and immutable model/provider
configuration; the adapter reports supported features and effective configuration.
Operations return RAH identities and neutral event streams. Host ownership covers
shutdown, cancellation deadlines, stale generation rejection and hard recovery.
Optional discovery/preflight returns observed catalog/readiness plus provenance
and timestamp, not authority. Persistence/resume declares whether it supports
native continuation or bounded transcript reconstruction.

Move Desktop concrete runtime storage, connection lifecycle coordination,
configuration snapshots, generic error/status mapping and operation ownership
behind this neutral seam. Keep Codex spawn/version/schema/RPC DTOs, provider
config encoding, native thread mapping, dynamic Tool aliases and transport in
rah-runtime-codex. Executable certification remains host policy implemented
through the adapter's bounded artifact requirements; it must not become a
generic model-supplied executable capability.

The interface must allow hosted APIs with no process, local runtimes without
Codex threads, and optional native conversation persistence. A future native
OpenAI adapter may implement ModelBackend under a RAH-owned agent/tool loop;
it is not necessarily the same architectural object as a complete Codex
AgentRuntime. Alternate hosted/local backends need the same neutral lifecycle,
tool-call continuation, error and cancellation semantics, not pretend
thread/start or modelProvider methods. Local inference-engine internals remain
outside RAH. No provider implementation is selected in this task.

## 14. Tool boundary

`rah-protocol/src/tools.rs` owns ToolName, ToolDefinition(name, description,
JSON input schema, permission), ToolCall/untrusted ToolInput, and
ToolOutput(Text/Json, is_error). These are already neutral and sufficient for
current text/JSON bridge semantics. Future adapter capability limits must be
reported explicitly if a provider cannot represent a schema/result; do not
weaken Tool validation or invent provider authority.

Codex `bridge.rs` owns inputSchema/dynamicTools, private aliases, correlation,
response contentItems/success, routing and replay/dedup handling. Authorization
and ToolRegistry dispatch remain the actual RAH path. Reusable orchestration
may move above the adapter only with behavior-preserving design; Tool protocol
translation stays adapter-local. The Generic Tool Bridge is unchanged.

## 15. Event and error boundaries

| Existing event / representation | Classification |
| --- | --- |
| Started, ModelRequestStarted, ModelDelta, Completed, Failed, Cancelled with RAH IDs | Generic enough; no Codex thread or provider wire identifier in AgentEvent. |
| ToolRequested, ToolStarted, ToolFinished, ApprovalRequired | Generic enough, provided only actual authorized execution emits execution events. Approval semantics must remain RAH-owned. |
| One text final AgentOutput; terminal stream; session-oriented cancel | Uncertain / later design: adequate today, but multi-turn operation identity and durable continuation need specification. Not a demonstrated Codex wire leak. |
| Adapter conversion of turn status, message-role flattening and model-request synthesis | Codex-specific translation detail; message role preservation must be reviewed for a native backend. |
| CodexAdapterError / private connection events | Correct adapter locality. Structured diagnostics are lost when crossing generic Runtime(message) and Failed(message). |
| Desktop status fields / FrontendError naming and Arc<CodexRuntime> lifecycle | Codex-specific leakage into application composition and presentation, not into AgentEvent itself. |

A narrow error improvement should preserve observed stage/kind/status while
retaining neutral public categories and a sanitized unknown result. Do not put
CodexErrorInfo enums in rah-protocol. A later design can define generic diagnostic
provenance with adapter-private detail and correlation IDs; Task 494 does not
define a new public error contract.

## 16. Session, thread and restart identity

ADR 0005 requires independent RAH SessionId and private Codex thread identity.
`runtime.rs:SessionRecord` implements this with thread_id/active_turn/tool
snapshots in a runtime-local map. start creates a new thread and a new RAH
SessionId after turn acceptance. resume looks up an existing local mapping,
calls thread/resume and returns a passive stream; it does not mean a generic
persisted session can continue after restarting the adapter. cancel maps that
session to the active turn and waits for interrupted termination.

Desktop persisted conversation resume is bounded text-context import into a
fresh current connection, not restoration of Codex native thread or repository
authority. `rah-session` neutral state is useful but not a durable provider
continuation design. There is no reason to expose Codex thread IDs upward.

Future design must distinguish RAH durable conversation/session identity,
individual operation identity, adapter-instance generation and optional opaque
provider continuation token. Bind continuation to adapter/provider/account and
supported semantics; never interpret it as authority. Define unavailable native
resume, reconnect and explicit transcript replay separately. Do not promise
exact native thread continuation across another provider or after process loss.

## 17. Security implications and ownership invariants

To update operational compatibility without arbitrary binary execution, a
trusted local host must explicitly install/select exact executable and sibling
identities backed by bounded certification. Verify bytes against the trusted
certification artifact before invocation, enforce platform/path/file checks,
and establish/revalidate running child identity where required. Unknown version,
digest or unsupported adapter contract fails closed. Provider/catalog/model
metadata cannot select paths, write admission records, promote versions or
trigger download/update. No silent self-update or unrestricted latest.

Do not mistake current self-reported version acceptance for that complete future
security answer. Protect artifact trust and actual execution binding in the
design, without claiming that today's manifest eliminates malicious local
replacement. Rollback is explicit policy, not automatic error recovery.

Always keep outside provider decisions: repository admission and active selection,
remembered workspace descriptive rules, repository generation/leases,
HostExplicit exactly 11, permission policy, Trusted Profile composition,
ToolRegistry authorization and ordinary Tool contracts, fail-closed executable
admission, and mutation uncertainty. Models/providers never choose repository
authority. Adapter replacement must preserve process supervision versus sandbox
distinctions and ADR 0015's initial-endpoint limits.

## 18. Three horizons and migration strategy

1. **Horizon 1 — immediate operational mitigation:** separately implement narrow
   diagnostic preservation; design an explicit bounded catalog/preflight flow
   with honest unknown states and ADR 0015 alignment. Today's R2 justifies a
   separate candidate certification task, not baseline promotion here.
2. **Horizon 2 — transitional lifecycle:** design multiple exact certified
   runtimes/preferred selection and possible operational certification artifacts.
   Preserve schema checks, live Windows Tool certification, rollback and immutable
   release evidence. Implement only after artifact trust/lifecycle design.
3. **Horizon 3 — final architecture:** design a neutral factory/lifetime/operation
   seam and optional discovery/continuation; incrementally replace Desktop's
   concrete Codex lifecycle, keep wire/process translation in the adapter, then
   prove Desktop can operate with no Codex installation using a separately
   authorized native/alternate adapter.

Suggested migration sequence: neutral lifecycle design; behavior-preserving
Desktop composition migration; session/operation/continuation semantics;
adapter contract conformance and shutdown/error controls; optional backend/tool
loop proof; user-selectable production alternative only after its certification.
Do not solve the final architecture by accumulating Codex policy in core.

Long-term Codex removal is feasible at the architecture level because neutral
protocol/model/Tool abstractions and independent session IDs already exist.
It is not a configuration switch today: production Desktop has concrete Codex
ownership and no complete native orchestration alternative. This research
understands the seam sufficiently for a separate design task, not to prescribe
an unreviewed API or claim implementation readiness for every provider.

## 19. Non-goals and validation

No Rust/frontend/permission/Tool/ADR/Cargo/version/CI changes, no baseline
promotion or trust-store insertion, no alternate adapter implementation, no
v0.34 capability selection, no WIP reconstruction, no release/tag. Only this
report changes inside the repository. Temporary evidence stays outside it.

Validation: `git diff --check` is the required documentation check. No workspace
test suite is required or claimed. Live controls and schema generation above
are research observations, not deterministic product or production certification.
Final check, commit, normal GitHub/mirror transport and exact-head CI results
are reported in the task closure response; this report cannot contain its own
future commit SHA or CI result.

## 20. Recommended separate tasks and classification

- **Transitional task:** preserve backend error stage/observed structured detail
  safely and design explicit selected-model preflight/catalog diagnostics,
  including ADR 0015 probe scope, stale/unknown results and no silent fallback.
- **Runtime abstraction design task:** define neutral runtime factory/lifetime,
  operation identities, optional catalog/preflight, shutdown/cancel and native
  continuation versus transcript replay; keep all authority RAH-owned.
- **Certification lifecycle design / bounded certification task:** evaluate
  exact multi-runtime artifact admission; separately certify a candidate with
  schema/deterministic/direct/Desktop neutral and Tool evidence before promotion.
- **Migration tasks:** incrementally move Desktop concrete lifecycle behind the
  reviewed neutral seam, preserving event/tool/security behavior.
- **Optional native/alternate adapter proof:** only after the abstraction exists;
  demonstrate no-Codex operation with host-authorized Tool dispatch.

**A — TRANSITIONAL COMPATIBILITY GATE IS JUSTIFIED; DECOUPLING RESEARCH IS SUFFICIENT FOR A SEPARATE DESIGN TASK**

The recurring lifecycle risk has historical and current controlled evidence.
Bounded error handling and an explicitly designed preflight are feasible, with
catalog/entitlement and CLI-age distinctions limited as above. The neutral seam
and hard continuation boundary are sufficiently identified for separate design.
This classification authorizes no automatic implementation or baseline upgrade.
