# RAH — Rust Agent Harness

RAH is a model-provider-agnostic, runtime-pluggable agent harness written in Rust.
It owns neutral runtime, model, event, session, tool, permission, and sandbox
boundaries. RAH orchestrates inference providers; it is not an inference engine
and does not load model weights or implement model execution.

## RAH v0.24.0 released

RAH v0.24.0 is the current immutable published release, with the release theme
**HostExplicit Reviewed File Deletion (`repo.delete-file`)**. v0.23.0 is the
prior immutable published release.

The immutable v0.24.0 release source is
`2d4cf0e4d89a461f54250ff81ad9808929d854f0`.

The accepted reviewed human workflow is:

```text
typed human {path}
 -> zero-effect deletion Prepare
 -> complete bounded backend-derived destructive review
 -> opaque process-local single-use ticket
 -> ticket-only Confirm
 -> connected-current/currentness checks
 -> retained deletion-preparer revalidation
 -> exact ToolDefinition / permission membership -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started -> authorized_tool_dispatch
 -> current ToolRegistry -> existing repo.delete-file
 -> ADR 0017 RepositoryFileDeletionPolicy
 -> strict five-status parsing -> independent reviewed-route proof
 -> status-only terminal HostActivity -> descriptive repository refresh
```

ADR 0017 remains the sole underlying repository file-deletion mutation
authority. ADR 0021 is the generic HostExplicit coordinator/currentness,
ticket, D2, and provenance boundary. ADR 0025 is the reviewed human deletion
HostExplicit boundary. The frontend is presentation and typed input only;
model output, human confirmation, `PermissionLevel::Execute`, Tool presence,
Trusted Profile/provider metadata, MCP, and Process Plugin metadata are not
authorization. There is no Desktop direct delete path.

The exact ten production HostExplicit Tools are:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
repo.delete-file
```

`repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools, Process
Plugin Tools, fixture/diagnostic Tools, and unknown Tools remain ineligible.
Admission is exact and host-owned, with no wildcard, prefix, effect-category,
permission-derived, or provider-derived rule.

The closed Prepare request is `{path}`. The reviewed route bounds the
canonical request to 8192 bytes, the logical path to 1..=1024 UTF-8 bytes, the
source to 0..=65536 raw bytes of strict UTF-8 with no NUL, the complete
serialized review to 262144 bytes, and retained private preparation to
524288 bytes. The complete escaped source is shown; it is not a preview and
is never truncated. The ordinary ADR 0017 Tool remains broader: it retains
`path`, `expected_file_sha256`, and `expected_file_byte_length`, a 1 MiB
ordinary-file bound, and binary ordinary deletion.

The reviewed target is exactly one existing clean HEAD-tracked regular file
with one normal stage-0 entry, HEAD/worktree/index equality, mode 100644 or
100755, captured FileIdentity, link count 1, supported repository state,
strict UTF-8, no NUL, and at most 64 KiB. A verified deletion leaves one
unstaged Git deletion only; it does not Stage, Unstage, Commit, mutate the
index/HEAD/refs/history, rename, move, restore, clean up, or recurse.

The five statuses are `deleted_verified`, `known_no_effect`, `invalid_input`,
`precondition_failed`, and `uncertain`. The first requires valid Tool output
and independent confirmed-absence proof; the second requires valid Tool
output and independent exact-original-preimage proof. Malformed,
contradictory, or post-Started failures remain uncertain when proof is
insufficient. Windows uses one native `DeleteFileW` attempt and one immediate
proof pass, with no polling, retry, replay, cleanup, or rollback. The accepted
Task 285 Windows success evidence is carried forward and the destructive live
test is not rerun.

## RAH v0.23.0 released — previous immutable release

RAH v0.23.0 is the previous immutable published release for **HostExplicit
Reviewed New-File Authoring (`repo.create-file`)**. v0.22.0 is the prior
immutable published release.

```text
typed human {path, content}
 -> zero-effect shared Prepare
 -> complete bounded backend-derived review
 -> opaque single-use ticket-only Confirm
 -> currentness / exact-definition / permission / preparer checks
 -> shared revalidation -> D2 -> reviewed Commit invalidation
 -> HostExplicit Started -> authorized_tool_dispatch
 -> current ToolRegistry -> existing repo.create-file
 -> ADR 0013 RepositoryFileCreationPolicy
 -> strict result classification -> descriptive repository refresh
```

ADR 0013 remains the sole underlying file-creation authority; ADR 0024 owns
the capability-specific reviewed HostExplicit route; ADR 0021 owns generic
HostExplicit coordination, currentness, D2, tickets, lifecycle, and
provenance. HostExplicit is not generic filesystem authority, and model output
or provider metadata is never authorization.

The request is closed to exactly `{path: String, content: String}`. Unknown
fields fail closed. The route permits exactly one file, a 1..=1024 UTF-8 byte
path, 0..=262144 UTF-8 byte content, NUL rejection, a canonical request no
larger than 327680 bytes, and a complete review no larger than 262144 bytes.
Empty content is allowed and content bytes remain exact, without BOM/newline
conversion, Unicode normalization, templating, append, overwrite, or an
automatic final newline. Complete review is required; it may not be truncated
or ellipsized.

