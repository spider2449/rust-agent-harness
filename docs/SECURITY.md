# RAH v0.3 Security Model

## v0.3 release boundary

The public/host Execute capabilities are `host.cargo.version`,
`host.git.status`, `host.git.stage`, and `host.git.unstage`. They are
host-constructed, capability-specific tools, not generic model-selected process
authority. The hardened Execute deterministic/live fixture (`process.test.echo`)
and the repository-mutation deterministic/live fixture are validation
infrastructure, not public capabilities. `host.fixture.echo` does not exist.

The v0.3 boundary excludes arbitrary `shell.exec` and `process.exec`,
model-selected executable/argv/cwd/environment, worktree restore, arbitrary
file mutation, commit/history/ref operations, reset/clean/checkout/switch/stash,
merge/rebase, push/pull/fetch, network Git, and credential-bearing Git
execution. Worktree-destructive authority is deferred and requires a future
dedicated ADR.

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

Trusted profiles may additionally declare closed `mcp_providers` entries. Each
entry refers to an existing symbolic executable resource and contains a unique
provider ID plus an exact set of remote names, object schemas, and explicit RAH
permissions. Raw paths, argv, cwd, environments, inherited environment, and
resource-limit overrides are not profile fields. Static validation does not
launch a child; the separate explicit `profile validate-effective` operation
does. It composes into a fresh registry and returns no registry or inventory if
any provider fails. The effective profile owns the live MCP adapters, so proxy
tools cannot outlive their provider connection. Inventory shows only symbolic
provider/tool identities and never executable paths, cwd, environment, stderr,
or child diagnostics.

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

- Interactive Codex approvals are unsupported.
- The Codex adapter and external protocols are exactly pinned compatibility
  boundaries.
- Cancellation across any external process boundary cannot undo side effects.
- The broadcast event buffer is bounded; a lagging consumer receives a terminal
  failure instead of silently losing security-relevant events.
- Deterministic tests validate translation, policy, and lifecycle behavior; they
  do not claim live model, credential, third-party server, or platform-sandbox
  validation.
