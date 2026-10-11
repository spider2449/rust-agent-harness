# Task 497 — Neutral runtime error boundary design

## 1. Checkpoint, scope, and evidence

Authoritative starting HEAD: `5869a45d07433f47b1e1ce78430ef811e0b5e8e5`.
Task 494 exact-head CI `37010829329` PASS is supplied historical evidence.
GitHub master and internal mirror master were both read back at this exact SHA.
The primary checkout is clean but older, at
`d5ef2e541d744f8070019878f6754d16a4b974a8`; it was not advanced or edited.
This report uses a new worktree `F:/temp/rah-task497`, branch
`task-497-error-design`, based on the authoritative checkpoint.

Task 495/496 worktree `F:/temp/rah-task495` is read-only evidence: exact base
HEAD above, 13 modified tracked files and 5 untracked files. No reset, clean,
stash, edit, rebase, commit, test rerun, or live validation there. Its reports
and source were inspected without resuming the task. Historical adapter result
remains **86 passed, 2 failed, 1 ignored; exit 101**.

RAH remains 0.33.0; HostExplicit exactly 11; preferred/certified Codex 0.157.1;
v0.34 capability NONE SELECTED. No source, test, frontend, ADR, dependency,
permission, Tool contract, CI, release, tag, or version changes in this task.

Authority consulted: README; ARCHITECTURE (crate topology, runtime and bridge);
ARCHITECTURE_GUARDRAILS (stable boundary candidates and change control);
SECURITY (host authority, private diagnostics and uncertain effects); accepted
ADRs 0001, 0002, 0005, 0030; Task 494/495/496 evidence. This is an explicit
boundary-design task, not permission to implement a stable-interface change.
An implementation task must record the material public-boundary decision in
an ADR, as the guardrails require; no ADR is written here.

## 2. Proven conflict and current error-path map

The Task 495 `runtime.rs::agent_start_error` consumes `CodexAdapterError`, logs
it, obtains `error.diagnostic(stage)`, and constructs
`AgentError::RequestFailed { diagnostic }`. The original typed error is dropped.
Before Task 495 the same function made `AgentError::Runtime { message }` with
precise static stage plus error Display. That retained more text, but never a
recoverable typed cause. Neither representation satisfies both requirements.

References below are paths relative to the repository; WIP means Task 495's
read-only version, baseline means authoritative HEAD. Function names provide
stable navigation when WIP line numbers differ.

