# Task 192 — Effective Authority Review UX Contract

Date: 2026-09-04
Status: research complete; documentation-only contract
Baseline: `fe6cf094f3d5bb7335bbf3eb093140648c07b652` (`HEAD == origin/master`)
Released baseline: RAH v0.15.0, tag `v0.15.0`, release commit
`6b66a357cacea4b1fcf21131cbc9e72fab90d59c`

## 1. Executive decision

v0.16 should add a read-only Desktop Effective Authority review backed by one
immutable, host-created, sanitized snapshot per read. The snapshot describes
configured host intent, successfully composed RAH Tools, and the inventory
bound to the currently published runtime. It is presentation/observability,
not an authority object.

The backend must classify the snapshot. The frontend must not compare
arbitrary generations or infer authority from Tool names, provider metadata,
permissions, or its own state. A snapshot is `current` only when its captured
repository, model, and connection identities still match the current published
Desktop context. Any uncertainty is `stale`, `reconnect_required`,
`disconnected`, `connecting`, `unavailable`, or `no_repository`, never current.

The existing `ToolRegistry::definitions()` and `EffectiveProfile` should be
reused as inputs, with a new Desktop-owned sanitized representation required at
the boundary because neither currently carries the complete host source,
repository binding, runtime publication, and status classification together.
No change to `rah-protocol`, ToolRegistry semantics, policies, profiles,
provider protocols, release metadata, or authority is required.

## 2. Current product gap

Desktop currently exposes coarse `AppStatus` rows (`runtime_status`,
`codex_status`, profile, repository, repository-tools, and model configuration)
and separate repository/model/status payloads. It does not provide one view of
the exact effective Tool inventory, host source, repository binding, provider
identity, or the reason a known first-party capability is absent. A user can
therefore confuse configured capability with effective composition, connected
advertisement, or permission to execute a particular request.

## 3. Existing inventory/state architecture

The relevant current sources are:

* `ToolRegistry::definitions()` returns registered `ToolDefinition` values,
  sorted by public `ToolName`. It contains no provider/source or repository
  binding metadata.
* `desktop_tool_registry` constructs a fresh Desktop registry. Its current
  built-in/repository inventory is `echo`, `fs.read`, `repo.file-info`,
  `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.patch`,
  `repo.create-file`, `repo.edit-files`, and conditionally
  `repo.create-directory`, `repo.delete-file`, `repo.rename-file`, and
  `repo.commit`. Conditional tools require the corresponding host-created
  repository authority or commit capability.
* `TrustedStaticProfile::effective_profile()` returns a host-only redacted
  `EffectiveProfile` containing `EffectiveCapability` and `EffectiveProvider`
  records. It distinguishes declared/enabled/registered and uses symbolic
  resource IDs, never source paths. Static profile inspection does not prove
  runtime advertisement; effective composition may be a separate operation.
* `PermissionLevel` is part of `ToolDefinition` and external permission
  assignments are default-deny. It is an outer dispatch classification, not a
  substitute for the narrower repository policies required by ADRs 0010 and
  0012–0019.
* `ConnectionState::Connected` captures the runtime, executable source,
  repository generation, model generation, connection generation, and an
  optional `repo-context:<hash>` fingerprint. `Connecting`, `NotConnected`,
  `Disconnecting`, and `Error` are separate states.
* `DesktopAppState` owns repository selection/generation, model generation,
  connection generation, chat/session generation, repository workflow and
  ephemeral commit review. Repository selection invalidates old context and
  connection publication is rejected when captured repository or connection
  generation is no longer current.
* The current Codex bridge snapshots the host registry when the runtime is
  connected and translates public RAH definitions to private Codex dynamic
  definitions. The bridge does not become execution authority.
* MCP and Process Plugin adapters converge on RAH `Tool`; their host-approved
  logical identities and assigned permissions may be used by a later Desktop
  composition seam, but paths, commands, endpoints, environment, stderr, and
  tokens are not review data.
* Repository workflow state has a presentation enum for commit review,
  including identity/configuration, review-required, ready-to-authorize,
  authorized-pending, stale, failed, and revoked. The underlying review,
  selector, digest, and control remain Rust-only and one-shot.

