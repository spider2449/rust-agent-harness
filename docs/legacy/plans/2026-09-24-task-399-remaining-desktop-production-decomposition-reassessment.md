# Task 399 — Remaining Desktop Production Decomposition Reassessment

**Verdict: PASS — DESKTOP DECOMPOSITION STRATEGY SHOULD CHANGE**

Recommend Task 400 research the sibling test topology and propose test-module seams before any further production move. The three audited micro-extractions are body-preserving and individually understandable, but the current candidates have stronger ownership ties, and `main_tests.rs` is now the main structural constraint on moving cohesive production ownership.

This is research only. No Rust source, tests, permissions, IPC contracts, authority behavior, persistence behavior, Cargo files, or frontend behavior changed.

## Starting checkpoint and measurement

- Expected and observed `HEAD`: `925da00f173910f84fc5a9f74ef713307c20d0f5` (`refactor: extract desktop model configuration command`).
- Starting `git status --short`: empty.
- `crates/rah-desktop/src/main.rs`: **9,879 physical lines**, counted from the current text. The trailing newline is not counted as an extra physical line.
- `crates/rah-desktop/src/main_tests.rs`: **25,271 physical lines**.
- The extraction sequence started with 9,931 lines before Task 391. The current count is 52 fewer lines.
- Scope inspected: current root and child modules, Tauri registration, `main_tests.rs`, Tasks 388–398 research/audit artifacts, architecture/security documentation, and the relevant accepted authority and persistence boundaries. No Cargo command or test was run.

## Current production module inventory

The root is `main.rs`; it declares 13 Windows-gated production child modules. `main_tests.rs` is its test child.

| Module | Approx. physical lines | Current responsibility |
|---|---:|---|
| `codex_baseline.rs` | 681 | Resolve and validate the configured Codex executable baseline. |
| `conversation_persistence.rs` | 990 | Durable conversation records, namespace selection, transcript presentation, resume lineage, and persistence warnings. |
| `desktop_model_configuration_commands.rs` | 46 | One Tauri model-configuration query adapter. |
| `desktop_preferences.rs` | 1,459 | Stored Desktop preferences and preference persistence policy. |
| `desktop_preferences_commands.rs` | 18 | One Tauri preferences-warning query adapter. |
| `desktop_status_commands.rs` | 9 | One Tauri application-status query adapter. |
| `effective_authority.rs` | 907 | Authority snapshot and presentation data model. |
| `git_discovery.rs` | 421 | Host-side Git executable and repository discovery. |
| `host_invocation.rs` | 1,365 | Host invocation coordination, request/review structures, and bounded host command data. |
| `provider_composition.rs` | 529 | Provider activation and allowed tool-registry composition. |
| `remembered_workspace.rs` | 1,828 | Descriptive remembered candidate catalog, IDs, persistence, mutation, and lookup. |
| `repository_membership.rs` | 608 | Inert repository-member identity and membership state transitions. |
| `trusted_profile_selection.rs` | 308 | Static provider-only Trusted Profile loading and presentation selection. |

Line counts are inventory measures only; they do not establish an extraction boundary. Several modules are existing domain owners and are not candidates for movement in this task.

## Current `main.rs` responsibility map

Ranges are inclusive, refer to the verified current source, and can contain adjacent supporting definitions used across regions. Approximate counts are physical source lines.