| Boundary / actual type | Ownership and actual use | Loss / presentation |
| --- | --- | --- |
| `process.rs`, `protocol.rs`, `transport.rs`: `Result<_, CodexAdapterError>` | Owned error returned from fixed child transport or JSON-RPC parsing; `AppServerTransport: Send + 'static`, async methods take `&mut self` | IO errors remain sources; protocol parse details sometimes already stored as strings |
| `connection.rs::Command::Request`: `oneshot::Sender<Result<Value, CodexAdapterError>>` | Owned error sent across connection task; request receives it | Matched RPC response retains code/message. Receive/parse failure sends the original error to one pending request; other pending requests get synthetic disconnected ProtocolViolation |
| `ConnectionEvent` broadcast | `Clone + Debug`; `broadcast::channel(128)` requires cloning. Notifications own JSON `Value`; `Fault { message: String }` | `broadcast_fault` calls `error.to_string()`; unknown response IDs produce fault strings. Typed source is lost here, before AgentRuntime |
| `CodexRuntime::connect*`, `from_transport*`, WIP preflight, `shutdown` | Concrete `Result<_, CodexAdapterError>`; runtime owns connection via Arc and lifecycle tasks | Desktop connection mapping and WIP `model_preflight::present` consume/map the concrete error; no retained source after presentation |
| `AgentRuntime::start/resume/cancel` | Concrete neutral `Result<AgentHandle, AgentError>` or `Result<(), AgentError>` | Baseline start and `agent_error` stringify. WIP start drops source into diagnostic; resume/interrupt still stringify. Cancel's Fault uses its already flattened message |
| `AgentHandle::into_events`, `AgentEventStream` | Owned `Pin<Box<dyn Stream<Item = AgentEvent> + Send>>`; implicit object lifetime is `'static` | No Result item and no local typed failure object. Terminal `AgentEvent::Failed { code, message }`; WIP adds optional diagnostic |
| `runtime.rs::event_stream/passive_stream/failed`, `bridge.rs` | Own session/turn routing; terminal guards supervise lifecycle | Fault and failed turn become strings. WIP completion extracts safe numeric metadata from JSON; source itself is not retained |
| `MinimalTestRuntime` in `minimal.rs` | Owned streams; shared session/cancel state | Model and Tool failures also become `AgentEvent::Failed` messages; this is a generic problem, not solely a Codex mismatch |
| Desktop `ConnectionState`, `run_chat`, `frontend_error` | Currently concrete `Arc<CodexRuntime>`; async chat task consumes stream; generation checks govern publication | Start error logged with `%error`; Failed message logged. Frontend receives closed codes; WIP additionally forwards diagnostic clones |
| `ChatEvent`, `ModelPreflightPresentation`, frontend `status.js` | Owned serializable snapshots, no Rust source object in JS | WIP renders stage/kind and optional numeric RPC/HTTP status. No frontend downcasting |
| CLI `main.rs::run_demo` | Minimal runtime; consumes events; start error wrapped with anyhow Context | Failed event becomes `bail!` text. No Codex-specific recovery |
| `rah-session` | `Session`, `SessionStatus`, messages and store errors | Not the runtime error transport; no AgentRuntime/AgentError/AgentEvent handling in its source. Do not invent a session-store source registry |

Connection shutdown currently sends `oneshot::Sender<()>`, ignores
`transport.shutdown()` failure both on explicit shutdown and loop cleanup,
then joins the connection task. Join error becomes ProtocolViolation text.
The proposed envelope cannot recover causes already ignored. Migration must
propagate explicit shutdown failure; drop cleanup remains best effort, with
safe diagnostic logging and no promise of a returned error from Drop.

## 3. Actual type / trait constraints

| Type | Clone / equality | Thread/lifetime requirements | Serde, Debug, Display, source/downcast |
| --- | --- | --- | --- |
| `CodexAdapterError` | Debug + thiserror Error; no Clone/Eq/PartialEq | Owned String/PathBuf/ExitStatus/io::Error/static string fields support Send + Sync + 'static; currently moved through spawned tasks/oneshots, not explicitly bounded on enum | No serde. Display can contain paths, stderr and peer messages. IO variants expose io::Error via source; no existing runtime consumer source traversal/downcast found |
| `AgentError` | Explicit Clone/Debug/Eq/PartialEq/Error derives, baseline and WIP | No explicit trait bound on enum; owned fields. Send output is needed by actual spawned start tests and Desktop async work | No serde. Baseline Display embeds message; WIP RequestFailed displays diagnostic Debug. No source field; source() returns None. Downcast of AgentError itself possible as Error, original adapter cause impossible |
| `AgentRuntime` | Trait has no clone/equality requirement | Explicit Send + Sync; async_trait uses Send futures | Object-safe concrete error return, no associated error, no serde |
| `AgentHandle` / stream | Neither handle nor stream Clone/Eq/serde | Stream is Send + 'static, not Sync; handle owns stream | No typed error item today |
| `ConnectionEvent` | Clone/Debug, not Eq/serde | Owned values carried through Tokio broadcast and spawned connection task | Fault text only; no Error implementation/source |
| `AgentEvent` | Clone/Debug/Eq/PartialEq | Owned fields, shared broadcast/streams | Serialize + Deserialize; tagged protocol enum. Not Error, no source/downcast |
| WIP `RuntimeDiagnostic` | Clone/Debug/Eq/PartialEq; stage/kind Copy | Owned closed fields; no borrowed provider data | Serialize/Deserialize, camelCase fields; no Error/source |
| Desktop `ChatEvent` | Clone/Debug/Eq/PartialEq | Task-to-IPC owned snapshot | Serialize, tagged enum; codes and optional WIP diagnostic only |
| WIP `ModelPreflightPresentation` | Clone/Debug, no Eq derive | Owned catalog/selection/diagnostic snapshot in status | Serialize; its `present` function currently discards typed error |