The exact nine HostExplicit Tools are:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
```

`repo.delete-file`, `repo.rename-file`, `repo.create-directory`, `repo.commit`,
MCP Tools, Process Plugin Tools, fixture Tools, and unknown Tools remain
ineligible. There is no wildcard, category, or provider-metadata route.

The existing parent must already be safe and identity-verified. The target must
be absent from the worktree, HEAD, and every index stage, including
intent-to-add. Ignored targets, submodules, unsupported sparse-checkout,
Windows reserved/device/ADS/UNC/verbatim names, reviewed trailing-dot/space
components, and symlink/junction/reparse traversal fail closed. Parent mkdir,
overwrite, append, delete, rename, Stage, and Commit do not occur. Exclusive
create-new is the native commit point; complete writing is not an atomic
all-or-nothing transaction.

The exact results are `ok`, `invalid_target`, `precondition_failed`,
`create_failed_known`, `write_failed_known`, and `uncertain`. Known no-effect
classification requires proof; `write_failed_known` may retain an attributable
empty or partial file; `uncertain` preserves unknown effect. The route does not
retry, replay, clean up, roll back, restore, or compensate. Its ticket is
opaque, process-local, in-memory, capability-specific, single-use, bound to
the exact preparation/currentness, and valid for an inclusive five-minute TTL.
Confirm and Cancel receive only the ticket ID. Generic activity has a distinct
non-authority ID (`ticket_id != activity_id`) and excludes authority tickets,
source-bearing review/content, raw Tool input/output, and private identities.

Task 274's accepted Windows 10 connected-current host evidence is carried
forward without rerunning the live gate or submitting a model prompt. It is
not model-selected evidence; zero model lifecycle counts do not certify a
model execution. The optional live Cancel-before-start subcase was not rerun;
deterministic cancellation evidence was accepted by Task 275. The release
preparation baseline is `533b0769618d25c1b9673a27c3e21af7a48809ca`, and this
preparation creates no tag, GitHub Release, or published artifact.

## RAH v0.22.0 released — historical

RAH v0.22.0 is an older immutable published release for **HostExplicit
Reviewed Multi-File Edit Authoring**. v0.21.0 is the prior immutable published
release.

The connected-current Desktop human workflow is:

```text
typed bounded multi-file request
 -> zero-effect Prepare
 -> complete backend-derived ordered review
 -> opaque ticket-only Confirm
 -> shared revalidation / D2
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> ToolRegistry
 -> existing repo.edit-files / ADR 0014
 -> strict result classification / descriptive repository refresh
```

The request is closed and typed: 1–4 existing clean HEAD-tracked regular
strict-UTF-8 files, 1–16 exact literal replacements per target, and at most 64
replacements total. The host derives exact original-snapshot matching,
preimages, postimages, hashes, lengths, and deterministic canonical path order;
frontend input order is not execution order. The operation is explicitly
non-atomic and preserves `ok`, `invalid_target`, `precondition_failed`,
`failed_known_no_effect`, `partial_effect`, and `uncertain` results.

The exact v0.22 HostExplicit eligibility is:
`fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`,
`repo.create-branch`, `repo.patch`, and `repo.edit-files`. The ineligible set is
`repo.create-file`, `repo.delete-file`, `repo.rename-file`,
`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools, and
unknown Tools. There is no wildcard or category-based eligibility.

ADR 0014 remains the existing underlying `RepositoryMultiFileMutationPolicy`;
ADR 0023 defines this capability-specific reviewed HostExplicit route; and ADR
0021 remains the general HostExplicit coordinator/currentness/D2 boundary.
Model output is never authorization. Frontend state is presentation and typed
input only. Permission classification, Trusted Profile composition, and
provider metadata cannot create mutation authority or self-enable HostExplicit.

Prepare is zero-effect. Confirm and Cancel receive only a process-local,
in-memory, single-use opaque ticket with an inclusive five-minute TTL bound to
the exact change and currentness. Generic HostExplicit activity carries a
separate RAH-generated activity ID: it is not the ticket, is not derived from
the ticket, cannot Confirm or Cancel, and the actual ticket and source-bearing
review content are excluded. Task 263 Attempt 1 discovered the ticket leak
under `invocationId` before Confirm with zero effect; Attempt 2 corrected it and
passed on a fresh repository.

The workflow never retries, replays, continues a prefix, rolls back, restores
preimages, stages, or commits automatically. `repo.patch` and `repo.edit-files`
invalidate stale reviewed Commit authorization at the Started boundary; fresh
repository refresh is descriptive only. A narrow empty-index Commit-review
fallback keeps descriptive status/diff refresh available without fabricating or
granting Commit authority.

Task 263 final Attempt 2 is the carried-forward Windows connected-current
production evidence: Windows 11 IoT Enterprise LTSC x64, Codex `0.149.0`,
SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`,
model `gpt-5.6-terra`, medium reasoning, caller order `d.txt, b.txt, a.txt,
c.txt`, backend/effect order `a.txt, b.txt, c.txt, d.txt`, Prepare `0/0`,
Confirm `1` Tool, native attempts `1/1/1/1`, final generations `[1, 0, 0, 1]`,
Idle coordinator/chat, zero model lifecycle/MCP/Process Plugin activity, and
marker `RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK`. This is host-driven evidence,
not a model-selected HostExplicit execution claim.

Known nonclaims include Unix live parity, generic filesystem writing,
structural HostExplicit authoring, HostExplicit `repo.commit`, MCP or Process
Plugin HostExplicit, network isolation, race-free TOCTOU, OS sandboxing, and
automatic recovery. Process supervision is not OS sandboxing.

## RAH v0.21.0 released

v0.21.0 is the previous immutable published release. v0.20.0 preceded it.

RAH v0.21 adds HostExplicit Reviewed Single-File Patch Authoring:

```text
inspect
 -> typed human repo.patch Prepare
 -> exact bounded review
 -> ticket-only Confirm
 -> existing repo.patch
 -> inspect diff
 -> existing Stage / Unstage
 -> existing reviewed Commit
