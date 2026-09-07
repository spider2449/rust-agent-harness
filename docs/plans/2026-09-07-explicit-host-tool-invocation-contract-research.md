# Task 235 — Explicit Host Tool Invocation Contract Research

## Status

**RESEARCH COMPLETE — AWAITING EXACT-HEAD CI.** This is a documentation-only
contract for v0.20 A2; it neither invokes a Tool nor changes Rust, Desktop,
Tauri, Cargo, the Codex baseline, an ADR, or live certification.

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- Verified after `git fetch origin --tags`: `HEAD == origin/master ==
  6f34d710f054945cf3482e7f57717fc3f97b81ef`; worktree was clean.
- Task 234 is complete at that SHA; its exact-head CI `34072324664` passed.
- Released baseline: `v0.19.0`; metadata baseline is 13 packages, all `0.19.0`,
  edition 2024, with no dependency drift.

## Task 234 decision

Preserve A2: a trusted Desktop host action may select one already-composed,
current, public Tool when no model selects it. Host explicit dispatch is not
runtime/model dynamic dispatch and neither route is capability authorization.
The route chooses only a Tool whose authority was already composed by the host.

## Current dispatch architecture

The Codex dynamic path is:

```text
Codex dynamic request
 -> active session/thread and active-turn ownership
 -> private alias belongs to that turn's advertised snapshot
 -> exact snapshot ToolDefinition/current registry definition comparison
 -> current definition PermissionLevel is in host allowed_permissions
 -> RAH ToolRequested
 -> RAH ToolStarted
 -> ToolRegistry lookup
 -> Tool::execute and capability-specific policy/preconditions
 -> RAH ToolFinished (or sanitized runtime failure)
```

The bridge also rejects namespaces, unowned requests, duplicate call-ID content
conflicts, excessive tracked calls, cancellation/disconnect late calls, and
replay after cancellation. Its private `CallKey` is `(thread_id, turn_id,
call_id)`. It creates a new RAH `ToolCallId`, records actual lifecycle only for
an actual bridge execution, and runs the registry in an owned task. `ToolRegistry`
itself only finds the registered name and calls `Tool::execute`.

Classification of checks outside `ToolRegistry`:

| Check | Classification | Host route |
| --- | --- | --- |
| active turn, thread, private alias, advertised snapshot ownership | model-routing-only | do not reuse |
| duplicate Codex call ID / bridge replay cache | model-routing-only lifecycle | do not reuse |
| current definition equals captured name, description, schema | reusable dispatch authorization/currentness | reuse, with a host ticket fingerprint rather than alias snapshot |
| current `PermissionLevel` is in host `allowed_permissions` | reusable dispatch authorization | reuse exactly |
| Tool registration/lookup | reusable dispatch | reuse |
| Tool parser, repository/provider authority, CAS, lease, review, policy | capability authorization | reuse unchanged |
| `ToolRequested`/Started/Finished for a model call | runtime lifecycle bookkeeping | do not reuse for host work |
| Desktop composition/generation and Effective Authority currentness | Desktop currentness | host route must apply |

## Current permission boundary

`ToolDefinition` carries `None`, `Read`, `Write`, or `Execute`; the current
Desktop composer builds its `allowed_permissions` from host inputs. With a
repository it includes `None`, `Read`, and `Execute`; external permissions are
added only from admitted Trusted Profile records, never provider discovery
metadata. Without a repository it starts at `None`. The current first-party
Desktop registry uses `Read` for `fs.read`, and `Execute` for repository
observation, authoring, ref, and history tools. `Write` is not a
first-party Desktop authorization shortcut and must retain its exact current
host-composed meaning if a future admitted Tool requires it.

For host invocation, `None` means only that the current host composition permits
the definition's no-external-capability dispatch category; `Read` means the
same composition permits the definition's configured read category; `Execute`
means the same composition permits its configured execution category. None of
these grants repository content, index, history, ref, provider, network, or
process authority. The Tool's own policy decides that effect. The frontend
cannot select a level, and a name never implies a level.

## Security problem with raw ToolRegistry dispatch