Concrete equality use: `rah-runtime/tests/minimal_runtime.rs` compares
SessionNotFound with `assert_eq!`; runtime conformance accepts
`&dyn AgentRuntime`; protocol/runtime/bridge tests compare full AgentEvents.
Inspection found no runtime-error clone call requiring deep cloning of an
adapter source. Clone remains useful at the connection broadcast boundary and
for retained failures shared between consumers. Whole opaque-source equality
is neither required nor meaningful. Unrelated ToolError and provider Tool
contracts keep their equality and cloning unchanged.

No shared source-carrier abstraction was found in the runtime path. CLI's
anyhow Context is not the public runtime contract. Do not replace domain
classification with an unstructured anyhow return.

## 4. Failing-contract semantics

`runtime::tests::explicit_effective_model_and_provider_must_match` starts with
explicit selected model/provider, accepts matching effective fields, then
rejects effective model `fallback` for selected `selected`. It requires Display
to contain `explicit model/provider mismatch`. `verify_effective_model_config`
rejects either model or provider mismatch with ProtocolViolation and preserves
requested/effective values in its message. This async test specifically covers
model mismatch; adjacent `explicit_effective_model_config_requires_matching_top_level_fields`
covers additional model/provider validation. Never accept fallback as success
or claim catalog membership proves effective selection.

`runtime_tests::malformed_response_and_unexpected_exit_are_typed_failures`
sends nonnumeric response ID during thread/start, requires `response ID must be`
in Display and stopped-peer evidence. Its second case injects ProcessExited,
fixture exit 7 / `captured stderr`, requires stderr in Display and stopped-peer
evidence. It does not downcast, assert exit code, or prove multi-pending-request
correlation. Historical failure stopped at the first assertion; the second
scenario was not established as passing in that run.

Guarantees are fail-closed rejection, useful original failure detail, selected
configuration fidelity and transport shutdown ownership. Test names alone do
not establish typed recovery at the neutral boundary. Future tests must retain
all these semantic checks. Outer Display becomes safe by design: relocate raw
detail assertions to the recovered source, add typed/exit-code assertions and
keep stopped-peer assertions. That is a deliberate documented presentation
contract change, not weakening assertions until green. Literal old outer
Display behavior and guaranteed sanitized outer formatting cannot both remain.

## 5. Candidates A–E

| Candidate | Fit and limitations | Decision |
| --- | --- | --- |
| A: neutral envelope + `Box<dyn Error + Send + Sync + 'static>` | Preserves typed source with one owner; Send streams fit. Box does not clone, so cannot share one original cause among broadcast receivers/pending requests without another carrier or ownership redesign. Never require provider Error: Clone | Reject Box as primary carrier, retain as a valid single-owner constructor input |
| A: neutral envelope + `Arc<dyn Error + Send + Sync + 'static>` | Shares immutable original, clones carrier without cloning error. Supports broadcast, result, stream, tests and lifetime after runtime shutdown. No serialization/equality for source | Preferred |
| B: adapter-owned opaque detail | `Any + Send + Sync` permits downcast but lacks Error/source/causal semantics. A custom trait adding these just recreates A with extra API. Detail may be useful for non-error data later, but generic layers must not require adapter access | Reject unrestricted Any bag; unnecessary R2 accessor |
| C: independent operational and source channels | Observer/store keyed by IDs risks missing entry, ordering race, retention leak, stale generation and uncorrelated terminal failures. Log-only source is not recoverable to runtime caller | Reject independent channels. Accept atomic local envelope plus explicit serialized projection as the serialization split, not two independently emitted failures |
| D: associated error on AgentRuntime | An associated type can be used on `dyn AgentRuntime<Error = E>`, so it is not inherently non-object-safe. Different E values prevent one heterogeneous trait-object store; an erasing wrapper restores A anyway. Existing conformance uses plain dyn AgentRuntime and Desktop already needs decoupling | Reject on public trait. Concrete lower-level adapter transport may retain its own typed Result without adding an associated public runtime error |
| E: exhaustive neutral semantic enum | Rich classification can describe operational effects, but cannot losslessly encode arbitrary future adapter errors, IO source chains, protocol-specific values or stderr. Expanding neutral enum to every provider's wire semantics conflicts with ADRs | Reject as replacement for source; use a deliberately broad neutral taxonomy within A |

