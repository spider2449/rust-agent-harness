# RAH — Rust Agent Harness

RAH is a Rust agent harness built around a plugin kernel: **everything is a
plugin**. A minimal kernel owns plugin lifecycle, typed services, hooks, events,
scoped resources and configuration. Model providers, the agent loop, tools,
session logs, storage, sandboxes, policy, context, skills, scheduling and
frontends are all plugins that can be swapped or recomposed without patching
the core. RAH orchestrates inference providers; it is not an inference engine.

## Status

`master` has completed phases P0 to P6 of the
[plugin-kernel pivot plan](docs/plans/2026-10-11-plugin-kernel-pivot-plan.md)
(first released as 0.40.0; current release 0.43.0):
the kernel, the service contracts (`rah-api`), the plugins and the `rah` binary
run end to end with a mock model, with OpenAI or llama.cpp through the
Responses API, and with Anthropic through the Messages API. Tool calls are guarded by the fail-closed `policy-authority`
plugin, and tools come from built-in, repository, shell, MCP and
out-of-process plugins. The architecture decision is
[ADR 0036](docs/adr/0036-plugin-kernel-everything-is-a-plugin.md). The legacy
Desktop and its crates were removed in P5; the Web UI (`rah --web`) replaces
it. See [Architecture](docs/ARCHITECTURE.md) and [Security](docs/SECURITY.md).

## Quick start

```powershell
.\scripts\start.ps1              # Web UI with llama.cpp; opens the browser
.\scripts\start.ps1 -Preset dev  # Web UI with the mock model, no model server
.\scripts\start.ps1 -Cli         # terminal REPL instead
```

Scripts (Windows PowerShell 5.1 or later; if scripts are blocked, run them
as `powershell -ExecutionPolicy Bypass -File .\scripts\start.ps1`):

| Script | Does |
|---|---|
| `scripts\start.ps1` | Builds and runs `rah`. Web UI by default, opening the printed URL with its token; `-Cli` for the REPL. Options: `-Preset`, `-Config`, `-Model`, `-Root`, `-SessionDir`, `-Bind`, `-Release`, `-NoBrowser`. Warns when llama-server does not answer. |
| `scripts\install.ps1` | `cargo install` of `rah`, so `rah --web` works anywhere. Run it again after updating; `-Uninstall` removes it. |
| `scripts\check.ps1` | The validation suite (fmt, check, test, clippy, `git diff --check`), with the browser tests when Edge or Chrome is found. `-Quick`, `-Fix`, `-NoBrowser`, `-Browser <path>`. |
| `scripts\sessions.ps1` | Lists deleted sessions, `-Restore <id prefix>` moves one back, `-Purge [-OlderThanDays n]` removes them for good. |

Without the scripts:

```powershell
cargo run -p rah-cli -- --preset dev
```

The `dev` preset mounts `policy-authority`, `storage-fs` (plugin state in
`~/.rah/storage`), `session-jsonl` (logs in
`~/.rah/sessions`), `session-titles` (names each session from its first turn),
`tools-builtin` (`echo` and `fs.read` bounded to the current
directory), `sandbox-process` and `tools-repo` (read-only `repo.status`,
`repo.diff`, `repo.list` and `repo.search` over Git), `tools-repo-write`
(`repo.create-file`, `repo.edit-files` and `repo.patch`, each asking for
approval), `context-default` (a
sliding history window and memory notes kept per workspace in
`~/.rah/memory/<folder>-<hash>.md`, which the model can extend with
`memory.remember` after you approve), `skills-fs` (skills, shared by all
workspaces, in
`~/.rah/skills/<name>/SKILL.md`, listed to the model and loaded on demand with
`skills.load`), `scheduler-basic` (background jobs, and `agent.delegate` to
hand a task to a subagent in its own session), `model-mock`,
`loop-react` and `frontend-cli`. Type text to run a turn. Try
`read README.md` or `/call repo.status {}`, or use `/history`, `/fork <seq>`,
`/sessions`, `/resume <id>`, `/rename [title]` and `/plugins`. `/stop <plugin>`
stops a plugin and keeps its configuration, `/start <plugin>` mounts it again
and `/restart <plugin>` restarts it. `/` or `/help` lists the commands, and
Ctrl-C cancels a running turn (with no turn running, it exits). `--prompt <text>` runs one turn and exits, and
`--session <id>` resumes a session.

