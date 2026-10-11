# RAH v0.4 Trusted-Profile Milestone Audit

Date: 2026-08-22
Audited checkpoint: `eb2bf25 feat: add trusted Process Plugin profile composition`

## Recommendation

**A. READY FOR V0.4 RELEASE PREPARATION**

Task 043 closed the final live blocker with an opt-in actual-composer run. An
explicit absolute temporary trusted profile loaded through `TrustedStaticProfile`
selected a copied native `rah-plugin-echo` fixture by symbolic resource. The
shared `rah profile validate-effective` composer admitted exactly
`plugin.test.echo` with profile-assigned `PermissionLevel::None`, retained
provider ownership, and yielded a one-tool fresh registry and redacted inventory.
That registry was passed directly to the Generic Codex Tool Bridge, whose private
alias was `rah_tool_0`.

Using exactly `codex-cli 0.149.0`, the live run observed one ToolRequested, one
ToolStarted, one ToolFinished, one fixture provider call, ModelDelta continuation,
and terminal Completed. The final marker was exact. Fixture lifecycle observation
recorded `spawn -> call -> shutdown -> exit`; the Codex app-server shutdown also
completed. The live output contains only profile/provider/tool identities and
counts, not paths, temporary directories, environment, stderr, or raw profile
content.

## Actual boundary

**IMPLEMENTED:** an operator supplies one absolute profile path to
`TrustedStaticProfile::load`. The source reader validates bounded UTF-8 regular
files and rejects unsupported/link/reparse forms; strict v1 parsing validates
closed symbolic resources and declarations. Static construction creates a fresh
built-in registry and redacted inventory. `rah profile validate` stops here.

**IMPLEMENTED:** `rah profile validate-effective` calls the CLI-private effective
composer. It resolves symbolic executables, invokes the hardened MCP adapters
first and Process Plugin adapters second, gives each exact expected name/schema
and explicit permission, registers admitted immutable proxies into a fresh
registry, then marks the inventory validated. The returned composition owns
the adapters and shuts plugins down before MCP adapters.

**VERIFIED:** model input, repository discovery, and provider metadata have no
path to select or mutate profile authority. The profile schema has no raw argv,
cwd, environment, shell, network, Git-ref/history, or arbitrary filesystem
mutation fields. Unknown capability/resource/permission/schema/version fails
closed.

**VERIFIED:** deterministic mixed-provider cleanup plus an opt-in live
profile-to-Codex bridge validation.

**DEFERRED:** reload/auto-discovery, PluginManager/install/download/restart,
network or Streamable HTTP MCP, generic subprocess profiles, model-facing
profile APIs, and any new mutation authority.

## ADR 0011 fulfillment

| Invariant | Audit result |
| --- | --- |
| Trusted-host explicit source, strict version/content/source validation | VERIFIED by `rah-tools` source/profile tests and static CLI tests. |
| Existing-authority-only closed schemas | VERIFIED by the DTO fields and denied unknown fields. |
| Adapter-local exact admission and explicit permissions | VERIFIED per MCP and Plugin deterministic suites. Missing permission is rejected; `None` is only accepted when explicitly configured. |
| Fresh registry / no replacement on failure | VERIFIED. The composer returns its registry only on full success; CLI prints inventory only after success; mixed late-failure cleanup is deterministically observed. |
| Provider lifecycle ownership | VERIFIED. The composition owns adapter vectors and exposes a shared registry handle without releasing provider ownership; deterministic mixed ownership and the live fixture shutdown are observed. |
| Redacted host inspection | VERIFIED for static and CLI failure cases; MCP-only profile redaction is tested. Mixed success output is structurally redacted but lacks a sentinel test. |
| Immutability and no auto-reload | VERIFIED by absence of reload/watch/discovery paths and closed CLI commands. |

## Composition, cleanup, and collisions

The current fixed construction order is all MCP providers, then all Process
Plugin providers. Therefore “Plugin valid, later MCP fails” is not an executable
scenario in this implementation; its equivalent is a staged MCP followed by a
failing Plugin. On connection or registration failure, the composer shuts down
the just-created adapter and then staged plugin/MCP adapters in reverse groups.
No partially created registry is returned or rendered.

Possible collisions include built-in/MCP, built-in/Plugin, MCP/MCP,
Plugin/Plugin, and MCP/Plugin because provider-local names are prefixed but IDs
and remote names can still coincide. `ToolRegistry::register` rejects them and
the composer reports `DuplicateRegistration`; it does not replace a tool.
This behavior is implemented and mixed late-failure cleanup is observed by the
Task 041 deterministic fixture matrix.

