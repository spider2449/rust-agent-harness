# RAH — Rust Agent Harness

RAH is a model-provider-agnostic, runtime-pluggable agent harness written in Rust.
It owns neutral runtime, model, event, session, tool, permission, and sandbox
boundaries. RAH orchestrates inference providers; it is not an inference engine
and does not load model weights or implement model execution.

## v0.15.0 release preparation: bounded repository directory creation

RAH v0.15.0 is prepared but not yet released. It adds the separate,
host-owned `repo.create-directory` capability described below. The v0.14
bounded file rename/move capability remains distinct.

RAH v0.10.0 established the existing bounded Desktop host configuration.
Desktop selects a certified `codex-cli 0.149.0` baseline, accepts one
human-selected bounded `llama_cpp` endpoint under ADR 0015, and retains saved
model preferences as inactive desired state until an explicit Connect or
reconnect. It neither manages a llama.cpp process nor installs a provider or
model.

The selected repository is canonical host-owned context: native Git discovery
and observation are fixed to that repository, Codex starts with its verified
repository CWD (or an app-owned neutral workspace), and launch-CWD/`AGENTS.md`
context cannot substitute it. Conversations are stored privately in
repository-scoped SQLite namespaces; Resume is explicit bounded display/context
replay and never restores repository, model, tool, or other authority.

Remote llama.cpp generation proof remains **DEFERRED / NOT VALIDATED**. A
bounded initial endpoint is not transport confinement: redirect, proxy, DNS,
peer-identity, and effective-destination guarantees are **NOT CLAIMED**.

### Desktop authoring, deletion, review, and bounded repository commit

The v0.14 candidate retains the existing bounded Desktop local repository
workflow:

```text
inspect -> model bounded authoring -> human Stage / Unstage -> host-observed staged review -> human reviewed-snapshot authorization -> message-only bounded commit -> verified result / refresh
```

The model-visible commit Tool is `repo.commit`, with the closed input
`{"message":"..."}`. It requires `PermissionLevel::Execute` as an outer gate,
but Execute alone is not repository-history authority. A trusted profile must
compose the exact host-selected repository, native Git executable, and identity,
and the host must separately authorize one fresh reviewed staged snapshot for
each call. Stage / Unstage are host actions, not model Tool authority; the
frontend presents host-owned state but does not own authorization. The
authorization is in-memory, one-shot, and never restored or replayed.

`repo.commit` creates at most one ordinary commit from the already staged
snapshot on the current attached branch. It never stages files, accepts no
branch/ref/path/Git argv/identity input, and grants no amend, merge, rebase,
cherry-pick, tag, remote, credential, network-Git, or generic Git authority.
Uncertain external effects are not retried or rolled back. Windows local live
validation is certified with the complete official Codex 0.149.0 runtime
(including its same-version code-mode host); Ubuntu CI is deterministic
evidence, not Linux live certification.

The v0.13 release added `repo.delete-file`, a separate ADR 0017 authority for
deleting exactly one explicitly named repository-relative regular file. The
target must be clean, HEAD-tracked, and match the exact authorized HEAD blob
preimage, including SHA-256 and byte length. The operation makes one native
worktree deletion attempt, never auto-stages, and does not grant commit, ref,
history, or network Git authority. Trusted Profile composition and the Generic
Codex Tool Bridge can expose the capability only when the host has already
constructed the separate deletion authority; neither model requests,
provider metadata, Execute permission, tool definitions, nor the frontend can
manufacture it. The canonical public tool name is `repo.delete-file`; any
provider-private alias is an implementation detail.

Windows live-certified v0.13 evidence used `codex-cli 0.149.0` and observed
one request, start, and finish, verified deletion of the intended target, an
unchanged sentinel and index, an unstaged deletion, unchanged HEAD/refs/
history, no replay, and `RAH_REPO_DELETE_FILE_LIVE_OK`. Linux live
certification is not established.

RAH v0.14 adds `repo.rename-file` under accepted ADR 0018. It moves exactly
one clean, HEAD-tracked regular file within the selected repository, either in
the same directory or to another existing directory in that repository. The
request supplies `source_path`, `destination_path`,
`expected_source_file_sha256`, and `expected_source_file_byte_length`. The
destination must be absent and is never overwritten. The host revalidates
repository and runtime-generation identity immediately before one native
no-replace rename/move attempt; a possible effect is never replayed. This is
not generic filesystem rename authority, and it grants no create, delete,
content-write, index, commit, shell, process, or Git authority.

RAH v0.15 adds `repo.create-directory` under accepted ADR 0019. It creates
exactly one new ordinary directory leaf at an explicit repository-relative
path. The parent must already exist and the destination must be absent. This
separate `RepositoryDirectoryCreationPolicy` does not recursively create
parents, ensure an existing directory, create placeholder files, or mutate
Git. A possible effect is never replayed or rolled back.

## Preserved bounded repository mutation and workflow inspection

