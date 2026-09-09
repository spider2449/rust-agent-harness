# RAH v0.22.0 Security Model

This document describes the released v0.22.0 security model. RAH v0.22.0 is
released and is the current immutable published release; v0.21.0 is the prior
immutable release. The immutable v0.22.0 release source is
`76895b4067c38167f3c41a3536f6616cffa8293a`.

The later Task 267 documentation cleanup commit is not the v0.22.0 release
source.

v0.20.0 preceded v0.21.0.

v0.18.0 preceded v0.19.0.

## ADR 0023 reviewed HostExplicit multi-file authoring boundary

The v0.22 capability is **HostExplicit Reviewed Multi-File Edit Authoring**.
It binds a typed Desktop human request to the existing ADR 0014
`RepositoryMultiFileMutationPolicy`:

```text
typed bounded request
 -> zero-effect Prepare
 -> complete backend-derived ordered review
 -> opaque ticket-only Confirm
 -> shared revalidation / D2
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.edit-files / ADR 0014
 -> strict result classification / descriptive refresh
```

The closed request allows 1–4 existing clean HEAD-tracked regular strict-UTF-8
files, 1–16 exact literal replacements per target, and 64 replacements total.
The host derives exact original-snapshot matching, preimages, postimages,
hashes, lengths, and canonical path order. The frontend is presentation and
typed input only; it cannot provide Tool JSON, authority, permission,
repository, native paths, hashes, postimages, or execution order. Complete
mutation-relevant review is required, and `review_too_large` fails closed.

The exact HostExplicit allowlist is:

- `fs.read`
- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`
- `repo.create-branch`
- `repo.patch`
- `repo.edit-files`

Only these eight names are eligible. `repo.create-file`, `repo.delete-file`,
`repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools, Process
Plugin Tools, and unknown Tools are not eligible. There is no wildcard or
category-based eligibility. ADR 0023 adds the reviewed workflow boundary;
ADR 0014 remains the underlying mutation authority and ADR 0021 remains the
general HostExplicit coordinator/currentness/D2 boundary. Model output is
never authorization. Permission classification does not create mutation
authority, and Trusted Profile/provider metadata cannot self-enable
HostExplicit.

Prepare performs zero Tool executions, native attempts, repository effects, or
authority persistence. The opaque ticket is process-local, in-memory,
single-use, exact-change/currentness-bound, and valid for an inclusive
five-minute TTL. Confirm and Cancel accept only `{ ticketId }`; they do not
accept trusted bindings from the frontend. Revalidation and D2 occur before
`Started`, and dispatch then follows the existing ToolRegistry path. Restart,
disconnect, expiry, cancellation, stale currentness, or consumption never
restores or replaces a ticket.

Generic HostExplicit activity uses a separate RAH-generated non-authority
activity ID. It is not the ticket, is not derived from the ticket, cannot
Confirm or Cancel, and the actual ticket is absent from generic activity.
Generic activity also excludes source-bearing review material, raw Tool input
or output, native/absolute paths, and source errors. Task 263 Attempt 1
discovered the actual ticket leak under `invocationId` before Confirm with
zero effect. Task 263 Attempt 2 corrected the privacy separation and passed on
a fresh repository; the failed attempt remains historical security evidence.

The operation is explicitly non-atomic. ADR 0014's six result classes remain
distinct: `ok`, `invalid_target`, `precondition_failed`,
`failed_known_no_effect`, `partial_effect`, and `uncertain`. A
`partial_effect` reports only the verified committed prefix; `uncertain` does
not infer unchanged state. Timeout, cancellation, disconnect, crash, or lost
response does not imply rollback. There is no retry, replay, prefix
continuation, rollback, restore-preimage, compensation, or transaction.

Effectful `repo.patch` and `repo.edit-files` start invalidate repository-bound
reviewed Commit authorization, including rejected post-Started dispatch paths.
Refresh is descriptive observation only. When optional reviewed-Commit
preparation is unavailable because the staged index is empty, the narrow
fallback retains status/diff refresh, returns no staged changes, and does not
fabricate or grant Commit authority. Stage and Commit remain separate explicit
actions.

## v0.22 live evidence boundary

Task 263 final Attempt 2 is the carried-forward connected-current production
evidence on Windows 11 IoT Enterprise LTSC x64. It used Codex `0.149.0`,
SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning. Caller order was `d.txt, b.txt, a.txt,
c.txt`; backend/effect order was `a.txt, b.txt, c.txt, d.txt`. Prepare had
zero Tool/native attempts; Confirm had one Tool and native attempts `1/1/1/1`.
Final generations were `[1, 0, 0, 1]`; coordinator and chat were Idle; model
lifecycle, MCP, and Process Plugin counts were zero; and the marker was
`RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK`.

This is host-driven connected-current evidence, not model-selected
HostExplicit certification. Unix deterministic or platform-gated testing is
not Windows-equivalent live certification. The evidence does not claim
atomicity, rollback, race-free TOCTOU, network isolation, OS sandboxing,
generic filesystem write, structural HostExplicit authoring, HostExplicit
`repo.commit`, MCP/Process Plugin HostExplicit, ticket persistence/resume, or
automatic Stage/Commit. Process supervision is not OS sandboxing.

## ADR 0022 reviewed HostExplicit authoring boundary

ADR 0012 remains the sole underlying `repo.patch` worktree-content mutation
authority. ADR 0021 remains the general HostExplicit dispatch/currentness/D2
boundary. ADR 0022 adds only the reviewed human workflow around that existing
capability and adds no generic write authority.

The exact v0.21 HostExplicit set is:

