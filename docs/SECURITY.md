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
- `commands` lists `shell.exec` commands that run without asking (ADR 0050):
  `["git", "status"]` matches the program and arguments exactly, and a last
  element `"*"` accepts any further arguments. A rule only turns Ask into
  Allow; it does not override `decisions`, a missing assignment or a denied
  level. A wildcard allows every option of the program, such as
  `git diff --output=<file>`, so write one only for a program whose options
  are all acceptable. `program` is compared as a string, so `./git` and
  `/usr/bin/git` are not `git`.
- `approve` lists exact tool names that run without asking (ADR 0054), for a
  scripted run: `rah --prompt ... --approve repo.create-file` adds the names
  for one run and prints a warning. Like `commands` it only turns Ask into
  Allow and never overrides `decisions`, a missing assignment or a denied
  level; patterns and `*` are refused. It trusts the tool, not one call, so
  every argument the model sends to a listed tool is allowed.
- An invalid configuration is refused before it is applied (ADR 0044): at
  boot `rah` refuses to start instead of running unguarded, and a changed
  configuration (Web UI Settings or the Plugins panel) that is refused
  leaves the running policy and its configuration in place.
- `Ask` needs a `tool.approval` handler; without one it is a denial.
  `frontend-cli` shows the call and its arguments on the terminal and
  approves only an explicit `y` or `yes`. End of input or any other answer
  denies.

The policy registers at hook priority 0. A mounted plugin with a higher
priority runs first and can pre-empt it; mounting such a plugin is a trust
decision.

The policy also bounds how many tool calls run at once, across all sessions
(`max_in_flight`, default 128, the limit of the legacy host), through the
`tool-admission` service (ADR 0045). `ToolInvoker` asks for a slot after the
policy allowed a call and holds it while the tool runs, so the slot is
released however the call ends. A call over the limit is not started: it
returns an error, its effect is `none`, and nothing waits or retries.

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
  credentials and other variables are not passed), any names in
  `inherit_env_extra`, and the caller's spec;
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

`sandbox-os` (ADR 0042) provides the same service with the same
supervision plus OS-level confinement. **On Linux**, landlock (ABI 3,
Linux 6.2, or later) limits writes to the root, a private `TMPDIR` and the
configured `writable` paths, and reads to `readable` (default `/`); unless
`network = true`, a seccomp filter denies every socket except Unix-domain
ones, and `io_uring`. Confinement is applied before `exec`, so it covers
every descendant. It refuses to start without landlock ABI 3 or seccomp,
and on platforms other than Linux and Windows; it never falls back to plain
supervision. On Linux it does not hide readable files (the default
`readable` includes the user's credentials), does not stop connections to
Unix socket files such as `ssh-agent` or `docker.sock`, does not restrict
the network with `network = true`, and sets no CPU, memory or disk limits.
No preset mounts it.

On **Windows**, `sandbox-os` (ADR 0043) runs every program with a Low
integrity token without privileges (Administrators deny-only), started
suspended so it joins the Job Object before it runs. It labels the root, a
private `TEMP` and the `writable` paths Low, so the program cannot write
anything else the user owns (only places already labeled Low, such as
`AppData\LocalLow`). **The label stays on the root and `writable` after
unmount**, and new files there inherit it; `icacls "<dir>"
/setintegritylevel "(OI)(CI)medium"` removes it. Labeling needs
`WRITE_OWNER`; where the user lacks it (a standard user holding only Modify,
as on a non-system drive), `sandbox-os` first adds an inheritable ACE
granting the user `WRITE_OWNER`, which also stays. The owner could grant
that anyway, so it gives the user no access it could not take. Drive roots, the user
profile and the Windows directory are refused. It does **not** restrict
reads or the network: `readable` is refused and `network` must be set to
`true`, so a program can read the user's credentials and send them away. A
file moved into the root keeps its own label and stays unwritable. Batch
files are refused; run `cmd.exe /c` explicitly.

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
- `tools-http` sends model-chosen parameters to the API its configuration
  names (ADR 0047). The plugin is opt-in and each tool declares `read` or
  `write`, so the policy must assign every tool. Input is checked against the
  declared parameters, path values are percent-encoded, redirects are not
  followed, credentials come from environment variables, and responses are
  bounded, have angle brackets replaced and arrive inside
  `<untrusted-api-response>`. A `write` tool that loses its response is
  `uncertain` and is never replayed. It is not network isolation: the
  process can reach whatever `base_url` reaches.
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
no `model.before_request` hooks. With a `storage` provider, summaries of
conversations are kept there in plain text (with `storage-fs`, under
`storage/context-compact` in the data directory) until deleted.

## `storage`

The `storage` service (ADR 0041) holds plugin state. Namespaces separate
plugins: in-process plugins choose theirs (trusted code, a convention), and
the process and WASM hosts fix it to the plugin's id, so such a plugin can
reach only its own entries and only when `storage` is in its `requires`.
Names are limited to the plugin-id alphabet, so a key cannot form a path;
`storage-fs` additionally encodes every name into a single file name inside
its directory. Stored values are data a plugin wrote, possibly derived from
model output, and grant nothing. `storage-fs` files are plain, unencrypted
and readable by the user's account.

## `telemetry-log`

`telemetry-log` follows the `TurnEvent`s loops publish on the event bus
(ADR 0040) and writes records to local files. Records hold session ids,
tool names, policy decisions, durations, error messages (cut to 500
characters) and token counts, never message text or tool output. Tool
arguments, which can contain paths or secrets the model passed, are
recorded only with `include_arguments = true`. Files are plain text in
`dir`, deleted after `keep_days`. Token counts are provider metadata: they
are shown and recorded, never used for a decision.

## `tool-input-repair`

`tool-input-repair` attaches to `tool.repair_input`, which `ToolInvoker`
runs before `tool.before_invoke` (ADR 0049). The policy, the approver and
tool admission therefore see the repaired input, the same input the tool
receives. A handler can change only `call.input`: the invoker keeps the
tool name, call id and definition it found, and a handler that fails leaves
the input as the model sent it. The plugin converts, unwraps or drops
values; it never adds one, and only where the tool's input schema makes the
fix certain. The session log records the call as the model requested it.

## `format-on-write`

`format-on-write` runs formatters through the `sandbox` service on files that
write tools changed (ADR 0053). A formatter reads the project's own
configuration, so the defaults are limited to `rustfmt`, `gofmt`, `ruff`,
`black` and `clang-format`, none of which run code from that configuration.
`prettier` is not a default because its configuration can be JavaScript and
its plugins are code: naming it in `formatters` lets a file the model writes
make it run code at the sandbox's isolation, after only a `write` approval.
The same holds for any formatter you name. Only files that resolve inside
`root` and are at most 1 MiB are handed to a formatter, as a path relative to
it. The formatter's error output is shown to the model on one line, cut to
200 characters.

## `lsp-diagnostics`

`lsp-diagnostics` starts language servers through the `sandbox` service and
sends them the text of files that write tools changed. A language server is
trusted code the user configured (the defaults only run when the program is
installed): it can read the workspace and does whatever its isolation
allows, which under `sandbox-process` is anything the user's account can do.

A server can also run code from the workspace it analyzes: `rust-analyzer`
runs build scripts and procedural macros, and some servers load configuration
scripts. A build script the model writes can therefore run when the server
checks it, so a `write` approval can become code execution at the sandbox's
isolation. Enable the feature with `sandbox-os` confinement, or only for
workspaces whose writes you review.

Diagnostics are text from that program about files that may come from
untrusted sources. They are appended to tool output cut to `max_diagnostics`
entries of `max_message_chars` each, on one line and marked as output of an
external program. They grant nothing; the policy decides every later tool
call. Only files that resolve inside `root` and are at most 1 MiB are sent.

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
  output is inserted as text only. The model's replies are rendered as
  Markdown by default (`markdown.js`): it builds elements with fixed tags and
  text content, never HTML, so HTML in a reply stays visible text; links are
  made only for `http`, `https` and `mailto` URLs and open in a new tab
  without a referrer; images are shown as links and never loaded. The
  Display setting (`raw_markdown`) shows the source instead.
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

- `PUT /api/preferences` stores the UI's preferences in `storage` (key
  `preferences`): the display setting, recent workspaces (paths the user
  picked; at most 8), the folders open in the session list (at most 1000)
  and the side panel layout. They change only how the page looks and what
  the folder picker suggests.