| Responsibility and range | Main contents and ownership | Coupling and sensitivity |
|---|---|---|
| Root wiring and shared definitions, 1–586 (~586 lines) | Module imports, constants, test counters/hooks, connection/chat state, `DesktopAppState`, construction, and process-level synchronization state. | Nearly every region shares the root-owned state. High test coupling because tests construct `DesktopAppState` and inspect its private fields. Authority-owning state is present. |
| Persistence binding and conversation context, 587–1,194 (~608) | Repository persistence key/fingerprint, warning emission, conversation context identity/change, and conversation lifecycle state. | Persistence namespace is derived from the host-selected repository. Shared by repository publication, chat completion, clear/resume/transcript paths, and test fixtures. Persistent-data and lifecycle sensitive. |
| Chat/runtime presentation data and currentness helpers, 1,195–2,218 (~1,024) | Repository and commit identity data, model selection/readiness, activity/presentation DTOs, status and connection publication helpers. | Crosses model/provider generations, repository currentness, commit identity generation, and presentation. Tests use private types and fields directly. |
| Effective authority snapshot, 2,219–2,500 (~282) | `get_effective_authority_snapshot` and the ~275-line state-to-snapshot calculation. | Descriptive endpoint backed by authority-owning state. Reads repository, model, profile, runtime, workflow, and generations. Directly called by tests and ignored/live harnesses; high authority sensitivity and test coupling. |
| Host composition, classification, and dispatch support, 2,501–3,314 (~814) | Current host composition, tool definitions, activity preparation, output classification, and `run_host_tool`. | Owns the connection between policy-approved tools and host activity, but relies on the accepted ToolRegistry/HostInvocation boundaries. Helpers are shared by multiple command paths and tests. Authority-adjacent/owning. |
| Host read, preparation, confirmation, and cancellation commands, 3,315–4,581 (~1,267) | Read-only host invoke and bounded prepare/confirm/cancel handlers, review ticket checks, and dispatch authorization. | Review-ticket lifecycle, explicit host permissions, filesystem mutation boundary, currentness, and uncertain external effects. Large helper and test coupling; not a file-size extraction candidate. |
| Trusted Profile policy and commands, 4,582–4,834 (~253) | Selection/forget lifecycle checks, generation publication, durable remembered path save/restore/forget, query and mutation handlers. | Query reads both active profile and remembered preference. Mutations coordinate lifecycle and preference ordering; selection feeds provider composition. Authority-adjacent with persistence coupling. |
| Model/preferences/commit identity and readiness commands, 4,835–5,156 (~322) | Model configuration setter/reset, commit identity query/setter, preference reset, endpoint readiness probe and publication. The model-configuration query itself is in its extracted module. | Desired model state, provider readiness, preference writes, commit identity generation, and commit authorization revocation. Multiple distinct policies share synchronization and tests. |
| Repository observation and staged review, 5,157–5,550 (~394) | Git selection, repository snapshot/status/diff presentation, target observation, staged-review digest and parsing. | Descriptive query results feed commit review/currentness. Git observation is bounded and host-selected. Shared with tests and later mutation workflows. |
| Repository workflow, admission, activation, and index-effect coordination, 5,551–6,775 (~1,225) | Prepared workflow/review state, commit context revocation, repository admission and identity checks, activation barriers and publication, Stage/Unstage effect reservations. | Authority-owning lifecycle and mutation coordination. Shared membership/lifecycle locks, provider/currentness, review state, and test barriers make a physical move especially risky. |
| Remembered Workspace Desktop adapters and admission bridge, 6,776–7,111 (~336) | Candidate DTOs, catalog/reveal/mutation handlers, parsing and presentation helpers, plus admission bridge into repository membership. | Remembered catalog is descriptive and persisted separately from executable membership. The candidate-ID parser and admission response cross into membership. Seven prior direct test references (three live/certification paths), including direct private DTO/field access; Task 390 rejected the group. |
| Repository membership presentation and lifecycle commands, 7,112–7,642 (~531) | Membership DTOs, close transition, query, inactive removal, activation, and chooser/admission command handoffs. | A coherent membership lifecycle family exists, but it owns admission/activation/removal/close interactions with active repository state and runtime lifecycle. Tests exercise generation, concurrency, stale intent, active-only composition, and ignored live behavior. Authority-owning. |
| Commit review authorization and index actions, 7,643–7,959 (~317) | Authorization capture/currentness, commit review authorization, Stage/Unstage selection and dispatch. | Directly owns reviewed-Commit authorization and index mutation dispatch. Must remain with its policy and currentness evidence; tests are strongly coupled. |
| Runtime/provider connection lifecycle, 7,960–8,728 (~769) | ToolRegistry construction, provider composition, connect preparation/publication, connect/disconnect and lifecycle transitions. | Owns execution composition and runtime lifecycle; depends on repository, model, Trusted Profile and commit generations. Many tests use barriers and directly inspect state. High authority sensitivity. |
| Chat activity and runtime turn execution, 8,729–9,552 (~824) | Chat event/activity publication, uncertainty refresh, execution loop and send/cancel commands. | Runtime, ToolRegistry, host invocation exclusion, repository currentness and conversation persistence interact. Ignored live harnesses exercise these flows. Authority and lifecycle sensitive. |
| Conversation commands and durable presentation, 9,553–9,788 (~236) | New conversation, clear history, resume previous conversation, transcript query, and resume/persistence error mapping. | These commands own user-visible restore/reset/read decisions and repeatedly select the namespace. The query cannot be understood independently of namespace policy. Persistence and lifecycle sensitive. |
| Tauri bootstrap and test child declaration, 9,789–9,879 (~91) | Managed state setup, 47 command registrations, close-window shutdown, executable entry point, and `main_tests` inclusion. | Registration path is the parent-side consumer for child commands. No IPC command names or registration order are proposed to change. |

