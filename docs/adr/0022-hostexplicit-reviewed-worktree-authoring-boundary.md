# ADR 0022 — HostExplicit Reviewed Worktree Authoring Boundary

Status: Accepted

Date: 2026-09-07

## Context

ADR 0012 authorizes the existing bounded `repo.patch` worktree-content
mutation. ADR 0021 defines the general HostExplicit dispatch, currentness,
permission, lifecycle, ticket, and provenance boundary. Task 246 established
the capability-specific review and preimage contract needed to make the
existing mutation reachable through a durable reviewed human workflow.

The workflow is intentionally narrower than generic authoring. The trusted
host remains the source of repository selection, capability composition,
permission policy, preimage identity, currentness, and execution authority.
Human input is an explicit request for one literal replacement; it is not a
Tool call, a permission grant, or a source of security-sensitive bindings.

## Decision

### 1. No new underlying authority

ADR 0012 remains the sole underlying `repo.patch` worktree-content mutation
authority.

ADR 0021 remains the general HostExplicit dispatch/currentness/D2/provenance
boundary.

ADR 0022 adds only a durable reviewed human workflow around the existing
`repo.patch` capability. It does not create, broaden, or compose a second
worktree mutation authority.

### 2. First v0.21 scope

HostExplicit authoring eligibility expands only to:

    repo.patch

and only the H1 human form:

    path
    expectedOldText
    replacementText

The human supplies a logical repository-relative path, nonempty literal old
text, and literal replacement text, which may be empty. The existing
model-visible `repo.patch` schema and bounded replacement form are unchanged.

The following remain disabled for HostExplicit authoring:

- `repo.create-file`;
- `repo.edit-files`;
- `repo.delete-file`;
- `repo.rename-file`;
- `repo.create-directory`;
- `repo.commit`; and
- external provider Tools, including MCP and Process Plugin Tools.

### 3. Preparation architecture

Adopt Task 246 P2/O2: a shared non-effectful `RepositoryPatchPreparer` in
`rah-tools`.

The preparer reuses the same `repo.patch` semantic primitives for:

- preimage admission;
- UTF-8 and BOM handling;
- literal matching;
- uniqueness and overlap checks;
- range calculation;
- postimage construction; and
- all existing limits.

The preparer is not a `Tool` and is not mutation-capable. It must not call
`Tool::execute`, create or replace a target, write the index, Stage, Commit,
alter refs, invoke an arbitrary Tool, or bypass `ToolRegistry`. No generic
`ToolRegistry` downcast and no generic mutation-preview API is introduced.
The narrow preparer handle is bound to the same host-selected first-party
composition and repository/Git identity as `repo.patch`.

### 4. Human versus host input

The human supplies only:

- `path`;
- old text; and
- replacement text.

The host derives and binds:

- `expected_file_sha256`;
- `expected_file_byte_length`;
- the canonical legacy `repo.patch` ToolInput;
- preimage and postimage identity;
- the complete `ToolDefinition`;
- permission policy;
- repository, composition, and currentness identity; and
- review identity.

The frontend cannot override or provide these values. Native paths or handles,
repository roots, Git identity, effect class, authority category, generations,
raw ToolInput, and other security-sensitive bindings remain host-only.

The canonical H1 ToolInput contains exactly the existing single-replacement
fields: `path`, host-derived `expected_file_sha256`, host-derived
`expected_file_byte_length`, human `expected_old_text`, and human
`replacement_text`. The complete-file digest and byte length cover the exact
raw preimage, including BOM and all newline bytes.

### 5. Prepare

Prepare is zero-effect. It requires, in order:

1. coordinator `Idle` and no active model turn;
2. connected-current composition;
3. backend `repo.patch` eligibility;
4. D2 preflight for the expected definition and permission;
5. typed H1 validation;
6. shared non-effectful patch preparation;
7. exact review construction; and
8. an opaque ticket.

Prepare performs no Tool execution and no native replacement attempt. It also
does not write the index, Stage, Unstage, Commit, refs, or HEAD, activate a
provider, inject conversation content, or start a model lifecycle. Bounded
read-only repository and Git observation is allowed.

Prepare rejects the existing `repo.patch` unsupported or stale states,
including absent, untracked, staged or index-divergent, sparse, conflicted,
linked, reparse, special-file, binary, malformed, oversized, absent-match,
multiple-match, duplicate, and overlapping cases. A verified no-op is handled
under Task 246 N2: equal old and replacement text fails Prepare with a bounded
`no_effect`/`nothing_to_change` refusal, creates no confirmation ticket, and
does not expose an effectful activity. Prepare success is not future authority.

### 6. Review

Use Task 246 R4 exactly. The review contains:

    complete changed range
    + complete escaped old text
    + complete escaped replacement text