- Successful changes through `/api/plugins` are stored in `storage` (key
  `plugins`, at most 256 plugin ids) and applied again when `frontend-web`
  starts, after the Settings forms: mounted or configured (rolled back when
  the plugin does not start), stopped, or unmounted. Stopping or removing a
  policy plugin is never stored or replayed, so a start never comes up
  unguarded because of it; a stored policy configuration is applied with
  the same rollback as the Settings form. Whoever can write the `storage`
  directory can therefore choose the plugins of the next start, as with
  `rah.toml`.
- `PUT /api/web-search {enabled}` is the Web UI's web search switch. On,
  it first adds `decisions = { "web.*" = "ask" }` to every mounted
  `policy-authority` that does not already decide `web.*`, then mounts
  `tools-web-search`; off, it unmounts `tools-web-search` and leaves the
  rule. The choice is stored in `storage` (namespace: the frontend's id,
  key `web-search`) and applied again when `frontend-web` starts, so
  whoever can write the `storage` directory can turn web search on, still behind
  the ask rule.
- `PUT /api/settings/{slot} {id, kind, config}` saves the Settings forms
  (`model`, `assistant`, `tools`, `loop`): configure, replace or mount
  through `admin`, rolled back to the previous configuration when the
  plugin does not start (a broken policy never stays failed), then stored
  in `storage` (key `settings-<slot>`) and applied again at the next start,
  the policy first. The stored `tools` settings are the whole
  `policy-authority` configuration, so whoever can write the `storage`
  directory controls the policy of the next start. The data directory
  (`~/.rah` by default, or `--data-dir`/`RAH_HOME`) is outside the workspace
  unless the workspace contains it; the repository write tools also refuse
  paths under `.rah/` for this reason (see below). A data directory with
  another name inside the workspace is not protected that way, and
  `shell.exec`, when mounted and approved, is not restricted either.
- `PUT /api/workspace` points the workspace plugins (`sandbox-process`,
  `sandbox-os`, `tools-builtin`, `tools-repo-write`) at another existing directory, as
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
  `requires` (error `-32003`), even when that service is mounted, and keeps
  `storage` calls in the namespace of the plugin's id;
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
the configured root; `git apply` rejects paths outside the work tree. None
of them writes under `.rah/`, where RAH keeps sessions, memory, skills and
`storage` (with the saved Web UI settings, the policy among them) when the
workspace contains the default data directory:
`repo.patch` lists the patch's paths with `git apply --numstat` first,
renames included. A failure after the first write is recorded as an
`uncertain` effect.

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
