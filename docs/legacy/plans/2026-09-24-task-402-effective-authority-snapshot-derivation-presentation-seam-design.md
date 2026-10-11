# Task 402 — Effective Authority Snapshot Derivation / Presentation Seam Design

Date: 2026-09-24
Scope: Research/design only. No Rust, test, visibility, authority, IPC, permission, capability, or ADR changes.

## Verdict

**PASS — EFFECTIVE AUTHORITY SNAPSHOT SEAM DESIGN READY**

Freeze a bounded root-gather / module-compose seam for Task 403. Root remains the only reader of `DesktopAppState` and the owner of lifecycle coordination, lock acquisition, expiry cleanup, and the existing shared generation comparison. `effective_authority.rs` receives copied and already-closed observations, derives reported current/status and HostExplicit availability, and builds the complete closed `EffectiveAuthoritySnapshot`.

This is an observational presentation seam. It does not transfer repository, provider, runtime, Commit, or HostExplicit execution ownership. The design does not claim a transactionally coherent snapshot: it preserves the existing best-effort lock sequence and generation rules.

## Checkpoint and scope

- Expected starting HEAD: `5e557f8a9cc068b79480708724373c581617943c` (`docs: research effective authority snapshot ownership`).
- Verified starting HEAD: exact match.
- Verified starting worktree: clean (`git status --short` empty).
- Task 401 artifact reviewed: `docs/plans/2026-09-24-task-401-effective-authority-snapshot-ownership-boundary-research.md`.
- Only this plan file is authorized to change in Task 402. Do not modify Rust/tests, write an ADR, change Tauri IPC, permissions/capabilities, or authority semantics. Do not push, tag, or start Task 403.

## Current ownership map

| Responsibility | Current owner/evidence | Task 402 design owner |
|---|---|---|
| Tauri command and `DesktopAppState` lock acquisition | Private `get_effective_authority_snapshot` and `effective_authority_snapshot_for_state` in `main.rs:2219-2497` | ROOT |
| Active repository admission, activation, membership and generation | Repository membership/lifecycle plus `state.repository` and `repository_generation` | EXISTING OWNER; root only observes projection |
| Model configuration and generation | `DesktopModelState` and model lifecycle | EXISTING OWNER; root only captures generation |
| Trusted Profile selection and profile generation | `trusted_profile_selection` and root selection/generation locks | EXISTING OWNER; root captures sanitized configuration summary/descriptors |
| Provider activation/readiness and published effective composition | Provider composition/lifecycle and connected `ConnectionState` | EXISTING OWNER; root captures the published connection/composition |
| ToolRegistry construction and capability authorities | Provider/repository composition and the underlying Tools | EXISTING OWNER |
| HostExplicit prepared ticket, retained payload, dispatch and lifecycle | `HostInvocationCoordinator` in `host_invocation.rs` and root handlers | EXISTING OWNER; root cleanup and coarse-state observation only |
| Reviewed Commit workflow and authorization state | Commit workflow in `RepositoryWorkflowState` and Commit subsystem | EXISTING OWNER; root captures `CommitAuthorizationPresentation` only |
| Snapshot DTO/schema, safe labels, Tool classification, external unavailable records, Commit presentation mapping | `effective_authority.rs:18-210, 585-628`; root currently completes final assembly | EFFECTIVE_AUTHORITY MODULE |
| Cross-owner connection publication equality | Shared `connection_publication_is_current` in `main.rs:1999-2015`, also used outside snapshot | ROOT; do not duplicate or move in Task 403 |

`effective_authority.rs` may consume these owners' observations. It must not own repository admission/activation/membership mutation, ToolRegistry construction, Trusted Profile selection, provider activation, runtime lifecycle, review-ticket issuance, Commit authorization, HostExplicit dispatch, or prepared-ticket ownership.

## Captured-input and derived-value classification

