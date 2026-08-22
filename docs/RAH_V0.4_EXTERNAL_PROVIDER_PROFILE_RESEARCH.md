# RAH v0.4 external provider profile integration research

Status: **PLANNED -- research/design only**

Date: 2026-08-22

Task: 031

Terminology: **VERIFIED** is supported by the current checked-out source and
its deterministic tests. **IMPLEMENTED** exists in that source. **PLANNED** is
a recommendation for a later task. **DEFERRED** is intentionally outside that
task. Current source, rather than earlier research, is authoritative.

## 1. Current profile implementation baseline

**VERIFIED / IMPLEMENTED.** `TrustedStaticProfile` in `rah-tools` accepts one
explicit absolute JSON file, reads it through `trusted_profile_source`, rejects
links/reparse points and malformed/oversized/non-UTF-8 input, performs strict
duplicate-key/schema/version checks, resolves symbolic executable and
repository resources, and creates a fresh `ToolRegistry`. Its supported
capabilities are only `fs.read`, `host.cargo.version`, and `host.git.status`.
It returns no registry on failure and exposes a redacted host-only effective
inventory. It has no external-provider entries.

**VERIFIED / IMPLEMENTED.** Task 029's `rah profile validate <path>` constructs
that profile. At present, therefore, it validates profile syntax/source and
built-in construction only; it does not launch an external child.

**PLANNED.** External providers must preserve this authority path:

```text
trusted static profile
-> trusted symbolic provider configuration
-> existing/hardened provider constructor
-> bounded child process and discovery
-> exact host external-permission mapping
-> immutable RAH Tool proxies
-> fresh ToolRegistry and redacted inventory
```

The profile, model, MCP server, and plugin child must never select or amend a
provider executable, argv, cwd, environment, timeout, logical ID, or
permission assignment.

## 2. MCP constructor and lifecycle audit

**VERIFIED / IMPLEMENTED.** `McpServerConfig::stdio(server_id, program)`
accepts a validated lower-case server ID and a non-empty `PathBuf`. Its public
builders add arbitrary fixed argument strings, set an arbitrary positive-or-
zero `Duration` call timeout (there is no validation), and add exact remote-name
permission assignments through `ExternalToolPermissionPolicy`. The protocol is
hard-pinned to `2025-06-18`; startup/discovery timeouts are private constants of
two seconds. There is no constructor input for expected tool list/schema, cwd,
environment, queue limit, message/result limit, stderr policy, restart policy,
or startup timeout.

**VERIFIED / IMPLEMENTED.** `McpAdapter::connect` invokes `Command::new` with
the configured program and direct argument vector, pipes stdin/stdout, discards
stderr, and sets `kill_on_drop(true)`. It does not call `current_dir` or
`env_clear`: the child inherits the RAH process working directory and ambient
environment. It does not canonicalize or validate the executable before spawn.
It initializes, sends `notifications/initialized`, calls `tools/list`, and
maps all discovered tools to immutable `mcp.<server_id>.<remote_name>` proxies.

**VERIFIED / IMPLEMENTED.** Discovery rejects a missing/empty remote name,
duplicates, absent object input schema, and every discovered tool without an
exact permission assignment. It does not compare discovery to a host-declared
expected set, nor pin a discovered schema/description. A configured permission
for an absent tool is not detected; an empty list succeeds. MCP input/result
and NDJSON line sizes are unbounded; the actor command channel is unbounded;
there is no outstanding-request bound. Child stderr is unavailable to host
diagnostics because it is discarded. On timeout/drop it sends
`notifications/cancelled`, ignores late responses, and does not replay; child
exit/disconnect fails pending calls and there is no restart/reconnect path.

### MCP authority delta

| Requirement | Classification | Evidence and consequence |
| --- | --- | --- |
| Host-owned server ID, direct argv, pinned protocol, exact permission map | already safe | Constructor/configuration owns these values; discovery cannot assign permission. |
| Symbolic executable -> native identity -> fixed argv | requires constructor hardening | Current program is only non-empty; no absolute/native/reparse/identity/revalidation checks. |
| Host-selected cwd | requires constructor hardening | Inherits RAH cwd. |
| Cleared/minimized environment | requires constructor hardening | Inherits all ambient environment. |
| Bounded queue, frames, results, and retained diagnostics | requires constructor hardening | Unbounded command channel and line/result handling; stderr is discarded. |
| Host-configurable bounded startup/call policy | requires constructor hardening | Startup is private fixed constant; call timeout is not bounded/validated. |
| Exact expected discovery set and schema drift detection | safe with profile composition after bounded discovery hardening | Profile can compare proxies only once discovery is bounded; missing expected tools and schema changes need explicit comparison. |
| Cancellation/no replay/no restart | already safe | Existing actor sends cancellation and retires the generation after failure. |

