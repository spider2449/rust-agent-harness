# Task 401 — Effective Authority Snapshot Ownership Boundary Research

Date: 2026-09-24
Scope: Research only; no Rust/test/visibility/authority changes.

## Verdict

**PASS — EFFECTIVE AUTHORITY OWNERSHIP BOUNDARY REQUIRES DESIGN**

The root calculation is **C. A MIXTURE OF AUTHORITY DERIVATION/GATING AND PRESENTATION ASSEMBLY**. It derives whether a connected composition is current, whether Tools are advertised, and which HostExplicit operation descriptors are presently eligible; then it creates the closed Desktop DTO. The derivation reads separately owned live states. `effective_authority.rs` owns the snapshot/presentation types and Tool classification/composition layer, but it does not own state gathering or cross-owner currentness.

There is no bounded, body-equivalent extraction into the existing module that preserves ownership cleanly without making that module consume the full root state graph. A narrow future seam is plausible, but its inputs and division of currentness/eligibility/presentation responsibilities require design first. No production boundary is frozen by this task.

## Checkpoint and scope

- Expected starting HEAD: `909748a82632898c71d8614da28413c7a2423a54` (`docs: research desktop test topology decomposition`).
- Verified starting HEAD: exact match.
- Verified starting worktree: clean (`git status --short` empty).
- `crates/rah-desktop/src/main.rs`: 9,879 physical lines.
- `crates/rah-desktop/src/effective_authority.rs`: 906 physical lines.
- All current inventory/counts below are from this checkpoint. No Task 402 work was started.

## Symbol inventory

### Root calculation and endpoint

| Symbol | Source range / visibility | Callers | State read / written | Role |
|---|---|---|---|---|
| `get_effective_authority_snapshot` | `main.rs:2219-2223`; private Tauri handler | One `generate_handler!` registration at `main.rs:9836`; test/live callers below | Delegates to state helper; no direct write | Handler adapter only. |
| `effective_authority_snapshot_for_state` | `main.rs:2225-2497`; Windows-gated, private | Handler; 35 direct test-source references across 12 caller functions | Reads repository, repo/model/profile/connection/identity generations, connection/composition/permissions, workflow authorization, profile selection and HostInvocation coordinator. Calls expiry cleanup, which can clear expired prepared state. | Mixed state gathering, currentness/availability derivation, HostExplicit eligibility derivation and sanitized DTO assembly. |
| `ConnectionPublicationCurrentness` | `main.rs:1999-2007`; private | Currentness helper and connection publication code | Captured/current generation tuple | Internal currentness input. |
| `connection_publication_is_current` | `main.rs:2009-2015`; private | Snapshot helper plus connection publication/currentness call sites | No state access/write | Equality rule over repository, model, profile, connection and commit-identity generations. |
| `DesktopAppState` | `main.rs:392-438`; private (snapshot fields `393-430`) | Desktop host and root helpers | Owns locks for inputs below | Aggregate storage owner, not snapshot-specific state. |
| `ConnectionState` | `main.rs:239-257`; private | Snapshot and connection/lifecycle code | Captured generations, runtime source, composition, allowed permissions, connection phase | Published runtime/composition decision consumed by snapshot. |
| `DesktopRepository` | `main.rs:1194-1206`; private | Repository lifecycle/composition | Root/display path, observation tools, optional branch/directory/delete/rename authorities | Active selected repository and authority objects; snapshot uses safe display and authority presence, not membership catalog. |
| `DesktopCommitIdentity` / `DesktopCommitCapability` | `main.rs:1286-1321`; private | Commit identity and composition workflows | Identity preference and repo/model/identity captured commit capability | Snapshot does not read either value; the connection's captured identity generation gates currentness, while workflow authorization presentation is read separately. |
| `RepositoryWorkflowState` | `main.rs:1323-1335`; private | Repository workflow/commit code | Observation/review/action data and authorization presentation | Snapshot reads only commit authorization presentation. |
| `CommitAuthorizationPresentation` | `main.rs:1384-1398`; private | Commit workflow and snapshot mapper | Already-derived state; no ticket passed | Existing review/authorization presentation. |
| `RepositoryMembershipState` / membership presentation | `repository_membership.rs`; root presentation at `main.rs:7208-7219` | Admission, activation, membership commands | Member/admission identities | Not read by snapshot; active `repository` is its projection. |
| `DesktopTrustedProfileSelection` | `trusted_profile_selection.rs:15` onward; type `pub(crate)`, fields module-private | Profile selection/provider composition; snapshot | Static validated profile, presentation and external descriptors | Configured intent/provider inventory, not active provider authority. |
| `DesktopModelState` | `main.rs:1717-1723`; private | Model configuration and connection publication | Selection, generation and readiness; snapshot reads generation only | Model generation is a currentness input, not model content or permission. |
| `DesktopProviderActivation` | `provider_composition.rs:17-24`; `pub(crate)` | Connection composition/lifecycle | Effective profile composition, configured permissions and external descriptors | Not directly read by snapshot; its published ToolRegistry/permissions are retained through `ConnectionState`. |
| `RepositoryMemberId`, `InertRepositoryMember`, `WorkspaceMembershipState` | `repository_membership.rs:17-79`; `pub(crate)` | Membership and admission methods in that module; root active-repository lifecycle | Process-local member/admission identity, roots, active ID and membership generation | Not read by snapshot; membership identity is not promoted to Effective Authority. |
| `HostInvocationCoordinator`, `CoordinatorState`, `reap_expired`, `state` | `host_invocation.rs:340-375`; coordinator and methods `pub(crate)` | HostExplicit lifecycle and snapshot | Prepared operation state; snapshot mutably reaps expiry, then reads coarse state | HostExplicit workflow availability input; no ticket is returned by snapshot. |
| `HostInvocationDescriptor`, `host_descriptor_with_rename` | `host_invocation.rs:57-65`, `569-626`; descriptor and function `pub(crate)` | effective-authority composition, root snapshot and HostExplicit tests | Derived eligible/kind/unavailable reason; pure mapping from passed predicates | Reports capability availability, not dispatch authorization. |

