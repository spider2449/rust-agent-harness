# Changelog

## Unreleased

- `rah-desktop`: the Web UI in a native window (tao and wry; WebView2 on
  Windows). It boots the same distribution as `rah --web` in-process, with
  `frontend-web` on a free loopback port, and has no API or UI code of its
  own, so the browser and the desktop behave the same (ADR 0046). Closing
  the window stops RAH; a start failure is shown in the window.
- Modes, shared by the browser and the desktop: a picker in the header and
  `GET /api/modes`, `PUT /api/mode {mode}`. **Daily work** stops
  `tools-repo-write`, turns web search on and hides the workspace panel;
  **Coding** runs `tools-repo-write` and shows the workspace. A mode's
  `instructions` are added to every model request. One mode is active per
  `rah`; a switch is pushed to every window, remembered in `storage` and
  entered again at the next start. `frontend-web` takes `mode` and `modes`
  (replacing the built-in ones; policy and frontend kinds are refused), and
  `rah --mode` / `rah-desktop --mode` pick the mode at start.
- Web UI: **Wrap** switches in the file viewer and the activity log (a
  wrapped file keeps its line numbers beside the first row of each line and
  its highlighting), and **themes** under Settings → Display: follow the
  system, Light, Dark, Sepia, Midnight and High contrast. Both are kept in
  the preferences (`wrap_files`, `wrap_activity`, `theme`), so the browser
  and the desktop agree.
- Web UI: the session list, the file tree and the file view/activity log
  dividers can be dragged (and moved with the arrow keys); sizes are kept in
  the preferences (`layout`). The "Switched the workspace" warning now
  clears itself after a few seconds; it was never written to the session.
- `rah_cli::select_distribution`, `read_distribution`, `USER_CONFIG`,
  `Distribution::set_root` and `Distribution::use_web_frontend`, so `rah` and
  `rah-desktop` choose and adjust the distribution the same way.
- A plugin configuration is checked before it is applied. Plugins gain
  `check_config`, which the kernel runs before a mount and before a
  configuration change; a refused configuration fails the request with the
  reason (`invalid configuration for plugin ...`, HTTP 400 in the Web UI)
  and the running plugin keeps its old configuration. Before, a typo stopped
  the plugin and its dependents first, and for `policy-authority` left tool
  calls unguarded until the Web UI rolled the change back. Every built-in
  plugin checks its configuration with the same code it applies, and the
  unused `config_schema` manifest field is removed (ADR 0044). At boot, an
  invalid configuration now stops `rah` instead of leaving that plugin
  inactive.
- `policy-authority` limits how many tool calls run at once, across all
  sessions (`max_in_flight`, default 128, as in the legacy host). A call over
  the limit is not started and returns an error. The limit uses the new
  `tool-admission` service: `ToolInvoker` holds a permit while a tool runs,
  so it is released however the call ends (ADR 0045).
- `loop-react` and `loop-plan` send `ToolStarted` when the tool starts,
  after the policy and any approval, instead of after the tool returned, so
  frontends can show a long call as running.
- `rah --init-config` (`scripts\start.ps1 -InitConfig`) writes a preset
  (`--preset`, else `default`) as `rah.toml` in the data directory, or to
  `--config <file>`, as a starting point to edit, with the opt-in
  `tools-shell` commented out. It never overwrites an existing file.
- `shell.exec` tells the model what it is for: its description says it
  runs command-line programs (with `git pull` as an example), lists the
  programs `allow_programs` permits, and each input field is described,
  so a model can pick it for "git pull for me" without being told the tool
  name.
- `sandbox-process` and `sandbox-os` pass `ProgramData` to programs on
  Windows by default. Without it rustc cannot locate Visual Studio, so
  `cargo build` under `shell.exec` failed with "linker `link.exe` not
  found" although the Build Tools were installed.
- `sandbox-process` and `sandbox-os` take `inherit_env_extra`, variable
  names added to the default environment list, so passing one more
  variable no longer means restating the whole `inherit_env`. Their
  configuration examples (shown in the Web UI) now list the default
  `inherit_env`.
- The Web UI's plugin editor shows help for each configuration key of
  `sandbox-process`, `sandbox-os` and `tools-shell`: what it does, its
  default, and the related settings (the sandbox and policy entry
  `shell.exec` needs, `inherit_env_extra` for a missing variable). The
  `admin` service gains an optional `config_help`, and `GET /api/catalog`
  returns it as `help`.

## v0.43.0 (2026-10-15)

> One data home for everything RAH keeps between runs, a new
> `model-chat-completions` plugin for OpenAI-compatible servers (DeepSeek,
> Ollama, vLLM, llama-server) with a `deepseek` preset, and Windows fixes
> for `sandbox-os` (standard and elevated users) and `storage-fs`.

### One data home

- All data RAH keeps between runs now lives in one data directory,
  `~/.rah` by default (`--data-dir` or `RAH_HOME` move it), instead of
  `.rah` in whatever directory `rah` started in. Data under an old
  `./.rah` is not moved automatically.
- `rah` resolves the data paths of `storage-fs`, `session-jsonl`,
  `skills-fs`, `telemetry-log` and `context-default` against the data home
  at mount, also for plugins mounted or configured through the `admin`
  service. Relative paths are relative to the data home; memory stays
  opt-in.
- Without `--preset` and `--config`, `rah.toml` in the data home runs when
  it exists. `scripts/start.ps1` follows and gains `-DataDir`.
- The Web UI remembers Plugins panel changes (mount, configure, stop,
  start, remove, replace) in `storage` and applies them again at start,
  after the Settings forms; stopping or removing the policy is never
  replayed. Recent workspaces, open folders and the side panel layout are
  server preferences instead of browser localStorage.
- Session export asks where to save where the browser has a save dialog,
  and falls back to a download. A folder in the session list can delete
  all its sessions at once (`POST /api/sessions/delete`).

### `model-chat-completions`

- New crate `plugins/model-chat-completions` with the plugin
  `model-chat-completions`: streamed OpenAI-compatible Chat Completions
  (`<base_url>/chat/completions`) for DeepSeek, Ollama, vLLM, llama-server
  and gateways. `provider` (`deepseek`, `ollama`, `llamacpp`, `vllm`) fills
  in the address, model and key variable; `base_url`, `model`, `api_key_env`,
  `max_tokens` and `temperature` override them. `base_url` must be HTTPS or
  plain HTTP to a loopback address. Without a model, the sole model of
  `<base_url>/models` is used.
- Tool calls are streamed as fragments by index (whole calls, calls without
  an index or id, and object arguments are also accepted) and validated:
  one choice, consistent ids and names, JSON object arguments, a required
  finish reason (`length` and `content_filter` fail the request). Reasoning
  (`reasoning_content` or `reasoning`) is a signal only; usage, including
  DeepSeek's `prompt_cache_hit_tokens`, is reported (ADR 0040). Images in
  tool results become a short text.
- `rah`: new preset `deepseek` (`DEEPSEEK_API_KEY`); `--model` also sets
  `model-chat-completions`. The Web UI's model settings offer it with a
  server choice.
- Tested against loopback fixtures and run once against a live
  llama-server; the live DeepSeek API is not verified.

### Fixes

- `sandbox-os` on Windows now starts for a standard user whose root or
  `writable` path grants only Modify, as on a non-system drive. Labeling
  needs `WRITE_OWNER`, so it failed with "Access is denied"; the user is now
  granted `WRITE_OWNER` there first, and the grant stays like the label
  (ADR 0043, amended).
- `sandbox-os` on Windows now runs programs for an elevated user. With
  Administrators deny-only in the token, Windows could not create the
  program's hidden console, and every program exited with
  `STATUS_DLL_INIT_FAILED` (`0xC0000142`) before it ran. Each program now
  gets a pseudo console that RAH creates and its descendants share; what is
  written to the console itself is discarded (ADR 0043, amended). Closes
  the known issue about the Low-integrity tests failing on a Windows 10
  machine.
- `sandbox-os` on Windows no longer fails to start a program with "All pipe
  instances are busy" when two instances in one process name a pipe at the
  same instant; pipe names now include the instance.
- `storage-fs` retries a `put` whose final rename Windows briefly refuses
  ("Access is denied") while another writer replaces the same entry or a
  scanner holds it open.

## v0.42.0 (2026-10-14)

> The plugin-gaps plan (G1-G5): file tools without Git, `context-compact`,
> `telemetry-log` with token usage, `sandbox-os` on Linux (ADR 0042) and
> Windows (ADR 0043), the `storage` service with `storage-fs` (ADR 0041);
> plus process-plugin hooks and events (ADR 0039), a keyless web search
> plugin, Markdown replies, remembered Web UI settings and example plugin
> configurations.

### Example configurations in the plugin editor

- The Web UI's plugin editor shows the kind's example configuration (mostly
  its defaults) as a grey hint when the configuration is empty, and changes
  it with the kind. It is only a reference: an empty editor still means the
  defaults, and an empty configuration is shown empty instead of `{}`.
- **Fill in example** copies the example into the editor (asking first when
  it is not empty). Example values in angle brackets, such as
  `"<model id>"`, must be replaced: the editor refuses to save while one is
  left and names it.
- Every catalog plugin, `process` and `wasm` has `CONFIG_EXAMPLE`, tested to
  parse as its configuration. `PluginAdmin` gains `config_example` (a
  default method returning `None`), and `GET /api/catalog` returns
  `examples` by kind.

### OS sandbox on Windows

- `sandbox-os` now runs on Windows (ADR 0043): every program gets a Low
  integrity token without privileges (Administrators deny-only), is started
  suspended with `CreateProcessAsUserW` and joins the Job Object before it
  runs, with named pipes for its standard streams. The root, a private
  `TEMP`/`TMP` removed on unmount, and `writable` are labeled Low, so
  nothing else the user owns can be written. **The label stays on the root
  and `writable`** (new files inherit it; a labeled root is not relabeled).
  Drive roots, the user profile and the Windows directory are refused.
- Reads and the network are not restricted on Windows: `readable` is
  refused and `network = true` is required. Batch files are refused (run
  `cmd.exe /c`). AppContainer was tried and rejected: the `NUL` device is
  denied inside it, so Git cannot start.

### Formatted replies

- The Web UI renders the model's replies as Markdown (headings, lists and
  task lists, tables, block quotes, highlighted code blocks, links) with a
  local renderer (`markdown.js`) that builds DOM nodes only: HTML in a reply
  stays text, only `http`, `https` and `mailto` links are made, and images
  are shown as links, never loaded.
- **Settings → Display → Show replies as raw Markdown** shows the source
  instead. `frontend-web` stores it through `GET|PUT /api/preferences` in
  `storage`, so it is the same in every browser and after a restart.

### Remembered settings and a busy line

- The Web UI's Settings (model, assistant, tools, loop) are remembered:
  `frontend-web` saves them through `PUT /api/settings/{slot}` into
  `storage` and applies them again at the next start (the policy first). A
  saved configuration that does not start is rolled back, so a broken
  policy never leaves tool calls unguarded.
- While a turn runs, an animated line above the message box shows what it
  is doing and for how long; it turns amber while it waits for your
  approval, and the tab title is marked `● RAH — working`.
- `tools-repo-write` refuses paths under `.rah/` (sessions, memory, skills,
  storage with the saved settings); `repo.patch` checks every path in the
  patch, renames included. **Breaking** for a model that edited `.rah/skills`
  through these tools.

### Web search switch

- The Web UI's **Settings → Tools** has a **Web search** checkbox. Ticking
  it makes the policy ask before every `web.*` call (unless it already
  decides `web.*`), then mounts `tools-web-search`; unticking unmounts it.
  `frontend-web` serves it as `GET|PUT /api/web-search {enabled}` and keeps
  the choice in `storage`, applying it again at the next start.

### OS sandbox on Linux

- New opt-in plugin `sandbox-os` (ADR 0042), in the `sandbox-process`
  crate: the `sandbox` service with `sandbox-process` supervision plus,
  on Linux, landlock confinement (writes only under the root, a private
  `TMPDIR` removed on unmount, and `writable`; reads and execution only
  under `readable`, default `/`) and, unless `network = true`, a seccomp
  filter that denies every non-Unix socket and `io_uring`. It needs
  landlock ABI 3 (Linux 6.2) and, without network, an x86-64 or AArch64
  host; otherwise, and on every other platform, it fails to start rather
  than run programs unconfined. It is a workspace plugin (`--root`,
  `set_workspace`) and not in any preset.

### Plugin storage

- `rah-api` adds the `storage` service (ADR 0041): `KvStore` with `get`,
  `put` (atomic replace), `delete` and `list` (by prefix), byte values in a
  namespace per plugin (its id), names of 1 to 128 bytes of letters, digits
  and `._:-`. `MemoryStore` implements it in memory for tests.
- New plugin `storage-fs` (`plugins/storage-fs`) provides it as one file per
  entry in `dir` (default `.rah/storage`), with reversible lowercase file
  names, writes through a flushed, exclusively created temporary file and a
  rename (safe when several stores or `rah` processes share the
  directory: last writer wins per key, never a mixed value), and
  `max_value_bytes` (16 MiB). There is no compare-and-swap. Every built-in
  preset mounts it.
- Process plugins (protocol 3, unchanged version) and WASM plugins may list
  `storage` in `requires` and call `get {key}`, `put {key, value}`,
  `delete {key}` and `list {prefix?}` with JSON values; the host fixes the
  namespace to the plugin's id.
- `context-compact` keeps its summaries in `storage` when a provider is
  mounted, in the namespace of its id. **Breaking:** its `dir` option is
  removed; a configuration with `dir` fails to mount. Old cache files are
  not migrated (summaries are rebuilt on demand).

### Telemetry and token usage

- `rah-protocol` adds `ModelUsage` (optional `input_tokens`,
  `output_tokens`, `cached_input_tokens`), `ModelEvent::Usage` and
  `AgentEvent::ModelUsage` (ADR 0040). `model-responses` reports
  `response.usage`, `model-anthropic` reports `message_start` and
  `message_delta` usage (cache reads and writes counted as input), and
  `model-mock` reports an estimate. Loops forward it per model request.
- Loops publish every turn event on the kernel event bus as
  `rah_api::agent_loop::TurnEvent`, so plugins can follow turns they did
  not start.
- New opt-in plugin `telemetry-log` (`plugins/telemetry-log`) writes
  `turn`, `model_request` and `tool` records as JSONL to
  `.rah/telemetry/<YYYY-MM-DD>.jsonl` (UTC): outcome, error code,
  durations, time to first text, tool decision (`allowed`, `approved`,
  `denied`, `rejected`) and token usage. It never records message text or
  tool output; `include_arguments` adds tool arguments. Files roll over at
  `max_file_bytes` (10 MiB) and are deleted after `keep_days` (14).
- The Web UI status bar shows the input and output tokens reported for the
  open session since the page was loaded.

### Summarizing long conversations

- New opt-in plugin `context-compact` (`plugins/context-compact`). On
  `model.before_request`, before `context-default`, it replaces history
  beyond `trigger_chars` (default 60 000) with one system item holding a
  summary written by the mounted model, keeping the newest `keep_chars`
  (default 20 000) whole. The cut is always at a user message and never
  inside the current turn. Old history is sent in chunks of at most
  `chunk_chars`, each merged into the summary so far; tool results and
  arguments are shortened in the transcript.
- Summaries are cached by an FNV-1a hash of the history prefix they cover,
  so they are reused across requests and by forks that share that prefix,
  and a longer history extends the newest summary. (`dir`, which kept them
  on disk, was replaced by the `storage` service; see above.) A failure or `timeout_secs` (default 120) leaves the
  request as it was and is not retried for the same history for five
  minutes. Optional `model` and `instructions` change the summary request.
- `rah` lists the plugin in its catalog; no preset mounts it.

### File tools without Git

- `tools-builtin` adds `fs.list` (`path`, `depth`, `hidden`), `fs.glob`
  (`pattern`, `path`, `hidden`) and `fs.grep` (`pattern`, `path`, `glob`,
  `ignore_case`, `hidden`), so the model can find files in a folder that is
  not a Git repository. They follow the `fs.read` path rules (relative, no
  `..`, no symbolic link out of the root), walk in name order without
  following symbolic links or junctions, skip hidden entries unless asked
  and do not descend into `skip_dirs` (default `.git`, `target`,
  `node_modules`). Globs support `*`, `?`, `**` and `{a,b}`; a pattern
  without `/` matches file names at any depth. `fs.grep` takes a Rust
  `regex` pattern, skips binary, non-UTF-8 and files over `max_bytes`, and
  says how many it skipped. New limits `max_results` (default 500 lines) and
  `max_scan` (default 20 000 entries) bound each call, and output says when
  one cut it short. All three are declared `read`.
- The presets' `policy-authority` assignment changes from `"fs.read"` to
  `"fs.*"`. A distribution file that assigns only `"fs.read"` keeps
  denying the new tools until it assigns them.
- New dependency `regex` for `fs.grep`.

### Keyless web search plugin

- New opt-in plugin `tools-web-search` (`plugins/tools-web-search`): the
  `web.search` tool (`query`, optional `limit`, `engine` and `time_range`)
  searches without an API key. DuckDuckGo (HTML) and Bing are scraped, SearXNG instances are
  read as JSON, and engines are tried in configured order. A rate-limited
  engine cools down for five minutes and a blocked one for one minute.
  Results (title, URL, snippet of at most 300 characters) are deduplicated
  and wrapped in `<untrusted-web-content>`, with a `Note:` line naming the
  engine that answered. `time_range` (`day`, `week`, `month`, `year`) maps
  to DuckDuckGo `df` and SearXNG `time_range`; Bing ignores freshness
  filters without cookies, so a time-ranged search skips it and says so.