**PLANNED.** Do not add a raw MCP environment map. A later MCP-local config
should have a closed `EnvironmentPolicy` with `Clear` plus a small named,
host-defined allowlist/value-reference mechanism; v1 profile composition should
use no values unless a verified existing host secret source is separately
authorized. Cwd should be an adapter-created isolated directory or a
host-validated symbolic non-repository directory, never inherited or
profile-model-selected.

## 3. Process Plugin constructor and lifecycle audit

**VERIFIED / IMPLEMENTED.** `PluginConfig::stdio(plugin_id, plugin_version,
program)` validates the lower-case plugin ID, non-empty bounded version, and
non-empty program. Its public builders accept direct fixed argv, exact expected
protocol version, arbitrary call timeout, raw named environment name/value
pairs, bounded `PluginLimits`, and exact remote-name permissions. Limits bound
outstanding requests (maximum 32), queue (64), message/result (1 MiB), and
stderr (64 KiB). Startup is a private two-second constant.

**VERIFIED / IMPLEMENTED.** `PluginAdapter::connect` requires protocol `1`,
canonicalizes the configured program and requires a regular file, creates a
unique temporary isolated cwd, clears the inherited environment, sets
`RAH_PLUGIN_PROTOCOL=1`, preserves `SystemRoot` on Windows when available, and
adds only configured environment entries. It pipes and bounds stderr as
host-only diagnostics. It currently does not require an absolute/native
executable, reject scripts/reparse points, capture executable identity, or
revalidate it before spawning.

**VERIFIED / IMPLEMENTED.** The handshake requires reported protocol, plugin
ID, and plugin version to exactly equal trusted configuration. Discovery is
strict: its JSON envelope/fields, tool count, names, descriptions, schemas,
messages, results, queue, and stderr are bounded; duplicate/invalid metadata
fails the connection. Permission identity is constructed by the host as
`plugin:<configured_plugin_id>:<remote_tool_name>`. A child-reported ID cannot
rename this namespace because mismatch fails before discovery. Missing
assignments fail the complete generation; no assignment is never interpreted as
`PermissionLevel::None`. It does not detect an explicitly configured but absent
tool or compare a discovered schema to a host-declared schema.

**VERIFIED / IMPLEMENTED.** Cancellation is best effort: timed-out/dropped
calls retire the request, send `tools/cancel` only when negotiated, and ignore
known late results. A protocol failure/child exit fails pending requests,
terminates/reaps the child, removes its temporary cwd, and does not restart or
replay. Process supervision and isolation of cwd/environment are expressly not
an OS sandbox.

### Process Plugin authority delta

| Requirement | Classification | Evidence and consequence |
| --- | --- | --- |
| Trusted plugin ID/version and handshake identity | already safe | Exact configured/reported ID and version comparison precedes discovery. |
| Fixed direct argv | safe with profile composition | Static profile may provide a bounded literal vector; model/child cannot alter it. |
| Isolated cwd and cleared/minimized environment | already safe | Adapter creates/removes cwd and calls `env_clear`; profile should leave cwd adapter-owned. |
| Raw environment values | safe with profile composition only if omitted | Existing builder accepts values. Profile must not expose an arbitrary map; begin with no optional values. |
| Bounded transport, diagnostics, cancellation/no restart/no replay | already safe | `PluginLimits`, bounded diagnostics, retired request IDs, and owned supervisor enforce this. |
| Symbolic executable -> native identity -> fixed argv | requires constructor hardening | Canonical regular-file check is weaker than the current `HostExecutionPolicy` native identity seam. |
| Expected tool set and schema-drift check | safe with profile composition | Profile layer can require exact discovered names and schema fingerprints before registry publication. |
| Host-configurable bounded startup policy | requires constructor hardening | Startup timeout is a private fixed constant, unlike limits/call timeout. |

## 4. Permission mapping and discovery mismatch semantics

**VERIFIED / IMPLEMENTED.** Both adapters use exact identity lookup through
`ExternalToolPermissionPolicy`; MCP uses remote name and Plugin constructs
`plugin:<trusted-id>:<remote-name>`. Unknown discovered tools fail before tools
are returned. This is the correct authorization seam.

**PLANNED recommendation: exact remote-name mapping only.** A provider profile
entry names each expected remote tool and its `PermissionLevel`; it is not a
default. Exact mapping is narrower than wildcard/default mapping, which lets a
new remote name inherit authority, and narrower than capability-class mapping,
which makes untrusted metadata decide an authorization class. A literal
`None` remains an explicit low-authority grant; absence is rejection.

