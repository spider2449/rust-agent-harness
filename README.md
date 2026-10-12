# RAH — Rust Agent Harness

RAH is a Rust agent harness built around a plugin kernel: **everything is a
plugin**. A minimal kernel owns plugin lifecycle, typed services, hooks, events,
scoped resources and configuration. Model providers, the agent loop, tools,
session logs, storage, sandboxes, policy, context, skills, scheduling and
frontends are all plugins that can be swapped or recomposed without patching
the core. RAH orchestrates inference providers; it is not an inference engine.

## Status

`master` has completed **phase P5** of the
[plugin-kernel pivot plan](docs/plans/2026-10-11-plugin-kernel-pivot-plan.md):
the kernel, the service contracts (`rah-api`), the plugins and the `rah` binary
run end to end with a mock model, and with OpenAI or llama.cpp through the
Responses API. Tool calls are guarded by the fail-closed `policy-authority`
plugin, and tools come from built-in, repository, shell, MCP and
out-of-process plugins. The architecture decision is
[ADR 0036](docs/adr/0036-plugin-kernel-everything-is-a-plugin.md). The legacy
Desktop and its crates were removed in P5; the Web UI (`rah --web`) replaces
it. See [Architecture](docs/ARCHITECTURE.md) and [Security](docs/SECURITY.md).

## Quick start

```powershell
cargo run -p rah-cli -- --preset dev
```

The `dev` preset mounts `policy-authority`, `session-jsonl` (logs in
`.rah/sessions`), `tools-builtin` (`echo` and `fs.read` bounded to the current
directory), `sandbox-process` and `tools-repo` (read-only `repo.status`,
`repo.diff`, `repo.list` and `repo.search` over Git), `tools-repo-write`
(`repo.create-file`, `repo.edit-files` and `repo.patch`, each asking for
approval), `context-default` (a
sliding history window and memory notes in `.rah/memory.md`, which the model
can extend with `memory.remember` after you approve), `skills-fs` (skills in
`.rah/skills/<name>/SKILL.md`, listed to the model and loaded on demand with
`skills.load`), `model-mock`,
`loop-react` and `frontend-cli`. Type text to run a turn. Try
`read README.md` or `/call repo.status {}`, or use `/history`, `/fork <seq>`,
`/sessions`, `/resume <id>` and `/plugins`. `--prompt <text>` runs one turn and exits, and
`--session <id>` resumes a session.

| Preset | Model plugin | Policy |
|---|---|---|
| `default` (the default) | `model-llamacpp` | `policy-authority` |
| `openai` | `model-openai` | `policy-authority` |
| `dev` | `model-mock` | `policy-authority` |
| `minimal` | `model-mock` | none: `rah` prints an `UNGUARDED` warning |

`policy-authority` denies every tool without a host permission assignment in
its own configuration; tool metadata can only make a decision stricter. The
presets assign `echo` (`none`), `fs.read` and `repo.*` (`read`), allow `none` and `read`,
and ask for `write` and `execute`. `frontend-cli` asks on the terminal and
approves only an explicit `y`; end of input denies. If a configured policy
plugin fails to start, `rah` refuses to run.

General program execution is opt-in. Add `tools-shell` to a distribution file
next to `sandbox-process` (process supervision, not an OS sandbox), and assign
`shell.exec` in the policy:

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { echo = "none", "fs.read" = "read", "shell.exec" = "execute" } }

[[plugin]]
use = "sandbox-process"
config = { root = "." }            # cwd bound; cleared environment

[[plugin]]
use = "tools-shell"
config = { allow_programs = ["git", "cargo"] }   # optional allow list
```

`shell.exec` runs one program with an argument vector (no shell syntax), with
a timeout, bounded output and process-tree termination. With the mock model,
`/call shell.exec {"program":"git","args":["status"]}` exercises it.

MCP servers (stdio transport) are mounted one `tools-mcp` instance per
server. Only the tools listed in `tools` are exposed, as
`mcp.<server>.<tool>`, with the permission level written there; the policy
must assign them as well:

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { "mcp.files.*" = "read" } }

[[plugin]]
use = "tools-mcp"
id = "mcp-files"
config = { server = "files", program = "my-mcp-server", args = ["--stdio"], tools = { read_file = "read" } }
```

The server is started through `sandbox-process` (cleared environment,
working directory inside the sandbox root) and ends with the plugin.

Plugins can also be separate processes in any language, speaking JSON-RPC 2.0
on stdio ([protocol v2](docs/PROCESS_PLUGINS.md)). The Python example
[`word_tools.py`](examples/process-plugins/word_tools.py) provides two tools
and reads the session log through the host, which allows only the services
the plugin lists in `requires`:

```toml
[[plugin]]
use = "process"
id = "word-tools"
config = { program = "python", args = ["examples/process-plugins/word_tools.py"] }
```

`--web` serves a browser UI instead of the terminal REPL:

```powershell
cargo run -p rah-cli -- --preset dev --web            # http://127.0.0.1:8765
cargo run -p rah-cli -- --preset dev --web --bind 127.0.0.1:0
```

`rah` prints the address with a per-start access token; open exactly that
URL. The UI lists, opens, creates and forks sessions, streams turns, and asks
you to allow or deny tool calls the policy marks `Ask`. Its Plugins panel
adds, configures, restarts, replaces and removes plugins while `rah` runs:
replacing `model-mock` with `model-llamacpp`, or one loop with another, takes
effect on the next turn without a restart. It listens on loopback only and
has no external resources.

Real models use the same composition with a different model plugin:

```powershell
# llama.cpp: start llama-server on http://127.0.0.1:8080 first (loopback only).
cargo run -p rah-cli                                 # default preset, sole served model
cargo run -p rah-cli -- --model qwen

# OpenAI: the key is read from the backend environment, never from config.
$env:OPENAI_API_KEY = '...'
cargo run -p rah-cli -- --preset openai --model your-model-id
```

The model plugins are verified against deterministic loopback fixtures only,
not against a live llama-server or the live OpenAI API.

Compose your own plugins with a distribution file:

```toml
# rah.toml — run with: rah --config rah.toml
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.read" = "read" } }

[[plugin]]
use = "session-jsonl"
config = { dir = ".rah/sessions" }

[[plugin]]
use = "tools-builtin"

[[plugin]]
use = "model-mock"
id = "model-a"            # instance id; defaults to the `use` name
config = { label = "A" }

[[plugin]]
use = "loop-react"            # or "loop-plan": plan first, then execute step by step
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

## Validate

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The normal suite uses deterministic local fixtures and needs no network access,
credentials, paid API or real model. Some tests need `git` on PATH; the Python
process-plugin example runs only when a Python 3 interpreter is on PATH.

## Contributing

Read [AGENTS.md](AGENTS.md) before changing code. It defines the kernel,
plugin and trust rules for `master`.