The endpoint is small. The helper is 273 physical source lines (`2225-2497`); endpoint plus helper is `2219-2497` (279 lines). These exact ranges supersede prior approximate counts.

### `effective_authority.rs` inventory

The module is Windows-gated at line 1. Listed API types/functions use `pub(crate)`; DTO fields use `pub` within the crate. Unlisted helpers are private.

| Symbols / range | Current responsibility and callers |
|---|---|
| `SnapshotStatus`, `RepositoryKind`, `RepositoryIdentity`, `ConnectionBindingState`, `SourceKind`, `EffectClass`, `AuthorityCategory`, `UnavailableState`, `UnavailableReason`, `ReviewedCommitState` (`18-123`) | Closed serializable status/classification vocabularies; consumed by DTOs, root derivation, composition and tests. |
| `RepositoryBinding`, `ConnectionBinding`, `ConfiguredSummary`, `EffectiveToolEntry`, `UnavailableCapability`, `EffectiveAuthoritySnapshot` (`125-198`) | Closed serializable Desktop response DTO. Fields: schema version, status, repository, connection, configured summary, effective Tools, unavailable capabilities, reviewed Commit status. No executable ticket/registry is serialized. |
| `ExternalToolDescriptor` (`200-210`), `CompositionError` (`212-217`), provider label constants (`219-220`), `DesktopToolComposition` (`222-233`) | Provider metadata carrier/error and internal effective registry composition. Composition retains `ToolRegistry`, expected definitions, classified Tools/unavailable records and preparers; these are not wire output. |
| `external_tool_descriptors` (`237-271`) | Builds sorted external labels/classifications from `TrustedStaticProfile`; used by `provider_composition.rs`, `trusted_profile_selection.rs`, and tests. Does not activate providers. |
| `sanitize_provider_label` (`273-286`), `metadata` (`288-335`), `make_unavailable` (`337-344`) | Private bounded labeling, fixed built-in Tool classification and unavailable DTO construction. Classification is not a permission grant. |
| `compose` (`346-583`) | Production callers: root wrapper (`main.rs:8103`) and `provider_composition.rs:333`. Classifies registry definitions, validates external descriptor correspondence/permissions, creates preparers from selected repository and emits unavailable records from missing repository authority/tool presence. Authority-adjacent composition, beyond serialization. |
| `external_unavailable` (`585-595`), `source_label` (`597-603`), `reviewed_commit` (`605-628`) | Root snapshot helpers. `reviewed_commit` maps existing `CommitAuthorizationPresentation` plus selected flag to closed status; it does not inspect/consume a ticket. |
| Unit tests (`630-906`) | Classification/composition/closed serialization tests, including `complete_snapshot_serialization_is_sanitized_and_closed`; no state-gathering test or pure full-snapshot calculator. |