Calling `desktop_registry.execute(call, ToolContext::default())` directly would
skip the bridge's current-definition and `allowed_permissions` checks. It would
therefore create a new effectful route with a weaker dispatch gate. It is not a
complete production host route and is prohibited by this contract.

## Host intent boundary

A human action is a structurally separate, backend command sequence: request a
known operation form, submit bounded fields, receive backend-sanitized review,
and explicitly confirm an opaque in-memory ticket. Model text, transcript
content, provider metadata, Effective Authority display, tool advertisement,
or JavaScript string matching can never create that sequence or dispatch it.
Frontend IPC is an untrusted request for the backend to validate, not authority.

Existing repository selection, Stage/Unstage action IDs, reviewed-commit
authorization, Connect/Disconnect, profile Choose/Restore/Forget, and Effective
Authority refresh demonstrate useful patterns: backend-issued opaque action or
review identity, generation checks, and backend re-observation. Buttons and
snapshots alone are presentation; only their backend handlers enforce intent.

## Dispatch architecture options

- **D1 — duplicate in Desktop:** simple locally, but creates a second copy of
  definition/current-permission semantics and an inevitable drift risk.
- **D2 — neutral authorized-dispatch primitive:** accepts a registry, current
  host permission set, expected definition identity, and `ToolCall`; it
  re-resolves, compares, permission-checks, then invokes `ToolRegistry`.
  Codex supplies its snapshot expectation; Desktop supplies its prepared-ticket
  expectation. It contains no alias, Tauri, frontend, or provider protocol.
- **D3 — Desktop policy plus equivalence tests:** can contain host-specific
  rules, but still duplicates the security-critical generic checks and tests
  cannot prevent future semantic divergence.

## Selected dispatch architecture

**D2.** Add, in a later task, a narrow public or crate-visible neutral
authorized-dispatch primitive in `rah-tools`, alongside `ToolRegistry`, because
that crate already owns the Tool abstraction and has no dependency on runtime,
Codex, or Desktop. It must take an already host-owned allowed-level set and an
expected current `ToolDefinition` identity (name, description, schema, and
permission); re-fetch the registered Tool; require exact equality; require the
definition permission in the supplied set; then call `ToolRegistry::execute`.
It returns structured dispatch rejection separately from Tool output.

It does not decide eligibility, create a registry, compose capability authority,
translate provider names, issue tickets, own tasks, emit events, or interpret
Tool results. Codex keeps active-turn/alias/replay ownership around D2; Desktop
keeps user intent/currentness/review around D2. This preserves dependency
bottom direction: `rah-tools` is neutral; `rah-runtime-codex` and
`rah-desktop` depend downward, with no cycle or new crate.

## ADR decision

**ADR REQUIRED.** This is an enduring new dispatch/provenance boundary: a
human-host route can reach effectful Tools without model mediation, decides
whether explicit intent may dispatch, and cannot rely on `ToolRegistry` alone
for all current permission gates. Existing ADRs govern underlying repository
and profile authority, but none decides this route's dispatch provenance,
currentness, and non-forgery rules.

Task 236 should propose **ADR 0021 — Explicit Host Tool Invocation Dispatch
Boundary**. Its scope is only `human-explicit dispatch -> current host
eligibility/policy -> normal ToolRegistry -> existing capability authority`.
It grants zero new underlying capability authority and forbids provenance
forgery, permission bypass, text-triggered execution, generic JSON consoles,
authority creation, replay, and rollback claims.

## Eligible Tool inventory

| Tool/class | v0.20 classification | Reason |
| --- | --- | --- |
| `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged` | **ELIGIBLE v0.20** | fixed first-party schemas, read-only classification, selected-repository binding, and bounded output/policy already exist |
| `repo.patch`, `repo.create-file`, `repo.edit-files`, `repo.delete-file`, `repo.rename-file`, `repo.create-directory` | **DEFERRED** | effectful but each needs its own typed host authoring/review DTO, preimage presentation, and mutation-specific first-release UX/certification |
| `repo.create-branch` | **ELIGIBLE v0.20** | fixed one-field schema, bounded first-party ref authority, explicit uncertainty result classification, and independently certified host-driven Windows effect |
| `repo.commit` | **DEFERRED** | reviewed-history authorization is separate and one-shot; do not broaden A2 before a dedicated binding/consumption UX is specified |
| production `host.*` | **NOT PRESENT IN DESKTOP** | no production `host.*` Tool is registered in the Desktop registry |
| `echo`, fixture/diagnostic tools | **NEVER PRODUCT HOST-INVOKABLE** | test/diagnostic use is not product eligibility |
| admitted local stdio MCP and Process Plugin Tools | **DEFERRED** | no safe generic host form/review/effect/lifecycle contract exists |

