# Task 407 — Reassess Desktop Production Decomposition After Effective Authority Seam

## Verdict

**PASS — DESKTOP STRUCTURAL DECOMPOSITION HAS REACHED A STABLE CHECKPOINT**

The three small command modules remain useful bounded Tauri adapter seams. The
Effective Authority split is a stronger ownership boundary: root owns live
state capture, lock/currentness observations and expiry bookkeeping, while
`effective_authority.rs` owns deterministic composition of a closed snapshot.
No further extraction has comparably clear ownership and low coupling at this
checkpoint. Keep structural decomposition paused and make Task 408 a
research-only v0.32 milestone-readiness audit for the repository structure
listing work already scoped and implemented. This does not authorize release
preparation, version changes, publication, or another production move.

Task 407 is research-only. No Rust, test, permission, IPC, Cargo, dependency,
frontend, or release file was changed.

## Starting checkpoint and method

- Expected and observed `HEAD`: `94c78613f923921b492a776ce558cecd00d37b81`
  (`test: cover effective authority expiry bookkeeping`).
- Starting `git status --short`: empty.
- Current `main.rs`: **9,870 physical lines** (`Get-Content` line count).
- `main_tests.rs`: **25,394 physical lines** by the same count; it is excluded
  from the production module inventory below.
- Scope inspected: current Desktop source and command registration, Tasks
  388–406 research/implementation/audit records, Task 374 v0.32 roadmap,
  current changelog/version, and applicable architecture/security boundaries.
- No Cargo, test, Clippy, live, or workspace validation was run.

## Current production module inventory

Counts are physical source lines. “Owner kind” distinguishes a descriptive
projection from authority-adjacent observation or a module that owns state or
policy. A module's size alone is not evidence for further movement.

| File | Lines | Primary responsibility; major state/types and handlers | Sensitivity and ownership kind |
|---|---:|---|---|
| `main.rs` | 9,870 | Desktop root: `DesktopAppState`; app, repository, connection, model, profile, conversation and workflow state; Tauri handlers; cross-owner lifecycle/currentness; command registration and bootstrap. | Mixed root composition plus several authority- and persistence-owning coordinators. |
| `effective_authority.rs` | 1,203 | Closed Effective Authority DTOs/enums, Tool composition/classification, provider labels and pure snapshot derivation; `compose_effective_authority_snapshot`. | Authority-adjacent composition and descriptive projection; no live app state or lifecycle owner. |
| `desktop_preferences_commands.rs` | 17 | `desktop_preferences_warning` Tauri adapter over preference warning state. | Descriptive query; preference state/persistence owned elsewhere. |
| `desktop_status_commands.rs` | 8 | `app_status` Tauri adapter. | Descriptive projection; reads root-owned app state. |
| `desktop_model_configuration_commands.rs` | 45 | `model_configuration` query and closed model/readiness presentation. | Descriptive adapter; root owns model state, generation and readiness lifecycle. |
| `desktop_preferences.rs` | 1,458 | Serialized Desktop preferences, model/provider selection, commit identity and remembered Trusted Profile path; validation and atomic persistence. | Persistence state owner; root handlers own apply/reset/restore/forget policy. |
| `conversation_persistence.rs` | 989 | Durable transcript schema/migration, namespace operations, resume lineage, presentation and persistence warnings. | Persistence mechanism/state owner; root selects namespace and invokes lifecycle policy. |
| `provider_composition.rs` | 528 | Provider activation value/error, Tool registry merge and allowed-permission composition. | Authority-owning provider composition boundary; consumes selected profile inputs. |
| `trusted_profile_selection.rs` | 307 | Validated static profile selection, closed presentation, profile loading and selection error. | Profile data/selection type owner; root owns active selection publication, generations and remembered preference policy. |
| `repository_membership.rs` | 607 | `RepositoryMemberId`, inert members, `WorkspaceMembershipState` and membership state transitions. | Authority-adjacent membership state owner; root orchestrates admission/activation/close/removal with other live owners. |
| `remembered_workspace.rs` | 1,827 | Descriptive candidate catalog, IDs/location hints, validation, persistence store and mutation APIs. | Persistence/state owner for remembered candidates; catalog does not grant executable repository membership. |
| `host_invocation.rs` | 1,364 | HostExplicit kind/request/response types, prepared payload and ticket coordinator, expiry, bounded request validation and eligibility descriptors. | Authority-owning review-ticket and HostExplicit coordinator. |
| `git_discovery.rs` | 420 | Git executable source/selection, registry lookup and bounded path resolution. | Descriptive executable discovery; no repository authority decision. |
| `codex_baseline.rs` | 680 | Pinned Codex executable baseline resolution and verification. | Runtime prerequisite discovery/validation; no provider dispatch or authority grant. |

