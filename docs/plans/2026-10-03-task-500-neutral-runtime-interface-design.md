# Task 500 — Neutral runtime interface design

## 1. Checkpoint, scope, and evidence

Research/design only. Starting design HEAD is exactly
`908639f2057c60e96a7493d6409042e5d90e5223`; fetched `origin/master` equals it.
Natural push CI `37075484510` was read through `gh run view`: success, exact
matching head. The supplied Task 499 production results remain checkpoint
evidence, not repeated Task 500 experiments: advertised gpt-6-astra connected
and completed a neutral turn; absent selection was stopped before publication,
with nine alternatives and zero inference thread starts; gpt-6.1-sol was absent
from that catalog observation.

The shared main checkout was clean at older `d5ef2e541d744f8070019878f6754d16a4b974a8`.
It is preserved. Research and publication use clean isolated worktree
`F:/temp/rah-task500`, branch `task-500-design`, based on the exact checkpoint.
Version 0.33.0, HostExplicit exactly 11, preferred/current certified Codex
0.157.1, v0.34 capability NONE SELECTED. No product, Cargo, ADR, permission,
version, tag, release, baseline, host configuration, or Task 499 changes.

Authoritative material inspected: README; ARCHITECTURE and
ARCHITECTURE_GUARDRAILS; SECURITY; ADRs 0001–0006 (runtime, adapter, Tool,
no-inference-engine, process and bridge boundaries), 0015 (host-selected
initial endpoint), 0027 (repository composition), 0030 (moving certification),
0032 (typed local failures); Task 494/498/499 reports; current source below.
Accepted ADRs remain unchanged. This is a proposed implementation design;
changing stable boundary candidates requires the guardrails' later ADR review.
The user explicitly authorizes architecture research here. The report does
not itself amend ADR 0006's public-contract restriction or ADR 0032.

Source references below are paths relative to the repository and symbols at
this checkpoint, rather than assumptions based on crate names. Internal mirror
lag/authentication is transport state, not architectural evidence.

## 2. Current runtime path and concrete boundaries

| Boundary | Observed call/type and ownership |
| --- | --- |
| Desktop connection | `crates/rah-desktop/src/main.rs::connect_codex` snapshots repository/model/profile/connection generations, prepares host composition and provider activation, then calls `resolve_prepare_and_connect_codex` |
| Executable preparation | `PreparedCodexConnection` contains executable, `CodexModelConfig`, `CodexExecutableSource`; `codex_baseline.rs::resolve` selects override, certified stored baseline, or PATH fallback; stored artifacts have manifest/hash checks |
| Construction | `CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(executable, registry, allowed_permissions, model_config, workspace)` receives active-only host registry and canonical selected root, or neutral workspace |
| Artifact/connection | `process.rs::ProcessTransport::start` resolves executable, runs version/schema checks, starts fixed `app-server --stdio`, owns child/stdin/stdout/stderr task; `connection.rs::AppServerConnection::initialize` creates actor, pending RPC replies, broadcast events and sole server-request receiver, then initialize/initialized handshake |
| Model preflight | `runtime.rs::preflight_selected_model` returns neutral `ModelPreflight` or `RuntimeFailure`; explicit OpenAI invokes private `catalog.rs::discover` (`model/list`), Inherit/other providers return NotChecked; Desktop gates publication and stores generation-scoped presentation/local failure |
| Ready publication | `PendingConnectedPublication` and `publish_connected_provider_state` revalidate host generations/currentness before publishing `Arc<CodexRuntime>`, provider activation, composition and commit capability; stale runtimes are shut down |
| Desktop request | `DesktopConversationState::request_messages` builds bounded completed text history plus prompt; `send_chat` creates `AgentRequest { request_id, input: AgentInput { messages }, options }`; `run_chat(Arc<CodexRuntime>, …)` calls `AgentRuntime::start` |
| Codex operation | `runtime.rs::start` snapshots/aliases Tools, sends restricted `thread/start`, verifies effective model/provider/cwd, then `turn/start`; creates fresh RAH `SessionId`, private `SessionRecord { thread_id, active_turn, bridge_tools, bridge_aliases }` |
| Streaming | `AgentHandle::into_runtime_events` returns owned local `RuntimeEventStream`; Codex maps agent text delta, terminal turn and connection faults; Desktop checks active chat/runtime/session/generation before presentation and completed-history commit |
| Tool bridge | `bridge.rs::run_bridge` consumes sole server requests; validates private thread/turn/call routes and aliases, constructs fresh RAH `ToolCallId`, checks `authorize_tool_dispatch` and later `authorized_tool_dispatch`, emits actual RAH lifecycle, responds through private RPC correlation |
| Cancel/recovery | Desktop `cancel_chat` invokes session cancel and bounded recovery; adapter sends `turn/interrupt`, waits for matching interrupted terminal notification; hard recovery calls concrete `shutdown` rather than claiming cancel success |
| Disconnect/exit | `disconnect_codex`, rejected/stale publication, exit cleanup and hard recovery call concrete shutdown; connection actor owns transport task; transport kills owned child where needed, bridge task is joined; unexpected exit carries status/stderr as private typed source |

`rah-runtime/src/lib.rs::AgentRuntime` is an async-trait, Send + Sync,
object-safe boundary with only `start(AgentRequest)`, `resume(SessionId)`,
`cancel(SessionId)`. `AgentHandle` owns session ID and local event stream.
Creation, shutdown, discovery and configuration are outside it. Desktop stores
concrete Arcs even though start/cancel use the trait.