```

The human supplies only typed `path`, `expectedOldText`, and `replacementText`.
The host derives the preimage/postimage SHA-256 and lengths and canonical
`ToolInput`, prepares a non-effectful exact escaped review, and issues an opaque
single-use five-minute ticket. Confirm receives only the ticket ID and performs
shared preparation revalidation, D2 preflight before `Started`, and
`authorized_tool_dispatch` before the existing ADR 0012 `repo.patch` mutation.
Malformed or uncertain post-start results are handled conservatively; there is
no retry, replay, rollback, or restore-preimage.

The exact v0.21 HostExplicit eligibility is:
`fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`,
`repo.create-branch`, and `repo.patch`. `repo.create-file`, `repo.edit-files`,
`repo.delete-file`, `repo.rename-file`, `repo.create-directory`, `repo.commit`,
MCP Tools, and Process Plugin Tools are not eligible.

Task 252 Verdict A completed the milestone audit. Task 251 provides the final
Windows connected-current production-backend certification: Prepare `0 Tool / 0
replacement`, Confirm `1 Tool / 1 replacement`, `ChangedVerified`, exact
postimage, protected repository state unchanged, and no model or external
provider activity. This is not a model-selected or GUI mouse-automation claim.

## RAH v0.20.0 released

At publication, v0.20.0 was the current immutable published release. v0.19.0
was the prior release.

RAH v0.20 adds an explicit Desktop Host Tool invocation workflow for a closed
first-party Tool set. The exact first-release HostExplicit eligibility is:
`fs.read`, `repo.file-info`, `repo.status`, `repo.diff`,
`repo.diff-staged`, and `repo.create-branch`.

HostExplicit is a connected-current backend Host action — not Model activity.
The backend chooses typed operations, retains the current registry, expected
definitions, permission policy, selected repository/context, and bound
generations, then revalidates through the shared D2 current-definition and
permission gate. `repo.create-branch` uses Prepare, sanitized review, and
explicit Confirm with a single-use opaque ticket. There is no generic
`ToolName + arbitrary JSON` console and no authority amplification.

Windows Desktop connected-current explicit Host Tool invocation is certified
for `repo.status` and `repo.create-branch` using the production HostExplicit
backend path. No model request or model Tool lifecycle was used. The other
four eligible Tools are deterministically verified but were not separately
live certified. Real GUI mouse-click automation and Linux/macOS HostExplicit
live behavior were not certified.

Model-selected `repo.create-branch` dispatch remains not certified from the
historical Task 229 evidence. Task 207 remains unchanged; MCP and Process
Plugin Tools are not HostExplicit eligible in v0.20.

## RAH v0.19.0 released

RAH v0.19.0 is the prior immutable published release for the Bounded Local
Branch Creation at Captured Attached HEAD milestone. v0.18.0 preceded it.

The accepted ADR 0020 capability is `repo.create-branch`. It accepts only
`{"name":"<validated-logical-branch-name>"}` and, when the host has composed
the separate authority for the selected repository, creates one absent local
`refs/heads/<name>` at the host-captured attached committed `HEAD`. The fixed
expected-absence CAS and Git-owned reflog are host-controlled. It does not
switch branches, change the index or worktree, set tracking, or provide
generic Git/ref/history authority. Execute is only the outer dispatch gate;
Trusted Profile/provider metadata and frontend presentation cannot create this
authority.

Windows host-driven Desktop repo.create-branch authority/effect path is certified. Model-selected repo.create-branch dispatch was not observed in two bounded Codex live attempts and is not certified.

The fresh host-driven branch name and OID were internally asserted but not
printed on the successful output path. Linux live branch certification was not
established, and the accepted Task 207 external-provider limitation remains
unchanged.

## RAH v0.18.0 released

RAH v0.18.0 delivers Inert Trusted Profile Persistence and Explicit Restore.
Desktop can remember one host-selected Trusted Profile source path locally, but
startup is remembered-not-restored: it does not select, validate, compose,
spawn, advertise, or otherwise restore provider authority. Explicit Restore
freshly loads and statically validates the current source without spawning.
Only explicit Connect or reconnect may activate admitted local stdio MCP and
Process Plugin providers.

Forget removes only the durable preference; it does not alter an already
connected composition. Profile generation participates in currentness, and
Connect fresh-loads current Trusted Profile bytes. Effective Authority remains
sanitized and keeps Configured, Effective, Advertised, and Current state
separate. Provider lifecycle ownership and conservative external-effect
handling remain unchanged.

Task 207 remains **INCONCLUSIVE / externally blocked for the model-selected
external Tool execution sub-gate**. Windows provider selection, admission,
composition, effective inventory, dynamic Tool advertisement, lifecycle
ownership, and cleanup were verified. Actual model-selected MCP/Process Plugin
Tool execution was not established with the tested current ChatGPT-auth
Codex model/runtime combinations. The hardened gates failed closed at
`ToolRequested=0`; no RAH product defect was found. Real external-effect
review invalidation and repository refresh are deterministically verified but
not live-certified through a real external provider model call. This is the
accepted non-blocking Task 207C / Task 208 limitation.

The certified Codex baseline remains exactly `codex-cli 0.149.0` with
`codex.exe` SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
The current live-gate model is `gpt-5.6-terra`. `codex-cli 0.153.4` is
research-only compatibility evidence, is not certified, and did not restore
Tool selection.

v0.18 does not add active-provider auto-restore, profile hot reload, network or
Streamable HTTP MCP, provider download/install/update, PluginManager expansion,
generic shell/process, filesystem, or generic Git/branch/ref/history authority,
OS sandboxing, network isolation,
rollback, absence of ambient external provider effects, or Linux
Desktop persistence lifecycle certification. `repositoryBound=false` and
`PermissionLevel` do not prove that an external provider cannot affect
repository or host state.

v0.19 adds only ADR 0020 bounded local branch creation; generic Git/ref/history
authority remains absent.

## RAH v0.16.0 released: Effective Authority Review

RAH v0.16.0 is released. The Desktop Effective
Authority panel is an informational review surface for the current host-composed
Tool inventory. It distinguishes configured, effective, and runtime-advertised
state and shows generation-aware repository/runtime current, stale, and
reconnect-required state, public Tool names, host-derived effect/authority/
permission/source classifications, bounded unavailable-capability reasons, and
reviewed-commit presentation state.

The panel does not grant, revoke, edit, reload, or otherwise compose authority.
Refresh Authority is read-only. Model Tool requests remain subject to
ToolRegistry lookup, host PermissionLevel and policy checks, repository/workspace
constraints, generation/precondition checks, and one-shot reviewed-commit
authorization where applicable. Configured, effective, advertised, and
individual request authorization remain distinct. No new model-accessible
authority was added.

Windows Effective Authority live certification is PASS using the certified
`codex-cli 0.149.0` baseline; Linux live certification is not established.
MCP and Process Plugin remain existing Tool providers, but their Effective
Authority presentation is not currently reachable through the Desktop
composition path and is not claimed as live-certified in v0.16.

## v0.15.0 released: bounded repository directory creation

RAH v0.15.0 is released. It adds the separate,
host-owned `repo.create-directory` capability described below. The v0.14
bounded file rename/move capability remains distinct.

RAH v0.10.0 established the existing bounded Desktop host configuration.
Desktop selects a certified `codex-cli 0.149.0` baseline, accepts one
human-selected bounded `llama_cpp` endpoint under ADR 0015, and retains saved
model preferences as inactive desired state until an explicit Connect or
reconnect. It neither manages a llama.cpp process nor installs a provider or
model.

The selected repository is canonical host-owned context: native Git discovery
and observation are fixed to that repository, Codex starts with its verified
repository CWD (or an app-owned neutral workspace), and launch-CWD/`AGENTS.md`
context cannot substitute it. Conversations are stored privately in
repository-scoped SQLite namespaces; Resume is explicit bounded display/context
replay and never restores repository, model, tool, or other authority.

Remote llama.cpp generation proof remains **DEFERRED / NOT VALIDATED**. A
bounded initial endpoint is not transport confinement: redirect, proxy, DNS,
peer-identity, and effective-destination guarantees are **NOT CLAIMED**.

### Desktop authoring, deletion, review, and bounded repository commit

The v0.14 candidate retains the existing bounded Desktop local repository
workflow:

```text
inspect -> model bounded authoring -> human Stage / Unstage -> host-observed staged review -> human reviewed-snapshot authorization -> message-only bounded commit -> verified result / refresh
```

The model-visible commit Tool is `repo.commit`, with the closed input
`{"message":"..."}`. It requires `PermissionLevel::Execute` as an outer gate,
but Execute alone is not repository-history authority. A trusted profile must
compose the exact host-selected repository, native Git executable, and identity,
and the host must separately authorize one fresh reviewed staged snapshot for
each call. Stage / Unstage are host actions, not model Tool authority; the
frontend presents host-owned state but does not own authorization. The
authorization is in-memory, one-shot, and never restored or replayed.

`repo.commit` creates at most one ordinary commit from the already staged
snapshot on the current attached branch. It never stages files, accepts no
branch/ref/path/Git argv/identity input, and grants no amend, merge, rebase,
cherry-pick, tag, remote, credential, network-Git, or generic Git authority.
Uncertain external effects are not retried or rolled back. Windows local live
validation is certified with the complete official Codex 0.149.0 runtime
(including its same-version code-mode host); Ubuntu CI is deterministic
evidence, not Linux live certification.

The v0.13 release added `repo.delete-file`, a separate ADR 0017 authority for
deleting exactly one explicitly named repository-relative regular file. The
target must be clean, HEAD-tracked, and match the exact authorized HEAD blob
preimage, including SHA-256 and byte length. The operation makes one native
worktree deletion attempt, never auto-stages, and does not grant commit, ref,
history, or network Git authority. Trusted Profile composition and the Generic
Codex Tool Bridge can expose the capability only when the host has already
constructed the separate deletion authority; neither model requests,
provider metadata, Execute permission, tool definitions, nor the frontend can
manufacture it. The canonical public tool name is `repo.delete-file`; any
provider-private alias is an implementation detail.

Windows live-certified v0.13 evidence used `codex-cli 0.149.0` and observed
one request, start, and finish, verified deletion of the intended target, an
unchanged sentinel and index, an unstaged deletion, unchanged HEAD/refs/
history, no replay, and `RAH_REPO_DELETE_FILE_LIVE_OK`. Linux live
certification is not established.

RAH v0.14 adds `repo.rename-file` under accepted ADR 0018. It moves exactly
one clean, HEAD-tracked regular file within the selected repository, either in
the same directory or to another existing directory in that repository. The
request supplies `source_path`, `destination_path`,
`expected_source_file_sha256`, and `expected_source_file_byte_length`. The
destination must be absent and is never overwritten. The host revalidates
repository and runtime-generation identity immediately before one native
no-replace rename/move attempt; a possible effect is never replayed. This is
not generic filesystem rename authority, and it grants no create, delete,
content-write, index, commit, shell, process, or Git authority.

RAH v0.15 adds `repo.create-directory` under accepted ADR 0019. It creates
exactly one new ordinary directory leaf at an explicit repository-relative
path. The parent must already exist and the destination must be absent. This
separate `RepositoryDirectoryCreationPolicy` does not recursively create
parents, ensure an existing directory, create placeholder files, or mutate
Git. A possible effect is never replayed or rolled back.

## Preserved bounded repository mutation and workflow inspection

RAH retains the v0.4 trusted-host static capability profile and the v0.5
separate, accepted worktree-content authority: `repo.patch`. It can
conditionally perform a legacy single exact replacement or one to sixteen
bounded exact replacements within one existing, HEAD-tracked, unstaged,
strict-UTF-8 worktree file. All matches use the same original snapshot;
overlapping replacements are refused and non-overlapping replacements are
applied deterministically. Full-file SHA-256 and byte-length preconditions are
required, and the operation does not stage changes. The capability is
host-constructed through a private `RepositoryWorktreeMutationPolicy`; it is
not generic filesystem write, shell/process, index, or Git history authority.

RAH additionally provides four host-fixed, read-only repository observers:
`repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`. They
inspect one validated repository-relative path, normalized repository status,
unstaged worktree-versus-index changes, and staged index-versus-HEAD changes.
They are `Execute`-gated subprocess capabilities, not generic Git or filesystem
authority; model input cannot select their executable, argv, cwd, environment,
repository, refs, or baselines. Their precise claim is **no intentional
repository mutation**, not zero incidental filesystem writes.

RAH v0.8.0 adds `repo.create-file`: one host-authorized
exclusive creation of one absent UTF-8 file at a model-selected, validated
repository-relative path. It uses a separate private
`RepositoryFileCreationPolicy` (ADR 0013), requires an existing real parent,
rejects links/reparse traversal, ignored/index/HEAD/submodule/sparse targets,
and never overwrites, creates directories, appends, stages, or mutates Git
history or refs. One call creates one file only: paths are limited to 1024
UTF-8 bytes, content to 256 KiB, and the serialized request to 320 KiB. It is
not generic filesystem-write authority and provides no rollback or replay.

RAH v0.9.0 adds `repo.edit-files`: one through four existing, clean,
HEAD-tracked strict-UTF-8 files with exact SHA-256 and byte-length
preconditions, all replacements resolved against original snapshots, and
deterministic host-owned commit order. It is not transactional and provides no
rollback or replay. The capability is composed through Trusted Profile v1 and
the Generic Tool Bridge; certified Windows live validation using exactly
`codex-cli 0.149.0` emitted `RAH_REPO_EDIT_FILES_LIVE_OK`.

```text
Built-in Tool -----------\
MCP-backed RAH Tool ------+-> Tool -> ToolRegistry -> host permission -> execution
Process Plugin RAH Tool --/
```

The deterministic native runtime demonstrates the complete neutral loop:

```text
User input
 -> AgentRuntime
 -> ModelBackend
 -> ToolCall
 -> ToolRegistry
 -> host permission policy
 -> Sandbox / workspace policy where applicable
 -> Tool
 -> ToolOutput
 -> ModelBackend
 -> AgentEvent stream
 -> final output
