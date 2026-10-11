# RAH Architecture — Plugin Kernel

Governing decision: [ADR 0036](adr/0036-plugin-kernel-everything-is-a-plugin.md).
Roadmap and phase acceptance criteria:
[plugin-kernel pivot plan](plans/2026-10-11-plugin-kernel-pivot-plan.md).
The authority-first architecture (v0.35.0 and earlier) is documented in
[docs/legacy/ARCHITECTURE.md](legacy/ARCHITECTURE.md).

## Target architecture

```text
rah-protocol                         data types (dependency bottom)
   ^
rah-kernel                           lifecycle / services / hooks / events / scopes / config
   ^
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

- Lifecycle: `Registered -> Pending -> Starting -> Active -> Stopping -> Disposed`.
  A plugin starts when every `requires` service is available. It is stopped
  when a required service disappears, and restarted when the service returns.
- Services: typed `ServiceKey<S>`; several named implementations per service,
  chosen by configuration.
- Hooks: ordered chains with bail (`tool.before_invoke`: Allow, Deny or Ask),
  waterfall (`model.before_request`, `tool.after_invoke`) and serial
  (`turn.*`) semantics.
- Scopes: tasks, effects, services and hooks registered by a plugin are
  released when its scope is disposed. Nothing outlives its owner.

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

## Current state (transitional, phase P0)

The kernel does not exist yet. `master` still contains the legacy crates, which
are transformed or removed phase by phase:

| Crate | Fate | Phase |
|---|---|---|
| `rah-protocol` | kept; gains model-event and session-log types | P1–P2 |
| `rah-runtime-openai` | split into `model-openai-responses`, `model-llamacpp`, `loop-react` | P3 |
| `rah-runtime`, `rah-model` | replaced by `rah-api` services and `model-mock` | P3 |
| `rah-tools` | `Tool` moves to `rah-api`; tools split into `tools-builtin`, `tools-repo`, `tools-shell`; authorization becomes `policy-authority` | P4 |
| `rah-tools-mcp` | becomes `tools-mcp` | P4 |
| `rah-tools-plugin` | generalized into `rah-plugin-host-process` | P4 |
| `rah-sandbox` | becomes `sandbox-process` | P4 |
| `rah-session`, `rah-profile-composition` | removed | P4 |
| `rah-cli` | becomes the `rah` distribution binary and `frontend-cli` | P2 |
| `rah-desktop` | legacy, excluded from default members; removed once `frontend-web` reaches parity | P5 |

Removed in P0: `rah-runtime-codex` (the Codex adapter and its certification
harness) and `rah-core` (empty).

The legacy runtime still works as released in v0.35.0. Desktop uses the
native OpenAI and llama.cpp adapters, and model tool requests go through the
legacy host port, permission dispatch and `ToolRegistry`.