- `fs.read`
- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`
- `repo.create-branch`
- `repo.patch`

`repo.create-file`, `repo.edit-files`, `repo.delete-file`, `repo.rename-file`,
`repo.create-directory`, `repo.commit`, MCP Tools, and Process Plugin Tools are
not eligible. There is no other HostExplicit authoring Tool enablement.

The human supplies only typed H1 input: `path`, `expectedOldText`, and
`replacementText`. The host derives SHA-256 and byte-length values and the
canonical `repo.patch` ToolInput. The shared non-effectful
`RepositoryPatchPreparer` derives the postimage and exact bounded escaped R4
review. No mutation-relevant review content is hidden; if the complete changed
material cannot fit, preparation fails closed.

The security distinctions are explicit:

```text
human review != authorization
frontend != authority
model request != authorization
preparation != mutation
D2 != underlying capability authority
confirmation != generic write authority
```

Prepare has zero Tool executions and zero native replacements. Its opaque
process-local single-use five-minute ticket binds the host-derived input,
review, identities, definitions, permissions, and currentness. Confirm accepts
the ticket ID only, revalidates shared preparation before `Started`, performs
D2 preflight before `Started`, and uses `authorized_tool_dispatch` after
`Started` to call the existing `repo.patch` Tool. The underlying ADR 0012
policy performs the mutation, with one possible native replacement attempt.

The host strictly classifies the exact result schema. Malformed, contradictory,
uncertain, lost, or post-start `ToolError` outcomes are handled as possible
effect/uncertain; success is never inferred merely from observing changed
bytes. There is no retry, replay, rollback, restore-preimage, compensation, or
automatic second confirmation.

Patch review/source text remains bounded to process-local prepared state and
the local review. It is absent from generic activity, conversation persistence,
conversation replay, and evidence logs. Once an effectful patch reaches
`Started`, old reviewed-commit authorization is invalidated. Refresh may create
a new `ReadyToAuthorize` review, but it does not automatically authorize the
new review. Content mutation alone does not increment
`repository_generation`.

## ADR 0021 explicit host invocation dispatch boundary

Host explicit dispatch is not capability authority and is not runtime/model
dynamic dispatch. Model text, provider metadata, frontend state, Tool
visibility, Effective Authority, `PermissionLevel`, `Execute`, and human
confirmation cannot create or amplify authority. There is no model-text trigger
for HostExplicit invocation.

HostExplicit is connected-current only. The backend retains the current Tool
registry, expected definitions, shared allowed permission policy, selected
repository/context, and repository/model/profile/connection generations. Every
action requires current connected state and fails closed on stale or
disconnected state. It cannot silently reconnect, recompose, activate a
provider, restore a Trusted Profile, or execute while disconnected.

Eligibility is backend-owned and exactly limited to `fs.read`,
`repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`, and
`repo.create-branch`. MCP Tools, Process Plugin Tools, `echo`, fixtures,
generic diagnostic Tools, `repo.commit`, and worktree-authoring Tools are not
HostExplicit eligible in the first release. The frontend consumes a backend
descriptor; it does not infer eligibility from a Tool name, permission, effect
class, authority category, or advertised state.

HostExplicit accepts typed operations only. There is no generic
`ToolName + arbitrary JSON` route or production JSON console. The underlying
Tool parser and policy remain authoritative. Read operations use a path where
required; observer operations use `{}`; branch Prepare uses a validated name;
branch Confirm accepts only an opaque ticket ID.

The only added Tauri permission names are `host_invoke_read`,
`host_prepare_repo_create_branch`, `host_confirm_tool_invocation`, and
`host_cancel_tool_invocation`. There is no wildcard, generic D2 endpoint, or
invoke-any-tool permission. Effective Authority may expose only backend-owned
eligibility, kind, and bounded unavailable reason; that information is
observational and the frontend is not an authorization boundary. Tool text is
text-safe, JSON is escaped/preformatted, and raw HTML ToolOutput rendering is
not used.

## D2 shared authorization

The neutral D2 boundary binds the call name to the expected Tool definition,
re-resolves the current Tool from the current `ToolRegistry`, and requires
exact complete `ToolDefinition` equality: name, description, input schema, and
permission. The current `PermissionLevel` must be explicitly present in the
host allowed policy. Permission levels are not hierarchical. Admission
rejection executes zero Tools; successful admission executes exactly one Tool
and never retries.

D2 is a dispatch admission and execution boundary. It does not own HostExplicit
eligibility, capability authority, lifecycle/provenance, repository outcome
interpretation, replay, or rollback. HostExplicit does not create repository,
filesystem, branch/ref, commit, provider, process, or network authority.

## Codex bridge hardening

The Codex route retains thread ownership, active-turn ownership, private alias
mapping, replay/deduplication, cancellation, response translation, and the real
AgentEvent lifecycle. It calls `authorize_tool_dispatch` before `ToolStarted`
and `authorized_tool_dispatch` revalidates before execution. A captured
permission change is a stale ToolDefinition even when the replacement
permission would otherwise be allowed. This aligns dispatch admission; it does
not make model Tool selection more reliable.

## Desktop lifecycle, branch tickets, and concurrency

The typed Tauri surface has only narrow HostExplicit commands. Branch Prepare
does not execute a Tool and has no Git effect. Review Confirm consumes a
process-local, in-memory, non-persistent, single-use, Tool-bound, input-bound,
composition/currentness-bound ticket with a five-minute TTL. Confirm
revalidates currentness, does not auto-reprepare, and does not retry before D2
and existing ADR 0020 branch authority.

One HostExplicit action maximum is allowed. Host work is rejected during a
model turn; model work is rejected during `HostPrepared` or `HostRunning`;
read-only host actions participate; there is no wait queue. Host activity is
Desktop-private `host_explicit`, never forged `ToolRequested`, `ToolStarted`,
or `ToolFinished` activity, and no AgentEvent schema change exists.

Before start, prepared work can be cancelled with known no Tool effect. After
start, there is no active HostExplicit abort operation. A crash or lost result
does not imply no effect. There is no retry, replay, compensation, rollback,
ticket persistence, capability persistence, automatic resume, conversation
continuation, user/assistant chat message, or ToolOutput injection into model
context.

Process supervision is not OS sandboxing. RAH does not claim network isolation,
race-free TOCTOU behavior, rollback, or absence of ambient external effects.

## ADR 0020 bounded local branch creation authority

`repo.create-branch` is a separate, private host-owned
`RepositoryBranchCreationPolicy` exposed through an opaque
`RepositoryBranchCreationAuthority` and a public Tool. It is composed only by
the selected Desktop repository context. Execute is the outer dispatch gate,
not branch authority; model output, provider metadata, Tool definitions,
Trusted Profile composition, and frontend state cannot create or escalate the
policy.

The closed model input is name-only:
`{"name":"<validated-logical-branch-name>"}`. The host supplies the selected
repository, native Git executable, captured attached committed `HEAD`,
validated `refs/heads/<name>` target, zero-old OID, hooks directory, identity,
argv, environment, and configuration. One authorized call can create exactly
one previously absent ordinary local branch at that captured OID using the
fixed native Git shape:

```text
git update-ref --create-reflog -m "RAH create local branch"
  refs/heads/<name> <captured-head-oid> <zero-old-oid>