```

The built-in `EchoTool`, `FsReadTool`, and `ShellExecTool` implement the same
provider-neutral `Tool` trait as external-tool proxies. `FsReadTool` is validated
through the registry with an explicit host `Read` permission and workspace path
policy in deterministic tests and an opt-in live Codex example.

`ExternalToolIdentity` gives each discovered external tool a host-side identity.
`ExternalToolPermissionPolicy` is default-deny: an MCP or process-plugin tool is
not registered unless the host explicitly assigns its RAH `PermissionLevel`.
External metadata never grants permission.

ADR 0011 defines the explicitly selected trusted-host capability profile as the
composition boundary for already-approved built-in capabilities and external
providers. It is not model authority and does not replace capability-specific
permission, execution, workspace, or repository-mutation policies.

The authority path is deliberately host-owned:

```text
trusted host
 -> explicit trusted static profile
 -> source validation
 -> symbolic resource resolution
 -> capability/provider-specific constructor and security policy
 -> exact provider admission
 -> fresh ToolRegistry
 -> runtime/model-visible Tool definitions
```

Profiles configure existing authority. A model request remains non-authoritative.

## Generic Codex Tool Bridge

Codex is an optional adapter, not RAH's architecture. `CodexRuntime` implements
`AgentRuntime` and communicates with an exactly version-pinned `codex app-server`
subprocess over newline-delimited stdio JSON-RPC. It does not depend on Codex Rust
crates.

The sole supported Codex baseline is exactly `codex-cli 0.149.0`; RAH does not
claim multi-version Codex compatibility.

In explicitly enabled bridge mode, the Generic Codex Tool Bridge snapshots the
host-supplied `ToolRegistry`, translates definitions to private Codex dynamic-tool
definitions, and translates requests back into RAH `ToolCall` values. RAH then
performs permission checks and dispatches through `ToolRegistry`. Codex never
executes or authorizes the tool itself.

RAH-owned MCP and process-plugin tools use this generic path as ordinary tools:

```text
Codex dynamic-tool request
 -> Generic Codex Tool Bridge
 -> RAH ToolCall
 -> ToolRegistry
 -> MCP-backed or Process Plugin-backed RAH Tool
 -> RAH ToolOutput
 -> Codex model continuation