Current Desktop status is therefore evidence for the review, not the review
contract. No existing command returns the required atomic sanitized snapshot.

## 4. Configured vs effective vs runtime-bound model

The panel uses three explicitly labelled layers:

1. **Configured** — trusted host/profile declarations and known host intent.
   This can include disabled, unassigned, failed, repository-dependent, or
   not-yet-composed entries. It is not authority and is not proof of a usable
   Tool.
2. **Effective host composition** — Tools successfully constructed and
   registered by the host in the current composition. This is the source of
   the effective list, but it remains bound to its captured context.
3. **Connected/advertised** — the definitions delivered to the currently
   published runtime/bridge. This is a runtime fact and can be a strict subset
   of configured declarations. It is not unconditional execution authority.

Every request still passes ToolRegistry lookup, the applicable `PermissionLevel`
gate, repository/workspace policy, generation and precondition checks, and any
one-shot commit authority. The UI should say: “Shown Tools describe the
host-composed and runtime-bound inventory. Each request remains subject to
host permission and policy checks.”

## 5. Sanitized snapshot contract

The future Desktop backend should expose the following language-neutral shape.
Names are proposed contract names, not existing Rust types.

```text
EffectiveAuthoritySnapshot {
    schema_version: 1,
    status: SnapshotStatus,
    repository: RepositoryBinding,
    connection: ConnectionBinding,
    configured: ConfiguredSummary,
    effective_tools: [EffectiveToolEntry],
    unavailable_capabilities: [UnavailableCapability],
    reviewed_commit: ReviewedCommitState,
}

SnapshotStatus =
    no_repository
  | disconnected
  | connecting
  | connected_current
  | reconnect_required
  | stale
  | unavailable

RepositoryBinding {
    selected: bool,
    display_name: optional safe string,
    kind: selected_repository | none,
    current_generation: optional u64,
    captured_generation: optional u64,
    identity: current | not_selected | stale | unknown,
}

ConnectionBinding {
    state: not_connected | connecting | connected | disconnecting | error,
    runtime_kind: optional host-approved label,
    runtime_source: optional host-approved label,
    captured_repository_generation: optional u64,
    captured_model_generation: optional u64,
    captured_connection_generation: optional u64,
    advertised: bool,
}

ConfiguredSummary {
    profile_source: optional trusted_profile | host_builtin | repository_host,
    configured_provider_count: u32,
    configured_capability_count: u32,
}

EffectiveToolEntry {
    public_tool_name: string,
    source_kind: built_in | trusted_profile | repository_host | mcp | process_plugin,
    source_label: safe host-derived label,
    effect_class: read_only | repository_mutation | index_mutation | commit | execute | external,
    authority_category: host-defined closed category,
    permission: none | read | write | execute,
    repository_bound: bool,
    advertised: bool,
}

UnavailableCapability {
    public_tool_name: optional known first-party name,
    source_kind: optional safe source kind,
    state: configured_unavailable | not_effective,
    reason: closed reason code,
}

ReviewedCommitState =
    not_applicable | identity_not_configured | review_required
  | ready_to_authorize | authorized_pending | stale | authorization_revoked
  | unavailable
```

`effective_tools` contains only host-registered public RAH names, sorted by
`public_tool_name`. It contains no callable object, alias, schema, request
input, or policy handle. `advertised` is false or the entry is excluded when
the runtime-bound fact is not known; a configured declaration must not be
promoted to advertised.

The backend should derive `effective_tools` from the same registry and source
metadata used to publish the runtime. It must not reconstruct classification
from model-visible definitions after the fact. Explicit host metadata is
required for provider and repository-bound classification; Tool-name prefix
inference is not a contract.

## 6. Field-by-field source and sanitization table