Session orchestration is not a hidden shared session engine:
`rah-session::{Session, AgentContext, SessionStatus, SessionStore}` provides
neutral storage; current Desktop/Codex path does not compose that store to
restore a thread. `rah-core` does not own this operational loop.
`rah-runtime::MinimalTestRuntime` runs a neutral `ModelBackend`/Tool loop;
`rah-cli` composes it, while production Desktop composes Codex. `ModelBackend`
is one completion operation, not a replacement for runtime lifecycle.

Codex `resume(SessionId)` only finds its in-memory map, sends `thread/resume`,
and returns `passive_stream`. That stream ignores additive thread notifications
and does not submit a new user turn. Desktop does not call it for transcript
resume. Each Desktop chat currently starts a fresh Codex thread with full
completed text. `resume_previous_conversation` imports bounded completed
user/assistant pairs after fresh current connection checks and lineage commit;
it restores neither native IDs nor Tools/authority. Disconnect/reconnect does
not make those IDs durable continuation handles.

## 3. Codex coupling inventory

Legend: **R** = RAH-owned semantic; **C** = Codex-specific implementation detail;
**M** = mixed/misplaced composition boundary. M means a migration seam, not
evidence that existing authorization is bypassed.

| Significant coupling | Class | Future placement |
| --- | --- | --- |
| Host chooses configured adapter, immutable selection and generations | R | Desktop host |
| `PreparedCodexConnection`, concrete runtime constructors in main | M | adapter-specific configured factory behind neutral construction |
| Executable resolution, baseline names, sibling binary, manifest/hash checks | C/M | Codex artifact component; host still explicitly selects artifact |
| Fixed spawn args, stdio JSON-RPC, version/schema fixtures and initialize | C | rah-runtime-codex |
| Explicit model/default selection intent and mismatch rejection | R | neutral selection and selected-context verification |
| `CodexModelConfig::Inherit`, `modelProvider`, provider aliases/presets/config overrides | C | Codex configuration translator |
| DesktopModelSelection converts to concrete Codex provider/model types | M | host chooses configured adapter; Codex plugin composition does translation |
| `thread/start`, `thread/resume`, `turn/start`, `turn/interrupt` | C | adapter-private operations |
| Request/session correlation, accepted turn, completion/failure, bounded text input | R | neutral operation contract |
| One new session per start while Desktop conversation spans many starts | M | explicit conversation versus operation identity |
| Text deltas/final assistant output/terminal failure | R | RuntimeEvent/AgentEvent semantics |
| Codex notification names, terminal statuses, thread/turn route checks | C | protocol translator |
| ToolDefinition/ToolCall/ToolOutput, permissions, actual dispatch | R | existing RAH Tool path; scoped request port |
| Dynamic tool DTO, alias constraints, RPC request ID, thread/turn/call dedupe | C | adapter transport; no generic RPC identity |
| Bridge currently holds registry/permission list and invokes shared dispatch | M | neutral host port hides registry and permissions from future adapters |
| Provider Tool notifications versus actual execution lifecycle | R/C | translator rejects unsupported activity; host produces execution events |
| Alive/disconnected, owned work, failure fanout, bounded stop | R | runtime lifecycle guarantees |
| Child PID/kill/reap/stderr and code-mode sibling identity | C | local/Codex artifact and transport implementation |
| `model/list` params, nextCursor, selector field, includeHidden, 5s/100/256 limits | C | Codex discovery implementation |
| Scoped catalog advertisement, absence/unknown, pre-ready host gate | R | neutral discovery/preflight |
| Desktop concrete preflight call and Codex-labelled catalog guidance | M | generic lifecycle + adapter discovery |
| RuntimeDiagnostic + process-local RuntimeFailure + Error::source | R | unchanged Task 498 boundary |
| CodexAdapterError and conversion to neutral diagnostic | C | adapter edge |
| Desktop exhaustively matches CodexAdapterError for generic lifecycle | M | generic envelope mapping; optional adapter detail panel |

## 4. RAH-owned invariants

Repository admission, active selection, leases/currentness, remembered
workspace authority, switching, ToolRegistry authorization, permissions,
Trusted Profile composition, HostExplicit, mutation uncertainty and
staging/reviewed Commit remain entirely RAH-owned. Provider IDs and metadata
cannot compose, restore or select them. No generic shell/process/credential
Tool is introduced. HostExplicit stays 11. Tool contracts and separate
observation/worktree/index/history authorities remain intact.

The host composes one active-only registry before connecting. Runtime readiness
does not grant authority; every Tool request is untrusted, checked before
admission and immediately before actual execution through the existing dispatch
path. Generation binding and started/uncertain effect ownership survive runtime
failure. No timeout, reconnect, cancel, or resume causes effect replay.
ADR 0015 endpoint selection stays host-owned and bounded; adapter choice cannot
silently widen endpoint policy or introduce automatic provider fallback.

## 5. Minimum neutral semantics and preferred split

Choose **C: configured factory + live runtime + conversation**, returning an
owned turn handle/event stream. It fills demonstrable gaps without a separate
turn trait, generic JSON channel, or new crate. Names below are conceptual,
not a prematurely stabilized API.

1. Host builds a configured factory using adapter-specific validated settings.
   Factory validates static config and creates live instance; successful create
   means transport/config readiness, not model entitlement or inference success.
2. Live instance reports a closed capability set, lifecycle state/notifications,
   optional scoped model catalog, opens/restores a conversation, and shuts down.
