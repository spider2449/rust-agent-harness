# RAH Security Model — Plugin Kernel

Governing decision: [ADR 0036](adr/0036-plugin-kernel-everything-is-a-plugin.md),
section 4. The authority-first security model of v0.35.0 and earlier is
documented in [docs/legacy/SECURITY.md](legacy/SECURITY.md).

## Trust boundary

**Mounting is trust; model output is never trusted.**

| Source | Trust |
|---|---|
| Plugins mounted by the user's configuration (static or process) | Trusted code, comparable to a dependency |
| Process plugins calling host services | Limited to the services in their own `requires`, enforced by the host |
| WASM plugins | Capability-isolated: no WASI imports; host services only from their own `requires`; fuel and memory bounded per call ([WASM plugins](WASM_PLUGINS.md)) |
| Model output, tool-call arguments, provider and tool metadata | Untrusted requests |
| Frontend presentation | Grants nothing by itself; Ask confirmations come from the user |

## Invariants

1. Every model-originated tool call passes through `ToolInvoker` and the
   `tool.before_invoke` hook chain. No loop or model plugin calls tool providers
   directly.
2. Under the `default` preset, `policy-authority` decides Allow, Deny or Ask.
   A missing permission assignment fails closed. Provider or tool metadata
   cannot grant or escalate permission.
3. The `minimal` preset runs without a policy plugin and must print a prominent
   *unguarded* warning at startup.
4. Uncertain external effects are never replayed automatically. Timeout,
   cancellation, disconnect, crash or a lost response does not imply rollback.
   Session replay never re-executes tool calls.
5. Shell and process execution exist only as opt-in plugins that require a
   `sandbox` service. They use a fixed program and argument vector, explicit
   cwd and environment policy, bounded output, timeout and cancellation. Under
   `policy-authority` they default to Ask.
6. Process supervision is not OS sandboxing. RAH makes no network-isolation,
   rollback or strong-sandbox claim until one is implemented and tested.
7. Credentials are backend configuration (environment or a storage plugin).
   They never come from model output and never travel through frontend
   channels.

## `policy-authority`

The policy decides each call from its own configuration only:

- A tool needs a host assignment (`tools`, by exact name or a `prefix*`
  pattern; exact names win, then the longest prefix). No assignment means
  Deny.
- The effective level is the stricter of the assigned level and the level the
  tool declares, so tool metadata can tighten but never grant.
- The effective level is allowed when listed in `allow`, asked when listed in
  `ask`, and denied otherwise. A level listed in both is a configuration error.
- A host decision (`decisions`, `allow`, `ask` or `deny` by exact name or
  `prefix*` pattern, matched the same way) replaces these rules for its
  tools. It is the user's configuration (the Web UI's Settings write it); it
  can allow a tool that declares `write` without asking, turn off one tool of
  a pattern, or ask before a read. Tool metadata still cannot grant anything.
- An invalid configuration fails the plugin, and `rah` then refuses to boot
  instead of running unguarded.
- `Ask` needs a `tool.approval` handler; without one it is a denial.
  `frontend-cli` shows the call and its arguments on the terminal and
  approves only an explicit `y` or `yes`. End of input or any other answer
  denies.

The policy registers at hook priority 0. A mounted plugin with a higher
priority runs first and can pre-empt it; mounting such a plugin is a trust
decision. The legacy in-flight limit is not carried over yet.

## `sandbox-process` and `tools-shell`

`sandbox-process` provides the `sandbox` service. It is **process supervision,
not an OS sandbox**: a program runs with the authority of the RAH process, and
there is no filesystem or network isolation. It guarantees:

- a fixed program and argument vector, never a shell string;
- a working directory inside the configured root (relative, no `..`,
  canonicalized);
- a cleared environment refilled only from `inherit_env` (a short platform
  list by default: the path, system and temporary directories, and the home
  directory so programs such as Git find the user's own configuration;
  credentials and other variables are not passed) and the caller's spec;
- null stdin, per-stream output limits and a timeout capped by the plugin;
- termination of the process tree (a kill-on-close Job Object on Windows, a
  process group on Unix) on timeout, output overflow, cancellation, unmount
  of the plugin or a dropped call. Descendants still holding the output pipes
  2 s after the program exits are killed too. On Windows the child joins its
  Job Object right after spawn, so a process it starts before that can
  escape; on Unix a descendant that leaves the process group can escape.

`spawn` starts a long-running program (for example an MCP server) under the
same environment and working-directory rules, with piped stdin and stdout,
a bounded stderr tail and no timeout. Its supervision tasks belong to the
`sandbox-process` scope, so unmounting the sandbox, or dropping the returned
handle, terminates the process tree.

`tools-shell` provides `shell.exec` with `execute` permission and requires
`sandbox`. It accepts `program`, `args`, `cwd` and `timeout_ms` only, and can
be restricted further with `allow_programs`. A program terminated by the
sandbox, or a supervision failure, is reported as an error, which the
`ToolInvoker` records as an `uncertain` effect; it is never retried.

## `tools-mcp`

`tools-mcp` connects to one MCP server over stdio, started through the
`sandbox` service. The server is external code chosen by the user's
configuration; its messages and metadata are untrusted:

- Only tools listed in the plugin's `tools` map are exposed, as
  `mcp.<server>.<tool>`, and each declares the level written there. Server
  annotations (read-only or destructive hints) are ignored. The policy must
  still assign the exposed names; otherwise they are denied.
- A listed tool the server does not offer, an unsupported protocol version or
  a failed handshake fails the plugin, and the server is terminated.
- Messages are size-bounded, and outstanding requests are limited. A request
  that times out, is cancelled or loses its connection is never replayed: the
  server gets `notifications/cancelled`, and a `write` or `execute` tool
  records an `uncertain` effect. Requests the server never received record no
  effect.
- Server-to-client requests other than `ping` are refused, so a server cannot
  ask RAH for sampling, roots or elicitation.
- `tools-mcp-http` (ADR 0038) connects to a server RAH does not start, so
  process supervision does not apply. Its `url` comes from the
  configuration only: `https`, or `http` for a loopback host. Redirects and
  proxies are not used. An optional bearer token is read from an environment
  variable at start, sent only to that URL, and kept out of errors and logs.
  A remote server receives model-chosen tool arguments. A lost stream or
  timeout leaves a `write` or `execute` effect `uncertain`, and a request is
  never resent. A `404` for an ended session means the server did not run
  the request.
- `tools-web-search` sends model-chosen query text to DuckDuckGo, Bing or
  configured SearXNG instances (`web.multi_search` sends it to all of them
  at once), and `web.fetch` requests a model-chosen URL, so a model can leak
  context through a query or a URL. The plugin is
  opt-in, both tools declare `read`, and the policy must assign them;
  `decisions = { "web.*" = "ask" }` shows each request first. Engine URLs
  come from the configuration only, request URLs are kept out of search
  errors, bodies are bounded to 2 MiB, and no proxy is used unless `proxy`
  is set (`web.fetch` never uses one). Results are wrapped in
  `<untrusted-web-content>` with angle brackets in page text replaced, but
  they remain untrusted model input.
- `web.fetch` guards against server-side request forgery: only `http` and
  `https` without credentials; IP literals (including IPv4-mapped, NAT64 and
  numeric forms) and every resolved address must be public, so loopback,
  private, link-local (cloud metadata), shared (CGNAT), documentation,
  benchmarking, multicast and reserved ranges are refused. A name whose
  answer mixes public and non-public addresses is refused as a whole, the
  connection uses the checked address, and each of at most five redirects
  is checked again. The guard does not stop a public server from returning
  harmful text, and `fetch_private_networks = true` turns it off for the
  whole instance.
- Image content is accepted only as PNG, JPEG, GIF or WebP base64 of bounded
  size (ADR 0037); anything else becomes a short note. The Web UI shows only
  those types, as `data:` images, never SVG or HTML. An image is untrusted
  model input like any other tool output: text inside it can try to
  instruct the model.

## `context-compact`

`context-compact` sends old conversation history, including tool output, to
the mounted `model` service and puts the reply into later requests as one
system item. The summary is model output derived from untrusted content, so
it can carry instructions planted in a tool result into a more prominent
position. It is labeled as background, not instructions, bounded to
`max_summary_chars`, and grants nothing: tool calls it prompts still go
through `ToolInvoker` and the policy. The summary request has no tools and
no `model.before_request` hooks. Cache files in `dir` hold summaries of
conversations in plain text; they are kept until deleted.

## `frontend-web`

The Web frontend serves the agent to the local machine only:

- It listens on a loopback address; any other bind address fails the plugin.
- Every start creates a random token (two UUIDv4, 244 random bits). Every
  `/api` request and the WebSocket must carry it (`Authorization: Bearer`, or
  `?token=` for the WebSocket); comparison is constant-time. `rah` prints the
  URL with the token in the fragment, which browsers do not send to servers;
  the UI keeps it in `sessionStorage` for the tab and removes it from the
  address bar.
- Every request must name this server in `Host` (against DNS rebinding), and a
  present `Origin` must be this server (against cross-site requests and
  cross-site WebSocket hijacking).
- The UI is embedded static HTML, CSS and JavaScript served with a strict
  Content-Security-Policy (`default-src 'self'`, no framing). Model and tool
  output is inserted as text only.
- `Ask` decisions go to the connected browsers. With no browser connected the
  frontend does not answer (a denial); an unanswered request is denied after
  `approval_timeout_secs` (300).

The plugin management endpoints (`/api/catalog`, `/api/plugins` changes)
use the distribution's `admin` service, which can mount or unmount any
catalog plugin, including `policy-authority`. They are behind the same token
and checks, and no tool can reach them, so model output cannot change the
composition. The UI warns before unmounting the policy, and the header shows
`UNGUARDED` whenever no `tool.before_invoke` handler is mounted. The admin
service refuses to unmount or reconfigure the `distribution` plugin itself,
and refuses to stop a policy plugin (`policy-authority`): a stop is meant to
be temporary, and tool calls would run unguarded meanwhile. Unmounting the
policy remains possible, behind the warning.

