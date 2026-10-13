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
   model-openai-responses, model-anthropic, model-llamacpp, model-mock
   loop-react
   tools-builtin, tools-mcp, tools-repo, tools-repo-write, tools-shell
   session-jsonl, storage-fs, sandbox-process
   policy-authority, context-default, skills-fs, scheduler-basic
   frontend-cli, frontend-web
rah-plugin-host-process              process plugin protocol v2 (JSON-RPC over stdio)
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

### A turn

```text
frontend --TurnRequest--> loop
  loop: model.before_request hooks (context, skills, memory)
     -> model.stream()
     -> ToolCall -> ToolInvoker.invoke()
          -> tool.before_invoke hooks (policy: Allow / Deny / Ask)
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
  hooks
  `tool.before_invoke`, `tool.approval`, `tool.after_invoke` and
  `model.before_request`.
- Plugins in `plugins/`: `model-mock`; `model-openai` and `model-llamacpp`
  (crate `model-responses`); `model-anthropic` (Anthropic Messages API);
  `loop-react` and `loop-plan` (plan-and-execute;
  either can replace the other at runtime); `tools-builtin` (`echo` and
  workspace-bounded `fs.read`); `session-jsonl`; `session-titles` (asks the
  model for a title after a session's first turn); `policy-authority`;
  `context-default` (system prompt, sliding history window and file-backed
  memory notes through `model.before_request`, plus `memory.remember`; with
  `memory_dir` one file per workspace, chosen from the sandbox root on every
  request);
  `skills-fs` (skills listed through `model.before_request`, loaded with
  `skills.load`); `sandbox-process` (supervised processes, not an OS
  sandbox); `tools-repo`
  (read-only Git tools, requires `sandbox`); `tools-repo-write`
  (`repo.create-file`, `repo.edit-files`, `repo.patch`, declared `write`);
  `tools-mcp` (one MCP stdio server
  per instance, requires `sandbox`); `tools-shell` (opt-in `shell.exec`,
  requires `sandbox`); `frontend-cli`, which also answers `tool.approval`
  on the terminal; and `frontend-web`, a loopback HTTP and WebSocket API
  with an embedded Web UI (`rah --web`), which answers `tool.approval` in the
  browser.
- `admin` service (`rah-api::admin::PluginAdmin`): runtime plugin management
  (mount, unmount, configure, restart, replace). `rah` always mounts the
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
  stdio ([protocol v2](PROCESS_PLUGINS.md)). They may provide `tools` and call
  the host's `session` service only if it is in their `requires`. `rah` mounts
  them with `use = "process"`.
- `rah-plugin-host-wasm`: WebAssembly components of the WIT world
  `rah:plugin/wasm-plugin` ([WASM plugins](WASM_PLUGINS.md)), run in-process
  by wasmtime without WASI. They provide `tools`; their only import is
  `call-service`, limited to their `requires` (currently `session`). Each call
  runs in a fresh store with a fuel and memory budget. `rah` mounts them with
  `use = "wasm"`.
- `rah` (crate `rah-cli`): the distribution binary. It mounts plugins from
  `rah.toml` or a preset (`default`, `openai`, `dev`, `minimal`). Every preset
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
