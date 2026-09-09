# ADR 0024: HostExplicit Reviewed New-File Authoring Boundary

Status: Accepted

Date: 2026-09-09

## Context

ADR 0013 accepts the narrow underlying `repo.create-file` capability: one
exclusive create-new attempt for one bounded UTF-8 regular file at one
validated repository-relative path. ADR 0021 defines the general
HostExplicit dispatch, currentness, D2, ticket, lifecycle, and provenance
boundary. ADR 0022 and ADR 0023 define capability-specific reviewed workflows
for existing-file replacement and existing-file multi-file editing.

Task 269 established that new-file review is a separate capability-specific
contract. A new-file review must establish target absence, show complete new
content, bind an existing parent, preserve the exclusive-create and partial
effect semantics of ADR 0013, and keep source content out of generic activity
and persistence. The research verdict was:

> A — NEW-FILE REVIEW CONTRACT SOUND — CAPABILITY-SPECIFIC ADR MAY BE
> ACCEPTED

This decision accepts that reviewed HostExplicit workflow boundary only. It
does not implement the workflow, make the Tool eligible, or amend any prior
ADR.

## Decision

### 1. Reviewed workflow and authority relationship

Accept the following future capability-specific boundary:

> A connected-current Desktop human may eventually prepare, completely review,
> and explicitly confirm one bounded new-file creation through the existing
> `repo.create-file` Tool.

The required route is:

```text
closed typed human {path, content}
 -> zero-effect creation Prepare
 -> complete bounded backend-derived review
 -> opaque single-use ticket
 -> creation-specific currentness revalidation
 -> D2
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.create-file
 -> ADR 0013 RepositoryFileCreationPolicy
 -> strict result classification / descriptive refresh
```

ADR 0024 owns this reviewed HostExplicit workflow boundary. ADR 0013 remains
the sole underlying file-creation mutation authority. Acceptance is the
required boundary for future HostExplicit reviewed `repo.create-file`
implementation; it is not implementation completion, live certification,
production enablement, or release completion.

The distinction is normative:

- a model request is not authorization;
- human review is not underlying mutation authority;
- HostExplicit eligibility is not underlying mutation authority;
- `PermissionLevel` is an outer dispatch classification only;
- the frontend is presentation and input only;
- Trusted Profile composition does not create this HostExplicit route;
- provider metadata cannot self-enable HostExplicit;
- ADR 0024 does not create filesystem authority;
- ADR 0024 does not alter the ordinary model/runtime `repo.create-file` Tool
  path; and
- `Tool` and `ToolRegistry` remain the extension and dispatch boundary.

The reviewed route must dispatch through the existing `ToolRegistry`. No
direct filesystem mutation shortcut is accepted.

### 2. Closed human Prepare request

The capability-specific human Prepare request contains exactly:

```json
{
  "path": "src/new_module.rs",
  "content": "..."
}
```

Only `path` and `content` are human input. Unknown fields fail closed. The
human does not provide:

- repository root;
- absolute or native path;
- Git executable;
- parent identity or target identity;
- hashes or byte lengths;
- file mode;
- permission;
- `ToolName` or `ToolInput`;
- provider or model metadata;
- authority flags;
- retry controls;
- temporary path;
- Stage or Commit instructions; or
- any other mutation or currentness material.

All other mutation, currentness, identity, and authority material is derived
by the host.

### 3. Request and content bounds

The reviewed route preserves the ADR 0013 and Task 269 bounds, or may narrow
them for this route, but may never widen ADR 0013 authority implicitly:

- exactly one file;
- logical path: 1--1024 UTF-8 bytes;
- content: 0--262144 bytes inclusive;
- NUL is rejected;
- empty content is allowed;
- serialized request: maximum 327680 bytes;
- no implicit encoding conversion;
- no BOM insertion or removal;
- no newline conversion;
- no Unicode normalization;
- no template expansion;
- no append; and
- no overwrite.