| Discovery result | Required profile-load behavior |
| --- | --- |
| Expected `A` exposed with expected schema | Accept only after all other provider checks pass. |
| Expected `A` absent | Reject entire provider/profile load. |
| Additional `B` exposed | Reject entire provider/profile load, even if `B` is unassigned. |
| `A` schema differs from the declared/pinned schema fingerprint | Reject entire provider/profile load. |
| Duplicate/invalid name or malformed definition | Reject entire provider/profile load. |
| Disconnect/handshake/list timeout | Reject entire provider/profile load and reap the child. |

The current adapters already reject additional *unassigned* names. The profile
must additionally prove set equality, so stale permission entries and empty
discovery cannot silently reduce requested authority. A host-held canonical
JSON/schema fingerprint is sufficient; it is an admission check, not a new
model-facing protocol type.

## 5. Executable and resource trust

**VERIFIED.** `HostExecutionPolicy` already supplies the strongest current
host execution seam: absolute path, link/reparse rejection, canonical native
executable validation (`.exe` only on Windows; regular executable ownership/
mode checks on Unix), captured identity, and revalidation immediately before
spawn. It also documents the residual check-to-spawn replacement race.

**PLANNED.** Do not duplicate a weaker executable validator in profile parsing.
Extract or reuse an adapter-appropriate internal, host-owned native executable
resolution/identity helper from that policy in a prerequisite hardening task.
Both adapters must hold the resolved canonical identity and revalidate it at
each spawn. Profile resources contain an absolute literal path only under a
symbolic executable ID; provider entries refer only to the symbol. Provider
logical identity (`server_id`/`plugin_id`) stays separate; a generic `provider`
resource class adds no security value and is unnecessary.

Windows requires rejection of `.cmd`, `.bat`, `.ps1`, file associations,
`PATH` lookup, UNC paths, and reparse-point traversal unless an explicitly
supported design demonstrates safe semantics. Require a native `.exe` and use
direct program/argv creation. Canonical comparison must be case-insensitive and
document that it cannot remove TOCTOU replacement risk.

On Unix require an absolute regular executable after symlink resolution,
disallow unexpected link traversal, test execute mode and ownership where the
host can establish them, and avoid ambient `PATH`. Symlink/mode checks improve
integrity but are not a sandbox and cannot prove immutability after validation.

## 6. Provider schema and inventory

**PLANNED recommendation: provider-specific entries plus the existing symbolic
resource table (Option 3).** Use conceptual `mcp_providers` and
`process_plugins` sections, each with their own closed fields, while sharing
`resources.executables`. This is stronger than a generic tagged subprocess
object: MCP cannot silently gain plugin-only environment/diagnostic knobs and
plugins cannot gain irrelevant MCP fields. It is also clearer than adding a
generic provider resource.

```text
resources.executables: { mcp-echo, plugin-echo }
mcp_providers:       { local-mcp-test -> executable mcp-echo }
process_plugins:     { local-plugin-test -> executable plugin-echo }
```

Provider entries should contain a symbolic provider ID, executable resource,
fixed bounded argv, exact pinned protocol/version, expected-tools table with
exact permissions and schema fingerprints, and only provider-safe bounded
limits. Plugin cwd remains an implementation constant. MCP must first gain an
equally explicit cwd/environment/limit policy. No generic process kind, raw
path in a provider entry, generic env map, arbitrary cwd, executable download,
or child-supplied expansion is allowed.

**PLANNED inventory.** Trusted host inventory may show provider kind and ID,
`status=validated`, tool count, stable RAH proxy name, explicit permission, and
registered state. Remote names are safe for trusted-operator display when they
are the already-registered tool names, but remain model-visible only through
ordinary `ToolDefinition`s. Never render executable path, argv, cwd, env names
or values, endpoint/credential data, raw stderr, temporary directory, schema
body/fingerprint internals, or private policy details.

## 7. Validation, spawning, and atomicity

**PLANNED.** External effective validation necessarily launches a configured
child and performs handshake/discovery. This is controlled host execution, not
JSON parsing. Retain distinct future command semantics:

| Future operation | Meaning | Child spawn |
| --- | --- | --- |
| `profile check` | Parse/version/closed schema only | No |
| `profile validate` | Source/resource checks plus built-in construction | No for a built-in-only profile; explicit status for external entries |
| `profile inspect --effective` or explicit provider validation | Launch configured provider, handshake/discover, build effective inventory | Yes |

Do not silently make today's `profile validate` launch a child. If a later
command retains that name but executes providers, its help/output must state
that it launches named trusted local children and performs discovery; a
confirmation/explicit flag is preferable. It must never execute a profile
selected by a model or repository autoload.