3. Conversation owns immutable model context, one scoped Tool request port and
   private continuation state. It accepts a turn, exposes owned stream/control,
   and closes. One active turn per conversation initially; no parallel-turn
   feature or provider feature bag.
4. Turn handle owns one fresh RAH operation SessionId, request correlation,
   event stream and cancellation control. SessionId remains the existing
   operation/session ID; do not reinterpret old stored session IDs as durable
   conversation IDs. Separate RAH conversation ID and runtime-instance identity
   are process-local routing/currentness values, not authority.

Factory captures executable/auth/provider-specific config privately; it never
receives repository membership, leases, permissions or registry. For Codex,
host supplies only the already selected canonical working-context path needed
for effective thread cwd verification in a Codex-specific configuration helper,
plus connection binding identity. That path is an operational context, not an
authorization token or generic runtime field. It must match the host's scoped
port composition; neutral providers need no path. Do not let the factory resolve
or choose another repository. Preserve current cwd verification, not global CWD.

Capabilities are advertised by shipped adapter code after validating configured
transport; provider metadata can only narrow available behavior, never raise
authority. Factory descriptor identifies adapter and static configuration needs;
live capability report is authoritative for this connection. Factory does not
certify arbitrary binaries or perform an inference probe.

## 6. Model/provider contract and Task 499 placement

Selection is `Explicit(ModelId)` or `RuntimeDefault`. ModelId is a bounded,
opaque selector scoped to the configured adapter/provider context, not a global
name or catalog row ID. Existing Codex Inherit translates to RuntimeDefault
inside its adapter while continuing to inherit host Codex config. A native
adapter can resolve a host-configured default or reject missing default with
InvalidConfiguration; no requirement to imitate Codex Inherit. No silent
replacement of explicit selectors. Effective context mismatch fails closed
where observable; inability to observe a resolved default is reported as unknown.

Provider selection is configuration of a configured adapter instance, not a
mandatory generic `modelProvider` enum or per-turn field. Host may offer several
configured factories (including several Codex providers); each freezes one
transport/provider context. Scoped descriptor distinguishes them. Catalog
labels may describe provider scope but cannot switch endpoint/account/adapter.
Codex presets remain adapter-specific helpers under existing host endpoint
rules. Arbitrary custom provider support in the Codex Rust API does not imply
new Desktop endpoint authority.

Proposed catalog: `{ scope: CatalogScope, completeness: Complete | Partial,
models: Vec<ModelDescriptor> }`; descriptor is `{ id: ModelId,
display_label: Option<BoundedText> }`. Scope is the host/configured runtime
context identity, not an untrusted provider endpoint. Normalize selectors and
validate/deduplicate within bounds; labels are optional inert text. No capability
metadata initially: current preflight only needs membership and alternatives.
Add a closed metadata field later only for a demonstrated RAH behavior.

Discovery support is `Unavailable` or `Available { coverage }`. A partial or
unrelated catalog cannot prove absence. Neutral preflight produces Advertised,
NotAdvertised (complete same-scope only), or NotChecked(reason: default,
unsupported, outside coverage, partial). Discovery failure remains a typed
RuntimeFailure, never absence. Live runtime owns catalog I/O; host owns gate.
For current Codex explicit OpenAI preserve Task 499's complete bounded catalog
requirement and stop on error/incomplete result; other providers/default remain
NotChecked. Capability absence is visible unknown, not fabricated compatibility.

Migration: generic connection -> initialized adapter -> discover if applicable
-> neutral preflight -> host generation checks -> ready publication. No
conversation/turn before the gate, no refresh/update/fallback in this task.
Keep today's ModelCatalog/ModelPreflight APIs during additive migration; convert
them at the Codex edge before introducing richer internal descriptors. Model
advertisement remains separate from entitlement, successful inference, adapter
certification and permanent availability.

## 7. Conversation, continuation, and resume

RAH logical conversation spans completed turns; RAH SessionId denotes one
accepted operation; provider conversation/thread/response IDs are adapter-private;
process/connection lifetime is independent of all three. A conversation object
need not allocate a remote resource until send. Stateless adapters can implement
it as validated immutable context plus routing and replay strategy.

Initial mode is **host text replay**: every send receives a complete bounded
AgentRequest text snapshot. Adapter must consume that snapshot exactly once,
not silently append it to an already populated native conversation. Codex can
continue making a new restricted thread per send, preserving current behavior.
Stateless APIs send the snapshot directly. A provider that only accepts native
conversation IDs can allocate fresh native state and seed the snapshot. Do not
feed duplicate full history into an existing native thread. Only completed
user/assistant text is Desktop replay data; it cannot stand in for outstanding
Tool calls or a faithful provider-native tool transcript.

Native continuation is a separate optional open/restore mode, never inferred
from transcript persistence. Capability describes `None`, `LiveInstanceOnly`,
or `Reconnectable` plus continuation mechanism `ProviderState` or
`AdapterReplay`. Restore takes an opaque in-process continuation handle with
private contents bound to adapter/config/account scope, conversation identity,
and completed checkpoint. It is not a raw String supplied by frontend/model.
In continuation mode sends carry new input rather than full history. Caller
chooses mode explicitly; types distinguish replay snapshot from continuation
input and reject mixed use. No generic persisted token schema in phase one.

