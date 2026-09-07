# Task 246 — HostExplicit Worktree Authoring Review Contract Research

## Status

RESEARCH COMPLETE — AWAITING EXACT-HEAD CI.

This task is research/documentation only. It changes no Rust, tests, Desktop or
frontend, Cargo/dependencies, Tool schema, ADR, version, release metadata,
workflow, or authority. It runs no live effects, model prompts, or model
experiments. Task 247 is not started automatically.

## Starting checkpoint

The portable checkpoint was resolved dynamically from Git:

- repository: `spider2449/rust-agent-harness`;
- origin: `https://github.com/spider2449/rust-agent-harness.git`;
- `HEAD == origin/master == 84b57a402dcec92397441d7139ba41be29f907b7`;
- commit: `docs: define RAH v0.21 scope and authority roadmap`;
- Task 245 exact-head CI: `34094698417 PASS`;
- worktree: clean;
- released version: `v0.20.0`;
- workspace: 13 packages, all `0.20.0`, edition 2024, no dependency drift;
- accepted ADRs: 0001 through 0021.

No drive, checkout path, username, environment directory, machine name, or Git
executable path is part of this contract.

## Task 245 selected milestone

Task 245 selected A1 — HostExplicit Reviewed Single-File Patch Authoring. The
product sequence is:

    inspect -> Prepare reviewed patch -> Confirm -> inspect diff -> Stage -> reviewed Commit

A1 is a human route to the already-existing `repo.patch` capability. It adds a
typed host-only review/dispatch workflow, not a new worktree mutation authority.
The other authoring Tools, branch switching, create-and-switch, external
HostExplicit, network MCP, and generic JSON Tool invocation remain deferred.
HostExplicit should precede branch switching because A1 does not change HEAD,
the index, refs, or history.

## Existing ADR 0012 authority

ADR 0012 remains the underlying authority. Its private
`RepositoryWorktreeMutationPolicy` authorizes one bounded replacement in one
existing tracked regular worktree file. It excludes generic filesystem write,
arbitrary full-file replacement, creation, deletion, rename, binary editing,
index mutation, commit/history/ref mutation, shell/process authority, rollback,
and automatic replay.

The trusted host owns the canonical repository root, Git identity, limits,
temporary paths, and private preimage/audit state. `PermissionLevel::Execute`
is only an outer dispatch category. A model Tool request is untrusted; the
private policy remains the mutation authority.

## Existing repo.patch implementation

The primary implementation is `crates/rah-tools/src/repository_worktree_patch.rs`.
`RepositoryWorktreePatchTool` is host-constructed with a fixed Git executable
and repository root. It exposes `repo.patch` with `PermissionLevel::Execute` and
a closed `oneOf` schema. The private policy owns the repository lease, Git and
filesystem identity checks, preimage capture, postimage construction, temporary
file, native replacement, and result conversion.

The current sequence is: parse input; acquire the repository lease; capture and
validate preimage; build the complete postimage; write, flush, and validate a
same-directory temporary; revalidate immediately before replacement; perform
at most one native replacement; and verify the exact postimage. Windows uses
`MoveFileExW` with replace/write-through flags. Unix preserves the existing
mode and renames the temporary file.

Private `MutationEvidence` contains target identity, preimage SHA-256/length,
postimage SHA-256/length, and outcome class. The current public ToolOutput does
not serialize this evidence or a target path. A1 must not invent a second
public patch-result schema.

## Existing HostExplicit boundary

ADR 0021 defines HostExplicit as a structurally distinct Desktop action by a
trusted human. It selects only a backend-eligible Tool already in the
connected-current host composition. It is not model/runtime dispatch and is
not capability authorization.

The current `rah-desktop/src/host_invocation.rs` and `main.rs` provide `Idle`,
`ModelTurn`, `HostPrepared`, and `HostRunning`; one prepared action; a
five-minute in-memory process-local ticket; typed `deny_unknown_fields` DTOs;
generation, repository fingerprint, composition, definition, and permission
binding; D2 preflight plus execution revalidation; ticket-ID-only Confirm and
Cancel; private `host_explicit` activity; no post-start abort; and no retry,
replay, compensation, rollback, or persistence.

