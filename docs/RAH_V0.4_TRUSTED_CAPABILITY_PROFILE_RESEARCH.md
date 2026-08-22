# RAH v0.4 trusted capability profile research

Status: **PLANNED — design research only**
Date: 2026-08-22
Baseline: RAH v0.3.0, Rust edition 2024, `codex-cli 0.149.0`

Status terminology is intentional. **VERIFIED** means the cited v0.3 source
and its recorded deterministic or named opt-in live evidence support the claim.
**IMPLEMENTED** means it exists in the v0.3 baseline. **PLANNED** is a
recommendation for a later task, not code. **DEFERRED** is excluded from this
increment.

## 1. Problem statement

**VERIFIED / IMPLEMENTED.** RAH already has host-constructed `fs.read`, the
bounded `host.cargo.version`, `host.git.status`, `host.git.stage`, and
`host.git.unstage` capabilities, plus stdio MCP and Process Plugin adapters.
Their model path is still `ToolCall -> ToolRegistry -> permission/policy ->
Tool -> ToolOutput`. `HostExecutionPolicy` and `RepositoryMutationPolicy`
constrain the host Execute and index-mutation capabilities; external tool
permissions are default-deny.

**PLANNED problem.** An application should be able to compose those existing
capabilities from an operator-controlled profile instead of bespoke host code,
then show the operator the authority actually registered. This is configuration
of host authority, not a capability language and not a new authority source for
the model.

The design must preserve the existing capability constructors and policies. A
profile cannot select a command from model input, discover authority from a
repository, or turn a discovered MCP/plugin description into permission.

## 2. Authority model

**PLANNED.** The trusted operator deliberately selects one profile through a
trusted launch/configuration mechanism. The profile names only supported,
already-implemented capability kinds and trusted symbolic resources. The host
resolves those resources, validates them, constructs the existing capabilities,
and registers their normal RAH `ToolDefinition`s. At execution time, the normal
`ToolRegistry` permission gate and capability policy remain authoritative.

```text
trusted operator -> trusted profile -> validated host construction
                                      -> ToolRegistry -> model-visible schema
model ToolCall ---------------------------------------> registry permission
                                      -> capability-specific policy -> execution
```

A model may request a registered tool only with that tool's existing input
schema. It cannot load, amend, select, reload, or inspect raw profile values.
Repository files, MCP responses, plugin handshakes, and tool metadata are
untrusted inputs and cannot modify the effective profile.

**DEFERRED.** The profile is not generic `process.exec`, generic shell,
filesystem write, destructive worktree authority, Git commit/ref/history,
network Git, credentials, a PluginManager, or rollback/replay machinery.

## 3. Trust-source classification

**PLANNED.** Trust attaches to how a profile is selected and protected, not to
the profile syntax. A valid document from an untrusted location remains
untrusted.

| Source | Classification | Rule |
| --- | --- | --- |
| Explicit CLI/config path selected by a trusted user | Conditionally trusted | Accept only at process start; validate file identity and permissions as far as the platform can establish them. The user is responsible for selecting it. |
| Application-owned configuration directory | Conditionally trusted | Accept only when the directory/file passes platform-specific ownership, ACL, reparse-point, and regular-file checks. Otherwise fail closed. |
| Repository-local configuration | Forbidden as an authority source | It may be read only when a separately trusted outer profile explicitly names and pins it; it never autoloads and cannot add capabilities. Prefer not to support this in v0.4. |
| Environment variables | Forbidden for profile content | Do not accept a serialized profile, path expansion, executable, argv, cwd, permission, or secret through environment variables. A launch flag may select a trusted profile path. |
| Generated runtime configuration | Forbidden | Runtime/model/session state cannot synthesize, merge, or widen a profile. Trusted hosts may generate a file before launch, then it is treated as an explicit static file. |
| Plugin-provided configuration or metadata | Forbidden | Plugin identity/version may be verified against host configuration, but plugin data never configures its own executable, permission, environment, or registered authority. |
| Model-provided configuration | Forbidden | Model content is never a profile, override, include, or reload request. |

