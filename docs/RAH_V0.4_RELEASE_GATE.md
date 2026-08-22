# RAH v0.4 release-gate audit

Status: **READY FOR RELEASE PREPARATION COMMIT**

Date: 2026-08-22

This report records the v0.4 local release boundary and required verification.
It does not create a tag, publish a release, push, or claim CI validation of the
release-preparation commit.

## 1. Release boundary

RAH v0.4 adds a trusted-host static capability profile that atomically composes
existing built-in capabilities and hardened local external Tool providers into a
fresh `ToolRegistry`. It uses explicit permissions, symbolic host resources,
source/static and effective validation, provider lifecycle ownership, and
redacted authority inspection.

The authority path is:

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

Profiles configure existing authority; they do not create generic authority. A
model request and provider metadata remain non-authoritative. The profile is
not an `AgentRuntime`, and the optional Codex adapter remains downstream of the
host-supplied registry.

## 2. Implemented capability inventory

### Trusted profile

- Hardened trusted profile source validation, strict versioned static profile
  parsing, closed symbolic host resources, built-in capability profile
  composition, and redacted static/effective inventory.
- `rah profile validate <absolute-profile>`: non-spawning static/source/schema/
  resource validation.
- `rah profile validate-effective <absolute-profile>`: explicit effective
  composition that may launch configured trusted provider processes, performs
  handshake/discovery/admission, and returns a redacted effective inventory.

### Preserved v0.3 capabilities

- Generic Tool Bridge, `fs.read`, MCP Tool adapter, and Process Plugin Tool
  adapter.
- Hardened Execute policy and `RepositoryMutationPolicy`.
- `host.cargo.version`, `host.git.status`, `host.git.stage`, and
  `host.git.unstage`.

These are retained v0.3 capabilities; v0.4 profiles compose existing authority
and do not represent them as newly introduced generic execution capability.

## 3. Provider hardening

### MCP

Implemented and deterministically verified: local stdio MCP; native executable
validation/revalidation; isolated cwd; cleared/minimized environment; bounded
queues, messages, results, and stderr; bounded lifecycle timeouts; exact
expected tool-set and normalized schema admission; explicit host permission
mapping; atomic provider load; and trusted-profile composition.

Deferred: MCP Streamable HTTP, network MCP, provider installation/download,
automatic restart, and hot reload. Codex-owned MCP remains disabled.

### Process Plugin

Implemented and deterministically verified: RAH Process Plugin protocol `1`;
bounded NDJSON stdio; host-configured plugin identity; native executable
validation/revalidation; isolated cwd; minimized environment; bounded
lifecycle/resources; exact expected tool-set and normalized schema admission;
explicit permission mapping; atomic provider load; and trusted-profile
composition.

Deferred: `PluginManager`, plugin installation/download, automatic restart,
hot reload, and a generic plugin platform.

## 4. Effective composition verification

The real effective composer has deterministically verified mixed built-ins +
MCP + Process Plugin composition. It constructs a fresh registry, preserves
declared permissions, validates a redacted inventory, and publishes no partial
registry when admission fails. Staged providers are cleaned up after a later
failure, and successful effective profiles retain lifecycle ownership.

Registry duplicate registration fails closed. Provider-qualified names and
validated provider IDs structurally exclude the tested external naming
collisions; any remaining duplicate registration is rejected rather than
replaced. Static validation is non-spawning; effective validation is explicitly
spawning.

## 5. Trusted profile source and security non-guarantees

On Windows, source validation fails closed for relative paths, UNC where
unsupported, verbatim/device paths, ADS, lexical aliases, symlink/junction/
reparse points, and non-regular sources. Native external providers require
`.exe`; `.cmd` and `.ps1` forms are rejected. Executable identity is
canonicalized/revalidated before spawn.

This is not proof of exclusive ACL ownership or an OS-level trusted store. RAH
does not eliminate source or executable filesystem TOCTOU races, provide OS
sandboxing or network isolation, or guarantee rollback. Provider cancellation,
timeout, and lifecycle supervision cannot undo an external effect.

## 6. Architecture and ADR status

The accepted ADR inventory is:

1. 0001 runtime abstraction
2. 0002 Codex adapter
3. 0003 tools are extension boundary
4. 0004 no inference engine
5. 0005 Codex app-server runtime boundary
6. 0006 Codex dynamic tool bridge
7. 0007 RAH MCP tool adapter
8. 0008 Process Plugin adapter
9. 0009 Execute process policy
10. 0010 Repository mutation policy
11. 0011 Trusted capability profile authority boundary

No new ADR or public architecture boundary is introduced by release
preparation. `rah-protocol` remains dependency-bottom and no provider/Codex
types cross RAH public boundaries.

## 7. Deterministic release gate

The release-preparation commit must pass the following local commands before
tagging. The normal suite is deterministic and requires no model, credentials,
network, paid API, or GPU.

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
```

The metadata command must resolve each inherited workspace package to `0.4.0`.

## 8. Live validation carried forward

Task 043 passed the opt-in trusted-profile path using exactly `codex-cli
0.149.0`:

```text
profile id: live-trusted-profile-bridge
provider: Process Plugin test
tool: plugin.test.echo
permission: PermissionLevel::None
private alias: rah_tool_0
```

It observed exactly one `ToolRequested`, `ToolStarted`, `ToolFinished`, and
provider execution, followed by:

```text
Started
-> ModelRequestStarted
-> ToolRequested
-> ToolStarted
-> ToolFinished
-> ModelDelta...
-> Completed
```

The final marker was `RAH_TRUSTED_PROFILE_LIVE_OK`. The Codex app-server and
Process Plugin child were reaped. The evidence contains no machine-specific
absolute paths.

## 9. Platform status

Windows is the primary verified development and release-validation platform.
Verification includes trusted profile source validation, native `.exe` provider
identity, `.cmd`/`.ps1` rejection, reparse/symlink handling, process
supervision, mixed-provider deterministic validation, and the live Codex
trusted-profile validation. Residual process/filesystem races remain documented.

Unix-specific executable/source tests exist where platform-gated but were not
executed on the Windows release-validation host. v0.4 is therefore a
Windows-verified baseline, not a claim of cross-platform live verification.

## 10. Explicit deferrals

- Profile auto-discovery, hot reload, editing/mutation, generic provider schema,
  and generic subprocess schema.
- MCP Streamable HTTP and network MCP.
- `PluginManager`, provider/plugin installation/download, automatic restart,
  and generic plugin platform.
- Generic `shell.exec`, generic `process.exec`, and model-selected executable,
  argv, cwd, or environment.
- `host.git.restore-worktree`, arbitrary worktree mutation, Git commit,
  refs/history mutation, network Git, and credential-bearing Git.
- New model-facing profile APIs, OS sandboxing, network isolation, and rollback
  guarantees.

## 11. Release decision and checklist

- [ ] Workspace version and lockfile resolve to `0.4.0`.
- [ ] Release-preparation-only diff reviewed; no credentials, temporary
  profiles, target artifacts, or unrelated dependency upgrades.
- [ ] Deterministic release commands pass before commit.
- [ ] One release-preparation commit is clean and the commands pass again.
- [ ] CI is green, then a `v0.4.0` tag and published release are separately
  authorized actions.

With the carried-forward Task 043 live evidence and passing deterministic
release commands, RAH v0.4.0 is ready for release preparation only. Tagging,
publishing, and pushing remain outstanding.