`main_tests.rs` is the one current test child and is not a production module.
It remains a child of root and retains direct access to root-private fixtures
and fields. There are 13 production child modules in addition to `main.rs`.

## Tasks 391–406 cumulative structural impact

| Measure | Result |
|---|---|
| `main.rs` before Task 391 sequence | 9,931 lines. |
| `main.rs` before Effective Authority seam (Task 399 checkpoint) | 9,879 lines; net reduction of 52 from the three handler extractions. |
| Current `main.rs` | 9,870 lines; net reduction of 61 from 9,931. |
| Production modules added | Three command modules: preferences, status, and model configuration. `effective_authority.rs` already existed; Task 403 created a new internal responsibility seam in it. |
| Tauri handlers relocated | Three: `desktop_preferences_warning`, `app_status`, `model_configuration`; one per command module. |
| Other production movement | Task 403 moved closed snapshot mapping/composition into the existing Effective Authority module. Diff numstat was 133 additions / 142 removals in root and 298 additions / 1 removal in the module; the root's net line reduction was 9. |
| Internal visibility changes | Three extracted handlers and `compose_effective_authority_snapshot` are `pub(super)`. No public or `pub(crate)` API was added; no state, field, DTO, or helper visibility was widened. |
| Tests added/moved | Four deterministic module-local composition tests in Task 403 and one root gathering/expiry contract test in Task 406; five added total. No tests were moved, rewritten, ignored, or removed. |
| Public API / IPC / permission / dependency changes | None. Tauri command names, parameters, registration semantics, schema, permission set, capabilities, Cargo edges and dependencies are unchanged. |
| Authority behavior | No executable authority policy or owner changed. Task 405 clarified the existing expiry-bookkeeping contract; Task 406 tests that existing behavior. |

The work materially improved ownership boundaries, rather than only moving
lines. The three handler moves made registration adapters easier to locate,
but left state and policy with their owners. The Effective Authority seam had
the greatest architectural value: it made a closed, deterministic projection
independently testable while preserving live observation, currentness and
expiry work at root. The command moves were useful but modest; the root shrank
by 52 lines. The seam's net root reduction was 9 lines and should be judged by
its ownership clarity, not that count.

## Current root responsibility map

Ranges are inclusive current `main.rs` lines and approximate physical sizes.
Supporting declarations overlap conceptually where several workflows use the
same shared state. Test coupling refers to direct `main_tests.rs` access, not
unique test counts.

