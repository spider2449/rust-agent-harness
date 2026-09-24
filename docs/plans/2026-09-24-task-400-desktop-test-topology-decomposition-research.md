# Task 400 — Desktop Test Topology Decomposition Research

**Verdict: PASS — TEST TOPOLOGY IS NOT THE PRIMARY DECOMPOSITION BLOCKER**

Test placement creates real visibility costs for a few bounded adapter groups, especially Remembered Workspace. Across the remaining large production regions, however, the tests mirror actual production ownership crossings: shared application state, lifecycle transactions, currentness barriers, durable namespace selection, and authority composition. Moving test files alone would not untangle those edges. Do not begin a broad test rewrite before a production boundary is established.

## 1. Starting checkpoint and scope

- Expected and observed `HEAD`: `0e623adf26379b83b286bdb980810b621c4502a4` (`docs: reassess remaining desktop production decomposition`).
- Starting `git status --short`: empty.
- `crates/rah-desktop/src/main.rs`: 9,879 lines, confirmed from the checked-out file.
- `crates/rah-desktop/src/main_tests.rs`: 25,270 lines by `rg` physical line numbering (`ReadAllLines` count); final numbered source line is 25,270.
- Fresh source search found **231** `#[test]`/`#[tokio::test]` attributes in `main_tests.rs`; 17 carry `#[ignore]`.
- There is one further ignored host probe in `crates/rah-desktop/src/git_discovery.rs`, so the package has **18 ignored tests** across those source files. The 17 in `main_tests.rs` comprise 16 Windows live/certification scenarios and the Task 126 host runtime probe.
- Existing production modules retained unchanged: `desktop_preferences_commands.rs`, `desktop_status_commands.rs`, and `desktop_model_configuration_commands.rs`.
- No Rust source, test, visibility, Cargo, dependency, architecture, authority, or certification code was changed. No Cargo command or live certification was run.

The Task 399 artifact records 25,271 lines and 133 test attributes. At this exact checkpoint, a fresh source scan gives 25,270 numbered lines and 231 attributes. The one-line difference is a counting convention around the terminal newline; the attribute discrepancy is not. All family counts below use the current source scan, not the historical 133 count. The expected `main.rs` size does match.

## 2. Test-family map

Ranges are inclusive `main_tests.rs` line numbers. Counts are source attributes in each range, not executed/discovered/passed counts. Approximate line count is `end - start + 1`; regions can mix assertions that exercise more than one responsibility.

| Family / range | Approx. lines | Test attrs | Ignored | Main responsibilities and notable production symbols |
|---|---:|---:|---:|---|
| Shared test scaffolding and connect preparation, 1–910 | 910 | 4 | 0 | `ReadinessTestServer`, `TestRepository`, `LinkedWorktreeFixture`, tool registries, ticket/commit setup; `prepare_codex_connection`, `resolve_codex_executable`, `DesktopAppState` types. Most helpers are used later. |
| HostExplicit live scenarios and repository observation/review, 911–3,152 | 2,242 | 14 | 2 | Two ignored create/delete scenarios, then snapshot, staged review, target observation, observer errors, Git environment and review-selector tests; calls snapshot and HostInvocation paths. |
| Chat/runtime cancellation, application status and Task 126 probe, 3,153–3,725 | 573 | 12 | 1 | Terminal ownership, cancellation recovery, status/error presentation, runtime identity; one ignored pinned Codex `runtime.start` probe. |
| HostExplicit/live mutation scenarios, 3,726–7,143 | 3,418 | 7 | 7 | Rename, patch, multi-file edit and branch creation live flows; direct prepare/confirm/cancel handler calls, host state and filesystem/Git evidence. |
| Provider/currentness and membership admission, 7,144–8,917 | 1,774 | 12 | 1 | Codex presentation, repository status, generation tuples, membership admission/activation, linked worktrees and one ignored inactive-member certification. |
| Activation/authorization/index support, 8,918–9,603 | 686 | 3 | 0 | Barrier installers/releases, real index reservation helpers, fake executable and fixture builders; mostly support for later tests. |
| Repository close, membership lifecycle and activation races, 9,604–11,686 | 2,083 | 32 | 0 | Close/remove/activate, generation and publication races, Stage/Unstage exclusion, host-confirm and commit-review invalidation. Tests directly call transition helpers and mutate test hooks/state. |
| Chat contracts, tool composition and host preparation/dispatch, 11,687–13,119 | 1,433 | 21 | 0 | Prompt/events, authority classes, Host ToolRegistry composition, patch/create/delete/rename preparation, classification and dispatch helpers. |
| Delete/rename/create mutation and review invalidation, 13,120–14,792 | 1,673 | 14 | 0 | Real preparation paths, ticket/currentness behavior, uncertain effects, repository refresh and commit-review revocation. |
| Commit/activity/authority and external effects, 14,793–15,821 | 1,029 | 24 | 0 | Commit control, branch and directory authority, activity publication, Stage/Unstage and external tool-effect lifecycle. Many tests cross host tools, repository observation and commit authorization. |
| Model configuration/provider readiness, 15,822–16,379 | 558 | 20 | 0 | `DesktopModelState`, `ProviderEndpoint`, readiness probe/publication, model selection and generations; `ReadinessTestServer` is shared from the file prefix. |
| Conversation state and durable replay contracts, 16,380–16,731 | 352 | 9 | 0 | In-memory conversation pair completion, resume lineage, replay limits, context changes, clear/new commands and closed frontend result. Exercises root `DesktopConversationState` plus persistence APIs. |
| Preferences, Trusted Profile and Remembered Workspace, 16,732–18,167 | 1,436 | 32 | 0 | Preference restore/apply/reset, profile remembered path/selection/forget, remembered candidate catalog/admission/reveal/update; several tests share root app state and filesystem fixtures. |
| Task 324 observer and host evidence, 18,168–20,001 | 1,834 | 12 | 2 | Event observers, managed state cleanup, Windows process identity, app-server ownership, two-repository host-driven certification. |
| Remembered Workspace Windows live certification, 20,002–20,674 | 673 | 1 | 1 | `task_335_windows_host_driven_remembered_workspace_live_certification`; joins catalog presentation, repository admission, app runtime, frontend events and filesystem evidence. |
| Active Repository Close Windows live certification, 20,675–22,567 | 1,893 | 1 | 1 | `task_352_windows_active_repository_close_live_certification`; close, persistence, membership and host evidence. |
| Linked-worktree live fixture/scenario support, 22,568–24,067 | 1,500 | 0 | 0 | `Task361LiveFixture`, repository capture and scenario helpers; consumed by the test below, with many layout/admission/identity cases. |
| Repository search/list and linked-worktree certifications, 24,068–25,270 | 1,203 | 3 | 3 | Ignored search, list and linked-worktree host scenarios; list/search surfaces, link/reparse behavior, admission and active composition. |

