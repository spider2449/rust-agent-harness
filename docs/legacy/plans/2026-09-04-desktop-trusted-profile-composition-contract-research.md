# Task 202 — Desktop Trusted Profile Composition Contract Research

## Scope

Define the implementation-ready v0.17 contract for bringing Trusted Profile
external providers into the Windows Desktop host path without changing existing
repository authority, runtime boundaries, Generic Tool Bridge semantics, or the
meaning of ADR 0011.

This task is research/documentation only. It does not authorize Rust,
frontend, Cargo, profile-schema, provider-protocol, runtime, ADR, or release
changes.

Task 201 selected Desktop Trusted Profile external-provider integration as the
v0.17 milestone. Task 202 narrows that recommendation into crate placement,
profile admissibility, host binding, lifecycle, publication, Effective
Authority classification, and failure semantics.

## Starting architecture

The relevant established paths are:

```text
Trusted Static Profile
  -> static source/schema validation
  -> explicit effective composition
  -> admitted MCP / Process Plugin adapters
  -> fresh ToolRegistry
  -> Generic Codex Tool Bridge
```

and, separately:

```text
Desktop host-selected canonical repository
  -> Desktop first-party repository authorities
  -> Desktop ToolRegistry
  -> Codex thread/start cwd
  -> repository/model/connection generations
  -> repository-scoped conversation namespace
```

The v0.16 Desktop Effective Authority UX observes host-owned composition and
lifecycle state. It is not a composition or activation surface.

## Decision 1 — shared composition belongs in a narrow composition crate

The current effective Trusted Profile composer is hosted by `rah-cli`. That is
an implementation location, not an appropriate long-term dependency boundary
for Desktop.

Rejected options:

- `rah-desktop -> rah-cli`: app-to-app dependency and CLI-host coupling;
- move composition into `rah-tools`: MCP and Process Plugin adapters already
  depend on `rah-tools`, which would force or invite a dependency cycle;
- move composition into `rah-runtime` or `rah-runtime-codex`: runtime must
  remain provider-neutral and downstream from host composition;
- move composition into `rah-core`: core is an intentionally low dependency
  boundary and must not acquire provider or host-composition concerns; and
- copy the composer into Desktop: duplicated security-critical admission and
  cleanup logic would drift.

Accepted direction:

```text
rah-tools -----------\
rah-tools-mcp --------+-> rah-profile-composition <- rah-cli
rah-tools-plugin ----/                         \
                                               -> rah-desktop
```

Create a narrow `rah-profile-composition` crate by extracting the existing
host-effective composition logic from CLI without redesigning it.

The extraction must not introduce a generic ProviderManager, generic provider
trait, runtime abstraction, dynamic plugin system, or new authority model. CLI
behavior and semantics must remain unchanged after the move.

This placement also restores the layering anticipated by the earlier Trusted
Profile MCP design: effective profile composition sits above the neutral Tool
and provider adapter crates rather than inside a particular application host.

## Decision 2 — Desktop v0.17 accepts provider-only Trusted Profiles

Desktop must **not** treat Trusted Profile v1 as a complete replacement for the
v0.16 first-party Desktop ToolRegistry.

For the v0.17 Desktop path, an accepted profile is a provider-only overlay:

```text
profile.capabilities == []
profile.mcp_providers      may be non-empty
profile.process_plugins    may be non-empty
```

Resources needed by those provider declarations remain allowed under the
existing closed schema and validation rules.

If a selected profile contains any first-party `capabilities` declaration,
Desktop static validation fails closed for v0.17. No provider may be spawned as
part of that failed selection/validation.

This is a Desktop product contract only. It does not remove or alter the CLI's
existing full Trusted Profile behavior.

### Why full-profile takeover is rejected

Desktop already owns a stronger product-level repository context contract than
the generic CLI profile path. A full-profile takeover could create conflicting
repository resources, duplicate or omit first-party authorities, or make
profile schema evolution determine the Desktop's canonical repository Tool set.

It would also create an immediate asymmetry because
`repo.create-directory` is a first-party Desktop capability backed by a
separate host-owned opaque authority, while Trusted Profile v1 does not
manufacture or currently express that authority. Replacing the Desktop registry
with a profile-composed first-party registry would therefore regress or distort
the v0.16 workflow.

The provider-only overlay avoids that problem entirely.

## Decision 3 — Desktop repository ownership is unchanged

The host-selected canonical Desktop repository remains authoritative for:

- first-party repository Tool construction;
- repository generation and stale-context checks;
- Codex `thread/start` working directory;
- repository-scoped conversation persistence;
- reviewed-commit workflow state; and
- repository mutation/review refresh semantics.

Trusted Profile must not select, replace, override, or rebind that repository
in v0.17.

The final Desktop registry is composed conceptually as:

```text
Desktop first-party ToolRegistry
              +
Trusted Profile admitted external provider Tools
              |
              v
       fresh final ToolRegistry
              |
              v
       Generic Codex Tool Bridge
```