| Field | Source of truth | Host/provider derived | Sanitization | Stale/current semantics | User-facing purpose |
|---|---|---|---|---|---|
| `schema_version` | Desktop DTO contract | Host | Closed integer; deny unknown versions in tests | Independent | Stable serialization contract |
| `status` | Desktop connection/publication state and Task 175C checks | Host | Closed enum only | `connected_current` only after all checks | Primary explanation |
| repository `selected` | Host repository state | Host | Boolean | Snapshot point only | Answers whether a repository is selected |
| repository `display_name` | Existing selected repository presentation, if approved | Host | Final directory/display label only; omit on ambiguity | Mark stale if captured identity differs | Human binding context |
| repository generations | Host repository/connection captures | Host | Bounded integers; no path | Match is required for current | Advanced troubleshooting |
| repository `identity` | Host comparison | Host | Closed enum | Unknown on race/failure | Prevents stale claims |
| connection `state` | `ConnectionState` | Host | Closed enum | Point-in-time | Connection clarity |
| runtime kind/source | Host Codex/source selection or approved adapter identity | Host | Allowlisted label; no executable path | Omit when absent/stale | Runtime provenance |
| captured model/connection generations | `ConnectionState` | Host | Bounded integers, advanced only | Mismatch is not current | Explain reconnect |
| `advertised` | Runtime publication seam | Host | Boolean | True only for the published matching runtime | Distinguish runtime-bound from composed |
| configured counts/source | `EffectiveProfile` and host configuration | Host | Counts and closed source labels; no profile path | May remain visible as historical/configured | Explain intent versus availability |
| `public_tool_name` | `ToolDefinition.name` / host registration | Host | Canonical `RAH ToolName`; reject private aliases | Old entries are stale/hidden from current list | Identify the Tool |
| `source_kind` | Host composition record | Host | Closed enum | Unknown if composition is incomplete | Explain provenance |
| `source_label` | Host-approved logical profile/provider/plugin ID | Host; provider ID is input but not authority | Validate length/characters; never pass through arbitrary descriptions | Omit if not safely known | Identify source without secrets |
| `effect_class` | Explicit host classification at constructor/composition seam | Host | Closed enum | Cannot be upgraded by provider metadata | Broad grouping without erasing category |
| `authority_category` | Explicit host classification and applicable ADR | Host | Closed enum; preserve separate create/delete/rename/directory/index/commit categories | Unknown on missing metadata | Precise authority meaning |
| `permission` | Registered definition/host assignment | Host | Closed `PermissionLevel` label | Does not imply policy success | Show dispatch boundary honestly |
| `repository_bound` | Tool constructor/composition metadata | Host | Boolean; never prefix-derived | False/unknown must not imply binding | Explain repository context |
| unavailable `reason` | Host-known composition outcome | Host | Closed reason code only; no raw error | A race becomes `unknown`/stale | Bounded explanation |
| reviewed commit state | Existing `CommitAuthorizationPresentation` | Host | Closed state only; no selector/digest/token | Recomputed for current generations | Show review readiness without exposing authorization |

Provider-supplied name/description/schema is not a source of authority labels.
Provider metadata may identify a discovered tool only after host admission and
sanitization. Tool descriptions and JSON schemas are deferred from v1.

## 7. Explicit excluded fields

The DTO must never serialize private `rah_tool_N` aliases, raw Tool pointers or
handles, policy objects, mutation leases, commit review IDs/selectors/tokens or
digests, executable paths, process IDs, command lines, cwd, raw profile paths,
canonical or absolute repository paths, environment variables, credentials,
tokens, endpoints, provider stderr, provider responses, raw OS/Git/transport
errors, or untrusted provider authority claims. A fingerprint is not a
capability token and is never accepted back by any command.

There is no v1 raw-debug or “show everything” mode, clipboard export, authority
persistence, or hidden rich backend object that the frontend is expected to
redact. Sanitization occurs before serialization at the backend boundary.

## 8. Snapshot state model

The exact public states are:

* `no_repository`: no repository selected; no repository-bound effective
  inventory is presented as current.
* `disconnected`: repository may be selected, but no runtime is connected;
  configured/effective host summaries may be shown as inactive, never as
  connected authority.
* `connecting`: connection is in progress; do not publish a partial registry
  as final. A prior snapshot may be shown only under an explicit stale-history
  label.
* `connected_current`: the published runtime and host composition match the
  current repository/model/connection context.
* `reconnect_required`: a selected repository or model/context generation
  differs from the connected runtime. Old inventory is historical and cannot
  be labelled effective for the new context.