The numbered attribute total is 231. Range responsibilities are descriptive; notably the first 3,152 lines mix reusable fixture infrastructure with ordinary repository-review tests, while the last 7,103 lines contain multiple purpose-built Windows evidence harnesses.

### Minimum family distinctions required by the task

- **Status/presentation:** 3,358–3,416 and 7,144–7,217; app status, adapter errors, Codex presentation, repository status/error sanitization. Adjacent currentness tests in 7,218–7,327 are already cross-domain.
- **Model configuration/provider readiness:** 15,822–16,379 (20 tests): endpoint normalization and closed authority, readiness network behavior, stale-generation discard, model choices and presentation.
- **Desktop Preferences:** 16,732–16,876 and 17,602–18,167 (roughly 12 tests): preference restore/apply/reset and persistence/error matrices. Several deliberately assert separation from conversation persistence.
- **Trusted Profile:** 16,661–17,200 interleaved with preference and remembered tests (roughly 5 tests): remembered profile path, selection generation, clear/restore/forget, provider-only selection. Profile composition tests also appear in connection/currentness ranges.
- **Repository observation:** 2,181–3,152 plus portions of 7,180–7,217 and tool activity ranges. Snapshot, status, diff/review digest, target currentness, observer failure and stale selector behavior.
- **Repository admission:** 7,328–7,575 and 17,357–17,565, then broad host-driven scenarios. Admission is validated against identity/current facts and is kept inert until explicit activation.
- **Repository membership:** 7,328–8,917 and 9,604–11,685 (at least 44 attributes across these ranges): member presentation/removal, activation publication, barriers, close interaction, generation races and linked worktrees.
- **Repository mutation and review tickets:** 911–2,180, 3,726–7,143 and 11,917–14,791 (over 25 ordinary tests plus 9 ignored/live scenarios): preparation/confirmation, mutation proofs, ticket freshness, uncertain effects, refresh and no replay.
- **Commit identity/authorization:** 2,298–3,037, 7,263–7,327, 11,725–11,852, 12,757–12,778, 13,757–14,791, 14,668–15,821, and close/race tests. Identity generation binds to repository/index review and revocation; it is exercised with activation, mutation, connection and close.
- **Effective authority:** 11,725–11,916 plus direct snapshot use throughout root state/live tests, including 19 direct calculation/query references cited in Task 399. Tests classify status, binding, reviewed Commit and tool authority across profile/provider/repository state.
- **Remembered Workspace:** 16,877–17,601 (13 tests, including four direct root-private adapter couplings detailed below) and the ignored Task 335 certification at 20,002–20,674.
- **Provider/runtime lifecycle:** 3,153–3,725; 7,218–7,327; 7,328–8,917; and 9,604–11,686. Model readiness and generation tests are at 15,822–16,379. Connect/disconnect, provider composition, runtime start, activation publication and cancellation are spread across families.
- **Conversation execution:** 3,153–3,357 and 14,840–15,821 plus the large Task 335/352/Task361 host scenarios. Cancellation/terminal ownership and activity are tested with repository and runtime currentness.
- **Conversation persistence:** 16,380–16,731 and preference-separation assertions at 18,060–18,167; Task 335/352 also check durable outcomes. It is tied to selected repository namespace and lifecycle boundaries.
- **Windows host/live certification:** all 17 ignored `main_tests.rs` tests are individually enumerated in §8. Their code and support occupy several thousand lines; there is no single, uniform live-only fixture layer.

## 3. Private production coupling inventory

`main_tests.rs` is declared as `crate::tests`, a direct child of the Windows root module (`main.rs:9878–9879`). Its imports from `super` explicitly include root-private types, fields-bearing state, helpers, and command handlers. Rust child privacy allows these tests to access private root ancestors. A sibling responsibility module does not inherit that privilege. A test nested under the production module it tests would retain access to that module's own private items and its private ancestors.

### 3.1 Private fields and DTO data

The following private state groups are named or mutated directly by tests. The access is mixed: some tests inspect assertions, while setup and race tests must install state or test hooks.