```

The zero-old OID is an expected-absence CAS; existing refs are never
overwritten. Only the Git-owned creation reflog for that branch is permitted.
There is no branch switch, checkout, create-and-switch, deletion, rename,
force-update, tracking/upstream setup, tag or remote operation, arbitrary
revision/OID/ref input, generic update-ref, generic Git, or remote/network Git
authority.

The name language is closed ASCII with bounded length and components. Unicode,
invalid names, exact collisions, ancestor/descendant prefix collisions, and
ASCII case-fold collisions are rejected. Loose and packed local heads are
covered; observation is byte-safe and restricted to `refs/heads/`, not
unrelated ref namespaces. Count and output bounds fail closed. This is not
arbitrary Git-ref-name support.

Admission is limited to an ordinary non-bare repository with a real `.git`
directory, conventional files-backed refs, and an existing attached committed
`HEAD`. Dirty, staged, and mixed ordinary states are allowed. Detached or
unborn `HEAD`, bare repositories, `.git` indirection, linked worktrees,
unsupported ref backends, and required-rejected merge/rebase/sequencer or
other special-operation states are not admitted.

The selected repository and native Git executable are identity-revalidated,
including rejection of same-path replacement. Git runs with a host-owned
canonical empty hooks directory whose identity and emptiness are revalidated;
hostile repository `core.hooksPath` cannot widen execution. Security-critical
Git configuration and environment are minimized and pinned, with system/global
configuration disabled where the policy requires it. No PATH-selected
executable is used. These checks do not claim race-free TOCTOU exclusion.

There is one possible-effect mutating attempt. The exact public outcomes are
`invalid_input`, `precondition_failed`, `known_no_effect`,
`branch_created_verified`, `desired_state_observed_after_uncertain_attempt`,
and `uncertain`. `known_no_effect` requires both target-ref and target-reflog
absence. Verified or desired-state evidence requires the accepted fixed
reflog. Timeout, cancellation, disconnect, crash, or lost response does not
imply rollback: uncertain effects are not replayed, retried, rolled back, or
compensated by branch deletion.

Verified create-only creation does not intentionally change symbolic `HEAD`,
the checked-out branch, `HEAD` OID, index, staged or unstaged worktree state,
tracking/upstream state, pre-existing refs, tags, remotes, repository/model/
profile/connection generations, conversation namespace, or provider
composition. It preserves a previously valid reviewed commit authorization;
malformed, fully uncertain, and started-but-unfinished possible effects are
handled conservatively with invalidation and bounded refresh.

Process supervision is not OS sandboxing. This capability makes no claim of
network isolation, rollback, or race-free exclusion from external Git,
filesystem, editor, sync, antivirus, or privileged processes. It grants no
Trusted Profile/provider branch authority; `repo.create-branch` is first-party
host composition only.

## Windows host-driven certification and model-selected limitation

Task 229C certified the real Windows Desktop path from
`DesktopRepository` through `RepositoryBranchCreationAuthority`,
`RepositoryBranchCreationTool`, and `ToolRegistry` to one host-driven native
Git effect, including the protected-state and currentness invariants. The
fresh successful branch name and OID were asserted internally but not printed
on the success path; this is a nonblocking evidence-capture limitation and the
effectful gate was not rerun.

Windows host-driven Desktop repo.create-branch authority/effect path is certified. Model-selected repo.create-branch dispatch was not observed in two bounded Codex live attempts and is not certified.

Linux live branch certification is not established. Task 207's prior
externally blocked model-selected MCP/Process Plugin execution limitation is
unchanged.

## Effective Authority Review

The Effective Authority panel consumes a host-owned, closed,
backend-sanitized DTO. Raw repository paths, secrets, provider stderr and
endpoints, private Tool aliases, and review handles are excluded before
frontend serialization/rendering. Provider metadata cannot self-classify or
escalate authority. Unknown schema or status values fail closed.

The snapshot is an observation of configured, effective, and runtime-advertised
state. Advertisement or display does not mean unconditional authorization:
requests still pass ToolRegistry lookup, PermissionLevel gates, applicable host
policy, repository/workspace constraints, generation/precondition checks, and
separate one-shot reviewed-commit authorization where applicable. A review
state is presentation state, not an authority token.

Backend-derived stale or reconnect-required state cannot be labelled Current.
Inspection and Refresh Authority have zero lifecycle, Tool, repository, chat,
authority, or persistence side effects. The panel grants no authority, does
not dynamically grant or revoke permissions, does not reload profiles or
manage provider lifecycles, and persists/restores no authority.

## Inert Trusted Profile preference persistence

v0.18 may persist one host-selected Trusted Profile source path as inert local
preference. On startup this is remembered-not-restored: it cannot select,
validate, compose, spawn, advertise, or restore provider authority. Explicit
Restore fresh-loads and statically validates the source without spawning;
explicit Connect or reconnect alone can activate providers, and it fresh-loads
the source before composition. Forget removes only the durable preference and
does not affect an already active composition.

The persisted path, its presentation, model output, provider metadata, and
frontend controls are not authority. Profile generation is included in
currentness so stale state cannot be labelled Current. This release adds no
active-provider auto-restore, automatic baseline repair/download/migration,
profile hot reload, credential persistence, network MCP, generic shell,
filesystem, Git, or provider authority. Process supervision remains not an OS
sandbox; network isolation, rollback, and absence of ambient provider effects
are not claimed.

v0.16 added no authority. The v0.17 provider-only overlay adds no generic
shell, filesystem, Git, branch/ref, or network authority and makes no
OS-sandbox, network-isolation, or rollback guarantee. MCP and Process Plugin
remain existing host-controlled Tool providers; admitted local providers are
now composed by Desktop at Connect and presented through the same sanitized
Effective Authority path. Refresh remains observational and does not activate
or reload them.

## ADR 0019 bounded repository directory creation authority

`repo.create-directory` is a separate host-owned authority for exactly one
new ordinary directory leaf at an explicit repository-relative path. Its
parent must already exist and its destination must be absent. It uses the
host-bound `RepositoryDirectoryCreationPolicy`, immediate pre-effect
revalidation, and one handle-relative Windows or descriptor-relative
Unix/Linux native attempt. The verified result is
`directory_created_verified` with `uncertain=false`.

This is not generic `fs.mkdir`, recursive `mkdir -p`, ensure-directory,
directory-tree, shell/process, or fallback authority. It does not create
placeholder files, mutate Git, stage, commit, or alter refs. It is separate
from file create/delete/rename, content mutation, index mutation, reviewed
commit/history mutation, and Execute. Model requests, provider metadata,
Tool definitions, Trusted Profile composition, and frontend state are not
authorization; no existing authority implies directory creation.

The operation is one possible-effect attempt only. Timeout, cancellation,
disconnect, or crash does not imply rollback or compensation, and a possible
effect is never retried or replayed. Desktop binds the authority to the
selected repository and repository generation; verified directory creation
invalidates reviewed-commit authorization and refreshes repository state even
if `git status --short` is empty. Git does not track empty directories, so
clean Git status is not proof that no filesystem mutation occurred.

## ADR 0018 bounded repository file rename/move authority

`repo.rename-file` is a separate host-owned authority for exactly one clean,
HEAD-tracked regular file in the selected repository. It permits a
same-directory rename or same-repository cross-directory move when the source
matches the exact expected raw-byte SHA-256 and byte length. The explicit
destination must already have an existing parent and must not exist; there is
no overwrite or replacement.

Immediately before one native no-replace filesystem effect, the host
revalidates repository, source, destination, HEAD, index, branch, runtime, and
repository-generation identity. It does not use generic `fs.rename`, `git mv`,
copy-delete, or shell/process fallback, and never stages or commits. A possible
effect is `uncertain` and is not replayed; no rollback guarantee is made.

Model requests, provider metadata, Execute permission, Tool definitions,
Trusted Profile composition, and frontend state are not authorization. Rename
authority does not imply creation, deletion, arbitrary content-write, index,
commit/history, branch/ref, network Git, shell, or process authority. Create and
delete are separate authorities. Process supervision is not OS sandboxing, and
network isolation is not claimed.

Windows is live-certified for this capability using the complete certified
`codex-cli 0.149.0` pair; Linux live certification is not established.

## ADR 0017 bounded repository file deletion authority

`repo.delete-file` remains a separate host-owned authority for exactly one explicit
repository-relative regular file. The target must be clean, HEAD-tracked, and
match the exact authorized HEAD blob preimage, including raw bytes, SHA-256,
and byte length. The operation makes one native worktree deletion attempt and
does not stage, commit, alter refs/history, or grant rename/move, directory or
recursive deletion, arbitrary untracked deletion, generic filesystem,
shell/process, or Git authority.

Model requests, provider metadata, Execute permission, Tool definitions,
Trusted Profile composition, and frontend state cannot manufacture this
authority. The Generic Codex Tool Bridge uses the canonical public name and
private provider aliases only for translation. Uncertain effects are not
automatically replayed or rolled back. Windows is live-certified using
`codex-cli 0.149.0`; deterministic Windows and Ubuntu/Linux CI/test evidence
does not establish Linux live certification.

## ADR 0016 bounded repository commit authority

`repo.commit` is the v0.11 exception that completes the host-reviewed local
workflow from an already staged index to one ordinary commit. ADR 0016 remains
authoritative. The trusted host owns the authority: the public model input is
only a bounded UTF-8 `message`; it cannot select repository, native Git,
branch/ref, parent, index/tree/hash, identity, hooks, config/environment,
argv, remote, or credential.

The v0.12 Desktop workflow does not add authority around that existing
exception. Human Stage / Unstage remain host actions. The host observes the
staged review, and human Authorize arms the opaque Rust-only reviewed snapshot.
Frontend presentation is not authorization, and model request or Execute
permission is not commit authorization.

Trusted Profile composition has a closed `repo.commit` schema with symbolic
repository and executable resources, explicit host identity, and an `Execute`
outer permission. It does not authorize a commit. A host-only
`RepositoryCommitControl` must explicitly capture and arm one fresh reviewed
snapshot. One pending authorization is retained in memory; malformed messages
do not consume it, while stale/precondition, known-no-effect, uncertain, and
successful attempts consume it. It is neither serialized to SQLite nor
reconstructed after restart.

The policy admits only an exact host-selected ordinary repository with attached,
non-unborn HEAD and its existing current branch. The authorization binds the
expected parent and compound reviewed index snapshot (raw index SHA-256,
canonical staged-entry digest, and `git write-tree` OID). It rejects detached
or unborn HEAD, special Git states, linked worktrees, and staged gitlinks. The
fixed native-Git command performs one normal non-amend commit only; no automatic
staging, signing, merge/rebase/cherry-pick, or retry/replay is available.

Git executable and host-owned empty hooks-directory identities are revalidated.
The child uses host-fixed minimized configuration/environment, explicit trusted
author/committer identity, `--no-verify`, `core.hooksPath`, and
`commit.gpgSign=false`. The policy proves either `committed_verified`, a
post-observed `known_no_effect`, or conservatively `uncertain`; it never claims
rollback. Model-visible output is bounded/redacted: a status and, only on
verified success, the commit OID—not raw Git stderr, paths, index hashes,
authorization data, configuration, or credentials.

The shared per-repository mutation lease serializes RAH-owned stage, unstage,
patch, create-file, edit-files, and commit operations. It does not exclude
external Git or other process actors. This offline local authority grants no
branch/ref history control, tags, remote Git, credentials, network operation,
generic filesystem write/delete/rename, generic shell/process execution,
linked-worktree/submodule support, OS sandbox, network isolation, or rollback.
Windows is the certified live platform using the complete official Codex 0.149.0
runtime including its same-version code-mode host. Ubuntu CI is deterministic
evidence only; Linux and macOS live parity are not claimed.

## v0.9 boundary and preserved capabilities

The public/host Execute capabilities include `host.cargo.version`,
`host.git.status`, `host.git.stage`, `host.git.unstage`, `repo.create-file`,
`repo.edit-files`, `repo.rename-file`, and the fixed
repository observers `repo.file-info`, `repo.status`, `repo.diff`, and
`repo.diff-staged`. They are
host-constructed, capability-specific tools, not generic model-selected process
authority. The hardened Execute deterministic/live fixture (`process.test.echo`)
and the repository-mutation deterministic/live fixture are validation
infrastructure, not public capabilities. `host.fixture.echo` does not exist.

The v0.3 boundary excludes arbitrary `shell.exec` and `process.exec`,
model-selected executable/argv/cwd/environment, worktree restore, arbitrary
file mutation outside the accepted bounded `repo.patch`, `repo.create-file`, and `repo.edit-files`
policies,
commit/history/ref operations, reset/clean/checkout/switch/stash,
merge/rebase, push/pull/fetch, network Git, and credential-bearing Git
execution. ADR 0012 grants the existing-worktree-content exception: one conditional
literal replacement in one existing HEAD-tracked, unstaged strict-UTF-8 file
under a private host-owned `RepositoryWorktreeMutationPolicy`. It does not
grant generic write, index, history/ref, network, rollback, or replay authority.

ADR 0013 separately grants bounded new-path creation through `repo.create-file`:
one absent UTF-8 regular file, at a model-selected validated repository-relative
path, in a host-bound repository. It requires an existing real parent, rejects
link/reparse traversal, ignored/index/HEAD/submodule/sparse targets, and uses
exclusive native creation. It grants no generic filesystem write, overwrite,
mkdir, append, delete, rename, chmod, staging, index/history/ref mutation,
rollback, or replay authority. A possible partial write is retained and
classified conservatively rather than deleted or replayed. `repo.patch` and
`repo.create-file` share the per-repository mutation lease.

ADR 0014 separately grants `repo.edit-files` only through private host-bound
`RepositoryMultiFileMutationPolicy`: one through four existing clean tracked
UTF-8 files, deterministic host order, and no cross-file transaction. Its
outer permission is `Execute`; it grants no rollback, retry, replay, staging,
history/ref, or network Git authority. Bounded `partial_effect` and `uncertain`
results retain only logical target inventory. Trusted Profile v1 static
validation is nonmutating; effective composition host-constructs and registers
the capability only after complete success. Generic Tool Bridge uses ordinary
generic dispatch. Certified Windows live validation using exactly `codex-cli
0.149.0` emitted `RAH_REPO_EDIT_FILES_LIVE_OK`; Unix live Codex validation is
not claimed.

Repository observation remains fixed read-only: `repo.file-info`,
`repo.status`, `repo.diff`, and `repo.diff-staged`. The host fixes executable,
repository, cwd, argv, environment, limits, and diff baseline; model input
cannot choose any of them. Cleared Git environments disable system/global
configuration, inherited HOME/XDG/PATH, pager, external diff/textconv,
fsmonitor, untracked cache, optional locks, terminal prompting, and ambient
credential/proxy variables. NUL-framed machine output is normalized into UTF-8
or tagged base64 paths without returning binary content. The tools make no
intentional repository mutation claim, but do not claim that Git and the host
perform zero incidental filesystem writes. Their shared lease is not a
cross-process snapshot transaction; detectable contradictions fail closed and
external races remain a documented best-effort limitation.

Process supervision is not OS sandboxing and RAH makes no network-isolation or
rollback guarantee. Timeout or cancellation can leave uncertain mutation
effects; uncertain mutations are never automatically replayed. Windows Job
Object assignment remains post-spawn, external OS processes can race repository
mutation, and Git configuration may influence Git semantics.

## Trust and authorization boundary

Model output and external-provider metadata are untrusted. A tool request or
declaration never authorizes execution. The supported path is:

```text
parsed ToolCall
 -> ToolRegistry
 -> host permission decision
 -> Sandbox / workspace policy where applicable
 -> Tool
 -> ToolOutput
