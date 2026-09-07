# ADR 0021: Explicit Host Tool Invocation Dispatch Boundary

## Status

Accepted

## Date

2026-09-07

## Context

RAH historically reaches Tools through runtime/model requests. Tasks 207 and
229 demonstrated a repeated product reachability problem: Tools can be
correctly composed, registered, advertised, and current while the tested
model/runtime produces no Tool request. Task 229C independently proved that a
real host-owned registry/effect path can work correctly even when model
selection does not occur.

v0.20 therefore adds an explicit human-host route for a deliberately selected
subset of already-composed Tools. This is a new architectural route because it
reaches effectful Tools without model mediation, `ToolRegistry` alone does not
contain all existing dispatch permission checks, provenance differs from
runtime-driven execution, and currentness and user-intent binding must be
explicit. This ADR records that route boundary only.

## Decision

The core invariant is:

```text
host explicit dispatch != runtime/model dynamic dispatch != capability authorization
```

Host explicit dispatch is a structurally distinct Desktop action by a trusted
human. It may choose only a host-eligible Tool already in the current host
composition. Runtime/model dispatch remains a runtime request for Tools
supplied through the normal runtime contract, with runtime-owned lifecycle.
Capability authorization remains the Tool's existing authoritative policy:
for example, `RepositoryBranchCreationAuthority`, `RepositoryCommitControl`,
worktree policy, repository identity, and provider admission. Neither route
manufactures those capabilities.

This route adds no repository, filesystem, index, commit/history, branch/ref,
provider, network, process/executable, credential, or persisted authority. It
cannot turn `PermissionLevel::Execute`, Tool presence, Effective Authority
visibility, human confirmation, frontend state, or model text into authority.

### Neutral authorized-dispatch boundary

D2 is accepted. A future implementation adds a neutral authorized-dispatch
primitive in `rah-tools`, alongside `Tool` and `ToolRegistry`. It receives only
host-owned inputs: the current registry, current allowed `PermissionLevel`
policy, expected `ToolDefinition` identity, `ToolCall`, and the existing
`ToolContext`.

It must re-resolve the current Tool by public `ToolName`, obtain its current
definition, require exact expected/current definition identity, require the
current definition permission in the current allowed policy, and only then
invoke ordinary `ToolRegistry` execution. Identity includes at least name,
description, input schema, and permission; same name alone is insufficient.
It returns a dispatch rejection distinctly from the underlying `ToolOutput`.

The primitive does not choose host eligibility, compose a registry, create or
infer capability authority, issue Desktop tickets, own Tauri IPC or forms,
model aliases, turns, Codex replay, provider protocol, repository outcome
interpretation, review authority, reconnect/recomposition, retry, or rollback.
This preserves `rah-tools` neutrality.

The future Codex bridge should reuse this definition/permission gate where
practical. Codex-specific thread and active-turn ownership, private aliases,
dynamic snapshot routing, call-ID replay/deduplication, and Codex lifecycle
remain outside it. Thus the Codex route cannot check permission while a host
route silently bypasses that same current-definition plus current-host-
permission semantic.

Direct Desktop `ToolRegistry::execute` is not a production host path: the
registry currently performs lookup and `Tool::execute`, but not the bridge's
reusable definition/current-permission checks.

### Host intent, currentness, and eligibility

A host invocation begins only through an explicit backend-recognized human
action contract. Model text, conversation content, NLP detection, string
matching, model-generated UI action, provider metadata auto-trigger, and
frontend JavaScript inference cannot invoke it. The frontend is an untrusted
requester; backend validation is authoritative.

`CONNECTED_CURRENT` is mandatory in v0.20. Invocation requires a current
published Desktop composition and registry, exact currently registered Tool,
current Effective Authority, matching repository context where repository-
bound, matching profile/model/connection state in the currentness tuple, and
no reconnect-required state. The route must not silently build a disconnected
registry, reconnect, recompose, activate Trusted Profile providers, or restore
authority.

Visibility is not eligibility. For the first release, backend-owned explicit
first-party eligibility is limited to:

- `fs.read`
- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`
- `repo.create-branch`

Eligibility additionally requires presence in the current composition, the
expected first-party definition and host classification, allowed permission,
selected/current repository where required, and all capability-specific
authority and preconditions. It is not stored in provider-controlled metadata,
inferred in the frontend, inferred from effect class, or inferred from
permission alone.

External MCP Tools and Process Plugin Tools are deferred: arbitrary provider
schema and effect semantics have no safe generic typed form, review,
eligibility, lifecycle, uncertainty, or cancellation contract. Provider
metadata cannot declare itself host-invokable. `repo.commit` is deferred
because reviewed commit authorization is distinct and one-shot. The worktree
authoring Tools `repo.patch`, `repo.create-file`, `repo.edit-files`,
`repo.delete-file`, `repo.rename-file`, and `repo.create-directory` are
deferred until each has capability-specific typed review DTOs and
preimage/effect review contracts. Echo and fixture/diagnostic Tools are not
product host-invokable. These deferrals are not permanent prohibitions.

### Input, review, and ticket binding

The production Desktop contract rejects `ToolName + arbitrary JSON`. The
backend owns typed, bounded descriptors; Tool parsing and policy remain
authoritative. Read-only first-release forms require no second confirmation
after normal explicit submission. `repo.create-branch` uses:

```text
prepare -> sanitized review -> explicit confirm -> revalidate -> dispatch
```

The user supplies only its already-permitted bounded `name`, never repository,
HEAD OID, ref namespace, permission, authority, effect class, Git executable,
raw argv, hooks, or configuration.

Effectful preparation creates an opaque RAH-generated, in-memory,
nonserializable, process-local, bounded-lifetime, single-use ticket. It is
Tool-, input-, composition-, and currentness-bound. It binds at least public
Tool name, complete expected definition, exact reconstructed input, sanitized
review identity, registry/composition identity, allowed-permission policy
identity, repository identity/generation where relevant, model/profile/
connection generations, current Effective Authority identity, and relevant
capability-authority identity. Immediately before dispatch, the backend
revalidates. A stale ticket fails closed; there is no automatic re-prepare or
execution against updated state.

Permission is only the dispatch plane. The frontend never chooses it; a ticket
never grants it; and a Tool name never implies it. The route uses the same
current host-composed allowed-permission policy as normal runtime dispatch,
while D2 rechecks the current definition permission at dispatch time.

### Concurrency, provenance, and results

v0.20 permits at most one prepared or in-progress host-explicit invocation.
There is no overlap with a model turn: host work is rejected while a model turn
is active, and a model turn cannot start while host work is active. Read-only
host work also participates in this exclusion. Existing repository mutation
leases and provider lifecycle locks remain independently authoritative.

Host provenance is `HostExplicit`. It must not emit or synthesize
`AgentEvent::ToolRequested`, `AgentEvent::ToolStarted`, or
`AgentEvent::ToolFinished` unless a real runtime/model request occurred. There
is no `AgentEvent` schema change in v0.20; Desktop maintains its own
source-distinct host activity.

Dispatch-level activity states are `prepared`, `rejected_not_eligible`,
`rejected_permission`, `rejected_stale`, `rejected_busy`, `invalid_input`,
`started`, `tool_completed`, `tool_error`, `cancelled_before_start`, and
`possible_effect_unknown`. They do not erase the Tool's structured result:
`tool_completed` plus `branch_created_verified` is not generic success, and
an underlying `uncertain` remains uncertain.

Host and runtime provenance differ; capability result semantics do not. Reuse
existing exact parsers, currentness, review handling, and conservative
uncertainty treatment rather than a loose host parser. In particular,
`repo.create-branch` `branch_created_verified` preserves its established
review semantics, does not increment `repository_generation`, does not
reconnect, and remains current. No v0.20-eligible Tool consumes reviewed
commit authorization. The route may not create `RepositoryCommitReview`, arm
`RepositoryCommitControl`, synthesize or serialize commit authorization, or
infer it from confirmation.

### Cancellation, crash, and conversation boundaries

Prepared/unstarted work may be cancelled with `cancelled_before_start`, known
to have no Tool effect. After `started`, v0.20 has no active abort command:
the UI may dismiss activity, but owned execution continues to terminal handling
where possible. There is no retry, replay, compensation, or rollback claim.
Lost terminal result after possible effect is `possible_effect_unknown` and
uses capability-specific conservative observation.

Tickets, authority, and in-flight execution are not persisted. After Desktop
restart, authority is freshly composed and repository/provider state freshly
observed; execution is not resumed or replayed and no prior effect is claimed
absent. Host invocation creates no chat message, ToolOutput injection, fake
model Tool call, or automatic agent-turn continuation; results are separate
Desktop activity.

Effective Authority remains observational. It may later show backend-owned
eligibility and bounded unavailable reasons such as `not_supported`, `stale`,
`permission_denied`, `review_required`, `model_turn_active`,
`provider_not_supported`, or `repository_required`; its snapshot is never
dispatch authority and command-time checks remain mandatory.

Preparation, activity, and output must be sanitized and bounded. They must not
expose absolute repository or executable paths, environment, tokens,
credentials, raw profile source paths, provider stderr, private Codex aliases,
internal authority handles, reviewed-commit handles, or raw backend errors.
Prefer relative or bounded public values.

The route itself has no business effect; the selected Tool owns the effect
boundary. Timeout, cancellation/disconnect, process failure, Desktop crash, or
lost response never implies rollback, and a possible effect is never replayed
only because host invocation lacked a terminal response.

## Future Windows certification

This ADR does not run certification. A future Windows live gate uses connected-
current Desktop with `repo.status` as the read-only case and `repo.create-
branch` as the effectful case, with a fresh disposable repository and branch,
never a Task 229C fixture. It proves backend eligibility, explicit host action,
typed input, review/confirm where required, revalidation, the same permission
gate, normal registry dispatch, underlying authority, exactly one execution,
`HostExplicit` provenance, no fake model request, correct result and
review/currentness processing, terminal handling, and cleanup. No model
request is required.

## Consequences

Already-authorized first-party Tools become reliably reachable by explicit
human action without relying solely on model selection. Host and model routes
share current-definition/permission semantics while capability authorization
remains unchanged. Costs include a new dispatch/provenance boundary, typed
backend descriptors, ticket/currentness handling, explicit concurrency and
Desktop lifecycle state, and a narrow first-release allowlist. External Tool
invocation remains deferred.

## Alternatives rejected

1. **Raw `ToolRegistry::execute` from Desktop:** bypasses reusable permission
   and current-definition checks.
2. **Duplicate Desktop permission checks:** creates security-semantic drift
   from runtime dispatch.
3. **Arbitrary JSON Tool console:** creates a generic execution surface with
   weak review semantics.
4. **Model text auto-execution:** model text is neither human intent nor
   authority.
5. **Fake model events:** host dispatch is not model dispatch.
6. **Required-tool Codex workaround:** certified Codex 0.149.0 has no
   established required-tool contract.
7. **Automatic replay after lost result:** possible effects cannot be assumed
   absent.
8. **External Tools by default:** admission or advertisement does not create
   safe generic host eligibility/review semantics.
9. **Immediate `repo.commit`:** reviewed commit authorization is separate and
   one-shot.

## Non-goals

ADR 0021 does not add repository, filesystem, branch-switching, generic
shell/process, generic Git/ref, arbitrary JSON, external provider, commit,
network MCP, ProviderManager/install/update, profile hot reload, required-tool
model support, Codex baseline migration, chat injection, persisted tickets,
post-start cancellation, retry, replay, compensation, rollback, OS sandboxing,
or network-isolation claims.

## Relationship to existing ADRs

ADR 0003 retains `Tool` / `ToolRegistry` as the extension boundary. ADR 0006
retains the Codex dynamic Tool bridge as the runtime/model route. ADR 0011
retains Trusted Profile as host authority composition, not provider
self-authority. The repository/content/index/history/ref ADRs retain every
capability-specific policy, and ADR 0020 retains unchanged underlying
`repo.create-branch` authority. ADR 0021 adds only the human-explicit dispatch
provenance, currentness, and permission route; it supersedes none of them.