| Preset | Model plugin | Policy |
|---|---|---|
| `default` (the default) | `model-llamacpp` | `policy-authority` |
| `openai` | `model-openai` | `policy-authority` |
| `anthropic` | `model-anthropic` | `policy-authority` |
| `deepseek` | `model-chat-completions` (`provider = "deepseek"`) | `policy-authority` |
| `dev` | `model-mock` | `policy-authority` |
| `minimal` | `model-mock` | none: `rah` prints an `UNGUARDED` warning |

`policy-authority` denies every tool without a host permission assignment in
its own configuration; tool metadata can only make a decision stricter. The
presets assign `echo` (`none`), `fs.*` and `repo.*` (`read`), allow `none` and `read`,
and ask for `write` and `execute`. `frontend-cli` asks on the terminal and
approves only an explicit `y`; end of input denies. If a configured policy
plugin fails to start, `rah` refuses to run. The policy also runs at most
`max_in_flight` tool calls at once (default 128) and refuses one over the
limit. A plugin configuration with a typo is refused before it is applied, so
a refused change at runtime leaves the running plugin as it was.

General program execution is opt-in. Add `tools-shell` to a distribution file
next to `sandbox-process` (process supervision, not an OS sandbox), and assign
`shell.exec` in the policy:

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { echo = "none", "fs.*" = "read", "shell.exec" = "execute" } }

[[plugin]]
use = "sandbox-process"
config = { root = "." }            # cwd bound; cleared environment

[[plugin]]
use = "tools-shell"
config = { allow_programs = ["git", "cargo"] }   # optional allow list
```

`shell.exec` runs one program with an argument vector (no shell syntax), with
a timeout, bounded output and process-tree termination. With the mock model,
`/call shell.exec {"program":"git","args":["status"]}` exercises it.

Programs start with a cleared environment refilled from a short default list
(the path, system, temporary and home directories, and on Windows
`ProgramData`). When a program needs another variable, name it in
`inherit_env_extra`, which adds to the defaults; `inherit_env` replaces the
whole list. Both `sandbox-process` and `sandbox-os` take these keys, and the
Web UI's configuration example lists the defaults.

```toml
[[plugin]]
use = "sandbox-process"
config = { root = ".", inherit_env_extra = ["CARGO_HOME", "RUSTUP_HOME"] }
```

On Linux (landlock ABI 3, Linux 6.2 or later), `sandbox-os` can take the
place of `sandbox-process` and also confines what programs touch: writes
only under the root, a private `TMPDIR` and `writable`, reads under
`readable` (default `/`), and no network sockets unless `network = true`.
On Windows it runs programs at Low integrity: writes only under the root, a
private `TEMP` and `writable`, which it labels Low (the label stays on them;
`icacls "<dir>" /setintegritylevel "(OI)(CI)medium"` removes it; where the
user lacks the `WRITE_OWNER` right labeling needs, it grants the user that
too, and that grant also stays). Reads and
the network are not restricted there, so `network = true` is required. It
refuses to start elsewhere. See `docs/SECURITY.md` for what it does not
cover.

```toml
[[plugin]]
use = "sandbox-os"
config = { root = ".", writable = ["/home/me/.cargo/registry"], network = false }
```

```toml
# Windows
[[plugin]]
use = "sandbox-os"
config = { root = ".", network = true }
```

MCP servers are mounted one `tools-mcp` (stdio) or `tools-mcp-http`
(Streamable HTTP) instance per server. Only the tools listed in `tools` are exposed, as
`mcp.<server>.<tool>`, with the permission level written there; the policy
must assign them as well:

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { "mcp.files.*" = "read" } }

[[plugin]]
use = "tools-mcp"
id = "mcp-files"
config = { server = "files", program = "my-mcp-server", args = ["--stdio"], tools = { read_file = "read" } }
```