The v0.20 HostExplicit allowlist remains `fs.read`, `repo.file-info`,
`repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.create-branch`.
`repo.patch` is not currently eligible. Task 246 changes no allowlist. The
frontend currently has typed read and branch forms, but no patch form or
arbitrary JSON console. Effective Authority is presentation only.

## Current repo.patch input contract

The current public schema has two mutually exclusive forms. Both have `path`,
`expected_file_sha256`, and `expected_file_byte_length`.

The single form has `expected_old_text` and `replacement_text`; the bounded form
has `replacements`, an array of one through sixteen objects with those two
fields. Unknown fields and mixed/partial forms are rejected. Empty replacement
text is literal removal; empty old text is rejected.

The current limits are:

- serialized request: 64 KiB;
- path: 1024 UTF-8 bytes;
- each old or replacement text: 64 KiB;
- aggregate replacement text: 64 KiB;
- file and expected byte length: 1 MiB;
- replacements: 1 through 16.

The parser validates lowercase 64-hex SHA-256, nonnegative bounded length,
logical path, strict string fields, no NUL, no BOM in replacement text, and
closed fields. These limits and the model-visible Tool schema are unchanged by
Task 246.

## Current precondition contract

The target is one existing regular file beneath the canonical host-selected
root in a normal non-bare repository. The implementation requires a real
repository root and directory `.git`; linked-worktree metadata is unsupported.
Root, parent, target, and relevant file identities/attributes are checked;
links/reparse points, unsupported special files, and hard-linked targets are
rejected.

Fixed Git observations require a successful root/non-bare check, existing HEAD,
exactly one regular HEAD tree entry for the logical target, exactly one normal
stage-0 regular index entry, equal HEAD and index entries, a normal index tag,
and a complete refs observation. `diff-files --quiet` requires the target to
be clean relative to the index. This is target cleanliness, not global
worktree cleanliness.

The raw target is bounded, NUL-free, strict UTF-8, and exactly equal to the
expected complete-file SHA-256 and byte length. ADR 0012 also excludes
untracked, staged, intent-to-add, unmerged, sparse/skip-worktree,
submodule/gitlink, nested-repository, link/reparse, binary, malformed, and
unsupported special-file cases. The future preparer must use this same
supported subset and cannot widen it.

## Current mutation commit point

The native replacement system call is the mutation commit point. Before it,
known no effect requires proof that the captured preimage remains intact and
temporary cleanup is proven. After it, the API return alone is insufficient.

Success requires exact postimage bytes plus target/parent/root identity and
unchanged Git state. A replacement failure is known failure only when fresh
post-observation proves the preimage intact. Any incomplete or contradictory
observation, target delta, identity change, cleanup ambiguity, timeout,
disconnect, crash, or lost result is uncertain. HostExplicit Prepare must stay
before this boundary; Confirm adds no replacement primitive.

## Current result taxonomy

`rah-protocol::ToolOutput` has `content: Vec<ToolContent>` and `is_error`.
The current `repo.patch` outcome contains exactly one JSON content item with
exactly `status`, `changed`, `uncertain`, and `reason`:

| status | changed | uncertain | is_error | reason |
| --- | ---: | ---: | ---: | --- |
| `precondition_failed` | false | false | true | one redacted class: `path_or_filesystem`, `repository_state`, `precondition`, `temporary`, or `replacement` |
| `ok` | true | false | false | `none` |
| `replacement_failed_known` | false | false | true | `replacement` |
| `uncertain` | false | true | true | one redacted failure class |

Input parsing may return `ToolError::InvalidInput` before an outcome exists;
execution may return a distinct `ToolError`. The private evidence has no public
target/path/hash/length fields. A future strict classifier must reject missing
or extra keys, wrong types, unknown statuses/reasons, malformed future hashes
or lengths, and contradictory flags. It must not infer success from a changed
target observation.

## Human input decision

Select H1: exactly one replacement pair. The future closed DTO is exactly
`{ path, expectedOldText, replacementText }`, with camelCase fields and no
extra fields. The human controls only the logical repository-relative path,
nonempty literal old text, and literal replacement text, which may be empty.
Confirm controls only the ticket ID.

The existing model-visible `replacements` array remains unchanged but is not
part of the first HostExplicit UI. H1 gives the clearest review and simplest
live proof while retaining the underlying same-original-preimage semantics.

## Host-derived precondition fields

