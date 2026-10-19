# ADR 0036: Plugin Kernel — Everything Is a Plugin

Status: Accepted

Date: 2026-10-11

## Context

RAH was conceived as a Rust counterpart of DeepSeek Harness: a small kernel in
which the model adapter, tools, agent loop, session, sandbox, storage,
scheduling and UI are all swappable plugins. From its first commit, however,
RAH grew an authority-first architecture instead. It has fixed extension traits
(`AgentRuntime`, `ModelBackend`, `Tool`, `SessionStore`, `Sandbox`), selects
providers at compile time, embeds the agent loop inside each runtime adapter,
and records host-owned authority composition as hard architectural rules
(ADRs 0001–0035). It has no plugin lifecycle and no runtime composition.

That line was released as the final authority-first version, `v0.35.0`, and
preserved on branch `legacy/authority-first`. The approved pivot plan is
`docs/plans/2026-10-11-plugin-kernel-pivot-plan.md` (route C: a new kernel, with
the legacy authority model retained as an optional policy plugin).

## Decision

### 1. Kernel

RAH's core is a minimal plugin kernel (`rah-kernel`). The kernel owns only:

- plugin lifecycle: register, resolve, start, active, stop, dispose;
- typed service registration and injection (`ServiceKey<S>`), allowing several
  named implementations of one service;
- dependency resolution from manifests (`provides`, `requires`, `optional`).
  A plugin whose required service disappears is stopped and is restarted
  automatically when the service returns;
- an event bus and ordered hook chains (bail, waterfall and serial semantics);
- scoped resources: every task, effect, service and hook belongs to a plugin
  scope and is released when that scope is disposed;
- per-plugin configuration with a schema.

The kernel contains no agent domain logic.

### 2. Everything is a plugin

Model providers, the agent loop, tool providers, session logs, storage,
sandboxes, policy, context assembly, skills, scheduling, frontends and
telemetry are all plugins. Service contracts live in `rah-api`. Plugins depend
only on `rah-api` and `rah-protocol`, never on each other. Replacing a component
means mounting a different plugin, not patching the core.

### 3. Plugin forms

1. **Static**: Rust crates implementing `Plugin`, linked by a distribution
   binary.
2. **Process**: JSON-RPC 2.0 over stdio, generalized from ADR 0008 to provide
   and consume arbitrary services. A process plugin may call only the services
   listed in its own `requires`; the host enforces this.
3. **WASM**: a wasmtime component, planned for a later phase.

Rust dylib and `abi_stable` loading are rejected, because Rust has no stable
ABI.

### 4. Trust model

- **Mounting is trust; model output is never trusted.** Plugins that the user
  mounts in configuration are trusted code, comparable to dependencies. Model
  output and provider metadata remain untrusted requests.
- Every model-originated tool call goes through the kernel-level `ToolInvoker`,
  which always runs the `tool.before_invoke` hook chain.
- **No automatic replay of uncertain effects** is a kernel and session-log
  contract, independent of whether a policy plugin is mounted. Session replay
  re-reads the log; it never re-executes tool calls.
- Process supervision is not OS sandboxing. RAH claims no isolation it has not
  implemented.
- RAH remains an orchestrator, not an inference engine. ADR 0004 is reaffirmed.

### 5. Resolved plan decisions

1. **Default policy.** The `default` preset mounts `policy-authority`, which
   carries over legacy fail-closed permission assignment and the Allow, Deny
   and Ask outcomes. The `minimal` preset mounts no policy and must emit a
   prominent "unguarded" warning at startup.
2. **Name and version.** The project keeps the name RAH. The first release of
   the plugin-kernel architecture is `0.40.0`. The workspace version stayed at
   `0.35.0` until that release was prepared (2026-10-12).
3. **Frontend.** The primary frontend is `frontend-web` (HTTP and WebSocket plus
   a Web UI). A Tauri shell that embeds the Web UI is optional and is not
   required for parity.
4. **Repository write tools.** Only `repo.patch`, `repo.edit-files` and
   `repo.create-file` are ported (to `tools-repo-write`). The other legacy
   mutation tools and every HostExplicit review flow remain on the legacy line.
   Confirmation is provided generically by the policy `Ask` outcome.
   (ADR 0055 added `repo.delete-file`, `repo.move-file` and `repo.commit` as
   new tools of the same plugin.)
5. **Documentation language.** ADRs, `AGENTS.md` and the architecture and
   security documents are written in English. Plans may be written in Chinese.

## Supersession

This ADR supersedes the following ADRs in full for `master`. They remain
authoritative for the `legacy/authority-first` line.

| ADR | Superseded subject | Replacement |
|---|---|---|
| 0001 | `AgentRuntime` as the runtime abstraction | `loop` and `model` services |
| 0002, 0005, 0006, 0030, 0034 | Codex adapter, app-server runtime, dynamic tool bridge, certification and admission | Removed. An external agent may return later as a `loop` plugin. |
| 0003 | Tool-only plugin model; plugins may not own session, loop, policy or sandbox | Everything is a plugin (section 2) |
| 0011 | Trusted capability profile as the composition boundary | Kernel configuration and presets; validation rules move to `policy-authority` |
| 0021–0026 | HostExplicit invocation and reviewed authoring flows | Policy `Ask`, confirmed in the frontend |
| 0033 | Neutral runtime composition and host Tool lifetime | Kernel scopes and `ToolInvoker`; the no-replay rule moves into section 4 |

All other ADRs, except 0004, become **Legacy**. They remain the record of the
authority-first line. Their capability-specific rules (repository observation,
worktree, index and commit separation, bounded mutation, execute policy, MCP
and process adapters, network endpoint bounds, failure sources, work-log memory)
are carried forward only where a plugin explicitly adopts them.

## Consequences

- `AGENTS.md` is rewritten for the plugin-kernel architecture. The prior
  version is kept at `docs/legacy/AGENTS.authority-first.md`.
- Legacy documentation moves to `docs/legacy/`.
- `rah-runtime-codex` and `rah-core` are removed from `master`. `rah-desktop`
  leaves the default members and is removed in phase P5.
- Generic shell execution becomes possible as the opt-in `tools-shell` plugin.
  It requires a `sandbox` service and defaults to `Ask` under `policy-authority`.
  This replaces the legacy rule against any generic shell authority.
- The legacy crates (`rah-runtime`, `rah-model`, `rah-session`,
  `rah-profile-composition`, `rah-tools-plugin`, `rah-runtime-openai`) are
  transformed into or replaced by plugins according to the pivot plan, phases
  P1–P6.