## First-release eligibility

The backend owns an exact allowlist, not ToolDefinition/provider metadata or
frontend heuristics: `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`,
`repo.diff-staged`, and `repo.create-branch`. A Tool must additionally be in
the current published composition and Effective Authority inventory, have the
matching first-party host classification, be permission-permitted, and pass
all currentness checks. Visibility, `advertised`, permission, and effect class
are independently insufficient.

## External-provider decision

**DEFER external host invocation.** Admitted external tools may have arbitrary
provider-defined schema and semantics, ambient remote/process effects, unknown
review quality, provider-owned cancellation/lifetime, duplicate identity
issues, and only coarse `External` effect classification. Trusted Profile
admission and Effective Authority description do not yield a generic safe form
or review contract. Provider metadata may never self-mark `hostInvokable`.
v0.20 improves selected first-party reachability; it does not solve explicit
host invocation for arbitrary external providers.

## repo.commit decision

**DEFER `repo.commit`.** Preparation may show that review is required but may
not arm, synthesize, serialize, or consume `RepositoryCommitReview` or
`RepositoryCommitControl`. A future commit-specific route would bind the exact
backend review identity and exact staged observation at confirmation; changed
staged state fails closed, while a branch creation that preserves the reviewed
snapshot does not by itself invalidate it. The existing Tool remains the only
consumer of a real one-shot reviewed authorization after normal revalidation.

## Input and review contract

H1 (name plus arbitrary JSON) is rejected: it is a generic console. H2 is
acceptable only when the backend owns exact typed/bounded fields. H3 is selected
for every effectful Tool: backend prepare -> sanitized review -> explicit
confirm opaque ticket -> revalidate -> dispatch. Read-only calls use the same
backend-owned typed preparation but may confirm once at form submission, with
no second confirmation.

The v0.20 forms are backend capability descriptors, not JSON-schema-generated
forms. Tool parsers/policies remain authoritative.

| Tool | User-editable fields and bounds | Backend-only / normalization / review | second confirmation |
| --- | --- | --- | --- |
| `fs.read` | `path`, a nonempty repository-relative UTF-8 path, at most 1,024 UTF-8 bytes | reject extra fields; normalize only as the Tool policy permits; show sanitized relative path and configured bounded-read category, never absolute root | no |
| `repo.file-info` | `path`, one nonempty repository-relative UTF-8 path, at most 1,024 bytes; total reconstructed request at most 4 KiB | reject extras; Tool parser/policy resolves it; show relative path and observation category | no |
| `repo.status` | no fields | backend constructs `{}` exactly; show selected repository display identity only | no |
| `repo.diff` | no fields | backend constructs `{}` exactly; fixed worktree-versus-index observation; review is a sanitized observation summary | no |
| `repo.diff-staged` | no fields | backend constructs `{}` exactly; fixed index-versus-HEAD observation | no |
| `repo.create-branch` | `name`: ASCII branch name, 1–128 bytes/characters as the Tool accepts | backend reconstructs `{name}`; no start OID, ref, repository root, authority, permission, or effect class is user-supplied; review shows sanitized branch name, “create local branch at current committed HEAD; no switch”, and mutation category | **yes** |

Implementation must read exact current schemas rather than widen them; a
descriptor cannot expose fields absent from the parser. All display strings,
field count, and output must be bounded. The branch Tool's existing parser and
authority remain the final name/CAS/precondition decision.

## Permission contract