The host derives `expected_file_sha256` and `expected_file_byte_length` from
the exact raw preimage, including BOM and all newline bytes. The frontend cannot
supply, override, or edit these fields.

The host also derives the canonical legacy ToolInput, ToolCall identity,
complete expected ToolDefinition, allowed permission policy, selected
repository, target/path/parent/root identities, captured Git state, exact
postimage hash/length, and review identity. Repository identity, native paths
or handles, Git executable, permission, effect class, authority category,
generations, and raw ToolInput are host-only.

## Preparation architecture options

P1, Desktop duplicating matching/postimage preview, is rejected because BOM,
newline, UTF-8, limits, overlap, uniqueness, and precondition drift would make
review differ from execution.

P2, a shared non-effectful preparer in `rah-tools`, is selected. It reuses the
same parse, target admission, preimage, literal matching, range, and postimage
semantics and returns canonical ToolInput plus bounded private review evidence.

P3, a public `dry_run` field, is rejected because it changes model-visible Tool
semantics and mixes review with execution. P4, a generic worktree review
engine, is rejected as premature generic write authority. No P5 alternative has
evidence-backed advantage.

## Selected preparation architecture

P2 is authoritative. One shared implementation must own H1 parsing and limits,
logical paths, repository/target/Git admission, raw preimage hash/length,
strict UTF-8/BOM handling, literal non-fuzzy matching against the same
original snapshot, unique/overlap checks, complete postimage construction, and
private evidence.

The mutating Tool and the preparer may have different final phases, but both
consume the same semantic primitives. The preparer must never call
`Tool::execute`, create a user file, replace a target, write the index, Stage,
Commit, alter refs, invoke an arbitrary Tool, or bypass `ToolRegistry`.

## Preparation ownership

Select O2: retain a narrow non-effectful `RepositoryPatchPreparer` handle
alongside the first-party composition, constructed from the same host-selected
repository/Git identity and fixed limits as `repo.patch`. Bind its identity to
the ticket. It is not a Tool and is not mutation-capable.

This is not a second independently composed repository authority. If
cross-crate visibility requires a public type, expose only this patch-specific
bounded preparation API; do not add a generic downcast/get-concrete mechanism
to `ToolRegistry`. O1 is acceptable only as an equivalent concrete sibling in
the same composition. O3 and O4 are rejected. No new crate or dependency is
needed.

## Prepare zero-effect contract

Prepare requires, in order: coordinator `Idle`; no model turn;
connected-current composition; backend eligibility for `repo.patch`; D2
preflight definition/permission admission; typed H1 validation; shared
non-effectful preparation; exact preimage/postimage derivation; canonical
ToolInput; sanitized review; and an opaque ticket.

It rejects absent, untracked, staged/index-divergent, unsupported
sparse/conflict/link/reparse/special, binary/malformed, oversized, absent or
multiple-match, duplicate/overlap, no-op, and other current `repo.patch`
refusals. A failed Prepare creates no ticket.

Prepare performs zero `repo.patch` Tool executions and zero native replacement
attempts. It performs no index write, Stage/Unstage, Commit, ref/HEAD mutation,
provider activation, conversation injection, or model lifecycle. Bounded
read-only Git observation is allowed. Prepare success is not future authority.

Select N2 for no-op: when old and replacement text are equal, fail Prepare with
bounded `no_effect`/`nothing_to_change`, create no confirmation ticket, and do
not expose an effectful activity. The model-visible Tool retains its existing
verified-no-op refusal and no-replacement behavior.

## Review representation options

R1 (old/new only) is exact but omits locations. R2 (full preimage/postimage)
exposes too much unchanged source. R3 (unified-style diff) is useful display
but risks confusion with executable hunk semantics and needs extra newline
handling. R4 (changed ranges plus exact old/new) directly represents literal
semantics without a new dependency. R5 is acceptable only if R4 remains
authoritative.

## Selected review representation

Select R4. The review contains the complete old and replacement strings and the
complete changed range for the one operation. The range is a raw-file UTF-8
byte range, with BOM included in offset accounting. Optional line/column
locations are derived display metadata, never execution coordinates.

It states operation `repo.patch`, relative path, replacement count 1, exact
escaped old/new material, effect and non-effects, and that the display is not a
unified diff, line edit, fuzzy hunk, Git apply input, or execution format.
Preimage/postimage hashes and lengths may be shown read-only, but are always
ticket-bound. Post-confirm `repo.diff` remains the inspection path; it is never
the patch executor.