```

This is distinct from Codex-owned capabilities. Codex-owned shell execution,
file operations, MCP, web search, image viewing, apps, and approval flows remain
disabled. Codex `mcp_servers` remains empty, MCP elicitation and approval requests
are rejected, and Codex shell/file/MCP tool items fail closed.

## External tool adapters

- `rah-tools-mcp` implements a RAH-owned, pinned MCP `2025-06-18` stdio client,
  discovers server tools, and exposes immutable `Tool` proxies. The current
  deterministic fixture provides `mcp.test.echo`.
- `rah-tools-plugin` implements RAH process-plugin protocol version `1` over
  bounded NDJSON stdio. It validates host-configured identity, clears and
  allowlists the child environment, assigns an isolated working directory, and
  exposes discovered tools such as `plugin.test.echo` through `ToolRegistry`.

Neither external adapter grants authority to its child process, and process
supervision is not advertised as operating-system sandboxing.

## Preserved v0.3 host capabilities and validation fixtures

### Public / host capabilities

The v0.3 host-owned Execute capabilities are deliberately narrow and must be
constructed and registered by a trusted host:

- `host.cargo.version`
- `host.git.status`
- `host.git.stage`
- `host.git.unstage`

`host.cargo.version` and `host.git.status` use a fixed trusted executable,
canonical host-selected working location or repository, fixed argv, cleared
environment, closed stdin, bounded output, and timeout. `host.git.stage` and
`host.git.unstage` additionally use the private `RepositoryMutationPolicy`.
Each accepts only `{}` and operates on one host-selected, tracked regular-file
target. They modify only the Git index; they do not write worktree bytes, move
refs, create commits, or use network Git.

`fs.read`, the Generic Codex Tool Bridge, the MCP Tool adapter, and the Process
Plugin Tool adapter are preserved v0.3 components. They use the same RAH-owned
`ToolRegistry` and permission boundary; v0.4 composes them but does not present
them as new capabilities.

### Validation fixtures

The following are deterministic and opt-in live-validation infrastructure, not
production or public host capabilities:

- the hardened Execute fixture, exposed to its tests/live bridge as
  `process.test.echo`;
- the repository-mutation fixture, used to validate `RepositoryMutationPolicy`
  before Git capabilities are exercised.

In particular, RAH v0.3 does **not** provide `host.fixture.echo`.

## Run the deterministic demo and profile validation

The CLI uses scripted model output and requires no model, credentials, network,
or GPU:

```powershell
cargo run -p rah-cli -- run "hello from rah"
cargo run -p rah-cli -- run "read Cargo.toml and report the workspace package information"
cargo run -p rah-cli -- tools
cargo run -p rah-cli -- doctor
cargo run -p rah-cli -- profile validate C:\\trusted-host\\rah-profile.json
cargo run -p rah-cli -- profile validate-effective C:\\trusted-host\\rah-profile.json
```

The manifest-report command dispatches `fs.read` through `ToolRegistry`, an
explicit host `Read` permission, and the workspace path policy.

`profile validate` is non-spawning static/source/schema/resource validation. It
accepts one explicitly supplied absolute trusted-profile path, then prints only
its redacted static inventory. Before parsing, the loader requires a bounded
UTF-8 regular file and rejects links and Windows reparse points. On Windows it
accepts only drive-rooted paths; UNC, verbatim/device paths, ADS, and lexical
aliases are rejected.

`profile validate-effective` is explicit effective composition. It may launch
the trusted MCP and Process Plugin executables named by the selected profile,
performs handshake/discovery/exact schema admission, and prints a redacted
effective inventory. It builds a fresh registry and publishes nothing on
failure. Neither command discovers profiles, selects one from environment or
repository configuration, reloads a profile, or enables model provider
selection.

### Trusted `repo.patch` profile binding

The existing hardened `repo.patch` capability can be requested only through a
trusted static profile using host-owned symbolic resources. Its narrow profile
entry is:

```json
{
  "resources": {
    "executables": {
      "git": { "path": "C:\\host-tools\\git.exe", "kind": "native" }
    },
    "repositories": {
      "worktree": { "path": "C:\\host-worktrees\\project" }
    }
  },
  "capabilities": [
    {
      "name": "repo.patch",
      "enabled": true,
      "permission": "execute",
      "executable": "git",
      "repository": "worktree"
    }
  ]
}
```

`execute` is the existing outer permission because the capability performs
bounded host-owned Git observations. It is necessary but not sufficient:
`PermissionLevel::Execute` is not generic worktree-write authority. During
effective composition the host resolves the two symbolic resources and invokes
the existing `RepositoryWorktreePatchTool` constructor; that constructor alone
creates the private `RepositoryWorktreeMutationPolicy`. The profile cannot
deserialize, construct, configure limits for, or bypass that policy.

The entry accepts no raw repository root, Git command/argv, shell command,
environment, mutation-policy settings, or filesystem write scope. The
repository resource must pass the existing constructor's canonical worktree,
repository identity, and confinement checks. `repo.patch configured` therefore
does not mean arbitrary writes are authorized, and a model call remains only a
request that must still pass `ToolRegistry`, runtime permission, and the private
policy's deterministic eligibility checks. This binding neither alters nor
substitutes for `WorkspacePolicy`; each capability retains its own applicable
host policy.

`rah profile validate` checks this entry's source/schema/symbolic-reference
shape only. It neither constructs `repo.patch`, runs Git, nor reads or mutates
worktree content. `rah profile validate-effective` resolves resources and may
perform the bounded non-mutating host-side construction/inspection needed to
register `repo.patch`; it never invokes the tool. Both inventories show only
the logical capability name, `Execute` permission, symbolic resource IDs, and
validation state, never native paths, file data, hashes, temporary paths,
policy internals, or environment. ADR 0012 is accepted; its worktree authority
remains separate from ADR 0010's index-only policy and ADR 0011's
composition-only profile boundary.

### Trusted `repo.create-file` profile binding

`repo.create-file` uses the same closed symbolic-resource shape as
`repo.patch`, but it constructs its separate private creation policy under
accepted ADR 0013:

```json
{
  "capabilities": [
    {
      "name": "repo.create-file",
      "enabled": true,
      "permission": "execute",
      "executable": "git",
      "repository": "worktree"
    }
  ]
}
```

The model supplies only the closed `{ "path", "content" }` request. Static
profile validation and effective composition are non-mutating; the latter
creates a fresh `ToolRegistry`. `Execute` is only the outer dispatch gate, and
neither model/provider metadata nor the profile can supply a repository root or
escalate creation authority.

## Run opt-in live Codex validation

These examples require the exactly supported Codex CLI version, configured live
model access, and may use network or paid API resources. They are excluded from
normal deterministic validation. Set `RAH_CODEX_EXECUTABLE` when `codex` is not
available through `PATH`. The Cargo and Git capability examples additionally
require an absolute trusted native executable through
`RAH_CARGO_VERSION_EXECUTABLE`, `RAH_GIT_STATUS_EXECUTABLE`,
`RAH_GIT_STAGE_EXECUTABLE`, or `RAH_GIT_UNSTAGE_EXECUTABLE`, respectively;
they create their own disposable validation repositories/targets.

```powershell
# Restricted text lifecycle and cancellation
cargo run -p rah-runtime-codex --example live_smoke -- "Reply with exactly: RAH_CODEX_SMOKE_OK"
cargo run -p rah-runtime-codex --example live_cancel_smoke

