# RAH v0.16.0 Release-Preparation Security Model

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

v0.16 adds no authority. It adds no generic shell, filesystem, Git, branch/ref,
network, or provider lifecycle authority, and makes no OS-sandbox, network
isolation, or rollback guarantee. MCP and Process Plugin remain existing
host-controlled Tool providers; their Effective Authority presentation is not
currently reachable through the Desktop composition path.

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