At prepare and immediately before dispatch, obtain the same current Desktop
composition's host-owned `allowed_permissions` supplied to normal Codex bridge
dispatch. D2 verifies the currently registered definition's complete identity
and checks its actual `PermissionLevel` is contained in that set. The frontend,
ticket, tool name, profile metadata, and Effective Authority snapshot neither
supply nor elevate it. `Execute` remains only an outer dispatch category.

## Composition/currentness binding

**CONNECTED_CURRENT is required.** The host route has no disconnected mode and
never silently composes a new registry. It requires current published registry
and Effective Authority, exact current Tool, selected repository where the Tool
is repository-bound, current profile/provider composition, and no
reconnect-required state.

## Prepared invocation binding

Preparation creates a RAH-generated opaque host invocation ID and an
in-memory, nonserializable, single-use, bounded-lifetime ticket. It binds:

- exact public Tool name and complete ToolDefinition fingerprint;
- exact backend-reconstructed ToolInput and sanitized review;
- active registry/composition identity and allowed-permission policy identity;
- repository identity and `repository_generation` where applicable;
- model, profile, and connection generations; and the Effective Authority
  current snapshot identity;
- relevant repository-bound authority identity; and, for future commit work,
  exact review identity.

Confirm re-resolves the Tool and repeats eligibility, connected-current,
definition, permission, repository/currentness, and normal Tool preconditions.
Any mismatch, removal, replacement composition, changed definition/schema or
permission, repository/profile/model/connection generation, disconnect, or
expired/used ticket is `rejected_stale` (or the more specific dispatch
rejection) with no automatic re-prepare and no dispatch.

## Concurrency

v0.20 permits at most one host-explicit invocation in flight, including its
prepared-confirmed transition. It rejects a second host request as busy. It
rejects host prepare/confirm while any model turn is active, and rejects model
turn start while an effectful host invocation is active. For simplicity and
safe repository/provider lifecycle ownership, a read-only host invocation also
blocks model-turn start until terminal state in this first release. Existing
Tool/capability repository leases and provider lifecycle rules remain in force;
this product lock does not substitute for them. Disconnect/reconnect is
rejected while prepared work exists; after start it follows conservative
possible-effect handling rather than cancelling/replaying.

## Model-turn interaction

Model and host routes are mutually exclusive for all v0.20 host invocations.
This is narrower than necessary for future read-only concurrency, but prevents
ambiguous shared registry, activity, refresh, and disconnect behavior until
there is evidence for safe parallel semantics.

## Lifecycle provenance

Use a Desktop-private host activity record with `source: HostExplicit`, opaque
host invocation ID, public Tool name, and states `Prepared`, `RequestedByHost`,
`Started`, `Finished`, `Failed`, `CancelledBeforeStart`, and
`PossibleEffectUnknown`. The inner `ToolCall` receives a normal fresh RAH
`ToolCallId` solely for registry execution correlation; it has no fake Codex
thread, turn, or call IDs.

## AgentEvent decision

**No AgentEvent change.** `AgentEvent::ToolRequested`, `ToolStarted`, and
`ToolFinished` remain runtime/model lifecycle semantics. A host activity must
never emit or be counted as those events. A neutral cross-runtime event would
be premature Desktop pollution without a demonstrated non-Desktop consumer.

## Repository effect handling

Reuse existing source-independent semantic parsers and repository handling:
branch result classification, first-party mutation refresh selection, reviewed
authorization invalidation rules, desired/current state, and conservative
started-but-unfinished/uncertain treatment. Do not duplicate parsers.
Only mapping a runtime `AgentEvent` to activity is runtime-specific; host state
maps its own terminal ToolOutput through the same classification helper. For
`repo.create-branch`, verified success preserves reviewed authorization and
does not advance repository/currentness generations; uncertain output gets the
existing conservative refresh/handling. Future deferred mutations use their
existing invalidation/refresh semantics, not a blanket rule.

## Reviewed commit semantics

No v0.20-eligible call consumes reviewed commit authorization. `repo.create-
branch` retains its established non-effect on the reviewed snapshot when
verified. The deferred `repo.commit` route cannot prepare before authority in a
way that arms it, cannot mint an authorization from a confirmation, and must
bind confirmation to the backend review identity if later designed. Any staged
state change makes that authorization stale; ordinary Tool preconditions remain
authoritative.