## Structural effect of Tasks 391–398

The checkpointed source and the three implementation/audit pairs establish:

| Measure | Evidence/result |
|---|---|
| `main.rs` before Task 391 | 9,931 lines, recorded by Task 391 at its clean starting checkpoint. |
| `main.rs` now | 9,879 lines. |
| Net root reduction | **52 lines**. Commit diffs report `15 deletions/3 insertions` for Task 391, `7/3` for Task 394, and `39/3` for Task 397, each net including module declaration and registration replacement. |
| Production modules added | **3**: preferences commands, status commands, model configuration commands. |
| Handlers moved | **3**, one per module: `desktop_preferences_warning`, `app_status`, `model_configuration`. |
| Visibility changes | **3 handlers to `pub(super)`**; no DTO, field, helper, constant, or state visibility changes. |
| Test files moved/changed | **0**. All existing tests stayed in `main_tests.rs`. |
| Authority, IPC, permission, dependency changes | **None**. Existing handler names, payloads, registration order, permissions, Cargo dependencies, and authority owners were preserved. |

These moves created useful named seams at three low-coupling Tauri adapter edges and removed local root clutter. They did not move ownership of status, preferences, model configuration state, or policy; those remain root-owned. The benefit is real but bounded: the root fell by 52 lines, while the new modules are 9, 18, and 46 lines, and 9,879 production lines remain. Continuing the pattern would produce more command-named leaf files without necessarily clarifying state ownership. A few superficially small handlers remain, but they no longer match the earlier low-coupling template as cleanly.

## Reassessment of known candidates

### `conversation_transcript`

- **Current range:** `main.rs:9776–9786` inclusive (~11 lines including attributes); registration at 9,826.
- **Production dependencies:** `DesktopAppState::select_persistence_namespace`, `DesktopAppState.persistence`, and `Persistence::presentation`; imported response alias `ConversationTranscriptPresentation`.
- **Namespace and policy owner:** `select_persistence_namespace` is root-owned on `DesktopAppState` and derives a namespace from the currently selected repository (`main.rs:484–504`). This is the host's durable-data ownership decision, not a mere formatting helper.
- **Tests:** No direct handler call or presentation-alias construction found. Persistence/state behavior is covered through state-level fixtures and APIs, including ordinary reset/restore/preference tests and ignored live remembered-workspace and repository-close scenarios. The one-handler move could need just `pub(super)`, but a sibling test module still exercises the surrounding state lifecycle.
- **Assessment:** Moving only this handler is mechanically possible but would place the read endpoint away from the command's namespace-selection policy. It would clarify the Tauri adapter location slightly while obscuring the durable namespace decision. Do not select it as another micro-extraction.

### `commit_identity`

- **Current DTO and handler:** DTO `main.rs:4893–4897`; query handler `4,899–4,909` (10 lines); setter begins at 4,911. Query registration is at 9,812.
- **Responsibility:** Descriptive `configured` boolean from `DesktopAppState.commit_identity`; no helper or request DTO.
- **Tests:** No direct query-handler or response DTO references found. `DesktopAppState.commit_identity` and its generation are accessed by commit authorization, connection currentness, mutation invalidation and ignored/live harness setup. Seven `set_commit_identity` calls appear in tests, in addition to many direct field/generation references.
- **Adjacent owner:** `set_commit_identity` validates and persists the identity, advances its generation and withdraws commit capability/workflow. Commit authorization consumes that state immediately downstream.
- **Assessment:** Low implementation complexity does not make a clean ownership seam. Moving only the query would separate the description from the setter/currentness policy while still depending on broad shared state. Keep the query beside commit-identity ownership.