Taxonomy stress cases: effective model/provider mismatch maps to configuration
inconsistency, but requested/effective details stay in original ProtocolViolation;
malformed JSON-RPC maps to protocol failure, exact parsing/correlation reason
stays in source; child exit maps to unavailable runtime, ExitStatus/stderr stay
in source; HTTP rejection maps to provider rejection only if adapter knows it,
status may be safe metadata but body is source-private; RPC rejection retains
numeric code as optional metadata and original code/message in source;
version/schema mismatch maps to incompatible runtime with original expected/
actual/missing data retained. Current CodexAdapterError has no HTTP variant:
WIP HTTP metadata comes from failed-turn JSON, not an HTTP exception. Do not
invent a current typed HTTP cause or infer authentication from status alone.

## 6. Preferred neutral contract and recoverability

Select A with Arc and R1. Conceptual process-local contract (not implemented):

```text
RuntimeFailure (Clone, Error; no Serialize/Deserialize or Eq)
  immutable RuntimeDiagnostic
  optional Arc<dyn Error + Send + Sync + 'static>

AgentError
  existing neutral request/session cases
  runtime failure carrying RuntimeFailure

RuntimeEvent (process-local, no serde or equality)
  ordinary AgentEvent excluding terminal Failed
  failed { session_id, AgentErrorCode, RuntimeFailure }
```

Use constructors/private fields to prevent attaching contradictory failure
messages and diagnostics. A local failed item has exactly one failure envelope;
its serializable AgentEvent::Failed is generated from that envelope. No separate
mutable diagnostic store. Projection may consume or borrow the envelope; typed
recovery is process-local and available while a caller retains it, not after
serialization or after all handles are dropped. Non-provider failures may have
no source, but every adapter error conversion must retain Some(original).

`AgentError::source()` returns RuntimeFailure; RuntimeFailure's source returns
the **inner error reference**, not the Arc wrapper. Adapter tests iterate
source() and use `downcast_ref::<CodexAdapterError>()`. Recovery means an
immutable reference to the original value, including original fields and its
own IO source chain. Owned extraction or mutable recovery is not required;
Arc may have multiple owners. R2 adds no needed guarantee; R3 is infeasible.
Generic orchestration never downcasts or imports CodexAdapterError. Existing
adapter tests/investigation inside rah-runtime-codex can downcast. Opaque
source storage exposes no concrete provider type in a generic signature and
does not authorize provider-specific interpretation in Desktop.