RAH retains the v0.4 trusted-host static capability profile and the v0.5
separate, accepted worktree-content authority: `repo.patch`. It can
conditionally perform a legacy single exact replacement or one to sixteen
bounded exact replacements within one existing, HEAD-tracked, unstaged,
strict-UTF-8 worktree file. All matches use the same original snapshot;
overlapping replacements are refused and non-overlapping replacements are
applied deterministically. Full-file SHA-256 and byte-length preconditions are
required, and the operation does not stage changes. The capability is
host-constructed through a private `RepositoryWorktreeMutationPolicy`; it is
not generic filesystem write, shell/process, index, or Git history authority.

RAH additionally provides four host-fixed, read-only repository observers:
`repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`. They
inspect one validated repository-relative path, normalized repository status,
unstaged worktree-versus-index changes, and staged index-versus-HEAD changes.
They are `Execute`-gated subprocess capabilities, not generic Git or filesystem
authority; model input cannot select their executable, argv, cwd, environment,
repository, refs, or baselines. Their precise claim is **no intentional
repository mutation**, not zero incidental filesystem writes.

RAH v0.8.0 adds `repo.create-file`: one host-authorized
exclusive creation of one absent UTF-8 file at a model-selected, validated
repository-relative path. It uses a separate private
`RepositoryFileCreationPolicy` (ADR 0013), requires an existing real parent,
rejects links/reparse traversal, ignored/index/HEAD/submodule/sparse targets,
and never overwrites, creates directories, appends, stages, or mutates Git
history or refs. One call creates one file only: paths are limited to 1024
UTF-8 bytes, content to 256 KiB, and the serialized request to 320 KiB. It is
not generic filesystem-write authority and provides no rollback or replay.

RAH v0.9.0 adds `repo.edit-files`: one through four existing, clean,
HEAD-tracked strict-UTF-8 files with exact SHA-256 and byte-length
preconditions, all replacements resolved against original snapshots, and
deterministic host-owned commit order. It is not transactional and provides no
rollback or replay. The capability is composed through Trusted Profile v1 and
the Generic Tool Bridge; certified Windows live validation using exactly
`codex-cli 0.149.0` emitted `RAH_REPO_EDIT_FILES_LIVE_OK`.

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

### Trusted `repo.patch` profile binding

The existing hardened `repo.patch` capability can be requested only through a
trusted static profile using host-owned symbolic resources. Its narrow profile
entry is:

```json
{
  "resources": {
    "executables": {
      "git": { "path": "C:\\host-tools\\git.exe", "kind": "native" }
    },
    "repositories": {
      "worktree": { "path": "C:\\host-worktrees\\project" }
    }
  },
  "capabilities": [
    {
      "name": "repo.patch",
      "enabled": true,
      "permission": "execute",
      "executable": "git",
      "repository": "worktree"
    }
  ]
}
```

`execute` is the existing outer permission because the capability performs
bounded host-owned Git observations. It is necessary but not sufficient:
`PermissionLevel::Execute` is not generic worktree-write authority. During
effective composition the host resolves the two symbolic resources and invokes
the existing `RepositoryWorktreePatchTool` constructor; that constructor alone
creates the private `RepositoryWorktreeMutationPolicy`. The profile cannot
deserialize, construct, configure limits for, or bypass that policy.

The entry accepts no raw repository root, Git command/argv, shell command,
environment, mutation-policy settings, or filesystem write scope. The
repository resource must pass the existing constructor's canonical worktree,
repository identity, and confinement checks. `repo.patch configured` therefore
does not mean arbitrary writes are authorized, and a model call remains only a
request that must still pass `ToolRegistry`, runtime permission, and the private
policy's deterministic eligibility checks. This binding neither alters nor
substitutes for `WorkspacePolicy`; each capability retains its own applicable
host policy.

`rah profile validate` checks this entry's source/schema/symbolic-reference
shape only. It neither constructs `repo.patch`, runs Git, nor reads or mutates
worktree content. `rah profile validate-effective` resolves resources and may
perform the bounded non-mutating host-side construction/inspection needed to
register `repo.patch`; it never invokes the tool. Both inventories show only
the logical capability name, `Execute` permission, symbolic resource IDs, and
validation state, never native paths, file data, hashes, temporary paths,
policy internals, or environment. ADR 0012 is accepted; its worktree authority
remains separate from ADR 0010's index-only policy and ADR 0011's
composition-only profile boundary.

### Trusted `repo.create-file` profile binding

`repo.create-file` uses the same closed symbolic-resource shape as
`repo.patch`, but it constructs its separate private creation policy under
accepted ADR 0013:

```json
{
  "capabilities": [
    {
      "name": "repo.create-file",
      "enabled": true,
      "permission": "execute",
      "executable": "git",
      "repository": "worktree"
    }
  ]
}
```

The model supplies only the closed `{ "path", "content" }` request. Static
profile validation and effective composition are non-mutating; the latter
creates a fresh `ToolRegistry`. `Execute` is only the outer dispatch gate, and
neither model/provider metadata nor the profile can supply a repository root or
escalate creation authority.

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

# Certified isolated live-gate wrapper (pins the approved model/config surface)
.\scripts\codex-live-gate.ps1 -Command { cargo run -p rah-runtime-codex --example live_echo_bridge }

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