| Situation | Semantics |
| --- | --- |
| Server-side conversation ID | private ID/checkpoint in handle; verify current scope; no implicit restore of Tools or approvals |
| Stateless API | full snapshot for replay; optional adapter replay checkpoint only if complete semantic history is retained, otherwise no native restore |
| Unsupported resume | explicit unsupported result before provider call; host may separately offer completed-text import |
| Resumable local process | LiveInstanceOnly unless separately proven reconnectable; process restart invalidates live handles |
| Resumable remote state | Reconnectable only with proven provider/account scope and checkpoint validation; transport reconnect is not operation retry |
| In-flight operation lost | terminal failure/uncertain effects; never restore by replaying pending Tool calls |

No initial Codex native-resume capability is claimed from its passive
AgentRuntime::resume implementation. Keep legacy trait behavior compatible;
only advertise new continuation after tests prove new-input submission and
scope semantics. Desktop Resume Previous Conversation remains explicit completed
text import under fresh current authority. Changing repository/model/profile
composition invalidates conversation binding and creates fresh context; reconnect
rebinds a fresh scoped port even if text remains available. Native restoration
cannot bypass host generation checks or restore old registry/leases.

## 8. Closed capability set

Only four operational dimensions initially:

| Capability | Host behavior affected |
| --- | --- |
| Catalog availability/coverage | preflight, alternatives, honest unknown |
| Native continuation support/scope/mechanism | restore availability, token validation, replay distinction |
| RAH Tool-call support | whether nonempty Tool snapshot can be offered; reject unsupported composition, never silently omit Tools |
| Cancellation mechanism: Unsupported / LocalStop / RemoteRequest / ProcessInterrupt | cancel presentation and bounded recovery; result reports observed assurance separately |

The semantic event stream always exists; a provider with no incremental text
emits final output through it, so no streaming flag is needed for current RAH
behavior. RuntimeDefault configuration is validated at creation, not a generic
provider-selection capability. No audio, images, arbitrary metadata or per-RPC
feature toggles. Capabilities affect availability, never authorize execution.

## 9. Neutral Tool bridge and call identity

Host creates a narrow per-conversation/per-turn request port whose private
implementation retains registry, expected definition snapshot, permissions and
generation/currentness binding. Port exposes read-only neutral Tool definitions
and `request(name, parsed_input)` returning a correlated neutral reply or typed
failure. Adapter receives neither registry mutation APIs nor permission setters,
repository selectors, HostExplicit tickets, or authority constructors. Factory
receives no port; conversation receives the host-bound port; port creates one
single-use active turn scope at admission and revokes it on terminal/close.

Host port allocates RAH ToolCallId and emits ToolRequested/Started/Finished only
through actual existing authorization/execution path. Adapter privately maps
provider call keys to that ID/reply. It translates definition formats/aliases,
accumulates and bounds streamed arguments before submission, delivers results
to provider and owns native correlation/dedupe. It cannot register arbitrary
Tools. Unknown name/changed definition/missing permission/stale turn fails closed.
Multiple distinct calls get distinct IDs; neither JSON-RPC ID nor provider call
ID becomes a RAH ID. Identical repeated native request joins/reuses the original
result only within the adapter's bounded correlation record; conflicting replay
fails, and lost/uncertain result never starts another dispatch.

Port invocation future must not own the lifetime of started effects alone.
RAH dispatch owner retains execution/reconciliation when adapter drops its
waiter. Cancellation revokes new admissions; started work retains uncertainty
and applicable mutation ownership until existing host rules release it.
Host merges ordered execution events into the turn stream and fences terminal
publication against admitted Tool work. No adapter-supplied ToolStarted or
ToolFinished event is accepted as execution evidence. Initial bridge migration
wraps current dispatch checks and dedupe behavior, not a rewritten authority
engine. ADR 0006 remains authoritative until separately reviewed changes.

## 10. Event boundary

Audit of every current AgentEvent variant:

| Variant | Class and contract |
| --- | --- |
| Started | neutral; accepted operation + RequestId, not durable conversation creation |
| ModelRequestStarted | neutral but ambiguous granularity; may be one logical model step rather than provider RPC; native Tool loops may have several |
| ModelDelta | neutral text; ordered within ModelRequestId, not provider item ID |
| ToolRequested | neutral untrusted request; host port producer for normalized admitted request |
| ToolStarted / ToolFinished | neutral RAH dispatch lifecycle, host-only producer |
| ApprovalRequired | neutral host policy concept; current Codex approvals are denied, not mapped here; new interactive support requires separate design |
| Completed | neutral final assistant message and successful turn terminal |
| Failed | neutral terminal code; arbitrary legacy message is unsafe for adapter failures—use RuntimeEvent::failed |
| Cancelled | neutral local operation terminal; current Codex producer requires interrupted notification; generic event alone cannot promise server stop |

No variant is intrinsically a Codex DTO. Ambiguity is operation scope and producer
trust, not names. Existing text-only final AgentOutput is enough for current
product behavior. No separate completed-assistant-message event is required:
Completed carries canonical final output, which may differ from concatenated
preview deltas (buffered providers can emit only final output). Tool round trips
may include several model steps; no one-RPC-per-turn assumption. No usage event
exists in AgentEvent; ModelBackend's ModelEvent::Usage is currently a lower-layer
concept. Do not promote usage/provider metadata without a real Desktop need.

Owned per-turn stream yields RuntimeEvent, at most one terminal event followed
by EOF. EOF without terminal, malformed route/identity, lost relevant events
or overflow fails the operation; no silent dropped Tool events. Adapter validates
wire data; host stream supervisor validates IDs/order/producer role and merges
host Tool lifecycle. Terminal completion waits for Tool lifecycle reconciliation;
uncertain host effects can outlive failed/cancelled stream under host ownership.
Runtime lifecycle notifications are separate Ready/Disconnected/Failed state
changes, including failure when idle; generic code never consumes child PIDs.