The module owns DTO schema, closed labels/enums, Tool registry classification/composition and pure DTO mappers. It does not own `DesktopAppState`, connection/repository selection and generations, provider/runtime lifecycle, cross-owner currentness, or workflow commit authorization. Root gathers these state dimensions. Repository evidence supports the existing split but does not show whether it was intended as final design; “DTO/composition module plus root state gathering/currentness” is the evidence-based explanation. Tauri/root state coupling is a technical constraint, not by itself an ownership argument.

## Authority input graph

```text
repository_membership/admission -> active repository projection -> repository_generation --+
model configuration -> model.generation -----------------------------------------------+
Trusted Profile selection -> selected static profile -> profile_generation ------------+-> captured ConnectionState tuple
connection lifecycle -> connection_generation -----------------------------------------+   + identity_generation
                                                                                           + composition / permissions
                                                                                                     |
                                                                                                     v
                                                                                       currentness / advertised status
                                                                                                     |
DesktopRepository optional authorities + preparers + ToolRegistry + coordinator state -> HostExplicit descriptors
RepositoryWorkflowState.authorization -----------------------------------------------> reviewed Commit display
                                                                                                     |
                                                                                                     v
                                                                                         closed snapshot DTO
```

| Input | Owner/source and fields read | Character | Scope/currentness |
|---|---|---|---|
| Repository selection/observation | `DesktopAppState.repository`; `DesktopRepository.root`, safe display name and optional authorities | Selected active repository is authority-bearing context; display name descriptive; optional authority objects indicate existing gates | Process-local, repository-scoped. No fresh repository probe. |
| Admission/membership | `repository_membership` owns member/admission identities; active repository is published to `repository` | Descriptive/lifecycle until active composition; snapshot does not enumerate membership | Not directly read. `repository_generation` is compared. |
| Trusted Profile | `trusted_profile`, `trusted_profile_generation`, `profile.presentation()`, `external_tools()` | Selection/configuration descriptive; published Tool composition effective; provider metadata cannot self-grant | Process/profile-scoped; generation gates currentness. |
| Provider/runtime | `ConnectionState`; connected source/runtime, composition, allowed permissions | Host-decided published state; snapshot does not activate/reload/spawn/reconnect/dispatch | Runtime/connection-scoped; captures repo/model/profile/connection/identity generations. |
| ToolRegistry | `composition.registry.definitions()`, `composition.tools` | Effective composition is authority-bearing context; classification/count consistency is derived; display is not execution authorization | Runtime/composition-scoped; advertisement requires currentness. |
| HostExplicit | `host_invocation`, `CoordinatorState`, repository optional authorities/preparers, `allowed_permissions`, `host_descriptor_with_rename` | Capability class/eligibility only; no ticket payload/ID exposed; coarse coordinator state is observed and expiry is reaped | Host/process workflow-scoped; repository operations also repository-scoped. |
| Review/Commit | `RepositoryWorkflowState.authorization`, selected flag | Review/authorization presentation, not Commit capability/token | Repository/workflow-scoped; decisions owned by Commit workflow. |
| Generations | Repository, model, profile, connection and commit identity | Equality guards, not authority tokens; snapshot derives present usability of captured composition | Process-local, cross-owner. |
| Model readiness | Connected status and captured model generation; model configuration details not read | Runtime readiness/currentness; model/provider metadata does not grant authority | Model generation compared. |