Duplicate public Tool names fail closed. No partial registry is published.

## Decision 4 — selection is inert; Connect is the activation point

Profile handling must preserve the distinction between configured intent and
effective authority.

### Startup

Desktop starts with no active external profile authority. Startup must not
launch MCP or Process Plugin children.

v0.17 does not persist or auto-restore profile activation across application
restart.

### Select profile

Profile selection is allowed only while disconnected and otherwise in a host
state where repository/model authority is not being published through a live
connection.

Selection performs only hardened static loading and validation:

- explicit host-selected absolute source;
- existing Trusted Profile source topology, size, UTF-8 and schema checks;
- existing exact provider declaration validation; and
- the Desktop-specific provider-only rule.

It must not spawn a provider, construct a Codex runtime, connect, advertise a
Tool, or alter first-party repository authority.

### Connect

Connect is the explicit effective-activation point.

At Connect, Desktop must load and validate the selected profile again from the
explicit selected source before effective composition. A successful earlier
static inspection is not treated as a durable live authority object.

No new long-lived profile-content fingerprint or hot-reload identity model is
introduced in v0.17. The established contract remains explicit static file
selection followed by fresh loading at explicit activation.

### Reconnect

Reconnect performs fresh static loading and fresh effective composition. It
must not reuse old provider children, stale Tool proxies, or a previously
published effective registry.

### Disconnect and shutdown

Disconnect removes the usable runtime/registry path and shuts down every
provider adapter owned by that effective composition. Application shutdown must
also reap those children.

No automatic restart, retry, replay, background recovery, or provider hot reload
is added.

## Decision 5 — connection publication is atomic

The required Connect sequence is conceptually:

```text
capture current repository/model/profile selection context
  -> fresh static profile load/validation
  -> construct current Desktop first-party registry
  -> privately construct every configured external provider
  -> exact tool/schema/permission admission
  -> merge into one fresh final ToolRegistry
  -> construct/connect Codex runtime with that registry and canonical cwd
  -> revalidate captured Desktop generations/currentness
  -> publish Connected
```

Nothing before the final publication step becomes the current Desktop
connection.

If any profile load, provider startup, provider admission, duplicate
registration, runtime construction, runtime connection, cwd verification, or
captured-generation revalidation fails:

- no new Connected state is published;
- no partial ToolRegistry becomes current;
- every staged provider is shut down;
- any staged runtime is shut down or otherwise reaped through the existing
  lifecycle contract; and
- the failure is reported through bounded sanitized Desktop error state.

This is an atomic publication guarantee, not a rollback guarantee for arbitrary
external provider side effects during initialization. RAH must not claim that
process cleanup reverses external effects.

## Decision 6 — effective composition owns provider lifetime

The published connection must retain an owning effective-profile composition
object for at least as long as any provider proxy Tool can be dispatched.

Conceptually the connected state owns:

```text
CodexRuntime
final DesktopToolComposition / ToolRegistry
EffectiveProfileComposition
captured repository generation
captured model generation
connection generation
```

The exact Rust layout is an implementation detail, but proxy Tools must never
outlive the MCP or Process Plugin adapter that services them.

Provider shutdown ordering must prevent new dispatch through a registry after
its backing provider has been intentionally torn down.

## Decision 7 — Effective Authority gains explicit external classifications

The v0.16 backend already owns the sanitized authority snapshot and already has
source/effect/authority categories for external providers. v0.17 should make
those categories reachable rather than inventing a second provider UX.

For an admitted MCP Tool:

```text
sourceKind       = mcp
sourceLabel      = bounded sanitized provider identity
 effectClass      = external
authorityCategory = external
permission       = host-configured RAH PermissionLevel
repositoryBound  = false
```

For an admitted Process Plugin Tool:

```text
sourceKind       = process_plugin
sourceLabel      = bounded sanitized provider identity
effectClass      = external
authorityCategory = external
permission       = host-configured RAH PermissionLevel
repositoryBound  = false
```

`repositoryBound = false` means only that RAH has not constructed this Tool as
one of its selected-repository authority capabilities. It must **not** be shown
or interpreted as proof that the provider child is unable to access repository
files through ambient OS permissions.

The backend classification source is the host-owned admitted provider record,
exact Tool definition, and explicit RAH permission mapping. Provider-authored
Tool descriptions or metadata cannot elevate or redefine authority.

Private Generic Codex Tool Bridge aliases remain private and must never appear
in the Effective Authority DTO or frontend.

## Decision 8 — external Tool effects require conservative repository handling

RAH cannot infer absence of filesystem or repository side effects from an
external Tool's `PermissionLevel`, description, provider metadata, or success
response.

Therefore, when a selected repository is active and an MCP or Process Plugin
Tool actually reaches the normal started/finished external dispatch lifecycle,
Desktop must conservatively treat the result as a possible external effect for
repository workflow presentation.

At minimum, after such a call completes or becomes uncertain, Desktop should:

- refresh repository/workflow presentation; and
- invalidate any outstanding reviewed-commit authorization so a commit cannot
  rely on a review snapshot that predates an untrusted-to-RAH external effect.

This does not assert that the provider changed the repository. It records that
RAH cannot prove it did not.

Known rejection before provider dispatch does not need to be classified as an
external filesystem effect merely because the requested Tool was external.

`repo.commit` still performs its own established fresh host revalidation. No
external Tool result can authorize or bypass reviewed commit authority.

## Decision 9 — Effective Authority Refresh remains observational

The v0.16 `Refresh Authority` contract remains unchanged.

Refreshing the authority view may re-read current in-memory host state and
render a fresh sanitized DTO. It must not:

- reload the profile source;
- spawn or stop a provider;
- reconnect a provider or runtime;
- alter the effective ToolRegistry;
- retry a failed provider;
- activate configured-but-inactive authority;
- execute a Tool; or
- change repository, model, connection, review, or authority generations.

## Error and sanitization requirements

Desktop-facing provider/profile failures must remain bounded and sanitized.
They may identify closed categories such as profile invalid, provider
unavailable, admission failed, duplicate registration, reconnect required, or
stale context.

They must not expose:

- raw native profile/executable paths;
- argv or environment contents;
- credentials or tokens;
- raw endpoint secrets;
- provider stderr beyond an explicitly existing redacted policy;
- private protocol correlation identifiers;
- Generic Codex Tool Bridge aliases; or
- opaque authority/policy internals.

## ADR decision

Task 202 finds no evidence requiring a new ADR for the recommended v0.17
milestone.

ADR 0011 already defines Trusted Profile as a host-only composition boundary for
existing approved capabilities and admitted external providers. The
provider-only Desktop rule is a narrower product/host composition policy, not a
new model-facing authority class.

A new ADR would become necessary if a future task introduces materially new
authority such as network endpoints/credentials, generic process execution,
dynamic hot-reload authority replacement, provider installation/update, or a
profile ability to select or manufacture Desktop repository authority.

## Explicit non-goals

Task 202 rejects or defers:

- full Trusted Profile takeover of Desktop first-party capabilities;
- Trusted Profile repository selection or repository-resource binding in
  Desktop;
- Trusted Profile support for manufacturing directory-creation authority;
- profile auto-discovery, persistence, auto-activation, editing, or hot reload;
- ProviderManager, provider installation, update, download, marketplace or
  automatic restart;
- network MCP / Streamable HTTP;
- generic subprocess, shell, executable, argv, cwd, or environment authority;
- new filesystem, Git branch/ref/history, network Git, or credential authority;
- changes to Generic Codex Tool Bridge routing semantics;
- rollback or compensation guarantees;
- new OS sandbox or network-isolation claims; and
- Linux live certification claims.

## Implementation sequencing

The smallest defensible v0.17 implementation sequence is:

### Task 203 — Shared Effective Profile Composer Extraction

Architecture-preserving refactor only:

- add `rah-profile-composition`;
- move the existing effective composer out of CLI;
- preserve CLI static/effective behavior;
- preserve provider admission, ownership, redaction and cleanup semantics; and
- do not integrate Desktop providers yet.

### Task 204 — Desktop Provider-Only Profile Selection

Add disconnected host selection and static validation only:

- explicit file selection;
- no persistence or auto-activation;
- no spawn;
- reject non-empty first-party `capabilities`; and
- expose only sanitized configured state.

### Task 205 — Desktop Effective Provider Composition and Lifecycle

Add effective MCP/Process Plugin activation during Connect:

- fresh provider composition;
- fresh final ToolRegistry;
- atomic publication;
- generation/currentness checks;
- disconnect/reconnect/shutdown cleanup; and
- deterministic lifecycle tests.

### Task 206 — External Effective Authority UX Hardening

Make the v0.16 review surface accurately present MCP and Process Plugin Tools:

- backend-owned external source/effect/authority classification;
- configured/effective/advertised/current state;
- sanitization and no private aliases;
- conservative external-effect repository refresh; and
- reviewed-commit authorization invalidation.

### Task 207 — Windows Desktop External Provider Live Certification

After deterministic gates pass, perform a separately scoped Windows live gate
using the certified Codex baseline and real local provider fixtures. Prefer
evidence covering both local stdio MCP and Process Plugin paths if the fixture
and certification environment supports both.

Linux live certification remains unclaimed.

## Task 202 conclusion

The implementation-ready v0.17 contract is:

> Extract the existing effective Trusted Profile composer into a shared narrow
> `rah-profile-composition` crate, then let Desktop use only provider-only
> Trusted Profiles as a disconnected-selected, Connect-activated overlay over
> the unchanged v0.16 first-party Desktop registry.

This introduces no new authority class, does not require a new ADR, preserves
the host-selected canonical repository, keeps provider lifecycle ownership
explicit, and closes the v0.16 Desktop external-provider reachability gap with
the smallest architecture change.

The next task is Task 203 — Shared Effective Profile Composer Extraction.