| Current lines | Approx. lines | Responsibility and symbols | Cross-region dependencies; test / authority / persistence |
|---|---:|---|---|
| 1–588 | 588 | Imports, bounds/constants, process counters and test hooks, `AppStatus`, connection/chat state, `DesktopAppState`, construction and shared synchronization. | Nearly all regions consume root state. Very high sibling-test coupling; contains authority-bearing state and preference/persistence handles. Root composition is appropriate for the aggregate state type. |
| 589–1,196 | 608 | Repository persistence identity helpers/warnings plus `ConversationContextIdentity`, `DesktopConversationState`, active chat and context-change behavior. | Shared by chat execution, repository activation and conversation persistence. High state/test coupling; namespace policy and durable persistence interactions are root-owned. |
| 1,197–1,539 | 343 | `DesktopRepository`, commit identity/capability and `RepositoryWorkflowState`; index/review action data; commit authorization presentation. | Crossed by admission, observation, index and Commit flows. Authority-owning state; tests directly construct and inspect fields. |
| 1,540–1,909 | 370 | Provider endpoint input validation, model state/readiness types, Tool/chat/activity data types and error conversion helpers. | Read by model config, provider activation, Effective Authority and chat; direct test coupling. Mixed descriptive types and lifecycle state. |
| 1,910–2,218 | 309 | App/model status projections, repository Tool authority, publication currentness and provider publication. | Connects model/profile/identity/repository generations and published runtime state. Authority-adjacent currentness, directly exercised by tests. |
| 2,219–2,491 | 273 | `get_effective_authority_snapshot`, root gatherer and private closed `EffectiveAuthoritySnapshotInputs`; locking, generation comparisons, expiry cleanup and carrier creation. | Reads repository/model/connection/profile/workflow/HostInvocation. Authority-observation and bookkeeping; tests call the root path. Stable gatherer boundary after Task 403. |
| 2,492–3,305 | 814 | Current host composition, Tool definitions/activity, output classification, safe result projection and `run_host_tool`. | Relates ToolRegistry composition to HostInvocation and repository effects. Authority-adjacent/owning; host tests call multiple helpers. |
| 3,306–4,572 | 1,267 | Host read, prepare, confirmation and cancellation commands; preparation errors, tickets, freshness and dispatch. | Uses `host_invocation`, repository context, permissions and activity. Authority-owning, uncertain effects not replayed; high test/live coupling. |
| 4,573–4,825 | 253 | Trusted Profile select/forget policy, generation publication, remembered-path persistence, restore/forget/clear and five profile commands. | Profile selection feeds provider composition and currentness; persistence ordering is coupled. Query is descriptive, mutation is authority-adjacent and persistence-owning. |
| 4,826–5,147 | 322 | Model configuration setter, commit identity query/setter, model preference reset/apply and readiness endpoint probe/publication. | Model generation gates connection; identity setter revokes Commit/workflow currentness and persists through Preferences. Mixed ownership; query-only extraction remains artificial. |
| 5,148–5,439 | 292 | Git executable selection, repository observation DTOs/parsing, snapshots and review observation. | Used by review/Commit, Effective Authority-adjacent presentation and repository commands. Observation is distinct from mutation but review capture feeds authority-owning workflow. Tests share real Git fixtures. |
| 5,440–6,165 | 726 | Target observation/digest, prepared repository workflow, publish/install/refresh, revocation, repository replacement/admission/identity and active publication. | Joins Git/index state, repository membership, commit identity and runtime. Authority-owning lifecycle; concurrency tests install barriers. |
| 6,166–6,775 | 610 | Generation/activation transactions, index-effect reservations, commit revocation and `activate_admitted_member` publication. | Coordinates repository, provider/runtime, review/index effects and currentness. Authority-owning lifecycle; high race-test coupling. |
| 6,776–7,112 | 337 | Remembered Workspace DTOs, catalog/reveal/mutation handlers, parsing and admission bridge. | Candidate catalog state is separate; `admit_remembered_candidate` crosses into membership/admission. Persistence plus authority bridge; Task 390 visibility/test blockers remain. |
| 7,113–7,633 | 521 | Membership presentation and close/removal/activation/chooser commands and transitions. | Crosses inert membership, active repository, lifecycle, runtime, Commit and index state. Authority-owning root orchestration; race/live tests span it. |
| 7,634–7,950 | 317 | Commit authorization capture/currentness, reviewed Commit authorization and Stage/Unstage effects. | Joins repository snapshot, identity, workflow and index reservation. Authority-owning; tests cross lifecycle and mutation. |
| 7,951–8,670 | 720 | ToolRegistry/provider preparation, connect and disconnect lifecycle, publication and currentness. | Consumes profile/model/repository and commit generations; creates/removes executable composition. Authority-owning; high barrier and live-test coupling. |
| 8,671–9,430 | 760 | Chat state transitions, event/activity publication, uncertain-effect refresh and `run_chat`. | Uses runtime/ToolRegistry, HostInvocation exclusion, repository currentness and conversation state. Authority- and lifecycle-sensitive; tests include cancellation and integrated host flows. |
| 9,431–9,780 | 350 | `send_chat`, cancel/new/clear/resume/transcript commands and durable namespace access. | Selects/read/writes conversation persistence; `conversation_transcript` reads the selected namespace. Persistence policy is not just a query adapter. |
| 9,781–9,870 | 90 | Tauri setup, command registration, window shutdown and executable entry. | Root composition is appropriate; registration is the consumer edge for child command modules. Preserve all 47 registered commands and existing permission-generation distinction. |