| Root type / private field group | Families and use | Assertion vs setup | Decomposition implication |
|---|---|---|---|
| `DesktopAppState`: `connection`, `chat`, `active_chat`, chat/connection generation counters | Runtime, status, activation and chat families | Both; state construction, state transition setup, assertions | Largest shared-state dependency; moving a family to another root child remains feasible only if it can access root ancestor members. Moving related production fields into a sibling owner would break existing access. |
| `DesktopAppState`: `repository`, `repository_generation`, `workspace_membership`, `membership_coordination`, `lifecycle_coordination` | Admission, membership, activation, close and repository authority | Both; member/repository injection, barrier setup and currentness assertions | Direct coupling reflects the actual admission/activation transaction and shared lock order. |
| `DesktopAppState`: `repository_workflow`, `repository_index_effect_reservation`, `next_repository_index_effect_token` | Staged review, commit authorization, Stage/Unstage and mutation | Both; reserve/replace workflow and prove stale/uncertain outcome | Crosses review, index effects and lifecycle; not test-only coupling. |
| `DesktopAppState`: `commit_identity`, `commit_identity_generation`, `commit_capability` | Commit identity, authorized Commit, profile/model/repository transitions, close | Both; direct state seed and revocation/assertion | Bound to several production domains by design. |
| `DesktopAppState`: `trusted_profile`, `trusted_profile_generation` | Profile selection and provider-composition tests | Both | Test seam is narrow in profile-only cases but composition depends on model/connection lifecycle. |
| `DesktopAppState`: `host_invocation`, `provider_activation` | Host ticket state and runtime/provider tests | Both | One excludes concurrent model/host work; the other owns the active runtime composition. |
| `DesktopAppState`: `remembered_workspace`, `preferences`, `preference_ordering` | Remembered catalog/admission; Desktop Preferences and Trusted Profile persistence | Both | Separate durable domains share app state and ordering; not safe to infer one persistence owner. |
| `DesktopAppState`: `model`, `conversation`, `persistence`, `neutral_workspace` | Model/readiness, chat, transcript and preference-separation tests | Both | Context/namespace selection deliberately connects repo, model and durable transcript behavior. |
| `DesktopAppState` test hooks: activation, authorization, connect-publication, index-effect and workflow-refresh hooks | Concurrent lifecycle, commit and close tests | Setup required; channel/barrier release and final assertions | Hooks are in the root state and used to force interleavings; moving tests requires the same production test seam or a support API. |
| Private nested structs/enums: `DesktopModelState`, `DesktopConversationState`, `DesktopRepository`, `DesktopCommitIdentity`, `RepositoryWorkflowState`, host activity and `ProviderEndpoint` inputs | Numerous deterministic tests construct/inspect internals | Both | Parent-child access currently exposes fields without public API changes. A sibling module boundary can create many field-level visibility edits. |
| Remembered adapter DTOs: presentation/status/candidates/candidate ID/label/location, `RememberedCandidateUpdateRequest.location_hint`, `RememberedLocationHintUpdate` variants | Four deterministic adapter tests and three live/certification paths (exact list §9) | Both | Task 390 counted seven `pub(super)` fields plus direct types/helper access for full move if tests remain in root. |

The list of `DesktopAppState` fields above follows the actual private state declaration at `main.rs:392–449`. Not every test family reads every field; the class-level coupling is broad, and the fixture builders construct cohesive state rather than isolated DTOs. The exact field-to-test relationship for the remembered adapters is available in Task 390 and enumerated in §9.

### 3.2 Private helper functions

The test import block at `main_tests.rs:17–75` directly names a wide root helper surface. These are the major responsibility groups, not a claim that each helper has one isolated caller:

| Helper group / representative symbols | Production responsibility | Caller topology |
|---|---|---|
| `current_app_status`, `model_configuration_status`, `frontend_error`, presentation converters | Status and closed/sanitized frontend results | Ordinary status, readiness and error tests; some host/live evidence tests inspect serialized result. |
| `effective_authority_snapshot_for_state`, `get_effective_authority_snapshot`, `repository_tool_authority`, `current_host_generation_tuple`, `connection_publication_is_current` | Authority snapshot and provider/repository currentness | Ordinary authority, connect, membership and commit tests; ignored Task 335/352/Task361 and live host flows also inspect snapshots. |
| `repository_snapshot`, `desktop_repository_snapshot`, `desktop_repository_snapshot_with_review`, `selected_git_executable`, `staged_review_digest` | Repository observation/review capture | Ordinary snapshot, stale-target, selector and review tests; live mutation harnesses use observations as pre/postcondition evidence. |
| `admit_repository`, `admit_repository_with_semantic_validation`, `admit_remembered_candidate`, `admit_remembered_workspace_candidate`, `replace_selected_repository` | Root-owned admission/identity and publication | Ordinary admission/member/currentness tests; remembered and multi-repository live scenarios. |
| `activate_admitted_member`, `activate_repository_member_selector`, `publish_activation_if_current`, `close_repository_transition`, `remove_repository_member_selector` | Membership, activation, close, removal and lifecycle coordination | Deterministic interleaving tests using hooks/barriers; ignored inactive-member, close and linked-worktree certification. |
| `prepare_codex_connection`, `resolve_prepare_and_connect_codex`, `begin_connect`, `connect_codex`, `connect_prepared_codex`, `disconnect_codex`, `publish_connected_provider_state` | Provider/runtime lifecycle | Ordinary transition tests; Task126 ignored host probe and other live host-driven scenarios. |
| `host_prepare_repo_*`, `host_confirm_tool_invocation`, `host_cancel_tool_invocation`, `validate_host_confirmation_ticket`, `run_host_tool`, `host_invoke_read` | HostExplicit command/ticket and bounded dispatch | Ordinary zero-effect and mutation tests; 8 ignored HostExplicit/multi-repo/Remembered live functions contain direct calls, per Task 393. |
| `authorize_repository_commit_review`, `repository_authorize_commit_review`, `repository_stage_action`, `repository_unstage_action`, `begin_repository_index_effect`, `complete_repository_index_effect`, `refresh_repository_workflow` | Reviewed Commit and bounded index mutation | Ordinary review/currentness/race tests and ignored close/live evidence. |
| `trusted_profile_selection_allowed`, `publish_trusted_profile_selection`, `restore_trusted_profile_selection`, `save_trusted_profile_preference`, `clear_trusted_profile_selection`, `forget_trusted_profile_preference` | Profile intent and persistence policy | Ordinary profile/persistence and connect composition; Task335 live evidence reads current profile/provider composition. |
| `remembered_catalog_presentation`, `reveal_remembered_location`, remembered parser/request helpers | Descriptive Remembered Workspace adapter behavior | Four deterministic tests and three broader live scenario functions; details in §9. |
| `begin_chat`, `clear_conversation_allowed`, `activity_event*`, terminal/cancellation helpers | Chat execution, cancellation, activity lifecycle | Ordinary deterministic tests and live host scenarios; shared currentness and persistence are exercised together. |