Provider initialization/handshake/discovery/schema/permission failures are
deterministically covered inside each adapter. The Task 041 matrix verifies an
earlier staged provider is reaped after a later provider fails, and that all
provider-backed tools remain usable until successful composition shutdown.

## Permission and redaction audit

Both provider kinds follow declaration -> `ExternalToolIdentity` -> explicit
`PermissionLevel` -> exact admission -> `ToolRegistry`; runtime/bridge dispatch
continues to enforce the registered definition permission. MCP and Plugin tests
cover absent assignments and contradictory provider metadata. Built-in profile
contracts retain their fixed Read/Execute mappings.

Profile, CLI, and adapter error surfaces use bounded generic messages. Effective
inventory contains profile/provider IDs, names, permissions, status, and
symbolic resource IDs, not executable/profile paths, argv, cwd, environment,
stderr, tokens, or raw profile data. MCP/Plugin diagnostics are host-only and
not `ToolOutput`. Mixed successful inventory should receive the sentinel test in
blocker 1.

## Bounds and platforms

MCP code still enforces: 1 MiB message/result/output, 64 KiB stderr, queue 64,
outstanding 32, retired IDs 64, initialize/discovery 2 s, default call 30 s,
and shutdown 500 ms. Process Plugin enforces the same 1 MiB/64 KiB/64/32 hard
ceilings and 2 s startup/discovery, 30 s default call, and 500 ms shutdown;
its control queue is fixed at 64 and retired-request capacity is adapter-local.
Profile composition does not override these limits.

**Windows VERIFIED on this host:** profile source tests cover relative, UNC,
verbatim/device, ADS, lexical alias, link/reparse handling; MCP and Plugin tests
cover native `.exe` admission and `.cmd`/`.ps1` and reparse rejection. Native
paths are canonicalized/revalidated rather than trusted by raw equality. The
remaining check-to-spawn and source-path TOCTOU windows are documented; no
strong OS-isolation claim is made.

**Unix IMPLEMENTED but not live-verified here:** Unix-gated tests cover regular,
executable, non-directory, and symlink behavior where applicable. They were not
executed on this Windows audit host. This does not block a Windows v0.4
milestone, but a cross-platform release claim requires Unix CI/host execution
before it is made; it is not folded into the two blockers above.

## CLI, Codex, public API, and deferred authority

`profile validate` is static/non-spawning; `validate-effective` is explicitly
spawning. README, clap help, and CLI tests state that distinction. The static
test does not use a spawn sentinel, but the call graph contains no adapter
construction.

Task 042 selected the exact `codex-cli 0.149.0` baseline because no trusted native
0.148.0 executable is available in the established local validation setup. Its
generated app-server schemas retain every captured required lifecycle and
dynamic-tool field; observed contract changes are additive only. The adapter
constant, captured contract, README, and architecture documentation use that exact
pin. No provider dependency enters
`rah-protocol`, core, runtime, or the production Codex adapter. Actual v0.4
edges are `rah-tools -> serde` (profile DTO parsing), Unix-only `libc` (source
hardening), and `rah-cli -> rah-tools-mcp`/`rah-tools-plugin` (host effective
composition). Profile DTOs are host-facing `rah-tools` types; no model protocol
or Codex/provider DTO crosses a RAH public protocol boundary.

Absent and still deferred: generic shell/process profiles; model-selected
executable/argv/cwd/env; worktree restore/arbitrary mutation; commit/ref/history
or network/credential Git; network/HTTP MCP; PluginManager/install/download;
automatic restart; reload/hot reload/auto-discovery; and model-facing profile
APIs.

## Milestone definition

RAH v0.4 introduces a trusted-host static capability profile that atomically
composes existing built-in capabilities and hardened local external Tool
providers into a fresh ToolRegistry, with explicit permissions, symbolic host
resources, non-spawning static validation, explicit effective provider
validation, lifecycle ownership, and redacted authority inspection.

It does not provide generic process authority, provider management, remote MCP,
profile reload, or a model-facing profile API.

## Repository checkpoint and working tree

Verified history includes, in order: `3a7ccc6`, `80bf43e`, `9ebbe05`,
`72da310`, `ec32499`, and `eb2bf25`. The initial working tree was clean.
The reported unstaged `.gitignore` entry for `target-task037/` is absent: the
current file contains only `/target/`. No `.gitignore` change was made or
committed; if it reappears as local build-output housekeeping it should remain
uncommitted for this milestone unless separately requested.