## 11. Error boundary

Preserve ADR 0032 exactly: sanitized RuntimeDiagnostic plus optional process-local
`Arc<dyn Error + Send + Sync + 'static>`, source traversal/downcasting locally,
shared allocation on clone, no serde/equality for source-bearing values.
Factory, catalog, open/restore/send/cancel/close/shutdown errors use RuntimeFailure
or a neutral operation error containing it; stream failures use RuntimeEvent.
Existing AgentError::Failure and AgentHandle projections remain compatible.
Generic host branches on closed neutral diagnostics; concrete error conversions
and detailed recovery stay adapter-local. No error-to-string erasure, raw provider
message, auth response body or stderr enters frontend IPC. rpc_code stays optional
and absent for non-RPC providers; do not overload it with HTTP status.

Unsupported capability, busy/stale handle, invalid input and already-terminal
are typed neutral operation outcomes rather than guessed provider failures.
New local operation types can map to existing SessionStart/SessionResume/Turn/
Cancellation/Shutdown diagnostics initially; any finer diagnostic expansion
needs a later explicit reviewed change. Missing source is legitimate when no
underlying error exists; retain a real provider source when present.

## 12. Cancellation, close, disconnect, and shutdown

Cancel receipt distinguishes request mechanism from observed stop:
`Unsupported`, `AlreadyTerminal`, or `Stopped { assurance: LocalOnly |
ProviderConfirmed }`. An RPC acknowledgement is not confirmation. Before
Stopped the local stream producer is quiesced and new Tool admission revoked;
no later successful completion for that accepted cancellation. Provider may
continue computing in LocalOnly case. If completion won the serialized terminal
race, return AlreadyTerminal and keep completion. Failed/timeout cancellation
retains typed failure and host ownership; no fabricated Cancelled.

Cancelled event means local turn has stopped publishing/accepting requests;
host stores/presents the receipt's assurance separately. Existing Codex adapter
can preserve ProviderConfirmed only on matching interrupted turn notification.
RemoteRequest capability alone never promises it. Stream drop initiates local
revocation and best-effort adapter interruption, not confirmed remote cancellation
or rollback. Drop has no async success guarantee; owned supervisor performs cleanup.

Conversation close rejects new sends, revokes its ports, and awaits/cancels
owned turn cleanup. It releases local resources; remote deletion is not implied.
Runtime shutdown is idempotent, rejects new conversations, closes local streams,
joins owned tasks and disconnects owned transport. Adapter implements bounded
escalation/reaping for a child or closes HTTP resources for a native provider.
Host timeout is not successful shutdown; retain non-quiescent/uncertain owners.
No generic kill-process API. Process kill/reap is Codex/local implementation;
it does not undo repository or external provider effects. Desktop Tool-provider
activation lifecycle remains separate from model runtime lifecycle.

## 13. Authentication and certification

Provider authentication belongs to trusted adapter configuration and its private
transport. Generic factory sees configured factory identity/readiness and a
sanitized config/auth failure; Tool calls and events carry no credentials. This
design adds no credential transport/UI/store. Current Codex config environment
references stay private, and Desktop endpoint rules remain authoritative.

Three distinct trust/evidence layers:

1. Shipped Rust adapter implementation is trusted host code, reviewed/tested under
   RAH invariants; no claim it is sandboxed from the host.
2. External executable artifact trust is required only for adapters that launch
   one. Codex version admission, schema checks, SHA manifests, saved baseline store
   and sibling artifact verification live in rah-runtime-codex or a Codex-specific
   artifact component. Host selection remains explicit; no generic baseline store.
3. Provider/model availability is dynamic operational observation, not artifact
   certification or authorization. Catalog does not certify an executable.

ADR 0030 exact version set/preferred baseline/history separation remains intact.
Artifact file-object identity, bytes/hash and process ownership are distinct
evidence. Native HTTP adapter has no invented executable/version/PID requirement.
Codex-specific source/version display can remain optional adapter details, not
prerequisites for generic readiness. No new artifact admission mechanism here.

## 14. Alternatives A–D

| Design | Complexity/testability | Migration/alternate support | Object safety/lifecycle |
| --- | --- | --- | --- |
| A: large AgentRuntime | few objects but start/resume/cancel/create/catalog/close/shutdown responsibilities accumulate; existing conformance retained | quickest initial method addition, but global conversation IDs/maps and provider lifetime leak into operation API | object-safe with erased async types; creation cannot be a heterogeneous instance constructor without separate composition; weaker ownership clarity |
| B: factory + runtime | good factory/lifecycle tests; fewer traits | sufficient for initial Desktop construction; full conversation semantics must use handles/maps on instance | object-safe; can be a transitional slice of C, but native continuation versus text replay remains implicit if only existing SessionId is used |
| C: factory + runtime + conversation | one extra boundary; separately test configuration, lifecycle, context and turns | matches actual Desktop multi-turn text owner and independent process/session IDs; Codex can preserve replay while native/stateful adapters supply different internals | object-safe erased futures/streams, concrete turn handle; strongest context/port/lifetime scope |
| D: command/event facade | central state machine plus correlation/backpressure; tests require protocol sequences | can implement any adapter but risks second generic RPC protocol and ambiguous ownership of commands/results | object-safe channels; async ownership moves to actor, command grammar still needs all semantic contracts; drop/terminal races harder to expose |