Direct helpers called from ignored/live paths are concentrated in the later certification families, but the distinction is not “helper used only by tests”: most are also production call targets. Moving a helper to a sibling production module while leaving its caller in `main_tests.rs` requires the helper itself (and sometimes its argument/result fields) to become visible to the root or test module.

### 3.3 Direct Tauri handler calls

Ordinary tests call command functions directly as Rust functions to verify behavior without launching the host. These include model selection/reset, Trusted Profile selection/restore/forget/clear, remembered catalog/reveal/mutation/admission, membership query/remove/activate/close, HostExplicit read/prepare/confirm/cancel, chat/conversation commands, commit identity and Stage/Unstage paths. Some use private request/response DTOs directly; this is why handler-only relocation can still cause sibling visibility pressure.

Ignored/live calls are separately concentrated in eight scenario functions with 20 direct handler references, as recorded in Task 393/396:

`windows_live_desktop_hostexplicit_create_file`, `windows_live_desktop_hostexplicit_delete_file`, `windows_live_desktop_hostexplicit_rename_file`, `windows_live_desktop_explicit_host_tool_invocation`, `windows_live_desktop_hostexplicit_repo_patch`, `windows_live_desktop_hostexplicit_multi_file_edit`, `task_324_c_windows_host_driven_two_repository_live_certification`, and `task_335_windows_host_driven_remembered_workspace_live_certification`.

These scenarios exercise production Tauri command seams within a host-driven runtime lifecycle. Other ignored scenarios often drive real ToolRegistry tools and observe the resulting host state rather than call the corresponding Tauri command directly. Preserve that distinction; an ignored scenario is not automatically a direct handler test.

### 3.4 Private state structures

Tests directly construct or mutate `DesktopAppState`, `DesktopConversationState`, repository and commit identity/workflow state, provider/runtime connection state, model/preferences/profile state, remembered catalog state, membership state, host invocation/review state, and live fixture state. Test hooks and barriers are also private root state. Setup usually seeds multiple cooperating fields to create an exact lifecycle generation or authority snapshot; assertions then verify all owners remain coherent. This is genuine transaction coverage as well as a privacy consequence.

## 4. Test-owned support topology

| Fixture/helper | Source range | Families using it | Shared? / private-root dependence |
|---|---:|---|---|
| Counting/failed create/delete/rename tools and readiness server | 132–305 | Host tool dispatch; model readiness | Readiness server is readiness-specific; counting tools recur across host preparation and mutation. Uses production tool types. |
| `TestRepository` and Git state builder | 406–540 | Snapshot/review, mutation, admission, membership and commit | Genuinely shared; filesystem and Git constructors centralize test semantics. Tests across those families use it. |
| `LinkedWorktreeFixture` | 548–641 | Linked worktree, membership and lifecycle | Specialized, but shared by admission/activation and Gitfile/junction checks. Encodes Windows Git setup/cleanup. |
| Registry/composition builders and ticket/commit authorization helpers | 643–864 | HostExplicit preparation, delete/rename, commit/review | Shared across multiple mutation families; depend on private tool composition and commit state. |
| Live Git/event/file identity helpers | 3,463–3,713 and 4,493 onward | HostExplicit, repository mutation, event-based certification | Live-only support, but grouped with and reused across different live scenario blocks; direct root state and host event dependencies. |
| Activation, member, authorization, connect-publication and index-effect barriers | 8,918–9,015 | Membership, commit, connection, close and index mutation races | Genuinely cross-responsibility; barriers are installed on root-private `DesktopAppState` hooks. Extracting them to a responsibility-only module would not remove their shared ownership. |
| Real index reservation/activation fixture, fake executable, runtime builder | 9,028–9,439 | Admission/currentness and actual Stage/Unstage interleavings | Reused in lifecycle and provider/runtime tests; state plus Git fixture dependency. |
| Activation/close fixture, close commit review, request and storage snapshot | 9,448–9,603 | Close, membership, commit review and conversation persistence | Cross-domain by design: it creates a coherent active repository and captures persisted files. |
| Remembered catalog builders/assertions | 16,877–17,217 | Remembered state, presentation, admission and reveal | Responsibility-specific catalog fixtures, but builder creates root app state and assertions prove zero repository authority. Root-private access is needed. |
| Task324 event observer/process/managed-state fixtures | 18,191–19,351 | Windows lifecycle and multi-repo certification | Certification-specific; contains event sequence, process and cleanup evidence. |
| Task335 host-driven remembered-workspace setup | 20,002–20,674 | Remembered workspace + admission + provider/runtime + frontend event proof | Not a reusable simple fixture. Full host lifecycle; direct calls to root adapter helpers. |
| Task352 repository capture and Windows Git fixture | 20,471–22,567 | Close + membership + persistence + host live evidence | Large specialized certification support. Depends on repository state and root snapshot/persistence APIs. |
| Task361 linked-worktree live fixture/capture/scenario builders | 22,168–24,067 | Linked worktrees, admission, repository list/search/activation/runtime scenarios | Large host certification subsystem with multiple scenario families. Not a generic helper library. |
| Serialization and authority/event assertions | distributed, notably 11,687–15,821 and later live ranges | Presentation, HostInvocation, activity and certification | Some are tiny reusable predicates; many encode responsibility-specific evidence and should remain near the evidence they assert. |

Fixture movement is not uniformly beneficial. `TestRepository`, lifecycle barriers, and active close fixtures are genuinely shared, while Task324/335/352/361 support is deliberately scenario-specific. A single `main_test_support.rs` would need to expose root-private builders/state to multiple test children or accumulate many unrelated Windows certification tools. A narrow support module might become useful after selecting one test family, but current evidence does not justify creating a second 20k-line root of test logic.

## 5. Cross-responsibility test inventory