| Significant value | Classification | Exact current use and proposed seam treatment |
|---|---|---|
| Active repository presence | CAPTURED INPUT | `state.repository.is_some()`; capture as `selected: bool`, not a `DesktopRepository` or membership object. |
| Active repository identity/generation | CAPTURED INPUT | Current `repository_generation` plus connected captured repository generation; membership ID/fingerprint is not read by this helper and must not be added. |
| Repository status/projection | CAPTURED INPUT, not consumed | The helper does not read repository status or refresh a repository projection. Do not add it to the carrier. |
| Safe repository display name | CAPTURED INPUT | Root computes `safe_repository_display_name(&repository.root)` and passes only `Option<String>`; no root path crosses. |
| Repository-bound capability state | CAPTURED INPUT | Capture booleans for branch authority and each existing patch, multi-file-edit, create-file, delete-file, and rename-file preparer. No authority/preparer object crosses. The `repo.commit` unavailable row is based on whether its Tool is present in current composition, not on `DesktopCommitCapability`. |
| Model generation | CAPTURED INPUT | Current `state.model.generation` and connected captured model generation. No model configuration, credentials, or readiness object crosses. |
| Trusted Profile generation | CAPTURED INPUT | Current `trusted_profile_generation` and connected captured profile generation. |
| Connection generation | CAPTURED INPUT | Current `next_connection_generation` value and connected captured connection generation. |
| Commit-identity generation | CAPTURED INPUT | Current generation and connected captured identity generation; the identity value itself and Commit capability do not cross. |
| Published connection phase | CAPTURED INPUT | Closed phase tag corresponding to `ConnectionState::{NotConnected, Connecting, Connected, Disconnecting, Error}`. For Connected also capture the existing source label input and connected captured generations. Do not pass runtime or error details. |
| Published composition and allowed permissions | CAPTURED INPUT | Capture cloned, already-classified `composition.tools`, cloned `composition.unavailable`, and allowed `PermissionLevel`s. Do not pass `DesktopToolComposition`, registry, definitions, preparers, or runtime. |
| Registry/tool inventory consistency | DERIVED CURRENTNESS | Preserve `composition.registry.definitions().len() == composition.tools.len()` exactly. Root computes this at the current connected-branch lock point and passes the resulting closed boolean, so the comparison does not move outside its current lock scope. |
| Connection `context_current` | DERIVED CURRENTNESS | Existing equality of captured/current repository, model and profile generations. Keep generation acquisition and this comparison at the root's present point under the connection guard to preserve lock topology. |
| `publication_current` | DERIVED CURRENTNESS | Existing `connection_publication_is_current` equality over repository, model, profile, connection and identity generations. Keep using the shared root helper; do not clone its rule in the new module. |
| Repository context match | DERIVED CURRENTNESS | Preserve `selected || captured_repository_generation == 0`. Feed this closed boolean/fact into module composition. |
| Final current/stale/reconnect classification | DERIVED CURRENTNESS | Preserve `current = context_current && publication_current && repository_context_matches && registry/tool counts match`; `ConnectedCurrent`, `Stale` when core context still matches, otherwise `ReconnectRequired`. Module derives the same final status from captured booleans. |
| Advertisement | DERIVED ELIGIBILITY | Exactly follows current connection publication (`connection_binding.advertised = current` and each effective Tool's `advertised` bit). Module owns this mapping; it grants no dispatch authority. |
| HostExplicit descriptors for the fixed 11 kinds | DERIVED ELIGIBILITY | Use each classified `EffectiveToolEntry`, currentness, selected repository, permission membership, captured repository authority/preparer-presence booleans and `CoordinatorState`. Module continues to call `host_descriptor_with_rename`; no ticket or mutable coordinator crosses. |
| Coordinator observational state | CAPTURED INPUT | Capture only `CoordinatorState::{Idle, ModelTurn, HostPrepared, HostRunning}` after root cleanup. It is a coarse availability fact, not ticket state or an authorization handle. |
| Configured profile intent | CAPTURED INPUT | Root maps existing profile selection to `ConfiguredSummary` counts and sanitized `ExternalToolDescriptor`s. Without a selected profile, preserve BuiltIn counts. Do not pass raw profile/provider objects. |
| Published provider readiness | CAPTURED INPUT | Capture connection phase/error classification and whether an effective composition exists. Do not infer readiness from configured intent or provider metadata. |
| Configured external capabilities missing from composition | DERIVED ELIGIBILITY / presentation input | Preserve existing `ProviderUnavailable` versus `ProviderNotEffective` reason and `external_unavailable` mapping using root-captured sanitized descriptors and error/composition facts. |
| Commit authorization presentation | CAPTURED INPUT | Copy the existing `CommitAuthorizationPresentation` (`Copy`) and selected bool. No workflow state, review object, commit capability, or ticket crosses. |
| Reviewed Commit state | PRESENTATION | Module maps `reviewed_commit(authorization, selected)` into existing `ReviewedCommitState`; no Commit authorization ownership moves. |
| Status labels, repository identity label, connection/source labels | PRESENTATION | Module assembles the existing closed enums and finite labels. Preserve current `Unknown`/`Current`/`Stale`/`NotSelected` mapping and omission/redaction rules. |
| Snapshot schema version and DTO fields | PRESENTATION | Module fully constructs current `EffectiveAuthoritySnapshot` schema version 1 and nested DTOs. No schema/IPC change. |

`selected` must remain a projection of active repository presence, not a count of admitted members. No full repository status, member ID, private repository path, prepared ticket, alias, provider endpoint, runtime handle, permission owner, or Commit handle belongs in the seam.

## Candidate input seam

One internal, owned carrier is justified because the existing result combines one observation point's repository, publication, profile and workflow facts. It is not a new public DTO or authority abstraction.

Conceptual shape only (do not create in Task 402):

```rust
struct EffectiveAuthoritySnapshotInputs {
    repository: RepositorySnapshotFacts,
    connection: ConnectionSnapshotFacts,
    configured: ConfiguredSummary,
    configured_external_tools: Vec<ExternalToolDescriptor>,
    commit_authorization: CommitAuthorizationPresentation,
    coordinator_state: CoordinatorState,
}
```

The named substructures above are descriptive grouping only. Task 403 should prefer one private carrier with direct fields unless implementation shows a genuinely independent owner-bound group. It should contain only:

- repository selected flag, safe display name, current repository generation, captured repository generation, and booleans for the currently consulted repository capabilities/preparers;
- a closed connection phase/source representation, current and captured generation values or root-derived currentness facts, current allowed permissions, classified effective Tool entries, unavailable entries, and the root-derived registry/tool consistency boolean;
- the current configured summary and sanitized configured external descriptors;
- copied `CommitAuthorizationPresentation` and coarse `CoordinatorState`.

Use owned values, not references. This avoids lifetimes and ensures no guard, owner, or borrowed raw object can escape gathering. The carrier is finite and deterministic. The module may consume the carrier by value. Prefer existing types (`EffectiveToolEntry`, `UnavailableCapability`, `ExternalToolDescriptor`, `ConfiguredSummary`, `PermissionLevel`, `CoordinatorState`, and the existing Commit presentation enum) where doing so does not expose executable state. A small closed connection-phase/source representation may be needed because `ConnectionState` contains runtime/error objects; it must carry no runtime or error payload.

Root computes the safe display string and configured profile summary/descriptors before constructing the carrier. It clones the already-present Tool/unavailable presentation values, permission list, and sanitized provider descriptors as the existing code already clones these inventories. Do not clone `DesktopRepository`, `DesktopToolComposition`, registry, preparers, runtime, profile owner, workflow owner, or coordinator into the carrier. No new getter is required by this design.

## Candidate designs compared

| Design | Ownership clarity | Visibility cost | Sensitive-data exposure | Coupling | Testability | Semantic duplication / drift |
|---|---|---|---|---|---|---|
| One private snapshot-input carrier, root gathers, module composes | Clear collection versus deterministic derivation/presentation boundary | One `pub(super)` compose function; carrier can be root-private with private fields because the child module can consume parent-private items | Low when limited to the closed facts above | One bounded carrier couples the module to closed DTO/input types already in it | Pure composition is module-local and deterministic; root gathering remains covered at root | Low if the existing comparison helper remains shared and generation rules are represented once |
| Several narrowly owned input structs | Can mirror owner domains, but risks implying separate abstractions for one observation | More types/constructors and likely more fields/functions visible across the parent-child boundary | Similar if each stays closed; more chances for accidental overexposure | Higher type coupling and orchestration burden | Individually easy to construct; full output still needs combined fixture | Greater risk that optional facts or consistency rules drift between carriers |
| Many function arguments using existing types | No carrier abstraction, but obscures which facts belong to one captured observation | Minimal type visibility, potentially a long signature | Low if arguments stay closed | Signature couples directly to many existing types | Pure function is testable, though fixtures are cumbersome | Call-site ordering and omitted facts become easy to miss |
| Root-precomputed currentness plus module presentation | Preserves shared/root currentness ownership and lock point; aligns with this design | One carrier and compose function | Lowest currentness exposure | Module consumes result facts without owning generation policy | Module tests status/presentation from explicit facts | Low if root's shared comparator remains sole equality rule |
| Move whole stateful helper into `effective_authority.rs` | Blurs state/lifecycle owners with output module | Makes state types reachable but structurally entangles module with root owners; likely broadens visibility for sibling tests | High: repository, profile, runtime, workflow and ticket owners become module dependencies | Broad root-state coupling | Hard to exercise without constructing live root state | High: lifecycle and presentation rules become co-located without becoming single-owner |

Choose one private owned carrier plus root-precomputed cross-owner currentness facts. Do not add independent per-owner carrier APIs or move `DesktopAppState` access into the module.

## Currentness ownership

| Dimension | Generation owner | Current value source | Current comparison location | Task 403 treatment |
|---|---|---|---|---|
| Repository | Repository lifecycle/active projection; `repository_generation` | `state.repository_generation` | Snapshot helper reads it before locking connection, then compares captured value in the connected branch | Root gathers and compares at the existing point; no repository owner transfer |
| Model | `DesktopModelState`/model configuration | `state.model.generation` | Snapshot helper reads it before locking connection and compares in connected branch | Root gathers and compares at the existing point |
| Trusted Profile | Profile selection publication/clear | `trusted_profile_generation` | Read under connection guard; compared in `context_current` | Root gathers/computes under the same guard |
| Connection | Connection lifecycle allocator | `next_connection_generation` | Read under connection guard; compared by `connection_publication_is_current` | Root gathers/computes under the same guard |
| Commit identity | Commit identity update owner | `commit_identity_generation` | Read under connection guard; compared by `connection_publication_is_current` | Root gathers/computes under the same guard |

The generic five-field equality remains in the existing `connection_publication_is_current` root helper, which is also used by publication paths. Do not duplicate or relocate it. Keep `context_current`, repository-context matching, and registry/tool count comparison in the same root gathering flow so generation reads and comparisons remain at their current lock point. Pass closed booleans needed by module composition; the module derives the final `current`, snapshot status, and advertised values deterministically from those facts.

This is intentionally the “root gathers captured/current facts; existing owner derives shared equality; Effective Authority derives reported currentness/status” split. It avoids moving an existing cross-lifecycle comparison into a module that cannot gather those generations without changing lock topology or duplicating policy.

## HostExplicit eligibility seam

`HostInvocationKind` and `host_kind` define exactly 11 supported kinds. Preserve that set exactly. The module may receive only:

- the existing classified `EffectiveToolEntry` values;
- `CoordinatorState` after cleanup;
- current/connected-current fact, selected-repository flag and allowed permissions;
- booleans indicating the existing repository capability/preparer presence.

`host_descriptor_with_rename` remains the existing eligibility/presentation mapper. Its output is a `HostInvocationDescriptor` in the closed snapshot. It describes reported eligibility only; normal host handler checks remain authoritative. Never pass `HostInvocationCoordinator`, `PreparedHostInvocation`, a live ticket/payload, authorization handle, dispatch closure, mutable coordinator, or preparer to the module.

Expiry cleanup must precede reading `CoordinatorState`, preserving current descriptor results and allowing an expired reservation to stop appearing busy. The module receives the resulting coarse state only.

## Repository input seam

The active repository observations actually used are: present/absent; safe basename display; current/captured repository generation; `branch_creation_authority.is_some()`; and presence of the five retained preparers used for HostExplicit reporting. `repo.commit` availability comes from Tool inventory. The snapshot does not use repository status/projection, membership state, repository member identity, repository fingerprint, or repository path beyond root-side safe display derivation.

Root should compute these facts while holding no repository lock after cloning the current `Arc<DesktopRepository>`, exactly as today. No `DesktopRepository`, `PathBuf`, native path, membership handle, authority object, Tool preparer, or new repository getter crosses the seam.

## Trusted Profile and provider input seam

Keep configured intent distinct from effective publication:

- Root captures whether profile selection exists and maps its existing `presentation()` counts to `ConfiguredSummary`; no profile means the current BuiltIn summary based on effective Tool count.
- Root maps selected profile `external_tools()` to the existing sanitized `ExternalToolDescriptor` values. Those descriptors are configuration presentation and do not activate providers or grant permission.
- Root captures the connection phase and, only when connected, the already-published composition, allowed permissions and classified inventory.
- Module preserves current unavailable mapping: with no composition, configured external tools are `ProviderUnavailable` for `ConnectionState::Error(_)`, otherwise `ProviderNotEffective`.
- Profile generation participates in currentness as above. Provider readiness remains a property of published connection/composition state, not configured profile intent.

Do not pass `DesktopTrustedProfileSelection`, provider activation owner, raw provider metadata, endpoints, credentials, stderr, SDK/client, or runtime into the carrier.

## Commit input seam

Pass only the copied existing `CommitAuthorizationPresentation` and active-repository selected bool. The current enum is already the workflow owner's closed presentation state and is `Copy`. Keep `effective_authority::reviewed_commit` mapping to `ReviewedCommitState` inside the module. Do not pass `RepositoryWorkflowState`, staged review, `RepositoryCommitReview`, `DesktopCommitCapability`, `RepositoryCommitControl`, authorization ticket, or review data. No Commit authorization ownership or behavior changes.

## Presentation and redaction ownership

The entire final `EffectiveAuthoritySnapshot` should be assembled in `effective_authority.rs`. This module already owns the serializable schema (`EffectiveAuthoritySnapshot`, nested bindings/summaries/tool/unavailable records and closed enums), classification, source labels, external unavailable mapping and Commit mapping. Root should not retain partial DTO assembly after the seam.

The module remains the final closed/redacted boundary. Preserve schema version 1 and every current serialized field/label, sorting, unavailable reason, `Unknown` versus `Stale` mapping, omitted-field behavior, safe display name rule, and source labels. Do not serialize paths, raw errors, provider metadata, aliases, generations beyond current existing summary fields, registry/definitions, authority/preparer handles, coordinator ticket state/IDs, or Commit review/ticket material.

## Lock topology and temporal consistency

Current helper lock/acquisition sequence (`main.rs:2227-2405`):

1. Lock `repository: Mutex<Option<Arc<DesktopRepository>>>`; clone the `Arc`; temporary guard ends at statement completion.
2. Lock `repository_generation: Mutex<u64>`; copy; guard ends at statement completion.
3. Lock `model: Mutex<DesktopModelState>`; copy `.generation`; guard ends at statement completion.
4. Lock `connection: Mutex<ConnectionState>`.
5. While connection remains locked, lock `repository_workflow: Mutex<RepositoryWorkflowState>` and retain that guard through function return; later read only its `authorization` field.
6. While connection and workflow guards remain held, lock `trusted_profile: Mutex<Option<DesktopTrustedProfileSelection>>`; clone; guard ends after the statement.
7. For a Connected branch, while connection and workflow guards remain held, read `trusted_profile_generation`, then `next_connection_generation`, then `commit_identity_generation`; each temporary guard ends after its copy. Compute context/publication/repository/count currentness while the connection guard remains held.
8. Read allowed permissions/error while connection is held. Explicitly drop the connection guard at current `drop(connection)` (`main.rs:2394`).
9. Clone composition Tool entries/unavailable records and derive root-side repository/profile presentation facts. Acquire `host_invocation: Mutex<HostInvocationCoordinator>` while the workflow guard is still retained; call `reap_expired(now)`, copy coarse state, then release coordinator guard at block end.
10. Complete descriptor, unavailable, summary, repository, and Commit snapshot mapping while the workflow guard remains in scope. The workflow guard is released when the helper returns.

`repository`, generation and model locks are not nested with each other. The nested chain is connection → workflow → profile selection, with connection also enclosing current profile-generation → next-connection-generation → commit-identity-generation reads. After explicit connection release, workflow remains held while coordinator is acquired (workflow → HostInvocation). Preserve this acquisition order, each release point, the connection guard through currentness, the coordinator-only expiry lock scope, and the workflow guard through final compose. Do not acquire state locks from `effective_authority.rs`.

The root carrier and call must not accidentally shorten `repository_workflow` guard duration or extend the connection guard across HostInvocation acquisition. Input copies can be used after their source guards release. Existing cloned values are already used (`Arc<DesktopRepository>`, profile selection, Tool entries, unavailable entries, allowed permissions); new carrier clones should be limited to the same bounded/sanitized values needed after collection.

This is not an atomic cross-owner snapshot today. Repository/model/current generations are captured before the connection lock; the profile selection is cloned separately from its generation; the coordinator is observed later; repository state can change between these acquisitions. Existing generations deliberately detect stale/reconnect context but do not make every observation transactional. Copying captured facts and deriving afterward preserves the exact captured values and branch rules, but a refactor must keep acquisition order and the comparison point to preserve current interleavings as closely as possible. Do not claim an atomic snapshot, eliminate races, add a global lock, or change fail-closed behavior in Task 403.

## Expiry semantics and observation contract

`PreparedHostInvocation::is_expired(now)` expires at `elapsed >= BRANCH_TICKET_TTL` (five minutes). At the current snapshot collection point, root takes the mutable coordinator lock, calls `reap_expired(Instant::now())`, then reads `state()`.

`reap_expired` acts only when state is `HostPrepared` and its retained ticket is expired. It calls `clear_prepared`, removing the retained ticket/payload and returning the coordinator to idle. It does not confirm, dispatch, consume a live ticket, grant authority, advance a generation, activate a provider/repository, persist state, or perform filesystem/Git I/O. `take_prepared` independently rejects an expired ticket and returns the coordinator to idle; therefore the expired ticket is already unusable for execution before snapshot cleanup. The cleanup releases stale coordinator reservation/representation and changes the coordinator's busy/availability state observed by this and later calls.

Classification: **WORKFLOW BOOKKEEPING CLEANUP** for an already-expired, already-unusable ticket. It is a real in-memory coordinator mutation and can change reported HostExplicit availability; it is not a new semantic authority grant/revoke or execution action. Keep `reap_expired` in the existing `HostInvocationCoordinator` owner and keep the snapshot command's cleanup in root gathering immediately before coarse-state capture (Model B). Do not remove, relocate into `effective_authority.rs`, or broaden `reap_expired`.

The HostInvocation unit test `ticket_matrix_is_single_use_for_valid_expired_cancelled_and_stale_paths` (`host_invocation.rs:1282`) covers expired ticket cleanup/non-reuse at the coordinator level. No test found asserts that the snapshot helper itself reaps an expired ticket. Existing docs' “zero lifecycle/Tool/repository/chat/authority/persistence side effects” phrasing is broader than this exact cleanup behavior if “lifecycle” includes coordinator reservation bookkeeping. The inspection contract should receive a separate documentation clarification describing this narrow expiry cleanup and distinguishing it from authorization/execution. A focused snapshot-expiry test may be considered in that documentation-contract follow-up if needed; it is not required to establish the Task 403 data seam. No ADR is warranted unless a later policy decision proposes changing whether inspection may reap expiry.

## Error semantics

`effective_authority_snapshot_for_state` returns `EffectiveAuthoritySnapshot`, not `Result`. It has no surfaced state-availability, poisoning, missing repository, invalid composition, or coordinator error branch:

- Poisoned mutexes recover using `PoisonError::into_inner`.
- Missing active repository maps to `NoRepository` or `Disconnected` and closed repository binding values; it is not an error.
- `ConnectionState::Error(_)` maps to `Unavailable`; its error details are not serialized. Configured external descriptors map to `ProviderUnavailable` in this case.
- Invalid/stale publication facts, repository-context mismatch, or registry/tool-count mismatch suppress advertisement and map through `Stale` or `ReconnectRequired` according to existing branches; they are not errors.
- Coordinator expiry cleanup has no error return; only coarse state is observed.
- Current root safe display failure becomes `None`, not an error.

Future seam constraint: preserve the infallible return and every existing label/closed fallback. Lock acquisition, clone, and poison recovery remain ROOT gather behavior. The deterministic module function should not invent a new derivation error. Do not change user-visible error strings or introduce partial-snapshot errors.

## Pure derivation feasibility

After root has gathered and copied the listed facts, performed expiry cleanup, and computed shared generation equality facts at the existing lock point, `effective_authority.rs` can own a deterministic function conceptually equivalent to:

```rust
fn compose_effective_authority_snapshot(
    inputs: EffectiveAuthoritySnapshotInputs,
) -> EffectiveAuthoritySnapshot
```

The function can be pure, mutation-free, deterministic for equal inputs, I/O-free, persistence-free, runtime-dispatch-free, ticket-free, and repository-path-free. It would derive final current/status/advertisement and HostExplicit descriptor fields from closed facts, then assemble the complete existing DTO. It must not call `reap_expired`, inspect a live lock/owner, activate or construct composition, issue tickets, or make external calls.

The only current mutation blocker is the coordinator `reap_expired`; keeping that operation in root gathering before capturing `CoordinatorState` removes it from the derivation. The existing shared `connection_publication_is_current` comparison remains root-side and is included as a fact so it is not duplicated. The module does not own all currentness equality; it owns the snapshot's combined reported status and gating derived from those root facts.

## Preferred ownership split

| Work | Owner | Frozen behavior |
|---|---|---|
| Tauri handler and invocation contract | ROOT | Keep `get_effective_authority_snapshot(State<DesktopAppState>) -> EffectiveAuthoritySnapshot` unchanged; no input/output/schema/registration change. |
| Locks, poison recovery, gathering, clones, safe basename computation | ROOT | Preserve exact existing acquisition order, lock release points, and source facts. |
| Shared five-generation comparison and its inputs | ROOT/shared existing helper | Keep `connection_publication_is_current`; compute at its current connection-guard point. |
| Expiry cleanup and coarse coordinator-state read | EXISTING OWNER called by ROOT | Keep `reap_expired(Instant::now())` in `host_invocation.rs`; root calls it before `state()` and before constructing the module input. |
| Existing authority owners | EXISTING OWNER | Continue owning selection, activation, registry construction, provider/runtime lifecycle, repository capability and Commit authorization. |
| Currentness combination, status/advertised result, HostExplicit descriptor eligibility presentation, configured/unavailable/reviewed Commit mapping | EFFECTIVE_AUTHORITY MODULE | Deterministic mapping from gathered closed facts; never dispatch authority. |
| Final DTO/redaction | EFFECTIVE_AUTHORITY MODULE | Build complete unchanged schema-version-1 snapshot. |

## Visibility and module direction

`effective_authority` is a child module of `main.rs`'s crate root. The root caller cannot access a child module's private function/type/fields; the child can access private ancestor items. Keep the input carrier private in `main.rs` with private fields so only root can construct it. The child module may consume this parent-private type through normal descendant privacy without making its fields crate-visible.

Expected minimum visibility changes: one new module function, `pub(super) fn compose_effective_authority_snapshot(...)`, because the crate root calls into its child. No new public or `pub(crate)` API, no public carrier fields, no `DesktopAppState` access from the module, and no new getter. Existing root `effective_authority_snapshot_for_state` stays private as the state-gathering wrapper, preserving same-parent/sibling test access. `get_effective_authority_snapshot` stays private and unchanged. Existing snapshot types retain current visibility; do not widen visibility for tests.

## Testability and caller impact

Keep existing `main_tests.rs` and ignored/live scenario call locations. The private root gathering helper remains their entry point and continues to gather real state/perform existing expiry cleanup. Add deterministic module-local tests for the pure carrier-to-snapshot composition, including all connection phases, stale/reconnect/current branches, registry/tool-count mismatch, repository absent/present and capability flags, all 11 HostExplicit descriptors, profile configured-but-ineffective/error mapping, Commit mapping, and closed serialization/redaction. Keep existing root integration and live tests in place; do not relocate certification tests or activate ignored/live paths.

The existing `effective_authority.rs` closed serialization and classification tests remain. No existing live tests need changes or reruns in Task 402. Task 403 should update a caller only if compilation requires it; do not expand into certification or IPC work.

## Implementation staging options

| Option | Assessment |
|---|---|
| 1. One-step carrier plus move of derivation/presentation | Preferred for Task 403. The root helper continues state gathering and existing currentness comparison; builds one closed carrier; calls a pure module function that now owns all status/eligibility/DTO mapping. Scope is bounded because no root owner or lock moves. Preserve lock scopes explicitly. |
| 2. Add structured input but temporarily keep calculation in root | Rejected. It creates a carrier that has no ownership consumer and leaves the 270-line mixed calculation in root; a later move would repeat edits or temporarily duplicate derivation. No meaningful intermediate contract is needed once the input boundary is frozen. |
| 3. Separate expiry cleanup first | Rejected as an implementation prerequisite. Root gathering can explicitly retain cleanup before coordinator capture while the module function remains pure. Changing/removing/repositioning cleanup is outside scope. The documentation wording clarification is a separate follow-up, not a prerequisite for this behavior-preserving seam. |
| 4. No implementation until deeper temporal/lock research | Rejected. The helper is non-atomic today, but lock ordering, comparison point, release points, expiry position, and captured branch rules are observable and can be frozen as constraints. No claim of stronger coherence is required. |

## Chosen strategy and frozen Task 403 boundary

**Chosen strategy: A. BOUNDED DESIGN READY FOR IMPLEMENTATION.** Use Option 1, one-step carrier plus move of derivation/presentation, with currentness equality kept at root under existing lock topology.

Freeze Task 403 as follows:

- **Target files:** `crates/rah-desktop/src/main.rs` and `crates/rah-desktop/src/effective_authority.rs`; update existing test code only if required by the same function move, without relocating certification tests. No other production files, ADRs, frontend, Tauri permissions/capabilities, or IPC contracts.
- **New internal type:** one private `EffectiveAuthoritySnapshotInputs` carrier in `main.rs`, private fields, owned values only.
- **Carrier fields:** selected bool; safe display name; current repository generation and connected captured repository generation; repository branch-authority and five preparer-presence booleans; closed connection phase and runtime source presentation; connection DTO's existing captured repository/model/connection generations; root-computed context/publication/repository-context and registry/tool-consistency facts (the raw current profile/connection/identity generations are not carried); allowed permission vector; classified Tool and unavailable vectors; configured summary; sanitized external descriptors; connection-error/composition-presence facts needed for current unavailable mapping; copied Commit authorization presentation; coarse coordinator state.
- **Function to add:** `pub(super) fn compose_effective_authority_snapshot(inputs: EffectiveAuthoritySnapshotInputs) -> EffectiveAuthoritySnapshot` in `effective_authority.rs`. It performs no lock/I/O/mutation and returns no error.
- **Function to move/reduce:** move all status selection, connection/repository binding and labels, advertised mapping, HostExplicit descriptor mapping, unavailable external mapping, configured summary/output mapping as needed, reviewed Commit mapping, and final `EffectiveAuthoritySnapshot` construction from `effective_authority_snapshot_for_state` into the module function. Root may continue safe-name/profile-summary/sanitized-descriptor preparation as input gathering.
- **Functions retained:** `get_effective_authority_snapshot` and private `effective_authority_snapshot_for_state` remain in root. Keep `connection_publication_is_current`, `safe_repository_display_name`, `reap_expired`, `host_descriptor_with_rename`, `source_label`, `external_unavailable`, and `reviewed_commit` at their existing owners unless the new module function merely calls existing effective-authority mappers.
- **Expiry owner/position:** root calls existing coordinator `reap_expired(Instant::now())` under its current lock, immediately before copying `CoordinatorState`, before module compose. Keep the expiry test and behavior unchanged.
- **Lock owner/topology:** root exclusively. Preserve the acquisition chain, currentness comparison while connection is held, explicit connection drop before HostInvocation lock, workflow lock through module compose/return, and coordinator lock only across reap/state read. Do not claim atomicity or move lock acquisition into the module.
- **Currentness owner:** root/shared comparator owns five-generation equality. Module combines the captured root facts into the same current/status/advertisement mapping. Do not duplicate or change comparison semantics.
- **Presentation owner:** effective-authority module owns the complete closed snapshot and redaction boundary.
- **Visibility budget:** one `pub(super)` compose function; one private root carrier with private fields; no other visibility changes or getters.
- **Test impact:** preserve current root integration callers; add module-local deterministic composition tests; do not move existing tests. Keep ignored/live tests untouched and do not run live certification as part of this docs task.
- **IPC/authority constraints:** command name/signature/registration and DTO serialization stay unchanged. No new authority, permissions, Tool, HostExplicit eligibility, ticket, runtime/provider/repository/Commit ownership, activation, or dispatch behavior.

## Non-goals

No implementation, Rust edits, tests, visibility or authority changes in Task 402; no DTO/API or IPC changes; no `reap_expired` removal/relocation; no permission/capability changes; no new ADR; no architecture/security policy rewrite; no cargo/workspace/live validation; no push/tag; no automatic Task 403 start.

## Validation and closeout

- Required validation: `git diff --check` only.
- Artifact: `docs/plans/2026-09-24-task-402-effective-authority-snapshot-derivation-presentation-seam-design.md`.
- Commit only this file as `docs: design effective authority snapshot seam`.
- Do not push or tag.
