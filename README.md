# RAH — Rust Agent Harness

RAH is a model-provider-agnostic, runtime-pluggable agent harness written in Rust.
It owns neutral runtime, model, event, session, tool, permission, and sandbox
boundaries. RAH orchestrates inference providers; it is not an inference engine
and does not load model weights or implement model execution.

## v0.1 architecture

The deterministic native test path demonstrates the complete neutral flow:

```text
User input
 -> AgentRuntime
 -> ModelBackend
 -> ToolCall
 -> ToolRegistry
 -> permission policy
 -> Sandbox / workspace policy
 -> Tool
 -> ToolOutput
 -> ModelBackend
 -> AgentEvent stream
 -> final output
```

Codex is an optional adapter, not RAH's architecture. `CodexRuntime` implements
`AgentRuntime`; it does not implement `ModelBackend`. The v0.1 adapter starts an
exactly version-pinned `codex app-server` subprocess and communicates over
newline-delimited stdio JSON-RPC. RAH does not depend on Codex Rust crates.

Before use, the adapter verifies the exact Codex executable version and checks the
locally generated app-server schema against its captured v0.1 contract. Codex
wire DTOs, JSON-RPC IDs, thread/turn IDs, process types, and transport types remain
private to `rah-runtime-codex`.

## Restricted Codex capability set

The first `CodexRuntime` supports only:

- initialize/initialized handshake;
- thread start and resume;
- turn start and terminal completion;
- streamed agent-message deltas;
- private RAH session-to-Codex thread mapping;
- turn interruption with terminal cancellation confirmation;
- owned subprocess and event-stream lifecycle;
- typed adapter failures and additive-notification tolerance.

It does not support Codex-owned shell execution, file modification, MCP execution,
dynamic tool execution, or interactive approval acceptance. Known Codex tool
surfaces are disabled in the restricted thread configuration, server-initiated
tool and approval requests are explicitly rejected, and Codex tool-item events
never become RAH tool lifecycle events.

## Run the deterministic demo

The CLI uses scripted model output and requires no model, credentials, network,
or GPU:

```powershell
cargo run -p rah-cli -- run "hello from rah"
cargo run -p rah-cli -- run "read Cargo.toml and report the workspace package information"
cargo run -p rah-cli -- tools
cargo run -p rah-cli -- doctor
```

The manifest-report command dispatches `fs.read` through `ToolRegistry`, an
explicit host `Read` permission, and the workspace path policy.

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal test suite uses `MockBackend`, fake subprocess transport, and captured
Codex 0.148.0 schema/JSON fixtures. It does not require the Codex executable,
network access, credentials, paid APIs, or a real model. The architecture gate is
part of `cargo test --workspace`.

## v0.1 limitations

- The CLI exposes the deterministic demo path, not live provider selection.
- The Codex adapter is restricted to text lifecycle behavior and is library-only.
- Interactive approvals are not represented by the current `AgentRuntime` API.
- General MCP/process plugins, a plugin manager, SQLite persistence, TUI/web UI,
  multi-agent orchestration, RAG, and long-term memory are deferred.
- Workspace path enforcement and process policy are not advertised as complete
  operating-system isolation.

See [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), the
[Codex integration spike](docs/CODEX_INTEGRATION_SPIKE.md), and
[ADR-0005](docs/adr/0005-codex-app-server-runtime.md).