Behind the same token and checks, and likewise unreachable from tools:

- `PUT /api/workspace` points the workspace plugins (`sandbox-process`,
  `tools-builtin`, `tools-repo-write`) at another existing directory, as
  `rah --root` does at start. It widens what tools can reach to that folder,
  which is the user's choice; a policy still decides every call.
- `GET /api/fs/dirs` lists the subdirectory names of any host directory the
  user's account can read, for the folder picker. It returns no file names
  or contents.
- `GET /api/files` and `GET /api/files/content` show the user the current
  workspace (the `sandbox` service's root), read-only: one directory's
  entries (without `.git`) or the first 512 KiB of one file. Paths are
  relative names only; absolute paths, drives, `:` streams and `..` are
  refused, and the canonical result must stay inside the canonical root, so
  a link leading outside is refused too. The UI shows file contents as text
  only, like model output: syntax highlighting splits them into text nodes
  inside spans with fixed class names, never HTML. A tool call's `path` argument can open the view,
  but only through these same checks.
- `GET /api/status` runs one read-only `git status` in the workspace
  through the `sandbox` service.
- `GET /api/git/changes` and `/api/git/diff` each run one read-only `git`
  command the same way: a fixed argument vector with `--no-optional-locks`
  (no index writes), `--literal-pathspecs` (no pathspec magic), and for
  diffs `--no-ext-diff` and `--no-textconv` (no configured diff programs).
  A path must pass the file browser's checks and is passed after `--`, so
  it cannot become an option. Output is bounded (512 KiB per diff). There
  is no write action: staging, committing and pushing stay outside the UI.

Any local process that can read the printed URL can use the API with the
user's authority; the token protects against other users' browsers and
websites, not against code already running as the user.

## Process plugins

A plugin mounted with `use = "process"` ([protocol v3](PROCESS_PLUGINS.md)) is
trusted code like any mounted plugin. The host:

- starts it with a fixed program and argument vector, a cleared environment
  refilled from `inherit_env` and `env`, and piped stdio;
- bounds every message, applies startup and call timeouts, and never replays a
  call (`write` and `execute` tools record `uncertain` effects on timeout,
  cancellation or a lost connection);
- refuses any `service/call` for a service outside the plugin's own
  `requires` (error `-32003`), even when that service is mounted;
- fails the start when the manifest differs from the one read at boot;
- kills the child when the plugin's scope is disposed. Descendants of the
  child are not tracked: this is supervision, not isolation.

The tools a process plugin provides still pass through `ToolInvoker` and the
policy like any other tool.

A process plugin may also attach to `tool.before_invoke`, `tool.after_invoke`
and `model.before_request` (ADR 0039). It is trusted like a Rust plugin on
the same hook: at a priority above `policy-authority` its `allow` bypasses
the policy. A hook that times out, disconnects or answers malformed fails
closed: the call is denied, the output becomes an error, or the model
request fails. Hook calls are never retried.

## `scheduler-basic`

Background jobs and subagents run ordinary turns: every tool call they make
passes through `ToolInvoker` and the policy, and an `Ask` reaches the
frontend's approver (without one it is denied). `agent.delegate` declares
`read`; the subagent's tool effects are recorded in the subagent's session,
not in the parent's. Delegation depth is limited (`max_depth`, 2), job
concurrency is limited (`max_concurrent`, 2), repeating jobs run at most once
per second, and unmounting the plugin cancels every job.

## `tools-repo-write`

`repo.create-file`, `repo.edit-files` and `repo.patch` declare `write`, so
under `policy-authority` they are at least `write` whatever their assignment,
and the presets ask for every call. Paths must be safe repository-relative
paths and resolve inside the canonical `root` without following a link out of
it; `repo.create-file` never overwrites. `repo.edit-files` validates every
edit (exactly one match each) before writing anything and replaces files
through a temporary file and a rename. `repo.patch` runs `git apply --check`
then `git apply` through the `sandbox` service, only when Git's top level is
the configured root; `git apply` rejects paths outside the work tree. A
failure after the first write is recorded as an `uncertain` effect.

## `tools-repo`

`tools-repo` provides `repo.status`, `repo.diff`, `repo.list` and
`repo.search`, declared `read`. They run Git through the `sandbox` service:

- with a hardened environment: no system or global configuration,
  `core.fsmonitor` and `core.untrackedCache` off, no optional locks, no
  terminal prompts, and literal pathspecs (`GIT_LITERAL_PATHSPECS=1`), so a
  model-supplied path is never a glob or magic pathspec;
- only when the configured directory is the top level of a work tree, so Git
  never reads a repository above it;
- with model-supplied paths limited to safe repository-relative paths (no
  `.`, `..`, `.git`, backslashes, colons or a leading `-`) and text queries
  passed after `-e` and searched literally;
- with bounded output and result counts, and `--no-ext-diff --no-textconv`
  for diffs.

Git still reads the repository's own `.git/config`, and a `.git` file or
linked worktree can point elsewhere. Configured filter drivers can run
programs during `status` or `diff`. The tools are therefore only as inert as
the configured repository, which is the user's choice; the legacy layout and
ownership checks are not ported.
