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
   model-openai-responses, model-llamacpp, model-mock
   loop-react
   tools-builtin, tools-mcp, tools-repo, tools-repo-write, tools-shell
   session-jsonl, storage-fs, sandbox-process
   policy-authority, context-default, skills-fs, scheduler-basic
   frontend-cli, frontend-web
rah-plugin-host-process              process plugin protocol v2 (JSON-RPC over stdio)
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

## Current state (transitional, phase P4 in progress)

Implemented:

- `rah-kernel` (P1) and `rah-api` (P2): service contracts `model`, `loop`,
  `session`, `sandbox` (`ExecEnv`: `run` to completion, or `spawn` a
  long-running program with pipes), tools with `ToolInvoker`, and the standard
  hooks
  `tool.before_invoke`, `tool.approval`, `tool.after_invoke` and
  `model.before_request`.
- Plugins in `plugins/`: `model-mock`; `model-openai` and `model-llamacpp`
  (crate `model-responses`); `loop-react`; `tools-builtin` (`echo` and
  workspace-bounded `fs.read`); `session-jsonl`; `policy-authority`;
  `sandbox-process` (supervised processes, not an OS sandbox); `tools-repo`
  (read-only Git tools, requires `sandbox`); `tools-mcp` (one MCP stdio server
  per instance, requires `sandbox`); `tools-shell` (opt-in `shell.exec`,
  requires `sandbox`); and `frontend-cli`, which also answers `tool.approval`
  on the terminal.
- `rah` (crate `rah-cli`): the distribution binary. It mounts plugins from
  `rah.toml` or a preset (`default`, `openai`, `dev`, `minimal`). Every preset
  except `minimal` mounts `policy-authority` first. `rah` warns `UNGUARDED`
  when no `tool.before_invoke` handler is mounted, and aborts the boot when a
  configured `policy-authority` does not become active.

The tools service key is private to `rah-api`. Tool providers register with
`provide_tools`, and the only way to call a tool is `ToolInvoker::invoke`.
`loop-react` resolves the model on every round, and `frontend-cli` resolves the
loop and session per use, so swapping a model plugin restarts neither.

The remaining legacy crates are transformed or removed phase by phase:

| Crate | Fate | Phase |
|---|---|---|
| `rah-runtime-openai` | ported to `model-responses` (P3); removed together with Desktop | P5 |
| `rah-runtime`, `rah-model` | replaced by `rah-api` and the model plugins; removed together with Desktop | P5 |
| `rah-tools` | `Tool` moved to `rah-api`; read-only tools ported to `tools-builtin`, `tools-repo` and `tools-shell`, authorization to `policy-authority`; the crate is removed with Desktop | P5 |
| `rah-tools-mcp` | ported to `tools-mcp` (P4); removed with `rah-profile-composition` | P4 |
| `rah-tools-plugin` | generalized into `rah-plugin-host-process` | P4 |
| `rah-sandbox` | ported to `sandbox-process` (P4); removed with the legacy tools and Desktop | P5 |
| `rah-session`, `rah-profile-composition` | removed | P4 |
| `rah-desktop` | legacy, excluded from default members; removed once `frontend-web` reaches parity | P5 |

Removed in P0: `rah-runtime-codex` (the Codex adapter and its certification
harness) and `rah-core` (empty).

The legacy runtime still works as released in v0.35.0. Desktop uses the
native OpenAI and llama.cpp adapters, and model tool requests go through the
legacy host port, permission dispatch and `ToolRegistry`.