The path remains a validated repository-relative logical path. The existing
creation policy rejects unsafe path syntax, repository metadata, namespace
aliases, links and reparse traversal, unsupported sparse state, submodule
boundaries, and other disallowed targets. The reviewed route does not turn
these bounds into generic filesystem-write authority.

### 4. Complete backend-derived review

Prepare must construct a complete backend-derived review that is sufficient
for the human to understand the exact new file. It is a review surface, not a
preview, and it is display-only. The serialized complete review is bounded to
262144 bytes inclusive after deterministic safe escaping and inclusion of all
required review metadata.

If the complete mutation-relevant review exceeds that bound, Prepare fails
with:

```text
review_too_large
```

No ticket is issued in that case. The route never authorizes from a preview,
truncated content, ellipsized content, or hidden mutation-relevant content.

The review must communicate at least:

- operation = `repo.create-file`;
- exactly one target;
- repository-relative logical path;
- target currently absent;
- safe existing parent;
- target absent from HEAD and the index under the retained creation
  contract;
- expected post-effect state = one new untracked regular file;
- complete deterministically escaped new-file content;
- host-derived content byte length;
- host-derived SHA-256;
- relevant BOM, newline, control-character, and format-character facts needed
  to understand the exact bytes;
- regular, non-executable-file intent;
- exclusive create-new and no-clobber semantics;
- no overwrite;
- no parent creation;
- no Stage;
- no Commit;
- no index, HEAD, ref, or history mutation;
- exclusive name acquisition is the mutation commit point;
- content creation is not crash-atomic;
- a write failure may leave an empty or partially written new file;
- cancellation, timeout, or disconnect is not rollback;
- no automatic cleanup; and
- no retry or replay.

Escaping is deterministic display encoding only. It must not normalize,
convert, or otherwise change the bytes that the Tool will receive. Native and
absolute paths and authority-bearing identities remain private backend state.

### 5. Existing parent only

The parent directory must already exist. The reviewed route does not authorize
`mkdir`, recursive parent creation, or structural namespace management.

Parent traversal remains bounded to safe ordinary directories within the
selected repository according to the ADR 0013 creation policy. Every
traversed component, including the root and immediate parent, must satisfy the
policy's containment, ordinary-directory, non-link, and non-reparse admission.
Directory creation remains a separate authority family under its own
authority and is not implied by this decision.

### 6. Target absence and currentness of absence

The reviewed workflow binds and revalidates target absence. The accepted
contract covers absence from all relevant forms, including:

- regular files, directories, links, broken links, reparse points, and other
  filesystem entry forms;
- case-equivalent and other supported namespace-alias targets;
- HEAD;
- every relevant index state;
- intent-to-add entries;
- unmerged entries;
- ignored targets;
- submodule boundaries; and
- unsupported sparse state.

The contract does not claim race-free absence or race-free TOCTOU behavior.
Immediately before native effect, absence and currentness are revalidated.
ADR 0013's exclusive create-new operation remains the final no-clobber target
name acquisition and protects against overwriting an entry that wins the
native race.

### 7. Native mutation commit point and effects

The native mutation commit point is direct exclusive create-new, equivalent in
intent to:

- Unix `O_CREAT | O_EXCL`; and
- Windows `CREATE_NEW`.

Exclusive name acquisition means only that the target name was acquired
without clobbering an existing entry. It does not mean that the complete
content write is atomic or crash-atomic. Writing, flushing, closing, and
verification happen after name acquisition. If writing, flushing,
verification, cancellation, disconnect, or process execution fails after
acquisition, an empty or partial new file may remain.

The reviewed route introduces no rollback authority. It must preserve the
ADR 0013 rule that an effect that may have occurred is not automatically
reversed, retried, or replayed.

### 8. Exact result classes

The reviewed route reuses ADR 0013's result vocabulary exactly:

```text
ok
invalid_target
precondition_failed
create_failed_known
write_failed_known
uncertain
```

