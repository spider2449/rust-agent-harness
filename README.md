# RAH — Rust Agent Harness

RAH is a Rust agent harness built around a plugin kernel: **everything is a
plugin**. A minimal kernel owns plugin lifecycle, typed services, hooks, events,
scoped resources and configuration. Model providers, the agent loop, tools,
session logs, storage, sandboxes, policy, context, skills, scheduling and
frontends are all plugins that can be swapped or recomposed without patching
the core. RAH orchestrates inference providers; it is not an inference engine.

## Quick start

```powershell
.\scripts\install.ps1          # or: cargo install --path crates/rah-cli --locked
rah config init                # pick a model, permissions and workspace
rah                            # terminal REPL
rah web                        # Web UI in the browser
rah-desktop                    # the same Web UI in a native window
```

`rah config init` asks a few questions and writes `rah.toml` in the data
directory (`~/.rah`). A small one looks like this:

```toml
model       = "anthropic/claude-sonnet-5-5"   # anthropic, openai, deepseek, openai-compatible, llamacpp, mock
permissions = "ask"                            # read-only | ask | auto-edit | full
features    = ["web-search"]                   # `rah features` lists them
```

API keys stay in environment variables (`ANTHROPIC_API_KEY`, ...). To try RAH
without a model or key: `rah --model mock`. Options such as `--model`,
`--permissions` and `--root` override the file for one run, and settings saved
in the Web UI are written to it. `rah config show` prints what would run.

Without installing: `.\scripts\start.ps1` builds and starts the Web UI from
this checkout (`-Cli`, `-Desktop`, `-Model mock`).

## Documentation

- [Configuring and running RAH](docs/CONFIGURATION.md): the configuration
  file, commands, the Web UI, features, sandboxes, MCP, providers, plugin
  entries and scripts.
- [Architecture](docs/ARCHITECTURE.md) and the decision records in
  [docs/adr](docs/adr), starting with
  [ADR 0036](docs/adr/0036-plugin-kernel-everything-is-a-plugin.md).
- [Security](docs/SECURITY.md): what the policy, sandboxes and the Web UI do
  and do not guarantee.
- [Process plugins](docs/PROCESS_PLUGINS.md), [WASM plugins](docs/WASM_PLUGINS.md),
  [MCP servers](docs/MCP_SERVERS.md), [harness2](docs/HARNESS2.md).

## Status

`master` has completed phases P0 to P6 of the
[plugin-kernel pivot plan](docs/plans/2026-10-11-plugin-kernel-pivot-plan.md)
(first released as 0.40.0; current release 0.45.1). RAH runs end to end with
a mock model, OpenAI and llama.cpp through the Responses API, Anthropic
through the Messages API, and Chat Completions servers. Tool calls are guarded
by the fail-closed `policy-authority` plugin. The model plugins are tested
against deterministic local fixtures; the live OpenAI, Anthropic and DeepSeek
APIs have not been verified.

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
