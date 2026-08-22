# RAH — Rust Agent Harness

RAH is a model-provider-agnostic, runtime-pluggable agent harness written in Rust.
It owns neutral runtime, model, event, session, tool, permission, and sandbox
boundaries. RAH orchestrates inference providers; it is not an inference engine
and does not load model weights or implement model execution.

## v0.4 trusted capability profiles

RAH v0.4 adds a trusted-host static capability profile that securely and
atomically composes existing built-in capabilities and hardened local external
Tool providers into a fresh `ToolRegistry`. It uses explicit permissions,
symbolic host resources, static and effective validation, provider lifecycle
ownership, and redacted authority inspection. It does not add a new generic
authority class.

```text
Built-in Tool -----------\
MCP-backed RAH Tool ------+-> Tool -> ToolRegistry -> host permission -> execution
Process Plugin RAH Tool --/
```

The deterministic native runtime demonstrates the complete neutral loop:

```text
User input
 -> AgentRuntime
 -> ModelBackend
 -> ToolCall
 -> ToolRegistry
 -> host permission policy
 -> Sandbox / workspace policy where applicable
 -> Tool
 -> ToolOutput
 -> ModelBackend
 -> AgentEvent stream
 -> final output
```

The built-in `EchoTool`, `FsReadTool`, and `ShellExecTool` implement the same
provider-neutral `Tool` trait as external-tool proxies. `FsReadTool` is validated
through the registry with an explicit host `Read` permission and workspace path
policy in deterministic tests and an opt-in live Codex example.

`ExternalToolIdentity` gives each discovered external tool a host-side identity.
`ExternalToolPermissionPolicy` is default-deny: an MCP or process-plugin tool is
not registered unless the host explicitly assigns its RAH `PermissionLevel`.
External metadata never grants permission.

ADR 0011 defines the explicitly selected trusted-host capability profile as the
composition boundary for already-approved built-in capabilities and external
providers. It is not model authority and does not replace capability-specific
permission, execution, workspace, or repository-mutation policies.

The authority path is deliberately host-owned:

```text
trusted host
 -> explicit trusted static profile
 -> source validation
 -> symbolic resource resolution
 -> capability/provider-specific constructor and security policy
 -> exact provider admission
 -> fresh ToolRegistry
 -> runtime/model-visible Tool definitions
```

Profiles configure existing authority. A model request remains non-authoritative.

## Generic Codex Tool Bridge

Codex is an optional adapter, not RAH's architecture. `CodexRuntime` implements
`AgentRuntime` and communicates with an exactly version-pinned `codex app-server`
subprocess over newline-delimited stdio JSON-RPC. It does not depend on Codex Rust
crates.

The sole supported Codex baseline is exactly `codex-cli 0.149.0`; RAH does not
claim multi-version Codex compatibility.

In explicitly enabled bridge mode, the Generic Codex Tool Bridge snapshots the
host-supplied `ToolRegistry`, translates definitions to private Codex dynamic-tool
definitions, and translates requests back into RAH `ToolCall` values. RAH then
performs permission checks and dispatches through `ToolRegistry`. Codex never
executes or authorizes the tool itself.

RAH-owned MCP and process-plugin tools use this generic path as ordinary tools:

```text
Codex dynamic-tool request
 -> Generic Codex Tool Bridge
 -> RAH ToolCall
 -> ToolRegistry
 -> MCP-backed or Process Plugin-backed RAH Tool
 -> RAH ToolOutput
 -> Codex model continuation
```

This is distinct from Codex-owned capabilities. Codex-owned shell execution,
file operations, MCP, web search, image viewing, apps, and approval flows remain
disabled. Codex `mcp_servers` remains empty, MCP elicitation and approval requests
are rejected, and Codex shell/file/MCP tool items fail closed.

## External tool adapters

- `rah-tools-mcp` implements a RAH-owned, pinned MCP `2025-06-18` stdio client,
  discovers server tools, and exposes immutable `Tool` proxies. The current
  deterministic fixture provides `mcp.test.echo`.
- `rah-tools-plugin` implements RAH process-plugin protocol version `1` over
  bounded NDJSON stdio. It validates host-configured identity, clears and
  allowlists the child environment, assigns an isolated working directory, and
  exposes discovered tools such as `plugin.test.echo` through `ToolRegistry`.

Neither external adapter grants authority to its child process, and process
supervision is not advertised as operating-system sandboxing.

## Preserved v0.3 host capabilities and validation fixtures

### Public / host capabilities

The v0.3 host-owned Execute capabilities are deliberately narrow and must be
constructed and registered by a trusted host:

- `host.cargo.version`
- `host.git.status`
- `host.git.stage`
- `host.git.unstage`

`host.cargo.version` and `host.git.status` use a fixed trusted executable,
canonical host-selected working location or repository, fixed argv, cleared
environment, closed stdin, bounded output, and timeout. `host.git.stage` and
`host.git.unstage` additionally use the private `RepositoryMutationPolicy`.
Each accepts only `{}` and operates on one host-selected, tracked regular-file
target. They modify only the Git index; they do not write worktree bytes, move
refs, create commits, or use network Git.

`fs.read`, the Generic Codex Tool Bridge, the MCP Tool adapter, and the Process
Plugin Tool adapter are preserved v0.3 components. They use the same RAH-owned
`ToolRegistry` and permission boundary; v0.4 composes them but does not present
them as new capabilities.

### Validation fixtures

The following are deterministic and opt-in live-validation infrastructure, not
production or public host capabilities:

- the hardened Execute fixture, exposed to its tests/live bridge as
  `process.test.echo`;
- the repository-mutation fixture, used to validate `RepositoryMutationPolicy`
  before Git capabilities are exercised.

In particular, RAH v0.3 does **not** provide `host.fixture.echo`.