The helper does not read remembered workspace candidates, persistence, admission catalog, member ID, or provider activation internals. It reports one active projection and current composition, not every admitted repository.

## Branch classification and currentness

| Range | Branch/calculation | Classification |
|---|---|---|
| `2227-2254` | Clone repository, read repo/model generations, lock connection/workflow/profile, derive selected | **MIXED** state gathering and selected-repository presentation predicate. |
| `2255-2307` | Connected tuple checks: repo/model/profile context equality; publication equality additionally includes connection/identity; selected-context and registry/tool-count checks | **AUTHORITY GATING** for whether composition may currently be advertised. Does not authorize execution. |
| `2308-2385` | Connection lifecycle variants mapped to status/binding DTO | **PRESENTATION** with currentness/status mapping. |
| `2386-2394` | Read permissions and connection error | **DESCRIPTIVE INPUT** for eligibility/unavailable mapping. |
| `2395-2405` | Clone composition metadata and read coordinator after expiry cleanup | **MIXED**, including mutation (see audit). |
| `2406-2434` | Set advertised bit and HostExplicit descriptor from status, selection, permissions, repo authority/preparers and coordinator state | **DERIVED AUTHORITY / AUTHORITY GATING** for reported operation availability, not dispatch authorization. |
| `2435-2452` | Report composition-unavailable or configured external Tools not effective | **DESCRIPTIVE + PRESENTATION**. |
| `2453-2464` | Configured source/provider/capability counts | **PRESENTATION**. |
| `2465-2486` | Safe repository binding, generation fields, Current/Stale/Unknown labels | **MIXED** redaction and currentness-derived identity. |
| `2487-2496` | Build version-1 DTO and map workflow authorization to reviewed Commit state | **PRESENTATION** over existing state. |

The root calculation is not presentation-only: it recomputes currentness and HostExplicit eligibility. It is also not the sole authority composer: the connected ToolRegistry and most capability-specific decisions are owned elsewhere. It is a cross-owner observation with authority-sensitive gating and a closed presentation boundary.

## Mutation audit

Snapshot collection is **not strictly read-only at the in-memory workflow-state level**:

- `main.rs:2398-2404` takes a mutable `host_invocation` lock and calls `HostInvocationCoordinator::reap_expired(Instant::now())` before reading state. `host_invocation.rs:363-372` clears an expired `HostPrepared` ticket via `clear_prepared`, changing coordinator state/prepared data to cleared/idle. This is time-driven expiry cleanup, not an authorization grant, dispatch consumption, provider/runtime mutation or persistence write.
- No repository activation, generation advancement, ticket confirmation/consumption, ToolRegistry mutation, provider/runtime lifecycle change, persistence write, filesystem/Git access or lazy authority construction was found in this helper. It clones the repository handle and reads/clones existing composition/preparer state.
- The output remains observational for execution. Cleanup can affect subsequent HostExplicit busy/eligibility state, so “literally zero state/workflow effects” is inaccurate.

This differs from broad statements at `docs/ARCHITECTURE.md:1005-1007` and `docs/SECURITY.md:1153-1157` that inspection has zero lifecycle/authority side effects. Carry this narrow implementation/documentation discrepancy into separate follow-up analysis; Task 401 does not alter it.

## Currentness and generation semantics

Connected state captures repository, model, profile, connection and identity generations (`main.rs:243-253`). `connection_publication_is_current` compares captured/current five-field structs for equality (`1999-2015`). The snapshot computes:

