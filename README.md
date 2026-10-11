# RAH — Rust Agent Harness

RAH is a Rust agent harness built around a plugin kernel: **everything is a
plugin**. A minimal kernel owns plugin lifecycle, typed services, hooks, events,
scoped resources and configuration. Model providers, the agent loop, tools,
session logs, storage, sandboxes, policy, context, skills, scheduling and
frontends are all plugins that can be swapped or recomposed without patching
the core. RAH orchestrates inference providers; it is not an inference engine.

## Status

`master` has completed **phase P2** of the
[plugin-kernel pivot plan](docs/plans/2026-10-11-plugin-kernel-pivot-plan.md):
the kernel, the service contracts (`rah-api`), the first plugins and the `rah`
binary run end to end with a mock model. The architecture decision is
[ADR 0036](docs/adr/0036-plugin-kernel-everything-is-a-plugin.md). The legacy
crates still build and behave as in v0.35.0 while they are transformed phase by
phase. See [Architecture](docs/ARCHITECTURE.md) and [Security](docs/SECURITY.md).

## Quick start

```powershell
cargo run -p rah-cli -- --preset dev
```

The `dev` preset mounts `session-jsonl` (logs in `.rah/sessions`),
`tools-builtin` (`echo` and `fs.read` bounded to the current directory),
`model-mock`, `loop-react` and `frontend-cli`. Type text to run a turn. Try
`read README.md`, or use `/history`, `/fork <seq>`, `/sessions`,
`/resume <id>` and `/plugins`. `--prompt <text>` runs one turn and exits, and
`--session <id>` resumes a session. The `dev` preset mounts no policy plugin,
so `rah` prints an `UNGUARDED` warning; a policy plugin arrives in P4.

Compose your own plugins with a distribution file:

```toml
# rah.toml — run with: rah --config rah.toml
[[plugin]]
use = "session-jsonl"
config = { dir = ".rah/sessions" }

[[plugin]]
use = "model-mock"
id = "model-a"            # instance id; defaults to the `use` name
config = { label = "A" }

[[plugin]]
use = "loop-react"
config = { max_rounds = 8 }

[[plugin]]
use = "frontend-cli"
```

| Phase | Scope |
|---|---|
| P0 | Freeze the legacy line, ADR 0036, new `AGENTS.md`, legacy docs archived, Codex adapter removed |
| P1 | `rah-kernel`: lifecycle, services, hooks, events, scopes, config |
| P2 | `rah-api` and a minimal end-to-end path: mock model, `loop-react`, built-in tools, JSONL sessions, CLI |
| P3 | Real model plugins (OpenAI Responses, llama.cpp) |
| P4 | `policy-authority`, sandbox, MCP, repository tools, process plugins v2 |
| P5 | `frontend-web` replaces the legacy Desktop |
| P6 | WASM plugins, skills, scheduler and subagents, repository write tools |

The first release of the plugin-kernel architecture will be `0.40.0`.

## Legacy line

The authority-first architecture ended with
[v0.35.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.35.0)
and is preserved on branch `legacy/authority-first` (reference and critical
fixes only). Its README, architecture, security model, release gates and plans
are archived under [docs/legacy](docs/legacy/README.authority-first.md).

## Running the current legacy Desktop

Until P5, `rah-desktop` (Windows) remains runnable with the native OpenAI and
llama.cpp Responses adapters. It is excluded from the workspace default members,
so build it explicitly:

```powershell
cargo build -p rah-desktop
$env:RAH_RUNTIME_PROVIDER = 'llama_cpp'   # or 'openai'
# llama.cpp: start your llama-server; the default endpoint is http://127.0.0.1:8080
# OpenAI: set OPENAI_API_KEY and RAH_OPENAI_MODEL in the backend environment
```

Credentials are backend-only environment configuration. The optional
`RAH_LLAMA_CPP_API_KEY` is separate from `OPENAI_API_KEY`.

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal suite uses deterministic local fixtures and needs no network access,
credentials, paid API or real model. Known issue: on some Windows hosts, 43-44
legacy `rah-desktop` tests fail with `RepositoryMemberStale`, a failure that is
pre-existing since v0.34.0 (see the
[v0.35 release gate](docs/legacy/RAH_V0.35_RELEASE_GATE.md)). CI runs on Linux,
where Desktop tests are not exercised.

## Contributing

Read [AGENTS.md](AGENTS.md) before changing code. It defines the kernel,
plugin and trust rules for `master`.