## Run the deterministic demo and profile validation

The CLI uses scripted model output and requires no model, credentials, network,
or GPU:

```powershell
cargo run -p rah-cli -- run "hello from rah"
cargo run -p rah-cli -- run "read Cargo.toml and report the workspace package information"
cargo run -p rah-cli -- tools
cargo run -p rah-cli -- doctor
cargo run -p rah-cli -- profile validate C:\\trusted-host\\rah-profile.json
cargo run -p rah-cli -- profile validate-effective C:\\trusted-host\\rah-profile.json
```

The manifest-report command dispatches `fs.read` through `ToolRegistry`, an
explicit host `Read` permission, and the workspace path policy.

`profile validate` is non-spawning static/source/schema/resource validation. It
accepts one explicitly supplied absolute trusted-profile path, then prints only
its redacted static inventory. Before parsing, the loader requires a bounded
UTF-8 regular file and rejects links and Windows reparse points. On Windows it
accepts only drive-rooted paths; UNC, verbatim/device paths, ADS, and lexical
aliases are rejected.

`profile validate-effective` is explicit effective composition. It may launch
the trusted MCP and Process Plugin executables named by the selected profile,
performs handshake/discovery/exact schema admission, and prints a redacted
effective inventory. It builds a fresh registry and publishes nothing on
failure. Neither command discovers profiles, selects one from environment or
repository configuration, reloads a profile, or enables model provider
selection.

## Run opt-in live Codex validation

These examples require the exactly supported Codex CLI version, configured live
model access, and may use network or paid API resources. They are excluded from
normal deterministic validation. Set `RAH_CODEX_EXECUTABLE` when `codex` is not
available through `PATH`. The Cargo and Git capability examples additionally
require an absolute trusted native executable through
`RAH_CARGO_VERSION_EXECUTABLE`, `RAH_GIT_STATUS_EXECUTABLE`,
`RAH_GIT_STAGE_EXECUTABLE`, or `RAH_GIT_UNSTAGE_EXECUTABLE`, respectively;
they create their own disposable validation repositories/targets.

```powershell
# Restricted text lifecycle and cancellation
cargo run -p rah-runtime-codex --example live_smoke -- "Reply with exactly: RAH_CODEX_SMOKE_OK"
cargo run -p rah-runtime-codex --example live_cancel_smoke

# Generic bridge with built-in RAH tools
cargo run -p rah-runtime-codex --example live_echo_bridge
cargo run -p rah-runtime-codex --example live_fs_read_bridge

# Hardened Execute validation fixture (not a public capability)
cargo build -p rah-tools --bin rah_execute_fixture
cargo run -p rah-runtime-codex --example live_execute_fixture_bridge

# Public host capabilities; set each documented absolute trusted executable
# and repository/target configuration required by the corresponding example.
cargo run -p rah-runtime-codex --example live_cargo_version_bridge
cargo run -p rah-runtime-codex --example live_git_status_bridge
cargo run -p rah-runtime-codex --example live_git_stage_bridge
cargo run -p rah-runtime-codex --example live_git_unstage_bridge

# RepositoryMutationPolicy validation fixture (not a public capability)
cargo build -p rah-tools --bin rah_repository_mutation_fixture
cargo run -p rah-runtime-codex --example live_mutation_fixture_bridge

# Generic bridge with a RAH-owned MCP tool
cargo build -p rah-tools-mcp --bin rah-mcp-echo-server
cargo run -p rah-runtime-codex --example live_mcp_echo_bridge

# Generic bridge with a RAH-owned process-plugin tool
cargo build -p rah-tools-plugin --bin rah-plugin-echo
cargo run -p rah-runtime-codex --example live_plugin_echo_bridge

# Trusted-profile effective composition through the Generic Codex Tool Bridge
cargo build -p rah-tools-plugin --bin rah-plugin-echo
cargo run -p rah-runtime-codex --example live_trusted_profile_bridge
```

The MCP and process-plugin commands exercise RAH-owned adapters. They do not
enable Codex-owned MCP, shell, or file capabilities.

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal suite uses `MockBackend`, deterministic local fixtures, fake Codex
transport, and captured Codex 0.149.0 schema/JSON fixtures. It does not require a
Codex executable, network access, credentials, a paid API, or a real model.

## v0.4 limitations and explicit deferrals

- The CLI exposes deterministic demos and explicit host-selected profile
  validation, not provider/profile auto-discovery or model-facing profile APIs.
- The Codex dynamic-tool protocol remains experimental and exactly version-pinned.
- MCP support is local pinned stdio only; Streamable HTTP and network MCP are
  not implemented.
- Process plugins are a bounded stdio protocol, not a `PluginManager`, generic
  plugin platform, installer/download mechanism, automatic restart, or hot reload.
- Profiles have no editing/mutation, discovery, auto-discovery, or hot-reload
  capability; provider schemas and generic subprocess schemas are not exposed.
- Arbitrary `shell.exec`, arbitrary `process.exec`, and model-selected
  executable, argv, cwd, environment, or timeout are not live-model authority.
- Worktree restore, arbitrary file mutation, Git commit, refs/history mutation,
  reset, clean, checkout, switch, stash, merge, rebase, push, pull, fetch,
  network Git, and credential-bearing Git execution are deferred. Destructive
  worktree authority requires ADR 0011.
- Process supervision is not OS sandboxing; RAH makes no network-isolation or
  rollback guarantee. Timeout/cancellation may leave uncertain effects, which
  are never automatically replayed.
- OS sandboxing, network isolation, and rollback guarantees are not provided.
- Interactive approvals, SQLite persistence, TUI/web UI, multi-agent
  orchestration, RAG, and long-term memory remain out of scope.

See [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), and the
accepted [ADRs](docs/adr/).