* `stale`: a previously captured snapshot no longer matches, or collection
  observed a race/unknown identity. It is not current and cannot authorize.
* `unavailable`: host composition, state collection, or required safe metadata
  failed. Show a bounded error category, not an internal error.

`Disconnecting` maps to stale/disconnected presentation until the host has a
new stable state. No optimistic current state is allowed.

## 9. Generation/currentness rules

The backend must collect one state point and apply existing lifecycle rules:

```text
current iff
  repository selection identity is present when repository-bound entries exist
  AND captured repository_generation == current repository_generation
  AND captured model_generation == current model_generation
  AND captured connection_generation == current published connection generation
  AND the captured runtime is the currently published runtime
  AND the registry was composed for those same captured identities
```

Repository and model mismatch maps to `reconnect_required`; a disconnected or
unpublished runtime maps to `disconnected`; a race or unverifiable comparison
maps to `stale` or `unavailable`. The implementation must reuse the existing
`connection_context_is_current` and `connection_publication_is_current`
semantics rather than create a parallel state machine.

`runtime_generation` is the future DTO name only if needed for clarity; current
Desktop uses `connection_generation` as the runtime connection identity. Do not
expose both as independent counters. `session_generation` identifies chat
conversation lineage, not Tool authority, and is omitted from the normal
snapshot. `model_generation` participates in currentness but is advanced-only
or omitted from normal display. Normal UX shows status text; advanced details
may show captured/current repository, model, and connection generations.

Collection must hold the existing state synchronization boundary long enough to
copy a coherent state description, but must not hold locks across provider I/O.
Snapshot rendering requires no provider I/O. If a repository switch,
disconnect, or runtime publication occurs during collection, return the one
internally consistent observed point and classify it stale/reconnect-required;
never combine fields from two points and never retry with a lifecycle side
effect.

## 10. Repository identity presentation

The inventory should send `selected: true/false`, a safe display name when the
existing Desktop selected-repository UX already has one, and a host-derived
binding state. It should not duplicate an absolute path. The existing
repository panel may continue to display its separately authorised product
path; that does not authorise adding the path to this new authority DTO.

The current `repo-context:<hash>` value is useful for host diagnostics but is
not demonstrated as a user-facing stable privacy guarantee. It is derived from
the root representation and could enable correlation. Therefore v1 omits it
from normal UX. If advanced diagnostics later need it, show only a shortened
display-only value with an explicit “diagnostic identifier” label, never call it
a capability, never accept it as input, and retain the existing hashing
contract rather than inventing a new cryptographic guarantee.

## 11. Tool/provider/authority classification

The current first-party Desktop names are the observer Tools
`repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.file-info`; bounded
content/read Tools `fs.read`, `repo.patch`, and `repo.edit-files`; creation
Tools `repo.create-file` and `repo.create-directory`; destructive structural
Tools `repo.delete-file` and `repo.rename-file`; and reviewed commit Tool
`repo.commit`. `echo` is a built-in deterministic Tool and should be marked
development/test-oriented or omitted from a user-facing inventory according to
the later product decision; it must not be mistaken for repository authority.
There are no current Desktop registrations for arbitrary shell authority.

The host classification must preserve distinct categories for repository
observation/read, worktree content mutation, file creation, file deletion, file
rename/move, directory creation, index mutation, reviewed commit, and Execute.
The display may add an effect class such as read-only, repository mutation,
index mutation, commit, or external, but never collapse the specific category
into a generic “write” claim. `PermissionLevel::Execute` is shown as the
dispatch permission and explicitly described as insufficient by itself for
repository mutation or commit.

Safe source labels are `built_in`, `trusted_profile`, `repository_host`, `mcp`,
and `process_plugin`, plus a validated logical provider/plugin ID where the
host configuration already exposes it. MCP and Process Plugin executable paths,
endpoints, commands and child details remain private. Unknown or unassigned
external Tools are absent; an arbitrary negative inventory is not claimed.

## 12. Unavailable-capability reason model

V1 shows only bounded host-known omissions, primarily first-party capability
slots that Desktop can deterministically explain. Proposed closed reason codes
are:

`not_configured`, `authority_not_granted`, `repository_required`,
`reconnect_required`, `provider_not_effective`, `provider_unavailable`,
`permission_not_configured`, `review_required`, `stale_context`, and
`unknown`.

These codes are not raw errors. For example, failure to construct a provider
may display “Provider unavailable” with `provider_unavailable`; it must not
display stderr, a command line, an OS error, or an endpoint. A missing
`repo.create-directory` entry can display `authority_not_granted` only when
the host has an explicit known capability slot and absence is established by
host composition. An external Tool that was never configured or discovered is
simply absent. No universal negative inventory is required.

## 13. Backend API/command recommendation

Add one read-only, no-argument Tauri command such as
`get_effective_authority_snapshot`. A dedicated command gives the DTO a closed
serialization boundary, deterministic tests, and an explicit security review;
it avoids inflating every ordinary repository/status payload and does not make
the frontend reconstruct a cross-state view. The command must only copy already
known host state and may return a sanitized `unavailable` result.

The alternative of extending `app_status` is smaller but couples a high-detail
inventory to a coarse frequently-polled payload. Adding it to connection
events would omit disconnected/configured states and complicate event ordering.
Those payloads may later carry a summary status, but the authoritative v1
review response should be the dedicated read operation. No user-supplied
repository, provider, profile, tool, or authority parameter is accepted.

## 14. Frontend presentation contract

Use an “Effective Authority” panel or drawer with this information order:

1. status banner and one-sentence explanation;
2. repository binding and connection/runtime state;
3. effective/advertised Tool list with host category, effect class, permission,
   source, and repository-bound marker;
4. bounded unavailable first-party capabilities;
5. optional advanced generation details.

The panel must distinguish “Configured”, “Host-composed”, and
“Connected/advertised” labels. A stale snapshot remains visibly historical;
when switching A to B, the old A list is hidden from the current list or placed
under a stale label and never retitled as B. The panel contains no enable,
disable, grant, revoke, reload, reconnect, provider lifecycle, repository
switch, execute, or commit-review action. Existing controls may remain nearby
but are not part of this read contract.

## 15. Zero-side-effect inspection contract

Opening, polling, refreshing, serializing, or closing the panel has exactly zero
lifecycle and authority effects. It must not connect/disconnect/reconnect,
spawn/restart/stop providers, reload a profile, create a runtime/session,
increment repository generation, execute a Tool, mutate repository or index
state, grant/revoke permission, consume commit review, persist authority, or
replay an uncertain effect. Provider composition must already be owned by the
current connection/composition lifecycle; inspection does not perform provider
discovery or I/O.

## 16. Security/privacy analysis

The primary threats are path/user-name leakage, endpoint/credential leakage,
plugin command and environment leakage, provider stderr/response leakage,
private alias exposure, policy-handle exposure, and stale inventory being
misread as authority. Backend construction addresses these by using a closed
DTO, allowlisted enums/labels, host-owned classification, no raw error strings,
and generation-derived status. Serialization tests must inspect the complete
payload for excluded terms, not rely on frontend filtering.

Corrupt, incomplete, or raced state fails toward `unknown`, `stale`, or
`unavailable`. Provider metadata cannot self-label a Tool “safe”, “trusted”,
“read-only”, or “admin”. Permission display cannot imply a broader policy than
the applicable host policy. The snapshot cannot be submitted back as a token.

## 17. Concrete state-transition matrix

| Scenario | Status | Inventory/currentness | Binding and explanation |
|---|---|---|---|
| A. No repository, disconnected | `no_repository` | No repository-bound current Tools; configured built-ins may be shown inactive | “Select a repository to view repository-bound capabilities.” |
| B. Repository selected, disconnected | `disconnected` | Host/configured summary only; no connected inventory is current | Show selected safe name and “Connect to advertise runtime Tools.” |
| C. Selected, connecting | `connecting` | Partial registry is not final; old list is stale history only | “Connecting; effective runtime inventory pending.” |
| D. Repository A connected | `connected_current` | A composition and advertised inventory are current | Matching repository/model/connection generations. |
| E. Switch A to B | `reconnect_required` or `stale` | A list cannot be current for B; hide or label historical | B generation differs; “Reconnect to activate B-bound Tools.” |
| F. B reconnect succeeds | `connected_current` | Fresh B registry/advertised list current | New connection generation and matching B repository/model generations. |
| G. Provider composition failure | `unavailable` or connected current without that provider | Failed provider absent; no raw cause | “Provider unavailable”; bounded reason code. |
| H. First-party permission absent | `connected_current` with omission, or `unavailable` if composition failed | Known capability is unavailable, not effective | `authority_not_granted` or `permission_not_configured`; no policy internals. |
| I. Runtime disconnect | `disconnected` | Previous list is stale history, never current | “Runtime disconnected; no connected runtime inventory.” |