## Cancellation

**C2: UI dismissal only after start; cancellation before start is supported.**
Dropping an unconfirmed/queued ticket produces `CancelledBeforeStart`, known to
have no effect. Once `Started` is recorded, v0.20 offers no active Tool abort:
the UI may dismiss activity but execution remains owned to a terminal result.
There is no retry, replay, compensation, or claim that cancellation means no
effect. If process/runtime loss prevents a terminal result after possible
effect, record `PossibleEffectUnknown` and apply conservative observation.

## Crash/lost-result semantics

**No crash persistence in v0.20.** Tickets, authority, ToolCall correlation,
and in-flight host activity are process-local and are never restored or
resumed. A crash after start can leave an unknown possible effect. On restart,
Desktop freshly composes authority and freshly observes selected repository
state; it does not claim no effect, retry, or replay. No descriptive in-flight
marker is required for this bounded first release; adding one later must be
bounded, secret-free, descriptive only, and never an authority/capability
handle.

## Conversation semantics

**No conversation injection and no chat message.** Host activity is displayed
separately, with visible `Host action — not Model` provenance. A user may later
describe a result in a normal message, but the system never automatically feeds
the ToolOutput into model context or makes host history appear as model Tool
activity.

## Effective Authority / UX contract

Effective Authority remains observational. Extend it later with backend-owned
per-tool host-invocation eligibility and a bounded unavailable reason, such as
`not_supported`, `stale`, `permission_denied`, `review_required`,
`model_turn_active`, `provider_not_supported`, or `repository_required`.
Snapshot data grants nothing; backend commands repeat all checks.

The first release is one non-debug flow: **Effective Authority -> eligible
first-party Tool -> Invoke -> typed form -> sanitized review -> Confirm (branch
only) -> HostExplicit activity -> terminal result**. There is no free-form
public name box or JSON editor. It displays provenance, effect/authority
category, and current/stale status.

## Sanitization

Review/activity/output reuse existing sanitized ToolOutput and error handling
where available, cap displayed text/JSON and activity fields, and show only
relative paths or selected repository display identity. They must not expose
absolute repository/Git/provider executable paths, environment, credentials,
tokens, profile source paths, provider stderr, private Codex aliases, internal
authority handles, review handles, or raw backend errors.

## Host dispatch outcome taxonomy

Dispatch-level states are distinct from underlying structured ToolOutput:

| Dispatch outcome | Meaning |
| --- | --- |
| `prepared` | valid bounded ticket created; no effect |
| `rejected_not_eligible`, `rejected_permission`, `rejected_stale`, `rejected_busy` | route denied before Tool start |
| `invalid_input` | backend descriptor/form rejected before dispatch |
| `started` | D2 admitted and Tool execution began; possible-effect boundary is Tool-specific |
| `tool_completed` / `tool_error` | execution returned output/result; preserve underlying structured status |
| `cancelled_before_start` | ticket removed before dispatch; known no effect |
| `possible_effect_unknown` | started call lost terminal result; observe, never replay |

For example, `tool_completed` plus underlying `branch_created_verified` is not
the same thing as a generic success, and an underlying `uncertain` remains
uncertain.

## Negative/misuse matrix

Future deterministic tests must reject: noneligible/public arbitrary names;
malformed, extra, and oversized fields; forged permission/effect class; stale
generation, definition/schema, permission, registry/tool removal, duplicate or
replaced composition, wrong repository, profile/model/connection changes, and
stale reviewed authorization. They must cover a host request during a model
turn, a second host request, disconnect while prepared and after start,
repository switch after prepare, cancellation before and after possible-effect
start, Tool error, structured uncertain ToolOutput, external metadata claiming
eligibility, and no replay after uncertainty.

They must also prove host activity emits no fake model lifecycle, frontend
cannot forge host intent through arbitrary JSON, definition changes are caught
by D2, and capability-specific policy/preconditions remain decisive.

## Windows live-certification contract