The server is started through `sandbox-process` (cleared environment,
working directory inside the sandbox root) and ends with the plugin.
[docs/MCP_SERVERS.md](docs/MCP_SERVERS.md) walks through adding a server,
with Blender ([`examples/mcp/blender.toml`](examples/mcp/blender.toml)) as
the example.

Web search is off by default. The simplest way to turn it on: in the Web UI
(`rah --web`), open **Settings → Tools** and tick **Web search**. That mounts
`tools-web-search`, makes the policy ask before every search or page fetch,
and is remembered (in `storage`) for the next start; untick it to turn it
off. In a distribution file it looks like the example below.

Web search is the opt-in `tools-web-search` plugin: `web.search` needs no
API key and tries DuckDuckGo, Bing, then any configured SearXNG instances
until one answers (a rate-limited or blocked engine cools down). An optional
`time_range` (`day` to `year`) limits results by age; Bing cannot filter by
age and is skipped then. `web.multi_search` asks all engines at once and
merges the results, marking which engines returned each. `web.fetch` reads
one page as text (title, body without scripts or styles, `<pre>` kept,
declared charsets decoded, cut to `max_chars`) and refuses private and local
addresses, also behind DNS names and redirects. Scraped engines rate-limit
and sometimes answer with bot checks or unrelated results (both fall over);
a SearXNG instance you run (`searxng = ["http://..."]`) is the most
reliable backend. Results come back inside
`<untrusted-web-content>`. Queries and URLs go to third-party servers, so the
policy must assign the tools; asking first is a good default:

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { "web.*" = "read" }, decisions = { "web.*" = "ask" } }