It must not import result vocabulary from `repo.patch` or `repo.edit-files`,
including `failed_known_no_effect` or `partial_effect`. The HostExplicit layer
may validate and present these six classes, but it does not redefine them.

`ok` requires exact post-effect verification. `invalid_target` and
`precondition_failed` are pre-effect rejection classes when the relevant
precondition is proven. `create_failed_known` is reserved for proven no RAH
creation effect. `write_failed_known` is a known attributable partial effect.
`uncertain` is used whenever the effect or its exact classification cannot be
proven.

#### `write_failed_known`

`write_failed_known` is not a known-no-effect result. It may mean all of the
following are proven:

- exclusive create-new succeeded;
- a new target exists;
- only a bounded partial state was written; and
- that state is known and attributable to this attempt.

The route must not automatically delete the target, restore anything, retry
writing, replay the Tool, create again, Stage, or Commit. The partial file is
retained. Deleting it would require separate deletion authority.

#### `create_failed_known`

The interpretation is conservative. A native create error may be reported as
`create_failed_known` only when post-observation proves that this RAH attempt
produced no creation effect. A native error category alone is insufficient.
If no-effect cannot be proven, the result is `uncertain`.

This is an explicit future implementation prerequisite.

#### `uncertain`

`uncertain` covers, among other causes:

- a lost native result;
- failed verification;
- parent or target identity change;
- timeout;
- cancellation;
- disconnect;
- crash;
- write or flush uncertainty; and
- failed post-observation after creation may have occurred.

Unknown stays unknown. There is no retry, replay, automatic second create,
automatic delete, rollback, or inference that the repository is unchanged.
The ticket remains consumed. Descriptive refresh cannot upgrade an uncertain
effect into `ok` or create a new authorization.

### 9. Zero-effect Prepare

Preparation is required to perform:

- 0 Tool executions;
- 0 native create attempts;
- 0 target creation;
- 0 repository temporary mutation artifacts;
- 0 parent creation;
- 0 worktree mutation;
- 0 index mutation;
- 0 HEAD, ref, or history mutation;
- 0 Stage;
- 0 Commit;
- 0 model or runtime execution;
- 0 MCP execution;
- 0 Process Plugin execution; and
- 0 durable authority persistence.

Non-effectful host reads, hashing, Git observation, and identity inspection
are permitted. Prepare may retain bounded private state in memory for the
pending review, but it does not use repository-adjacent temporary files as a
substitute for a non-effectful preparer. Failed Prepare produces no ticket.

### 10. Host-derived currentness state

The future implementation must retain enough creation-specific state to
revalidate immediately before effect. At minimum it must bind and recheck:

- selected repository identity;
- canonical repository root;
- host-selected Git executable identity where required;
- exact Tool name;
- complete `ToolDefinition`;
- current permission membership;
- `ToolRegistry` identity;
- composition identity;
- Effective Authority identity;
- relevant repository, model, profile, and connection generations;
- capability-specific preparer identity;
- review identity;
- safe parent identity;
- target absence identity and observations;
- HEAD, index, and ref observations;
- ignore, submodule, and sparse admission;
- exact content and review binding; and
- canonical `ToolInput`.

Name-only Tool equivalence is insufficient. Any relevant definition,
permission, repository, parent, target, registry, composition, authority,
generation, preparer, review, Git, content, or canonical-input drift fails
closed before Tool execution.

The reviewed workflow does not claim that current path-text-oriented retained
values are identity-backed proof. The identity requirements are future
implementation obligations.

### 11. D2 and registry dispatch

The accepted sequence remains:

```text
ticket-only Confirm
 -> creation-specific revalidation
 -> D2 complete current-definition / authority check
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.create-file
 -> ADR 0013
```

The implementation must reject any architecture using:

- direct `Tool::execute`;
- direct `std::fs` creation from Desktop;
- frontend filesystem APIs;
- live-test-only mutation paths;
- generic Tool JSON invocation; or
- model substitution.

