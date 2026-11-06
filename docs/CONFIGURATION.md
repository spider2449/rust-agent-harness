# Configuring and running RAH

Everything `rah` runs comes from one file, `rah.toml` in the data directory
(`~/.rah`, `%USERPROFILE%\.rah` on Windows). Command-line options override it
for one run and never write it; settings saved in the Web UI are written to it
([ADR 0067](adr/0067-one-user-facing-configuration.md)). `rah config show`
prints what would run and where each value comes from.

- [Getting started](#getting-started)
- [The configuration file](#the-configuration-file)
- [Commands](#commands)
- [The terminal REPL](#the-terminal-repl)
- [The Web UI](#the-web-ui)
- [Features](#features)
- [Programs and sandboxes](#programs-and-sandboxes)
- [Tools from elsewhere](#tools-from-elsewhere): MCP, web search, HTTP APIs, process plugins
- [Context and prompts](#context-and-prompts)
- [Diagnostics](#diagnostics)
- [Model providers](#model-providers)
- [Data directory](#data-directory)
- [Advanced: plugin entries](#advanced-plugin-entries)
- [Scripts](#scripts)

## Getting started

```powershell
rah config init        # asks for the model, permissions and workspace; writes rah.toml
rah                    # terminal REPL
rah web                # Web UI in the browser
rah-desktop            # the same Web UI in a native window
```

`rah` without a configuration asks the same questions on a terminal before
it starts, and elsewhere exits with a message saying how to choose a model.
It never guesses one. To write the file without questions:

```powershell
rah config init --model anthropic/claude-sonnet-5-5 --permissions auto-edit --with web-search
```

To install RAH, or update it after pulling changes, run `.\scripts\update.ps1`
from the checkout; see [Scripts](#scripts).

## The configuration file

```toml
model       = "anthropic/claude-sonnet-5-5"   # <provider>/<model id>
permissions = "ask"                            # read-only | ask | auto-edit | full
root        = "D:/work/project"                # workspace; default: where rah starts
features    = ["web-search", "shell=git,cargo"]

[model_options]                                # optional, provider-specific
max_tokens = 32000
```

### `model`

| Provider | Plugin | Key variable | Notes |
|---|---|---|---|
| `anthropic` | `model-anthropic` | `ANTHROPIC_API_KEY` | Claude Console key (API billing). Default model `claude-opus-5-5`. |
| `openai` | `model-openai` | `OPENAI_API_KEY` | Responses API. |
| `deepseek` | `model-chat-completions` | `DEEPSEEK_API_KEY` | Default `deepseek-chat`. |
| `openai-compatible` | `model-chat-completions` | `api_key_env` in `model_options` | Any Chat Completions server; `model_options.base_url` names it. |
| `llamacpp` | `model-llamacpp` | `RAH_LLAMA_CPP_API_KEY`, optional | llama-server on `http://127.0.0.1:8080`; without an id, its sole model. |
| `mock` | `model-mock` | none | A canned reply that calls `echo`; for trying RAH out and for tests. |

`<provider>` alone uses the plugin's default model, and the id may contain
`/` (`llamacpp/org/model`). API keys stay in environment variables: the file
names the variable (`model_options.api_key_env`), never the key. Everything
in `model_options` goes to the model plugin's configuration; see
[Model providers](#model-providers).

### `permissions`

Every tool declares a permission level, and the `policy-authority` plugin
assigns each tool one (`echo` is `none`, `fs.*` and `repo.*` are `read`, the
write tools `write`, `shell.exec` and `repo.commit` `execute`). The mode says
which levels run and which ask first:

| Mode | Runs | Asks first | Denied |
|---|---|---|---|
| `read-only` | none, read | | write, execute |
| `ask` (default) | none, read | write, execute | |
| `auto-edit` | none, read, write | execute | |
| `full` | everything | | |

No mode grants a tool the policy has no assignment for: it is denied, and tool
metadata can only make a decision stricter. `full` prints a warning at every
start. A question is answered in the terminal (only `y` or `yes` approves; `a`
also allows that tool for the rest of the session, never one that needs
`execute`) or in the Web UI.

### `root` and `features`

`root` is the workspace for file tools and programs; without it, the
directory `rah` starts in. [Features](#features) turn on opt-in capabilities.

### Precedence

1. Command-line options (`--model`, `--permissions`, `--root`, `--with`), for
   this run only.
2. `rah.toml` (or `--config <file>`), including what the Web UI saved.
3. The base composition: a guarded set of plugins (policy, storage, sessions,
   file, repository and shell tools, context, skills, scheduler, telemetry,
   plugin builder, loop, frontend).

`rah config show` names the origin of the model, permissions and root, and
`rah config show --plugins` lists every plugin with its configuration.

## Commands

| Command | Does |
|---|---|
| `rah` | Terminal REPL. `--session <id>` resumes a session. |
| `rah run "<prompt>"` | One turn, then exit. `--session <id>` continues a session; `--summary <file>` writes a JSON summary. |
| `rah web` | Web UI. `--bind 127.0.0.1:0` picks a free port; `--mode coding` enters a mode. |
| `rah sessions list` | Sessions, newest first, with title and workspace. Needs no model. |
| `rah sessions export <id> --format md\|json\|trace` | One session on standard output, without running a turn. |
| `rah config show [--plugins]` | What would run. |
| `rah config check` | Checks every plugin's configuration as a start would; exit 2 on a problem. Starts nothing. |
| `rah config init` | Writes `rah.toml` (asks, or takes `--model`). Never overwrites. |
| `rah config edit` | Opens the file in `$VISUAL`, `$EDITOR`, or Notepad / vi. |
| `rah config migrate` | Moves settings an earlier Web UI stored in the data directory into the file. |
| `rah features` | Lists the features. |

Global options go before or after the command: `--model`, `--permissions`,
`--root`, `--with <feature>` (repeatable), `--config <file>` and
`--data-dir <dir>`. `rah-desktop` takes the same ones, plus `--mode`.

`--summary` is written even when the turn fails: `outcome` (`completed`,
`failed`, `cancelled` or `stopped`), `error`, `wall_ms`, `model_requests`,
`model_retries`, `usage` (token counts summed over the provider's reports; a
count no report carried is `null`, not 0) and `tools` (`calls`, `errors`,
`uncertain`, and `unrecorded` for calls without a recorded result).
`--format trace` writes one tagged line per record (`[USER]`, `[ASSISTANT]`,
`[ACTION]`, `[OBSERVATION]`, `[OBSERVATION ERROR]`, `[ERROR]`, long text cut
with a count), for tools that read agent runs ([HARNESS2.md](HARNESS2.md)).

In Windows PowerShell 5.1 a piped `y` may arrive with a byte order mark and be
refused; use `--permissions`, or pipe from Bash.

## The terminal REPL

Type text to run a turn; Ctrl-C cancels it (with no turn running, it exits).
`/` or `/help` lists the commands: `/new`, `/sessions`, `/resume <id>`,
`/fork <seq>`, `/rename [title]`, `/history`, `/changes` and `/undo` (take
back the session's newest file change while the files are still what it left,
[ADR 0060](adr/0060-undo-the-file-changes-of-a-session.md)), `/plugins`,
`/stop`, `/start` and `/restart <plugin>`. With the mock model,
`/call <tool> <json>` requests a tool and `/notool <text>` answers without
one, for example `/call repo.status {}`.

## The Web UI

`rah web` prints the address with a per-start access token; open exactly that
URL. It listens on loopback only and loads nothing from outside.

- **Sessions** are grouped by the folder they worked in, searchable and
  titled by the model after the first turn. They can be renamed, forked,
  exported (Markdown or JSON) and deleted (moved to
  `~/.rah/sessions/deleted/`; `scripts\sessions.ps1` restores them). The last
  answer can be regenerated and an earlier message edited, both in a new
  branch.
- **Approvals**: tool calls the policy asks about appear in the conversation.
  "Allow for this session" trusts that tool until the process ends; "Always
  allow this command", on a `shell.exec` call, adds that exact program and
  arguments to the policy's `commands` and keeps it in `rah.toml`.
- **Workspace**: the status line shows the workspace, its Git branch and
  changes, the model and loop, and whether a policy guards tool calls. Click
  the path to open another folder or add one; like a multi-root workspace in
  VS Code, tools and Git work in the active folder. The right-hand panel shows
  the files read-only, Git changes and diffs per repository, and an activity
  log of the session.
- **Settings** sets the model, the system prompt, which tools may run, the
  loop, web search and the display (formatted or raw Markdown, themes).
  Saving writes `rah.toml`, keeping its comments; a plugin's entry holds only
  what differs from the base, so additions in later releases still reach it.
  When a change cannot be kept (no configuration file, or a file that lists
  its plugins), the UI says so and the change lasts until the next start.
- **Plugins** adds, configures, replaces and removes plugins while `rah`
  runs; replacing the model or loop takes effect on the next turn. These
  changes are written to `rah.toml` too. Stopping and starting a plugin is a
  runtime switch and is not kept. The policy and the model cannot be removed
  from the file.

Display preferences, the layout, recent workspaces, the active mode and the
connected API group are UI state, kept in `storage` for every browser and
`rah-desktop` alike.

### Modes

The mode picker in the header (`--mode` at start) switches how RAH works, for
every window at once. **Daily work** stops the repository write tools, turns
web search on, hides the workspace and asks for direct, sourced answers;
**Coding** runs the write tools and shows the workspace. The last mode is
entered again at the next start. The `modes` key of `frontend-web` replaces
the built-in ones:

```toml
[[plugin]]
use = "frontend-web"

[plugin.config]
mode = "coding"

[[plugin.config.modes]]
id = "coding"
label = "Coding"
instructions = "Keep changes small and say which files you changed."
plugins = { "tools-repo-write" = true }

[[plugin.config.modes]]
id = "writing"
label = "Writing"
workspace = false
plugins = { "tools-repo-write" = false }
```

`plugins` runs (`true`) or stops (`false`) plugin kinds or instance ids;
policy and frontend plugins cannot be listed. `instructions` is added after
the system prompt; `workspace = false` hides files and Git; `views` lists the
plugin views the mode shows. A mode with `extends` inherits its parent's.

### Layout and views

The panels (`sessions`, `files`, `plugins`, and views plugins add, named
`<plugin id>/<view>`) sit in a left and a right column, as tabs.
`ui-layout-config` moves them
([ADR 0062](adr/0062-plugin-contributed-ui-views.md)); a misspelt panel stops
the start, and a panel left out is hidden:

```toml
[[plugin]]
use = "ui-layout-config"
config = { left = ["sessions", "telemetry-log/usage"], right = ["files", "plugins"], right_open = true }
```

`telemetry-log/usage` shows turns, requests, tokens and tool calls of the
process; `plugin-builder/plugins` lists your personal plugins; the `todo`
feature adds `todo/list`.

## Features

Each feature mounts its plugins and adds the policy entries they need; a
policy entry you wrote is never overwritten, and tools still go through the
policy.

| Feature | Does |
|---|---|
| `shell[=git,cargo]` | Program execution (`shell.exec`), limited to the named programs when given. On in the base. |
| `web-search` | `web.search`, `web.multi_search`, `web.fetch`; the policy asks before each call. |
| `format` | Runs the project's formatter on files the write tools changed. |
| `lsp` | Appends language server diagnostics to the output of file-writing tools. |
| `compact` | Summarizes old history with the model instead of dropping it. |
| `instructions` | Gives the model the workspace's `AGENTS.md` (text only; it grants nothing). |
| `telemetry` | One JSON line per turn, request and tool call. On in the base. |
| `todo` | A to-do list the model keeps per session, shown in the Web UI. |
| `plugin-builder` | The model writes personal plugins on request; saving asks. On in the base. |
| `plan` | Plan first, then execute step by step (`loop-plan`). |

## Programs and sandboxes

The base mounts `tools-shell` over `sandbox-process` (process supervision,
not an OS sandbox), and the policy asks before each `shell.exec`
([ADR 0064](adr/0064-guarded-presets-mount-the-shell.md)). `shell.exec` runs
one program with an argument vector (no shell syntax), with a timeout,
bounded output and process-tree termination. `features = ["shell=git,cargo"]`
limits the programs; `remove = ["tools-shell"]` turns it off.

Commands you always approve can be listed in the policy (ADR 0050). A rule
matches the program and arguments exactly; a last `"*"` accepts any further
arguments, so prefer exact rules:

```toml
[[plugin]]
use = "policy-authority"
config = { commands = [["git", "status"], ["git", "log", "--oneline"], ["cargo", "check", "*"]] }
```

Programs start with a cleared environment refilled from a short list (path,
system, temporary and home directories; on Windows `ProgramData`).
`inherit_env_extra` adds names, `inherit_env` replaces the list:

```toml
[[plugin]]
use = "sandbox-process"
config = { inherit_env_extra = ["CARGO_HOME", "RUSTUP_HOME"] }
```

On Linux (landlock ABI 3, Linux 6.2 or later) `sandbox-os` can take its place
and confine programs: writes only under the root, a private `TMPDIR` and
`writable`, reads under `readable` (default `/`), and no network unless
`network = true`. On Windows it runs programs at Low integrity: writes only
under the root, a private `TEMP` and `writable` (which it labels Low; the
label stays, `icacls "<dir>" /setintegritylevel "(OI)(CI)medium"` removes
it). Reads and the network are not restricted there, so `network = true` is
required. It refuses to start elsewhere; [SECURITY.md](SECURITY.md) lists what
it does not cover.

```toml
remove = ["sandbox-process"]

[[plugin]]
use = "sandbox-os"
config = { writable = ["/home/me/.cargo/registry"], network = false }
```

## Tools from elsewhere

**MCP servers** are one `tools-mcp` (stdio) or `tools-mcp-http` (Streamable
HTTP) instance each. Only the tools listed in `tools` are exposed, as
`mcp.<server>.<tool>`, and the policy must assign them. The server starts
through the sandbox and ends with the plugin; [MCP_SERVERS.md](MCP_SERVERS.md)
walks through one.

```toml
[[plugin]]
use = "tools-mcp"
id = "mcp-files"
config = { server = "files", program = "my-mcp-server", args = ["--stdio"], tools = { read_file = "read" } }

[[plugin]]
use = "policy-authority"
config = { tools = { "mcp.files.*" = "read" } }
```

**Web search** (`features = ["web-search"]`, or the switch in **Settings →
Tools**): `web.search` needs no API key and tries DuckDuckGo, Bing, then
configured SearXNG instances until one answers; `time_range` (`day` to
`year`) limits by age. `web.multi_search` asks all engines and merges the
results. `web.fetch` reads one page as text and refuses private and local
addresses, also behind DNS names and redirects. A SearXNG instance you run
(`searxng = ["http://..."]`) is the most reliable backend. Results come back
inside `<untrusted-web-content>`, and queries go to third-party servers:

```toml
[[plugin]]
use = "tools-web-search"
config = { engines = ["duckduckgo", "bing"], max_results = 8 }
```

**Internal HTTP APIs**: every `tools-http` instance is one API with its
endpoints declared in configuration. In daily work an "Internal API" selector
connects one at a time ([ADR 0047](adr/0047-api-groups-as-sub-modes.md)).
Tools default to `GET` and `read`; declare `method = "POST"` and
`permission = "write"` for an endpoint that changes anything.
`examples/http-api/` has a runnable version:

```toml
[[plugin]]
id = "http-ddt"
use = "tools-http"

[plugin.config]
base_url = "https://ddt.example/api"
header_env = { Authorization = "DDT_AUTH" }
tools = [
  { name = "ddt.order", description = "Looks up one order.", path = "/orders/{id}", params = { id = { type = "string", required = true } } },
]
```

**Process plugins** are separate programs in any language speaking JSON-RPC
2.0 on stdio ([protocol v4](PROCESS_PLUGINS.md)); WASM components are
described in [WASM_PLUGINS.md](WASM_PLUGINS.md). The host allows only the
services a plugin lists in `requires`. Examples in `examples/process-plugins/`:
`word_tools.py` (two tools), `tool_guard.py` (denies and redacts through the
tool hooks), `todo_board.py` (a Web UI panel):

```toml
[[plugin]]
use = "process"
id = "word-tools"
config = { program = "python", args = ["examples/process-plugins/word_tools.py"] }
```

**Personal plugins**: when you ask for a RAH plugin, the model writes a
process plugin in Python or Node.js (standard library only) with
`plugin-builder`. `plugins.save` checks it and writes it to `plugins/<name>/`
in the data directory with a `mount.toml` that lists the `[[plugin]]` entry
and the policy assignments to add to `rah.toml`. Saving asks first and never
mounts or runs anything. The `plugin-builder/plugins` panel lists, exports,
imports and deletes them.

## Context and prompts

`context-default` keeps a window of the newest history and memory notes per
workspace (`~/.rah/memory/<folder>-<hash>.md`, extended with
`memory.remember` after you approve). Without `system` or `system_file` it
sends a built-in agent prompt and appends the operating system, workspace,
date and how `shell.exec` runs programs
([ADR 0065](adr/0065-built-in-agent-prompt-and-environment.md));
`system = ""` sends none and `environment = false` leaves the facts out.

The `compact` feature (`context-compact`) summarizes the old part of a long
conversation with the model once it passes `trigger_chars`, keeping the
newest `keep_chars`. Summaries are cached in `storage` and reused; the session
log is never changed. When a provider says a request does not fit, more is
summarized and the request is sent once more (ADR 0052). A busy or
rate-limited model is retried by the loop (`retries`, `retry_delay_ms`).

The `instructions` feature (`context-instructions`, ADR 0061) gives the model
the workspace root's `AGENTS.md` on every request. `names` changes the file
names looked for and `files` adds your own (relative to the data directory).
The text grants nothing, and a repository you did not write can use it to
steer the model ([SECURITY.md](SECURITY.md)).

Skills (`skills-fs`) live in `~/.rah/skills/<name>/SKILL.md`, are listed to
the model and loaded on demand with `skills.load`. `scheduler-basic` runs
background jobs and `agent.delegate` hands a task to a subagent in its own
session.

## Diagnostics

`telemetry-log` (in the base) writes one JSON line per turn, model request
and tool call to `~/.rah/telemetry/<date>.jsonl`, with durations, policy
decisions and token usage, never message text or tool output (ADR 0040).

The `lsp` feature tells a language server about every file a write tool
changed and appends the errors to the tool's output (ADR 0048): it tries
`rust-analyzer`, `typescript-language-server`, `pyright-langserver`, `gopls`
and `clangd`. The `format` feature runs `rustfmt`, `gofmt`, `ruff` or `black`
(where `pyproject.toml` configures them) and `clang-format` (where
`.clang-format` exists) and tells the model which files changed (ADR 0053).
A default server or formatter that cannot start is skipped and reported once
(ADR 0057). `servers` and `formatters` add others or disable one:

```toml
[[plugin]]
use = "format-on-write"
config = { formatters = { zig = { command = ["zig", "fmt", "$FILE"], extensions = [".zig"] } } }
```

## Model providers

`model-chat-completions` speaks `<base_url>/chat/completions`, served by
DeepSeek, Ollama, vLLM, llama-server and many gateways. Through
`openai-compatible`, `model_options` sets `base_url`, `api_key_env`,
`max_tokens` and `temperature`:

```toml
model = "openai-compatible/qwen3:8b"

[model_options]
base_url = "http://127.0.0.1:11434/v1"
```

`base_url` (and `model-llamacpp`'s `endpoint`) may be HTTPS or plain HTTP,
including the local network, but a key is sent only over HTTPS or to
loopback, so it never crosses the network in clear text. Reasoning is not sent
back in later requests, so a thinking model that requires it between tool
calls may reject the request.

`model-anthropic` also takes `max_tokens`, `effort` (`low` to `max`) and
`refusal_fallback`. Without an Anthropic key it can run against
llama-server's Anthropic-compatible endpoint (`endpoint =
"http://127.0.0.1:8080/v1/messages"`, `api_key_env` pointing at any non-empty
variable).

The model plugins are tested against deterministic loopback fixtures;
`model-chat-completions` was also run against a live llama-server. The live
OpenAI, Anthropic and DeepSeek APIs have not been verified.

## Data directory

Sessions (`sessions/`), plugin storage (`storage/`), memory notes, skills,
telemetry, personal plugins and `rah.toml` live in one directory, `~/.rah`,
whichever directory `rah` starts in. `--data-dir <dir>` or `RAH_HOME` moves
it. A relative data path in a configuration (`dir`, `memory_dir`,
`memory_file`) is relative to the data directory.

`storage-fs` provides the `storage` service (ADR 0041): each plugin keeps
entries under its own id, one file per entry in `~/.rah/storage`. Process and
WASM plugins that list `storage` in `requires` can keep state there.

## Advanced: plugin entries

The user keys cover most setups. Under them, a configuration is a list of
plugins, and a file with user keys can change that list with `[[plugin]]`
entries and `remove`, as the Web UI does:

```toml
model = "anthropic/claude-sonnet-5-5"
remove = ["session-titles"]        # drop base plugins by instance id

[[plugin]]                         # in the base: changes it in place
use = "loop-react"
config = { max_rounds = 8 }

[[plugin]]                         # tables merge, so the base's assignments stay
use = "policy-authority"
config = { tools = { "mcp.files.*" = "read" } }

[[plugin]]                         # not in the base: added before the frontend
use = "tools-mcp"
id = "mcp-files"
config = { server = "files", program = "my-mcp-server", tools = { read_file = "read" } }
```

A changed plugin keeps the base configuration except the keys written: tables
merge key by key, and a string, number or array replaces. `replace = true` on
an entry starts that plugin's configuration over. To swap a plugin for
another kind, `remove` it and add the new one; `id` names a second instance.

The composition order is fixed: the base, then `model`, `permissions` and
`root`, then `remove` and the entries, then `features`. `rah config check`
reports every configuration problem before anything starts
([ADR 0059](adr/0059-check-every-configuration-before-boot.md)).

A file without user keys is read as before: `extends = "<preset>"` with
changes ([ADR 0058](adr/0058-distribution-file-extends-a-preset.md); presets
`default`, `dev`, `minimal`, `openai`, `anthropic`, `deepseek`), or the whole
plugin list. The Web UI does not write such a file. Leaving
`policy-authority` out runs tool calls unguarded, and `rah` warns
`UNGUARDED` at every start.

```toml
# The whole list: nothing is added for you.
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.*" = "read" } }

[[plugin]]
use = "session-jsonl"

[[plugin]]
use = "tools-builtin"

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
```

## Scripts

Windows PowerShell 5.1 or later; if scripts are blocked, run them as
`powershell -ExecutionPolicy Bypass -File .\scripts\start.ps1`.

| Script | Does |
|---|---|
| `scripts\update.ps1` | **The one to use.** Builds this checkout (release) and installs `rah` and `rah-desktop` into `%LOCALAPPDATA%\Programs\RAH`, the installer's directory, so existing Start Menu entries and PATH run the new version; on a new machine it adds them. Prints the old and new version, warns when another `rah` comes first on PATH, and trims the debug build cache when `target` passes 30 GB (`-MaxTargetGB`). Never touches `~\.rah`. |
| `scripts\start.ps1` | For development: runs `rah` from the checkout without installing: the Web UI (opening its URL), `-Cli` for the REPL, `-Desktop` for the window, `-InitConfig` to write `rah.toml`. Options: `-Model`, `-Permissions`, `-Config`, `-Root`, `-DataDir`, `-Bind`, `-Mode`, `-Release`, `-NoBrowser`. |
| `scripts\install.ps1` | Alternative to `update.ps1`: `cargo install` into `~\.cargo\bin`; `-Only Web\|Desktop`, `-Shortcuts` for Start Menu entries, `-Uninstall`. |
| `scripts/install.sh` | Linux: installs `rah`, `rah-desktop` and `rah-web`, menu entries and icon; `--only`, `--prefix`, `--no-menu`, `--uninstall [--purge]`. `rah-desktop` needs `libwebkit2gtk-4.1`. |
| `scripts/package.sh`, `scripts\package.ps1` | For releases only: archives (Linux tarball; Windows installer with Inno Setup 6, and a portable zip). |
| `scripts\check.ps1` | The validation suite, with browser tests when Edge or Chrome is found; `-Quick`, `-Fix`, `-NoBrowser`. |
| `scripts\sessions.ps1` | Lists deleted sessions, `-Restore <id prefix>`, `-Purge [-OlderThanDays n]`. |