It also identifies operation `repo.patch`, the bounded relative path, one
replacement, the intended effect and non-effects, and that the representation
is display-only. It is not unified-diff execution, `git apply`, a fuzzy patch,
line/range execution, or any other execution format. Post-confirm
`repo.diff` remains the inspection path; it is never the patch executor.

Escaping is deterministic and display-only: CR is `\\r`, LF is `\\n`, TAB is
`\\t`, other C0 controls and DEL use `\\u{...}`, and bidi, zero-width, and
format characters use `\\u{...}`. Trailing spaces are explicit. A leading BOM
is shown as `BOM U+FEFF (preserved; excluded from matching)`. Final-newline
state and EOF are explicit. There is no Unicode normalization, case folding,
or newline normalization; CRLF, LF, and CR remain distinct.

The hard rule is that there is no hidden mutation-relevant changed content.
All exact changed text, byte ranges, invisible-character markers, and
newline/final-newline state must fit the review bounds. Only unchanged
surrounding context may be omitted, and the review must say when it was
omitted. If exact changed content does not fit, Prepare fails closed with
`review_too_large` and creates no ticket. Clipped text, ellipses, or redactions
cannot remain confirmable.

Task 246’s bounds are preserved: Prepare IPC serialized DTO 64 KiB; path 1024
UTF-8 bytes; each text 64 KiB; aggregate old plus replacement text 64 KiB;
one replacement; file and postimage 1 MiB; escaped changed material 192 KiB;
optional unchanged context 32 KiB; complete serialized review 256 KiB; ticket
stored representation 512 KiB; and terminal activity/result metadata 32 KiB.
The changed-material and complete-review measurements occur after safe
escaping. These bounds do not increase the existing `repo.patch` limits.

### 7. Ticket

Reuse ADR 0021 ticket principles and Task 246 bindings. A ticket is opaque,
RAH-generated, in-memory, process-local, single-use, non-persistent, and
bounded by a five-minute TTL, inclusive at the TTL boundary. Restart discards
it. It is not authority by itself.

The ticket binds the exact canonical ToolInput, review identity, complete
review, preimage and postimage evidence, repository and target identity,
ToolDefinition, allowed permission policy, composition and currentness values,
preparer identity, and the repository/Git state required by ADR 0012. This
includes public Tool name, definition identity, repository/model/profile/
connection generations, Effective Authority identity, root/parent/target
identity evidence, and preimage/postimage SHA-256 and byte lengths.

Confirm receives the ticket ID only. It does not resend path, text, hashes,
lengths, Tool name, or JSON. The backend trusts the ticket state, never
frontend review content. There is no automatic refresh, reprepare, expected-
hash rewrite, retry, replay, or execution against the latest file.

### 8. Confirm and execution

Confirm revalidates host workflow state before Tool start. The required route
is:

    ticket validation
    -> coordinator/currentness/eligibility
    -> authorize_tool_dispatch
    -> HostExplicit Started
    -> authorized_tool_dispatch
    -> ToolRegistry
    -> existing repo.patch
    -> ADR 0012 policy

Before HostExplicit `Started`, `authorize_tool_dispatch` performs the D2
preflight. Missing Tool, changed name, incomplete or changed definition, or
permission rejection therefore causes zero Tool execution and releases the
coordinator. After `Started`, the owned task calls
`authorized_tool_dispatch` with the exact canonical ToolCall and current host
inputs. The current registry definition and allowed permission are checked
again before ordinary Tool execution.

The registered `repo.patch` Tool independently reparses and revalidates the
repository, path, target, preimage, literal matching, repository lease, final
identity, one-attempt limit, and postimage semantics immediately before its
single possible native replacement attempt. Raw `ToolRegistry::execute` is
not sufficient host authorization.

### 9. Concurrency and provenance

Reuse ADR 0021’s coordinator states:

    Idle
    ModelTurn
    HostPrepared
    HostRunning

There is no model-turn overlap, wait queue, or read-only parallelism. One
prepared or in-progress HostExplicit invocation is permitted. Prepared
unstarted work may be cancelled; no active post-start HostExplicit abort is
added.

HostExplicit remains Desktop-private provenance. Do not synthesize
`AgentEvent::ToolRequested`, `AgentEvent::ToolStarted`, or
`AgentEvent::ToolFinished`, and do not inject a conversation message or
ToolOutput into model context.

### 10. Effect and reviewed-commit authorization

The only intended user effect is that one existing tracked regular file’s
contents may change according to the existing `repo.patch` capability. No
index, HEAD, branch, ref, history, other user file, provider/profile,
conversation, model, shell, process, or Git state may be intentionally
mutated by this boundary. The same-directory temporary file remains an
ADR 0012 implementation detail, not file-creation authority.

Preserve the current Desktop rule identified by Task 246: once an effectful
`repo.patch` HostExplicit execution reaches `Started`, reviewed commit
authorization is cleared conservatively. Prepare is zero-effect; it preserves
existing reviewed commit state but does not manufacture or arm commit
authority. ADR 0016 remains a separate bounded reviewed index/history
authority. A successful patch does not increment `repository_generation` just
for changed bytes, although observation and workflow state may refresh.