## Previously rejected candidate reassessment

### Conversation transcript

- Current command family: `new_conversation` 9,601–9,625;
  `clear_conversation_history` 9,626–9,649;
  `resume_previous_conversation` 9,668–9,767;
  `conversation_transcript` 9,768–9,780. The state and namespace helpers are
  earlier in root; persistence mechanisms/presentation live in
  `conversation_persistence.rs`.
- Task 403 changed snapshot derivation and did not touch transcript,
  `DesktopConversationState`, or `select_persistence_namespace`.
- The transcript handler first chooses the durable conversation namespace
  from current repository selection and then queries persistence. That
  selection is policy-bearing. Moving only the handler would separate it from
  namespace policy; moving the family would require moving state/lifecycle and
  broad sibling-test access.
- **Disposition:** remains closed; no new evidence supports reopening.

### Commit identity

- Query and setter remain adjacent at approximately `main.rs:4,889–4,951`;
  `DesktopCommitIdentityPresentation`, `commit_identity`, and
  `set_commit_identity` are in that interval.
- Setter validates and persists identity, advances identity generation, and
  invalidates/revokes current Commit authorization/workflow. Identity is also
  captured by provider connection publication currentness and Effective
  Authority reporting.
- Tests directly seed/read identity and generation and exercise setter-driven
  invalidation. Task 403 kept generation comparisons and Commit owner
  semantics unchanged.
- Query-only separation remains artificial; extracting the pair would put
  persistence and Commit-revocation policy in a new command module without
  moving the dependent workflow owner.
- **Disposition:** do not extract on the current evidence.

### Trusted Profile selection

- Current root family: policy/persistence helpers and query/mutation handlers,
  `main.rs:4,573–4,825`; the domain module owns static selection/profile
  representation and loading, not live publication.
- Active selection and `trusted_profile_generation` are root state. Mutations
  publish/clear a selection, update the generation and save/restore/forget the
  remembered profile path. Provider composition consumes selection and its
  currentness gates published Tool composition. Queries reflect both active
  state and remembered preference.
- Task 403 carries copied profile summary/descriptors into the snapshot, but
  does not move selection or alter publication/currentness.
- A family boundary exists conceptually, but its live owner crosses provider
  composition and persistence sequencing. More source movement needs a design
  that places state, generation, policy and tests together.
- **Disposition:** root composition remains justified; no production cut is
  frozen.

### Repository membership lifecycle

- Domain state machine remains `repository_membership.rs` (607 lines). Root
  commands and transitions remain around `main.rs:5,440–7,633`, with explicit
  command family around `7,113–7,633`.
- Query, admission, activation, close and removal cross inert membership,
  host-selected admission and identity validation, active repository
  publication, lifecycle/member locks, runtime/provider composition, Commit
  capability revocation and index reservations.
- The existing module is a clearer state-owner cut, while root remains the
  application transition coordinator. A move of commands alone would split
  authority ownership; moving the entire family relocates root lifecycle
  composition and carries broad barrier/live tests.
- **Disposition:** do not extract the query or command family. The current
  state-machine/root-coordinator split is traceable and no new ownership
  evidence appeared.

### Remembered Workspace

- Root adapters and admission bridge remain at `main.rs:6,776–7,112`;
  catalog persistence/state owner is `remembered_workspace.rs`.
- Later work changed none of Task 390's private DTO/field coupling, sibling
  test access, candidate-ID parser consumption by admission, or live
  certification coupling. Task 406 touched only Effective Authority expiry
  test coverage.