# Bounded repository worktree replacement (opt-in live validation)
cargo run -p rah-runtime-codex --example live_trusted_profile_repo_patch_bridge

# Bounded repository file creation (opt-in certified live validation)
cargo run -p rah-runtime-codex --example live_trusted_profile_create_file_bridge
```

The MCP and process-plugin commands exercise RAH-owned adapters. They do not
enable Codex-owned MCP, shell, or file capabilities.

### Host-attested live markers

Live examples record final assistant text as diagnostic output, but it is not
release-gate authority. A marker such as `RAH_ECHO_BRIDGE_OK`,
`RAH_REPOSITORY_OBSERVERS_LIVE_OK`, `RAH_MULTI_PATCH_LIVE_OK`, or
`RAH_CREATE_FILE_LIVE_OK` is emitted by
the host harness only after it has observed the required tool lifecycle,
validated tool outputs and state postconditions, observed `Completed`, and
cleaned up the app-server child. Model-generated marker text is weaker than
host-observed execution state: a model request or statement is not execution
evidence and never overrides the host-owned ToolRegistry, policy, or sandbox
boundaries.

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

## Actions Cleanup

The repository-owned [Actions Cleanup](.github/workflows/actions-cleanup.yml)
workflow runs weekly on Sunday at 03:00 UTC. It retains the newest 20 completed
runs for each workflow and preserves runs whose head commit is tagged. Run it
manually with **Actions Cleanup** > **Run workflow**; `dry_run` defaults to true
and lists candidate run IDs without deleting them. Scheduled runs delete eligible
old records. The workflow uses only the repository `GITHUB_TOKEN` with
`actions: write` and `contents: read` permissions.

## v0.10 limitations and explicit deferrals

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
- `repo.patch` is limited to a legacy single exact replacement or a bounded
  `replacements` array of one to sixteen exact replacements in one existing,
  HEAD-tracked, unstaged strict-UTF-8 worktree file. It has no automatic
  staging, file creation/deletion/rename, multi-file transaction, Git commit,
  refs/history mutation, reset, clean, checkout, switch, stash, merge, rebase,
  push, pull, fetch, network Git, or credential-bearing Git execution authority.
  Destructive worktree authority remains constrained by the private policy
  described in accepted ADR 0012; ADR 0011 is composition-only.
- `repo.create-file` creates only one previously absent UTF-8 file per call at
  an existing validated parent. It has no overwrite, mkdir, append, delete,
  rename, chmod, binary-file, multi-file transaction, staging, commit/history/
  ref mutation, rollback, or automatic replay authority. Partial files can
  remain after a possible effect and are reported conservatively; ADR 0013 is
  the separate accepted creation authority.
- `repo.delete-file` deletes only one clean HEAD-tracked regular file whose raw
  bytes match the exact authorized HEAD blob preimage, including SHA-256 and
  byte length. It is a separate ADR 0017 authority, leaves deletion unstaged,
  and has no directory/recursive, untracked, rename/move, generic filesystem,
  staging, commit, ref/history, or network Git authority.
- `repo.rename-file` moves only one clean HEAD-tracked regular file within the
  selected repository under ADR 0018. It has no directory/recursive move,
  overwrite, case-only Windows rename, generic filesystem, shell/process, or
  Git authority; create and delete remain separate authorities.
- `repo.create-directory` creates exactly one new ordinary directory leaf under
  ADR 0019. Its parent must already exist and its destination must be absent.
  It has no recursive mkdir, ensure-directory, placeholder/.gitkeep, implicit
  file creation, generic filesystem, shell/process, Git, retry, replay, or
  rollback authority. A clean Git status is expected because Git does not
  track empty directories; filesystem postconditions prove creation.
- Repository observers are best-effort point-in-time observations, not a
  snapshot transaction or cross-process lock. They provide no intentional
  mutation authority, file creation/deletion/rename, generic patches/hunks,
  commit/history, or network Git authority. Live observer validation is Windows
  only at exactly `codex-cli 0.149.0`; Unix live Codex validation is unverified.
- Process supervision is not OS sandboxing; RAH makes no network-isolation or
  rollback guarantee. Timeout/cancellation may leave uncertain effects, which
  are never automatically replayed.
- OS sandboxing, network isolation, and rollback guarantees are not provided.
- Desktop has no llama.cpp process management, provider/model installation,
  generic network Tool, network MCP/Streamable HTTP, generic shell/process
  authority, model-selected executable/cwd/endpoint, or automatic authority
  restoration. It grants no Git commit/ref/history authority or generic
  repository delete/rename authority. Moving or renaming a repository
  intentionally changes its conversation-persistence namespace.
- SQLite is private Desktop storage, not a model/tool SQL capability. Resume
  replays only bounded completed text after a fresh connection; it has no
  rollback guarantee for uncertain external effects.
- Interactive approvals, TUI/web UI, multi-agent orchestration, RAG, and
  long-term memory remain out of scope.

See [Architecture](docs/ARCHITECTURE.md), [Security](docs/SECURITY.md), and the
accepted [ADRs](docs/adr/).