## No-hidden-change rule

All mutation-relevant changed material must fit the review bound. Otherwise
Prepare fails closed with `review_too_large` and creates no ticket. Confirm must
not remain available with clipped old text, replacement text, ellipses, or
redactions.

Only unchanged surrounding context may be omitted. The review must say that
context was omitted, and no omitted portion may contain part of a changed
string. Exact changed text, byte range, BOM state, newline markers,
trailing-space markers, and final-newline state remain represented completely.

## Encoding / newline / invisible-character review

The canonical input preserves exact UTF-8 characters and bytes. The display
uses deterministic escaping: CR as `\\r`, LF as `\\n`, TAB as `\\t`, other C0
controls and DEL as `\\u{...}`, and bidi/zero-width/format characters as
`\\u{...}`. Trailing spaces are explicit. A leading BOM is shown as
`BOM U+FEFF (preserved; excluded from matching)`. A final newline is shown
before an explicit EOF marker; no-final-newline is stated.

There is no Unicode normalization, case folding, or newline normalization.
CRLF, LF, and CR remain distinct, and the escaped form is display-only. The
underlying NUL rejection remains in force. Exact escaping is the selected safe
mechanism for currently permitted invisible/control characters; no broader
Tool restriction is introduced.

## Review size bounds

Future HostExplicit bounds are: Prepare IPC serialized DTO 64 KiB; path 1024
bytes; each text 64 KiB; aggregate old plus replacement text 64 KiB; exactly
one replacement; file/postimage 1 MiB; escaped changed material 192 KiB;
unchanged context 32 KiB optional; complete serialized review 256 KiB; ticket
stored representation 512 KiB; terminal activity/result metadata 32 KiB.

The changed-material and complete-review measurements occur after safe escaping.
Exceeding either exact-content/review bound returns `review_too_large`. Context
is the only omittable content. These bounds do not increase `repo.patch`.

## Canonical ToolInput binding

For H1 the preparer creates exactly the existing legacy form with `path`,
host-derived `expected_file_sha256`, host-derived
`expected_file_byte_length`, human `expected_old_text`, and human
`replacement_text`. Serialization is closed and deterministic. The frontend
cannot send raw ToolInput or hashes.

Private review identity is a hash over canonical ToolInput, target/preimage
identity and digest/length, postimage digest/length, and the complete escaped
review. Confirm trusts backend ticket state, never frontend review content.

## Ticket contract

The ticket is opaque, RAH-generated, process-local, in-memory, non-persistent,
single-use, bounded-lived, and not authority by itself. The default TTL is five
minutes, inclusive at the TTL boundary, matching the existing HostExplicit
principle. Restart discards it.

It binds the public Tool name; complete expected ToolDefinition; exact
canonical ToolInput; review identity/hash; selected repository and composition
identity; root/parent/target identity evidence; preimage SHA-256/length;
postimage SHA-256/length; allowed permission policy and Execute category;
repository, model, profile, and connection generations; Effective Authority
identity; preparer identity; and the captured Git/index/HEAD/refs state needed
by the existing policy.

Confirm receives only `ticketId`. It does not resend path, text, hashes,
lengths, Tool name, or JSON. Cancel also receives only ticket ID and is valid
only before Tool start.

## Ticket stale conditions

Confirm fails closed before Tool execution for changed repository selection,
canonical root, target path/identity/parent, bytes, preimage hash/length,
mode/attributes, links/reparse state, supported-file state, Git/index/HEAD/refs
state, tracking/cleanliness/sparse/conflict state, or any policy-bound
repository state. It also fails for disconnect/reconnect, non-current
composition, repository/model/profile/connection generation, Tool absence or
definition/schema/permission change, permission policy/Effective Authority
change, registry/composition change, preparer change, expired/unknown/wrong or
already-consumed ticket, and any target replacement.

There is no automatic ticket refresh, reprepare, expected-hash rewrite, retry,
replay, or use of the latest file.

## Confirm revalidation

Host workflow revalidation checks ticket, coordinator, current composition,
repository, generations, eligibility, current definition, allowed policy,
preparer identity, target/Git binding, and review identity without mutation.
The ticket is consumed before HostRunning so it cannot be confirmed twice.