No include/import, inheritance, environment interpolation, repository-relative
path resolution, or automatic search path belongs in version 1. Each one makes
authority provenance ambiguous. A later product may offer a trusted installer
or UI, but must produce the same explicit host-owned input boundary.

## 4. Proposed conceptual profile schema

**PLANNED.** Version 1 should be a strict, closed schema. A TOML or JSON
encoding is an implementation choice; examples use TOML for readability. The
parser must reject unknown fields, duplicate keys/entries, aliases, and
implicit defaults that affect authority. The profile schema is host-only and is
not a `ToolDefinition`, JSON Schema sent to a model, or RAH protocol message.

```toml
profile_version = 1
profile_id = "local-development"

[resources.executables]
trusted-cargo = { path = "C:/Program Files/Rust/bin/cargo.exe", kind = "native" }
trusted-git = { path = "C:/Program Files/Git/cmd/git.exe", kind = "native" }

[resources.repositories.rah-self]
path = "F:/coding/otherPrj/rust-agent-harness"

[resources.targets.release-manifest]
repository = "rah-self"
path = "CHANGELOG.md"

[[capabilities]]
name = "host.cargo.version"
enabled = true
permission = "execute"
executable = "trusted-cargo"
cwd_resource = "rah-self"

[[capabilities]]
name = "host.git.status"
enabled = true
permission = "execute"
executable = "trusted-git"
repository = "rah-self"

[[capabilities]]
name = "host.git.stage"
enabled = false
permission = "execute"
executable = "trusted-git"
repository = "rah-self"
target = "release-manifest"
```

`enabled = false` is documentation/inventory state only: it constructs and
registers nothing. A disabled declaration is allowed only if it is syntactically
valid; resource resolution may be skipped so a host can retain a deliberately
disabled future entry. An enabled declaration must have every required field,
and a capability's permission must exactly match its fixed required permission;
the profile cannot downgrade or elevate it.

The minimal supported v0.4 allowlist should be `fs.read`, `host.cargo.version`,
`host.git.status`, `host.git.stage`, `host.git.unstage`, configured stdio MCP,
and configured stdio Process Plugin providers. It must exclude `shell.exec`
and arbitrary `HostExecutionTool` descriptions. The fixed constructor owns each
capability's exact argv, timeout defaults/bounds, environment behavior, output
bounds, and mutation verification; a profile supplies only named, bounded
constructor inputs where the existing constructor requires them.

For example, `fs.read` may reference one workspace resource and a bounded
`max_bytes` value. It must not turn a model tool input into an absolute path.
For Git stage/unstage, a target reference maps to the one host-defined tracked
regular-file target required by the existing mutation policy. The model keeps
the current empty object input.

## 5. Symbolic resources versus raw paths

**PLANNED recommendation: use symbolic names at capability entries.** Raw
native paths appear only in a top-level trusted resource table. A capability
uses `repository = "rah-self"`, `target = "release-manifest"`, and
`executable = "trusted-git"`; it never repeats a path. Initialization resolves
the resource to canonical native identities and records an opaque resolution
record for use and revalidation.

This improves reviewability and avoids accidental drift between multiple
capabilities. It makes profiles portable at the intent layer: a copied profile
can retain names while its trusted local paths are edited and validated on the
new host. It does not make a copied profile automatically safe or portable:
resources must be revalidated, and an unresolved or identity-mismatched name
fails closed.

Store in configuration: symbolic identifiers, absolute literal resource paths,
capability-to-resource bindings, fixed literal argv, explicit environment
allowlist/value references, bounded limits, and expected protocol versions.
Derive at runtime: canonical paths, filesystem object identities, Git
repository/.git identities, executable native-file identity, platform support,
and redacted presentation values. Do not persist resolved temporary directories,
backup/recovery locations, hashes used only for diagnostics, or child stderr in
the profile.