### `trusted_profile_selection`

- **Current range:** `main.rs:4751–4770` inclusive (~20 lines); five related handlers span 4,751–4,834. Registration at 9,800.
- **Role:** Describes active Trusted Profile selection and whether a preference is remembered.
- **Ownership:** Reads `preferences.remembered_trusted_profile_path()` and `trusted_profile`; the profile selection is host-owned static intent, with a generation consumed in provider activation/publication currentness. The surrounding choose/restore/forget/clear paths enforce chat/connection constraints, write or retain preference state, and publish/clear selection.
- **Tests and visibility:** Selection helpers and profile state are directly used by profile and lifecycle tests. The DTO is owned in `trusted_profile_selection.rs`, but root-side active profile/generation, preference ordering, save/restore helpers and handlers remain coupled. Parent registration alone would require one `pub(super)` handler; extracting only it would not move a meaningful ownership unit.
- **Assessment:** Query-only movement would split presentation from active selection plus remembered preference state. Keep it with Trusted Profile composition policy.

### `repository_membership`

- **Current handler:** `main.rs:7402–7409` (~8 lines); presentation converters at 7,208–7,242. Related member lifecycle commands and types span 7,112–7,642. Registration at 9,809.
- **Role and family:** Query the current admitted member list, active member, and membership generation. A real family now sits nearby: membership query, close, remove, activate, chooser/admission handoff, and related presentation types.
- **Tests:** `repository_membership_presentation` has 53 test-source references, including 18 ignored tests overall in `main_tests.rs` and multiple live certification scenarios. Tests exercise private presentation helpers, state transitions, stale selectors, concurrency, activation/close races, admission, and active-only composition. They are not query-only tests.
- **State/authority:** The family is coordinated with membership and lifecycle mutexes, admission validation, runtime/provider state, currentness, commit capability withdrawal, and index effects. `repository_membership.rs` already owns the lower-level membership state machine; root owns application composition and command lifecycle orchestration.
- **Assessment:** A coherent family exists conceptually, but the command region is authority-owning and couples to many other regions. Extracting only the query is weak; moving the family would relocate application lifecycle ownership and incur substantial test coupling. It is not a safe near-term production move.

### Effective authority snapshot

- **Current references:** Handler `main.rs:2217–2223`; calculation helper `2,225–2,499` (~275 lines); 47-handler registration at 9,858.
- **Tests:** 19 direct source references to `get_effective_authority_snapshot` in `main_tests.rs`, including ignored/live flows, plus tests of the calculation and snapshot DTO. References cross host prepare/confirm, provider publication, repository observation/review, mutation and multi-repository certification.
- **Ownership and visibility:** Handler is read-only, but snapshot computation reads and classifies profile, model, connection, repository membership/currentness, host invocation and review/commit state. Test calls to the handler from the sibling module mean a move entails more than registration visibility if tests remain unchanged; moving its helper would move a large authority-sensitive computation.
- **Assessment:** High-risk, authority-descriptive endpoint whose output is derived from authority owners. It is not a mechanical command extraction and remains rejected.

### Remembered Workspace

- **Current location:** Root-owned remembered DTOs and six catalog/reveal mutation commands at `main.rs:6776–7,053`; admission bridge continues through 7,111. Lower-level state/persistence lives in `remembered_workspace.rs`.
- **What changed since Task 390:** The root shifted because Tasks 391–397 removed three unrelated handlers. No test was relocated, no DTO ownership changed, no private field was widened, no helper/test coupling was removed, and the admission bridge still consumes the candidate-ID parser and membership presentation.
- **Evidence:** Task 390 found seven direct test functions coupled to proposed symbols, including three live/certification scenarios; several use private DTO fields/types. Current searches still find the same presentation/helper references in tests and live harnesses. No genuinely new decoupling evidence exists.
- **Assessment:** Remains closed. Do not repeat Task 389's boundary or propose it as a Task 400 production extraction.

## Coherent command-family analysis