- The same plugin provides `web.fetch` (`url`, optional `max_chars`): one
  `http`/`https` page of a text-like type, at most 2 MiB, returned as its
  title and text (scripts, styles and other non-text elements dropped, cut
  to `fetch_max_chars`, default 20 000) in the same wrapper. It refuses
  private, loopback, link-local, shared and reserved addresses, for IP
  literals, resolved names and every redirect, uses no proxy, and follows at
  most five redirects; `fetch_private_networks = true` lifts the guard. No
  preset mounts the plugin.
- `web.multi_search` (`query`, optional `limit`, `engines` and
  `time_range`) queries the engines concurrently and merges the results by
  URL; each lists the engines that returned it, and results more engines
  agree on come first. It shares the cooldowns of `web.search`.
- A scraped engine whose results mention no word of the query (Bing serves
  unrelated pages to clients it suspects are bots) counts as an invalid
  response and falls over to the next engine.
- `web.fetch` decodes the charset of the content type or of an HTML
  `<meta>` tag (Big5, GBK, Shift_JIS, Latin-1 and the rest of the WHATWG
  encodings, UTF-8 otherwise) and keeps the whitespace of `<pre>` blocks.

### Web UI: folder-grouped sessions and a workspace panel

- The session list is grouped by the folder each session worked in: the
  current workspace first and open, other folders folded (remembered per
  browser), ten sessions per folder until "Show more". The "This folder /
  All folders" selector is gone.