[[plugin]]
use = "tools-web-search"
config = { engines = ["duckduckgo", "bing"], max_results = 8 }   # all optional
```

Long conversations: `context-default` keeps a window of the newest history
and drops the rest. The opt-in `context-compact` plugin summarizes the old
part with the mounted model instead, once the history passes
`trigger_chars`, keeping the newest `keep_chars` as they are. Summaries are
cached by the history they cover, reused by later requests and forks, and
extended as the conversation grows; with a `storage` provider mounted (the
presets mount `storage-fs`) they survive restarts. Each summary is an extra
model request, and the session log is never changed:

```toml
[[plugin]]
use = "context-compact"
config = { trigger_chars = 60000, keep_chars = 20000 }   # all optional
```

Data directory: everything RAH keeps between runs (sessions, plugin
storage, memory notes, skills and telemetry) lives in one directory,
`~/.rah` (`%USERPROFILE%\.rah` on Windows), whichever directory `rah`
starts in. `--data-dir <dir>` or the `RAH_HOME` environment variable moves
it. A relative data path in a configuration (`dir`, `memory_dir`,
`memory_file`) is relative to the data directory, an absolute one is used
as is, and `--session-dir` overrides only the session logs.

Your settings live there too. Without `--preset` and `--config`, `rah` runs
`rah.toml` in the data directory when it exists (your own plugin
composition, see below), otherwise the `default` preset. `rah --init-config`
(or `scripts\start.ps1 -InitConfig`) writes a preset (`--preset`, else
`default`) there as a starting point, or to `--config <file>`, with the
opt-in `tools-shell` commented out; it never overwrites a file. What you change in
the Web UI is kept under `storage/frontend-web`: the Settings forms, the
web search switch, changes made in the **Plugins** panel, the display
preference, the recent workspaces, which folders of the session list are
open and the layout of the side panel.

Plugin state: `storage-fs`, mounted by every preset, provides the `storage`
service (ADR 0041), a key-value store in which each plugin keeps its
entries under its own id, one file per entry in `~/.rah/storage`
(`config = { dir = "storage", max_value_bytes = 16777216 }`).
`context-compact` keeps its summaries there, and process and WASM plugins
that list `storage` in `requires` can keep state between calls and
restarts.

Diagnostics: the opt-in `telemetry-log` plugin writes one JSON line per
turn, model request and tool call to `~/.rah/telemetry/<date>.jsonl`, with
durations, the policy decision of each tool call and the token usage
providers report (ADR 0040). It sees every turn, including background and
subagent turns, and never records message text or tool output. The Web UI
status bar shows the tokens reported for the open session since the page
was loaded.

```toml
[[plugin]]
use = "telemetry-log"
config = { dir = "telemetry", keep_days = 14, include_arguments = false }   # all optional
```

Plugins can also be separate processes in any language, speaking JSON-RPC 2.0
on stdio ([protocol v3](docs/PROCESS_PLUGINS.md)). The Python example
[`word_tools.py`](examples/process-plugins/word_tools.py) provides two tools
and reads the session log through the host, which allows only the services
the plugin lists in `requires`.
[`tool_guard.py`](examples/process-plugins/tool_guard.py) has no tools; it
attaches to the tool hooks to deny and redact. Mounting `word_tools.py`:

```toml
[[plugin]]
use = "process"
id = "word-tools"
config = { program = "python", args = ["examples/process-plugins/word_tools.py"] }
```

`--web` serves a browser UI instead of the terminal REPL:

```powershell
cargo run -p rah-cli -- --preset dev --web            # http://127.0.0.1:8765
cargo run -p rah-cli -- --preset dev --web --bind 127.0.0.1:0
```

`rah` prints the address with a per-start access token; open exactly that
URL. The UI lists sessions grouped by the folder they worked in (the current
workspace first and open, the others folded, ten per folder until "Show
more") and searchable, with titles the model generates after the first turn; you can rename, delete
(moved to `~/.rah/sessions/deleted/`; the ✕ on a folder deletes all its
sessions, forks first, and leaves the folder's files alone), export (Markdown or JSON, saved where
you pick in browsers with a save dialog, otherwise as a download) and fork them,
regenerate the last answer or edit an earlier message and send it again (both
in a new branch, so the original is kept). It streams turns and asks you to
allow or deny tool calls the policy marks `Ask`. A status line shows the
workspace root, its Git branch and pending changes, the model and loop in
use, and whether a policy guards tool calls. Click the workspace path to
switch the folder the tools work in, without restarting. **Settings** sets
the model (llama.cpp address and model, OpenAI, Anthropic, or the mock), the system
prompt, which tools may run (allow, ask first or off) and the agent loop with
forms, and under **Display** whether replies show formatted (the default:
headings, lists, tables, highlighted code) or as raw Markdown; what you save there is remembered (in `storage`) and applied again at
the next start, and replaces that plugin's configuration from the preset or
`rah.toml`. While a turn runs, a line above the message box shows what it is
doing (waiting for the model, thinking, writing, running a tool, waiting for
your approval) and for how long, and the tab title is marked. The right-hand panel (**Workspace** in the header) shows the
workspace's files as a tree, opens them read-only in tabs (with syntax
highlighting for common languages), shows the branch and the Git changes
(by folder) under the tree, with a splitter between the two, opens a
change's diff read-only, and keeps an
activity log of the open session's turns and tool calls; it can be resized
or hidden. Its **Plugins** tab remains for raw plugin configuration: it
adds, configures, stops, starts, restarts, replaces and removes plugins while `rah` runs:
replacing `model-mock` with `model-llamacpp`, or one loop with another, takes
effect on the next turn without a restart. These changes are remembered and
applied again at the next start, after the Settings forms (saving a form for
the same plugin replaces its panel change); stopping or removing the policy
is never carried over. Typing `/` in the message box
lists commands (`/new`, `/plugins`, `/stop`, `/start`, `/restart`, `/cancel`)
and completes plugin ids. It listens on loopback only and
has no external resources.

Real models use the same composition with a different model plugin:

```powershell
# llama.cpp: start llama-server on http://127.0.0.1:8080 first (loopback only).
cargo run -p rah-cli                                 # default preset, sole served model
cargo run -p rah-cli -- --model qwen

# OpenAI: the key is read from the backend environment, never from config.
$env:OPENAI_API_KEY = '...'
cargo run -p rah-cli -- --preset openai --model your-model-id

# Anthropic: an API key from the Claude Console (API billing, not a
# Claude.ai subscription). Without --model it uses claude-opus-5-5.
$env:ANTHROPIC_API_KEY = '...'
cargo run -p rah-cli -- --preset anthropic
cargo run -p rah-cli -- --preset anthropic --model claude-sonnet-5-5