C is preferred because current Desktop has separate completed-history and active
operation lifetimes, while Codex connection is already independent and native
resume is not Desktop resume. B is an implementation stage, not a competing final
design. A cannot remove direct lifecycle/config coupling merely by replacing Arc
types; D adds another transport-shaped contract without simplifying real needs.
No evidence of required flag-day session rewrite; classification C (already
sufficient) and D (larger rework) are therefore unsupported.

## 15. Rust object safety, ownership, and visibility

Illustrative signatures, not code to implement in Task 500:

```rust,ignore
#[async_trait]
trait RuntimeAdapterFactory: Send + Sync {
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure>;
}
#[async_trait]
trait RuntimeInstance: Send + Sync {
    fn capabilities(&self) -> RuntimeCapabilities;
    fn lifecycle(&self) -> RuntimeLifecycleReceiver;
    async fn models(&self) -> Result<CatalogObservation, RuntimeFailure>;
    async fn open(&self, seed: ConversationSeed)
        -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure>;
    async fn shutdown(&self) -> Result<(), RuntimeFailure>;
}
#[async_trait]
trait RuntimeConversation: Send + Sync {
    async fn send(&self, input: TurnInput) -> Result<RuntimeTurnHandle, RuntimeFailure>;
    async fn close(&self) -> Result<(), RuntimeFailure>;
}
```

ConversationSeed is a closed FreshReplay or RestoreContinuation form with host
binding, selected model and scoped port. Restore rejects capability absence
before I/O. RuntimeTurnHandle is concrete: existing-compatible AgentHandle/local
event stream plus `Arc<dyn TurnControl>` for async cancel; host consumes stream
once while retaining control. No generic methods, associated types, return Self,
or provider-specific error type. async-trait boxes Send futures; native async
trait methods alone do not supply this dynamic-dispatch shape. Heterogeneous
configured factories and instances can be stored behind Arc dyn objects.

Requests, seeds and tokens are owned; streams are
`Pin<Box<dyn Stream<Item = RuntimeEvent> + Send + 'static>>`, not Sync; borrow
no stack request/factory. Instance/conversation/control are Send + Sync with
short state locks; do not hold synchronous mutex across await. Runtime owns
supervisor/tasks/transport; conversation holds shared runtime state, supervisor
holds weak routing or explicit removable entries to avoid Arc cycles. Turn
guard/control owns scoped state and cleanup registration. Only one consumer
owns a stream; cloneable diagnostics do not imply cloneable event subscription.
Shutdown is serialized/idempotent across Arcs, and returned runtime cannot be
used after shutdown even if a conversation Arc remains. Creation cancellation
and open/send failure must clean partially allocated resources through owned
guards; accepted external effects are never retried automatically.

RAH host reserves turn/currentness before send; published handle IDs are fresh
RAH values and validated against reservation. Compatibility wrapper can keep
legacy AgentRuntime allocation/signatures while new host-owned routing is added.
Port captures host binding; adapter cannot mint an active request scope just by
guessing IDs. Stream merging and terminal fencing have one supervisor owner.

## 16. Native OpenAI thought experiment

This is a hypothetical implementation test of the semantic design, not a claim
about a current API/version, endpoint entitlement or SDK. A configured HTTP
factory privately holds auth/default settings and returns live transport context
without local executable. Catalog translates available selectors to the scoped
neutral shape, or reports coverage limitations. Explicit/default resolves or
fails configuration; streaming translates text and canonical final output;
function requests call the host port and results are translated back. It owns
the multi-step agent loop if the provider API only performs one model completion.
No provider-side executable Tools are enabled.

Conversation may send full text snapshot on each turn. If provider has resumable
state, optional private continuation handles express it; otherwise native
resume is unavailable while Desktop text import works. Cancellation is LocalStop
unless actual remote confirmation is available; HTTP/source errors convert to
RuntimeFailure with rpc_code absent. Shutdown closes transport/owned tasks.
Every operation fits without codex-cli, threads, RPC IDs, version/schema fixtures
or executable certification. Result: provider-neutral at design level; future
real adapter needs separately authorized implementation and production proof.

## 17. Local runtime thought experiment

A configured factory may own a fixed approved local process without loading
weights or implementing inference inside RAH. Catalog can be Unavailable;
explicit selector is validated as configuration and preflight remains NotChecked.
RuntimeDefault requires a real configured default. No native resume is assumed;
fresh full-text replay works if input context is supported. Buffered-only output
still produces final Completed through the stream. Tool capability absence
rejects nonempty Tool composition or requires an explicit host text-only mode;
it never silently discards Tools. Different tool syntax stays adapter-local.
Cancellation may be process interruption or LocalStop; shutdown joins/reaps
owned child. Generic Desktop consumes lifecycle state, not PID assumptions.
Result: capability absence is cleanly representable; unsupported mandatory
behavior fails before operation rather than claiming a universal adapter.

## 18. Direct Desktop Codex dependencies and migration backlog

This inventories significant direct dependencies, including build/frontend
contracts; test occurrences are migration evidence, not additional runtime seams.