- Candidate catalog is descriptive; explicit admission crosses to repository
  authority and remains root-coordinated.
- **Disposition:** Task 390 stays authoritative; do not reopen.

## Effective Authority seam after implementation

Current ownership is the designed split:

- **Root:** Tauri handler, `DesktopAppState` reads, lock and poison handling,
  generation equality comparisons, repository-context and registry/tool-count
  checks, live input capture, safe display-name derivation, expiry cleanup and
  construction of a private owned carrier.
- **`effective_authority.rs`:** deterministic closed status/availability
  derivation, connection/repository projection, HostExplicit descriptor
  presentation, unavailable external mapping, reviewed Commit projection and
  complete schema-v1 DTO assembly.

The seam is stable. The module grew to 1,203 lines, including four
module-local tests, but it remains coherent around Effective Authority DTOs,
Tool composition/classification and pure snapshot derivation. Its functions do
not own app locks, runtime lifecycle, repository membership, tickets or
expiry. Further movement is not naturally indicated by size. Moving additional
live observation into it would mix state capture with closed projection;
extracting another small mapper would over-partition one already explicit
boundary.

Task 405 clarified that gathering may reap an already-expired, already
unusable HostExplicit preparation. Task 406 tests the root gathering path and
confirms no ticket is issued and unrelated authority owners stay unchanged.
These close the contract gap without changing expiry timing or executable
authority behavior.

## Newly visible seams and serious candidates

Task 403 left a more legible seam between the root gatherer and module
composition. A fresh scan found these plausible areas:

| Candidate | Responsibility/range/symbols | State ownership and callers | Sensitivity; assessment |
|---|---|---|---|
| Trusted Profile family | `main.rs:4,573–4,825`; selection/forget helpers and five commands. | Root active selection/generation and preference sequencing; provider composition consumes published selection; direct tests and connect tests. | Authority-adjacent plus persistence-sensitive. Coherent domain name, but owner spans state, provider currentness and persistence. Needs design before movement. |
| Commit identity family | `main.rs:4,889–4,951`; query/setter and identity presentation. | Root state/generation; preferences persistence; Commit capability/workflow revocation and connection currentness; several direct tests. | Authority- and persistence-owning. Query-only move artificial; full move splits Commit owner. |
| Conversation lifecycle family | `main.rs:9,601–9,780`; new/clear/resume/transcript. | Root namespace choice and conversation state; persistence module provides durable operations/presentation; chat completion and repository switching also select namespace. | Persistence-policy owner coupled to root state; handler-only move misleading. No movement indicated. |
| Repository membership lifecycle | `main.rs:7,113–7,633`, plus admission/activation support at `5,440–6,775`; membership query, close/remove/activate/choose. | Root app transition; `repository_membership.rs` state machine; runtime, workflow, index and commit owners; broad barrier tests. | Authority-owning family exists, but moves application lifecycle across owners. Existing state-machine seam is sufficient absent a deeper design question. |
| Git observation / review | `main.rs:5,148–5,439`; snapshot parsing, staged review and target observation. | Root `DesktopRepository`, workflow and authorization consume observations; tests/live fixtures call helpers. | Observation is descriptive; review capture feeds Commit authorization. Mixed mapper could be investigated only if a specific duplicated policy appears; no clean cut established. |
| Root adapters to owner modules | Root invokes `host_invocation`, preferences, conversation persistence, remembered workspace, provider composition, and Git discovery. | Policy and lifecycles often remain at root because they coordinate multiple owners; local adapters call typed module APIs. | Some are appropriate root composition. No particular pure adapter cluster was found whose movement clarifies ownership without separating policy. |

No newly visible low-risk lifecycle family or closed DTO mapper cluster offers
more value than the completed Effective Authority seam. The candidates with
stronger domain coherence are exactly those with broader authority/persistence
coupling.

## Root-composition classification