| Candidate family | Handlers and range | Types/helpers/state/tests | Boundary assessment |
|---|---|---|---|
| Conversation lifecycle and transcript | `new_conversation` 9,610–9,634; `clear_conversation_history` 9,635–9,658; `resume_previous_conversation` 9,677–9,776; `conversation_transcript` 9,777–9,786. | Shares `DesktopConversationState`, `Persistence`, namespace selection, resume validation/errors and transcript presentation. Test fixtures cross conversation persistence with preferences and repository switches; ignored live scenarios inspect persistence during close and remembered-workspace workflows. No DTO is defined in this small range; helpers and state live earlier or in `conversation_persistence.rs`. | Responsibility is real, but moving handlers alone would detach policy and lifecycle from their owners. Moving state/helpers raises privacy/test-fixture coupling. Persistence-sensitive; not selected. |
| Repository membership lifecycle | `close_repository` 7,393–7,401; `repository_membership` 7,402–7,410; removal 7,554–7,562; activation 7,582–7,590; chooser/admission handoff 7,592–7,642, plus supporting family beginning at 7,112. | Shared membership DTOs and conversion, admission/activation/removal/close helpers, `WorkspaceMembershipState`, lifecycle and membership coordination, repository/runtime/commit/index state. Extensive direct helper/state access and ignored tests. | Conceptually coherent but authority-owning and broad. A move would relocate desktop application lifecycle coordination and bring large cross-domain test coupling. Reject for now. |
| Trusted Profile commands | Query/choose/restore/forget/clear, 4,751–4,834. | Shared selection/presentation type in child module, preferences, active selection/generation, model selection, connection/chat guards, provider composition and persistent preference helpers. Profile tests plus connect/publication tests. | True domain family, but it owns active profile intent, persistence and provider lifecycle currentness. It is not a low-risk Tauri command extraction. |
| Commit identity query/setter | Query 4,899–4,909 and setter 4,911 onward (model/reset commands interleave in the same region). | Shared identity state/generation, Preferences, revocation helpers, workflow and Commit authorization. Many tests and ignored setup paths. | More coherent than query-only if grouped with its setter, but the setter is an authority-revocation transaction. Not selected as a line-count-driven module. |
| Existing three extracted query adapters | `desktop_preferences_warning`, `app_status`, `model_configuration`, each already out of root. | Their state/policy/DTO owners remain in `main.rs`; all registration stays in the root. | Independently audited seams, but they do not form a shared domain family. Do not consolidate them in this task. |

The family review finds no small 2–5-handler family that combines shared ownership, low authority/persistence sensitivity, minimal state coupling, and low test coupling. Physical adjacency is not enough to establish one.

## One-handler-per-module reassessment

- The three modules each name an independently audited Tauri adapter responsibility. `desktop_preferences_commands.rs` is an event-like preferences warning read; `desktop_status_commands.rs` is a status delegate; `desktop_model_configuration_commands.rs` builds model/readiness presentation. They are coherent as bounded registration seams but are not full ownership units for their backing domains.
- Adding more one-handler modules for transcript, commit identity, Trusted Profile selection or membership would fragment commands from their policy and state owners. The earlier low-risk pattern is no longer plentiful among the known candidates.
- A grouped “presentation/query commands” module could technically contain several registered queries with parent-private types/helpers, but its only unifying property would be command shape. It would mix preferences warnings, application status, model readiness, commit identity and persisted transcript reads. That is not stronger ownership than the current names.
- Grouping the current three now would change module paths and registration paths without moving the state/DTO owners. It would erase three useful responsibility names and require edits to modules explicitly held fixed by this task. No structural evidence supports consolidation.

Conclusion: stop treating one handler per module as the default. Preserve the three audited leaves, and require a demonstrated owner boundary and test plan before future production movement.

## Test topology

`main_tests.rs` is 25,271 lines, compared with 9,879 in the production root. It has 133 `#[test]`/`#[tokio::test]` attributes by source search and 18 explicitly ignored host/live scenarios. The prior package audit reported 315 passed and 18 ignored (333 discovered); that result is historical validation evidence, not a test run in Task 399.