| Current location/symbol | Classification | Concrete backlog |
| --- | --- | --- |
| Cargo.toml Windows `rah-runtime-codex` dependency; main imports at 79–81 | temporary transitional dependency | feature-gated composition only; neutral operational modules import rah-runtime |
| ConnectionState (248), PendingConnectedPublication (2122), RejectedProviderPublication (2138) | must move behind interface | store Arc dyn RuntimeInstance and neutral identity; keep all host generation/composition checks |
| ActiveChat (1086), register_chat_session/active_chat/request_cancel/claim_terminal/is_current_chat/begin_hard_recovery, run_chat (9197) | must move behind interface | neutral instance identity + owned turn control/session; replace concrete Arc pointer comparisons coherently |
| DesktopModelSelection::codex_model_config (1721), set-model validation (5076) | must move behind interface | adapter-config composition helper; explicit/default generic selection; preserve endpoint validation/generation semantics |
| PreparedCodexConnection (8033), prepare/connect/resolve helpers, connect_codex (8274) | must move behind interface | configured factory and capability/preflight staging; no moved repository authority |
| Concrete preflight at 8505; model_preflight.rs concrete error/test factory fixtures | must move behind interface | neutral catalog/coverage gate; replace fixture coupling with fake factory without reopening Task 499 |
| disconnect_codex (8689), exit shutdown, rejected/stale shutdown, cancel_chat hard recovery (9676) | must move behind interface | neutral shutdown/cancel outcomes; preserve uncertain owner retention and Tool-provider cleanup |
| frontend_error (1948) matches CodexAdapterError | must move behind interface | generic diagnostic mapping; adapter-local conversion; keep optional sanitized Codex detail codes in Codex integration |
| codex_baseline.rs: resolver/store/version/hash/sibling/source enum | may remain adapter-specific helper | move to Codex artifact component, host selection wrapper stays feature-gated; generic connection never calls it |
| NEUTRAL_WORKSPACE_DIRECTORY, creation/cleanup and cwd wiring | may remain adapter-specific helper | Codex working-context preparation only; HTTP provider creates no fictitious directory |
| DesktopStatus codex_status/version/source/error, CodexExecutableSourcePresentation, connection_presentation, preferred version display | temporary transitional dependency | generic lifecycle/adapter identity; optional artifact details; compatibility serialization transition |
| effective_authority.rs source_label/runtime_source and literal runtime_kind "codex" | must move behind interface | neutral runtime descriptor projection; do not accept adapter metadata as effective authority |
| Tauri connect_codex/disconnect_codex handler registration, build.rs manifest, capabilities/default.json, autogenerated permission TOMLs | temporary transitional dependency | compatible aliases first, deliberate coordinated generic rename later; preserve inventory parity and no new authority |
| frontend/index.html and status.js Codex labels/commands/status keys/Inherit/provider explanations/catalog guidance; repository_membership_test.js | temporary transitional dependency | generic connect/model rendering plus adapter details; same backend host authority and stale-state behavior |
| Main/codex_baseline/effective_authority/model_preflight tests and live evidence labels | temporary transitional dependency | retain exact Codex regression fixtures; add generic fake tests and later migrate production evidence schema deliberately |

No direct Desktop Codex wire `model/list` implementation was found: it calls a
concrete method whose return types are already neutral. The backlog is to erase
concrete discovery access and context assumptions, not relocate DTOs into core.
No Codex Rust crates are linked today; mandatory dependency means production
composition/external executable, not imported upstream codex-core.

## 19. Crate/API implications

Existing rah-runtime is sufficient for neutral interface, lifecycle/port wrapper
and conformance tests: it already depends on protocol, model and tools.
rah-runtime-codex retains wire/process/artifact translation. No new neutral crate
or reversed dependency is justified. rah-protocol remains dependency-bottom and
business-light; only necessary neutral value types belong there, not factories,
provider configs, supervisors or typed sources. rah-session remains storage,
not authority restoration or source-error persistence.

Start with internal interface module; expose only experimental workspace-public
traits/types needed across Desktop/Codex crates (pub(crate) cannot span crates).
No stable external API promise. Keep AgentRuntime/AgentHandle and existing
conformance intact with compatibility adapters. The current API is insufficient
for generic construction/shutdown/discovery and distinct continuation input;
those are RAH lifecycle gaps, not mirroring Codex RPCs. Additive interface is
smaller/safer than changing legacy resume semantics in place. Later ADR review
must authorize new experimental boundary, port producer ownership and any stable
interface changes before promotion. Task 500 changes no dependency/API/ADR.

## 20. Incremental migration and safety gates

| Phase | Bounded deliverable and gate |
| --- | --- |
| 1 | Review required ADR proposal; introduce experimental neutral factory/runtime/conversation/turn-control value contracts alongside legacy APIs; fake adapters and contract tests; no production switch |
| 2 | Codex implements factory/live lifecycle and wraps existing error/discovery path; artifact/config helpers isolated; exact version/schema and existing regression behavior retained |
| 3 | Desktop constructs generic instance, stages neutral discovery/preflight before current host publication; preserve Task 499 present/absent/default/non-OpenAI behavior and stale-generation cleanup |
| 4 | Move chat turn/conversation ownership behind interface in replay mode; wrap dispatch in scoped host port, preserve aliases/dedupe/late authorization/uncertainty; legacy trait remains compatible |
| 5 | Move status/error/shutdown/recovery to neutral paths and optional Codex details; migrate frontend/IPC inventory coherently; remove concrete types from operational modules |
| 6 | Deterministic non-process adapter proof, including Tool loop/cancellation/capability absence; build without Codex feature; separately implement/certify an authorized production alternate adapter |
| 7 | Make Codex composition/dependency optional, test both feature configurations; production Desktop operates with alternate and no codex-cli installed; publish only if separately authorized |

Each phase retains existing deterministic tests and source/error projection,
HostExplicit 11, all currentness/authority rules, production behavior and separate
provider activation lifetime. Required milestone/production gates run at those
future milestones, not claimed by this documentation check. No passing focused
suite substitutes for required Windows live Tool and lifecycle evidence. Native
resume implementation is later opt-in work, not prerequisite for replay parity.
No automatic implementation starts from this report.