## 18. Deterministic test plan

Future implementation tests must cover no repository, selected/disconnected,
connecting, connected-current, A-to-B reconnect-required, fresh B reconnect,
repository-generation mismatch, connection/runtime-generation mismatch,
provider sanitization, no path/env/stderr/endpoint/token/profile-path leakage,
no private aliases, repository identity sanitization, first-party
`authority_not_granted`, provider-unavailable reason, permission/category and
repository-bound classification, deterministic sorting, closed DTO schema,
stale inventory exclusion, misleading provider metadata rejection, and commit
review non-disclosure.

They must also prove viewing does not execute Tools, spawn/restart providers,
alter repository generation, reconnect, mutate repository/index, or consume or
revoke commit review; that concurrent repository switch/disconnect/publication
returns one internally consistent classified snapshot; and that frontend
rendering (if convention supports it) treats backend status as authoritative.

## 19. Windows live-validation plan

Task 192 performs no live validation. A later Windows gate should launch the
exact Desktop build, select disposable repository A, connect with the certified
native Codex baseline where needed, inspect a current A snapshot, switch to B,
verify reconnect-required/stale treatment and no current A authority claim,
reconnect and verify fresh B current inventory, and close normally. Evidence
must show public names, sanitized payload absence of paths/secrets/aliases, and
no repository mutations or provider lifecycle effects from viewing. A single
connected-runtime live check plus deterministic state-machine/security tests is
enough for most UX claims; native Codex is required only for the connected /
advertised bridge claim, not no-repository or disconnected fixtures.

## 20. Explicit v0.16 non-goals

Defer authority editing/grant/revocation, profile reload, provider lifecycle,
Tool enable/disable, plugin installation, MCP endpoint configuration,
repository switching from the panel, automatic reconnect, persistence,
import/export/cloud sync, session persistence, raw debug dumps or paths,
credentials/tokens/environment/stderr, private aliases/handles, Tool execution,
commit-review creation, permission overrides, dynamic authority mutation, and
Linux live certification.

## 21. ADR revalidation

**NO NEW ADR REQUIRED.** The contract remains a host-local sanitized read view
over existing ToolRegistry, Trusted Profile, external admission, lifecycle,
repository-policy, and reviewed-commit decisions. It creates no authority
plane, persistence semantics, provider trust semantics, or dynamic activation.
If implementation expands into any of those areas, stop and create a dedicated
architecture/security decision before proceeding.

## 22. Implementation sequencing

1. **Task 193 — backend sanitized effective-authority snapshot implementation.**
   Add explicit host source/category metadata at the narrow Desktop composition
   seam, collect one coherent state point, serialize the closed DTO, and add
   deterministic sanitization/currentness tests.
2. **Task 194 — Desktop frontend Effective Authority review UX.** Render only
   the DTO, with stale/reconnect language and no authority controls.
3. **Task 195 — cross-layer deterministic/security hardening.** Exercise race,
   excluded-field, provider-metadata, and zero-side-effect contracts.
4. **Task 196 — Windows live UX validation.** Validate one connected bridge and
   repository switch/reconnect flow with disposable repositories.
5. **Task 197 — v0.16 milestone audit.** Confirm exact release and evidence
   gates without changing v0.15 release state.

## 23. Open questions

No security-critical question remains unresolved. Task 193 may choose whether
the development-only `echo` entry is hidden or visibly labelled, and may choose
the exact internal enum names, but it must preserve this closed contract. It
must also confirm the narrowest host seam for explicit source/category metadata
without moving Desktop-only presentation types into `rah-protocol` or
provider-neutral core APIs.