- The test child imports root-private items and directly constructs or inspects root-private state. Current `DesktopAppState::new`/literal references occur 121 times; references to the state fields named across authority, persistence, model, connection, profile, and chat occur broadly (499 matching source lines for the inspected field set).
- Production regions with direct sibling-test access to root-private fields/types include at least: application/runtime state; preferences and model state; Trusted Profile state; conversation/persistence; repository membership and active repository; commit identity/capability/workflow; host invocation/review; provider/runtime connection; and test synchronization hooks. A moved sibling production module cannot inherit `crate::tests` access to those private members.
- Handler and helper coupling is also cross-region: 19 direct snapshot-handler references; 53 membership-presentation references; seven `set_commit_identity` references; and direct calls to profile publication/restore helpers. These counts are references, not unique tests, so they should not be added as if disjoint test totals.
- Fixtures such as `TestRepository`, `LinkedWorktreeFixture`, activation/authorization/connect/index barriers, live Git helpers, host-event evidence and temporary persistence roots support multiple test families. The ignored scenarios include host invocation, repository mutation, remembered workspace, membership removal, close, repository search/list and linked worktree certification.
- Several broad live tests assert multiple outcomes across production responsibilities. Moving only the matching ordinary unit tests would leave sibling-private access in the live harness; moving a whole live scenario to a new child would also move its shared fixtures or require a root test-support layer.

Test coupling now materially constrains ownership moves. Task 400 should research test organization first; this does not authorize moving tests or creating a test-support API.

## Authority topology

| Area | Current owner/source | Classification |
|---|---|---|
| Repository admission | Host-selected chooser and remembered-candidate admission in root; semantic identity validation and inert-member state in `repository_membership.rs` and admission helpers. | **Authority-owning boundary:** only host-selected roots become admitted members; admission and activation are separate. |
| Repository membership | `repository_membership.rs` owns inert membership state; root owns admission/activation/close/remove commands and composition publication. | **Authority-owning lifecycle:** membership state is not itself executable authority, but root transitions coordinate active repository and runtime authority. |
| Repository mutation | Host invocation preparation/confirmation and ToolRegistry dispatch; Stage/Unstage reservation/action path; repository authoring tools. | **Authority-owning:** distinct bounded host mutation and repository index authorities remain separated. |
| Review tickets | `host_invocation.rs` coordinator and root prepare/confirm/cancel handlers; workflow/review selectors in root. | **Authority-owning:** confirmation dispatches only a currently valid host-prepared operation. |
| Commit authorization | Root commit capability, review capture/currentness, authorization and commit dispatch helpers around 1,291–1,404 and 7,643–7,959. | **Authority-owning:** reviewed Commit binds identity, repository/index/HEAD state and revocation. |
| Effective authority | `effective_authority.rs` owns snapshot types; root computes the live snapshot from current state. | Handler is **descriptive**, but its calculation is **authority-adjacent** and classifies all major authority owners. |
| Trusted Profile composition | `trusted_profile_selection.rs` owns static profile type/loading; root owns selected profile, generation, preference and publication into provider composition. | Query is **descriptive**; selection/publish and later provider composition are **authority-adjacent/owning**. |
| ToolRegistry/runtime execution | `provider_composition.rs` owns provider activation composition; root builds active-only ToolRegistry and connect/disconnect/run paths. | **Authority-owning:** active composition gates model/runtime tool execution and host authority. |
| Application/model status | Root `AppStatus` and model/readiness mapping; extracted command adapters call/read it. | **Descriptive only**, though status reflects authority state and must not become its owner. |

No future file move should separate authority calculation or lifecycle coordination from the owner simply because its Tauri entry is small.

## Persistence topology

- **Stored Desktop Preferences:** `desktop_preferences.rs` owns serialized model/provider selection, commit identity, remembered Trusted Profile path, warning state and transactional persistence. Root command policy selects when to apply, reset, save, restore, or forget these values.
- **Conversation persistence:** `conversation_persistence.rs` owns durable transcript records, presentation, resume lineage, namespace-specific operations and warning mapping. Root owns the selected `DesktopConversationState`, command policy, and when writes/read/reset/resume occur.
- **Remembered workspace state:** `remembered_workspace.rs` owns a separate descriptive candidate catalog and its storage/mutation APIs. It is not repository membership and does not restore executable authority.
- **Persistence namespace:** root `DesktopAppState::persistence_namespace` and `select_persistence_namespace` choose `neutral-v1` or a root-derived repository key. Repository publication, completed chat pairs, separators, clear/resume and transcript query select it. Namespace choice determines which durable conversation data is visible.
- **Restore/reset/forget:** preferences and Trusted Profile handlers have distinct restore/reset/forget policies; conversation resume explicitly appends/commits a resume lineage, clear removes the selected namespace history, and new conversation separates in-memory context. These actions are not equivalent to a descriptive read.
- **Descriptive reads:** `desktop_preferences_warning`, status/model queries, `commit_identity`, and `conversation_transcript` are read-shaped. The transcript query still performs namespace selection before reading, so its storage target is policy-bearing.
- **Write/lifecycle ownership:** preferences and conversation writes are invoked from root lifecycle and command paths; remembered candidate writes remain in its domain module. No clean generic persistence command façade is evidenced.