An operator-facing error may say `repository resource "rah-self" could not be
resolved`; verbose trusted diagnostics may identify a redacted path token. This
avoids turning an absolute home/repository layout into routine output. Stale
resources do not fall back to `PATH`, a current directory, or a similarly named
resource.

## 6. Load and validation lifecycle

**PLANNED.** Profile initialization is a single all-or-nothing host action:

```text
load explicit source -> parse strict versioned syntax -> validate declarations
-> resolve trusted resources -> canonicalize/capture identities
-> validate capability-specific policy -> construct adapters/tools
-> register into a fresh ToolRegistry -> publish redacted effective profile
-> begin runtime
```

The old registry remains active until a replacement registry is completely
validated. For initial startup, any enabled-capability failure prevents runtime
start; do not silently omit it. This preserves the meaningful distinction
between an operator's requested authority and actual authority. Disabled
entries remain non-authority inventory records.

| Condition | Required behavior |
| --- | --- |
| Invalid syntax, unknown field, or unsupported profile version | Reject before resource access. |
| Unknown or unconfigured capability | Reject. No best-effort registration. |
| Duplicate capability name/provider ID/resource/target or conflicting declaration | Reject deterministically. |
| Missing executable/repository/target or stale symbolic reference | Reject the enabled profile. |
| Capability permission differs from fixed requirement | Reject. |
| Unsupported platform or non-native executable wrapper | Reject that enabled profile. |
| Repository/executable identity mismatch or unsafe link/reparse result | Reject. |
| MCP/plugin discovery returns an unassigned external tool | Fail adapter initialization; never register a partial discovered set. |
| ToolRegistry duplicate after construction | Reject the full profile. |

Partial loading is unsafe for version 1 because it can make configuration errors
look like a smaller intentional grant. Future optional groups would require an
explicit independently validated group model and are not recommended here.

## 7. Effective-profile inspection model

**PLANNED.** After successful construction, a trusted operator should be able
to request an immutable, redacted effective-profile inventory from the host.
It should include profile version and ID, source class (not raw source path),
capability/provider symbolic ID, enabled/registered/disabled state, fixed RAH
permission, symbolic resources, validation state, protocol expectations, and
platform availability. It should also report profile fingerprint and resource
identity fingerprints as non-reversible hashes when needed to correlate logs.

Three visibility levels are recommended:

| Audience | Content |
| --- | --- |
| Model | Existing registered `ToolDefinition`s only. No profile inventory, resource names unless already inherently in a tool name, paths, environment, or diagnostics. |
| Ordinary operator CLI | Redacted inventory: symbolic names, statuses, permissions, platform reason codes, and short fingerprints. |
| Trusted debug log | More failure context, still redacted; raw configuration and secrets are never required in normal logs. An explicit local secure diagnostic facility is deferred. |

The inventory must describe effective state, not merely requested input. A
failed load produces a bounded redacted diagnostic, no effective profile, and
no runtime. It must not include private paths, tokens, raw plugin arguments,
temporary directories, or recovery/audit paths.

## 8. Redaction and secret rules

**PLANNED.** Treat every profile field as potentially sensitive until assigned
an output class. In particular, do not put environment values, credentials,
tokens, authorization headers, cookies, private endpoints, full executable or
home paths, repository paths, MCP/plugin argv, config contents, child stderr,
or isolated cwd paths into model-visible `ToolOutput`, RAH events, model tool
descriptions, errors, or inspection intended for the model.

The parser must reject obvious secret-bearing fields outside the explicit future
secret mechanism: names matching credential/token/password/secret/key may not
be accepted as ordinary profile values, and unrestricted environment maps are
forbidden. This is a guardrail, not a reliable secret detector. Explicit
environment entries are exceptional host configuration, must be allowlisted by
name, redacted in all rendering, and should be prohibited for the initial MCP
profile increment unless an existing adapter constructor can accept them safely.

