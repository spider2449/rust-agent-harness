# RAH Architecture — Plugin Kernel

Governing decision: [ADR 0036](adr/0036-plugin-kernel-everything-is-a-plugin.md).
Roadmap and phase acceptance criteria:
[plugin-kernel pivot plan](plans/2026-10-11-plugin-kernel-pivot-plan.md).
The authority-first architecture (v0.35.0 and earlier) is documented in
[docs/legacy/ARCHITECTURE.md](legacy/ARCHITECTURE.md).

## Target architecture

```text
rah-protocol                         data types (dependency bottom)
rah-kernel                           lifecycle / services / hooks / events / scopes / config
   ^           ^                     (depends on no RAH crate)
rah-api                              service contracts: ModelProvider, AgentLoop, Tool,
   ^                                 ToolInvoker, SessionLog, KvStore, ExecEnv, ...
plugins/*                            depend only on rah-api and rah-protocol
   model-openai-responses, model-anthropic, model-llamacpp, model-chat-completions,
   model-mock
   loop-react
   tools-builtin, tools-mcp, tools-mcp-http, tools-repo, tools-repo-write, tools-shell,
   tools-web-search
   session-jsonl, storage-fs, sandbox-process
   policy-authority, context-default, skills-fs, scheduler-basic
   frontend-cli, frontend-web
rah-plugin-host-process              process plugin protocol v4 (JSON-RPC over stdio)
rah-plugin-host-wasm                 WASM components (wasmtime, WIT world rah:plugin), no WASI
rah (bin)                            distribution: reads rah.toml, mounts plugins
```

### Kernel

- Lifecycle: `Pending -> Starting -> Active -> Stopping`, plus `Failed` and
  `DependencyCycle`. A plugin starts when every `requires` service resolves.
  When a provider it is bound to stops, it is stopped first (dependents before
  providers, reverse activation order) and restarts once the service resolves
  again. Lifecycle operations are serialized; calling one from inside `apply`
  or a dispose effect returns `Reentrant`.
- Services: typed `ServiceKey<S>`; several named instances per service. A
  mount `binding` selects an instance, otherwise the earliest-registered one
  is used. A plugin may only provide services in its `provides` and inject
  services in its `requires` or `optional`.
- Hooks: typed `BailHook`, `WaterfallHook` and `SerialHook` chains, ordered by
  priority then registration (for example `tool.before_invoke` as bail,
  `model.before_request` as waterfall). A panicking handler yields
  `HookError::Panicked`, which security callers treat as a denial.
- Events: typed, synchronous, panic-isolated handlers; the kernel emits
  `PluginStatusChanged` on every lifecycle transition.
- Scopes: services, hooks and handlers registered in `apply` commit only if
  `apply` succeeds. On dispose, owned tasks are cancelled and drained, then
  dispose effects run in reverse order. Nothing outlives its owner.
- Configuration: a plugin's `check_config` runs before a mount or a
  configuration change. A refused configuration fails the operation with
  `InvalidConfig` and leaves a running instance as it was (ADR 0044).

### A turn

```text
frontend --TurnRequest--> loop
  loop: model.before_request hooks (context, skills, memory)
     -> model.stream() (opened again with a wait when the provider cannot take it, ADR 0051;
        with a smaller request from model.overflow when it does not fit, ADR 0052)
     -> ToolCall -> ToolInvoker.invoke()
          -> tool.repair_input hooks (tool-input-repair: only call.input changes)
          -> tool.before_invoke hooks (policy: Allow / Deny / Ask)
          -> tool-admission providers (policy: in-flight limit)
          -> ToolStarted
          -> ToolProvider
          -> tool.after_invoke hooks
     -> ... -> Completed
  every AgentEvent -> EventBus -> session log (append-only), frontend, telemetry
```

### Session log

Sessions are append-only JSONL with `resume`, `fork(at)` and `replay`. Replay
re-reads entries and never re-executes tool calls. Tool results carry
`effect: none | completed | uncertain`.

A session also has metadata: a title (`rename`, stored as an appended
metadata line, last one wins; records are never rewritten), its last change
(`updated`) and a preview of its first message. `delete` removes a session
from the list (`session-jsonl` moves the file to `deleted/`) and is refused
while forks of it exist. The provider emits `SessionAppended` after every
append; `session-titles` uses it to name a session after its first turn.

## Current state (after phase P5)

Implemented:

- `rah-kernel` (P1) and `rah-api` (P2): service contracts `model`, `loop`,
  `session`, `sandbox` (`ExecEnv`: `run` to completion, or `spawn` a
  long-running program with pipes), tools with `ToolInvoker`, and the standard
  hooks `tool.repair_input` (ADR 0049), `tool.before_invoke`, `tool.approval`,
  `tool.after_invoke`, `model.before_request` and `model.overflow` (ADR 0052,
  runs after `model.stream()` fails to fit, never before
  `model.before_request`). `tool-admission` (`ToolAdmission`, ADR 0045)
  admits authorized calls before they start; `policy-authority` uses it to
  bound how many calls run at once. Loops publish each turn event on the event bus as
  `TurnEvent`, and providers report token usage as `ModelEvent::Usage`
  (ADR 0040). `storage` (`KvStore`, ADR 0041): a namespaced key-value
  store for plugin state that must survive a restart, each plugin in the
  namespace of its id.
- Plugins in `plugins/`: `model-mock`; `model-openai` and `model-llamacpp`
  (crate `model-responses`); `model-anthropic` (Anthropic Messages API);
  `model-chat-completions` (OpenAI-compatible Chat Completions: DeepSeek,
  Ollama, vLLM, llama-server);
  `loop-react` and `loop-plan` (plan-and-execute;
  either can replace the other at runtime); `tools-builtin` (`echo` and
  workspace-bounded `fs.read`, `fs.list`, `fs.glob` and
  `fs.grep`, which need no Git); `session-jsonl`; `session-titles` (asks the
  model for a title after a session's first turn); `policy-authority`;
  `telemetry-log` (mounted by every preset: JSONL records of turns, model
  requests and tools with durations, policy decisions and token usage, and
  the Web UI's `usage` view);
  `todo` (opt-in feature `todo`: `todo.write` and `todo.read`, permission
  `none`, keep a to-do list per session in memory, rebuilt from the session
  log when unknown, and its `list` view shows the open session's list);
  `tool-input-repair` (mounted by every preset: converts arguments that are
  one unambiguous step from valid for the tool's schema, such as `"20"` for
  an integer, before the policy sees the call; ADR 0049);
  `format-on-write` (opt-in: runs the project's formatter on files completed
  `write` tools changed, through `sandbox`, before `lsp-diagnostics` looks at
  them; ADR 0053);
  `lsp-diagnostics` (opt-in: appends language server diagnostics to the
  output of completed `write` tools through `tool.after_invoke`; servers
  start through `sandbox`, ADR 0048);
  `context-compact` (opt-in: summarizes history beyond a size limit with the
  model before `context-default` applies its window; summaries are cached by
  the history prefix they cover, and kept in `storage` when it is mounted);
  `storage-fs` (the `storage` service as one file per entry under
  `storage` in the data directory, mounted by every preset; `rah` resolves
  every plugin's data paths against one data directory, `~/.rah` unless
  `--data-dir` or `RAH_HOME` says otherwise);
  `context-default` (system prompt, sliding history window and file-backed
  memory notes through `model.before_request`, plus `memory.remember`; with
  `memory_dir` one file per workspace, chosen from the sandbox root on every
  request);
  `context-instructions` (opt-in, ADR 0061: the workspace root's
  `AGENTS.md` and configured instruction files, read on every request
  through `model.before_request`; `context-default` can also read its system
  prompt from `system_file`);
  `skills-fs` (skills listed through `model.before_request`, loaded with
  `skills.load`; problems with a skill are told to the user once as a
  `HostNotice`); `sandbox-process` (supervised processes, not an OS
  sandbox); `sandbox-os` (opt-in: the same supervision plus, on Linux,
  landlock file confinement and a seccomp network filter, ADR 0042, and on
  Windows a Low integrity token with Low-labeled writable directories,
  ADR 0043);
  `tools-repo`
  (read-only Git tools, requires `sandbox`); `tools-repo-write`
  (`repo.create-file`, `repo.edit-files`, `repo.patch`, `repo.delete-file`,
  `repo.move-file`, declared `write`, and `repo.commit`, declared `execute`
  and provided only when a commit identity is configured, ADR 0055; it also
  provides `file-undo`, an in-memory undo history of each session's file
  changes that the frontends offer as `/undo` and an Undo button, ADR 0060);
  `tools-mcp` (one MCP stdio server
  per instance, requires `sandbox`); `tools-mcp-http` (one Streamable HTTP
  MCP server per instance, ADR 0038); `tools-shell` (opt-in `shell.exec`,
  requires `sandbox`); `tools-http` (opt-in; one instance is one API group
  whose endpoints are declared as tools in configuration, ADR 0047);
  `tools-web-search` (opt-in keyless `web.search` over
  DuckDuckGo, Bing and SearXNG with failover, merged `web.multi_search`, and
  `web.fetch` limited to public addresses); `frontend-cli`, which also answers `tool.approval`
  on the terminal; and `frontend-web`, a loopback HTTP and WebSocket API
  with an embedded Web UI (`rah --web`), which answers `tool.approval` in the
  browser and owns the modes (daily work, coding, configured ones) that
  switch plugins through `admin` and add instructions on
  `model.before_request`. `rah-desktop` is not a plugin: it boots the same
  distribution and shows that Web UI in a native window, so every behavior
  is defined once (ADR 0046).
- `ui-layout` service (`rah-api::ui`, ADR 0062): where `frontend-web` places
  its panels (`sessions`, `files`, `plugins`) in the left and right columns.
  Without a provider the page uses `Layout::standard()`, the layout it has
  always had; the conversation, header, approvals and status bar are never
  panels, so no layout moves or hides them. `ui-layout-config` provides it
  from its configuration.
- `ui-views` service (`rah-api::ui`, ADR 0062): views plugins contribute as
  panels (`<instance>/<view>`), as data from a fixed vocabulary that
  `frontend-web` renders as text, with buttons and forms that call the
  provider on the user's click. Not a tool, so the model cannot reach it.
  A provider emits `UiChanged` when a view changed, and windows showing it
  render it again. `telemetry-log` offers a `usage` view, `todo` a `list` view.
- `admin` service (`rah-api::admin::PluginAdmin`): runtime plugin management
  (mount, unmount, configure, restart, replace, stop, start) and an example
  configuration per mountable kind (`config_example`; each plugin crate
  keeps its own as `<Plugin>::CONFIG_EXAMPLE`). `stop` unmounts
  a plugin but keeps its kind, configuration and bindings, so `start` can
  mount it again under the same id; its dependents wait as `Pending`. The
  kernel has no stopped state: the distribution remembers stopped plugins.
  `rah` always mounts the
  `distribution` plugin first, which provides it from the distribution's
  catalog; `frontend-web` exposes it to the local user. `replace` mounts the
  new plugin and unmounts the old one only once the new one is active, so
  consumers that resolve services per use switch without a restart.
- `scheduler` service (`rah-api::scheduler`), provided by `scheduler-basic`:
  background and repeating turns run through the `loop` service; the same
  plugin provides `agent.delegate`, which nests a subagent turn inside a tool
  call (the loop is a service, so loops compose). `frontend-web` exposes jobs
  at `/api/jobs`.
- `rah-plugin-host-process`: out-of-process plugins over JSON-RPC 2.0 on
  stdio ([protocol v4](PROCESS_PLUGINS.md)). They may provide `tools`,
  attach to `tool.before_invoke`, `tool.after_invoke` and
  `model.before_request`, follow `session.appended` events (ADR 0039),
  provide `ui-views` and announce changes with `ui/changed` (ADR 0063), and
  call the host's `session` and `storage` services only if they are in
  their `requires` (`storage` always in the namespace of their id). `rah` mounts
  them with `use = "process"`.
- `rah-plugin-host-wasm`: WebAssembly components of the WIT world
  `rah:plugin/wasm-plugin` ([WASM plugins](WASM_PLUGINS.md)), run in-process
  by wasmtime without WASI. They provide `tools`; their only import is
  `call-service`, limited to their `requires` (`session`, and `storage` in
  the namespace of their id). Each call
  runs in a fresh store with a fuel and memory budget. `rah` mounts them with
  `use = "wasm"`.
- `rah` (crate `rah-cli`): the distribution binary. It mounts plugins from
  `rah.toml` or a preset (`default`, `openai`, `dev`, `minimal`); a
  `rah.toml` with `extends = "<preset>"` lists only its changes to that preset
  (ADR 0058). Every preset
  except `minimal` mounts `policy-authority` first. `rah` warns `UNGUARDED`
  when no `tool.before_invoke` handler is mounted, and aborts the boot when a
  configured `policy-authority` does not become active.

The tools service key is private to `rah-api`. Tool providers register with
`provide_tools`, and the only way to call a tool is `ToolInvoker::invoke`.
`loop-react` resolves the model on every round, and `frontend-cli` resolves the
loop and session per use, so swapping a model plugin restarts neither.

The legacy crates are gone from `master`: `rah-session` (P4), and
`rah-desktop`, `rah-runtime`, `rah-runtime-openai`, `rah-model`, `rah-tools`,
`rah-tools-mcp`, `rah-tools-plugin`, `rah-sandbox` and
`rah-profile-composition` (P5). Their capabilities live on as plugins:
`model-responses`, `loop-react`, `tools-builtin`, `tools-repo`, `tools-shell`,
`tools-mcp`, `sandbox-process`, `policy-authority`, `rah-plugin-host-process`
and `frontend-web`. The v0.35.0 tag keeps the legacy line.

Removed in P0: `rah-runtime-codex` (the Codex adapter and its certification
harness) and `rah-core` (empty).