Capability revalidation is independent: the owned task calls
`authorized_tool_dispatch` with the exact canonical ToolCall and current host
inputs. The registered `repo.patch` Tool reparses and revalidates repository,
path, target, preimage, matching, lease, final identity, one-attempt, and
postimage semantics. Neither layer replaces the other.

## D2 dispatch relationship

Before HostExplicit `Started`, Confirm calls `authorize_tool_dispatch`. Name,
missing Tool, complete definition, or permission rejection therefore causes
zero Tool execution and releases the coordinator. After `Started`, the owned
task calls `authorized_tool_dispatch`, which repeats the admission immediately
before invoking the current registry Tool.

The route is typed Prepare -> shared preparation -> exact review -> ticket-only
Confirm -> host revalidation -> D2 preflight -> HostExplicit Started -> owned
D2 execution -> ToolRegistry -> `repo.patch` -> ADR 0012 policy. Desktop must
not use raw `ToolRegistry::execute` as a complete host authorization route.
There is no generic `host_invoke_tool(name, json)`.

## Effect / non-effects

The only intended effect is replacement of one existing authorized tracked
regular file's content according to the canonical literal ToolInput. A new
target identity and preserved supported mode are part of the existing strategy.

No other path may change. There is no create/delete/rename/move/directory
effect, index write, Stage/Unstage, HEAD/branch/ref/tag/remote/history change,
branch switch, Git config change, provider/profile change, conversation change,
model invocation, shell/process execution, or authority change. The temporary
same-directory file is an ADR 0012 implementation detail, not user-file
creation authority.

## Reviewed commit authorization matrix

Current Desktop first-party mutation handling includes `repo.patch` and clears
reviewed commit authorization at the started mutation boundary. This is
consistent with ADR 0016's separate reviewed index/history authority and does
not claim that a known no-effect result changed bytes.

| outcome | target effect | reviewed commit authorization | refresh |
| --- | --- | --- | --- |
| Prepare success | none | preserve; do not arm | no mutation refresh |
| Prepare no-op | none | preserve; no ticket | none |
| stale/late D2 before Started | none proven | preserve unless fresh observation independently proves reviewed state stale | reobserve when useful |
| `precondition_failed` after Started | no replacement, but started first-party lifecycle | invalidate conservatively | yes |
| `ok` | exact replacement | invalidate | yes |
| `replacement_failed_known` | preimage intact | invalidate conservatively, matching current started mutation handling | yes |
| `uncertain`, ToolError, malformed, or lost terminal | possible effect | invalidate | yes, conservatively |

## Repository refresh/currentness matrix

| result | Tool/native counts | presentation | connection | retry |
| --- | --- | --- | --- | --- |
| Prepare refusal/no-op | 0/0 | no mutation refresh | unchanged | no |
| pre-Started stale/D2 rejection | 0/0 | optional fresh observation | existing stale/current state only | no |
| `precondition_failed` after Started | 1/0 unless policy proceeded to boundary | refresh/reobserve | Current only if composition/generations still match | no |
| `ok` | 1/1 | refresh/reobserve | same; no repository-generation increment for bytes | no |
| `replacement_failed_known` | 1/1 | refresh/reobserve | same | no |
| `uncertain` | 1/possible | refresh and possible-effect-unknown | do not claim Current after disconnect/recomposition | no |
| malformed/ToolError/lost terminal | 1/possible | refresh/reobserve conservatively | conservative currentness | no |

`repository_generation` is repository selection/currentness identity, not a
generic mutation counter. A successful patch does not increment it merely for
changed bytes. Existing observation generation may advance while reviewed
workflow state is revoked.

## Result classifier contract

The future strict classifier should be source-independent in `rah-tools` and be
usable by RuntimeModel and HostExplicit where an existing seam supports it.
Desktop adds only HostExplicit activity/provenance. It accepts exactly one JSON
content item with the current four keys and exact flag/status/reason
combinations. It rejects missing/extra keys, wrong types, unknown values,
malformed future hashes/lengths, and contradictions.

After Started, malformed output or ToolError is possible-effect-unknown, not
success or known no-effect. Before Started, D2 rejection is a zero-execution
dispatch rejection.

## Concurrency / cancellation / crash

Reuse the ADR 0021 coordinator. HostPrepared blocks model turns and other host
actions; HostRunning blocks model turns and second host actions. There is no
wait queue or read-only parallelism.