1. `context_current`: repository, model and Trusted Profile generation equality (`2278-2286`).
2. `publication_current`: equality over all five values (`2287-2302`).
3. `current`: both checks, selected repository context (or captured repository generation zero), and `registry.definitions().len() == composition.tools.len()` (`2303-2307`).

Current becomes `ConnectedCurrent`; failed currentness with `context_current` becomes `Stale`; changed repo/model/profile context becomes `ReconnectRequired` (`2308-2314`). Stale publication/identity suppresses advertisement; changed core context requests reconnect. The snapshot derives these combined status/advertisement semantics; it does not advance generations. `RepositoryWorkflowState.observation_generation` is not used here. `connection_publication_is_current` is also used in publication flows.

Owners: repository generation changes with repository activation/close (`main.rs:6153`, `6690-6717`, `7379`); model generation belongs to model configuration state; profile generation advances on selection publish/clear (`4663-4682`); connection generation is allocated during connect (`8220-8225`); commit identity generation is owned by commit-identity updates and compared here. Equality generations are guards, not capabilities.

## Presentation/redaction and HostExplicit

`EffectiveAuthoritySnapshot` and nested DTOs (`effective_authority.rs:18-198`) are the closed serializable Desktop presentation. They contain public Tool names/classifications, safe source labels, selected/display-name/generation summaries, status, advertised flags, unavailable reasons, HostInvocation eligibility descriptors and reviewed Commit state. They omit raw repository paths, runtime objects, provider endpoints/stderr/secrets, private Tool aliases, raw identity/currentness handles, ToolRegistry/preparers, coordinator tickets/IDs and Commit authorization tickets. Runtime source maps to a finite label (`source_label`); repository display uses `safe_repository_display_name`.

Tests include `complete_snapshot_serialization_is_sanitized_and_closed` (`effective_authority.rs:635` onward), root serialized-output assertions around `main_tests.rs:20083`, and stale/no-active/restart assertions. Backend is the redaction boundary. `ARCHITECTURE.md:981-1010` and `SECURITY.md:1140-1157` describe the DTO as closed/sanitized and observational. Presentation responsibility explains the DTO types in the module, but not root currentness/eligibility derivation.

HostExplicit relationship:

- `HostInvocationKind` (`host_invocation.rs:29-41`) has **exactly 11** variants: `FsRead`, `RepoFileInfo`, `RepoStatus`, `RepoDiff`, `RepoDiffStaged`, `RepoCreateBranch`, `RepoPatch`, `RepoEditFiles`, `RepoCreateFile`, `RepoDeleteFile`, `RepoRenameFile`.
- Snapshot calls `host_descriptor_with_rename` (`main.rs:2411-2433`; mapping in `host_invocation.rs:553-626`) to report operation class/eligibility using composition, permission, repository/authority/preparer and coordinator predicates.
- It does not inspect prepared/authorized operation payloads or issue/consume tickets. It does inspect coarse coordinator state (`Idle`, `ModelTurn`, `HostPrepared`, `HostRunning`) after expiry cleanup.
- It derives/reports **availability descriptors**, not executable HostExplicit authorization. Dispatch still revalidates host policy and operation-specific authorization.

## Repository, Trusted Profile/provider and runtime relationships

- Repository authority is **aggregated, not owned** by this snapshot. Active selection comes from the root projection; membership/admission are not calculated. Composition has already been built from the selected repository and registry. Root derives selected/current/stale state, safe display and descriptor availability from existing optional authority, preparer, permission and currentness state; it does not grant repository permissions.
- Trusted Profile selection is not effective authority. It supplies configured counts and, without an effective composition, configured external Tool descriptors marked unavailable/not effective. Connected `DesktopToolComposition` is the effective inventory. Profile generation gates currentness. Provider activation/readiness is consumed from connection/composition owners.
- Model generation/runtime connection describe readiness/currentness, not model-derived authority. No model request/provider metadata grants or escalates capabilities.
- The snapshot both reports existing authority-bearing composition and derives whether it is currently reportable/advertised, plus HostExplicit presentation eligibility. It is not itself runtime authority composition or the execution gate.