Raw literal executable/repository paths and non-secret fixed argv may be
available to the trusted launching process, but ordinary logs/inventory show
symbolic names and salted/non-reversible identity fingerprints instead. Error
messages use field name, symbolic identifier, and reason code, for example
`invalid executable resource "trusted-git": not a native executable`; they
never echo the supplied value. Query strings and userinfo make future HTTP
endpoints sensitive by default.

**VERIFIED / IMPLEMENTED.** Process Plugin stderr is already host-only, bounded
diagnostic data and must never become `ToolOutput`. The same rule applies to
profile parsing/validation errors and MCP/process startup output.

## 9. Permission-policy integration

**PLANNED.** A profile composes existing authority layers; it does not replace
or flatten them:

```text
profile enables a known capability
  + constructor validates HostExecutionPolicy / RepositoryMutationPolicy
  + ToolRegistry runtime permission check remains authoritative
  + capability-specific execution and postcondition policy remains authoritative
```

`PermissionLevel` retains its existing meaning. A profile declaration must
match a tool's fixed level, and the runtime's allowed permission set still
decides whether the registered tool can execute. `Execute` remains necessary
but insufficient. The profile cannot declare a generic Execute capability or
weaken executable identity, fixed argv/cwd/env, output limits, timeout,
revalidation, mutation lease, pre/postcondition checks, no-rollback, or
no-replay rules.

For MCP and plugins:

```text
profile configures one provider
  + ExternalToolPermissionPolicy assigns each expected discovered identity
  + adapter verifies identity/protocol and discovery
  + only explicitly assigned tools register
  + ToolRegistry permission remains authoritative
```

Missing permission is not `PermissionLevel::None`; it fails closed. Discovered
metadata never supplies a permission or a capability registration decision.

## 10. MCP configuration boundary

**VERIFIED / IMPLEMENTED.** The current MCP adapter is pinned to revision
`2025-06-18`, stdio only, direct-launches a configured program without a shell,
and adapts discovered tools through `ToolRegistry`. Streamable HTTP is deferred.

**PLANNED minimum profile entry.** A stdio MCP provider needs a unique symbolic
provider ID, native executable resource, a bounded fixed literal argv list,
exact MCP revision, explicit per-remote-tool RAH permission assignments,
startup and call timeouts bounded by host maxima, and message/output bounds.
It also needs an explicit working-directory policy: v1 should use a host-owned
isolated/non-repository directory or reject the provider when that cannot be
created. Inherited environment is cleared; the profile permits only a narrow
name allowlist with no values shown in inspection. Since the present adapter
does not yet expose every desired environment/cwd/startup-limit knob, Task 028
must either use its safe current defaults or stop and make a separately reviewed
adapter-local change. It must not silently invent a generic process layer.

The MCP provider cannot select its executable, argv, cwd, environment, or
permission. Its `tools/list` result is schema/metadata input, not authority.

## 11. Process Plugin configuration boundary

**VERIFIED / IMPLEMENTED.** The adapter uses pinned RAH Process Plugin protocol
version `1`, verifies configured/reported ID and version, clears the child
environment, creates an isolated temporary cwd, bounds IPC/stderr, and maps
only explicit `ExternalToolPermissionPolicy` assignments into RAH tools.

**PLANNED minimum profile entry.** It needs unique configured plugin ID,
expected plugin version, expected protocol version, native executable resource,
bounded fixed literal argv, explicit per-tool permissions, bounded startup/call
timeouts and `PluginLimits`, plus an explicit minimized environment allowlist.
The adapter—not the profile—creates and later removes its isolated cwd. No
plugin gets the RAH workspace as an implicit cwd. Profile inspection reveals
the symbolic plugin ID and limits, not values, temp paths, child stderr, or
environment.

This is provider configuration for existing adapters, not a generalized plugin
manifest, discovery service, installation path, restart manager, or plugin SDK.

## 12. Windows behavior