| Classification | Current regions and evidence |
|---|---|
| **ROOT COMPOSITION IS APPROPRIATE** | Tauri bootstrap/registration; aggregate `DesktopAppState`; Effective Authority live gather wrapper; provider connect/disconnect orchestration; chat execution coordinating runtime, repository currentness, HostInvocation and persistence; command bridges into typed owner modules. These join already-defined owners and require selected app state. |
| **MODULE OWNERSHIP IS CLEAR BUT NOT YET EXTRACTED** | No production region met this category with a meaningful owner boundary, low visibility cost and stable test seam. The three small command modules are already extracted. Root Git observation contains helper clusters but they feed root-owned review/Commit workflow; no independent lifecycle owner was found. |
| **OWNERSHIP IS MIXED AND NEEDS DESIGN** | Trusted Profile live selection plus preference/generation/provider currentness; Commit identity plus Preferences and revocation; repository membership transitions plus active runtime/Commit/index withdrawal; conversation namespace selection plus persistence lifecycle. These are potential research subjects, not extraction contracts. |
| **HISTORICAL / ACCIDENTAL ROOT PLACEMENT** | No current region can be assigned on source evidence alone. The root has history and broad responsibilities, but each remaining large cluster participates in cross-owner orchestration or contains root-owned app state/policy. Line count does not prove accidental placement. |

## Authority topology

| Region | Current owner/topology | Effect of moving code |
|---|---|---|
| Repository admission | Root validates host-selected candidate, semantic identity and publishes inert member/active selection; `repository_membership.rs` owns membership state operations. | Moving a pure membership state operation may clarify its existing owner; moving admission commands alone splits host selection/currentness from publication. |
| Membership lifecycle | Membership module owns state; root owns command-level admission/activation/close/removal coordination, generation publication and links to active runtime/review/index state. | Family move would relocate one authority transaction across a file boundary; query-only extraction would not clarify ownership. |
| Repository mutation | HostExplicit prepare/confirm is rooted in bounded command paths with `host_invocation` ticket owner; Stage/Unstage and authoring tools retain their own narrow authority paths. | Splitting the root workflow by command files risks obscuring permission, currentness, ticket and uncertain-effect checks. No move recommended. |
| Review tickets | `host_invocation.rs` owns prepared payload/coordinator/expiry; root owns app context capture, command lifecycle, validation and dispatch handoff. | Existing owner and adapter boundary are traceable; moving command fragments may split prepare/confirm/cancel lifecycle. |
| Commit authorization | Root workflow/capability capture, currentness, review authorization and Commit dispatch; identity setter and index-effect lifecycle revoke/update it. | Moving identity query/setter away from authorization dependencies would make revocation less traceable. |
| HostExplicit preparation/dispatch | Fixed 11-kind policy and ticket coordinator in `host_invocation`; root command and ToolRegistry composition enforce active repository, permissions, currentness and dispatch. | The coordinator module is the authority owner; root orchestration is necessary. Do not move just descriptors/commands in a way that hides the policy chain. |
| ToolRegistry/runtime execution | `provider_composition` composes activation/permissions; root owns active-only registry building and runtime connect/disconnect/run lifecycle; ToolRegistry dispatch remains behind RAH abstractions. | Moving provider lifecycle into a presentation/composition module would split executable owner state. Keep the adapter chain explicit. |
| Trusted Profile/provider activation | Static profile representation/loading in profile module; root selection/generation/persistence; provider module composes selected inputs; root publishes runtime state. | Further movement risks merging configured intent with effective provider authority. Current boundaries preserve the distinction. |

Every listed area is authority-owning or authority-adjacent except descriptive
status/identity projections. No proposed move is needed to make the authority
owner more visible; the existing typed modules and root orchestration expose
the lifecycle. Moving a larger family now would split a single authority owner
across files unless a separate design proves a clean module owner.

## Persistence topology