# DeepSeek, or any OpenAI-compatible Chat Completions server.
$env:DEEPSEEK_API_KEY = '...'
cargo run -p rah-cli -- --preset deepseek                     # deepseek-chat
cargo run -p rah-cli -- --preset deepseek --model deepseek-reasoner  # unverified, see below
```

`model-chat-completions` speaks the Chat Completions format
(`<base_url>/chat/completions`) that DeepSeek, Ollama, vLLM, llama-server and
many gateways serve. `provider` (`deepseek`, `ollama`, `llamacpp`, `vllm`)
fills in the address, model and key variable; `base_url`, `model`,
`api_key_env`, `max_tokens` and `temperature` override them, so another
server is just `base_url`:

```toml
[[plugin]]
use = "model-chat-completions"
config = { provider = "ollama", model = "qwen3:8b" }
# or: config = { base_url = "https://gateway.example/v1", model = "some-model", api_key_env = "GATEWAY_KEY" }
```

`base_url` must be HTTPS, or plain HTTP to a loopback address, so a key never
crosses the network in clear text. Without a model, the server's sole model
(from `<base_url>/models`) is used. Images in tool results are sent as a
short text, since many servers accept only text in a tool message.
Reasoning is not sent back in later requests; if a thinking model (such as
`deepseek-reasoner`) requires its reasoning between tool calls, it may
reject the request after a tool call.

`model-anthropic` also takes `max_tokens`, `effort` (`low` to `max`) and
`refusal_fallback` in its configuration. Without an Anthropic key, it can
run against llama-server's Anthropic-compatible endpoint: set `endpoint =
"http://127.0.0.1:8080/v1/messages"` and point `api_key_env` at any
non-empty variable.

The model plugins are tested against deterministic loopback fixtures.
`model-chat-completions` was also run against a live llama-server (tool
calls, reasoning and token usage); the live OpenAI, Anthropic and DeepSeek
APIs have not been verified.

Compose your own plugins with a distribution file:

```toml
# rah.toml — run with: rah --config rah.toml, or save it as ~/.rah/rah.toml
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.*" = "read" } }

[[plugin]]
use = "session-jsonl"     # logs in `sessions` under the data directory

[[plugin]]
use = "tools-builtin"

[[plugin]]
use = "model-mock"
id = "model-a"            # instance id; defaults to the `use` name
config = { label = "A" }

[[plugin]]
use = "loop-react"            # or "loop-plan": plan first, then execute step by step
config = { max_rounds = 8 }

[[plugin]]
use = "frontend-cli"
```

| Phase | Scope |
|---|---|
| P0 | Freeze the legacy line, ADR 0036, new `AGENTS.md`, legacy docs archived, Codex adapter removed |
| P1 | `rah-kernel`: lifecycle, services, hooks, events, scopes, config |
| P2 | `rah-api` and a minimal end-to-end path: mock model, `loop-react`, built-in tools, JSONL sessions, CLI |
| P3 | Real model plugins (OpenAI Responses, llama.cpp) |
| P4 | `policy-authority`, sandbox, MCP, repository tools, process plugins v2 |
| P5 | `frontend-web` replaces the legacy Desktop |
| P6 | WASM plugins, skills, scheduler and subagents, repository write tools |

The first release of the plugin-kernel architecture is `0.40.0`.

## Legacy line

The authority-first architecture ended with
[v0.35.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.35.0)
and is preserved on branch `legacy/authority-first` (reference and critical
fixes only). Its README, architecture, security model, release gates and plans
are archived under [docs/legacy](docs/legacy/README.authority-first.md).

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal suite uses deterministic local fixtures and needs no network access,
credentials, paid API or real model. Some tests need `git` on PATH; the Python
process-plugin example runs only when a Python 3 interpreter is on PATH. The
Web UI browser tests run only when `RAH_TEST_BROWSER` names a Chromium-based
browser (Chrome, Chromium or Edge), for example
`$env:RAH_TEST_BROWSER = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"`.

Open problems, local-only test failures and notes for setting up another
development machine are in [Known issues](docs/KNOWN_ISSUES.md).

## Contributing

Read [AGENTS.md](AGENTS.md) before changing code. It defines the kernel,
plugin and trust rules for `master`.