# Generic bridge with built-in RAH tools
cargo run -p rah-runtime-codex --example live_echo_bridge
cargo run -p rah-runtime-codex --example live_fs_read_bridge

# Certified isolated live-gate wrapper (pins the approved model/config surface)
.\scripts\codex-live-gate.ps1 -Command { cargo run -p rah-runtime-codex --example live_echo_bridge }

# Hardened Execute validation fixture (not a public capability)
cargo build -p rah-tools --bin rah_execute_fixture
cargo run -p rah-runtime-codex --example live_execute_fixture_bridge

# Public host capabilities; set each documented absolute trusted executable
# and repository/target configuration required by the corresponding example.
cargo run -p rah-runtime-codex --example live_cargo_version_bridge
cargo run -p rah-runtime-codex --example live_git_status_bridge
cargo run -p rah-runtime-codex --example live_git_stage_bridge
cargo run -p rah-runtime-codex --example live_git_unstage_bridge

# RepositoryMutationPolicy validation fixture (not a public capability)
cargo build -p rah-tools --bin rah_repository_mutation_fixture
cargo run -p rah-runtime-codex --example live_mutation_fixture_bridge

# Generic bridge with a RAH-owned MCP tool
cargo build -p rah-tools-mcp --bin rah-mcp-echo-server
cargo run -p rah-runtime-codex --example live_mcp_echo_bridge