| Data/policy | State or mechanism owner | Current policy/query/orchestration | Classification |
|---|---|---|---|
| Desktop Preferences | `desktop_preferences.rs` stores validated model/provider choices, commit identity, remembered profile path and warning state. | Root applies, resets, saves and restores preferences; `desktop_preferences_warning` is a query adapter. | Module is persistence state owner; root owns lifecycle policy; warning is presentation/query. |
| Conversation records | `conversation_persistence.rs` owns durable schema, operations, presentation and warnings. | Root `DesktopConversationState` and lifecycle select namespace, append/reset/resume and query transcript. | Persistence mechanism in module; namespace and lifecycle policy in root. |
| Remembered Workspace catalog | `remembered_workspace.rs` owns candidate catalog validation/store/mutation. | Root adapts requests/presentation and explicitly bridges one selected candidate into admission. | Module state owner; root presentation/query and authority admission orchestration. |
| Commit identity | Stored within Desktop Preferences. | Root query/setter coordinates persistence, identity generation and Commit revocation/currentness. | Persistence state in preference module; root policy/authority orchestration. |
| Trusted Profile remembered preference | Path stored in Desktop Preferences. | Root profile selection handlers order remembered preference writes with active selection publication/clear and generation. | Persistence state owner separate from active selection policy. |
| Restore/reset/forget | Underlying preference store is module-owned. | Separate root policies for model reset, profile restore/forget, conversation clear/resume and remembered-candidate delete. | Root orchestration; operations have distinct semantics and should not be generalized into a persistence facade. |
| Conversation namespace | No durable namespace policy is owned by persistence module. | Root maps selected repository to neutral or repository-derived namespace before reads/writes, including transcript. | Root policy owner; a transcript-only extraction would detach the durable-data selection rule. |

No current root persistence logic is a fully pure adapter whose movement would
preserve policy with its state owner and reduce coupling. Existing adapters
either apply lifecycle policy or cross authority boundaries.

## Module granularity and test topology

The three command modules remain useful names for low-coupling Tauri adapter
edges. They are small, audited and did not widen DTO/state/helper visibility.
They are not a precedent for one-handler-per-module everywhere. More files
for transcript, commit identity, Trusted Profile query or membership query
would separate commands from namespace selection, revocation, generation or
lifecycle owners. A generic “query commands” grouping would collect unrelated
domains and reduce navigability. Do not merge the existing three; no evidence
supports consolidation.

Task 400's result remains true after Task 403: **test topology is not the
primary decomposition blocker**. The Effective Authority composition has
closed owned inputs and four tests nested with the owner module; Task 406 adds
one root-path expiry contract test. This demonstrates that a pure owner seam
can get local tests without reorganizing `main_tests.rs`. For remaining broad
families, root tests still exercise real cross-owner transactions and direct
private state. Production ownership is the primary reason not to move them;
test visibility/coupling is a secondary cost. Remembered Workspace retains its
specific Task 390 visibility blockers.

## Structural options

### Option A — another bounded command extraction

**Benefit:** small reduction in root registration clutter and an obvious
adapter location. **Risk:** candidates have no low-coupling owner comparable
to the three completed commands. **Authority:** query-only Commit/Profile or
membership moves detach policy/currentness; no authority benefit. **Persistence:**
transcript/profile/identity queries participate in durable namespace or
preference policy. **Tests:** test calls remain rooted and can force sibling
visibility. **Visibility/diff:** low handler visibility cost, but weak
conceptual gain. **Follow-up:** another source/test coupling audit would still
be required. Rejected as line-count-driven.

### Option B — design one coherent lifecycle family seam

Candidate families are Trusted Profile selection, Commit identity, conversation
lifecycle or repository membership. **Benefit:** could establish a clearer
domain owner if state, policy and lifecycle can be kept together. **Risk:**
each spans existing state/persistence/runtime/authority owners; no clean module
cut is established. **Authority:** meaningful risk of splitting currentness,
revocation, membership or dispatch traceability. **Persistence:** multiple
families coordinate independent durable domains. **Tests:** broad direct state
and barrier coupling. **Visibility/diff:** high if implemented without a prior
design. **Follow-up:** choose one exact question in a separately authorized
research task. Deferred; no one family has evidence to select over the
others.

### Option C — retain current production topology and form a checkpoint