**PLANNED.** Windows validation must distinguish text from identity. Resolve
the profile only from an absolute trusted path; reject environment expansion,
PowerShell interpolation, `cmd` association, `.ps1`, `.cmd`, `.bat`, and other
wrapper/script launchers for Execute/MCP/Plugin executable resources. Require a
canonical native `.exe` identity (or another explicitly supported native format
only after evidence). `codex.ps1` and `codex.cmd` are not equivalent to a
native Codex executable identity.

Canonicalization must handle `\\?\` forms, case-insensitive equivalence,
drive-letter aliases, UNC paths, junctions and other reparse points. Version 1
should reject UNC paths and any profile/resource path crossing a reparse point
unless the exact supported semantics are demonstrated. It should canonicalize
then compare recorded identity immediately before use, while documenting the
remaining check-to-spawn replacement race. It must not claim that normalized
text establishes identity or that revalidation eliminates TOCTOU.

Profile files should be regular files owned/protected according to the current
user or application service account's effective ACL policy; inability to
establish the needed ACL/reparse facts fails closed for application-owned
profiles. There is no portable claim that every Windows ACL proves integrity.
Avoid automatic environment expansion and pass program/arguments as vectors to
direct process creation only; never construct PowerShell or command strings.

## 13. Cross-platform behavior

**PLANNED.** Schema, symbolic naming, permission mapping, redaction, no-shell,
and all-or-nothing validation are platform-neutral. A trusted host layer owns
native path validation, canonicalization, executable identity capture,
ownership/mode/ACL checks, process-group/Job-Object choice, and availability
decisions.

On Linux/macOS, use absolute paths, canonicalize symlinks, reject unexpected
non-regular executable files, and apply ownership/mode checks only where the
host can accurately obtain them. Do not use ambient `PATH` lookup. A profile
may be copied between hosts but must re-resolve all symbols and reject a changed
or missing executable/repository. Unix symlink resolution and mode checks, like
Windows reparse checks, reduce mistakes but do not prove post-check integrity.
Process groups and file permissions are not an OS sandbox.

## 14. Reload and mutability recommendation

**PLANNED recommendation: immutable for the process lifetime.** The process
loads one profile before constructing its registry and never watches or reloads
it. Automatic filesystem watch is rejected: a profile edit could expand host
authority during an active model session, creating unclear causality and a race
between the prompt, registry, and policy state.

A future explicit trusted reload may build a complete new registry from a newly
selected source, validate it all-or-nothing, terminate or drain old external
providers, and start new sessions only after an operator-visible generation
change. Existing sessions must retain their original immutable registry; they
must never gain newly configured tools. Per-run profiles may be considered only
when each run has the same trusted explicit selection and no active session
survives. Hot reload is **DEFERRED**.

## 15. Versioning strategy

**PLANNED.** Require exact `profile_version = 1`. An unknown, missing, or
future version rejects before interpretation; upgrades must not reinterpret
existing fields. The host should publish the supported profile versions in its
redacted inventory. Add a new profile version when a field changes authority
meaning, not just when adding a display field.

Profile validation also pins or checks independent compatibility boundaries:
RAH capability support, `codex-cli` compatibility when the profile explicitly
selects that already-supported runtime, MCP revision `2025-06-18`, and Process
Plugin protocol version `1` plus expected plugin version. Capability-specific
subschemas are selected by profile version 1 rather than independently
negotiated in v1. Unsupported combinations reject; no downgrade, fallback, or
best-effort reinterpretation is allowed.

## 16. Deterministic test plan

**PLANNED.** A future implementation should use temporary owned directories,
native fixture executables, fake MCP/plugin transports where possible, and no
network, credentials, real LLM, or GPU. It should prove at least:

- a valid minimal static profile builds the exact expected inventory;
- unknown capability, unknown field, duplicate capability/provider/resource,
  invalid permission, and unsupported profile version fail before registration;
- unavailable executable, non-native wrapper, repository identity mismatch,
  target outside its authorized repository, and symlink/reparse-point input
  fail closed on supported test platforms;
- each enabled capability preserves fixed argv/cwd/env and rejects model
  attempts to supply those controls;
- a missing MCP/plugin external-tool permission rejects discovery/registration;
- metadata cannot add a discovered tool or increase its permission;
- profile and tool errors/inventory redact tokens, environment values, paths,
  argv, stderr, endpoints, and temporary directories;
- an unsupported platform-specific capability fails with a redacted reason;
- a failed replacement/reload validation leaves the original effective profile
  unchanged, while v1 has no automatic reload; and
- no validation failure registers partial authority or expands a session's
  registry.

Platform-specific fixture assertions must avoid treating a textual path
comparison as a security proof. Tests should state the OS assumptions and skip
only assertions not representable on that host, not the default-deny result.

## 17. Live validation plan

**PLANNED.** Keep live validation opt-in, local, and narrow. A trusted operator
can select a known static profile which constructs a `ToolRegistry`, then use
the pinned Codex bridge to invoke one existing capability at a time:
`host.cargo.version`, `host.git.status`, `fs.read`, an MCP echo provider, and a
Process Plugin echo provider. Start with read/echo capabilities; do not use
stage/unstage as the first profile smoke test.

This proves the selected local executable/provider can be resolved, profile
construction reaches the normal registry/bridge, and a model call cannot alter
the fixed configuration. It does not prove a malicious provider is contained,
that all filesystem races are prevented, that model output is trustworthy, that
network isolation exists, or that uncertain side effects are reversible. Any
Git mutation live test remains separately opt-in and must retain its existing
index-only pre/postcondition evidence.

## 18. Explicit non-goals

**DEFERRED.** This design does not implement or authorize destructive worktree
restore; commit/ref/history; network or credential-bearing Git; generic
`shell.exec` or `process.exec`; arbitrary filesystem write; a new
`PermissionLevel`; protocol/model message changes; desktop/web UI; full plugin
manager; MCP Streamable HTTP; hot reload; or a secret-storage service.

## 19. Recommended implementation boundary

**PLANNED decision: the static-profile increment is sufficiently bounded, with
two limits.** Task 028 should implement only:

```text
one explicit trusted static profile source
-> strict parse/version validation
-> symbolic resource resolution and native identity checks
-> existing capability/adaptor constructors and policies
-> fresh ToolRegistry construction
-> redacted effective-profile inspection
```

It should begin with built-ins (`fs.read`, cargo version, Git status) and
add index-only Git/MCP/Plugin entries only if their current constructors expose
the precise safe inputs required above. Missing constructor seams are a reason
to split an adapter-local, security-reviewed task; they are not permission to
add generic process configuration. No new public protocol/API, dependency,
runtime authority, model schema, automatic reload, or provider routing is
needed for the boundary.

**ADR impact: PLANNED.** This research alone requires no ADR. Before code, a
new ADR is recommended only if the implementation introduces a durable
host-configuration trust model that changes the security model or an
architecture-defining extension point. If the implementation remains private
CLI/application composition over existing constructors, document the boundary
in the task plan and security documentation; do not revise accepted ADRs to
weaken their rules.

## 20. Recommended next task

**PLANNED — Task 028: static trusted capability profile implementation and
deterministic tests.** First inventory constructor seams against this document,
then implement the smallest built-in-only profile loader and redacted effective
inventory. Keep MCP/Process Plugin profile construction as explicitly gated
substeps if their current safe configuration surface is insufficient. Validate
that malformed input cannot leave partial registration and that no model-visible
output contains profile secrets or raw host topology.

## Sources and evidence boundary

**VERIFIED sources.** `docs/RAH_V0.4_SCOPE_AUTHORITY_ROADMAP.md`,
`docs/SECURITY.md`, ADRs 0007–0010, and the v0.3 implementation baseline are
the basis for claims about current capability boundaries. This document does not
claim that profile parsing, symbolic resolution, inspection, reload, or the
proposed schema is implemented.