These tests intentionally prove that one lifecycle or authority decision remains coherent across multiple domains. Their current placement in the root test child is semantically appropriate.

| Test(s) | Responsibilities crossed | Private internals / fixtures | Ordinary or live | Placement assessment |
|---|---|---|---|---|
| `restored_identity_and_fresh_bound_review_serialize_authorize_presentation`; `authorize_then_refresh_revokes_pending_and_rotates_review_selector_without_commit` | Commit identity + repository snapshot/review + authorization | Commit identity/capability, review selectors, `TestRepository` | Ordinary | Could live in a future integration-test child, but should remain root/integration while production ownership is shared. |
| `task_316_membership_admission_and_activation_matrix_is_inert_and_current`; `task_359_linked_worktrees_use_one_revalidated_active_composition`; `task360_linked_activation_rejects_gitfile_change_at_publication_barrier` | Admission + membership + provider composition + Git worktree identity | Root state, membership IDs, activation publication, Git fixtures/barriers | Ordinary | Keep root/integration; moving to one responsibility would hide what the test proves. |
| `task_341_remove_inactive_member_preserves_active_repository_lifecycle_state`; `task_341_target_bound_index_effect_is_busy_but_active_a_effect_is_unrelated`; `task_341_removal_preserves_remembered_candidate_bytes` | Membership removal + runtime/current repository + index effect + remembered catalog | Multiple root fields, activation/index barriers, remembered bytes | Ordinary | Keep root/integration. |
| `task349_close_withdraws_active_state_and_preserves_membership_and_persistence`; close race tests 10,076–10,807 | Close + active member + runtime + workflow/review + persistence | Close fixture, commit review, storage snapshot, state hooks | Ordinary | Keep root/integration; these are transaction tests. |
| `task_321_e_identity_change_at_connect_publication_barrier_is_stale`; `activation_*` and `task_320_*` race tests | Model/profile/commit/repository currentness + provider/runtime publication | Connect and activation hooks, generation fields | Ordinary | Keep root/integration. |
| `reset_and_restore_keep_preference_and-conversation-persistence-separate` (actual test: `reset_and_restore_keep_preference_and_conversation_persistence_separate`); `preference_and_conversation_warning_domains_do_not_cross` | Desktop Preferences + conversation persistence | Root preferences/conversation/persistence state and temporary storage | Ordinary | Could be split only by losing the tested non-interference contract; keep root/integration. |
| `task_333_remembered_admission_is_fresh_full_pipeline_and_inert`; `task_333_catalog_identity_is_not_process_member_identity_and_isolated` | Remembered Workspace + repository admission + membership identity | Remembered state, repository fixtures and root admission | Ordinary | Keep root/integration; catalog state must not be confused with admitted member identity. |
| `task_335_windows_host_driven_remembered_workspace_live_certification` | Remembered catalog + reveal + admission + profile/runtime + frontend events | Live host setup, event observer, root-private presentation fields | Ignored/live | Keep live/certification root; do not turn it into a unit fixture. |
| `task_352_windows_active_repository_close_live_certification` | Close + membership + authority snapshot + transcript persistence + host event proof | Task352 Git fixture, close/capture and root state | Ignored/live | Keep live/certification root. |
| `task361_run_live_scenarios` and `task_361_windows_linked_worktree_live_certification` | Admission + membership + linked worktree/reparse behavior + host runtime/tool execution | Task361 fixture/capture plus direct root authority snapshots | Ignored/live | Keep live/certification root. |

The named examples are not an exhaustive list of every test that touches two fields. They identify the strongest visibly intentional cross-responsibility contracts. The broadest supporting evidence is the 2,083-line close/activation range and the 1,500-line Task361 fixture/scenario support range.

## 6. Ignored/live certification topology

All ignored tests found in `main_tests.rs`:

| Line | Test | Certified behavior / direct handlers | Private support and environment | Cross-responsibility |
|---:|---|---|---|---|
| 913 | `windows_live_desktop_hostexplicit_create_file` | HostExplicit create-file dispatch | Certified Windows Codex gate, file identity/proof helpers, host event evidence; direct prepare/confirm path | Host authority + filesystem + activity/currentness |
| 1,536 | `windows_live_desktop_hostexplicit_delete_file` | HostExplicit delete-file dispatch | Same certified gate and live filesystem fixture | Host authority + delete proof + repository refresh |
| 3,419 | `task_126_host_probe_uses_desktop_repository_runtime_construction` | Pinned Codex `runtime.start` construction probe | Codex 0.149.0 executable and inherited config | Runtime + selected repository/tool composition |
| 3,728 | `windows_live_desktop_hostexplicit_rename_file` | HostExplicit rename dispatch | Certified gate, source/destination identity helpers | Host authority + rename proof + refresh |
| 4,508 | `windows_live_desktop_explicit_host_tool_invocation` | Explicit host tool invocation | Certified Windows gate, event/host state | Provider runtime + ToolRegistry + HostInvocation |
| 5,040 | `windows_live_desktop_hostexplicit_repo_patch` | Patch preparation/confirmation | Certified gate and real repository proof | Ticket + repository mutation + workflow refresh |
| 5,520 | `windows_live_desktop_hostexplicit_multi_file_edit` | Multi-file edit preparation/confirmation | Certified gate, file identity/capture | Ticket + multi-file mutation + review/presentation |
| 6,202 | `windows_live_desktop_repo_create_branch` | Repository branch creation | Certified Codex gate/authentication and Git state | Host invocation + repository authority + Git effect |
| 6,697 | `windows_live_desktop_repo_create_branch_host_driven` | Host-driven branch creation | Windows live certification environment | Host/runtime + branch mutation |
| 8,511 | `task_343_windows_host_driven_inactive_member_removal_live_certification` | Inactive member removal | Explicit `RAH_RUN_V028_INACTIVE_MEMBER_REMOVAL_LIVE=1` | Membership + active repository lifecycle |
| 19,171 | `task_324_d_windows_codex_app_server_ownership_evidence` | Codex app-server process ownership evidence | Explicit triage env + certified Windows gate; process census helpers | Runtime process + host ownership |
| 19,354 | `task_324_c_windows_host_driven_two_repository_live_certification` | Host-driven two-repository lifecycle | Explicit live env + certified Windows gate; Task324 fixtures | Repository admission/membership + provider/runtime + frontend events |
| 20,004 | `task_335_windows_host_driven_remembered_workspace_live_certification` | Remembered catalog/reveal/admission live evidence | Explicit live env, host events, root-private presentation helpers | Remembered state + admission + runtime |
| 20,677 | `task_352_windows_active_repository_close_live_certification` | Active repository close evidence | Explicit live env, Task352 Git fixture and persistent storage capture | Close + membership + persistence + authority |
| 24,070 | `task369_windows_repository_search_live_certification` | Repository search host tool | Explicit search-live env, repository capture | Runtime/tool + repository observation |
| 24,570 | `task379_windows_repository_list_live_certification` | Repository listing host tool | Explicit list-live env, repository capture | Runtime/tool + repository observation/admission |
| 25,238 | `task_361_windows_linked_worktree_live_certification` | Linked worktree admission/activation/effect lifecycle | Explicit live env, Task361 fixture including worktree/junction helpers | Linked-worktree identity + membership + runtime/tool execution |