## Structural options

### Option A — continue isolated command extraction

Candidate handlers include transcript, commit identity, Trusted Profile selection, and other remaining query-shaped commands. Each could be made parent-visible with one `pub(super)` declaration. Their present ownership differs materially:

- transcript selects durable namespace;
- commit identity reports a direct prerequisite of Commit authorization beside its persistence/revocation setter;
- Trusted Profile reads remembered preference and active selection beside publication/composition lifecycle;
- effective authority is explicitly rejected for large calculation/test coupling.

Likely benefit is further root line reduction and a local registration seam. Visibility cost appears low, but conceptual ownership clarity is weak and sibling tests remain rooted. **Rejected** as the default next step.

### Option B — extract a coherent multi-handler family

Plausible families are the four conversation lifecycle/read handlers, the Trusted Profile family, and repository membership lifecycle. The first is persistence-policy coupled; the second owns profile preference and provider currentness; the third is a broad authority-owning transaction family with extensive test and barrier coupling. A “query commands” grouping of current/new adapters lacks a domain owner. **Rejected** until a family can move with policy/state ownership and a concrete test seam.

### Option C — research test decomposition before production movement

Study the current 25,271-line sibling test module by responsibility and fixture dependency. Determine which deterministic test families can be nested under their production owner without moving shared fixtures or live certification, and whether live harnesses need a separate topology. Record field/helper/handler references, visibility implications and a non-mutating partition proposal. **Selected** because test access currently crosses every plausible remaining owner and is the main blocker to meaningful, private production boundaries.

## Chosen next direction and rejected directions

**Exactly one recommendation:** Task 400 should research whether `main_tests.rs` must be decomposed by production responsibility before any further `main.rs` production extraction.

Rejected alternatives:

- another isolated query handler: low registration visibility cost but separates query from persistence, profile or commit-state policy;
- a conversation command module: handlers without namespace and lifecycle ownership split durable-data policy;
- a repository membership command module: relocates application authority transitions and tests without a demonstrated containment boundary;
- an effective authority module move: high authority sensitivity, large state snapshot helper and live-test coupling;
- a repeated Remembered Workspace extraction: Task 390 blockers remain unchanged;
- consolidating the three existing leaves: no shared domain ownership and no benefit that warrants undoing audited boundaries.

## Exact Task 400 scope

**Research question:** Determine whether sibling test topology must be decomposed before further `rah-desktop` production movement, and identify one evidence-backed test-module organization proposal if it can be done without duplicating shared fixtures or broadening production visibility.

Task 400 should inventory ordinary tests, ignored/live certification functions, shared fixtures/barriers, direct root-private field/type/helper/handler access, and cross-responsibility assertions. It may recommend keeping a test family in the root when its fixture/use spans domains. It must not move tests or production code, change visibility, add test-support production APIs, reorganize extracted modules, or authorize a production extraction. A later task must separately select and freeze any implementation boundary.

## Non-goals and impact

- No Rust production/test edits, extraction, module consolidation, test relocation, or visibility changes.
- No authority, Trusted Profile, permission, IPC, persistence, API, dependency, or frontend redesign/fix.
- No Task 400 implementation is authorized by this recommendation.
- No push or tag.

## Validation and final repository state

- `git diff --check`: **PASS** after creating this artifact.
- Cargo formatting/check/test/clippy/workspace validation and Windows live certification: **not run**, as required for docs-only research.
- Rust files dirty: **none**.
- Commit: recorded in the Task 399 completion report.
- Final `git status --short`: **clean**.
- Push/tag: **none**.

**Recommended Task 400:** Research whether sibling test topology must be decomposed before any further Desktop production extraction. Do not start automatically.