- A right-hand panel (header **Workspace**, resizable, hideable) shows the
  workspace as a file tree, opens files read-only in tabs with line numbers,
  and keeps an activity log of the open session (turns, tool calls and
  results, live while a turn runs; a tool call's `path` opens that file).
  The Plugins panel moved into the same panel as a tab.
- The file view highlights Rust, JavaScript/TypeScript, Python, C-like
  languages, TOML, YAML, JSON, Markdown, shell/PowerShell, CSS and HTML/XML
  with an embedded tokenizer (`assets/highlight.js`, no external resources);
  files over 200 KiB stay plain.
- Git state under the file tree, read-only: the branch and the changed
  files grouped by folder with their status (`M`, `A`, `D`, `R`, `U`, `!`).
  A change opens its highlighted diff, an untracked file its content. A
  splitter (mouse, or arrow keys) sets the height of the tree and the Git
  list and is remembered.
- `frontend-web`: `GET /api/git/changes` and `/api/git/diff?path=`, each one
  `git` run through the `sandbox` service with `--no-optional-locks`,
  `--literal-pathspecs`, `--no-ext-diff` and `--no-textconv`. Paths are
  checked like the file browser's. A folder that is no repository answers
  `{error}`.
- `frontend-web`: `GET /api/files?path=` lists one workspace directory and
  `GET /api/files/content?path=` returns the first 512 KiB of one file
  (binary files return no text). Paths are relative to the `sandbox`
  service's root; absolute paths, drives, `:` streams, `..` and links that
  lead outside are refused.

### Hooks and events for process plugins (ADR 0039)

- Process plugin protocol 3. A manifest may list `hooks`
  (`tool.before_invoke`, `tool.after_invoke`, `model.before_request`, each
  with a `priority`) and `events` (`session.appended`). The host sends
  `hooks/call {hook, input}` requests and `event` notifications. Plugins
  answering protocol 2 still load and provide tools only.
- A hook that times out (`hook_timeout_ms`, default 5000), disconnects or
  answers malformed fails closed: the tool call is denied, the tool output
  becomes an error, or the model request fails. Hook calls are never
  retried.
- New example `examples/process-plugins/tool_guard.py`: denies tool calls
  whose arguments mention a configured word and redacts it from output.

## v0.41.0 (2026-10-13)

> Web UI usability (U1-U3: sessions, status line, workspaces, Settings),
> the `model-anthropic` plugin, images in tool output (ADR 0037) and MCP
> over Streamable HTTP (ADR 0038).

### MCP over Streamable HTTP (ADR 0038)

- New plugin `tools-mcp-http` in `plugins/tools-mcp`: the tools of one MCP
  server reached over Streamable HTTP (2025-03-26 or later), with JSON and
  SSE replies, session ids, `MCP-Protocol-Version`, `ping` answers,
  `notifications/cancelled` on timeout or cancellation, and a closing
  `DELETE`. It needs no `sandbox`.
- `url` must be `https`, or `http` for a loopback host; no redirects or
  proxies. `bearer_token_env` sends a token from an environment variable to
  that URL only. OAuth and the legacy HTTP+SSE transport are not supported.
- Lost streams and timeouts are `uncertain` and never resent. An ended
  session (`404`) fails calls until the plugin restarts.
- `tools-mcp` gains a `reqwest` dependency (as in the model plugins).
  Checked by hand against an MCP Python SDK 1.x server in SSE and JSON reply
  modes.

### Images in tool output (ADR 0037)

- `rah-protocol`: `ToolContent::Image { media_type, data }` (PNG, JPEG, GIF
  or WebP as base64). `rah_api::tools::image` checks untrusted images (type,
  base64, at most 4 MiB) and `describe_image` gives a short text for them.
- `tools-mcp` turns MCP image content into images; an unacceptable one
  becomes a note. Its default `max_message_bytes` is now 8 MiB. Process
  plugins may return `{"type": "image", "media_type", "data"}` items.
- `model-anthropic` sends image blocks in tool results and the Responses
  plugins send `input_image` parts. `model-llamacpp` does so only when
  llama-server lists image input for the model; otherwise the model gets the
  description. Request limits are 32 MiB.
- `context-default` sends images only during the turn that produced them.
  Earlier ones become their description, and each counts as 4,000
  characters in the history window.
- The Web UI shows tool images in the transcript; the terminal and Markdown
  export show the description. The Blender example exposes
  `get_screenshot_of_window_as_image`. Checked by hand: a Blender screenshot
  described correctly by a local llama.cpp vision model.

### Round limits, `repo.list` depth and a Settings notice fix

- `loop-react` and `loop-plan` no longer fail when a turn or step uses up its
  model rounds: one more request without tools asks for an answer from what
  was gathered. The turn fails only if that answer is empty.
- `repo.list` takes `depth` (1 to 16) to list a tree in one call.
- Fix: saving the loop in Settings could leave the wrong notice when a status
  poll ran during the save; out-of-order status answers are ignored.

### Stop and start plugins; interrupt turns

- `PluginAdmin` gains `stop` and `start`; a stopped plugin keeps its id and
  configuration. The policy plugin cannot be stopped. `/stop`, `/start` and
  `/restart` in the terminal, and Stop and Start in the Web UI.
- Ctrl-C cancels the running turn or approval prompt in the terminal
  (`host.interrupt` hook) and exits only when nothing handles it.
- The terminal lists commands with `/` or `/help` and suggests close matches;
  the Web UI has a slash command menu. `/call` and `/notool` run as turns.
- New guide `docs/MCP_SERVERS.md` with a Blender example.

### Anthropic model plugin

- New crate `plugins/model-anthropic` with the plugin `model-anthropic`: the
  Anthropic Messages API with an API key (API billing; no Claude.ai or
  Claude Code subscription login). The key is read at start from the
  variable named by `api_key_env` (default `ANTHROPIC_API_KEY`); a missing
  key leaves the plugin `Failed`. `endpoint` may only be the official
  endpoint or a loopback URL. `model` defaults to `claude-opus-5-5`;
  `max_tokens` (default 64000), `effort` and `refusal_fallback` (server-side
  refusal fallback for the models that support it, on by default) are
  optional.
- Requests stream with automatic prompt caching and are never retried.
  Leading system items become the system prompt; later ones are sent as
  marked user text. Thinking blocks are not replayed, because neutral
  context does not carry them. Output-limit and refusal stops are typed
  `Rejected` errors; an overloaded provider is a `Transport` error.
- `rah`: new preset `anthropic`; `--model` also sets `model-anthropic`.
  Web UI Settings offers Anthropic with model, key variable, effort and
  endpoint.
- Also accepts llama-server's Anthropic-compatible `/v1/messages` stream
  (tool-use starts without `input`, interleaved blocks). Checked by hand
  against a local llama-server with a tool round; not verified against the
  live Anthropic API.

### Web UI: show provider switches; keep the status line in place

- Saving the model or loop in Settings says what is now used, for example
  "Reconnected: llama.cpp (local server) as model-llamacpp", in the dialog
  and under the conversation. A model or loop replaced from the Plugins
  panel is announced the same way when the status line sees it. These
  notices end with the next message. A model provider connects on that
  message, so the notice does not claim the server is reachable.
- Fix: streaming output scrolls inside the conversation (up to half its
  height, following new output unless scrolled up) instead of growing the
  page, which pushed the status line up while the model was answering.

### Sessions belong to a workspace

- `rah-protocol`: `SessionInfo` gains `workspace` (additive).
- `session-jsonl` records the `sandbox` service's root as the workspace of
  a new session (in its header); a fork keeps its parent's workspace.
  Sessions created before this have none. `ExecEnv::root()` now reports the
  root without Windows' verbatim prefix, so recorded folders compare
  equal.
- Web UI: the list shows the sessions of the current workspace, then those
  without a recorded one; "All folders" lists every session with its
  folder. Opening a session of another folder, or sending in one, first
  switches the workspace to it, so a session's history and its tool calls
  always refer to the same files. Switching the workspace closes the open
  session of the folder left.
- Memory follows the workspace: `context-default` gains `memory_dir`, one
  memory file per workspace (`<folder>-<hash>.md`, named from the `sandbox`
  root on every request, so switching the workspace switches the notes and
  `memory.remember` writes to the current project's file). It excludes
  `memory_file`, which still sets one file for everything. The presets use
  `memory_dir = ".rah/memory"`; an existing `.rah/memory.md` is no longer
  read and can be moved to a project's file by hand. Skills stay shared.

### Fix: programs found the user's configuration

- `sandbox-process` now passes the home directory variables by default
  (`HOME`, and on Windows `USERPROFILE`, `HOMEDRIVE`, `HOMEPATH`). Without
  them Git could not read `~/.gitconfig` ("$HOME not set"), so
  `safe.directory` entries were ignored and repositories owned by another
  account failed with "dubious ownership" in the repository tools and the
  status line. They name a directory and carry no secret.
- The browser tests run one at a time.

### Settings (U3)

- `policy-authority`: new `decisions` (`allow`, `ask` or `deny` by tool name
  or `prefix*` pattern, matched like `tools`). A matching host decision
  replaces the level rules for that tool, so one tool of a pattern can be
  turned off, a read can ask first, or a write can run without asking. Tool
  metadata still cannot grant anything.
- Web UI: a Settings dialog with forms instead of JSON. Model: provider
  (llama.cpp, OpenAI, mock) with address, model name or API key variable;
  changing the provider replaces the plugin and keeps the old one if the new
  one does not start. Assistant: system prompt and history window
  (`context-default`). Tools: one rule for reading tools and one for writing
  tools and commands, and per tool Default (showing what applies), Allow,
  Ask first or Off. Loop: standard or plan-and-execute, with their limits.
  Each form changes only its own fields and keeps the rest of the
  configuration. The Plugins panel stays for everything else.
- Browser tests can save screenshots to `RAH_TEST_SCREENSHOT_DIR`.

### Status line

- `rah-api` `ExecEnv`: new `root()` (default `None`), the directory programs
  run in, for display only; `sandbox-process` reports its canonical root.
- `frontend-web`: `GET /api/status` reports the workspace root, its Git state
  from one read-only `git --no-optional-locks status --porcelain=v2 --branch`
  run through the `sandbox` service (branch or detached HEAD, upstream,
  ahead and behind, changed, untracked and conflicting files, or why Git
  failed), the model and loop providers the next turn uses (with kind and
  configured model), the number of tools and whether a policy guards them.
- The Web UI shows these in a status line at the bottom, refreshed at start,
  after every turn and plugin change, and every 30 seconds while visible.

### Switching the workspace

- `rah-api` `PluginAdmin`: new `set_workspace(root)` (default: unsupported)
  returning `Workspace { root, plugins }`. The distribution implements it
  with the plugins `rah --root` sets (`rah_cli::WORKSPACE_PLUGINS`:
  `sandbox-process`, `tools-builtin`, `tools-repo-write`, sandbox first):
  it checks that the directory exists, sets their `root` and restarts them
  with their dependents. Sessions, memory and skills stay where they are.
- `frontend-web`: `PUT /api/workspace {root}`, and `GET /api/fs/dirs?path=`
  (subdirectory names, Git repositories marked, and the drives or `/` to
  start from) for a folder picker.
- The Web UI opens the picker from the workspace path in the status line:
  type a path or browse (up, drives, folders, recently used folders), then
  use the folder. The picker refuses while a turn runs in its tab; a turn
  running elsewhere can see a tool call fail while the tools restart.

### Scripts

- `scripts/start.ps1` (run `rah`, Web UI by default, opening the tokenized
  URL; warns when llama-server is down), `scripts/install.ps1`,
  `scripts/check.ps1` (the validation suite, with browser tests when Edge or
  Chrome is found) and `scripts/sessions.ps1` (list, restore or purge deleted
  sessions). Windows PowerShell 5.1 compatible.

### Session management

- `rah-protocol`: `SessionInfo` gains `title`, `updated` and `preview`
  (additive; old readers ignore them), plus `normalize_title`.
- `rah-api` `SessionLog`: new `rename`, `rename_if_untitled` and `delete`
  (default implementations report `Unsupported`), errors `HasForks`,
  `InvalidTitle` and `Unsupported`, and the event `SessionAppended`, which
  providers emit after each append.
- `session-jsonl`: titles are appended `title` metadata lines (the last one
  wins; records are never rewritten). `delete` moves the log to
  `<dir>/deleted/` and is refused while forks of the session exist. `list`
  reports the last change (file time) and a preview of the first message.
  Logs with title lines cannot be read by 0.40.0.
- New plugin `session-titles`, mounted by every preset: after a session's
  first completed turn it asks the model, in one tool-free request without
  `model.before_request` hooks, for a short title, and stores one cleaned line
  (`<think>` blocks, quotes, `Title:` prefixes and final punctuation removed)
  only if the session is still untitled. Without a usable reply it uses the
  start of the first message.
- `frontend-web`: `PUT /api/sessions/{id}/title`, `DELETE /api/sessions/{id}`
  and `GET /api/sessions/{id}/export?format=md|json` (a download named after
  the session id). The UI lists sessions by last activity, grouped by day,
  with titles, a search box, and rename and delete actions; the conversation
  header renames, exports and deletes; user messages can be edited and sent
  again, and the last answer regenerated, each in a new branch that keeps the
  title.
- `frontend-cli`: `/rename [title]`; `/sessions` shows titles or previews.

### WASM: cancelled calls are interrupted

- `rah-plugin-host-wasm` enables wasmtime epoch interruption. Dropping a
  tool call, which is how `ToolInvoker` cancels, marks that call and moves
  the engine epoch; the marked guest traps at its next epoch check and frees
  its blocking thread. Unmarked calls of the same plugin continue.
  Previously a cancelled guest ran until it returned or its fuel ran out.
- Test: with a single blocking thread, a spinning guest with practically
  unlimited fuel is cancelled and the next call still runs.

### Web UI browser test

- New test `crates/rah-cli/tests/web_browser.rs` drives the embedded Web UI
  in a headless Chromium-based browser over the DevTools protocol, against an
  in-process `rah` with the mock model: connecting, sessions, turns, Enter to
  send, forks, the Plugins panel (configure, replace, add, remove), `Ask`
  approvals (allow, deny, cancel), reload, and a console free of errors and
  CSP violations. It runs when `RAH_TEST_BROWSER` names the browser and is
  skipped otherwise; CI sets it to the runner's Chrome. No new dependency.
- Fix: cancelling a turn while a tool call waits for policy or approval now
  ends the call at once (`cancelled before the tool started`, effect `none`).
  `ToolInvoker` previously ignored cancellation until the approver answered
  or timed out (300 seconds in `frontend-web`), and the approval card stayed
  open.
- Fix (Web UI): a notice from a failed startup request, such as "no session
  service is mounted", is no longer cleared when the WebSocket connects.
- Fix (Web UI): the approval card shows only the policy's reason; it no
  longer adds the tool's declared permission, which could contradict it
  ("requires `write` permission · permission: none").
- Fix (Web UI): no `/favicon.ico` request (and 404) on every page load.

## v0.40.0 - first plugin-kernel release (2026-10-12)

> First release of the plugin-kernel architecture ("everything is a plugin"),
> phases P0-P6 of the pivot plan. The authority-first line ended at v0.35.0.

### P6: `scheduler-basic` and subagents

- `rah-api`: new `scheduler` service (`submit`, `list`, `cancel` of
  `JobSpec { input, session, delay, every }`).
- New plugin `scheduler-basic`: jobs run a turn through the `loop` service
  after a delay, once or repeatedly, with a concurrency limit, as tasks of
  its scope; jobs can also come from its configuration. It provides
  `agent.delegate {task}` (`read`), which runs a subagent turn in a new
  session and returns the answer and session id, with a depth limit.
- Every preset mounts it; the preset policy assigns `agent.*` = `read`.
- `frontend-web`: `GET|POST /api/jobs` and `DELETE /api/jobs/{id}`.

### P6: `loop-plan`

- New plugin `loop-plan`, a second `loop`: one tool-free model request
  writes a numbered plan (recorded as an assistant message), then each step
  (`max_steps`, 8) runs as a bounded model and tool loop
  (`max_rounds_per_step`, 16) with the plan and current step in a system
  item. It follows the same contract as `loop-react`: `model.before_request`,
  `ToolInvoker`, log-before-continue, per-request model resolution and
  scope-owned turns.
- Tests run it from configuration and swap it in for `loop-react` at runtime
  on an existing session.

### P6: `skills-fs`

- New plugin `skills-fs`: skills are `<dir>/<name>/SKILL.md` files with
  optional `name` and `description` front matter. Every model request gets
  one system item listing them, and `skills.load {name}` (`read`) returns a
  skill's text on demand. The directory is read on every request. Skills
  are text only and grant nothing.
- Every preset mounts it with `dir = ".rah/skills"`; the preset policy
  assigns `skills.*` = `read`.

### P6: `tools-repo-write`

- New plugin `tools-repo-write` (requires `sandbox`) with
  `repo.create-file`, `repo.edit-files` (all-or-nothing exact-match edits,
  also matching CRLF files) and `repo.patch` (`git apply --check`, then
  `git apply`), all declared `write`. Every preset mounts it; the preset
  policy assigns the three tools `write`, so each call asks for approval.
- `rah --root` also sets its root.

### P5: legacy removal

- Removed the legacy Desktop (`rah-desktop`) and the crates only it used:
  `rah-runtime`, `rah-runtime-openai`, `rah-model`, `rah-tools`,
  `rah-tools-mcp`, `rah-tools-plugin`, `rah-sandbox` and
  `rah-profile-composition`. The Web UI (`rah --web`) replaces Desktop. The
  workspace no longer needs `default-members`, and 311 locked packages are
  gone.
- Removed the legacy test scripts (`scripts/`), the Desktop CI steps and the
  Desktop `.gitattributes` rule, plus the unused workspace dependencies
  `anyhow`, `sha2`, `tracing` and `tracing-subscriber`.
- `rah-protocol`: removed `live_evidence`, `RuntimeDiagnostic` and the
  unused `AgentInput`, `AgentRequest` and `AgentOptions`.
- The full workspace test suite now passes on Windows; the 43 legacy
  `RepositoryMemberStale` failures left with Desktop.

### P5: `context-default`

- New plugin `context-default`, a `model.before_request` handler: an
  optional system prompt; a sliding window over the history (`max_items`,
  `max_chars`) that starts only at a user message, so tool calls and results
  stay paired, always keeps the current turn and notes how many items were
  omitted; and memory notes from `memory_file`, injected on every request.
  With a memory file it provides `memory.remember` (`write` permission),
  which appends one line. The session log is never changed.
- Every preset mounts it with `memory_file = ".rah/memory.md"`, and the
  preset policy assigns `memory.*` = `write` (Ask).

### P5: plugin management

- `rah-api`: new `admin` service (`PluginAdmin`: `kinds`, `instance`,
  `mount`, `unmount`, `configure`, `restart`, `replace`). `replace` mounts the
  replacement (under the first free id) and unmounts the old plugin only once
  the replacement is active; otherwise the old one is kept.
- `rah` always mounts a `distribution` plugin first. It provides `admin` from
  the distribution's catalog (including `process` plugins) and records the
  kind and configuration of everything `rah` mounts. It refuses to unmount or
  reconfigure itself. `boot` and `admin` share one constructor path.
- `frontend-web`: `GET /api/catalog`, `POST /api/plugins`,
  `DELETE /api/plugins/{id}`, `POST /api/plugins/{id}/restart`,
  `PUT /api/plugins/{id}/config` and `POST /api/plugins/{id}/replace`, and a
  Plugins panel in the UI to use them. `GET /api/plugins` now includes kind
  and configuration.
- Acceptance: an end-to-end test replaces the model and the loop over HTTP
  while one WebSocket keeps running turns in one session; the next turn uses
  the new model and the session history is intact. The Plugins panel itself
  has not been exercised in a browser.

### P5: `frontend-web` (core)

- New plugin `frontend-web` (`axum` 0.8 with WebSocket support; the first new
  dependency of the pivot, for the HTTP server): a loopback-only API with an
  embedded Web UI. `GET /api/info`, `/api/plugins`, `GET|POST /api/sessions`,
  `GET /api/sessions/{id}`, `POST /api/sessions/{id}/fork`, and a WebSocket
  at `/api/ws` for turns, their events, cancellation and approvals.
- Access control: a per-start token on every API request, `Host` and
  `Origin` checks, and a strict Content-Security-Policy for the UI.
- `tool.approval` is answered in the browser; without a connected browser
  the call is denied, and unanswered requests time out as denied.
- `rah-api`: new host event `FrontendListening { plugin, url }`; `rah` prints
  it. `rah --web [--bind <address>]` replaces the terminal REPL with the
  Web UI.
- Tests run the API over real HTTP and WebSocket connections (access control,
  sessions, forks, turns, approvals, unmount). The UI's JavaScript is
  syntax-checked but has not been driven in a browser by an automated test.

### P4: process plugins (protocol v2) and cleanup

- New crate `rah-plugin-host-process` (depends on `rah-api` and
  `rah-protocol` only): plugins as child processes speaking newline-delimited
  JSON-RPC 2.0 on stdio, documented in `docs/PROCESS_PLUGINS.md`. A process
  plugin declares its manifest in `initialize`, may provide `tools`, and may
  call the host's `session` service (`list`, `read`) through `service/call`
  only if `session` is in its `requires`; other services are refused with
  `-32003`. Calls are bounded, cancellable (`$/cancel`) and never replayed.
- `rah` mounts process plugins with `use = "process"` (an `id` is required).
  It starts each one once at boot to read the manifest; every later start
  must report the same services.
- Example: `examples/process-plugins/word_tools.py` (Python, standard library
  only). The host is tested with a Rust fixture plugin; the Python example
  runs end to end when a Python 3 interpreter is on PATH and is skipped
  otherwise.
- `rah-session` is removed. `rah-tools-plugin`, `rah-tools-mcp` and
  `rah-profile-composition` stay until P5, because the legacy Desktop still
  composes its profile with them.

### P4: `tools-mcp`

- `rah-api`: `ExecEnv::spawn(SpawnSpec)` starts a long-running program with
  piped stdin and stdout and returns a `ProcessHandle` (`wait`, `terminate`,
  stderr tail) that terminates the program when dropped. The default
  implementation refuses. New `ToolError::NotStarted`: the call never started,
  so its effect is `none` (`tools-shell` now uses it for spawn failures).
- `sandbox-process` implements `spawn`; supervision and stderr draining are
  tasks of its scope, so unmounting it terminates spawned programs.
- New plugin `tools-mcp` (requires `sandbox`), one instance per MCP stdio
  server. It runs the initialize handshake (protocol `2025-06-18`, accepting
  `2025-03-26` and `2024-11-05`), pages through `tools/list` and exposes only
  host-listed tools as `mcp.<server>.<tool>` with host-declared permissions.
  Calls are bounded and never replayed; timeouts and cancellation send
  `notifications/cancelled`. Server requests other than `ping` are refused.
- Tests drive a scripted in-process server through a fake sandbox. A manual
  run of `rah` against the legacy `rah-mcp-echo-server` binary through
  `sandbox-process` succeeded; that path has no automated test.

### P4: `tools-repo`

- New plugin `tools-repo` (requires `sandbox`) with read-only `repo.status`,
  `repo.diff` (unstaged, or `staged`, optionally per path), `repo.list`
  (tracked direct children) and `repo.search` (`path` or literal `text`
  mode), declared `read`.
- Git runs through the sandbox with the legacy hardening environment plus
  literal pathspecs, only at the top level of a work tree, with validated
  repository-relative paths and bounded output. `repo.diff-staged` becomes
  `repo.diff` with `staged`; `repo.file-info` and `repo.stats` are not ported
  yet.
- Every preset now mounts `sandbox-process` and `tools-repo`; the preset
  policy assigns `repo.*` = `read`.

### P4: `sandbox-process`, `tools-shell` and terminal approval

- `rah-api` gains the `sandbox` service: `ExecEnv::run(ProgramSpec, cancel)`
  runs one program to completion with an argument vector, a cwd relative to
  the environment root, extra environment, a timeout and per-stream output
  limits, and reports any `Termination` (timed out, output limit, cancelled).
- New plugin `sandbox-process` (process supervision, not an OS sandbox): a
  cleared environment refilled from `inherit_env`, null stdin, bounded
  output, a capped timeout and process-tree termination (Job Object on
  Windows, process group on Unix), ported from `rah-sandbox`. Supervision
  runs inside the caller's future with no detached tasks, and unmounting the
  plugin cancels running programs.
- New opt-in plugin `tools-shell` with `shell.exec` (`execute` permission,
  requires `sandbox`, optional `allow_programs`). Terminated programs and
  supervision failures are recorded as `uncertain`; spawn failures and
  rejected input as `none`.
- `frontend-cli` answers `tool.approval` on the terminal: only `y` or `yes`
  approves, and end of input denies. `run_repl` takes the approval receiver.
- `model-mock` gains `/call <tool> <json>` to request any offered tool.
- `rah --root` now also sets the `sandbox-process` root.

### P4: `policy-authority` and guarded presets

- New plugin `plugins/policy-authority`. It registers a `tool.before_invoke`
  handler that decides from its own configuration only. A tool without a host
  assignment (exact name, or a `prefix*` pattern) is denied. The effective
  level is the stricter of the assigned and the declared level, so tool
  metadata can tighten a decision but never grant one. Levels map to Allow
  (`allow`), Ask (`ask`) or Deny. Invalid configuration fails the plugin.
- Presets are now `default` (llama.cpp, and the new default of `--preset`),
  `openai`, `dev` (mock model) and `minimal` (mock model, no policy). Every
  preset except `minimal` mounts `policy-authority` first, assigning `echo`
  and `fs.read`. The `llamacpp` preset is renamed `default`.
- `rah` aborts the boot when a configured `policy-authority` does not become
  active, before any later plugin mounts, instead of running unguarded.
  `minimal` still starts with the `UNGUARDED` warning.
- No frontend answers `tool.approval` yet, so `Ask` is a denial. The legacy
  in-flight limit is not carried over.

### P3: real model plugins

- New crate `plugins/model-responses` with two plugins, `model-openai` and
  `model-llamacpp`. The SSE framing, Responses event validation and llama.cpp
  event normalizer are ported from `rah-runtime-openai`.
- Neutral context is translated to Responses input (`store: false`), and
  every streamed event is validated. Reasoning content never leaves the
  plugin, and nothing is retried.
- Tool names are encoded for the provider: function names are limited to
  `[A-Za-z0-9_-]`, so `fs.read` is sent as `fs_read`, and calls are mapped
  back to the original name. Collisions get a suffix.
- `model-openai` reads its key from a backend environment variable (default
  `OPENAI_API_KEY`) and sends it only to the official endpoint or a loopback
  URL. A missing key leaves the plugin `Failed` with a clear message.
- `model-llamacpp` accepts only loopback origins. It checks `/health` and
  `/v1/models` lazily on the first request (cached, and cleared on transport
  errors), so the plugin may start before the server. Its optional key comes
  from `RAH_LLAMA_CPP_API_KEY` and never falls back to the OpenAI key.
- `rah` gains the `llamacpp` and `openai` presets and a `--model` flag.
- Verification uses deterministic loopback fixtures only: 13 plugin tests
  plus an end-to-end loop, tool and session round trip through
  `model-llamacpp`. Neither llama.cpp nor OpenAI is live-verified.
- `rah-runtime-openai`, `rah-runtime` and `rah-model` remain until P5,
  because the legacy Desktop still depends on them.

### P2: `rah-api`, first plugins and the `rah` binary

- `rah-protocol`: provider-neutral `ContextItem`, `ModelRequest`, `ModelEvent`,
  session log types (`SessionEntry`, `SessionRecord`, `ToolEffect`,
  `ForkPoint`, `SessionInfo`) and `FromStr` for `SessionId`.
- New `rah-api` crate, which re-exports the kernel. It defines the service
  contracts `model`, `loop` and `session`, plus the tools contract. The tools
  service key is private, so `ToolInvoker` is the only way to reach a tool,
  and it always runs `tool.before_invoke`. A policy panic, an `Ask` with no
  approver, or a post-processing failure all fail closed. Effects are
  classified by tool permission, and uncertain effects are never retried.
  Standard hooks: `tool.before_invoke`, `tool.approval`, `tool.after_invoke`
  and `model.before_request`. `rebuild_context` replays a log without
  re-executing tool calls.
- Kernel: `KernelHandle::get_all` and `has_handlers`.
- New plugins under `plugins/`:
  - `model-mock`: deterministic and rule-based.
  - `tools-builtin`: `echo`, and `fs.read` bounded to the workspace, with
    traversal, absolute-path, escaping-symlink and size checks.
  - `session-jsonl`: append-only, with forks that write only a header and
    repair of a torn final line.
  - `loop-react`: the model resolved per round, scope-owned turn tasks, and
    cancellation.
  - `frontend-cli`: a REPL with `/new`, `/sessions`, `/resume`, `/fork`,
    `/history`, `/plugins`, plus single-prompt mode.
- `rah-cli` is rewritten as the `rah` distribution binary: a plugin catalog,
  `rah.toml` files, the `dev` preset, flag overrides and an `UNGUARDED`
  warning when no policy is mounted. The legacy CLI demo and its tests are
  removed.
- Architecture tests enforce `plugins/* -> rah-api, rah-protocol` and
  `rah-api -> rah-kernel, rah-protocol`. End-to-end tests cover the tool round
  trip, workspace-bounded `fs.read`, resume across processes, forks,
  hot-swapping the model plugin without restarting the loop, cancellation,
  replacing the loop plugin, the REPL and the binary.

### P1: `rah-kernel`

- New `rah-kernel` crate. It covers the plugin lifecycle (`Pending`,
  `Starting`, `Active`, `Stopping`, `Failed`, `DependencyCycle`), typed
  `ServiceKey<S>` services with named instances and mount bindings, and
  manifest-enforced `provides`, `requires` and `optional` capability checks.
  When a provider stops, its dependents stop first and restart automatically
  once the service resolves again. Lifecycle operations are serialized, and
  re-entrant calls are rejected.
- Typed bail, waterfall and serial hook chains ordered by priority. A
  panicking handler is reported as `HookError::Panicked`.
- Typed, panic-isolated event bus, `PluginStatusChanged` events, and host
  subscriptions.
- Scoped resources: registrations commit only when `apply` succeeds. Owned
  tasks are cancelled on dispose, then dispose effects run in LIFO order.
  `update_config` and `restart` restart a plugin together with its dependents.
- The kernel depends on no RAH crate and stays under 2,000 source lines; both
  rules are enforced by architecture tests. 27 deterministic tests.

### P0: freeze, ADR 0036, archive

- Accept [ADR 0036](docs/adr/0036-plugin-kernel-everything-is-a-plugin.md):
  plugin kernel, everything is a plugin. It supersedes ADRs 0001, 0002, 0003,
  0005, 0006, 0011, 0021–0026, 0030, 0033 and 0034 on `master`, reaffirms
  ADR 0004, and marks the other ADRs as legacy.
- Rewrite `AGENTS.md` for the plugin kernel. The previous instructions are in
  `docs/legacy/AGENTS.authority-first.md`.
- Archive the legacy README, architecture, security, guardrails, Codex
  documents, release gates, roadmaps, audits and pre-pivot plans under
  `docs/legacy/`. Add new transitional `docs/ARCHITECTURE.md` and
  `docs/SECURITY.md`.
- Remove `rah-runtime-codex` (the Codex adapter, certification harness and live
  examples) and the empty `rah-core` crate. Remove the `provider-codex` and
  `certification-harness` features and the Codex-only code from `rah-desktop`.
  The default-build test set is unchanged apart from one Codex-only
  architecture test.
- Exclude `rah-desktop` from the workspace default members. It is still covered
  by `--workspace`.

## v0.35.0 - final authority-first release (2026-10-11)

> **LEGACY LINE: FINAL RELEASE OF THE AUTHORITY-FIRST ARCHITECTURE.**
> v0.35.0 is the last release of RAH's original "host-owned authority, fixed
> extension points" architecture. Development then pivots to a plugin kernel
> ("everything is a plugin", a Rust counterpart of DeepSeek Harness); see the
> [plugin-kernel pivot plan](docs/plans/2026-10-11-plugin-kernel-pivot-plan.md).
> Branch `legacy/authority-first` points at this release for reference and
> critical fixes only. No further feature work lands on that line.

### Changes since v0.34.0

- Desktop shows a transient "Waiting for model..." / "Model is thinking..."
  indicator with elapsed time (`AgentEvent::ModelReasoningStarted`); reasoning
  content never crosses the adapter edge.
- `repo.stats`: bounded, read-only tracked-file statistics observer.
- Native Tool continuation no longer has an eight-round limit (#1).
- Tool input rejections and effect-free (`None`/`Read`) execution failures are
  returned to the model as correctable results instead of failing the turn;
  `repo.list` and `repo.file-info` path errors are correctable (#2).
- Desktop: sliding replay window, context usage, host-observed work-log memory
  with repository paths of successful calls, activity correlation and chat
  failure recovery, opt-in diagnostic log, Activity failure reasons, theme
  switcher, refreshed UI with Enter-to-send, versioned title, and legacy
  WebView2 fallbacks.
- All 14 workspace packages, internal lock records and Tauri bundle metadata
  aligned to 0.35.0. No dependency edge, ADR, authority category or HostExplicit
  route changed; HostExplicit remains exactly 11.

**OPENAI_LIVE_NOT_VERIFIED** and **NSIS_INSTALLER_NOT_VERIFIED** remain.
**DESKTOP_WINDOWS_TESTS_NOT_GREEN**: 44 local Windows `rah-desktop` tests fail
(43 `RepositoryMemberStale`, reproduced unchanged at v0.34.0; 1 flaky); all other
crates pass. Source-only release; no executable or installer assets.
See [the release gate](docs/legacy/RAH_V0.35_RELEASE_GATE.md).

## v0.34.0 - released (2026-10-09)

### Immutable publication record

- Release source and peeled tag target: `7b972988f2f116b87b3770d66be1ef970e20b726`.
- Annotated tag: `v0.34.0`; tag object: `504c298947f81f531364d99c1189b8281ddf5a6f`.
- Release-source master CI: [37889995880](https://github.com/spider2449/rust-agent-harness/actions/runs/37889995880) - PASS.
- Tag CI: [37890500633](https://github.com/spider2449/rust-agent-harness/actions/runs/37890500633) - PASS.
- [GitHub Release](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.34.0)
  ID: `407568690`; published `2026-10-09T05:55:25Z`;
  `draft=false`; `prerelease=false`; uploaded assets=`0`.
- Source-only artifact scope: GitHub source archives; no executable or installer assets.

Task 516 prepared the release, Task 517 recorded the explicit human waiver,
and Task 518 performed authorized publication. Task 519 closes the historical
documentation without changing the release source or tag.

### Capability summary

- Native OpenAI Responses and loopback llama.cpp runtime integration, with
  separate backend credentials and no implicit Codex fallback.
- Provider-neutral runtime composition and revocable host Tool lifetime; Codex
  remains an optional legacy adapter.
- Native Desktop multi-turn lifecycle, cancellation terminal, next-turn and
  reconnect corrections; missing-key recovery remains explicit.
- Model-selected Tool requests pass through host permissions and ToolRegistry.
- Redesigned conversation workspace, resizable/persistent side panels, and
  Inspector/Workspace section navigation with narrow-window support.
- All 14 workspace packages, internal lock records and Tauri bundle metadata
  aligned to 0.34.0; repository/security authority and HostExplicit 11 unchanged.

**OPENAI_LIVE_NOT_VERIFIED** remains: the human explicitly approved Task 517's
documented source-only waiver. Real Responses API acceptance is not certified;
the waiver is not live PASS. **NSIS_INSTALLER_NOT_VERIFIED** remains; no
executable or installer assets were uploaded, and cross-platform live behavior
is not certified.
See [the release gate](docs/legacy/RAH_V0.34_RELEASE_GATE.md) for validation and limits.


## v0.33.0 - released (2026-10-01)

### Immutable publication record

- Release source and peeled tag target: `ad25355c81ed0a39499cd75e5b240a7594f3e7a5`.
- Annotated tag: `v0.33.0`; annotation: `RAH v0.33.0`.
- Tag object: `2a187c6e634b330cea4d45f457497f165c6f84f2`.
- Tag CI: `36862035589` - PASS.
- GitHub Release ID: `400948501`; published `2026-10-01T12:31:40Z`;
  `draft=false`; `prerelease=false`.

Task 486 prepared the release. Task 487 published the immutable source without
source-code changes or a repository commit. Task 488 is later documentation
cleanup and does not change the release source or tag target.

### Added

- `repo.edit-untracked-file` corrects one existing eligible untracked UTF-8
  file inside the host-selected active admitted repository. It requires exact
  whole-file SHA-256 and byte-length preconditions, bounded exact literal
  replacements, and immediate repository-state revalidation before one
  prepared replacement attempt. Uncertain effects are reported conservatively.
  The file stays untracked.

### Maintenance

- Remembered Workspaces location controls now fit the viewport and remain
  usable in narrow windows. Native folder-picker permission is corrected, and
  repository switching preserves the disconnected lifecycle.
- Corrected Tauri command permission coverage and strengthened Desktop and
  Windows validation reliability. The certified Codex baseline is 0.157.1.

### Authority and publication

The new Tool uses ordinary repository-bounded authority. It does not stage,
commit, edit tracked or ignored files, or grant arbitrary filesystem writes.
The existing tracked-file mutation Tools retain their scopes. HostExplicit
remains exactly 11. The v0.33.0 tag and GitHub Release are published.

## v0.32.0 - released (2026-09-27)

### Immutable publication record

- Release source: `30d42f88aeac685f6e9ffcc6a9da7310f972e53c`.
- Annotated tag: `v0.32.0`; annotation: `RAH v0.32.0`.
- Tag object: `c3c895f0ceb96da68d7919d389a1521a717db5d9`.
- Peeled target: `30d42f88aeac685f6e9ffcc6a9da7310f972e53c`.
- Release-readiness CI: `36285833929` - PASS.
- Tag CI: `36286173085` - PASS.
- GitHub Release ID: `397465640`; name: `RAH v0.32.0`.
- Published: `2026-09-27T01:41:32Z`; `draft=false`; `prerelease=false`;
  assets=`0`.

Tasks 412-423D prepared and stabilized the release. Task 424 published the
immutable source without a repository commit. Task 425 is later
documentation-only cleanup and is not the v0.32.0 release source.

The release theme is **Bounded Repository Structure Listing / Browse**.

### Added

- First-party `repo.list` for bounded, direct-child structural browsing of the
  host-selected active repository. It uses a fixed tracked Git inventory,
  synthesizes directory entries from visible tracked paths, sorts results
  deterministically by repository-relative path, bounds results, and returns
  sanitized closed errors. It performs structural listing only: no file
  content reads, arbitrary filesystem enumeration, or HostExplicit expansion.
- The closed result contains structural path and kind metadata only. It does
  not read file contents; tracked binary files are structural entries without
  content disclosure.
- Missing-ancestor handling and existing symlink/reparse ancestor
  classification were corrected and independently audited. Errors remain
  closed and sanitized.

### Authority and evidence

`repo.list` remains `ReadOnly` / `RepositoryObservation` / `Execute` and is
bound to the host-selected active repository. It adds no mutation authority,
PermissionLevel, authority category, or HostExplicit route; HostExplicit
remains exactly 11.

Task 374 defines the v0.32 scope. Tasks 375-382 record the contract,
implementation, audit, correction, and Windows certification history. Task 408
found a missing binary evidence case; Task 409 closed deterministic binary
no-content evidence at `580880f2619de4ed1e3810aec31b2ad27d6d91c3`. Task 410
recorded four Desktop test failures from Task 409, then could not reproduce
them: all four focused tests passed 3/3 and two consecutive full Desktop
suites passed. Task 411 closed the committed-harness Windows binary evidence
at certification HEAD `64467c9d8dfbb0cb465a8be72e3c312bc34c876a`; its later
documentation closeout is `ad9e755ac6d973fb39c58ad88e905c682e6fe9ba`.

Staged diff observation preserves its aggregate 15-second deadline while
clamping Git-layout probes and HEAD child execution to the aggregate remaining
time. The existing five-second per-child ceilings and fail-closed timeout
behavior remain unchanged. Tasks 412-422 record the initial intermittent
validation failure, policy clarification, bounded implementation correction,
and separate validation evidence; the original Task 412 run did not pass.

Tracked binary visibility is evidenced as structural non-disclosure only; it
does not claim general arbitrary-byte semantics. Windows symlink live
certification and Codex model-inference certification remain nonclaims. The
Tauri permission-generation discrepancy (47 invoke handlers and 44 generated
entries) remains unrelated existing debt.

### Explicit nonclaims

- No arbitrary filesystem enumeration, untracked/ignored discovery, or
  recursive browse.
- No content-reading capability, mutation authority, or HostExplicit
  expansion.
- No multiple-active repository support or model-selected repository.
- No general Windows symlink certification, Codex inference certification,
  or arbitrary-byte semantics beyond structural non-disclosure.

The immutable v0.32.0 source is released as recorded above. Publication does
not expand the capability, authority, evidence, or claims described here.

## v0.31.0 - released (2026-09-20)

The release theme is **Bounded Repository Discovery / Search**.

### Immutable publication record

- Release source: `c4d21deac56b034c2df6c96060b29ae259bf536f`.
- Annotated tag: `v0.31.0`; annotation: `RAH v0.31.0`.
- Tag object: `732ee33ff06ef456a8fa0d7d1b9af6a246e01980`.
- Peeled target: `c4d21deac56b034c2df6c96060b29ae259bf536f`.
- Release-preparation CI: `35447671491` - PASS.
- Tag CI: `35481008480` - PASS.
- GitHub Release ID: `392273508`; name: `RAH v0.31.0`.
- Published: `2026-09-20T01:20:20Z`; `draft=false`; `prerelease=false`;
  assets=`0`.

Task 370 prepared the immutable source and stopped before publication. Task
371 published the annotated tag and GitHub Release without a repository
commit. Task 372 is a later documentation-only cleanup and is **not** the
v0.31.0 release source.

### Added

- First-party `repo.search` with closed `path` and `text` modes for bounded
  tracked-file discovery in the selected active worktree.
- Fixed Git tracked inventory via `git --no-pager ls-files --cached
  --deduplicate -z --full-name --`; text mode reads current selected-worktree
  bytes and returns bounded repository-relative paths and line numbers.
- Literal, case-sensitive host-side matching with deterministic result and
  omission bounds, linked-worktree isolation, nested-repository rejection, and
  symlink/reparse non-traversal.

### Authority and audit record

`repo.search` remains `ReadOnly` / `RepositoryObservation` / repository-bound
with existing `Execute` permission. No new authority category or permission
level was added, and HostExplicit remains exactly 11; `repo.search` and
`repo.create-directory` remain ineligible.

Task 368 independently returned **PASS WITH NARROW HARDENING** and confirmed
the known parallel-test contention family was reproducible on the clean
pre-Task-367 baseline. Task 369 returned **PASS WITH EXPLICIT CODEX NONCLAIM**
after Windows live certification of the production and Generic Tool Bridge
paths. The v0.31 gate records the exact checkpoints and all preserved
nonclaims, including unavailable `codex-cli 0.149.0` model-selected inference
and unavailable Windows symlink live certification.

## v0.30.0 - released (2026-09-19)

The release theme is **Linked Git Worktree Repository Support**.

### Immutable publication record

- Immutable release source: `ea37a428a39ac4732335f563a9b31c0c86f3150a`.
- Annotated tag: `v0.30.0`; annotation: `RAH v0.30.0`.
- Tag object: `254e9ec110cc45fc677b543874728277ae67bef1`.
- Peeled target: `ea37a428a39ac4732335f563a9b31c0c86f3150a`.
- Release-preparation CI: `35425103860` - PASS.
- Tag CI: `35425678366` - PASS.
- GitHub Release ID: `391934835`; name: `RAH v0.30.0`.
- Published: `2026-09-19T06:08:26Z`; `draft=false`; `prerelease=false`;
  assets=`0`.

Task 364 is a later documentation-only descendant and is **not** the
immutable v0.30.0 release source. The v0.30.0 tag remains attached to the
release source above.

### Added

- Explicit admission of ordinary main worktrees and validated, already-registered
  linked Git worktrees as distinct process-local repository members.
- Absolute/default and supported relative linked relationship spellings, with
  closed private Git, common Git, backlink, registration, filesystem, and
  read-only semantic validation.
- Selected-worktree observation, content authoring, Stage/Unstage index
  isolation, selected-HEAD reviewed Commit, and selected-HEAD local branch
  creation across admitted linked members.
- Windows host-driven certification of the production linked-worktree paths,
  including external identity staleness and detached observation behavior.

### Authority and currentness

ADR 0029 is an Accepted **NARROW IDENTITY EXTENSION** under ADR 0027. It is
not a new authority category. HostExplicit remains exactly 11; no permission,
Tool schema, Stage/Unstage authority, Commit authority, generic Git/filesystem
authority, provider authority, model routing authority, or persistence/schema
authority was added.

Main, linked A, and linked B may share one common Git directory while remaining
distinct members. Same common Git state does not merge authority. The one-active
repository rule remains unchanged: inactive members carry no executable
composition and there is no model/provider worktree selector.

Currentness is operation-specific. An unrelated sibling Commit or ordinary
shared object creation does not globally stale a selected review, while movement
of the selected branch ref makes that review stale with zero native Commit
spawn and no retry or rebinding. Stage/Unstage target only the selected
worktree's private index.

### Closed scope and lifecycle

Invalid, arbitrary, copied, fabricated, or malformed gitfiles; stale/prunable
registrations; submodules; `--separate-git-dir`; bare repositories; unsupported
nested relationships; and symlink/reparse-mediated identity are rejected
fail-closed. RAH recognizes an existing valid worktree identity but does not
create, remove, prune, repair, move, lock, or unlock Git worktrees. External
move/removal or relation mutation makes retained identity stale and requires
fresh explicit admission.

### Certification record

Task 361 verdict: **PASS WITH EXPLICIT NONCLAIMS**. Marker:
`RAH_V030_LINKED_WORKTREE_LIVE_OK`. Production behavior source:
`a684405ecd0143093fba669b9b52e84bc32f7590`; deterministic/audit tree:
`3771a52227d2ef85944e971a2a0dc9c0e5d5856f` (CI `35352376409`, PASS);
Windows-certified source:
`93a522c2a42b438d539d296ba92cbb9dd1eea297` (CI `35420903926`, PASS);
certification docs head:
`1ed6380c6b5ebb6e57baa9c253a0923289c05470` (CI `35421216647`, PASS).

The certified environment was Windows 11 IoT Enterprise LTSC build 26100,
x64, Rust/Cargo 1.98.1, and Git 2.55.0.windows.5. The project lifecycle
baseline remains `codex-cli 0.149.0`; it was unavailable on the Task 361 host,
ambient `0.155.1` was not substituted, and Codex inference was not executed.

### Limitations and nonclaims

GUI automation, model-selected worktree routing, Codex inference, Linux/macOS
live certification, cross-platform live parity, OS sandboxing, network
isolation, race-free TOCTOU, a machine-wide external-Git lock,
rollback/replay/compensation, submodule support, `--separate-git-dir` support,
worktree lifecycle authority, multiple active repositories, and persistent
executable membership are not claimed. Existing broader RAH security limits
remain unchanged.

## v0.29.0 — released (2026-09-18)

The release theme is **Explicit Active Repository Close**.

### Immutable publication record

- Release source: `1568707e25705ca367a21fe096cfcbf16abfb79e`.
- Annotated tag: `v0.29.0` (`RAH v0.29.0`).
- Tag object: `12be50d8ba531e83f13a9aeb313afcd6032fa89e`.
- Peeled target: `1568707e25705ca367a21fe096cfcbf16abfb79e`.
- Release-preparation CI: `35294662087` — PASS.
- Tag CI: `35296021014` — PASS.
- GitHub Release ID: `391159613`; name: `RAH v0.29.0`.
- Published: `2026-09-18T01:39:37Z`; `draft=false`, `prerelease=false`;
  assets: 0.

Task 355 is a later documentation-only descendant and is NOT the immutable
v0.29.0 release source.

### Added

- An explicit human Desktop Close Repository workflow that withdraws the
  currently active repository into a valid zero-active state.
- The closed repository remains admitted as an inactive member. A later use
  requires a separate explicit activation.
- Confirmation captures the expected active member and repository generation.
  A stale confirmation for A is rejected if B becomes active; it never closes
  whichever repository happens to be active at Confirm time.
- Close withdraws safe prepared repository-bound workflow, Commit review and
  authorization state. Started or uncertain owners remain owned and block
  Close.
- Checked repository, observation, and action sequence advancement prevents
  generation wraparound and stale selector reuse.
- Windows host-driven live certification of the production Desktop backend.

### Authority and effects

Authority delta: **NONE**. Close withdraws existing active repository
authority under ADR 0027. It is not a Tool, HostExplicit action, Codex dynamic
Tool, MCP Tool, Process Plugin Tool, Git authority, or filesystem authority.
HostExplicit remains exactly 11. ADR 0027 and ADR 0028 remain authoritative and
unchanged; no v0.29 ADR was required. Zero-active state was already allowed by
ADR 0027, and remembered workspaces remain descriptive under ADR 0028.

Close requires the existing runtime/provider lifecycle to be fully
disconnected. It does not implicitly Disconnect. A connected-runtime Close is
rejected; explicit Disconnect must finish first. Close is not cancellation,
rollback, replay, or compensation. It performs no worktree write, create,
delete, rename, Stage, Unstage, Commit, branch/ref/history mutation, `.git`
mutation, network Git, or remembered-catalog write. Fixture setup outside a
measured Close interval is not attributed to Close.

### Windows certification record

Task 352 verdict: **PASS WITH EXPLICIT NONCLAIMS**. Certified source:
`c9cff2afac40f9d518310da38ba4c9e158af1641`; source exact-head CI
`35212728847` passed. Certification docs head:
`b51d85fcfa7fcca4ee81b4bcd73af391ebc8ad2e`; docs exact-head CI
`35214134363` passed. The marker was
`RAH_V029_ACTIVE_REPOSITORY_CLOSE_LIVE_OK`.

The host-driven run used Windows 10 Professional build 19045 x64, Rust/Cargo
1.96.0, Git 2.54.0.windows.1, and certified `codex-cli 0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
The Desktop package was 0.28.0, workspace count 13 at 0.28.0, edition 2024,
and HostExplicit count 11. The certified Codex runtime was used only for
connection lifecycle validation; no inference was run.

Measured Close intervals had zero Git mutation. Model requests, model Tool
requests, MCP activations, Process Plugin activations, network Git operations,
automatic commits, automatic activations, automatic provider reconnects, and
model-selected Close actions were all zero.

### Limitations and nonclaims

- GUI automation, a live model-turn/chat-owner busy case, a live HostRunning
  case, and full live HostExplicit Prepare/Confirm were **NOT EXECUTED**.
  The HostPrepared live check used a test-only reservation seam.
- Model-selected dynamic Tool dispatch is not established by this gate.
- No OS sandbox, network isolation, rollback/replay/compensation, race-free
  TOCTOU, cross-platform live parity, linked-worktree Close semantics, or
  persistent executable repository membership is claimed.
- Close does not implicitly Disconnect; restart does not automatically
  Resume a transcript.
- Historical certification-harness cleanup limitation: seven storage-only
  roots from earlier non-certifying attempts remained after inspection. Their
  repository fixture trees were absent, their owning test PIDs were no longer
  running, and execution policy rejected deletion of those exact roots. No
  alternate deletion was attempted. They were not live repository authority
  or running-process leaks.

## v0.28.0 — released (2026-09-16)

RAH v0.28.0 was published by Task 345. The release theme is **Explicit
Inactive Repository Member Removal**.

### Immutable release record

- Release source: `616af196b03c6473b93984d429e7dd0825edc0b7`.
- Annotated tag: `v0.28.0`.
- Tag annotation: `RAH v0.28.0`.
- Tag object: `2927c297e47cb04ea629c761b74afd43d2f685f1`.
- Peeled target: `616af196b03c6473b93984d429e7dd0825edc0b7`.
- Release-preparation CI: `35095044576` — PASS.
- Tag CI: `35095965370` — PASS.
- GitHub Release ID: `389903124`.
- Release name: `RAH v0.28.0`.
- Published: `2026-09-16T12:30:07Z` (`draft=false`, `prerelease=false`).
- Assets: 0.

Task 346 is a later documentation-only descendant and is not the immutable
v0.28.0 release source. The v0.27.0 immutable release identity is unchanged.

### Added

- Explicit host/Desktop removal of one admitted inactive repository member.
- Active-member removal rejection with no lifecycle or membership effect.
- Membership-generation advancement only after successful membership mutation.
- Target-bound busy fail-closed handling, while unrelated active-repository
  lifecycle state does not globally block removal of an inactive target.
- Preservation of the active repository's lifecycle, authority, provider,
  conversation, Commit, and Stage/Unstage state during inactive removal.
- Process-local stale-selector invalidation and fresh member identity on later
  re-admission.
- Desktop confirmation that states repository files and remembered workspace
  entries remain, plus the exact command-specific Tauri permission.
- Windows host-driven certification of the production backend path.

### Authority and limitations

Authority delta: **NONE**. HostExplicit remains exactly 11; inactive member
removal is host/Desktop lifecycle management, not a Tool, filesystem authority,
Git authority, model route, MCP Tool, Process Plugin Tool, or new capability.
Removal does not delete repository files, mutate `.git`, the index, HEAD,
refs/history, or the remembered workspace catalog. Started or uncertain effects
are not rolled back, replayed, cancelled, or compensated.

The certification is **PASS WITH EXPLICIT NONCLAIMS**. GUI automation was not
executed. Model-selected dynamic Tool dispatch, OS sandboxing, network
isolation, race-free TOCTOU, cross-platform live parity, cross-process
membership, and linked-worktree removal semantics are not claimed.

## v0.27.0 — released (2026-09-16)

RAH v0.27.0 was successfully published by Task 337. The immutable release
source is `1297787df07c725e41deb758cc7ecab5593b28d8`. Task 338 is a later
documentation-only descendant and is not the immutable v0.27.0 release source.
RAH v0.26.0 is the prior immutable published release.

### Immutable release record

- Release source: `1297787df07c725e41deb758cc7ecab5593b28d8`.
- Annotated tag: `v0.27.0`.
- Tag object: `587bb55747ac1c9cdf1c02bc56e7dff244ede93b`.
- Peeled target: `1297787df07c725e41deb758cc7ecab5593b28d8`.
- Release-preparation CI: `35061634078` — PASS.
- Tag CI: `35063921592` — PASS.
- GitHub Release ID: `389683736`.
- Release name: `RAH v0.27.0`.
- Published: `2026-09-16T06:31:29Z` (`draft=false`, `prerelease=false`).
- Assets: 0.

The release theme is **Durable Remembered Workspaces with Explicit Fresh
Re-Admission**. RAH Desktop may remember bounded descriptive repository
candidates across restart, while restart restores zero executable repository
authority. A remembered candidate requires explicit human fresh admission,
current repository validation, a fresh process-local `RepositoryMemberId`, and
separate explicit activation before active-only repository authority is
composed.

### Added

- A dedicated `RememberedWorkspaceStore` and closed JSON schema v1 in
  `remembered-workspace.json` for a bounded descriptive candidate catalog.
- Explicit catalog add, update, delete, and reorder actions, with explicit
  privacy-controlled location reveal and no generic path exposure.
- Inert startup catalog loading and explicit fresh re-admission integrated with
  the existing ADR 0027 membership and active-only authority pipeline.

### Verified

- Windows host-driven live certification covering restart isolation, fresh
  member identity, admission/activation separation, deletion isolation,
  current-facts rejection, corrupt/future catalog behavior, privacy, zero
  startup candidate-path access, A/B Git integrity, zero model requests and
  Tool requests, zero MCP/Process Plugin activations, zero automatic commits,
  zero network Git, and bounded cleanup.
- The certified production source was
  `b90e63dd138732d616582d935b1fa45b842fcfe8`, with Task 335 verdict
  `PASS WITH EXPLICIT NONCLAIMS` and marker
  `RAH_V027_REMEMBERED_WORKSPACE_LIVE_OK`.

### Security and limitations

- Remembered data is descriptive only. Persisted paths are location hints, not
  repository identity or authority; automatic re-admission and activation are
  forbidden; there is one active repository maximum and no union ToolRegistry.
- The store uses a closed schema, bounded catalog, same-directory atomic
  replacement, staged reread and exact validation, Windows ancestor/final-file
  reparse checks, persist-before-publish ordering, and bounded coordination.
- No encryption, race-free TOCTOU protection, network isolation, or automatic
  recovery/replay guarantee after uncertain native effects is claimed.
- `MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.`
- GUI automation was not certified; a real junction/reparse live fixture and
  true simultaneous two-process live mutation were not executed. No network
  isolation, race-free TOCTOU guarantee, or automatic recovery/replay guarantee
  after uncertain native effects is claimed.

## v0.26.0 — released (2026-09-15)

RAH v0.26.0 is the current immutable published release. RAH v0.25.0 is the
prior immutable published release. Task 325 prepared the immutable source;
Task 326 published the annotated tag and GitHub Release without a repository
commit; Task 327 is a later documentation-only cleanup and is not the v0.26.0
release source.

### Immutable release record

- Release source: `8b22b18739a54b752ce11e791fe380e522c10049`.
- Annotated tag: `v0.26.0`.
- Tag object: `f60ee2465c1819b59b5671f20387b74734ace019`.
- Peeled target: `8b22b18739a54b752ce11e791fe380e522c10049`.
- Release-preparation CI: `34965819523` — PASS.
- Tag CI: `34966821012` — PASS.
- GitHub Release: `389111343` — `RAH v0.26.0`.
- Published: `2026-09-15T12:07:53Z` (`draft=false`, `prerelease=false`).
- Assets: 0.

This release theme is **Explicit Multi-Repository Membership and Active-Only
Repository Switching**. RAH Desktop can explicitly admit multiple repositories
into one process-local membership set while executable repository authority
remains singular. A human/host explicitly selects the one active repository.
Only that repository receives `DesktopRepository`, repository ToolRegistry and
composition, workflow actions, Stage/Unstage authority, Commit
authorization/currentness, HostExplicit repository-bound currentness, and
conversation repository context. Inactive members remain inert. Switching
A→B→A builds fresh active-only authority and invalidates stale
repository-bound state.

The v0.26 milestone records Task 323 **READY**, Task 322 Recommendation A to
stop feature expansion, Task 324-B's host-driven certification disposition,
and Task 324-C-R4 **PASS — WINDOWS HOST-DRIVEN MULTI-REPOSITORY
AUTHORITY/EFFECT CERTIFICATION PASSED**. The live certification source was
`e59319166dc0776458c9c0bee61f4c73d9d6399c`, and the final certification
documentation head was `21bc151efab969dcd725842459c049f2d6dd74c8`.

The host-driven success marker is
`RAH_V026_HOST_DRIVEN_MULTI_REPOSITORY_LIVE_OK`.

The approved live evidence certifies host-owned authority/effects and the real
Codex process lifecycle. It does not claim provider-specific multi-repository
certification: Task 324-C-R4 ran with MCP 0 and Process Plugin 0.

`MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.`
Task 324-A showed real Codex/model turns completed but selected no RAH dynamic
Tool. Task 324-B therefore followed established RAH policy that model
selection is not host authorization. This is not a RAH authority failure and
not a model Tool PASS.

The exact production HostExplicit set remains 11 names:

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
repo.rename-file
```

`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools,
fixtures/diagnostics, and unknown/provider-defined Tools remain ineligible.
Stage/Unstage remain separate host index authority, and Commit remains separate
repository-bound reviewed authority.

## v0.25.0 — released (2026-09-12)

RAH v0.25.0 is the current immutable published release. v0.24.0 is the prior
immutable published release. This release records Task 304's **Verdict A -
READY FOR RELEASE PREPARATION** at baseline
`b3532f52b79be9575f3f9d8e818efa2096da4e12`.

### Immutable release record

- Immutable release source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.
- Annotated tag: `v0.25.0`.
- Tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`.
- Peeled target: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.
- Release-preparation CI: `34662233490` PASS.
- Tag CI: `34662635116` PASS.
- GitHub Release: `387406579` — `RAH v0.25.0`.
- Published: `2026-09-12T00:47:29Z`.

Task 305 prepared the immutable release source and stopped before
publication. Task 306 later created the annotated tag and GitHub Release with
no repository commit. Task 307 is documentation-only cleanup and is not the
v0.25.0 release source.

The release theme is **HostExplicit Reviewed File Rename/Move
(`repo.rename-file`)**. ADR 0018 remains the ordinary rename/move authority;
ADR 0021 remains the generic HostExplicit coordinator, currentness, ticket,
and D2 boundary; and ADR 0026 remains the capability-specific reviewed
rename/move boundary. The reviewed route uses typed human
`{source_path, destination_path}`, zero-effect Prepare, a complete bounded
review, an opaque single-use ticket, ticket-only Confirm, currentness, exact
`ToolDefinition`, permission, and D2 checks, reviewed Commit authorization
invalidation before Started, exactly one authorized Tool dispatch through the
current ToolRegistry, the existing `repo.rename-file`, independent reviewed
post-effect proof, and status-only activity.

The exact production HostExplicit eligible set is 11 Tools:

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
repo.rename-file
```

`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools,
fixture/diagnostic Tools, and unknown/provider-defined Tools remain ineligible.

The ordinary public input remains exactly `source_path`, `destination_path`,
`expected_source_file_sha256`, and `expected_source_file_byte_length`, with
`PermissionLevel::Execute` and statuses `renamed_verified`, `known_no_effect`,
`invalid_input`, `precondition_failed`, and `uncertain`. The native effect is
at most one no-replace rename attempt. There is no retry, replay, reverse
rename, rollback, compensation, copy-delete, `git mv`, or shell path.

The Task 303 correction is part of the final v0.25 security foundation:
Windows Git destination collision proof rejects filesystem-equivalent tracked
and index paths even when spelling differs. Bounded HEAD tree discovery,
bounded case-insensitive index discovery, strict Git candidate parsing, and
host-owned existing filesystem path-equivalence comparison cover tracked-but-
missing HEAD, index-only, intent-to-add, and conflict/unmerged collision
regressions. Rejection occurs before the native effect with zero native
attempts.

The reviewed subset remains deliberately narrower than the ordinary Tool. It
requires one existing clean HEAD-tracked regular source file in the same
repository, source at most 65,536 raw bytes, strict UTF-8, no NUL, supported
mode, link count 1, exact HEAD/index/worktree equality, an existing destination
parent, and destination absence from the worktree, HEAD, and every index stage.
Ignored destinations, Windows aliases/case-equivalent collisions,
cross-repository or unsupported cross-volume movement, overwrite, and parent
creation are rejected. The complete review is not truncated. The expected
effect is one unstaged worktree rename/move, with no content rewrite, Stage,
Unstage, Commit, branch/ref/history mutation, directory creation, import or
reference rewriting, or automatic retry/replay/rollback/compensation.

Task 303's corrected Windows host-driven certification is carried forward from
production commit `1aa4d3277bd7ba24dec1caa3d52cc57c2aa2584a`; later validation
documentation is at `b2e170cc6f8226891f572dce6ce453abba836aaa`. On Windows 10
Professional 10.0.19045 build 19045 x64, Rust/Cargo 1.96.0, Git
2.54.0.windows.1, and certified `codex-cli 0.149.0` with executable SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, Prepare
was Tool/native `0/0`, Confirm was `1/1`, final proof was `ReviewedSuccess`,
terminal status was `renamed_verified`, HEAD/index/refs were preserved, Commit
authorization was invalidated after Started/effect, model/MCP/Process Plugin
counts were zero, and Codex cleanup was reaped. Marker:
`RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.

This is connected-current host-driven evidence only; it does not claim
model-selected rename certification or Linux/macOS live parity.

## v0.24.0 — released (2026-09-10)

RAH v0.24.0 is the current immutable published release. v0.23.0 is the prior
immutable published release. This release is based on Task 286's accepted
verdict:

> RAH v0.24 REVIEWED DELETION MILESTONE READY FOR RELEASE PREPARATION

The preparation baseline is `4197d90cd04493b562db7ae9be315074f9320433`.
The workspace version moves from `0.23.0` to `0.24.0` across 13 packages;
all remain on Rust edition 2024. No dependency, public API, authority, or
production behavior change is part of this release preparation.

### Immutable release record

- Immutable release source: `2d4cf0e4d89a461f54250ff81ad9808929d854f0`.
- Annotated tag: `v0.24.0`.
- Tag object: `5708cfd0078ba1593e86077bf88c0dddaa03c1bf`.
- Peeled target: `2d4cf0e4d89a461f54250ff81ad9808929d854f0`.
- Release-preparation CI: `34456482581` PASS.
- Tag CI: `34457322318` PASS.
- GitHub Release: `386128676` — `RAH v0.24.0`.
- Published: `2026-09-10T08:53:24Z`.

Task 287 completed release preparation at the immutable source and stopped
before publication. Task 288 performed the later immutable publication with no
commit. Task 289 is a documentation-only descendant cleanup and is not the
v0.24.0 release source.

### Release theme

**HostExplicit Reviewed File Deletion (`repo.delete-file`)** is the reviewed
human capability added by the accepted v0.24 milestone. Its route is:

```text
typed human {path}
 -> zero-effect deletion Prepare
 -> complete bounded backend-derived destructive review
 -> opaque process-local single-use ticket
 -> ticket-only Confirm
 -> connected-current/currentness checks
 -> retained deletion-preparer revalidation
 -> exact ToolDefinition / permission membership
 -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.delete-file
 -> ADR 0017 RepositoryFileDeletionPolicy
 -> strict five-status parsing
 -> independent reviewed-route effect proof
 -> status-only terminal HostActivity
 -> descriptive repository refresh
```

ADR 0017 remains the sole underlying repository file-deletion mutation
authority. ADR 0021 remains the generic HostExplicit
coordinator/currentness/ticket/D2/provenance boundary. ADR 0025 remains the
capability-specific reviewed human deletion HostExplicit boundary. Human
confirmation, model output, frontend state, `PermissionLevel::Execute`, Tool
presence, provider metadata, Trusted Profile metadata, MCP, and Process Plugin
metadata do not create deletion authority.

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
There is no wildcard, prefix, effect-category, permission-derived, or
provider-derived admission.

The closed human request is `{path}`. Its bounds are a canonical serialized
request of at most 8192 bytes, a logical path of 1..=1024 UTF-8 bytes, a
reviewed source of 0..=65536 raw bytes with strict UTF-8 and no NUL, a complete
serialized review of at most 262144 bytes, and retained private preparation of
at most 524288 bytes. The complete escaped source is the review surface;
truncation is not permitted. The ordinary ADR 0017 Tool remains broader: its
input remains `path`, `expected_file_sha256`, and
`expected_file_byte_length`, with a 1 MiB ordinary-file bound and binary
ordinary deletion still allowed.

Prepare has zero Tool/native effects. Confirm uses the retained preparation,
currentness, exact definition/permission membership, and D2 before Started,
then reaches the current registry through `authorized_tool_dispatch`. A
verified result means exactly one reviewed worktree file is absent and one
unstaged Git deletion is present. It does not Stage, Unstage, Commit, mutate
the index, HEAD, branch, refs, or history; it does not rename, move, restore,
clean up, or recursively delete.

The exact deletion statuses remain `deleted_verified`, `known_no_effect`,
`invalid_input`, `precondition_failed`, and `uncertain`. `deleted_verified`
requires valid Tool output plus independent confirmed-absence proof.
`known_no_effect` requires valid Tool output plus independent exact-original
preimage proof. Malformed/contradictory output and post-Started failures remain
`uncertain` when safe proof is unavailable. The ticket is opaque, process-local,
in-memory, capability-specific, single-use, nonpersistent, nonresumable, and
currentness-bound; elapsed time `< 300s` is valid and `>= 300s` is expired.
The ticket ID is distinct from the non-authority activity ID, and Confirm and
Cancel are ticket-only. Generic activity is status-only and excludes the
source-bearing review and private authority values. Effectful Confirm
invalidates reviewed Commit authorization immediately before Started; no
automatic Commit occurs.

Windows semantics are one native `DeleteFileW` attempt followed by one
immediate reviewed proof pass: no polling, sleep/recheck loop, second delete,
retry, replay, cleanup, restore, or recovery. `DeleteFileW` success is the
effect commit point, not proof of final absence. Possible effects remain
uncertain. No race-free TOCTOU, OS sandboxing, network isolation, or rollback
claim is made; process supervision is not an OS sandbox.

### Evidence and limitations

Task 285's accepted Windows evidence is carried forward without rerunning the
destructive test. It used Windows 10 Professional `10.0.19045` x64, Rust/Cargo
`1.96.0`, Git `2.54.0.windows.1`, Codex `0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning. The fixture was
`src/rah-hostexplicit-live-delete.txt`, 80 bytes, SHA-256
`cdb8ee496bafc4f41bfdac75d37132cb6c1e561703c46b45ede3bfd21396d9fe`.
Prepare was `0 / 0` Tool/native; Confirm was `1 / 1`; the result was
`deleted_verified`; the HostActivity sequence was `prepared`, `started`,
`tool_completed`; the marker was `RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_OK`.
Index, HEAD, branch, refs, and unrelated staged state were preserved, Commit
authorization was invalidated before Started, duplicate and activity-ID
operations were rejected, model/MCP/Process Plugin counts were zero, and
owned Codex cleanup passed.

The live-certified code/test SHA was `e6cb63436541fe653ba0ea4165466aaa822bc206`;
the final Task 285 docs evidence SHA was
`3a465d3b5010f86e606e3e3a0281d20774aacbc8`. Exact CI identities are retained
in the v0.24 release gate. The evidence is a connected-current human/host
success path, not model-selected deletion, Linux/macOS parity, or all
Windows failure modes. v0.24 does not claim generic filesystem deletion,
recursive/directory/wildcard deletion, untracked cleanup, rename/move,
HostExplicit directory creation or commit, automatic Stage/Unstage/Commit,
backup/restore/Trash semantics, retry/replay/rollback/compensation, model or
provider deletion, generic shell/process authority, or broader platform
isolation. Timeout, cancellation, disconnect, crash, or lost response after a
possible effect is not rollback.

## v0.23.0 — released (2026-09-09)

RAH v0.23.0 is released and is the current immutable published release.
v0.22.0 is the prior immutable published release. The workspace version is
`0.23.0` across 13 packages, all edition 2024.

### Release record

- Immutable release source: `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Annotated tag: `v0.23.0`.
- Tag object: `5a27d84c269a0d57a8a6ad5f0fca89c06379e711`.
- Peeled target: `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Release-preparation exact-head CI: `34326184721` PASS.
- Tag CI: `34330434792` PASS.
- GitHub Release: `385352229` — `RAH v0.23.0`.
- Published: `2026-09-09T08:43:12Z`.

### Historical release-preparation record

During Task 276, the v0.23 release gate was **PREPARED / NOT YET PUBLISHED**.
Task 275 authorized preparation with verdict **A — v0.23 NEW-FILE HOSTEXPLICIT
MILESTONE COMPLETE — RELEASE PREPARATION MAY BEGIN**. The preparation baseline
was `533b0769618d25c1b9673a27c3e21af7a48809ca`. Task 276 itself deliberately
created no `v0.23.0` tag, GitHub Release, artifact, or other publication.
Publication was completed later in Task 277. Task 278 records this state in a
later documentation-only descendant; it is not the v0.23.0 release source.

### Added

- **HostExplicit Reviewed New-File Authoring (`repo.create-file`)** through
  the existing capability:

  ```text
  typed human {path, content}
   -> zero-effect shared Prepare
   -> complete bounded backend-derived review
   -> opaque single-use ticket
   -> ticket-only Confirm
   -> connected-current / exact-definition / permission / preparer checks
   -> shared creation revalidation
   -> D2
   -> reviewed Commit authorization invalidation
   -> HostExplicit Started
   -> authorized_tool_dispatch
   -> current ToolRegistry
   -> existing repo.create-file
   -> ADR 0013 RepositoryFileCreationPolicy
   -> strict result classification
   -> descriptive repository refresh
  ```

- ADR 0013 remains the sole underlying file-creation mutation authority. ADR
  0024 owns only this capability-specific reviewed HostExplicit route. ADR
  0021 remains the generic HostExplicit coordinator, currentness, D2, ticket,
  lifecycle, and provenance boundary.
- The human request is closed to exactly `{path: String, content: String}`;
  unknown fields fail closed. It is exactly one file, with a 1..=1024 UTF-8
  byte path, 0..=262144 UTF-8 byte content, NUL rejection, canonical request
  size at most 327680 bytes, and complete serialized review at most 262144
  bytes. Empty content is valid. Content bytes are exact: no BOM or newline
  transformation, Unicode normalization, templating, append, overwrite, or
  automatic final newline.
- The reviewed route requires an existing safe parent, target absence from the
  worktree, HEAD, every index stage including intent-to-add, and Git admission.
  It rejects ignored targets, submodules, unsupported sparse-checkout state,
  unsafe Windows reserved/device/ADS/UNC/verbatim names, reviewed trailing-dot
  or trailing-space components, and symlink, junction, or reparse traversal.
  It creates no parent directory. Native exclusive acquisition is the commit
  point (`O_CREAT | O_EXCL` intent on Unix; `CREATE_NEW` / `FILE_CREATE` intent
  on Windows). Complete writing is not an atomic all-or-nothing transaction.

### Security

- The exact nine eligible HostExplicit Tools are `fs.read`, `repo.file-info`,
  `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`,
  `repo.patch`, `repo.edit-files`, and `repo.create-file`. `repo.delete-file`,
  `repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools,
  Process Plugin Tools, fixture Tools, and unknown Tools remain ineligible.
  There is no wildcard, category, or provider-metadata route.
- The exact ADR 0013 result classes remain `ok`, `invalid_target`,
  `precondition_failed`, `create_failed_known`, `write_failed_known`, and
  `uncertain`. `create_failed_known` requires bounded post-observation proving
  no RAH creation effect; `write_failed_known` means exclusive creation
  succeeded and an attributable empty or partial file may remain; `uncertain`
  preserves unknown absent, empty, partial, complete, or replaced state.
  Malformed or contradictory results are not upgraded to success.
- Prepare performs zero Tool executions and zero native creation attempts. The
  ticket is opaque, process-local, in-memory, capability-specific,
  single-use, exact-preparation/currentness-bound, and valid for an inclusive
  five-minute TTL. It is not durable authority and has no persistence, resume,
  or replacement path. Confirm and Cancel receive only the ticket ID.
- Generic activity has a distinct non-authority activity ID, and
  `ticket_id != activity_id`. The activity ID is not derived from the ticket
  and cannot Confirm or Cancel. Generic activity, persistence, and logging
  exclude the ticket, complete source content or source sentinel, complete
  source-bearing review, raw ToolInput/ToolOutput, native paths and parent
  identities, and private object identities. Successful generic terminal
  output is status-only.
- Effectful creation invalidates stale repository-bound reviewed Commit
  authorization. Refresh is descriptive only. The workflow never retries,
  replays, deletes, rolls back, restores, compensates, stages, or commits.
  A partial file may remain after `write_failed_known`; an uncertain external
  effect may remain. Timeout, cancellation, or disconnect is not rollback.

### Validation and evidence

- Task 274 evidence is carried forward and is not rerun for release
  preparation. It was host/human initiated, not model-selected: Windows 10
  Professional `10.0.19045` x64; Codex `0.149.0`; Codex SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
  model `gpt-5.6-terra`; reasoning `medium`.
- The guarded command was the exact `scripts/codex-live-gate.ps1` command
  recorded in the v0.23 live-certification plan. It proved target
  `src/rah-hostexplicit-live-created.txt`, source length 79, source SHA-256
  `b61494980e795ac47bfc4598d9e30176a1821c0eca4e3e213503ee67b8237314`,
  Prepare `Tool 0 / native create 0`, Confirm `Tool 1 / native create 1`,
  result `ok`, `HostActivity tool_completed`, generation tuple `[1, 0, 0, 1]`,
  Stage/Commit `0 / 0`, model request and lifecycle counts all zero, MCP and
  Process Plugin counts zero, and marker
  `RAH_CREATE_FILE_HOSTEXPLICIT_LIVE_OK`. It also retained the fresh
  disposable repository, ordinary parent, exact bytes/hash/length, regular
  non-symlink/non-reparse target, unchanged index/HEAD/branch/refs and
  unrelated staged state, distinct ticket/activity IDs, privacy checks,
  duplicate/activity-ID rejection, Commit invalidation, and Idle coordinator.
  The optional live Cancel-before-start subcase was not rerun; deterministic
  cancellation evidence was accepted by Task 275.
- This evidence does not claim model-selected HostExplicit execution. It does
  not claim generic `fs.write`, shell/process authority, arbitrary filesystem
  writing, parent mkdir, overwrite, append, delete, rename, automatic Stage or
  Commit, HostExplicit `repo.commit`, MCP or Process Plugin HostExplicit,
  ticket persistence/resume, retry/replay, rollback/recovery/compensation,
  race-free TOCTOU, process supervision as OS sandboxing, network isolation,
  or Linux/macOS production live parity.

## v0.22.0 — released (2026-09-09)

RAH v0.22.0 is released and is the current immutable published release.
v0.21.0 is the prior immutable published release.

### Release record

- Immutable release source: `76895b4067c38167f3c41a3536f6616cffa8293a`.
- Annotated tag: `v0.22.0`.
- Tag object: `eb91039521eac190efc85ef57ee2a5c28ca4e9fe`.
- Tag CI: `34301369809` PASS.
- GitHub Release ID: `385176161` (`RAH v0.22.0`).
- Published: `2026-09-09T02:02:09Z`.

### Historical release-preparation record

- Task 264 verdict: **A — v0.22 MILESTONE COMPLETE — RELEASE PREPARATION MAY
  BEGIN**.
- Preparation baseline: `43dce0c505a39975b28f9a0be25aeec4ade1a88b`.
- Workspace version: `0.21.0` -> `0.22.0`; all 13 packages remain edition 2024.
- No tag, GitHub Release, artifact publication, or live certification rerun is
  part of this preparation.

### Added

- **HostExplicit Reviewed Multi-File Edit Authoring** through the existing
  `repo.edit-files` capability:

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

- ADR 0023's capability-specific reviewed HostExplicit workflow, reusing ADR
  0014's existing `RepositoryMultiFileMutationPolicy` and ADR 0021's general
  HostExplicit coordinator, currentness, and D2 boundaries.
- Typed Desktop review over 1–4 existing clean HEAD-tracked regular strict-
  UTF-8 files, with 1–16 exact literal replacements per target and at most 64
  replacements total. The host derives preimages, postimages, hashes, lengths,
  and canonical path order; caller order is not execution order.

### Security

- The exact HostExplicit allowlist is `fs.read`, `repo.file-info`,
  `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`,
  `repo.patch`, and `repo.edit-files`. `repo.create-file`,
  `repo.delete-file`, `repo.rename-file`, `repo.create-directory`,
  `repo.commit`, MCP Tools, Process Plugin Tools, and unknown Tools remain
  ineligible.
- Preparation is zero-effect and review is complete and backend-derived;
  `review_too_large` fails closed. Confirm/Cancel receive only a process-local,
  in-memory, single-use opaque ticket with an inclusive five-minute TTL,
  exact-change/currentness binding, and no persistence or resume.
- Generic HostExplicit activity uses a separate RAH-generated activity ID. It
  is not the ticket, is not derived from the ticket, cannot Confirm or Cancel,
  and the actual ticket and source-bearing review content are absent from
  generic activity. Task 263 Attempt 1 discovered the ticket-under-
  `invocationId` leak before Confirm with zero effect; Attempt 2 corrected it
  and passed on a fresh repository.
- ADR 0014's six result classes remain distinct: `ok`, `invalid_target`,
  `precondition_failed`, `failed_known_no_effect`, `partial_effect`, and
  `uncertain`. The operation is explicitly non-atomic: only a verified
  committed prefix is reported, uncertainty is preserved, and there is no
  retry, replay, prefix continuation, rollback, restore-preimage, transaction,
  automatic Stage, or automatic Commit.
- Model output is never authorization. Frontend input and presentation,
  permission classification, Trusted Profile composition, and provider
  metadata cannot create mutation authority or enable HostExplicit.

### Validation and evidence

- Task 262 provides deterministic fault, privacy, result, currentness,
  invalidation, and no-replay evidence. Task 263's final Attempt 2 provides
  the connected-current Windows production certification without a model
  prompt or model lifecycle.
- Task 263 evidence: Windows 11 IoT Enterprise LTSC x64; Codex `0.149.0`;
  SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
  model `gpt-5.6-terra`, reasoning `medium`; caller order `d.txt, b.txt,
  a.txt, c.txt`; backend/effect order `a.txt, b.txt, c.txt, d.txt`; Prepare
  `0` Tool/native attempts; Confirm `1` Tool and native attempts `1/1/1/1`;
  final generations `[1, 0, 0, 1]`; coordinator and chat Idle; model lifecycle,
  MCP, and Process Plugin counts all zero; marker
  `RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK`.
- During Task 265, the v0.22 release gate was `PREPARED / NOT YET PUBLISHED`.
  Unix deterministic or platform-gated testing is not Windows-equivalent live
  certification.

### Limitations and nonclaims

- `repo.edit-files` is non-atomic; `partial_effect` and `uncertain` may occur.
  There is no race-free TOCTOU, network isolation, OS sandbox, rollback,
  replay, or automatic recovery guarantee; process supervision is not OS
  sandboxing.
- No generic `fs.write`, `shell.exec`, structural HostExplicit authoring,
  HostExplicit `repo.commit`, MCP or Process Plugin HostExplicit, model-
  selected HostExplicit certification, authority-ticket persistence/resume,
  or automatic Stage/Commit is claimed.

## v0.21.0 — 2026-09-08

RAH v0.21.0 is released and is the current immutable published release.
v0.20.0 is the prior release.

### Release record

- Release commit: `7faa13a16425cc9d3bdb0dc540923eae8685ba90`.
- Annotated tag: `v0.21.0`.
- Tag object: `aa178f05d12d881745932800e8fc40a7431ce7b9`.
- Task 253 exact-head CI: `34198526278` PASS.
- Tag CI: `34199221802` PASS.
- GitHub Release ID: `384519493`.
- Published: `2026-09-08T07:27:35Z`.

### Added

- HostExplicit reviewed single-file `repo.patch` authoring workflow:
  `inspect -> typed human repo.patch Prepare -> exact bounded review ->
  ticket-only Confirm -> existing repo.patch -> inspect diff -> existing Stage /
  Unstage -> existing reviewed Commit`.
- Shared non-effectful `RepositoryPatchPreparer` and opaque revalidation for the
  host-derived canonical patch input.
- Exact bounded escaped review and ticket-only confirmation for the H1 fields
  `path`, `expectedOldText`, and `replacementText`.
- Frontend reviewed patch UX using typed fields and no generic Tool/JSON route.

### Security

- ADR 0022 adds the reviewed HostExplicit worktree-authoring boundary around
  the existing ADR 0012 `repo.patch` authority; ADR 0021 remains the general
  HostExplicit dispatch/currentness/D2 boundary.
- Host-derived SHA/length and canonical `ToolInput`, shared non-effectful
  preparation, exact R4 review, opaque process-local single-use five-minute
  tickets, strict pre-start revalidation/D2 ordering, and exactly-once mutation
  are retained.
- Confirm receives only a ticket ID; `authorized_tool_dispatch` reaches the
  existing `repo.patch` Tool after `Started`; strict result classification,
  source-review privacy, and conservative malformed/uncertain/post-start
  ToolError handling are required.
- No retry, replay, rollback, or restore-preimage is performed. Patch source
  text is absent from generic activity and persistence. Effectful patch start
  invalidates old reviewed-commit authorization; refresh may create a new
  `ReadyToAuthorize` review but never automatically authorizes it. Content
  mutation alone does not increment `repository_generation`.

### Validation

- Task 252 Verdict A: milestone complete; release preparation may begin.
- Deterministic workspace validation: 672 passed, 10 ignored, 0 failed.
- Task 251 final Windows connected-current HostExplicit reviewed `repo.patch`
  certification through the production backend, with Prepare `0 Tool / 0
  replacement`, Confirm `1 Tool / 1 replacement`, `ChangedVerified`, exact
  postimage, protected repository state unchanged, model lifecycle `0/0/0`,
  and MCP/Process Plugin providers `0`.

### Limitations

- No GUI mouse automation certification; no model-selected `repo.patch`
  certification; no Linux/macOS live certification.
- No other HostExplicit authoring Tool enablement and no external-provider
  HostExplicit.
- No generic filesystem write, shell, or generic process authority; no
  automatic Stage or Commit.
- No rollback guarantee, race-free TOCTOU guarantee, network isolation claim,
  or OS sandbox claim; process supervision is not OS sandboxing.
- No staged, binary, new-file, delete, rename, directory, or multi-file
  HostExplicit authoring, and no claim that a refreshed review is automatically
  authorized.

## v0.20.0 — 2026-09-07

At publication, RAH v0.20.0 was the current immutable published release.
v0.19.0 was the prior release.

### Release record

- Release commit: `0d2b4c46be5d879b70c660501f9b7236040a7a61`.
- Annotated tag: `v0.20.0`.
- Tag object: `bc3addb4f41914a97a91521eb01c4726ae3c2c73`.
- Task 242 exact-head CI: `34090581488` PASS.
- Tag-triggered CI: `34091553715` PASS.
- GitHub Release: `RAH v0.20.0` (numeric ID `383875194`).

### Added

- Explicit Desktop Host Tool invocation for a closed first-party Tool set.
- Exact six-Tool first-release HostExplicit eligibility:
  `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`,
  and `repo.create-branch`.
- Typed Host actions, branch Prepare/review/Confirm, and private
  `host_explicit` activity/provenance.

### Security

- ADR 0021 keeps host explicit dispatch, runtime/model dynamic dispatch, and
  capability authorization separate.
- HostExplicit reuses the shared D2 current-definition/permission gate and
  the Codex bridge now applies equivalent admission hardening.
- Connected-current composition, backend-owned eligibility, typed input,
  single-use branch tickets, and no generic ToolName/JSON route are retained.
- No replay, rollback, or authority amplification is introduced.

### Validation

- Task 239 deterministic evidence covers the six-Tool route and security
  boundaries.
- Task 240 certified Windows connected-current `repo.status` and
  `repo.create-branch` through the production HostExplicit backend path.
- The live branch effect was verified as `rah-host-explicit-live-18d2efa09d330900-2`
  at OID `e6b376c26b0d97e12c1be2c7981aecc32974e29c`.
- The live run recorded model lifecycle `runtime.start = 0`, `AgentRequest = 0`,
  `prompt = 0`, `ToolRequested = 0`, `ToolStarted = 0`, and `ToolFinished = 0`.

### Limitations

- `fs.read`, `repo.file-info`, `repo.diff`, and `repo.diff-staged` are
  deterministic-only HostExplicit release claims; they were not separately
  live certified.
- Real GUI mouse-click automation and Linux/macOS HostExplicit live behavior
  were not certified.
- Model-selected `repo.create-branch` remains not certified under Task 229;
  Task 207 remains unchanged.
- External provider Tools, `repo.commit`, and worktree-authoring HostExplicit
  routes remain deferred. Post-start HostExplicit cancellation is not offered.

## v0.19.0 — 2026-09-07

RAH v0.19.0 released for the Bounded Local Branch Creation at Captured
Attached HEAD milestone.

### Release record

- Release commit: `88463787d14e32a3f6692385161b59474bff1dea`.
- Annotated tag: `v0.19.0`.
- Tag object: `24fff2dbae77d6fbd281bf9f4ac34b855c116e3a`.
- GitHub Release: `RAH v0.19.0` (numeric ID `383770772`).
- Task 231 exact-head CI: `34040345951` PASS.
- Tag-triggered CI: `34071020347` PASS.

### Added

- `repo.create-branch`, backed by private host-owned branch authority.
- Exact captured-HEAD creation of one absent ordinary local branch ref.
- Explicit Desktop composition and the Effective Authority local-branch
  category.

### Security / authority

- `Execute` remains the outer gate only; the model supplies only a validated
  logical name.
- A fixed `update-ref` expected-absence CAS and Git-owned fixed reflog create
  the local ref without switching, tracking, or generic Git authority.
- Each call has one possible mutating attempt. Uncertain effects are not
  replayed, retried, rolled back, or compensated.
- Repository, executable, identity, hooks, configuration, and output bounds
  remain host-controlled and hardened.

### Validation and limitations

Windows host-driven Desktop repo.create-branch authority/effect path is certified. Model-selected repo.create-branch dispatch was not observed in two bounded Codex live attempts and is not certified.

- The fresh host-driven branch name and OID were internally asserted but were
  not printed on the successful output path; this is a nonblocking evidence-
  capture limitation and the effectful gate was not rerun.
- Linux live branch certification was not established.
- Task 207's prior external-provider model-selection limitation remains
  unchanged.

## v0.18.0 — 2026-09-06

RAH v0.18.0 released for the Inert Trusted Profile Persistence and Explicit
Restore milestone.

- Release commit: `bd0d237b8cda8f4cebdf56d2e3a792b5dd79ba2e`.
- Annotated tag: `v0.18.0` (tag object
  `e2ac5affc58984dd8ffad5a4d0b9f0de4780d8f0`).
- GitHub Release: [RAH v0.18.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.18.0)
  (numeric ID `383437699`).
- Task 219 exact-head CI: `34006233351` PASS.
- Tag-triggered CI: `34006623521` PASS.

### Added

- Desktop persists one host-selected Trusted Profile source path as inert local
  preference. Restart is remembered-not-restored: no selection, validation,
  composition, provider spawn, Tool advertisement, or authority restoration
  occurs implicitly.
- Explicit Restore fresh-loads and statically validates the current source
  without spawning. Connect/reconnect fresh-loads it again and remains the only
  provider activation boundary. Forget removes only the durable preference.
- Profile generation participates in currentness. The certified Codex 0.149.0
  baseline store has an explicit host-only repair workflow for legacy/invalid
  stores; it is not automatic repair, download, or migration.

### Validation and limitations

- Task 217 initially remained **INCONCLUSIVE** because the local baseline store
  was legacy v1. Task 217A repaired that host prerequisite; the resumed Task
  217 Windows lifecycle result is **PASS**. This is supporting baseline
  maintenance, not v0.18 product authority.
- Task 207 remains **INCONCLUSIVE / externally blocked for the model-selected
  external Tool execution sub-gate**. v0.18 makes no model-selected external
  Tool, Linux Desktop lifecycle, ambient-effect absence, OS-sandbox, network
  isolation, or rollback claim.

## v0.17.0 — 2026-09-05

Released as `RAH v0.17.0` for the Desktop Host-Selected Trusted Profile
External Provider Integration milestone.

### Added

- A Desktop host-selected, provider-only Trusted Profile overlay. Static
  profile selection is inert and non-spawning; local MCP stdio and Process
  Plugin providers activate only on explicit Connect or reconnect.
- A shared `rah-profile-composition` path that performs exact Tool-set/schema
  admission, preserves host-selected Trusted Profile permissions, merges
  admitted external Tools with the first-party Desktop Tool registry, and
  fails closed on duplicate public Tool names.
- Provider-owned lifecycle cleanup and sanitized Effective Authority external
  descriptors, with Configured, Effective, Advertised, and Current state kept
  distinct.

### Security / authority

- External effects use conservative lifecycle handling: ToolStarted revokes
  reviewed state, ToolFinished refreshes repository state, and uncertain
  effects are never replayed or presented as rolled back.
- The trusted host remains the authority boundary. Provider metadata,
  PermissionLevel, Tool advertisement, `repositoryBound=false`, and frontend
  presentation do not prove absence of ambient provider effects or grant
  generic shell, process, filesystem, or Git authority.

### Validation and limitation

- Hardened live-certification infrastructure fails closed unless it observes
  the complete hidden-nonce-backed Tool lifecycle and cleanup.
- Task 207 remains **INCONCLUSIVE / externally blocked for the model-selected
  external Tool execution sub-gate**. Windows provider selection, admission,
  composition, effective inventory, dynamic Tool advertisement, lifecycle
  ownership, and cleanup were verified, but actual model-selected MCP/Process
  Plugin execution was not established with the tested current ChatGPT-auth
  Codex model/runtime combinations. No RAH product defect was found and
  baseline migration was not justified.
- Real external-effect review invalidation and repository refresh remain
  deterministically verified but not live-certified through a real external
  provider model call. This is the accepted non-blocking Task 207C / Task 208
  limitation.

### Release record

- Release commit: `dc9ae03598f1ac48a571bb118ae9fd971250a2b7`.
- Annotated tag: `v0.17.0`.
- Tag object: `8bf0bbeb14f3d7e42f1f53f2ee5c6098d561a4aa`.
- Tag-triggered CI run `33959105671`: PASS.
- GitHub Release: published.

### Release limits

- v0.17 does not add network or Streamable HTTP MCP, provider
  download/install/update, PluginManager expansion, profile hot reload,
  active-provider auto-restore/persistence, generic shell/process,
  filesystem, or Git/branch/ref/history authority, OS sandboxing, network
  isolation, rollback, or Linux external-provider live certification.

## v0.16.0 — 2026-09-04

Released as `RAH v0.16.0` for the host-owned Effective Authority Review UX.
The immutable release commit is
`509a5ba8daefeabbf91da50853402a1661099668`; annotated tag `v0.16.0` has tag
object ID `c6ada41ed3c5edc677e392597c7d65dd5e9e69de` and peels to that commit.
The GitHub Release was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.16.0>.

### Added

- A read-only, backend-sanitized Desktop authority inventory showing public
  Tool names, host-derived effect/authority/permission/source classifications,
  and bounded unavailable-capability reasons.
- Clear configured, effective, and runtime-advertised state, with
  generation-aware Current, stale, and reconnect-required classification.
- Presentation of reviewed-commit state without exposing review authority or
  handles.

### Security / authority

- Backend sanitization is applied before frontend serialization/rendering;
  unknown schema and status values fail closed.
- Refresh Authority is observational and has zero Tool, lifecycle,
  repository, chat, or authority side effects.
- Tool visibility or advertisement is not unconditional execution
  authorization. Requests still pass ToolRegistry lookup, PermissionLevel,
  host policy, repository/workspace constraints, generation checks, and any
  one-shot reviewed-commit authorization.
- No new model-accessible authority or ADR was added.

### Validation

- Deterministic cross-layer security hardening and Windows live certification
  PASS using the certified `codex-cli 0.149.0` baseline.
- Linux live certification is not established.

### Release record

- Release commit: `509a5ba8daefeabbf91da50853402a1661099668`.
- Annotated tag: `v0.16.0`.
- Tag object: `c6ada41ed3c5edc677e392597c7d65dd5e9e69de`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.16.0>.
- Exact-head release-source CI run `33848123910` passed.
- Tag-triggered CI run `33848713329` succeeded.

## v0.15.0 — 2026-09-04

Released as `RAH v0.15.0` for the bounded repository directory-creation
milestone. The immutable release commit is
`6b66a357cacea4b1fcf21131cbc9e72fab90d59c`; annotated tag `v0.15.0` has tag
object ID `6ca031e66972b5e04dcade6766d6156a9c3e1a9b` and peels to that commit.
The GitHub Release was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.15.0>.

### Added

- Public `repo.create-directory` and separate host-owned
  `RepositoryDirectoryCreationPolicy` authority.
- Exactly one ordinary directory leaf at an explicit repository-relative path;
  the parent must already exist and the destination must be absent.

### Security / authority

- Directory creation is separate from file create/delete/rename and Execute;
  parent and destination validation, immediate pre-effect revalidation, and
  handle-/descriptor-relative native effects are preserved.
- One possible-effect attempt is allowed: no recursive mkdir, retry/replay,
  rollback/compensation, placeholder files, or Git mutation.

### Composition

- ToolRegistry composition and Generic Codex Tool Bridge integration preserve
  the public name, exact request/schema, structured result, and private alias
  boundary. Without host authority, the Tool is omitted.

### Desktop

- Host-owned selected-repository and repository-generation binding, stale
  runtime protection, repository refresh, reviewed-commit revocation, and
  Git-clean empty-directory handling are preserved.

### Validation

- Deterministic core, composition, Desktop, metadata, and release validation.
- Windows live `repo.create-directory` certification PASS using the certified
  `codex-cli 0.149.0` pair.
- Task 187 v0.15 milestone audit PASS.

### Limitations

- No recursive/tree creation, multiple directories per request, directory
  deletion or rename/move, overwrite, symlink/junction/reparse creation,
  implicit file or placeholder creation, generic `fs.mkdir`, shell/process,
  staging, commit, rollback, or replay authority.
- Windows is live-certified; Ubuntu/Linux has deterministic validation only,
  and Linux live certification is not established.

### Release record

- Release commit: `6b66a357cacea4b1fcf21131cbc9e72fab90d59c`.
- Annotated tag: `v0.15.0`.
- Tag object: `6ca031e66972b5e04dcade6766d6156a9c3e1a9b`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.15.0>.
- Task 188 exact-head CI run `33829088735` passed.
- Windows `repo.create-directory` live certification passed in Task 186.

## v0.14.0 — 2026-09-03

Released as `RAH v0.14.0` for the bounded repository file rename/move milestone.
The immutable release commit is
`52506521bdf838784dd45bb54df2d6bcff8bcd08`; annotated tag `v0.14.0` has tag
object ID `9193423e96dd0cda2fd8f5ed5619ab2b58483acc` and peels to that commit.
Task 177 exact-head release CI run `33727731967` passed. The GitHub Release was
published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.14.0>.

### Added / changed

- A separate host-owned bounded repository file rename/move authority through
  the public `repo.rename-file` Tool.
- Same-directory rename and same-repository cross-directory move for exactly
  one clean HEAD-tracked regular file, guarded by exact source SHA-256 and
  byte-length preconditions and no-replace destination semantics.
- Host-owned authority composition, Desktop integration, and Generic Codex
  Tool Bridge integration for the canonical public capability.

### Security / hardening

- Immediate pre-effect repository, source, destination, HEAD, index, branch,
  runtime, and repository-generation identity revalidation followed by one
  native no-replace effect.
- Possible-effect uncertainty is not replayed. There is no `git mv`,
  copy-delete fallback, automatic staging, or rollback guarantee.
- Repository-generation lifecycle hardening, stale runtime/connection
  publication protections, and reconnect requirements across repository
  switches.
- Complete live-evidence request, advertisement, marker, and structured-result
  observations, with process-wide atomic JSONL evidence serialization.

### Validation

- Deterministic validation on Windows and Ubuntu/Linux where CI/tests provide
  evidence.
- Windows live `repo.rename-file` certification PASS using the certified
  `codex-cli 0.149.0` pair (`codex.exe` and `codex-code-mode-host.exe`).
- Task 176 v0.14 milestone audit PASS.

This is distinct from the v0.13 `repo.delete-file` capability.

### Limitations

- No directory or recursive move, overwrite/replacement, Windows case-only
  rename in v1, untracked-file rename, dirty-file rename, or cross-volume
  copy-delete fallback.
- No generic `fs.rename`, generic filesystem mutation, shell/process authority,
  generic Git authority, or network Git.
- No rollback or transaction guarantee; timeout, cancellation, or disconnect
  does not imply rollback, and possible effects are not replayed.
- Process supervision is not OS sandboxing. Network isolation is not claimed.
- Windows is live-certified; Ubuntu/Linux has deterministic validation only,
  and Linux live certification is not established.

## v0.13.0 — 2026-09-02

Released as `RAH v0.13.0` for the completed bounded repository file-deletion
milestone. The immutable release commit is
`a432d7ecc4a5a564288e2bd50b550055b94920cf`; annotated tag `v0.13.0` has tag
object ID `4a40d8fd0f6065c771dd1a78e4808df0cd02c8e7` and peels to that commit.
Exact-head release CI run `33588660637` passed, and the GitHub Release was
published.

### Added

- `repo.delete-file`, a bounded capability under separate ADR 0017 deletion
  authority. It accepts one explicitly named repository-relative regular file
  only when the clean HEAD-tracked target matches the exact authorized HEAD
  preimage, including raw bytes, SHA-256, and byte length.
- Generic Codex Tool Bridge integration and canonical public tool-name
  discoverability for aliased tools. Provider-private aliases remain
  implementation details.
- Desktop selected-repository integration for the host-created deletion
  authority.

### Verified

- No automatic staging: successful deletion remains an unstaged worktree
  deletion with an unchanged Git index. No commit, ref, or history operation
  is included.
- Trusted Profile composition cannot manufacture deletion authority; model
  requests, provider metadata, Execute permission, tool definitions, and the
  frontend are not authority.
- Windows live validation using `codex-cli 0.149.0` observed public
  `repo.delete-file`, private alias `rah_tool_4` for that run only, and exactly
  `ToolRequested = 1`, `ToolStarted = 1`, `ToolFinished = 1`. The intended
  target was deleted, the sentinel and index were unchanged, the deletion was
  unstaged, HEAD/refs/history were unchanged, no replay occurred, and
  `RAH_REPO_DELETE_FILE_LIVE_OK` was observed.

### Limitations

- The capability does not provide rename/move, directory or recursive
  deletion, arbitrary untracked deletion, generic `fs.write`/`fs.unlink`,
  generic shell/process or Git authority, automatic staging or commit, or
  branch/ref/history/network Git authority.
- The initial aliased-tool discoverability failure is preserved as a failed
  observation that led to the generic canonical-name description fix. The
  CRLF/raw-byte `precondition_failed` observation is preserved as fail-closed
  behavior, not a successful deletion.
- In Task 163 evidence, `tool_finished.result` may be `null` because the helper
  did not capture `ToolContent::Json`; the JSON result was not captured. Task
  164 classified this as non-blocking observability technical debt.
- Deterministic validation is established on Windows and Ubuntu/Linux where
  CI/tests provide evidence. Windows is live-certified; Linux live
  certification is not yet established, and equivalent macOS live validation
  is not claimed.
- Task 120 remains **DEFERRED / NOT VALIDATED** and transport confinement
  remains **NOT CLAIMED**. Process supervision is not OS sandboxing, and
  uncertain effects are not automatically replayed or rolled back.

## v0.12.0 — 2026-09-01

Released as `RAH v0.12.0` for the audited Desktop repository-authoring
milestone. The immutable release commit is
`d1c1cd470fd337f141abb9675fb4642ccd2e00b0`; annotated tag `v0.12.0` has
object ID `4d002a8bc67b1877e692bb0aafd764fc5eb47b65` and peels to that commit.
Task 153 candidate CI run `33479033004` and Task 154 tag CI run `33479751261`
passed. The GitHub Release was published on `2026-09-01T06:58:38Z`.

### Added

- Windows Desktop end-to-end bounded repository workflow: model bounded
  repository authoring, human Stage / Unstage, host-observed staged review,
  human reviewed-snapshot authorization, message-only `repo.commit`, verified
  Git commit result, and Desktop repository refresh.

### Verified

- The existing authority boundaries are productized through Desktop; v0.12
  introduces no new authority. Model request is not authorization; Execute
  permission is not commit authorization; human Stage / Unstage are host
  actions; human Authorize is the reviewed-snapshot authorization event; and
  the frontend does not own authorization.
- `RepositoryCommitReview` remains opaque and Rust-only. `repo.commit` remains
  message-only, does not auto-stage, and uncertain external effects are not
  replayed.
- Windows live validation reused the Task 151 certified `codex-cli 0.149.0`
  bundle with closed manifest schema v2. The demonstrated complete pair is
  `codex.exe` and `codex-code-mode-host.exe`, each with a closed identity and
  SHA-256; the Task 151 hardening closes only the Desktop verifier gap that
  could accept a directory missing the code-mode host.
- The bounded repository-safe authoring path was live-proven. At
  `D:\\rah-task151-clean`, exactly one independently verified Git commit effect
  was observed: `90683f5eaab129a75e815879e69586ff75de5e86`, with no second
  commit or replay. This fixture commit is not the RAH v0.12 release commit.

### Security and limitations

- No generic Git, shell/process, or `fs.write` authority is introduced. There
  is no branch/ref, network, credential, or rollback authority.
- The exact live edit Tool label and exact live `repo.commit` activity-event
  counts were not durably retained. They remain documented non-blocking
  observability gaps; no unsupported lifecycle counts are asserted.
- Windows is live-certified. Unix/macOS live validation is not claimed. Task
  120 remains **DEFERRED / NOT VALIDATED** and transport confinement remains
  **NOT CLAIMED**.

## v0.11.0 — 2026-08-30

Released as `RAH v0.11.0`. The immutable annotated tag `v0.11.0` has object
ID `3fd37807f382c2c0c61328e72d7542984db05983` and peels to release commit
`44a2ee3c6580b862fd0a71b9e773984de757dc15`. Tag CI run `33300410414`
passed, and the GitHub Release was published.

### Added

- `repo.commit`, a bounded host-reviewed repository commit capability that
  creates one ordinary commit from one exact reviewed staged snapshot in the
  exact trusted-profile-selected repository.
- Trusted Profile composition for the exact repository, exact native Git
  executable, explicit trusted host identity, Execute outer permission, and
  separate host-only per-operation authorization.

### Verified

- Deterministic commit-policy hardening, Trusted Profile composition, and
  Generic Tool Bridge verification.
- Windows certified live Codex validation at exactly `codex-cli 0.149.0`; the
  complete same-version official code-mode host was required for the certified
  dynamic-tool path.
- The disposable live fixture completed lifecycle `1 / 1 / 1` with
  `committed_verified` at `13c200c5c772b3e4a0eceb0a2364981c849313e0`.
  This is fixture evidence, not a RAH repository release commit. There was no
  automatic staging, retry, replay, approval, or synthetic tool call.

### Security and limitations

- `repo.commit` is not generic Git authority. Execute alone is insufficient:
  every commit requires fresh host-reviewed authorization; the model controls
  only the message.
- No automatic staging, branch creation/switching, arbitrary ref mutation,
  detached/unborn commit, amend, merge, rebase, cherry-pick, reset, clean,
  stash, tag, remote/network Git, credential Git, linked worktree, or
  submodule/gitlink commit is supported.
- Uncertain effects are never retried or replayed, and no rollback guarantee is
  made. Windows is live-certified; Ubuntu is deterministic evidence only.
- Task 120 remote llama generation remains **DEFERRED / NOT VALIDATED**.
  Transport confinement remains **NOT CLAIMED**.

## v0.10.0 — 2026-08-29

Released as `RAH v0.10.0`. The immutable annotated tag `v0.10.0` has object
ID `d340120e5b316265d6a4cd83bdf08eb73d712d1a` and peels to release commit
`9f4947ce4e37e9ce5b1e49330ab5327c1bd61ffa`. Tag CI run `33248727210`
passed, and the GitHub Release was published.

### Added

- Desktop certified Codex baseline discovery and selection for exactly
  `codex-cli 0.149.0`, plus closed native Git executable discovery.
- One bounded host-selected llama.cpp provider endpoint under ADR 0015, inactive
  Desktop model-preference persistence, exact selected-repository observation,
  verified repository runtime-CWD binding, and launch-CWD/`AGENTS.md` isolation.
- Repository-scoped conversation persistence, explicit bounded Resume/replay,
  SQLite transcript storage, transactional V3-to-SQLite migration, A/B
  transcript isolation, and fail-closed SQLite corruption handling.

### Limitations

- Task 120 remote llama.cpp generation proof is **DEFERRED / NOT VALIDATED**.
  Transport confinement is **NOT CLAIMED**; ADR 0015 does not promise redirect,
  proxy, DNS, peer-identity, or effective-destination confinement.
- No llama.cpp process management or provider/model installation; no generic
  network Tool, network MCP/Streamable HTTP, generic shell/process authority,
  model-selected executable/cwd/endpoint, automatic authority restoration, Git
  commit/ref/history authority, or generic repository delete/rename authority.
- Repository move/rename intentionally changes the conversation-persistence
  namespace. SQLite is private Desktop storage, not generic SQL authority, and
  uncertain external effects have no rollback guarantee.

## v0.9.0 — 2026-08-25

Released as `RAH v0.9.0` on 2026-08-25. The immutable annotated tag `v0.9.0`
has object ID `fbb30c3787911bdb935417bf51d9c0c5f2bdf381` and peels to release
commit `d971790fd1de7df782a99d2274278a14f1f0066f`. Tag CI run `32824354008`
completed successfully, and the GitHub Release was published.

### Added

- `repo.edit-files` bounded multi-file repository edit authority for up to four
  existing, clean, tracked UTF-8 files, with exact original-snapshot
  replacements and deterministic host-owned commit order.
- Verified partial-effect and uncertain-outcome semantics, Trusted Profile v1
  composition, and Generic Tool Bridge integration.

### Verified

- Windows certified Codex live validation using exactly `codex-cli 0.149.0`
  emitted the structural marker `RAH_REPO_EDIT_FILES_LIVE_OK`.
- ADR 0014 is Accepted.

### Security and limitations

- `repo.edit-files` is not a cross-file transaction and provides no rollback
  or replay.
- It grants no generic filesystem write; it cannot create, delete, or rename
  files, and grants no staging, commit, history, ref, or network Git authority.
- Unix live Codex validation is not claimed.

## v0.8.0 — 2026-08-25

Released as `RAH v0.8.0` on 2026-08-25. The immutable annotated tag `v0.8.0`
has object ID `198eccd34a8ae76b9235736c3d1a64173692c351` and peels to release
commit `0b12d5448dcea89b158e4941e7b741b7539c8894`.

### Added

- Bounded repository file creation through `repo.create-file`: one
  host-authorized UTF-8 file at an existing parent directory per call.
- Native exclusive creation with no overwrite, composed through the Trusted
  Profile and the Generic Tool Bridge while retaining host-bound repository
  authority.

### Verified

- Deterministic Windows and Linux coverage, plus certified Codex live
  validation using exactly `codex-cli 0.149.0`.
- The release-preparation CI run `32804191964` and tag CI run `32804873958`
  completed successfully. The GitHub Release was published at
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.8.0>.

### Limitations

- No overwrite, delete, rename, directory creation, binary creation, staging,
  commit/history authority, multi-file transaction, rollback, or replay.
- `repo.create-file` creates one file per call and requires its parent to
  already exist.

## v0.7.0 — 2026-08-24

Released as `RAH v0.7.0` on 2026-08-24. The immutable annotated tag `v0.7.0`
has object ID `b4df68290053f7dd8f6a2b45671fd7cdab8d128f` and peels to release
commit `9521fa4e5f5c184eabd0061eb71854422752b8f1`.

### Added

- `repo.patch` retains its legacy single-replacement request and additionally
  accepts `replacements[]` with one through sixteen exact replacements in one
  existing, HEAD-tracked, regular UTF-8 worktree file.
- Every replacement is resolved against the same original snapshot. Duplicate,
  overlapping, absent, and ambiguous matches are refused; accepted
  non-overlapping replacements are applied deterministically in one final
  single-file replacement.
- Full-file SHA-256 and byte-length preconditions remain mandatory. The
  operation does not automatically stage changes.

### Verified

- Deterministic Generic Tool Bridge validation, Windows native Codex live
  multi-replacement validation, and repository-observer verification cover the
  milestone. Reproducible certified baseline tooling uses isolated configuration
  and host-attested structural markers; the Codex platform-alignment audit
  remains part of the release evidence. Tag CI run `32706469848` completed
  successfully.
- The certified live runtime is exactly `codex-cli 0.149.0`. This release does
  not claim Unix live Codex validation.

### Security and limitations

- `repo.patch` is not arbitrary filesystem write authority. It does not create,
  delete, or rename files; provide a multi-file transaction or rollback; or
  grant Git commit, history, ref, or network authority.
- It does not grant generic shell or process authority. Model requests and
  Codex approvals remain non-authoritative; host policy and `ToolRegistry`
  checks remain required.

## v0.6.0 — 2026-08-24

Released repository-aware read-only workflow inspection milestone. The immutable
annotated tag `v0.6.0` peels to
`6326c18937bbcfd1e515001692a2c88c6884d552`. The GitHub Release, titled
`RAH v0.6.0`, was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.6.0>.

### Added

- A repository-aware read-only observer toolkit: `repo.file-info`,
  `repo.status`, `repo.diff`, and `repo.diff-staged`.
- Trusted-profile composition for the four fixed host observer capabilities,
  deterministic Generic Tool Bridge verification, and Windows live Codex
  verification using exactly `codex-cli 0.149.0`.
- Ubuntu deterministic and cross-platform coverage for repository-observer
  behavior.

### Security

- No new mutation authority. Observers are fixed-command host capabilities;
  `PermissionLevel::Execute` is only their outer host-process gate.
- The observers do not provide generic Git execution or arbitrary executable,
  argv, cwd, or environment selection. They disable external diff and textconv
  behavior and make no intentional repository mutation.
- The existing guarded `repo.patch` worktree mutation capability remains
  separately governed by ADR 0012. ADR 0010 remains repository-index mutation
  only, and ADR 0011 remains trusted-profile authority composition.

### Verified

- Task 064 deterministically verified all four observers through the Generic
  Tool Bridge. Task 065 ran three fresh Windows live fixtures with exactly
  `codex-cli 0.149.0`; each observer was invoked once and the repository was
  unchanged.
- Release-preparation CI run `32685119256` and tag CI run `32685443380`
  completed successfully. This release does not claim Unix live Codex
  validation, transactional snapshot consistency, or zero incidental filesystem
  writes.

## v0.5.1 — 2026-08-22

Released and published as `v0.5.1`, tagged at
`0ea648d84d6f48720c33e8b1bb07e1c24101c870`. This is the portability-only
recovery release for the published v0.5.0 repository-mutation milestone; it
adds no authority, behavior, public API, dependency, or feature expansion.

### Fixed

- Corrected a Linux/Ubuntu clippy portability defect by importing
  `std::fs::File` only for the Windows-native repository identity path that
  uses it. The Ubuntu `unused import: File` failure is removed without changing
  `repo.patch` replacement, policy, or test behavior.

### Verified

- The minimal recovery commit passed the required GitHub Ubuntu CI job,
  including formatting, workspace check, workspace tests, and clippy.
- The release-preparation CI run `32574019502` and tag CI run `32574129999`
  both completed successfully. v0.5.1 is the corrected, fully required-CI
  verified v0.5.x baseline.
- The Windows live `repo.patch` release gate remains valid using exactly
  `codex-cli 0.149.0`; this release makes no Unix live Codex validation claim.

## v0.5.0 — 2026-08-22

Published feature release, tagged at
`b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786`. It contains the complete v0.5
`repo.patch` feature milestone and passed Windows release validation. Its
required Ubuntu CI later failed only because `std::fs::File` was imported
unconditionally while used only by the Windows native-identity path. This was a
lint-only portability defect, not a `repo.patch` authority or runtime-semantics
defect. The public release and immutable tag were preserved unchanged; v0.5.1
supersedes v0.5.0 operationally as the fully verified v0.5.x baseline.

### Added

- `repo.patch`, a repository-aware capability that conditionally replaces one
  exact literal text occurrence in one bounded existing, HEAD-tracked, unstaged
  UTF-8 worktree file.
- ADR 0012, accepted: a separate private, host-owned
  `RepositoryWorktreeMutationPolicy` for worktree-content mutation. The existing
  `PermissionLevel::Execute` is only an outer runtime gate, not the authority.
- Whole-file SHA-256 and byte-length preconditions; exact single-match
  replacement; bounded request, source-file, and postimage sizes; and strict
  UTF-8 handling that preserves a leading BOM and CRLF/LF bytes exactly.
- Repository, path, link/reparse-point, and hard-link protections; a
  same-directory exclusive temporary complete postimage; one-attempt/no-replay
  behavior; and known-failure versus uncertain-effect classification.
- Trusted-profile composition of `repo.patch`, Generic Tool Bridge verification,
  and a Windows live validation using exactly `codex-cli 0.149.0`. Restricted
  Codex-owned filesystem, shell, process, MCP, and network-tool capabilities
  remain disabled in that path.

### Security

- Worktree content mutation, index mutation, and Git history/ref mutation are
  separately authorized state planes. `repo.patch` does not grant generic
  filesystem write, generic shell/process, Git command, or network authority.
- The policy accepts one request and one native replacement attempt only.
  Successful results require post-observation; failures are reported as known
  only when the preimage is proven intact. Uncertain outcomes are never replayed.

### Verified

- Deterministic repository-patch, trusted-profile composition, and Generic Tool
  Bridge coverage; the opt-in live gate observed the one-request/one-attempt
  path, preserved index/HEAD/refs and unrelated content, and cleaned its
  temporary repository and app-server child.
- Windows is the verified v0.5 release baseline. This release makes no Unix
  live-validation claim.

### Limitations

- No file creation, deletion, rename/move, binary edits, multi-file
  transactions, staged or untracked target mutation, or `restore-worktree`.
- No Git history/ref mutation, network Git, automatic rollback, complete TOCTOU
  elimination, or Unix live-validation claim.

## v0.4.0 — 2026-08-22

Released 2026-08-22. Tag `v0.4.0` targets release commit `ebd6358`; CI passed
and the GitHub Release was published.

### Added

- Trusted static capability profiles with strict versioned parsing, hardened
  explicit source loading, symbolic host resources, built-in composition, and
  redacted static/effective inventories.
- `rah profile validate` for non-spawning static validation and `rah profile
  validate-effective` for explicit effective provider composition.
- Trusted-profile composition for hardened local stdio MCP and Process Plugin
  providers, including exact expected tool/schema admission and explicit host
  permission mapping.
- ADR 0011, the trusted capability profile authority boundary.

### Changed

- Effective composition constructs a fresh `ToolRegistry`, preserves declared
  permissions, fails closed on duplicate registration, and retains provider
  lifecycle ownership. Staged providers are cleaned up after later failure.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Security

- Profiles configure existing host authority only; model requests and provider
  metadata remain non-authoritative.
- MCP and Process Plugin providers use native executable validation/revalidation,
  isolated cwd, minimized environment, bounded stdio/lifecycle resources, and
  atomic admission. These controls are not OS sandboxing or network isolation.

### Verified

- Deterministic mixed built-in + MCP + Process Plugin composition, permission
  preservation, redacted inventory, duplicate fail-closed behavior, and staged
  provider cleanup.
- Opt-in trusted-profile Generic Codex Tool Bridge validation using exactly
  `codex-cli 0.149.0`: one `plugin.test.echo` execution, Codex continuation,
  and child/app-server cleanup.

### Deferred

- Profile discovery, reload, editing, or mutation; generic provider and
  subprocess schemas; MCP Streamable HTTP/network MCP; PluginManager;
  provider/plugin installation or download; automatic restart; and hot reload.
- Generic shell/process authority, model-selected executable/argv/cwd/env,
  destructive worktree authority, Git commit/ref/history mutation, network or
  credential-bearing Git, OS sandboxing, network isolation, and rollback.

## v0.3.0 — 2026-08-22

Git tag `v0.3.0` was created at release commit `1968326`.

### Verified

- Generic Tool Bridge, `fs.read`, the MCP adapter, and the process-plugin
  adapter remain available through RAH-owned neutral tool boundaries.
- Hardened `HostExecutionPolicy` is verified through deterministic and opt-in
  live fixture validation.
- Host-owned Execute capabilities are `host.cargo.version`, `host.git.status`,
  `host.git.stage`, and `host.git.unstage`.
- `RepositoryMutationPolicy` is verified through deterministic and opt-in live
  repository-mutation fixture validation; `host.git.stage` and
  `host.git.unstage` have deterministic and opt-in live validation.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Capability classification

`process.test.echo` is the hardened Execute validation fixture, and the
repository-mutation fixture validates mutation policy behavior. Neither is a
production/public host capability. In particular, v0.3.0 does not include
`host.fixture.echo`.

### Deferred

- arbitrary `shell.exec` and `process.exec`;
- model-selected executable, argv, cwd, or environment;
- worktree restore and arbitrary file mutation;
- Git commit, refs/history mutation, reset, clean, checkout, switch, stash,
  merge, rebase, push, pull, fetch, network Git, and credential-bearing Git
  execution.

Destructive worktree authority is deferred beyond v0.3 and requires ADR 0011.

### Security notes

Process supervision is not OS sandboxing. RAH makes no network-isolation or
rollback guarantee. Timeout or cancellation can leave uncertain mutation
effects, and uncertain mutations are never automatically replayed. On Windows,
Job Object assignment remains post-spawn; external OS processes can race
repository mutation, and Git configuration may still influence Git semantics.