The additional package ignored test is `git_discovery.rs:366`, `task_125` host-only production resolver probe. It is not in `main_tests.rs` and is not a Windows live Desktop scenario.

**Topology finding:** the 16 Windows host/live scenarios are not one uniform fixture suite. The first nine HostExplicit/runtime tests share a live event/host vocabulary but have different real mutation setups. Task324, Task335, Task352, repository search/list and Task361 each have specialized lifecycle or repository fixtures. Task335, Task352 and Task361 also have direct references to remembered presentation or root authority snapshots. The harnesses are interwoven with shared root state and fixtures, but splitting them into another sibling test module would preserve root-private access only while the production symbols remain in the root; it would not solve access to a future sibling owner. Do not split live certification as a proxy for production decomposition.

## 7. Visibility consequences and family suitability

### Case A — tests nested under the production module they test

This removes parent-root privacy requirements for items now owned by that module: the nested tests can access its private types, fields and helpers. Root command registration still needs parent-visible command handlers where a child production module is registered from `main.rs`; test colocation does not remove that registration edge. Tests that cross sibling production modules still need an integration location or a narrow test-support contract. Shared `TestRepository`, lifecycle barriers, and environment setup do not become visible just because the test is nested.

### Case B — sibling responsibility test file

A sibling such as `repository_membership_tests.rs` does **not** inherit private access to `repository_membership.rs`. It can still see private items in their common root ancestor, but not private fields/helpers defined in the production sibling. To use those private items, nest tests in the production module or introduce explicit visibility/API changes. A filename split alone is organizational, not a privacy solution.

### Case C — retained root integration tests

Tests spanning application state and multiple owners should remain children of the root/test integration seam (or another explicit root integration child). They have valid access to root-private setup because they test root composition. Moving them under one domain can obscure scope and create false ownership pressure. This includes membership/admission/activation, close, commit+identity+review, profile/provider composition, repository+runtime, and transcript namespace non-interference.

### Descriptive classification by family

| Test family | Classification | Evidence |
|---|---|---|
| Pure status presentation/error mapping | **POSSIBLE WITH SHARED FIXTURE EXTRACTION** | A small isolated presentation subset exists, but the query adapter consumes root-owned `AppStatus` and app state; current status tests are close to connection/runtime transitions. |
| Model endpoint/readiness and model configuration | **POSSIBLE WITH SHARED FIXTURE EXTRACTION** | 20 tests form a recognizable family. They share `ReadinessTestServer`, private endpoint/model DTOs and root state, and readiness publication depends on model generation and connection. |
| Desktop Preferences and Trusted Profile policy | **HIGHLY ENTANGLED — DEFER** | Tests explicitly prove preference/profile state, provider composition, and conversation persistence remain distinct and generation-safe. |
| Remembered Workspace adapter deterministic tests | **POSSIBLE WITH SHARED FIXTURE EXTRACTION** | Four direct private adapter tests are identifiable (see §9), but root app state and admission fixtures are shared; the three live callers retain direct access needs. |
| Repository observation/review | **POSSIBLE WITH SHARED FIXTURE EXTRACTION** | Several behavior tests are locally coherent, but `TestRepository`, commit authorization and live mutation evidence are shared. |
| Repository admission/membership/activation/close | **KEEP ROOT/INTEGRATION** | Test bodies deliberately cross admission, active membership, runtime/provider, commit/workflow, close and persistence; hook/barrier infrastructure is shared. |
| Repository mutation/review tickets and HostExplicit | **KEEP LIVE/CERTIFICATION ROOT** for ignored scenarios; **HIGHLY ENTANGLED — DEFER** for deterministic tests | Production command behavior is testable deterministically, but direct command and fixture use spans tickets, authority, filesystem effects, review and activity. Keep certification bodies at root. |
| Commit identity/authorization | **HIGHLY ENTANGLED — DEFER** | Identity changes revoke or bind review/workflow across connection, repository generation, mutation and close. This is real production coupling. |
| Effective authority snapshot | **KEEP ROOT/INTEGRATION** | The snapshot is computed from live root-owned state across provider, profile, repository and commit owners. The type module is already separated; calculation and assembly are not a pure DTO concern. |
| Conversation in-memory pair/replay rules | **POSSIBLE WITH SHARED FIXTURE EXTRACTION** | Nine deterministic tests form a compact range, but exercise root `DesktopConversationState` and namespace transitions as well as persistence calls. |
| Durable conversation persistence/namespace behavior | **KEEP ROOT/INTEGRATION** | Tests assert repository-selected namespaces, preference warning separation and close behavior. Namespace selection is policy-bearing root logic. |
| Windows ignored/live certification | **KEEP LIVE/CERTIFICATION ROOT** | Each harness depends on root state and responsibility-specific host evidence; no uniform fixture seam would make sibling visibility safe. |
| Task361 layout/identity cases | **HIGHLY ENTANGLED — DEFER** | The 1,500-line fixture/scenario layer spans member admission, linked worktree identity, tool execution and authority evidence. |