**PLANNED recommendation: atomic profile loading.** Build all built-ins and
connected provider generations privately; exact-check every discovery and
register into a fresh local registry. If any provider fails, shut down/reap all
staged providers, return no registry and no effective inventory, and leave any
old registry unchanged. Partial activation would make a typo or compromised
provider look like an intentional smaller authority grant. Optional provider
groups are DEFERRED and require their own explicit semantics.

## 8. Deterministic and live validation plan

**PLANNED deterministic MCP tests.** Cover valid configuration/discovery,
exact mapping, missing assignment, extra tool, expected missing tool, schema
drift, duplicate/invalid names, handshake/init failure, timeout, malformed
result, child exit, redacted inventory, executable validation failure, no
partial registry, and no restart/replay. Add fixtures that prove bounded queue,
message, result, stderr diagnostics, cleared environment, and isolated cwd
once MCP hardening exists.

**PLANNED deterministic Plugin tests.** Cover valid plugin, configured versus
reported ID, exact mapping, missing mapping, extra/missing tools, schema drift,
handshake/protocol mismatch, malformed discovery, timeout, child exit, bounded
stderr, isolated cwd, minimized environment, redacted inventory, executable
validation failure, no partial registry, and no restart/replay. Existing echo
fixtures already cover much of the transport/lifecycle set without network,
Codex, credentials, GPU, or real LLM.

**PLANNED live check after implementation.** Use only repository-owned MCP and
Plugin echo fixtures with `PermissionLevel::None`:

```text
trusted profile -> provider -> ToolRegistry -> Generic Codex Tool Bridge
-> echo proxy -> terminal Completed
```

This proves profile composition and fixed configuration, not containment of a
malicious child, network isolation, or replay safety beyond the existing
single-call behavior. It must be opt-in and local.

## 9. ADR assessment and readiness matrix

**PLANNED.** The existing provider execution models remain governed by ADR
0007 and ADR 0008. If Task 032 makes trusted capability profiles the shared,
durable admission and authority-composition boundary for built-ins and either
external provider, write an ADR first: *Trusted host capability profile as
authority-composition boundary*. It should record static explicit selection,
symbolic resources, exact permissions, external discovery admission, atomicity,
redaction, and spawn semantics. This research document alone does not change an
ADR.

| Dimension | MCP | Process Plugin |
| --- | --- | --- |
| Existing constructor narrowness | Broad process launch surface | Mostly narrow, but raw env and argv builders remain host inputs |
| Executable authority | No validation/canonical identity | Canonical regular file only; native identity/revalidation missing |
| argv ownership | Host builder, direct vector | Host builder, direct vector |
| cwd policy | Inherited RAH cwd | Adapter-created isolated temporary cwd |
| environment policy | Inherited ambient environment | Cleared; protocol/SystemRoot plus explicit entries |
| identity model | Trusted server ID only; no child identity handshake | Trusted ID/version exactly checked in handshake |
| permission mapping | Exact remote name, fail closed | Exact host-qualified identity, fail closed |
| discovery stability | Duplicate/name/schema presence checks; no set/schema pin | Strict bounded parsing; no expected set/schema pin |
| process supervision | Owned child; unbounded actor; no restart | Owned bounded supervisor; cleanup; no restart |
| deterministic fixtures | Echo fixture covers lifecycle/call failure paths | Echo fixture covers extensive lifecycle, limits, env, cwd paths |
| redaction fit | Stderr discarded, no safe diagnostics/inventory seam | Bounded host-only diagnostics fit redaction model |
| profile schema complexity | Requires new cwd/env/limit hardening | Can use closed provider entry with no env values initially |
| new security debt | High: ambient authority and unbounded resource use | Moderate: executable trust and admission comparison remain |
| readiness | Not ready | Not ready until shared executable hardening and explicit discovery admission are designed |

## 10. Explicit non-goals

**DEFERRED.** MCP Streamable HTTP/network MCP, PluginManager, restart/hot
reload, watchers, installation/downloading/package management, generic external
process configuration, arbitrary executable/argv/cwd/environment, permission
variants, protocol/model APIs, generic shell/process execution, Git restore or
commit/ref/history authority, network/credential Git, and desktop/web UI.

## 11. Proposed implementation boundary and Task 032

**PLANNED prerequisite hardening, before any profile provider integration.**
Task 032 should be an adapter-local MCP hardening task only: introduce bounded
stdio framing/queues/results/stderr, cleared closed environment policy,
adapter-owned isolated cwd, validated bounded timeouts, and reuse/extract the
existing host native executable identity/revalidation seam. It should not add
profile schema entries or change public APIs. Then reassess MCP and write the
ADR if the profile contract becomes cross-provider. A subsequent task can add
Plugin exact-discovery admission plus executable hardening, before composing
either provider into the profile.

D. neither yet — prerequisite hardening required