Before start, Cancel consumes the ticket and returns Idle with known zero
effect. After Started, there is no active abort. UI dismissal is not rollback.
Crash/restart discards prepared tickets. A started lost result is not replayed,
retried, restored, or treated as no effect; fresh observation is required.

## Conversation / persistence

HostExplicit does not add chat content, inject ToolOutput into model context,
continue a turn, ask the model to explain, or synthesize
`AgentEvent::ToolRequested`, `ToolStarted`, or `ToolFinished`.

Tickets and full review are not persisted to SQLite, generic activity history,
conversation replay, or evidence JSONL. Restart freshly composes authority and
observes the repository.

## Sensitive source-content handling

Full old/replacement text exists only in bounded process-local preparation state
and the local user's active review. Complete preimage bytes may be held
transiently while deriving hashes/postimage but are not stored in generic
activity records.

Never log or persist full source, preimage, replacement, temp/absolute paths,
Git executable, native IDs, environment, credentials, tokens, provider stderr,
aliases, or authority objects. Activity may retain bounded relative metadata,
review identity, hashes/lengths, result class, and sanitized Tool result.

## Backend / frontend boundary

Future IPC is closed and typed:

    host_prepare_repo_patch({ path, expectedOldText, replacementText })
    host_confirm_tool_invocation({ ticketId })
    host_cancel_tool_invocation({ ticketId })

The backend selects `repo.patch`, derives security-sensitive fields, performs
preparation, and returns bounded review plus opaque ticket ID. The frontend
cannot provide hashes/lengths, permission, authority category, effect class,
ToolDefinition, repository identity, native paths, or raw JSON. It renders a
backend HostExplicit kind and safe escaped review; Confirm sends only ticket
ID. Task 246 adds none of these commands or UI.

## Implementation dependency direction

No new crate or dependency is required. `rah-tools` owns exact patch parsing,
limits, path/repository/Git admission, UTF-8/BOM/newline semantics, matching,
postimage construction, non-effectful preparation, existing mutation authority,
execution, and the shared strict result classifier. It contains no Tauri,
frontend, Codex, AgentEvent, eligibility table, or conversation state.

`rah-desktop` owns coordinator, connected-current composition, eligibility,
typed IPC, ticket/review lifecycle, D2 calls, activity provenance, currentness,
refresh, and reviewed-commit invalidation. It retains the narrow preparer handle
as part of the same first-party composition but mutates only through D2 and the
current ToolRegistry.

## Required deterministic tests

Future implementation must cover: connected-current, eligibility, Execute
permission, Idle/no-model requirements; exact H1 DTO and rejection of hashes,
extra fields, raw ToolInput, arbitrary Tool names, and invalid logical paths;
clean tracked regular target admission; stale, staged, untracked, unsupported,
unique-match, multiple-match, overlap/duplicate, no-op, BOM/newline, control,
review exactness, review-too-large, and zero Prepare execution/attempts.

Ticket tests must cover opacity, single use, five-minute TTL, cancel, expiry,
wrong/second confirm, restart, all generation/composition/definition/permission
changes, changed file/identity/path, changed index/HEAD/refs, disconnect and
reconnect, and no refresh/reprepare/retry/replay.

Confirm tests must prove D2 before Started, late D2 rejection with zero Tool
execution, owned `authorized_tool_dispatch`, exactly one Tool execution, one
native replacement attempt maximum, and ticket-ID-only Confirm.

Result tests must cover all four statuses, strict missing/extra/wrong/unknown/
contradictory outputs, ToolError, malformed output, and lost terminal. State
tests must prove index, HEAD, refs/history, other paths, providers, profiles,
conversation, review authorization matrix, refresh matrix, coordinator release,
no AgentEvent forgery, and no conversation injection. Frontend tests must prove
backend host-kind forms, no name-based authority inference, no arbitrary JSON,
safe escaping, no hidden changed material, and ticket-only Confirm.

## Windows live-certification contract

Design, but do not run, a future Windows gate with a fresh disposable Git
repository, ordinary attached branch, one clean tracked strict-UTF-8 target,
non-sensitive old/new markers, and optional unrelated sentinels. Use the
production connected-current Desktop path, typed H1 Prepare, exact review,
ticket-only Confirm, HostExplicit Started, D2 revalidation, normal registry
dispatch, existing `repo.patch`, one native attempt, and exact observation.