```

## Trusted static capability profile source

ADR 0011 defines this explicitly selected trusted-host profile as RAH's
authority-composition boundary. It configures already-approved constructors and
their host-owned resources; it does not replace their capability-specific
policies or create generic process, filesystem, Git, network, or credential
authority.

`rah profile validate <absolute-profile-path>` accepts only an operator-selected
absolute profile path; it has no search, environment selection, reload, or
watching behavior. Before JSON parsing, `rah-tools` validates every path
component, rejects links and Windows reparse points, requires the final object
to be a regular file, opens it once, validates that opened object, and reads at
most 1 MiB from that same handle. Profile text must be valid UTF-8.

On Windows, the initial boundary accepts only normal drive-rooted paths. It
rejects UNC, verbatim (`\\?\\`) and device prefix forms, and paths with an ADS
colon in a normal component. `.` and `..` components are rejected on every
platform. Junctions, symbolic links, and all other objects with
`FILE_ATTRIBUTE_REPARSE_POINT` are rejected. Case-equivalent paths are not
treated as distinct trust identities; raw path-string equality is never an
identity claim. Unix permits hard links but rejects symbolic links; the opened
object's device/inode is compared to the post-open pathname object.

This is file identity/type validation, not a portable proof that only a trusted
OS principal can modify the file. RAH does not inspect or enforce ownership,
ACLs, or modes, and does not claim a trusted-store guarantee. Unix uses
`O_NOFOLLOW` for the final component and reads from the opened handle; all
platforms recheck pathname topology after open. An external actor can still
race parent-path replacement or filesystem behavior outside the checks,
especially on Windows where the standard library exposes no portable opened-file
identity comparison used here. Operators must therefore place profiles in an
OS-managed location with appropriate ownership and ACL controls.

The source policy rejects relative paths, lexical `.`/`..` aliases, links,
junctions/reparse points, non-regular sources, and unsupported ambiguous path
forms. On Windows it additionally rejects UNC, verbatim/device, and ADS forms.
It does not prove exclusive ACL ownership, provide an OS trusted-store claim,
or eliminate filesystem TOCTOU races.

`ExternalToolIdentity` is an opaque RAH-owned key for one tool discovered from
an external provider. `ExternalToolPermissionPolicy` maps those identities to
host-selected RAH `PermissionLevel` values. It is default-deny: absence is not
`PermissionLevel::None`, and an unassigned external tool fails before
registration. Duplicate assignments are rejected. MCP server and process-plugin
metadata cannot grant or escalate permissions.

The external assignment becomes the tool definition's required permission. The
runtime or Generic Codex Tool Bridge still checks that requirement against the
host's allowed permission levels before `ToolRegistry` dispatch. Permission
ownership therefore remains with the host at both composition and execution.

Trusted profiles may additionally declare closed `mcp_providers` and
`process_plugins` entries. Each
entry refers to an existing symbolic executable resource and contains a unique
provider ID plus an exact set of remote names, object schemas, and explicit RAH
permissions. Raw paths, argv, cwd, environments, inherited environment, and
resource-limit overrides are not profile fields. Static validation does not
launch a child; the separate explicit `profile validate-effective` operation
does. It composes into a fresh registry and returns no registry or inventory if
any provider fails. The effective profile owns the live MCP and Process Plugin adapters, so proxy
tools cannot outlive their provider connection. Inventory shows only symbolic
provider/tool identities and never executable paths, cwd, environment, stderr,
or child diagnostics.

`profile validate` is non-spawning static/source/schema/resource validation.
`profile validate-effective` is explicit effective composition and may launch
trusted configured provider processes for handshake, discovery, and admission.
Neither operation discovers, edits, reloads, or grants a model authority to
choose profiles or provider configuration.

## Built-in filesystem and subprocess tools

`FsReadTool` canonicalizes paths through `WorkspacePolicy`, rejects traversal and
outside-workspace paths, limits bytes, and rejects non-UTF-8/binary input.

`ShellExecTool` uses a program plus argument vector, validates its working
directory, captures stdout/stderr/exit status, and supports timeout through the
sandbox abstraction. These controls are policy and process boundaries; RAH does
not claim that path checks or process supervision provide strong OS isolation.
Because `ShellExecTool` accepts model-selected process details, ADR 0009 leaves
it unsuitable for live model exposure.

The deterministic v0.3 Execute prototype instead uses a capability-specific
`HostExecutionTool`. Its immutable `HostExecutionPolicy` selects one canonical
native executable, renders exact or typed argv, fixes cwd beneath a canonical
host root, clears and explicitly rebuilds the environment, closes stdin, fixes
the timeout, and enforces bounded concurrent stdout/stderr reads. Execute
permission remains a separate required runtime gate. Output overflow and timeout
attempt termination and return bounded structured error results; neither means
rollback. Windows uses best-effort Job Object ownership and Unix uses a
best-effort process group. These mechanisms supervise processes but do not
provide filesystem or network isolation.

The first real host-owned Execute capability is `host.cargo.version`. The host
constructs it with an absolute Cargo native executable and a non-sensitive
working directory before registration. The policy canonicalizes and records the
executable identity, revalidates it before every spawn, clears the environment,
closes stdin, applies a five-second timeout and the ADR 0009 output limits, and
supplies exactly `--version`. This narrow preauthorization does not authorize
other Cargo commands or make Cargo generally available to a model. Generic
shell execution remains disabled for model use. Because Execute can convey broad
ambient host authority in principle, each additional capability requires its
own host-owned policy and explicit registration.

`host.git.status` is a second capability-specific Execute tool. It means
"status of this host-authorized repository," not generic Git: trusted host
setup supplies an absolute native Git executable and one absolute repository
root, and the capability canonicalizes and records the root plus its `.git`
directory-or-file identity. Both repository and executable identities are
revalidated immediately before direct execution of exactly `status
--porcelain=v1`; model input cannot select Git arguments, cwd, paths,
environment, or timeout. Arbitrary Git commands and generic shell execution
remain unavailable.

The child environment is cleared. System and global Git configuration are
disabled, prompting and optional locks are disabled, and fixed command-scope
configuration disables fsmonitor and the untracked cache. Repository-local
configuration still exists as repository authority, though relevant fsmonitor
behavior is overridden. Includes and other repository-local settings may still
influence status semantics. Git ownership `safe.directory` checks remain
enabled; the capability does not inherit user configuration to bypass them, so
a host-selected repository that fails the ownership check fails closed. Status
is read-oriented but not claimed to be side-effect-free, and repository
metadata, working-tree content, and configuration remain untrusted process
inputs. No network operation is requested, but process supervision provides
neither filesystem nor network isolation. Repository and executable
revalidation also retain the documented TOCTOU limitation between final checks
and spawn.

## Deterministic repository-mutation fixture

ADR 0010 adds a deliberately narrow deterministic fixture for validating
repository-mutation authority independently of Git. `PermissionLevel::Execute`
remains required, but it is not mutation authorization: the private,
host-owned `RepositoryMutationPolicy` in `rah-tools` captures a canonical root
and root identity, maps the fixed symbolic `fixture-marker` target to one
existing regular file, and rejects links, Windows reparse points, substitutions,
and paths outside the root. The model-visible schema is an empty object; it
cannot supply a path, executable, argv, cwd, environment, or timeout.

The policy acquires an in-process lease keyed by repository identity before
pre-state capture and holds it through post-state verification and audit-result
construction. This serializes concurrent RAH mutations for one repository. It
does not prevent an external process from changing that repository; bounded
full-root snapshots detect unexpected additions, removals, or changes and fail
closed where they can be observed.

The fixture records pre/post root and target identities plus bounded content
snapshots. A successful exit code is insufficient: the only accepted effect is
the host-authorized marker replacement. Results expose only a bounded status,
symbolic target, and changed/partial/uncertain flags. They do not reveal host
paths, executable details, environment values, or audit paths.

RAH does not promise rollback. Timeout, abort, cancellation, disconnect, crash,
or a lost response after spawn may have caused an effect. The fixture reports a
post-spawn timeout with an observed mutation as uncertain and never retries it.
Dropping execution attempts process termination through the supervised-process
layer; it is not rollback. The initial fixture has no Git mutation, no network,
and no model-visible file-authoring surface.

`host.git.stage` and `host.git.unstage` are separate, host-constructed Git
capabilities using that same private mutation policy. Each accepts only `{}`
and binds one symbolic target to one tracked regular file. Stage invokes only
the fixed literal-pathspec `git add`; unstage invokes only `git
--literal-pathspecs restore --staged --source=HEAD -- <target>`. Unstage proves
that its target index entry equals the pre-observed `HEAD` tree entry and that
the full worktree snapshot, every unrelated index entry, `HEAD`, refs, and
repository identity remain unchanged. It never writes worktree bytes. Commit,
worktree restore, and all other Git mutation remain deferred.

## MCP process boundary

`rah-tools-mcp` directly launches an explicitly configured, absolute native MCP
executable without shell-string interpolation or `PATH` lookup. It rejects
symbolic links (and Windows reparse points), revalidates canonical file length
and modification identity before launch, and documents the remaining
check-to-spawn replacement race. Windows accepts only `.exe`; Unix requires a
regular executable file.

Each generation receives a host-created isolated temporary cwd and a cleared
environment. Windows retains only `SystemRoot` when it is present. The adapter
owns bounded stdio framing (1 MiB default), outstanding work (32), command
queue (64), result/output (1 MiB), host-only stderr tail (64 KiB), and retired
request tracking (64 IDs). Its bounded control queue holds every admitted
outstanding cancellation plus one stop signal. Direct process spawn fails
synchronously; initialize and discovery each time out after two seconds, tool
calls use the host-configured 30-second default, and shutdown waits 500 ms
before termination/reaping. The adapter owns the pinned stdio protocol,
request correlation, cooperative cancellation, shutdown, termination, and
process reaping. Child stderr never enters `ToolOutput`.

Discovery is all-or-nothing: the discovered remote names must exactly equal
the explicitly host-assigned set. Optional `McpExpectedTool` declarations pin
the normalized JSON input schema and permission as well. Missing, extra,
duplicate, malformed, or mismatched discovery returns no usable tool set.
Discovered definitions and results are validated and translated into neutral
RAH types; unsupported result content fails closed.

The MCP server is a separate process with its own possible filesystem, process,
and network authority. Owning and supervising that process does not sandbox its
internal actions. Cancellation may stop waiting and sends the protocol's
notification, but it is not rollback: the server may already have caused side
effects. Timed-out, cancelled, disconnected, or otherwise uncertain
`tools/call` operations are not automatically replayed.

## Process Plugin process boundary

`rah-tools-plugin` launches an explicitly selected absolute native executable
directly and uses RAH process-plugin protocol version `1` over bounded NDJSON
stdio. It rejects direct symlinks (and Windows reparse points), scripts and
non-native Windows executables, validates a canonical regular-file identity,
and revalidates length/modification identity immediately before spawn. This
narrows but cannot eliminate the final check-to-spawn replacement TOCTOU race.
It validates configured/reported identity and version before discovery. The
host controls every permission assignment.

The adapter applies bounded plugin IPC, including limits for queued commands,
outstanding requests, message bytes, result bytes, discovered metadata, and
retired request tracking. Plugin stderr is drained into a bounded, lossy,
control-escaped, host-only diagnostic tail; it is never tool output,
model-visible data, or authorization input.

The inherited environment is cleared. Only `RAH_PLUGIN_PROTOCOL`, Windows
`SystemRoot` where required to launch the child, and explicit host-allowlisted
name/value pairs are provided. Each generation receives a newly created isolated
temporary working directory rather than the RAH workspace, and the adapter
removes it after process termination.

Cancellation is best effort and is not rollback. A plugin can finish or begin an
external side effect before cancellation is observed. Each tool call is sent at
most once; timed-out, disconnected, and otherwise uncertain external calls are
never automatically replayed. Existing proxies fail after disconnection until
the host explicitly creates a new adapter.

Environment minimization, an isolated cwd, resource bounds, and child-process
supervision reduce accidental ambient authority and denial-of-service exposure.
They are not OS sandboxing and do not prevent arbitrary child syscalls,
filesystem access, subprocess creation, or network access.

Process Plugin discovery is all-or-nothing. The host may pin an exact expected
tool set, each with a recursive object-key-order-normalized JSON input schema
and explicit permission. Missing, extra, duplicate, malformed, invalid, or
schema-mismatched tools fail provider construction and publish no usable proxy.
Legacy explicit permission assignments still constrain the exact name set, but
only an expected-tool declaration pins schema equality. Child metadata never
grants authority.

## Restricted Codex adapter

The adapter pins the executable and schema contract before use. It owns stdin,
stdout, bounded retained stderr, JSON-RPC correlation, abnormal-exit reporting,
shutdown, and active-turn interruption.

Restricted mode disables Codex-owned shell, unified execution, file, MCP, web,
image, app, and approval surfaces. Generic bridge mode enables only the
version-pinned dynamic-tool request transport used to reach RAH's registry.
Codex `mcp_servers` remains empty. MCP elicitation and all approval requests are
rejected, while command, file-change, and MCP tool items fail the RAH stream.
They never become RAH tool lifecycle events because RAH did not authorize them.

Codex sandbox settings are defense in depth, not a replacement for RAH policy,
registry, tool, or sandbox contracts.

## Known limitations

RAH Desktop stores bounded completed conversation text in the current user's
application-local `conversation-transcript.sqlite3` database. Schema version
`1` contains `schema_metadata`, `namespaces`, `epochs`, and `pairs`, with
private opaque repository SHA-256 namespaces (or `neutral-v1`); it intentionally
does not expose raw repository paths, credentials, provider-native IDs,
ToolRegistry authority, tool output, or a generic SQL surface. Normal Windows
user-account filesystem protections remain the first-version at-rest boundary.

An absent database and valid V3 JSON migrates transactionally. Once that commit
succeeds, SQLite is authoritative even if archival of V3 fails; stale V3 never
wins. Corrupt authoritative SQLite is quarantined and fails closed, never
falling back to V3. V1/V2 data is never guessed into a repository or neutral
owner, and there is no dual write or active JSON backend after SQLite authority.

Recovered transcript text is display-only. Explicit Resume imports bounded
completed text only after a fresh current connection verifies; it restores no
model, repository, tool, replay, or other authority. Users can clear this local
Desktop transcript state without deleting repository/project files.

- Interactive Codex approvals are unsupported.

Desktop Resume Previous Conversation is an explicit model-context import, never
an automatic restart action. It uses only completed persisted text and lineage
after a fresh current connection is verified; the current host repository,
model, and tool state remain authoritative. Resume restores neither authority
nor a native Codex thread. Repository identity is only an opaque private
namespace, not a grant of repository authority.

- The Codex adapter and external protocols are exactly pinned compatibility
  boundaries.
- Cancellation across any external process boundary cannot undo side effects.
- The broadcast event buffer is bounded; a lagging consumer receives a terminal
  failure instead of silently losing security-relevant events.
- Deterministic tests validate translation, policy, and lifecycle behavior; they
  do not claim live model, credential, third-party server, or platform-sandbox
  validation.
- The host-selected ADR 0015 endpoint is bounded initial provider configuration,
  not transport confinement. Redirect/proxy/DNS/peer identity/effective
  destination guarantees and Task 120 remote generation proof are not claimed.
