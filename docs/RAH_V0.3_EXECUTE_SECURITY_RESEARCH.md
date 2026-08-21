# RAH v0.3 Execute Permission / `shell.exec` Security Research

Status: Research complete; implementation not authorized by this document

Date: 2026-08-21

Repository baseline: `v0.2.0` (`20290d2`)

## Executive answer

RAH may translate a model-requested `PermissionLevel::Execute` `ToolCall` into
operating-system process execution only when **all** of the following conditions
hold:

1. The host explicitly enabled `PermissionLevel::Execute` for the runtime or
   bridge session. The model request is never authorization.
2. The requested registered RAH tool resolves to a host-preconfigured execution
   capability. The model cannot select or replace the executable.
3. The capability resolves to an absolute, canonical, native executable selected
   by the host. No implicit or model-controlled `PATH` search occurs.
4. The executable resides in a host-trusted location that the capability cannot
   modify. Canonicalization alone is not executable integrity.
5. The capability applies a closed argument policy. For the first prototype this
   means an exact argv vector, not generic argv filtering.
6. The cwd is fixed by the host or resolved beneath a capability-specific
   canonical cwd root. Model-selected absolute cwd and traversal are rejected.
7. The child environment is cleared and rebuilt from fixed host values and a
   small exact allowlist. The model supplies no environment values.
8. The process is spawned directly from `program + argv`; no command interpreter
   is invoked. Windows `.cmd`, `.bat`, and `.ps1` files are rejected.
9. Stdin is null. The call is non-interactive.
10. Stdout, stderr, combined retained output, serialized `ToolOutput`, and runtime
    are hard-bounded before spawn.
11. A platform process supervisor owns the direct child, attempts descendant
    termination on timeout, cancellation, output overflow, and drop, and reaps
    the direct child. Guarantees are reported accurately per platform.
12. `ToolRequested -> ToolStarted` occurs before spawn. Exactly one terminal RAH
    outcome follows. Timeout and cancellation never claim rollback.
13. An uncertain execution is never automatically retried or replayed. Existing
    Generic Codex Tool Bridge duplicate-call protections remain unchanged.
14. The host accepts that, without a separately enforced OS sandbox, the allowed
    executable has the RAH process's effective filesystem, subprocess, network,
    and credential authority after environment minimization.

The current `ShellExecTool` does not meet conditions 2 through 7, 10, or 11. It
must not be enabled for live model use merely by adding `Execute` to the allowed
permission list.

## 1. Authoritative repository baseline

This research uses the v0.2.0 checkout, the v0.1 implementation plan, current
security documentation, and accepted ADRs as authority:

- Task 013 required direct execution, stdout/stderr/exit capture, timeout,
  cancellation readiness, cwd validation, and `Execute`; it explicitly rejected
  a shell-string primary API
  ([`RAH_IMPLEMENTATION_V0.1.md:762`](../RAH_IMPLEMENTATION_V0.1.md#L762)).
- Stable boundary candidates include `AgentRuntime`, `Tool`, `AgentEvent`, tool
  DTOs, and `Sandbox`; security-model changes require an ADR
  ([`ARCHITECTURE_GUARDRAILS.md:47`](ARCHITECTURE_GUARDRAILS.md#L47),
  [`ARCHITECTURE_GUARDRAILS.md:163`](ARCHITECTURE_GUARDRAILS.md#L163)).
- ADR 0003 makes `Tool`/`ToolRegistry` the common execution boundary
  ([`0003-tools-are-extension-boundary.md:9`](adr/0003-tools-are-extension-boundary.md#L9)).
- ADR 0005 keeps Codex-owned command execution disabled unless it passes through
  RAH registry, permission, and sandbox policy; interactive Codex approvals are
  unsupported
  ([`0005-codex-app-server-runtime.md:19`](adr/0005-codex-app-server-runtime.md#L19)).
- ADR 0006 makes Codex dynamic tools untrusted requests, keeps permission and
  execution authority in RAH, and requires Codex-owned shell/approval surfaces
  to remain disabled
  ([`0006-codex-dynamic-tool-bridge.md:15`](adr/0006-codex-dynamic-tool-bridge.md#L15)).
- ADRs 0007 and 0008 establish best-effort cancellation, no uncertain replay,
  bounded process data, minimized environment, and truthful non-sandbox claims
  for external process boundaries
  ([`0007-rah-mcp-tool-adapter.md:40`](adr/0007-rah-mcp-tool-adapter.md#L40),
  [`0008-process-plugin-adapter.md:19`](adr/0008-process-plugin-adapter.md#L19)).
- The current security document already says process supervision and path checks
  are not strong OS isolation
  ([`SECURITY.md:29`](SECURITY.md#L29)).

No accepted ADR authorizes arbitrary model-selected executable authority.

## 2. Current implementation review

### 2.1 `PermissionLevel::Execute`

`PermissionLevel` is a flat, provider-neutral enum. `Execute` means only
"subprocess execution is required"; it carries no executable, argv, cwd,
environment, network, filesystem, timeout, or approval policy
([`tools.rs:80`](../crates/rah-protocol/src/tools.rs#L80)).

Both runtimes treat permission values as exact membership in a host-owned list:

- `MinimalTestRuntime` defaults to `None`, adds values through `with_permission`,
  and checks the registered tool's current definition before dispatch
  ([`minimal.rs:20`](../crates/rah-runtime/src/minimal.rs#L20),
  [`minimal.rs:198`](../crates/rah-runtime/src/minimal.rs#L198)).
- Generic Codex Tool Bridge receives `allowed_permissions` from its host
  constructor, re-reads the registered definition immediately before execution,
  and requires its permission to be in that list
  ([`runtime.rs:81`](../crates/rah-runtime-codex/src/runtime.rs#L81),
  [`bridge.rs:330`](../crates/rah-runtime-codex/src/bridge.rs#L330)).

Therefore `Execute` is a broad first gate, not a complete process authorization.
It must be combined with a host execution policy.

### 2.2 `ShellExecTool`

The existing built-in is named `shell.exec`, but it does not invoke a shell. Its
definition advertises direct execution and requires `Execute`
([`shell_exec.rs:36`](../crates/rah-tools/src/shell_exec.rs#L36)). Its JSON input
lets the model provide:

```json
{
  "program": "...",
  "args": ["..."],
  "cwd": "...",
  "timeout_ms": 10000
}
```

The parser rejects unknown fields, an empty program, non-string argv elements,
and timeout values outside the tool's configured maximum
([`shell_exec.rs:61`](../crates/rah-tools/src/shell_exec.rs#L61),
[`shell_exec.rs:117`](../crates/rah-tools/src/shell_exec.rs#L117)). It then passes
the model-selected program, argv, and cwd unchanged to `Sandbox::execute`.

On completion it lossily decodes all captured bytes and returns one JSON
`ToolContent` containing stdout, stderr, optional exit code, and `timed_out`.
`is_error` is always false, including timeout and non-zero exit
([`shell_exec.rs:88`](../crates/rah-tools/src/shell_exec.rs#L88)).

Current risks are:

- arbitrary executable selection and implicit executable lookup;
- arbitrary argv, including interpreter code flags such as `python -c`;
- host environment inheritance;
- unbounded stdout/stderr allocation;
- direct-child-only termination behavior;
- no explicit spawn/termination/overflow metadata;
- no distinction between a normal non-zero exit and timeout at RAH event level;
- the misleading word `shell`, despite the safer direct-spawn behavior.

The current unit tests prove only direct `rustc --version` execution, result
capture, permission metadata, and rejection of a string in place of argv
([`shell_exec.rs:218`](../crates/rah-tools/src/shell_exec.rs#L218)).

### 2.3 `ToolRegistry` and `ToolContext`

`ToolRegistry` stores `ToolName -> Arc<dyn Tool>`, rejects duplicate names,
returns sorted definitions, fails unknown lookup, and dispatches the untrusted
input to `Tool::execute`
([`rah-tools/src/lib.rs:54`](../crates/rah-tools/src/lib.rs#L54),
[`rah-tools/src/lib.rs:68`](../crates/rah-tools/src/lib.rs#L68)). It intentionally
does not implement permission checks; the runtime/bridge performs them.

`ToolContext` is an empty struct. The `Tool::execute` contract receives no
cancellation token, deadline, audit sink, session identity, or host capability
resolver ([`rah-tools/src/lib.rs:21`](../crates/rah-tools/src/lib.rs#L21)).

This is important: a tool cannot observe a typed cancellation request through
the public contract. Current runtimes cancel tool work by dropping or aborting
the execution future. That can trigger internal RAII cleanup but cannot carry a
reason or wait for a reported process-tree termination outcome.

### 2.4 `Sandbox`, `ProcessSandbox`, and `WorkspacePolicy`

`SandboxPolicy` describes `ReadOnly`, `WorkspaceWrite`, and `FullAccess`.
`CommandSpec` contains only program, argv, optional cwd, and optional timeout;
`ExecutionResult` contains unbounded byte vectors, optional exit code, and a
timeout flag ([`rah-sandbox/src/lib.rs:14`](../crates/rah-sandbox/src/lib.rs#L14)).
There is no environment, stdin, output-limit, process-tree, or termination-result
field.

`ProcessSandbox` explicitly states that it is not OS isolation and accepts only
`FullAccess`. Requests for `ReadOnly` or `WorkspaceWrite` fail because it cannot
enforce them
([`process.rs:11`](../crates/rah-sandbox/src/process.rs#L11),
[`process.rs:51`](../crates/rah-sandbox/src/process.rs#L51)).

The cwd defaults to the canonical workspace root. A requested cwd is resolved as
an existing path through `WorkspacePolicy` and must be a directory. The policy
canonicalizes its root and candidate, resolves symlinks, and requires the result
to start with the canonical root
([`process.rs:27`](../crates/rah-sandbox/src/process.rs#L27),
[`workspace.rs:43`](../crates/rah-sandbox/src/workspace.rs#L43)). Tests cover
parent traversal, absolute outside paths, and symlink escape. On Windows,
`std::fs::canonicalize` returns extended-length/verbatim paths, so policy and
spawn code must consistently accept canonical `\\?\...` forms rather than
mixing lexical and canonical representations ([Rust `canonicalize`](https://doc.rust-lang.org/std/fs/fn.canonicalize.html)).

This cwd validation controls only which directory RAH supplies to the child. It
does not prevent the child from accessing any other path allowed by the RAH
process token.

### 2.5 Actual process spawn, environment, stdin, output, and timeout

`ProcessSandbox` constructs `tokio::process::Command::new(model_program)`, adds
literal argv entries, sets the validated cwd, sets stdin to null, pipes stdout
and stderr, enables `kill_on_drop`, and calls `output()`
([`process.rs:64`](../crates/rah-sandbox/src/process.rs#L64)). Consequences:

- No `cmd.exe`, PowerShell, `sh`, or Bash is explicitly invoked.
- `Command::new` searches for a non-absolute program. Rust documents that the
  child inherits the parent environment by default and that relative program
  names use platform-specific lookup. On Windows, Rust searches several system
  locations and the parent `PATH`; even `env_clear` alone does not suppress all
  of that lookup ([Rust `Command`](https://doc.rust-lang.org/std/process/struct.Command.html)).
- The implementation never calls `env_clear`; all RAH environment variables,
  credentials, proxies, language configuration, profiles, and `PATH` are
  inherited.
- Stdin is already safely null/non-interactive.
- `output()` accumulates complete stdout and stderr before returning. No byte or
  line limit exists, so a child can cause unbounded memory growth.
- A timeout wraps `process.output()`. On timeout the future is dropped;
  `kill_on_drop(true)` attempts to kill the direct child, and RAH immediately
  returns empty stdout/stderr, no exit code, and `timed_out: true`. It does not
  expose whether termination succeeded or whether the child was reaped
  ([`process.rs:79`](../crates/rah-sandbox/src/process.rs#L79)). Tokio documents
  kill-on-drop as invoking a kill operation on the child wrapper and describes
  reaping after dropped child handles as best effort
  ([Tokio `Command`](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)).

No code creates a Windows Job Object, a Unix process group/session, a restricted
token, namespace, seccomp filter, Landlock domain, cgroup, or macOS sandbox.

### 2.6 Agent event lifecycle and cancellation

The public lifecycle provides `ToolRequested`, `ToolStarted`, and `ToolFinished`;
terminal operation events are `Completed`, `Failed`, or `Cancelled`
([`events.rs:38`](../crates/rah-protocol/src/events.rs#L38)). There is no event for
process spawn, exit status, timeout, output truncation, or termination outcome.

`MinimalTestRuntime` emits `ToolRequested`, checks the current registered
permission, emits `ToolStarted`, and selects between cancellation and
`ToolRegistry::execute`. If cancellation wins, the tool future is dropped and
the runtime emits `Cancelled`; it does not await a tool cancellation handshake
([`minimal.rs:198`](../crates/rah-runtime/src/minimal.rs#L198),
[`minimal.rs:249`](../crates/rah-runtime/src/minimal.rs#L249)).

The Generic Codex Tool Bridge performs the same observable sequence. It runs
registry execution in a Tokio task; turn cancellation aborts that task, responds
to app-server with a cancellation error, and marks the call cancelled
([`bridge.rs:360`](../crates/rah-runtime-codex/src/bridge.rs#L360),
[`bridge.rs:449`](../crates/rah-runtime-codex/src/bridge.rs#L449)). The separately
implemented `AgentRuntime::cancel` sends `turn/interrupt` and waits for Codex's
terminal `interrupted` notification
([`runtime.rs:255`](../crates/rah-runtime-codex/src/runtime.rs#L255)).

Thus cancellation reaches `ShellExecTool` only as execution-future destruction.
The direct child gets a best-effort kill through `kill_on_drop`; descendants are
not owned. This is cancellation-ready in a narrow RAII sense, not confirmed
process-tree cancellation.

### 2.7 Generic Codex Tool Bridge enforcement and replay

Bridge mode remains explicitly opt-in. Codex is configured with
`approvalPolicy: never`, read-only Codex sandbox settings, disabled shell and
unified execution features, no MCP servers, and no Codex-owned file/web/app
capabilities
([`runtime.rs:203`](../crates/rah-runtime-codex/src/runtime.rs#L203),
[`runtime.rs:305`](../crates/rah-runtime-codex/src/runtime.rs#L305)). These are
defense in depth; RAH policy is authoritative.

The bridge validates thread/turn ownership and the advertised alias snapshot,
then emits `ToolRequested`. Immediately before `ToolStarted` it re-reads the
tool definition and checks current permission against the host list. Only then
does it call `ToolRegistry::execute`
([`bridge.rs:219`](../crates/rah-runtime-codex/src/bridge.rs#L219),
[`bridge.rs:316`](../crates/rah-runtime-codex/src/bridge.rs#L316)).

Bridge call state is keyed by `(thread_id, turn_id, call_id)`. A duplicate with
identical tool and arguments joins the in-flight call or receives the retained
response without re-execution. Reuse with different content fails, aborts the
task, and invalidates the call
([`bridge.rs:257`](../crates/rah-runtime-codex/src/bridge.rs#L257)). These
at-most-once protections must remain exactly as implemented. They do not prove
globally exactly-once side effects after host crash or ambiguous process state.

## 3. Threat and authority model

Treat all of the following as untrusted data:

- model-generated program names, arguments, cwd, and environment values;
- the complete tool input JSON, including type-confused or oversized values;
- executable stdout, stderr, exit status, signals, and timing;
- child processes and every descendant;
- every filesystem path derived from model input;
- repeated, delayed, conflicting, or malformed dynamic-tool requests.

The RAH host configuration is authority. It selects registered tools, allowed
permission levels, execution capabilities, executable identities, argument
policies, cwd roots, environment, resource limits, and whether an OS sandbox is
required. A model `ToolCall` is a request, never authorization. Codex tool
metadata, aliases, model reasoning, and `approvalPolicy` cannot grant authority.

An authorized executable is not automatically trusted with arbitrary argv.
Similarly, a trusted executable can emit malicious or secret-bearing output;
output is data, never a new command or authorization input.

## 4. Direct execution versus a shell

The v0.3 default must remain:

```text
canonical native executable + literal argv vector
 -> direct OS process creation
```

Do not invoke `cmd.exe /c`, `powershell -Command`, `sh -c`, or `bash -c`. Shell
metacharacters such as `&`, `|`, `;`, `$()`, backticks, redirection tokens, and
quotes must remain ordinary argv bytes/strings as interpreted by the selected
native executable. A future shell capability would require a separate explicit
security design because its argument is effectively executable source code.

The current public name `shell.exec` is inaccurate. `process.exec` would better
describe direct spawn, but a generic name still suggests arbitrary process
authority. Do not rename the current contract during this spike. For v0.3,
prefer capability-specific registered tools and leave `shell.exec` unregistered
or disabled. A later ADR may deprecate/rename it with compatibility policy.

## 5. Host-owned executable authorization

Executable authorization must use an immutable host-side capability table, for
example:

```text
capability ID
 -> absolute configured executable path
 -> canonical executable identity
 -> argv policy
 -> cwd policy
 -> environment policy
 -> time/output/process limits
```

Policy evaluation rules:

1. Resolve and canonicalize the host-configured path during capability creation.
2. Require an existing regular native executable appropriate for the platform.
3. Reject relative paths, bare basenames, model-selected paths, and implicit
   `PATH` search.
4. Store the canonical path, expected file identity/metadata, and optionally a
   host-configured digest or publisher policy.
5. Before each spawn, re-open/revalidate identity and fail closed on change.
6. Require the executable and its containing directories to be outside paths
   writable by the model capability. A digest check followed by path spawn still
   has a replacement race; trusted non-writable installation is the minimum
   prototype assumption.
7. Spawn the exact canonical path. Do not infer authorization from the requested
   string, basename, extension, file association, or reported version.

An exact path allowlist is stronger than a basename allowlist. A basename such
as `git` is ambiguous across user, workspace, package-manager, and system
locations. Configured aliases are useful only when the alias is host-owned and
maps to one canonical identity; the model never supplies an executable-like
alias that is later searched through `PATH`.

Platform resolution must occur at host configuration time, not call time. On
Windows, Rust's own documented search order means `env_clear()` is not enough to
disable lookup; an absolute path is required. On Unix, `env_clear()` without an
explicit `PATH` can still cause `execvp` to use a platform default, so an absolute
path is likewise required ([Rust `Command`](https://doc.rust-lang.org/std/process/struct.Command.html)).

## 6. Argument policy

Executable authorization alone is insufficient:

| Same executable | Effective authority |
| --- | --- |
| `git status` | Repository inspection, plus hooks/config/helper risks depending on command and environment |
| `git diff` | Repository inspection and potentially large output |
| `git clean -fd` | Destructive workspace deletion |
| `git push` | Network and remote mutation |
| `python script.py` | Executes code selected through a path |
| `python -c ...` | Executes arbitrary model-provided code |

Do not try to sanitize a generic command string. For v0.3 use closed structured
capabilities, in descending order of preference:

1. Exact argv, such as `cargo --version`.
2. A small typed parameter schema rendered by host code into argv, with every
   token position constrained and no free-form flag injection.
3. Exact subcommand plus enumerated flags/values and bounded validated operands.

Generic argv execution, arbitrary trailing args, prefix-only allowlists, and
"allow all except dangerous flags" are unsuitable for the first live
capability. Forbidden-flag lists are brittle because programs gain flags,
configuration sources, response files, environment controls, plugins, aliases,
and subcommands over time.

Capability definitions should also bound argument count and encoded byte length.
Empty strings and embedded NULs fail before spawn. Platform-specific quoting is
owned by the Rust/OS process API; no pre-quoting or concatenation is performed.

## 7. Working-directory authority

The model must not freely select cwd. Each capability chooses one of:

- a fixed canonical host-created directory;
- a fixed canonical workspace root;
- an optional relative subdirectory resolved through a capability-owned
  `WorkspacePolicy`-equivalent root.

The safest first prototype uses a host-created empty, non-sensitive directory
outside the repository. A workspace-oriented capability may use the repository
only when its purpose requires it and the host accepts the ambient authority.

For a model-supplied relative subdirectory, resolution must:

1. reject absolute, drive-relative, UNC/device, and verbatim-prefix input;
2. join only beneath the configured root;
3. require the directory to exist;
4. canonicalize the final directory, resolving symlinks/junctions;
5. compare canonical path components against the canonical root;
6. fail closed on traversal, reparse-point resolution errors, or identity change;
7. pass the canonical result directly to process creation.

Windows `\\?\` paths must have one consistent internal representation.
`std::fs::canonicalize` returns extended-length paths on Windows, while verbatim
paths disable normal lexical normalization semantics
([Rust `canonicalize`](https://doc.rust-lang.org/std/fs/fn.canonicalize.html),
[Rust path prefixes](https://doc.rust-lang.org/std/path/enum.Prefix.html)). Policy
must not strip `\\?\` and then perform a lexical comparison, or accept a raw
verbatim model path without canonical handle-based resolution.

Execute policy should own a configured cwd root independent of `FsReadTool`.
Filesystem read permission and process cwd serve different capabilities, and
sharing the read-tool root could falsely imply that child filesystem access is
confined there.

## 8. Environment policy

The default is:

```rust
Command::env_clear();
```

followed by an explicit host capability environment. Use policy B—host allowlist
plus fixed values—only where policy A—entirely fixed values—is insufficient.
Tool input must not supply environment names or values in v0.3.

Default exclusions include `PATH`, `HOME`, `USERPROFILE`, `TEMP`, `TMP`, Git
configuration variables, proxy variables, cloud credentials, API tokens, SSH
agent variables, dynamic-loader variables, and language-runtime configuration.
No wildcard/prefix inheritance is allowed. Log variable names only, never values.

Windows may require an exact host-copied `SystemRoot` for some native program
startup/runtime behavior. Add it only to capabilities proven to require it.
Use a host-created private temporary directory and fixed `TEMP`/`TMP` only if the
executable requires temporary files; otherwise omit them. Do not add `PATH` for
resolution because the executable path is already absolute.

The existing process-plugin adapter provides a repository precedent: it
canonicalizes a host program, uses an isolated cwd, calls `env_clear`, adds only
`RAH_PLUGIN_PROTOCOL`, optional Windows `SystemRoot`, and explicit host values
([`rah-tools-plugin/src/lib.rs:235`](../crates/rah-tools-plugin/src/lib.rs#L235)).

## 9. Stdin

V0.3 process capabilities should use `Stdio::null()` and support no interactive
stdin. This matches current `ProcessSandbox` behavior.

Piped arbitrary or binary stdin adds data-size limits, secret handling, partial
write/cancellation races, child protocols, and the risk of a child waiting
indefinitely for framing or EOF. If a later capability genuinely needs stdin,
it should define a bounded typed payload, close the pipe deterministically, and
specify whether secrets may be present. It must not inherit the host terminal.

## 10. Output and `ToolOutput` limits

Recommended initial hard maxima per call are:

| Resource | Maximum |
| --- | ---: |
| Retained stdout | 256 KiB |
| Retained stderr | 256 KiB |
| Combined retained stdout + stderr | 512 KiB |
| One logical line before delimiter, if line-aware decoding is used | 64 KiB |
| Final serialized `ToolOutput` | 768 KiB |

These are implementation maxima, not model-adjustable defaults. A capability may
choose smaller values. Read both pipes concurrently in bounded chunks so neither
can deadlock the child.

On the first individual or combined overflow:

1. retain only the bounded prefix already accepted;
2. mark the relevant stream and combined result as truncated;
3. initiate process-tree termination;
4. drain/reap according to the supervisor's bounded termination deadline;
5. return a structured `ToolOutput` with `is_error: true`, reason
   `output_limit_exceeded`, retained byte counts, dropped/truncated metadata,
   and termination outcome when known.

Terminating on overflow is preferable to merely truncating while allowing a
potentially infinite writer to consume CPU, pipe bandwidth, and time. If
termination supervision itself fails, return sanitized `ToolError::Execution`,
record the outcome in host-only audit data, and mark execution uncertain. Raw
stderr is model-visible only within its bounded retained field; it is never
authorization input and must be control-character safe at presentation layers.

## 11. Timeout semantics

Timeout is a host maximum. A model may not extend it; a capability may expose a
bounded request for a shorter duration only if useful. Recommended prototype
default and maximum are both 5 seconds for the exact version capability; tests
use shorter fixture-specific limits.

Observable flow:

```text
ToolRequested
 -> host permission and capability checks
 -> ToolStarted
 -> direct process spawn attempt
 -> timeout expires
 -> process-tree termination attempt
 -> bounded wait/reap attempt
 -> ToolFinished(is_error = true, timed_out = true, termination metadata)
```

Pre-spawn invalid input is `ToolError::InvalidInput`. Spawn/supervisor failure is
`ToolError::Execution` and therefore a terminal `AgentEvent::Failed` in current
runtimes. A completed timeout is a tool-domain result and should be a structured
error `ToolOutput`, allowing `ToolFinished` to preserve correlation. Failure to
terminate/reap is an execution-management error and must be treated as uncertain.

"Timed out" means RAH stopped waiting for normal completion and attempted
termination. It does not mean the process never ran, descendants stopped, files
were restored, network calls were recalled, or any side effect was rolled back.

## 12. Process-tree termination

### Windows

Use a Job Object owned by the per-call supervisor, configured with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and assign the process before allowing it
to run. The safest creation sequence uses a suspended child, job assignment,
then resume, avoiding a race in which the child spawns before assignment.
`TerminateJobObject` terminates all processes associated with the job and nested
jobs ([Microsoft `TerminateJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)); closing the last kill-on-close job handle also terminates associated processes
([Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)).

`CREATE_NEW_PROCESS_GROUP` is useful for console control signaling but is not a
security containment boundary. It allows group-directed console control behavior
and does not replace Job Objects. `taskkill /T` is a separate external command,
depends on process discovery/permissions, races with tree changes, and must not
be the primary supervisor.

Set handle inheritance to the minimum needed for stdin/stdout/stderr. Do not
inherit arbitrary RAH handles. Configure `CREATE_NO_WINDOW` for non-interactive
capabilities where compatible; it is UI behavior, not isolation.

Job containment has limits. Processes may use breakaway behavior where the job
permits it; existing parent-job/nested-job constraints vary by Windows version
and host. The capability must fail closed if RAH cannot establish its required
job ownership. Even a Job Object manages process membership and lifetime; it
does not itself restrict filesystem or network syscalls.

### Unix

Place the child into a new process group before exec and retain its PGID. On
timeout/cancellation/overflow, send `SIGTERM` to the group, wait a short bounded
grace period, then send `SIGKILL` to the group and reap the direct child.
`killpg` signals a process group
([Linux `killpg(2)`](https://www.man7.org/linux/man-pages/man2/killpg.2.html)).

A new session (`setsid`) can further separate terminal/session state; it creates
a new session and process group
([Linux `setsid(2)`](https://man7.org/linux/man-pages/man2/setsid.2.html)). The
exact process-group/session setup should be platform-specific internal code, not
a public RAH contract.

Process groups are not unescapable containment. A child with sufficient
authority can create a new process group/session, double-fork, delegate work to
another service, or complete side effects before a signal arrives. Cross-platform
full descendant termination is therefore **not currently available in RAH and
cannot be guaranteed against adversarial descendants**. Job Objects/process
groups provide supervised best effort for trusted capability executables. Strong
containment requires an OS sandbox/container and a separate threat model.

## 13. Cancellation

Desired flow:

```text
AgentRuntime::cancel
 -> runtime cancels/aborts in-flight Tool execution
 -> execution future drop guard signals the internal supervisor
 -> process-tree termination attempt
 -> bounded reap/cleanup
 -> one terminal AgentEvent::Cancelled
```

The current public `Tool`/`ToolContext` contract cannot receive an explicit
cancellation token or report a cancellation acknowledgement. This is an
architectural mismatch for confirmed cooperative cancellation and structured
termination audit. Current runtime behavior does, however, drop or abort the
tool future, so a v0.3 prototype can preserve public contracts if its internal
process supervisor is cancellation-safe on future drop and owns all cleanup.

Required prototype semantics:

- RAII/drop must always initiate termination, never detach the process;
- a dedicated internal supervisor may outlive the dropped tool future only to
  complete bounded termination and reaping;
- cancellation remains best effort and is not rollback;
- `Cancelled` must remain the sole terminal agent outcome; no later
  `ToolFinished` or `Completed` may appear;
- host-only diagnostics record whether termination/reap was confirmed.

This does not require a public contract change for the minimal exact-argv
prototype. A future requirement that tools cooperatively observe cancellation,
distinguish timeout from user cancel, or return termination outcome through
`AgentEvent` should trigger a separate public-contract review rather than being
hidden in `ToolContext` behavior.

## 14. Uncertain execution, duplicate calls, and replay

Side-effecting execution is at-most-once per accepted bridge call, not globally
exactly once. Apply these rules:

- Set call state to started before or atomically with spawn ownership.
- Never automatically retry after spawn may have occurred.
- Never automatically replay after timeout, cancellation, output overflow,
  transport interruption, runtime disconnect, host restart, or uncertain kill.
- Preserve the Generic Codex Tool Bridge's `(thread, turn, call)` deduplication
  and retained identical response behavior unchanged.
- Conflicting reuse of a call ID remains a failure and must not spawn.
- A duplicate received while the original is in flight shares the original
  outcome; it does not create another process.
- A host restart loses in-memory correlation. V0.3 must not infer that a
  repeated request is safe; durable execution journals are out of scope.

Race interpretation:

| Race | Required result |
| --- | --- |
| Cancel before spawn ownership | Do not spawn; terminal cancellation. |
| Spawn and cancel cross | Assume execution may have occurred; terminate; no retry. |
| Timeout and normal exit cross | Use one atomic terminal state; record observed status; no retry. |
| Disconnect after `ToolStarted` | Mark uncertain; terminate if locally owned; no replay. |
| Duplicate Codex request | Existing bridge call state decides; never spawn twice. |

## 15. Filesystem and network authority

`PermissionLevel::Execute` is potentially stronger than `Read + Write`. A process
can read or alter files, start other programs, use IPC, access credentials, and
change system/user state using any authority available to the RAH process. For
example, an authorized Python interpreter with `-c` bypasses the semantic limits
of `FsReadTool` and any future filesystem-write tool.

`WorkspacePolicy` validates paths RAH chooses. It does not mediate filesystem
syscalls made by a child. `SandboxPolicy::FullAccess` states the current reality;
do not claim workspace confinement merely because cwd is inside a workspace.

Likewise, permitted executables may access the network. `curl`, `git push`,
Python, Node, PowerShell, and many ostensibly local tools can open sockets or
invoke helpers. RAH v0.2 has no child network policy. Environment clearing
removes proxy/credential convenience but does not disable networking.

For v0.3, network-isolated arbitrary process execution is out of scope. The
initial capability must be a trusted, deterministic, non-networking executable,
and documentation must explicitly state that process policy does not enforce
network or filesystem isolation. Any capability whose intended or plausible
operation needs broad network/filesystem authority requires explicit host
acceptance or a future OS sandbox.

## 16. Process policy versus OS sandboxing

HostExecutionPolicy decides **what RAH is willing to launch**. OS sandboxing
constrains **what launched code can do**. They are independent layers.

Potential future enforcement mechanisms include:

- Windows: Job Objects for lifetime/resource grouping; restricted tokens or
  AppContainer/LPAC for authority; Windows Sandbox/virtualization for a stronger
  machine boundary. Microsoft describes AppContainer as restricting files,
  registry, devices, processes, windows, network, and credentials unless
  capabilities grant access
  ([Microsoft AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer)).
- Linux: namespaces for view/isolation, seccomp for syscall filtering, Landlock
  for unprivileged filesystem/access restrictions inherited by descendants, and
  cgroups for resource/accounting limits. Landlock is explicitly an additional
  restriction of ambient rights, not a replacement for other controls
  ([Linux Landlock](https://www.kernel.org/doc/html/latest/userspace-api/landlock.html)).
- macOS: platform sandbox profiles/entitlements or a stronger container/VM,
  subject to current platform support and distribution constraints.

None is present in current `ProcessSandbox`. Do not require these mechanisms for
the first trusted exact-command prototype, and do not claim properties they
would enforce. Arbitrary or third-party executable support should wait for a
separate OS-sandbox design.

## 17. Execute permission model

Keep the current enum:

```text
PermissionLevel::Execute
            +
HostExecutionPolicy
```

`Execute` answers the broad runtime question: may this session request a
host-preauthorized process capability? `HostExecutionPolicy` answers the
concrete question: which capability, executable identity, argv, cwd,
environment, limits, and supervisor apply?

Adding permission variants such as `ExecuteGit`, `ExecuteNetwork`, or
`ExecuteShell` would mix a coarse provider-neutral permission vocabulary with
an open-ended capability catalog and still would not constrain argv. Avoid that.

The policy can remain internal to `rah-tools` with platform-specific supervision
in `rah-sandbox`, preserving dependency direction. If the first prototype uses
one capability-specific `Tool`, the host policy may be a private immutable
configuration rather than a new public architecture extension point.

## 18. Approval semantics

Host preauthorization and interactive per-call approval are different:

- **Host preauthorization** constructs and registers a closed capability, enables
  `Execute`, and accepts its documented ambient authority before a turn starts.
- **Interactive approval** pauses one call, presents exact security-relevant
  details to a human, receives a correlated decision, and resumes or denies.

Current `AgentEvent::ApprovalRequired` can describe a request, but
`AgentRuntime` has no approval-response method. Codex approval requests are
explicitly denied, and `approvalPolicy: never` must remain unchanged. Therefore
v0.3 should support only host-preauthorized execution capabilities and defer
interactive approval. Do not reinterpret `never` as authorization; it means
Codex will not provide an approval path.

## 19. Safest tool input design

Three shapes were considered:

| Shape | Assessment |
| --- | --- |
| `{program,args,cwd}` | Unsafe for live v0.3: model selects all high-authority dimensions. |
| `{capability,args}` | Better, but free-form args still need a closed per-capability schema. |
| Registered capability-specific tools | Best for v0.3: definition, schema, executable, and argv renderer are host-owned. |

Generic arbitrary process execution should **not** be exposed to the model in
v0.3. Prefer registered tools such as:

```text
process.cargo_version   input: {}
process.git_status      input: { optional bounded display mode }
```

Each tool is an ordinary RAH `Tool` requiring `Execute`; its implementation
selects one internal host capability. This avoids trusting a model capability
string, keeps advertised authority legible, and lets the existing registry and
bridge operate unchanged.

If a single dispatcher is later needed, its input should be
`{"capability":"host-owned-id","parameters":{...}}`, where the capability ID is
an enum from an immutable registry and parameters have a distinct closed schema.
It must never accept a program field.

## 20. Security events and audit

Existing `AgentEvent` is sufficient for the minimal model-visible flow:

- `ToolRequested` identifies the registered capability tool and contains its
  small typed input;
- `ToolStarted` establishes that RAH authorized execution and began work;
- `ToolFinished` can carry bounded structured status/truncation/timeout fields;
- `Failed` and `Cancelled` provide one terminal runtime outcome.

It is not sufficient as a complete structured security audit. It has no fields
for canonical executable identity, resolved cwd, spawn time, termination
attempt/outcome, or uncertainty. Moreover, generic `ToolRequested` includes raw
input and therefore must not contain secrets. For the prototype:

- capability ID and sanitized parameter summary may be model-visible;
- canonical executable, cwd, exit status, timeout, truncation, and termination
  outcome go to bounded host-only tracing/audit records;
- argv is logged only after per-capability redaction; exact fixed argv may be
  recorded, but never secret-bearing values;
- environment variable names may be recorded; values must not;
- stdout/stderr remain bounded tool output and must not be duplicated into
  unrestricted logs.

No `AgentEvent` change is required for the prototype. A durable security audit
contract would be a separate public design.

## 21. Windows-specific findings

- Prefer an absolute canonical `.exe`/native PE path. Rust resolves relative
  executables itself and may search child `PATH`, the current executable
  directory, system directories, the Windows directory, and parent `PATH`.
- Reject `.cmd` and `.bat`. Microsoft documents that batch files require the
  command interpreter and recommends caution because of command-hijacking risk
  ([Microsoft `CreateProcess`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessa)).
- Reject `.ps1`; it is data for a PowerShell interpreter and cannot be directly
  executed as a native process capability. A PowerShell capability would be a
  separately authorized interpreter/code capability.
- Use Rust's argument API, not a constructed command-line string. Windows
  ultimately passes a command line, and target programs may use different
  parsers; capabilities must test the actual target's argv behavior.
- Preserve canonical `\\?\` path semantics consistently. Reject raw model
  device/verbatim paths and normalize through trusted handle-based resolution.
- Use a Job Object with kill-on-close and fail closed if assignment/required
  nesting cannot be established. `CREATE_NEW_PROCESS_GROUP` does not replace it.
- Use `CREATE_NO_WINDOW` for headless tools where compatible; never launch an
  interactive console/window as part of the safe prototype.
- Inherit only the three intended stdio handles. Avoid broad handle inheritance.
- On timeout/cancel, terminate the job, wait/reap the direct child within a
  bounded deadline, and report uncertainty if confirmation fails.

RAH currently implements none of the Job Object, creation-flag, suspended-start,
or explicit handle-list behavior for `ShellExecTool`.

## 22. Unix-specific findings

- Require an absolute canonical regular file with executable permission. Native
  binaries are preferred for the first prototype.
- Kernel shebang execution can select an interpreter from the file's first line;
  therefore scripts expand executable identity to include the interpreter and
  script contents. Reject scripts in the first safe prototype.
- Do not use `PATH`; spawn the canonical absolute file.
- Clear the environment and add exact fixed variables only.
- Create a dedicated process group/session before exec; signal the group with
  `SIGTERM`, then `SIGKILL` after a bounded grace period.
- Always wait/reap the direct child. Tokio explicitly describes post-drop reaping
  as best effort, which is insufficient as the only supervisor guarantee.
- Treat signal termination as structured exit metadata, not a numeric exit code.
- Process groups do not contain descendants that deliberately detach or move to
  another session; do not promise adversarial tree containment.
- Canonical cwd resolution must follow symlinks and fail outside the configured
  root, while acknowledging that cwd does not constrain later filesystem access.

## 23. Minimal safe prototype

The first deterministic prototype should use a crate-local native fixture
executable, not `cargo`, as the conformance target. The fixture can provide
controlled modes for constant version output, argv echo, environment inspection,
stdout/stderr flooding, sleeping, exit status, and descendant creation without
depending on the installed toolchain, user configuration, network, or repository
state.

Model-visible production-shaped capability:

```text
registered tool: process.fixture_version
permission: PermissionLevel::Execute
input: {}
executable: host-resolved canonical test fixture
argv: ["--version"]
cwd: host-created empty directory
environment: empty, plus SystemRoot on Windows only if proven necessary
stdin: null
timeout: 5 seconds
output: 256 KiB per stream, 512 KiB combined
supervisor: Windows Job Object / Unix process group
```

Flow:

```text
Codex dynamic tool request
 -> Generic Tool Bridge routing/deduplication
 -> current Execute permission check
 -> ToolRegistry
 -> capability-specific RAH Tool
 -> internal HostExecutionPolicy
 -> direct supervised native child
 -> bounded structured ToolOutput
 -> Codex continuation
```

After deterministic conformance passes, an opt-in acceptance capability may use
host-configured `process.cargo_version` with canonical cargo executable and exact
`["--version"]`. The model still cannot replace the executable or arguments.
`cargo --version` is less deterministic across machines and should not be the
only security test.

Do not register or expose the current generic `ShellExecTool` in this prototype.

## 24. Deterministic test plan

All default tests use the local fixture, no network, credentials, real model, or
GPU. Platform-specific tests are gated and assert only guarantees implemented on
that platform.

### Authorization and executable identity

- exact host-configured fixture executable is allowed;
- an unconfigured executable and model-supplied program field are denied;
- canonical path, symlink/junction alias, and replacement identity are tested;
- relative/basename executable is rejected and `PATH` lookup is disabled;
- executable in a capability-writable directory is rejected;
- malformed/unknown input fields, NULs, excessive argv count/bytes are rejected;
- Generic Codex bridge with no `Execute` fails before `ToolStarted`;
- `Execute` plus an unknown/unregistered capability still fails closed.

### Direct invocation and arguments

- shell metacharacters arrive as one literal argv element and cause no secondary
  command/file effect;
- exact allowed argv succeeds;
- missing, extra, reordered, prefixed, or forbidden args fail before spawn;
- Windows `.cmd`, `.bat`, and `.ps1` are rejected without invoking `cmd.exe` or
  PowerShell;
- Unix non-executable file and prototype script/shebang input are rejected;
- architecture/behavior test proves no Codex-owned shell or unified execution.

### Cwd and environment

- default child cwd is the host-created isolated directory;
- allowed relative cwd beneath a configured root canonicalizes correctly;
- parent traversal, absolute outside cwd, symlink/junction escape, drive-relative,
  UNC/device, and raw Windows verbatim forms fail closed as applicable;
- a sentinel parent API token, proxy, SSH, Git, profile, and `PATH` variables are
  absent;
- only exact fixed/allowlisted environment names and values appear;
- Windows `SystemRoot` behavior is tested both when omitted and explicitly
  required; no other environment is inherited.

### Output and process outcomes

- stdout and stderr are captured independently;
- stdout individual limit, stderr individual limit, combined limit, and long-line
  limit each terminate deterministically with bounded retention and metadata;
- final serialized `ToolOutput` remains under its hard maximum;
- zero and non-zero exit status map to documented structured output;
- signal termination on Unix and no-code termination on Windows are represented;
- child waiting on stdin observes EOF and cannot hang interactively.

### Timeout, cancellation, descendants, and replay

- timeout initiates tree termination, bounded reap, error `ToolOutput`, and no
  rollback claim;
- `AgentRuntime::cancel` during execution initiates termination and ends with one
  `Cancelled`, never later `ToolFinished`/`Completed`;
- direct child is gone after timeout, cancellation, overflow, and future drop;
- descendant fixture is terminated by Windows Job Object / Unix process group;
- deliberately detached Unix child or allowed Windows breakaway case documents
  the limit or makes capability startup fail closed; no false full-tree claim;
- spawn/cancel, timeout/exit, and overflow/exit races produce one terminal state;
- an uncertain post-spawn supervisor/transport failure is never retried;
- identical duplicate Codex `callId` executes once and shares/returns one result;
- conflicting duplicate `callId` fails and does not execute again;
- host restart/repeated call has no automatic replay path.

### Authority and architecture assertions

- tests explicitly state that cwd/path validation does not prove child
  filesystem confinement;
- tests explicitly state that cleared proxy variables do not prove network
  isolation;
- no test or documentation calls `ProcessSandbox` an OS sandbox;
- no Codex-owned shell/file/MCP execution or approval is enabled;
- current public RAH contracts remain source-compatible.

## 25. Public contract review

| Contract | Safe prototype change required? | Finding |
| --- | --- | --- |
| `AgentRuntime` | No | Current cancellation drops/aborts tool work and emits terminal cancellation. |
| `Tool` | No | A capability-specific implementation can own internal policy/supervision. Explicit cooperative cancellation remains a limitation. |
| `ToolRegistry` | No | Existing neutral registration and dispatch are correct. |
| `ToolContext` | No | May remain empty for the prototype; cannot carry typed cancellation/audit. |
| `ToolDefinition` | No | Existing name, schema, and `Execute` metadata are sufficient. |
| `ToolCall` | No | Capability-specific tool input prevents executable selection. |
| `ToolOutput` | No | JSON content can carry bounded process result metadata. |
| `ToolError` | No | Invalid input and execution/supervision errors fit current variants, though uncertainty is not typed. |
| `PermissionLevel` | No | Keep `Execute` coarse and add internal host policy. |
| `AgentEvent` | No | Sufficient for prototype lifecycle; insufficient for complete structured security audit. |
| `Sandbox` | No | Internal implementation can supervise capabilities; current `CommandSpec` is too weak for a generic policy API but need not change for one private prototype. |

The cancellation mismatch is explicit: `ToolContext` has no cancellation token,
and current cancellation relies on future drop. This prevents cooperative tool
cancellation and synchronous reporting of termination outcome, but it does not
force a public architecture change for a tightly scoped prototype with an owned
internal supervisor. If v0.3 requires confirmed cancellation before emitting
`Cancelled`, capability-generic public configuration, or structured uncertainty
events, a separate public-contract decision is required.

## 26. ADR plan

Implementation should propose:

```text
docs/adr/0009-execute-process-policy.md
```

before enabling any live `Execute` capability. An ADR is warranted because this
selects a new security model: coarse `Execute` plus host capability policy,
capability-specific tools instead of generic model-selected programs, direct
native spawn, environment/cwd/output defaults, process-tree supervision,
at-most-once/no-replay semantics, and explicit absence of filesystem/network/OS
sandbox guarantees.

The ADR should record at minimum:

- current generic `ShellExecTool` remains disabled for model use;
- host capability identity and immutable executable/argv policy;
- no implicit `PATH`, shell, scripts, model environment, or stdin;
- exact output/timeout limits and overflow behavior;
- Windows Job Object and Unix process-group best-effort guarantees;
- cancellation by execution-future drop as the current contract limitation;
- no uncertain retry/replay;
- no filesystem/network isolation claim;
- public contracts remain unchanged for the prototype;
- OS sandboxing and interactive approval are deferred.

This research-only task does not create ADR 0009, following the repository's
research -> accepted ADR -> implementation convention.

## 27. Final recommendation

Minimum `HostExecutionPolicy` before any live Execute tool is enabled:

- immutable host capability ID;
- canonical absolute native executable in a host-trusted non-capability-writable
  location, revalidated before spawn;
- exact or closed typed argv policy with count/byte bounds;
- fixed canonical cwd or fail-closed capability-specific cwd root;
- `env_clear` plus fixed exact host values, with no model environment;
- null stdin;
- hard stdout/stderr/combined/serialized-output limits and terminate-on-overflow;
- host-fixed timeout;
- Windows Job Object or Unix process-group supervisor, direct-child reap, bounded
  termination escalation, and truthful descendant limitations;
- at-most-once call handling and no retry after uncertain execution;
- bounded sanitized result/audit data with no secret environment exposure;
- explicit host acceptance that filesystem and network isolation are not
  enforced without a separate OS sandbox.

B. PROCEED WITH HARDENED HOST EXECUTION POLICY