Do not run it in this task. A later Windows gate uses a current Desktop
composition and a fresh disposable repository (never a Task 229C fixture).
Read-only case: **`repo.status`**. Effectful case: **`repo.create-branch`**
with a unique branch name. It proves a visible eligible public Tool, exact
explicit host action and backend revalidation, same permission gate, normal
registry dispatch, underlying authority, exactly one execution, HostExplicit
provenance, no model `ToolRequested`, correct result and post-effect Desktop
handling, review/currentness behavior, terminal lifecycle, and cleanup.
No model request is required or accepted as evidence.

## Security nonclaims

This contract does not claim host dispatch is model dispatch; a click creates
capability authority; Tool presence/advertisement/Effective Authority authorizes;
`Execute` alone authorizes an effect; ToolRegistry alone currently enforces
permission; cancellation/crash means rollback/no effect; generic external tools
are safe; frontend is authority; arbitrary JSON is a production console; model
text triggers host action; OS sandboxing; or network isolation.

## Implementation dependency direction

D2 belongs in `rah-tools` and depends only on `ToolRegistry`/protocol types.
Desktop owns exact eligibility descriptors, tickets, IPC/form DTOs, currentness,
activity, and UX. Codex owns aliases, active turns, dynamic replay, and runtime
events. Profile composition/providers retain composition and lifecycle; they
cannot grant host eligibility. No new crate, Cargo edge, provider protocol,
Codex alias, Tauri type, or frontend type enters the neutral primitive.

## Required deterministic tests

Later tasks need D2 unit tests for definition identity, permission changes,
unknown/replaced tools, and registry-only dispatch prevention; Desktop tests for
allowlist/typed DTO/ticket single-use-expiry-currentness/concurrency/provenance;
and source-independent result-refresh/review tests for host activity. Include
the full negative matrix above and positive exactly-once paths for `repo.status`
and `repo.create-branch`, without live credentials, models, network, or GPU.

## Decision summary

| Required decision | Selected contract |
| --- | --- |
| dispatch architecture | D2 neutral authorized-dispatch primitive |
| ADR | required: ADR 0021 |
| connected-current | required |
| eligibility source | backend exact first-party allowlist plus current composition checks |
| eligible tools | `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch` |
| external Tools | deferred |
| `repo.commit` | deferred |
| input model | backend typed descriptors; prepare/review ticket for effectful work |
| confirmation | second explicit confirmation only for `repo.create-branch` |
| permission source | same current host composition policy as bridge dispatch |
| prepared binding | definition, input, composition, permissions, repository/model/profile/connection generations, authority identity |
| concurrency/model coexistence | one host invocation; no model turn overlap |
| lifecycle / AgentEvent | Desktop-private `HostExplicit`; no AgentEvent change |
| cancellation | before-start cancel only; after-start dismiss while owned execution continues |
| crash persistence | no; fresh observation after restart, no replay |
| conversation injection | no |
| stale ticket | fail closed; no automatic re-prepare |
| live cases | `repo.status`; `repo.create-branch` |

## Recommended next-task sequence

1. **Task 236:** ADR 0021 — Explicit Host Tool Invocation Dispatch Boundary.
2. **Task 237:** minimal authorized dispatch foundation (D2).
3. **Task 238:** deterministic dispatch/security hardening.
4. **Task 239:** Desktop explicit invocation workflow.
5. **Task 240:** Windows live certification.
6. **Task 241:** milestone audit.

## Validation

Passed sequentially: `cargo fmt --check`, `cargo check --workspace`, and
`git diff --check`; `cargo metadata --no-deps --format-version 1` reported 13
packages, all `0.19.0`, edition 2024. `git diff -- Cargo.toml Cargo.lock` was
empty. Pre-commit scope inspection requires exactly this one changed file.

## Commit

Pending: `docs: define explicit host tool invocation contract`.

## Exact-head CI

Pending: push `master`, then require the completed successful `push` CI run for
the exact Task 235 commit before marking this task complete.

## Next task

Task 236 — ADR 0021 — Explicit Host Tool Invocation Dispatch Boundary; not
started.

ADR DECISION: REQUIRED