No family is classified **GOOD COLOCATION CANDIDATE** for a test-only move that both avoids shared-support work and materially advances production extraction. Existing `provider_composition.rs`, `desktop_preferences.rs`, `conversation_persistence.rs`, `remembered_workspace.rs` and `effective_authority.rs` already contain colocated unit suites for their own private implementation. Those existing module tests show the pattern works when production ownership is already local; they do not resolve root-owned Desktop lifecycle tests.

## 8. Remembered Workspace test-topology reassessment

Task 390's blocker is confirmed and refined. Its proposed production group was the six catalog/reveal/mutation handlers, DTO/presentation and parser helpers (about 269 production lines), separate from remembered storage and root admission.

| Exact test | Direct coupling | Fixture/cross-domain dependency | Type |
|---|---|---|---|
| `task_333_catalog_actions_are_durable_ordered_and_privacy_safe` (`main_tests.rs:17219`) | Calls private `remembered_catalog_presentation`; reads serialized fields | Root `DesktopAppState`, catalog mutation/storage and zero-authority assertions | Deterministic |
| `task_334_reveal_is_explicit_exact_and_authority_free` (`:17418`) | Calls private `reveal_remembered_location`; reads returned `candidate_id` | Root state, remembered location and authority-free result | Deterministic |
| `task_334_reveal_rejects_unavailable_catalog_without_path_access` (`:17469`) | Calls private reveal helper | Unavailable catalog and no-path-access setup | Deterministic |
| `task_333_update_request_distinguishes_missing_and_explicit_clear_hint` (`:17583`) | Names private `RememberedCandidateUpdateRequest`, reads `location_hint`, matches `RememberedLocationHintUpdate` variants | Root request/parser contract | Deterministic |
| `task_335_windows_host_driven_remembered_workspace_live_certification` (`:20004`) | Calls catalog/reveal helpers and reads private presentation fields | Host lifecycle, event evidence, admission/runtime and live environment | Ignored/live |
| `task_352_windows_active_repository_close_live_certification` (`:20677`) | Calls catalog presentation helper for lifecycle evidence | Close, membership, persistent capture and live host state | Ignored/live |
| `task361_run_live_scenarios` (`:22666`, called by ignored Task361) | Calls catalog presentation helper | Linked-worktree layout/admission/runtime scenario fixture | Live certification helper, not itself a test attribute |

The three private-field coupling points in Task 390 refer to tests directly naming the update request/variant and reading presentation outputs. Its full unchanged-test move analysis estimated at least 16 `pub(super)` production items plus 7 `pub(super)` fields if all seven callers remain in the root test child. If all seven test functions could nest under the future command module, test-only helper/DTO/field widening could be avoided; root command registration and the root admission parser would still need parent access.

**Conclusion:** Colocating just the four deterministic tests would remove their private access pressure, but the live Task335/Task352/Task361 references would still be callers of private presentation data. Moving all seven under a new module is not a clean bounded move: three are embedded in broad lifecycle/certification harnesses and depend on shared root setup. Thus test decomposition would materially reduce the deterministic portion, but would not by itself reopen Task390's production extraction. The Task390 conclusion remains: defer the Remembered Workspace root-adapter move until a complete live/test boundary can be kept private without dragging broad root fixtures, or accept an explicitly audited `pub(super)` budget in a separately authorized production task.

## 9. Other production regions: test pressure versus ownership coupling

| Region | Test-caused pressure | Genuine production coupling | Reassessment |
|---|---|---|---|
| Effective authority | High call/reference count; tests directly call `effective_authority_snapshot_for_state` and inspect serialized snapshot/fields; live suites also use it. A new sibling calculation module would need visibility or tests colocated there. | The calculation reads repository selection/generation, connection/provider composition, profile, tools, reviewed Commit and currentness. Those are separate root owners and lifecycle snapshots. | Test placement is a real cost, but the snapshot's inputs and consistency rule are the larger boundary problem. **Do not solve by moving tests alone.** |
| Repository membership lifecycle | Many tests use private state, barriers and direct transitions. Moving only test bodies would need activation/close/index fixtures. | Admission, membership publication, active repository, runtime composition, HostInvocation exclusion, workflow/review and close share ordered locks/currentness. | Production coupling dominates. Keep transaction/race tests as root integration tests. |
| Trusted Profile | Direct private profile/preferences/model/connection fields; selection helpers are imported and tested. | Profile is a static provider-only configuration, but selection generation, durable remembered path, connection/model currentness and provider publication are root lifecycle policy. | Tests show a possible isolated profile parser/presentation subset, but not a clean command/lifecycle owner. Test decomposition is secondary. |
| Commit identity/authorization | Tests seed private identity/capability/workflow fields and inspect revocation through multiple routes; direct test access is broad. | Identity authorization binds review to repository/index/HEAD facts and is revoked by repository changes, profile/model/connection transitions, and close/mutation. | Genuine policy and transaction coupling is primary. Tests belong at the root composition boundary until that model changes under separate authorization. |
| Conversation/persistence/transcript | Conversation tests construct private app state; preference and live tests assert namespace and warning separation. | `Persistence` is a child owner, but root chooses `neutral-v1` vs repository-key namespace and controls completed pairs, clear/resume/transcript read and lifecycle. | Keep namespace/policy integration tests root. Only lower-level persistence serialization tests are properly module-local and already live in `conversation_persistence.rs`. |