# Generic bridge with a RAH-owned process-plugin tool
cargo build -p rah-tools-plugin --bin rah-plugin-echo
cargo run -p rah-runtime-codex --example live_plugin_echo_bridge

# Trusted-profile effective composition through the Generic Codex Tool Bridge
cargo build -p rah-tools-plugin --bin rah-plugin-echo
cargo run -p rah-runtime-codex --example live_trusted_profile_bridge

# Bounded repository worktree replacement (opt-in live validation)
cargo run -p rah-runtime-codex --example live_trusted_profile_repo_patch_bridge

# Bounded repository file creation (opt-in certified live validation)
cargo run -p rah-runtime-codex --example live_trusted_profile_create_file_bridge
```

The MCP and process-plugin commands exercise RAH-owned adapters. They do not
enable Codex-owned MCP, shell, or file capabilities.

### Host-attested live markers

Live examples record final assistant text as diagnostic output, but it is not
release-gate authority. A marker such as `RAH_ECHO_BRIDGE_OK`,
`RAH_REPOSITORY_OBSERVERS_LIVE_OK`, `RAH_MULTI_PATCH_LIVE_OK`, or
`RAH_CREATE_FILE_LIVE_OK` is emitted by
the host harness only after it has observed the required tool lifecycle,
validated tool outputs and state postconditions, observed `Completed`, and
cleaned up the app-server child. Model-generated marker text is weaker than
host-observed execution state: a model request or statement is not execution
evidence and never overrides the host-owned ToolRegistry, policy, or sandbox
boundaries.

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal suite uses `MockBackend`, deterministic local fixtures, fake Codex
transport, and captured Codex 0.149.0 schema/JSON fixtures. It does not require a
Codex executable, network access, credentials, a paid API, or a real model.

## Actions Cleanup

The repository-owned [Actions Cleanup](.github/workflows/actions-cleanup.yml)
workflow runs weekly on Sunday at 03:00 UTC. It retains the newest 20 completed
runs for each workflow and preserves runs whose head commit is tagged. Run it
manually with **Actions Cleanup** > **Run workflow**; `dry_run` defaults to true
and lists candidate run IDs without deleting them. Scheduled runs delete eligible
old records. The workflow uses only the repository `GITHUB_TOKEN` with
`actions: write` and `contents: read` permissions.

## v0.10 limitations and explicit deferrals

- The CLI exposes deterministic demos and explicit host-selected profile
  validation, not provider/profile auto-discovery or model-facing profile APIs.
- The Codex dynamic-tool protocol remains experimental and exactly version-pinned.
- MCP support is local pinned stdio only; Streamable HTTP and network MCP are
  not implemented.
- Process plugins are a bounded stdio protocol, not a `PluginManager`, generic
  plugin platform, installer/download mechanism, automatic restart, or hot reload.
- Profiles have no editing/mutation, discovery, auto-discovery, or hot-reload
  capability; provider schemas and generic subprocess schemas are not exposed.
- Arbitrary `shell.exec`, arbitrary `process.exec`, and model-selected
  executable, argv, cwd, environment, or timeout are not live-model authority.
- `repo.patch` is limited to a legacy single exact replacement or a bounded
  `replacements` array of one to sixteen exact replacements in one existing,
  HEAD-tracked, unstaged strict-UTF-8 worktree file. It has no automatic
  staging, file creation/deletion/rename, multi-file transaction, Git commit,
  refs/history mutation, reset, clean, checkout, switch, stash, merge, rebase,
  push, pull, fetch, network Git, or credential-bearing Git execution authority.
  Destructive worktree authority remains constrained by the private policy
  described in accepted ADR 0012; ADR 0011 is composition-only.
- `repo.create-file` creates only one previously absent UTF-8 file per call at
  an existing validated parent. It has no overwrite, mkdir, append, delete,
  rename, chmod, binary-file, multi-file transaction, staging, commit/history/
  ref mutation, rollback, or automatic replay authority. Partial files can
  remain after a possible effect and are reported conservatively; ADR 0013 is
  the separate accepted creation authority.
- `repo.delete-file` deletes only one clean HEAD-tracked regular file whose raw
  bytes match the exact authorized HEAD blob preimage, including SHA-256 and
  byte length. It is a separate ADR 0017 authority, leaves deletion unstaged,
  and has no directory/recursive, untracked, rename/move, generic filesystem,
  staging, commit, ref/history, or network Git authority.
- `repo.rename-file` moves only one clean HEAD-tracked regular file within the
  selected repository under ADR 0018. It has no directory/recursive move,
  overwrite, case-only Windows rename, generic filesystem, shell/process, or
  Git authority; create and delete remain separate authorities.
- `repo.create-directory` creates exactly one new ordinary directory leaf under
  ADR 0019. Its parent must already exist and its destination must be absent.
  It has no recursive mkdir, ensure-directory, placeholder/.gitkeep, implicit
  file creation, generic filesystem, shell/process, Git, retry, replay, or
  rollback authority. A clean Git status is expected because Git does not
  track empty directories; filesystem postconditions prove creation.
- Repository observers are best-effort point-in-time observations, not a
  snapshot transaction or cross-process lock. They provide no intentional
  mutation authority, file creation/deletion/rename, generic patches/hunks,
  commit/history, or network Git authority. Live observer validation is Windows
  only at exactly `codex-cli 0.149.0`; Unix live Codex validation is unverified.
- Process supervision is not OS sandboxing; RAH makes no network-isolation or
  rollback guarantee. Timeout/cancellation may leave uncertain effects, which
  are never automatically replayed.
- OS sandboxing, network isolation, and rollback guarantees are not provided.
- Desktop has no llama.cpp process management, provider/model installation,
  generic network Tool, network MCP/Streamable HTTP, generic shell/process
  authority, model-selected executable/cwd/endpoint, or automatic authority
  restoration. It grants no Git commit/ref/history authority or generic
  repository delete/rename authority. Moving or renaming a repository
  intentionally changes its conversation-persistence namespace.
- SQLite is private Desktop storage, not a model/tool SQL capability. Resume
  replays only bounded completed text after a fresh connection; it has no
  rollback guarantee for uncertain external effects.
- Interactive approvals, TUI/web UI, multi-agent orchestration, RAG, and
  long-term memory remain out of scope.

See [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), and the
accepted [ADRs](docs/adr/).