## 21. Deterministic testing strategy

Before any alternate production adapter, use configurable fake factories with
no process/network/credentials. Shared conformance verifies:

- creation validation/failure/cancellation cleanup; heterogeneous dyn storage;
  lifecycle idle disconnect and typed failure fanout;
- same-scope complete/partial/unsupported catalog, bounded labels/selectors,
  explicit/default and missing default, absent-model no conversation/inference,
  stale generation cannot publish readiness;
- replay conversation opening, full snapshot exactly once across two turns,
  distinct conversation/SessionId/provider IDs, wrong mode/stale restore rejection;
  live/reconnectable/unsupported continuation and completed-text import without
  restoration of Tools, permissions or pending calls;
- buffered and streamed text, canonical final message, ordered model steps,
  exactly one terminal, malformed/wrong-route events and EOF/overflow failure;
- Tool definition translation and parallel distinct call correlation; unknown
  names, malicious arguments, permissions and late currentness denial;
  duplicate/conflicting provider IDs cannot repeat dispatch; real host-only
  lifecycle and uncertainty on dropped waiter/lost response;
- cancellation race with completion, unsupported/local-only/confirmed cases,
  no post-cancel admission/completion, lost cancel response, stream-drop cleanup;
- source downcast after clone/fanout/stream, sanitized serialization and formatting,
  no source carried by transcript/store or frontend projection;
- close versus cancel versus shutdown, idempotent stop, partial-create failure,
  all tasks owned, no Arc cycle, non-quiescent owner retained on timeout;
- capability absence changes host availability honestly and rejects required
  unsupported composition without enabling fallback or provider-owned execution.

Keep Codex fake-peer suites, schema/version/bridge tests, Desktop fixtures,
permission parity and HostExplicit inventory. Non-process fake proof tests the
abstraction; separately authorized live alternate proof tests a real adapter.

## 22. Security assessment

**Can a provider/runtime adapter gain repository or Tool authority through this
interface? Structurally no.** Only the trusted host composes authority and creates
the scoped request port; interface supplies inert definitions and an untrusted
request path into existing dispatch. It has no selector, registry registration,
permission escalation, ticket import or host-authority constructor. A guessed
conversation/call identifier cannot mint the private active scope.

Malicious catalog labels/IDs are bounded, escaped inert data and never executable
paths, provider endpoints or fallback instructions. Tool parameters are parsed/
bounded and remain untrusted; existing Tool validators and sandbox/repository
checks remain authoritative. Malformed events/IDs/order/duplicate terminal fail
closed; provider-issued identifiers remain private correlation, not host identity.
Adapter-owned notification of a Tool execution is not accepted as proof of host
execution. Runtime compromise may submit requests, disconnect, or falsify model
text, but cannot bypass host dispatch authorization through the interface.

This does not sandbox malicious shipped Rust adapter code or a compromised local
process's ambient OS authority. Adapter implementation is trusted code; external
process can perform syscalls outside protocol without RAH mediation unless OS
confinement is separately implemented/proven. Existing restricted Codex surfaces
and rejection of built-in execution remain mandatory. No claim of network
isolation, rollback, credential transport security, complete TOCTOU elimination,
or universal server cancellation. Scope revocation cannot undo started effects.

## 23. Codex removal completion criterion

RAH core and production Desktop build with Codex composition/dependency disabled
and run a certified production alternate adapter with no codex-cli installed or
baseline store present. It completes neutral chat and required host-authorized
Tool round trips, model-selection/discovery absence behavior, cancellation/error/
shutdown/currentness/transcript flows through the same neutral contracts.
Operational Desktop modules import no concrete Codex types; Codex configuration,
artifact admission and crate are optional adapter composition. Both Codex-enabled
and disabled builds pass applicable deterministic/production gates, preserving
HostExplicit 11 and authority boundaries. Fake adapter success alone is not
production completion, and retaining optional Codex source in workspace is not
failure to decouple. Feature-disabled workspace validation must prove normal
operation does not accidentally require the optional crate or executable.

## 24. Validation, publication, and exact next task

Only this report changes. Required research validation is `git diff --check`;
full product tests/live provider probes are neither required nor claimed.
Commit message: `docs: design neutral runtime interface`. Normal GitHub master
push and one normal mirror attempt if authentication permits; no forced update,
credential repair, tag or release. Natural exact-head CI and final Git results
are recorded in task closure; this file cannot contain its own future commit SHA.
Original main worktree remains untouched; isolated publication worktree must be
clean at closure.

Recommend **Task 501 — Experimental neutral runtime contracts and fake adapter
conformance**: separately authorize and review the necessary ADR proposal; add
the minimal experimental interfaces/value semantics and fake tests from phase 1,
retaining legacy AgentRuntime and production Codex path. No alternate production
provider, Desktop switch, native-resume feature or v0.34 capability selection in
that task. This is a recommendation, not automatic implementation authorization.

**A — NEUTRAL RUNTIME INTERFACE DESIGN READY FOR IMPLEMENTATION**

Preferred C is evidence-based; object/lifetime model is explicit; Tool authority
is host-owned; Task 498 envelope and Task 499 catalog fit; Codex replay migration
is incremental; hypothetical hosted and local adapters require no Codex thread,
RPC, binary or certification assumptions. Implementation readiness refers to
the bounded experimental migration, subject to explicit ADR review, not an
externally stable API or certified alternate provider.