The main tests contribute friction to moving root-owned fields into siblings. But where the test cannot be reduced without losing an intentional multi-owner invariant, the coupling is evidence of a real production relationship, not test organization noise.

## 10. Structural options

### Option A — responsibility-colocated deterministic tests

**Finding:** Technically valid only when the test can sit inside the production owner that it exercises and shared fixtures are available there. Candidate families include a carefully selected endpoint/readiness subset, Remembered Workspace adapter tests, and conversation pair/replay tests. A child test module can keep its production owner's private items private. Tauri registration still needs parent-visible handlers where applicable.

**Cost:** Existing tests frequently call root-private handlers and construct root state; moving them under a new sibling responsibility module does not grant access to that module's private implementation. Remembered Workspace has 3 broad live callers; endpoint/readiness needs its local server and root model state; conversation tests cross root state and persistence namespace policy. No bounded family currently provides an implementation move with zero shared support and clear production ownership payoff.

### Option B — extract shared test support first

**Finding:** There is a real shared core: `TestRepository`, Git/worktree fixtures, root-state composition and concurrency hooks. There are also four unrelated versioned Windows evidence fixtures (Task324/335/352/361).

**Cost:** A generic `main_test_support.rs` is likely to become a second giant shared module. To let nested responsibility tests use support functions, the support module and chosen functions would need a narrow parent-visible `pub(super)` test-only contract or a colocated support child. That creates a new visibility surface and does not fix production ownership coupling. Avoid extracting support globally before a single target family is frozen.

### Option C — separate live certification harness

**Finding:** A live-only module could be declared as another child of root and retain access to root-private items while production ownership remains in `main.rs`. It could reduce the ordinary `main_tests.rs` size on disk.

**Cost:** It does not reduce visibility pressure against a future sibling production owner, because a root sibling test harness still cannot access that owner's private items. Live fixtures are responsibility/version specific; Task335/352/Task361 have remembered catalog and snapshot helper calls. Moving them risks obscuring exact host-certification runner identities and shared fixture ownership. This is not the right first structural operation.

### Option D — retain test topology unchanged

**Finding:** Best current option. Keep the 17 ignored `main_tests.rs` tests at the root test boundary and retain cross-domain deterministic tests there. Continue to use nested tests inside already-owned production modules for local implementation behavior, as the repository already does.

**Cost:** `main_tests.rs` remains large and some isolated deterministic tests remain with root integration tests. This is acceptable until one future production boundary can name the precise private symbols and fixtures it actually owns.

## 11. Chosen direction, Task 401 and frozen boundary

**Chosen direction:** Do not decompose the Desktop test topology as a standalone structural project. For the next task, return to production ownership research and select one bounded candidate whose consistency rules can be owned by a module. Keep this test inventory as evidence for its exact test surface. Do not begin any Task 401 work automatically from this report.

**Recommended Task 401:** `Task 401 — Effective Authority Snapshot Ownership Boundary Research`.

Scope should be limited to the root snapshot calculation, its state inputs, callers, immutable snapshot type module, exact cross-domain currentness guarantees and the deterministic/live callers identified here. It should decide whether a pure calculation boundary can be expressed without moving lifecycle owners or widening visibility. It must not move tests, change test paths, change visibility, implement extraction, touch permission findings, or reorganize live certification.

No implementation boundary is frozen by Task 400. Consequently there is no destination production/test file, exact implementation test list, fixture extraction list, or visibility budget to authorize. The next research task must freeze those only if it finds an ownership boundary that survives the dependency audit.

## 12. Test identity and future validation

Current unit names are rooted under `rah_desktop::tests::<name>` (subject to Cargo's test target formatting). A nested production test module would change the path, for example to `rah_desktop::effective_authority::tests::<name>` or `rah_desktop::remembered_workspace_commands::tests::<name>`. Search of the repository found exact references for the Remembered Workspace live certification and Task352 command invocation in the prior research/audit docs, and the test names themselves in this report's source/task plans; arbitrary external scripts cannot be ruled out.

Potential consequences:

- CI that filters by a bare test-name substring should continue to match; fully qualified filters and exact manual commands need updating.
- The documented Task335/Task352 ignored live invocation names must remain resolvable if moved. Do not weaken `--ignored`, environment gates, test threading, or certification instructions.
- Git history and test runner names change even when a body is mechanically identical. Any future move should search the repository for each exact test name and preserve an alias only if the runner supports one; do not create duplicate test executions as aliases.
- For a future authorized test move, require exact old/new test-name checks and compare `cargo test -p rah-desktop -- --list` before/after for discovered, passed, ignored and filtered counts. Preserve the 18 package ignored count and every explicit ignored test identity.

If a mechanical test move is later authorized, minimum validation is:

```powershell
cargo fmt --check
cargo test -p rah-desktop -- --test-threads=1
cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings
git diff --check
```

Additionally compare exact old/new fully qualified names, full `--list` output counts, and every ignored test's name/gate. For moved live tests, require the existing host-driven invocation and certification record to remain intact; do not execute live certification as part of an ordinary deterministic move unless separately authorized.

## 13. Non-goals and validation

- No tests were moved, rewritten, duplicated or created.
- No `#[cfg(test)]` structure or test visibility changed.
- No production visibility or modules changed.
- No fixtures or live certification harnesses were reorganized.
- No authority, permission, provider, persistence, architecture or Cargo behavior changed.
- No unrelated permission finding was fixed.
- No `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`, workspace validation or Windows live certification ran.

Docs-only validation executed: `git diff --check`.

## 14. Completion record

- Artifact: `docs/plans/2026-09-24-task-400-desktop-test-topology-decomposition-research.md`.
- Commit message: `docs: research desktop test topology decomposition`.
- Commit SHA: recorded after commit in the Task 400 completion report.
- Git status: recorded after commit in the Task 400 completion report.
- No push or tag was performed.
- Recommended next task: **Task 401 — Effective Authority Snapshot Ownership Boundary Research**; do not start it automatically.