Require zero model request and zero model ToolRequested/Started/Finished; one
Prepare, one Confirm, one HostExplicit Started, one terminal host result, one
Tool execution, and one native attempt. Prove exact postimage/hash/length,
target-only change, unchanged other paths/index/HEAD/branch/refs/history,
HostExplicit provenance, review invalidation, currentness, no retry, and clean
shutdown. Evidence uses bounded redacted metadata and preserves markers after
any post-Started failure.

## Live failure boundary

Before HostExplicit Started, failure is zero Tool effect and zero replacement
attempt. After Started, the capability's native system call is the possible
effect boundary. Preimage-intact proof may classify a failed replacement as
known failure; any gap, contradiction, ToolError, timeout, disconnect, crash,
or lost terminal is uncertain/possible-effect-unknown. Never retry, replay,
restore, rollback, reset, or delete evidence.

## ADR decision

Select Option B: create a new narrow ADR 0022 after this research.

Proposed title: ADR 0022 — HostExplicit Reviewed Worktree Authoring Boundary.
It should record shared non-effectful preparation -> exact human-visible review
-> opaque preimage-bound ticket -> explicit ticket-only Confirm -> host and D2
revalidation -> existing `repo.patch` authority.

This adds no underlying worktree authority. ADR 0012 remains mutation authority
and ADR 0021 remains the general HostExplicit dispatch/currentness/provenance/
ticket/D2 boundary. A new ADR is durable architecture because ADR 0021
deliberately deferred worktree authoring until capability-specific typed review
and preimage/effect contracts existed. Task 247 is documentation-only and must
not implement Rust.

## Security nonclaims

A1 provides no generic `fs.write`, arbitrary full-file replacement, file
creation, deletion, rename/move, directory creation, multi-file edit, regex,
unified-diff execution, Git apply, fuzzy hunk, line/range, binary or staged-file
editing, branch switching, Stage, Commit, generic Git, shell/process execution,
rollback, retry/replay, OS sandbox, network isolation, or race-free TOCTOU
guarantee. The review is display-only and is never passed to `git apply`.

## Recommended implementation sequence

1. Task 247 — ADR 0022, HostExplicit Reviewed Worktree Authoring Boundary
   (documentation-only; no Rust).
2. Task 248 — Shared Non-Effectful `repo.patch` Preparation Foundation in
   `rah-tools`.
3. Task 249 — Desktop HostExplicit `repo.patch` Backend Integration.
4. Task 250 — Patch Review, Strict Result Hardening, and Frontend UX.
5. Task 251 — Windows Live HostExplicit `repo.patch` Certification.
6. Task 252 — v0.21 A1 Milestone Audit.

No step starts automatically from Task 246. Any authority or semantic expansion
requires a new research decision.

## Validation

Run sequentially:

    cargo fmt --check
    cargo check --workspace
    git diff --check
    cargo metadata --no-deps --format-version 1
    git diff -- Cargo.toml Cargo.lock

The first four commands must pass. Metadata must report 13 packages, all
`0.20.0`, edition 2024. The Cargo comparison must be empty. No live effect,
model prompt, or model experiment is part of validation.

## Commit

The only intended file is:

    docs/plans/2026-09-07-hostexplicit-worktree-authoring-review-contract-research.md

The requested commit message is:

    docs: define HostExplicit patch review contract

Before commit, require `git status --short`, `git diff --stat`, and
`git diff --check` to show the exact one-file documentation scope. No Rust,
tests, frontend, Cargo, lockfile, ADR, README, CHANGELOG,
architecture/security document, release gate, workflow, script, or unrelated
file may change.

## Exact-head CI

Before push, run `git fetch origin` and require `origin/master` remains
`84b57a402dcec92397441d7139ba41be29f907b7`. If it moves unexpectedly, stop;
do not rebase or silently change chronology. Push `master` normally.

Task 246 is complete only after the CI run for the exact Task 246 commit has
branch `master`, event `push`, exact `head_sha`, status `completed`, and
conclusion `success`. Absent, queued, in-progress, or different-head CI is not
success.

## Next task

Task 247 — ADR 0022 Reviewed HostExplicit Worktree Authoring Boundary.

It is not started automatically. No Rust implementation begins until that
architecture record is complete.