## Caller topology

Current source scan:

- Production handler: one `tauri::generate_handler!` registration at `main.rs:9836`; it delegates once to the helper. No other production snapshot caller found.
- `main_tests.rs`: 35 direct handler/helper expressions in 12 caller functions.
- Two nonignored deterministic tests contain 4 expressions total: `task_359_linked_worktrees_use_one_revalidated_active_composition` (2) and `task349_close_withdraws_active_state_and_preserves_membership_and_persistence` (2).
- `task361_run_live_scenarios` has 3 helper references and is called by the Task 361 live gate, so it is live scenario support even though it is not itself ignored.
- Nine ignored caller functions contain 28 expressions:
  - `windows_live_desktop_hostexplicit_create_file` (1)
  - `windows_live_desktop_hostexplicit_delete_file` (1)
  - `windows_live_desktop_hostexplicit_rename_file` (1)
  - `windows_live_desktop_explicit_host_tool_invocation` (6)
  - `windows_live_desktop_hostexplicit_repo_patch` (4)
  - `windows_live_desktop_hostexplicit_multi_file_edit` (2)
  - `task_324_c_windows_host_driven_two_repository_live_certification` (3)
  - `task_335_windows_host_driven_remembered_workspace_live_certification` (1)
  - `task_352_windows_active_repository_close_live_certification` (9)

Total test expressions: 4 ordinary deterministic + 3 live-helper + 28 ignored/live = 35. Module production callers: `compose` via `main.rs:8103` and `provider_composition.rs:333`; descriptor construction via provider/profile composition; `external_unavailable`, `source_label` and `reviewed_commit` via root snapshot. Serialization/composition functions also have module/provider unit-test callers. No full snapshot calculator currently lives in the module.

| Live scenario family | Callers | Asserted property and handler necessity |
|---|---|---|
| HostExplicit create/delete/rename, explicit invocation, `repo.patch`, multi-file edits | Six certified Codex gate functions above | Connected/current and operation eligibility/reviewed Commit around host operation. End-to-end wiring asserts handler; underlying calculation is the semantic subject. |
| Two-repository isolation | Task 324 | A/B active composition/currentness is isolated; snapshot is evidence, not authority. |
| Remembered Workspace | Task 335 | Serialized snapshot proves restart restores no executable repository authority; output is certification evidence. |
| Active repository Close | Task 352 | Selected/current, closed/inactive, reactivated and stale-close presentation; helper semantics needed, not necessarily Tauri wrapper for every observation. |
| Linked worktree deterministic | Task 359 | Active-only composition for selected worktree. |
| Close deterministic | Task 349 | Close/reopen authority presentation. |

Task 400's finding remains: test topology is secondary; production ownership is the larger issue.

## Natural ownership cuts and visibility consequences

| Option | Assessment | Minimum visibility/API consequences if pursued |
|---|---|---|
| A. Keep root ownership | Defensible for cross-owner state gathering/currentness; leaves DTO/composition split. | None; handler/helper private, DTOs `pub(crate)`. |
| B. Move pure calculation into `effective_authority.rs` | Future design, not mechanical: root must gather a coherent input set and module deterministically derive output. No such carrier/full-snapshot pure function exists today. | Likely one narrow parent-visible input carrier and calculation function (`pub(super)`/crate-internal as needed by the chosen module layout). Avoid exposing raw root state/getters. |
| C. Move presentation/redaction only | DTO schema/mappers already live in module, but root directly constructs DTO; a real seam needs an intermediate derived-state representation absent today. | Likely a crate-private intermediate/result type and mapper; no public API, but requires design. |
| D. Move full stateful ownership into module | A child module can reach parent-private Rust items, but it would directly couple to `DesktopAppState`, `ConnectionState`, repo/workflow/model/profile generations, coordinator, root helpers and Tauri `State`; this is a broad aggregator move. | Handler likely `pub(super)` for registration. Sibling tests need accessible helper (`pub(super)`/`pub(crate)`) or relocation. DTO visibility stays `pub(crate)`; new getters avoidable only by direct parent-state coupling. |
| E. Define smaller internal seam first | Best research path: define coherent captured inputs and decide which currentness/expiry/eligibility decisions remain with lifecycle owners versus pure output mapping. Do not invent the carrier now. | Candidate narrow crate-private input/result types and one function; freeze exact visibility only after design. |