D2 and `authorized_tool_dispatch` must use the existing current definition,
permission, registry, composition, and host-owned Tool inputs. HostExplicit
does not create a parallel registry or bypass the ordinary Tool extension
boundary.

### 12. Ticket contract

The authority ticket reuses ADR 0021 principles. It is:

- RAH-generated;
- opaque;
- capability-specific;
- process-local;
- in-memory;
- nonserializable as durable authority;
- single-use;
- exact-preparation and currentness-bound; and
- valid for an inclusive five-minute TTL, unless a future accepted ADR
  explicitly changes the general HostExplicit contract.

Confirm receives only the ticket identity. It receives no path, content,
hash, Tool JSON, repository, permission, provider, model, or authority fields.
There is no ticket persistence or resume and no automatic replacement ticket
after consumption, expiry, or drift.

The ticket is not filesystem authority, permission, or a durable capability.
Restart, disconnect, cancellation, expiry, recomposition, or currentness
drift discards it without replay or replacement.

### 13. Ticket and activity privacy

The authority ticket and generic activity correlation ID are different
values. The activity ID:

- is generated independently;
- is not the ticket;
- is not derived from the ticket;
- cannot Confirm; and
- cannot Cancel.

Generic activity must not contain the actual ticket value under any alternate
key. Future deterministic tests must use value-level sentinels, including
alternate-key attempts such as `invocationId`, rather than checking only
prohibited field names.

HostExplicit activity remains source-distinct from model/runtime lifecycle.
The route must not create model Tool lifecycle events, fake model turns, chat
continuation, or authority-bearing activity.

### 14. Source and content privacy

Complete new-file content is allowed only in the bounded local review surface
required for human authorization and in private retained preparation state.
It must not enter generic:

- `HostActivityEvent`;
- conversation transcript;
- persistence;
- telemetry;
- logs;
- provider metadata; or
- generic Tool activity.

Generic surfaces must exclude complete source content, content sentinels, raw
ToolInput, source-bearing ToolOutput, native or absolute paths, native parent
identities, and the actual authority ticket. Hashes and other source-derived
metadata that current policy treats as source-sensitive must follow that
policy consistently.

The complete local review is rendered as untrusted presentation data. The
frontend does not infer authority, construct raw Tool JSON, persist the
review, or accept a native path.

### 15. Commit-authorization interaction

Creation is a repository-bound authoring operation. If a confirmed
`repo.create-file` reaches HostExplicit `Started`, existing repository-bound
reviewed Commit authorization is invalidated. This also applies to an owned
post-`Started` rejection where the established authoring precedent requires
invalidation.

Post-operation repository refresh is descriptive only. It does not authorize
Commit, Stage the new file, automatically construct a new-file authorization,
or automatically rerun creation.

An untracked created file may require status/file-info and host hash/content
verification because ordinary Git worktree diff does not necessarily expose
untracked content.

### 16. No automatic Stage or Commit

The reviewed new-file workflow ends with a worktree/untracked file effect
only. It never automatically:

- runs `git add`;
- Stages;
- Commits;
- mutates history; or
- mutates refs.

Existing Stage/Unstage and reviewed Commit workflows remain separate explicit
authority paths.

### 17. Explicit exclusions

ADR 0024 does not authorize:

- generic `fs.write`;
- arbitrary filesystem paths;
- overwrite;
- append;
- multiple-file creation;
- directory creation;
- delete;
- rename or move;
- chmod or executable-file selection;
- arbitrary ACL selection;
- binary-write APIs;
- Stage;
- Commit;
- Git history or ref mutation;
- shell;
- generic process execution;
- network;
- MCP HostExplicit;
- Process Plugin HostExplicit;
- provider-defined HostExplicit;
- arbitrary Tool JSON;
- model-selected HostExplicit authority;
- ticket persistence or resume;
- retry or replay; and
- rollback or cleanup/deletion of partial files.

`repo.create-file` remains an ordinary Tool capability governed by ADR 0013
when its existing model/runtime path is configured. This ADR does not alter
that path.