**Benefit:** preserves the new root/module split and current authority and
persistence owners while recording a deliberate stopping point. **Risk:** root
remains 9,870 lines and requires navigation by responsibility map. **Authority:**
no owner split or semantic effect. **Persistence:** preserves separate store
and root policy boundaries. **Tests:** no topology churn; existing state-level
and module-local coverage remains. **Visibility/diff:** no production
visibility or code diff. **Follow-up:** use project roadmap evidence to audit
the completed v0.32 product capability and then decide whether release
preparation is in scope. Selected.

### Option D — immediately prepare a release

**Benefit:** could advance an implemented product milestone. **Risk:** current
repository evidence has a v0.32 scope roadmap but no v0.32 milestone audit or
release gate artifact; this Task 407 scope authorizes neither release work nor
publication. **Authority:** no direct effect, but release claims need a
milestone evidence boundary. **Persistence/tests:** release-level validation
and live nonclaims need an exact checkpoint. **Visibility/diff:** unrelated to
production decomposition. **Follow-up:** first produce a readiness audit and
freeze missing gates. Rejected as the immediate Task 408 scope; it skips the
documented audit step.

## Stop assessment and roadmap implications

The current root is large, but its major regions are understandable and tie
to live application composition, cross-owner lifecycle or explicit policy.
Existing child modules already own preference storage, conversation storage,
provider composition, profile selection types, membership state, remembered
catalog state, HostInvocation tickets and executable discovery. The recent
work clarified the strongest remaining pure derivation seam. Further small
handler moves would mostly add indirection; broader moves risk dividing
authority and persistence owners. Structural decomposition should stop at this
checkpoint, subject to a future source-grounded design question.

Roadmap evidence: workspace/Cargo version remains **0.31.0**, and the latest
published release recorded in `CHANGELOG.md` is **v0.31.0**. Task 374 selected
bounded repository structure listing as the sole v0.32 primary capability.
Tasks 375–383 carried that work through contract, implementation, audit,
correction and Windows recertification. No v0.32 milestone audit or
`RAH_V0.32_RELEASE_GATE.md` exists in the current tree. Therefore a focused
Task 408 readiness audit is supported; direct release preparation is not yet
supported by a completed milestone record.

Task 408 should research-only audit the v0.32 repository-structure-listing
checkpoint against Task 374's frozen scope, Tasks 375–383 evidence, current
source, required validation and explicit live nonclaims. It should identify
whether any release gate remains open and recommend a separate next step.
Task 408 must not modify production/tests, bump versions, create a release gate,
prepare artifacts, publish, tag or push unless separately authorized. The
existing Task 382 Windows live recertification is evidence for the corrected
repo.list tree; a later source change to that capability would need fresh
checkpoint-specific consideration. Task 407 itself requires no Windows live
recertification.

## Chosen direction and exact Task 408 recommendation

**Exactly one next direction:** stop structural decomposition and audit the
v0.32 milestone checkpoint.

**Recommended Task 408:** “Research v0.32 Repository Structure Listing
Milestone Readiness.” Freeze it as docs-only research to compare Task 374's
scope/authority constraints with Tasks 375–383 source, deterministic/security
and Windows live evidence; identify unclosed gates and nonclaims; and recommend
whether a later task may prepare release materials. Do not perform
Task 408 automatically as part of Task 407.

No production boundary is frozen. No visibility budget applies. No release
preparation, version bump, tag, push, or publication is authorized here.

## Non-goals

- No Rust edits, test edits/moves, module creation/consolidation or visibility
  changes.
- No permission finding fixes or permission/capability changes.
- No authority, IPC/schema, persistence, dependency, public API or frontend
  change.
- No reopening of Task 390 or other closed candidates without new evidence.
- No release preparation, version change, publication, push or tag.
- Do not start Task 408 automatically.

## Validation and closeout

- Required validation: `git diff --check` only.
- Cargo, test, Clippy, workspace and live validation: not run; docs-only task.
- Rust/test files dirty: none expected.
- Commit only this artifact as `docs: reassess desktop decomposition after authority seam`.
- Push/tag: not authorized; none performed.