### 11. Failure and uncertainty

Use the exact existing `repo.patch` result taxonomy documented by Task 246. A
valid outcome contains exactly one JSON content item with exactly
`status`, `changed`, `uncertain`, and `reason`:

| status | changed | uncertain | is_error | reason |
| --- | ---: | ---: | ---: | --- |
| `precondition_failed` | false | false | true | one redacted class: `path_or_filesystem`, `repository_state`, `precondition`, `temporary`, or `replacement` |
| `ok` | true | false | false | `none` |
| `replacement_failed_known` | false | false | true | `replacement` |
| `uncertain` | false | true | true | one redacted failure class |

Input parsing may return `ToolError::InvalidInput` before an outcome exists,
and execution may return a distinct `ToolError`. The host-side classifier
strictly validates output: missing or extra keys, wrong types, unknown
statuses/reasons, malformed future hashes or lengths, and contradictory flags
are rejected. After `Started`, malformed, lost, contradictory, or errored
results are conservative possible-effect/uncertain handling. Success is never
inferred merely because the file appears changed.

ADR 0012 independently determines known failure, verified success, or
uncertainty from its preimage/postimage and identity observations. Its native
replacement system call remains the mutation commit point. No retry, replay,
rollback, restore-preimage, compensation, or automatic second confirmation is
allowed. Prepared cancellation before start remains supported. A started
operation with a lost result requires fresh observation and is not treated as
no effect.

### 12. Source-content handling

Full reviewed old and new source material may exist only in bounded
process-local prepared state and the local user-visible review as defined by
Task 246. Complete preimage bytes may be held transiently while deriving
hashes and the postimage, but they are not persisted to conversation SQLite,
generic activity history, conversation replay, or evidence JSONL.

Full source text must not be emitted into diagnostic or live-evidence logs.
Logs and activity retain only bounded, redacted metadata such as relative
metadata, review identity, hashes/lengths, result class, and sanitized Tool
result. Absolute/native paths, temporary names, executables, environment,
credentials, provider stderr, aliases, and authority objects remain excluded.

## Security nonclaims

This ADR explicitly provides no:

- generic `fs.write`;
- arbitrary full-file write;
- file create, delete, or rename;
- directory creation or multi-file edit;
- regex or fuzzy patch;
- executable unified diff or `git apply`;
- Stage or Commit authority;
- branch switching or arbitrary ref/history authority;
- shell, process, or generic Git authority;
- retry, replay, rollback, restore, or compensation;
- OS sandbox claim;
- network isolation claim; or
- race-free TOCTOU claim.

It also does not authorize binary or staged-file editing, provider activation,
network MCP, generic JSON Tool invocation, or an arbitrary provider schema.
Process supervision and fixed subprocess mechanics do not become an OS
sandbox, and absence of intended network authority is not a network-isolation
claim.

## Consequences

RAH gains a durable, exact human review route to the already-existing bounded
`repo.patch` capability. The route makes literal matching, complete changed
content, preimage identity, currentness, permission, D2 dispatch, and
conservative uncertainty visible as one host-owned workflow without creating
generic authoring authority.

The costs are a capability-specific non-effectful preparer, exact escaped
review and bounded process-local ticket state, another currentness and
provenance path to validate, strict result classification, and continued
cross-platform evidence requirements. Future implementation remains separate:
Task 248 may build the shared preparation foundation, followed by Desktop
integration, review/result hardening, live certification, and milestone audit.

## Relationships

- ADR 0012 is the mutation authority. It independently owns repository
  admission, exact preimage checks, one bounded replacement attempt, postimage
  proof, and conservative uncertain-effect handling.
- ADR 0021 is the general explicit-host dispatch boundary. It owns the
  HostExplicit route, currentness, D2 definition/permission checks, ticket and
  coordinator principles, and Desktop-private provenance.
- ADR 0022 is the exact reviewed HostExplicit worktree-authoring boundary. It
  adds the H1 review/preparation/ticket workflow for `repo.patch` and no new
  underlying authority.
- ADR 0016 remains separate reviewed index/history commit authority. A patch
  does not imply Stage or Commit, and Commit does not imply worktree authoring.

ADR 0021 is not rewritten. v0.20 remains historical, and worktree authoring
had deliberately been deferred there pending capability-specific review,
preimage, and effect research. ADR 0022 records the later, narrower decision
after Task 246 completed that research; it supersedes none of the earlier ADRs.

## Implementation boundary

This ADR is documentation-only. It authorizes no Rust, Desktop, frontend,
Tool-schema, Cargo, dependency, release, workflow, or live-certification
change. Implementation must preserve the exact Task 246 contract and requires
its own task-level validation and evidence.