### 18. Current HostExplicit allowlist remains unchanged

Architectural acceptance is distinct from implementation and eligibility. At
the end of Task 270, the production HostExplicit allowlist remains exactly
the v0.22 eight:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
```

`repo.create-file` remains not HostExplicit eligible after Task 270. ADR
acceptance alone never enables the Tool. A later backend implementation task
may add eligibility only after the required preparation and currentness
foundations exist.

### 19. Known implementation prerequisites

Task 269 found that current implementation and preparation primitives are
insufficient for the reviewed currentness claim. Before implementation can
claim compliance with this ADR, future work must provide identity-backed
proof and revalidation for:

- safe existing parent identity;
- traversed parent safety;
- target absence and alias observations; and
- target and parent drift.

The current path-text-oriented retained values do not already satisfy this
requirement. This ADR records the prerequisite and does not fix it.

The current native create-error handling is also too broad for this reviewed
contract when native error category alone is treated as known no effect.
Before compliance can be claimed, future implementation must ensure that:

- `create_failed_known` requires post-observation proof of no RAH creation
  effect; and
- otherwise the result is `uncertain`.

This ADR records the prerequisite and does not fix production code in Task
270.

### 20. Future implementation sequencing

Non-normatively, subsequent implementation should follow the roadmap order:

1. Task 271: shared non-effectful creation preparation and revalidation,
   including the required identity/currentness foundations and any prerequisite
   core hardening needed for truthful result classification.
2. Task 272: deterministic HostExplicit backend integration and eligibility.
3. Typed Desktop review workflow.
4. Fault and privacy hardening.
5. Windows connected-current live certification.
6. Milestone audit.
7. Release sequence.

Task 270 does not start any of these tasks.

### 21. Consequences

Positive consequences:

- closes the major new-file authoring gap;
- reuses the existing narrow ADR 0013 authority;
- keeps human authorization capability-specific;
- preserves the ToolRegistry and D2 architecture; and
- allows complete human review of bounded content.

Costs and risks:

- complete content review can fail `review_too_large`;
- identity and currentness implementation is nontrivial;
- create-new is not complete-content atomicity;
- partial created files are possible;
- uncertainty requires conservative UX;
- source content requires strict privacy separation; and
- Windows identity and race behavior requires dedicated deterministic and live
  evidence.

### 22. Relationship to existing decisions

ADR 0013 remains the sole underlying `repo.create-file`
`RepositoryFileCreationPolicy` authority and its exact six result classes.
ADR 0024 does not reinterpret ADR 0013 as already providing a reviewed
HostExplicit boundary.

ADR 0021 remains authoritative for general HostExplicit dispatch, D2,
connected-current composition, ticket principles, currentness, lifecycle,
and provenance. ADR 0024 adds no generic dispatch or permission authority.

ADR 0022 remains authoritative for the reviewed HostExplicit `repo.patch`
workflow, and ADR 0023 remains authoritative for the reviewed HostExplicit
`repo.edit-files` workflow. Their result vocabularies and underlying
authorities are not imported into this capability.

This ADR does not modify ADR 0013, ADR 0021, ADR 0022, or ADR 0023. It does
not change frontend behavior, Cargo or dependencies, package versions, the
HostExplicit allowlist, or ordinary model/runtime behavior.

### 23. Nonclaims

This decision does not claim:

- race-free TOCTOU;
- atomic complete-file creation;
- rollback;
- recovery;
- automatic cleanup;
- Unix or macOS production live certification for the future HostExplicit
  route;
- model-selected HostExplicit certification;
- OS sandboxing; or
- network isolation.

Process supervision remains distinct from OS sandboxing.

## Acceptance

Accepted as the required boundary for future HostExplicit reviewed
`repo.create-file` implementation. Task 270 accepts the architecture and
workflow contract only. It does not state that the workflow is implemented,
live certified, production enabled, or release complete.