Moving only the handler is not meaningful (three-line adapter). Moving the full helper verbatim is syntactically plausible but relocates a stateful aggregator while state/authority owners stay in root; it does not create a coherent ownership boundary.

## Test impact

A future seam could preserve test locations if an appropriate helper remains sibling-accessible. A module-private moved function cannot serve current sibling `main_tests` direct callers. The 35 references across 12 functions mix handler calls, private helper calls and live helpers. An implementation task must preserve those contracts or explicitly update callers; module tests currently cover classification/serialization, not state gathering. No test relocation is required by evidence. Ignored/live scenarios stay ignored; no live certification is authorized here.

## Architecture/ADR consistency

- `ARCHITECTURE.md:979-1010` and `SECURITY.md:1138-1157` explicitly define a backend-sanitized closed observational DTO; display/advertisement does not imply execution authority. Implementation/schema align.
- ADR 0011 (Trusted Profile), ADR 0021 (HostExplicit dispatch), ADR 0027 (repository identity/authority composition) preserve separation of profile intent, active repository composition, Tool execution and human HostExplicit workflow. Current reporting does not replace those owners; no ownership contradiction found.
- Expiry cleanup is a narrow code/documentation discrepancy with broad zero-side-effect claims above; recommend separate contract clarification. No new ADR or authority model is warranted by this research.
- HostExplicit remains exactly 11; no authority category, permission, capability or IPC field change is proposed.

## Mechanical extraction assessment and chosen outcome

**No mechanical extraction is frozen.** Moving only the endpoint has no ownership gain; moving the helper verbatim directly couples the module to all root state dimensions and carries expiry mutation; moving only DTO assignments requires an intermediate derived-state carrier; moving currentness alone still requires a defined captured-input/currentness contract.

Chosen required classification:

**B. OWNERSHIP BOUNDARY EXISTS BUT REQUIRES DESIGN WORK**

A production boundary was **not** frozen. No IPC/schema, permission, currentness or authority semantic change is authorized.

## Exact next-task scope — recommended Task 402

**Task 402 — Design the Effective Authority Snapshot derivation/presentation seam.** Research/design only. Specify candidate root-gathered coherent inputs and decide which derivations stay with root lifecycle owners versus which pure closed-output mapping belongs beside `EffectiveAuthoritySnapshot`. Account for HostInvocation expiry cleanup ownership/state capture and all five generations, active repository projection and optional authority state, Trusted Profile intent versus published provider composition, allowed permissions, commit review presentation, and registry/tool-count consistency. Freeze the narrowest visibility budget and explain treatment of the 35 caller expressions, including ignored/live paths. No Rust implementation, test relocation, visibility widening, authority changes, ADR/IPC/schema changes, live certification, commit/push/tag, or Task 403 start.

## Non-goals

No production/test movement; visibility changes; permission/capability/HostExplicit count changes; new authority abstraction in Task 401; schema/IPC changes; behavior cleanup; ADR edits; build/test/workspace/live validation; push/tag; or automatic Task 402 start.

## Validation and closeout

- Required validation: `git diff --check` only; result is in the completion report.
- Artifact: `docs/plans/2026-09-24-task-401-effective-authority-snapshot-ownership-boundary-research.md`.
- Commit only this artifact as `docs: research effective authority snapshot ownership`.
- No push or tag.