Rust's stable Error source and downcast_ref APIs support this mechanism;
Error::provide and Error::sources convenience iteration are not needed.
See [Rust Error documentation](https://doc.rust-lang.org/std/error/trait.Error.html).
Arc cloning shares allocation; it does not make an unsafe inner value thread
safe, hence the explicit Send + Sync source bounds.
See [Rust Arc documentation](https://doc.rust-lang.org/std/sync/struct.Arc.html).

All present CodexAdapterError fields satisfy those bounds without a Clone
requirement. A future provider must attach an owned, thread-safe original
error, or produce an adapter-owned typed snapshot at its non-thread-safe SDK
boundary and explicitly document that limitation. Do not advertise preservation
of an original non-Send/non-Sync object that cannot cross this async boundary.

## 7. Neutral diagnostic fields and safety policy

Put serializable diagnostic value types in dependency-bottom rah-protocol;
process-local RuntimeFailure/RuntimeEvent in rah-runtime. No new crate edge.
Proposed initial operation vocabulary: creation, connection, catalog observation,
session start/resume, request submission/completion, cancellation, shutdown.
Stage distinguishes dispatch, response validation and event consumption where
known; no `thread/start`, JSON field paths or Codex wire enums in generic fields.
Unspecified is valid; generic layers must not fabricate precision.

Broad kind vocabulary: invalid configuration, incompatible runtime, provider
rejection, protocol failure, transport failure, unavailable runtime, deadline,
operation failure. User cancellation is a terminal Cancelled event, not a
failure kind. Tool/permission errors retain their existing AgentErrorCode and
authority meaning; this design does not reclassify them as provider errors.

Use bounded closed summary/detail/action identifiers with generic presentation
templates rather than arbitrary provider text. Initial optional metadata:
RPC numeric code and HTTP status restricted to 100–599, only if the adapter
has structured evidence. No interpreting their meaning in generic code. No
raw stage suffix or provider string extension map. An adapter may retain
precise private stage context in a source wrapper whose source is the original
error; R1 chain recovery still works.

Do not add a generic retryable Boolean. Separate effect certainty
(`not_dispatched`, `may_have_effect`, `unknown`) from connection disposition
(`unusable`, `unknown`); default unknown. Neither field is an authorization to
retry or publish. A successful preflight and current host-generation checks
remain necessary for connection publication. Model/provider mismatch rejects
the operation; it does not alone prove the child exited. Timeout, process death
and cancellation do not establish rollback. No automatic replay, reconnect,
credential diagnosis or inference-readiness claim.

## 8. Ownership, clone, equality, and lifecycle

Retain Clone on AgentError/envelope by cloning immutable diagnostics and Arc;
remove Eq/PartialEq on source-bearing process-local errors instead of inventing
pointer or Display equality. Existing neutral case tests use pattern matching
and field equality. Serializable diagnostic/protocol/frontend snapshots keep
Clone/Eq/PartialEq. A source need not itself clone or compare.

At connection fault, allocate Arc once and share the same original failure with
affected pending requests and broadcast subscribers, preserving per-request
operation context in envelopes. Change private pending reply types as needed;
do not rebuild original errors from text. For unmatched response IDs construct
a typed ProtocolViolation before fanout. Never claim a connection-wide fault
belongs to just whichever HashMap request is found first. Distinguish the
causal connection failure from each request's uncertain effect status.

Stream futures remain Send + 'static; no Sync requirement for stream itself.
Envelope ownership outlives the connection if retained, without holding a
runtime/child/task Arc inside the source. Sources contain diagnostic values,
not lifecycle handles. No detached diagnostic task, global error registry,
mutex across await or unbounded history. Existing TurnGuard interruption and
bridge task ownership stay intact. Dropped consumers lose their unretained
causes by normal ownership, not via hidden asynchronous cleanup.

## 9. Serialization, security, and responsibilities

The split occurs when a host consumer projects RuntimeEvent into AgentEvent,
and when Desktop builds ChatEvent/app_status or maps a connection factory
failure into FrontendError plus diagnostic. No serde implementation on local
error/envelope; skipping a source field on deserialization would silently
pretend a reconstructed error still has its original cause. Only the safe
snapshot crosses IPC, persistence or model-facing boundaries.

Adapter owns evidence interpretation and safe metadata production. Generic
runtime owns schema bounds, closed fields, safe outer Debug/Display and default
unknown safety semantics. Host owns publication, permission and presentation
rules. Frontend renders closed operation/stage/kind, bounded detail/action,
optional numeric status; it never examines concrete errors or arbitrary bodies.

Outer Error Display and Debug show only the safe snapshot, never source Debug/
Display. Error::source intentionally exposes private detail to process-local
callers: arbitrary recursive error reporters can still expose it. Audit CLI
anyhow reporting, `%error`/`?error`, adapter tracing and Desktop logs during
migration; default logs use safe fields. The source is not a redaction boundary
for a caller explicitly traversing it. Adapter investigation may inspect source
under controlled test/debug handling; do not automatically log raw provider
bodies, unrestricted stderr, credentials, auth headers or private paths.
Retention of existing private details grants no new logging permission.

Codex responsibility: RPC/HTTP interpretation, framing/correlation, child
status/stderr, certification/schema, requested/effective model/provider/cwd,
failed-turn wire detail and precise private stages. Generic layers use only
neutral fields, never pattern-match these variants. Desktop currently imports
and matches CodexAdapterError in frontend_error and WIP preflight presentation;
that existing adapter-specific composition seam must be migrated, not treated
as the desired neutral architecture. No generic layer gains Codex semantics.

## 10. Public API blast radius and session/stream coverage

The current trait uses multiple paths: concrete AgentError Results for
start/resume/cancel, plain AgentEvent stream for post-start outcomes, concrete
adapter creation/preflight/shutdown Results outside AgentRuntime. It has no
generic connection/creation/shutdown methods today. Do not add them merely to
carry this error. Host factory can translate adapter Results at composition.

Result methods retain their signatures but AgentError's variants/derives change:
public API change, not only an internal crate correction. New RuntimeEvent
stream changes AgentEventStream's Item and handle consumers: also public API.
AgentRuntime remains dynamically dispatchable with no associated error. This
is the smallest coherent full-lifecycle source-preserving design; a result-only
change is possible but would leave known stream loss and is not the final goal.
Keeping the old Item with a separate source registry is the rejected C fallback.
Thus no prerequisite wholesale AgentRuntime redesign is needed (classification
C is not warranted), but implementation must explicitly approve/record these
bounded public changes before coding.

Affected implementation/test consumers: rah-runtime lib/minimal and conformance/
minimal tests; rah-runtime-codex connection/runtime/bridge and transport fixtures,
runtime/bridge tests and examples that consume streams; rah-cli demo; Desktop
chat/connection/preflight/shutdown handling and tests; rah-protocol diagnostic
and Failed projection/roundtrip tests; frontend safe diagnostic rendering.
`OwnedTurnStream::Item`, BridgeControl/ConnectionEvent RahEvent routing and every
AgentHandle constructor must agree. Keep ordinary Tool events and authorization
contracts unchanged. rah-session store types and dependencies need no change.

Apply the same carrier to creation/connection, session start/resume, turn
submission, streamed protocol/provider failure, interrupt rejection, child
death and explicit shutdown. A completed interruption remains Cancelled;
cancel failure carries a cause. Failed-turn JSON may require a new **private
adapter typed error** since current code starts with a Value, not an existing
CodexAdapterError. Keep source details bounded under established adapter policy;
never retain whole arbitrary wire history. Synthetic channel closure/lag and
stream exhaustion need typed neutral/local failures, not fictional Codex causes.

## 11. Deterministic testing strategy

Implementation gates must prove neutral fields and adapter source properties
separately, without trait-object equality:

1. Construct a non-Clone custom Error in neutral runtime tests; wrap, clone,
   traverse source and recover identical original reference. Compile-time
   Send/Sync/'static checks; no Codex dependency in these tests.
2. Codex tests recover ProtocolViolation mismatch/correlation detail and
   ProcessExited exit code/stderr; preserve rejection and stopped-peer checks.
   Add independently exercised provider mismatch and effective configuration
   validation. Assert safe outer formatting excludes raw fixture values.
3. Fault fanout retains same cause for multiple pending requests and stream
   subscribers. Correlate request/session/turn locally; terminal event is emitted
   once. Test resume, cancellation, stream drop, process death and shutdown error.
4. Failed-turn metadata fixtures cover RPC/HTTP numeric values, malformed/missing
   values and protocol/schema/version causes. No model/credential/network need.
5. Serde roundtrip applies only to diagnostics/projected protocol events; secret,
   header, body, stderr and private-path sentinel values never appear in IPC,
   frontend snapshots, default log formatting or serialized messages.
6. Retain generic conformance on dyn AgentRuntime and MinimalTestRuntime. Validate
   unchanged Tool policy and lifecycle ordering through RAH-owned interfaces.

These are future acceptance criteria, not tests executed by Task 497.

## 12. Task 495 salvage and provider removal

**S2 — partially salvaged.** Catalog parser/bounds, one-observation policy,
five-second deadline, explicit-selection advertisement gating, Inherit/custom
provider no-probe behavior and generation-scoped status intentions are reusable.
They remain historically partial/unvalidated; reuse is not certification.
Diagnostics need redesign: RequestFailed diagnostic-only enum, agent_start_error,
connection fault strings, completion translation, Desktop direct Codex matches,
preflight present's consumed cause, raw logging, protocol/local stream split and
frontend projection. This reaches beyond replacing one converter, so S1 is
too optimistic; bounded catalog logic does not justify discarding everything
under S3. Keep stopped WIP frozen; port selected pieces in a new worktree only
under explicit future authorization.

Would it work with Codex removed and native OpenAI or local runtime instead?
**Yes.** SDK/network/local process errors supply their own owned Error sources;
generic operation/kind/effect uncertainty and sanitized projection still apply.
RPC/HTTP metadata is optional, not mandatory provider identity. No model weights,
tokenizers, inference kernels or provider SDK types enter rah-runtime. This
supports Task 494 decoupling rather than moving Codex variants into the core.

## 13. Migration and exact next task

Recommend **Task 498 — Implement the neutral runtime failure envelope and
process-local failure stream, with deterministic source-preservation gates**.
Use a clean authoritative-head worktree. First record/approve the bounded
AgentError/stream API decision in an ADR, consistent with 0001/0002/0005; then
implement diagnostic values, Arc source envelope and atomic stream projection.
Migrate MinimalTestRuntime and conformance, Codex result/fault/stream/shutdown
paths, CLI and Desktop safe consumption. Include adapter creation/factory
translation without designing a new provider-selection framework. Port only
necessary Task 495 diagnostic pieces read-only; do not resume its catalog/live
work or alter its worktree. Run focused deterministic regression and required
integration checks for that public change. No live inference, release, new
capability or runtime-version admission is implied.

Then a separately authorized Task 499 can port reusable Task 495 catalog/
preflight behavior onto the implemented boundary and finish its outstanding
deterministic and specifically authorized live gates. Boundary readiness is
not Task 495 acceptance or product readiness.

## 14. Validation and publication record

Task 497 changes only this report. Local `git diff --check` and staged
`git diff --cached --check` passed; the staged stat is one report, 406 added
lines before this publication-note refinement. No product suite or
failed-contract rerun was executed or required.
Publication commit message: `docs: design neutral runtime error boundary`.
Normal exact-head master pushes to GitHub and internal mirror, no tag/release.
The final response records commit SHA, both remote readbacks and matching
natural push CI, avoiding a self-referential commit hash in this document.
The clean Task 497 worktree and frozen Task 495 status are checked at closure.

## 15. Classification

**A — NEUTRAL ERROR BOUNDARY DESIGN READY FOR IMPLEMENTATION**

Preferred Arc-backed neutral envelope retains R1 typed recovery; generic
layers stay provider-neutral; local source versus serialized safe snapshot is
explicit; clone/equality/stream constraints and deterministic tests are defined;
Task 495 has an S2 migration path. This is design readiness, not implementation,
validation or permission to resume the stopped WIP.
