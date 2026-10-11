# Task 396 — Select and Freeze the Next Bounded Desktop Production Extraction Candidate

## Verdict

**PASS — NEXT BOUNDED DESKTOP PRODUCTION EXTRACTION CANDIDATE FROZEN**

Freeze the `model_configuration` Tauri query handler for Task 397. Move the handler and its existing attributes only. Keep the response DTO, its fields, provider endpoint presentation type, configuration status helper, all state, and every test in `main.rs`.

This is a research and selection record. No Rust production source was changed.

## Starting checkpoint

- Expected and observed `HEAD`: `a22a04d9ff1e36bd88b1476ebfec1fbfbc8bc0bf` (`refactor: extract desktop application status command`).
- `git status --short` was empty.
- `crates/rah-desktop/src/main.rs` has **9,915 physical lines**, measured with PowerShell `Get-Content` count at the verified starting tree.
- Existing extracted modules are `desktop_preferences_commands.rs` and `desktop_status_commands.rs`; `main_tests.rs` remains separate. None is modified by this task.
- The current `tauri::generate_handler!` list has 47 entries (lines 9835–9881 inclusive).

## Method and boundaries

Inspected current production declarations and the registration list, Desktop crate callers and `main_tests.rs` references, the Task 393 selection record and Task 394–395 extraction/audit records, architecture/security guardrails, and the relevant current Desktop authority and persistence boundaries. Ranges below are inclusive and refer to the verified starting `HEAD`.

The candidate scan includes six plausible read/query handlers. The audit distinguishes direct test references to a candidate handler from tests of retained DTOs, helpers, and shared state. No tests are proposed to move. No permission findings or permission/capability files were considered for modification.

## Candidate inventory

### 1. Model configuration query — selected

- **Responsibility:** Return the current model/provider endpoint and readiness presentation.
- **Exact handler range:** `main.rs:4832–4868`, 37 physical lines including `#[cfg(target_os = "windows")]`, `#[tauri::command]`, signature, and body.
- **Handler:** `model_configuration` (one).
- **Request / response:** No request DTO; managed `State<'_, DesktopAppState>` input; returns `ModelConfigurationPresentation`.
- **DTO/type ownership:** `ModelConfigurationPresentation` remains at `main.rs:1739–1750`; its six private fields remain untouched. `ProviderEndpointPresentation` and its `From<&ProviderEndpoint>` implementation remain at `main.rs:1752–1776`. `DesktopModelProvider`, `ProviderScheme`, `ProviderEndpoint`, `ReadinessState`, and `ConnectionState` remain root-owned.
- **Helpers / constants:** Calls retained `model_configuration_status` (`main.rs:1966–1975`), `ProviderEndpointPresentation::from`, and `ProviderEndpoint::insecure_transport`. No constant moves.
- **Shared state:** Reads `DesktopAppState.model` and `.connection` under their existing locks. It does not move or expose state fields.
- **Production callers:** Definition only plus current Tauri registration at `main.rs:9841`. No other Desktop production module calls the handler. No wrapper or re-export is needed.
- **Tests:** No direct call to `model_configuration`. `main_tests.rs` imports/uses retained `ModelConfigurationPresentation` in `model_configuration_presentation_is_closed_and_sanitized` (line 16301); that test constructs all six private fields and verifies serialization. Keep the DTO in the parent so this test and its six field accesses remain unchanged. Retained helper `model_configuration_status` is exercised by `model_and_repository_connection_generations_are_independent` (line 16367) and `preference_write_matrix_excludes_all_non_apply_reset_events` (line 17908). These are tests of retained DTO/helper behavior, not moved-handler coupling. No ignored/live scenario directly calls the handler or constructs the DTO.
- **Test-coupling counts:** Moved handler references 0; moved type references 0; moved helper references 0; moved private-field accesses 0; direct ignored/live references 0. One ordinary test references the retained DTO and six private fields; two ordinary tests exercise the retained status helper.
- **Authority sensitivity:** Low. It reports model configuration and readiness; it does not select, activate, or authorize a provider, compose `ToolRegistry` or `HostExplicit`, or perform runtime lifecycle work.
- **Persistence sensitivity:** None in this handler; it neither selects a persistence namespace nor reads/writes persisted data.
- **Minimum visibility delta:** `model_configuration` -> `pub(super)` for parent registration. Types 0, fields 0, helpers 0, constants 0. The child command module can use the root-owned private DTO and helper through the parent scope; no widening of any DTO field is needed.
- **Registration:** Direct `model_configuration_commands::model_configuration` at the existing list position, replacing `model_configuration` only.

### 2. Conversation transcript query

- **Responsibility:** Return the current persisted conversation transcript presentation.
- **Range:** `main.rs:9812–9823`, 12 lines including cfg/command attributes and body.
- **Handler:** `conversation_transcript` (one).
- **Request / response:** No request DTO; managed `State<'_, DesktopAppState>`; returns `ConversationTranscriptPresentation`, an alias imported from `conversation_persistence::Presentation` at `main.rs:31–34`.
- **Types / helpers / constants:** No candidate-owned DTO or enum. Calls `DesktopAppState::select_persistence_namespace` and `Persistence::presentation`; both remain in their owning modules/state. No constant moves.
- **Shared state / callers:** Uses `DesktopAppState.persistence` and the selected repository namespace. Production caller is the registration at `main.rs:9871`; no other production caller found.
- **Tests:** No direct `conversation_transcript` or `ConversationTranscriptPresentation` references in `main_tests.rs`; no moved-type field access. Retained persistence presentation is exercised through state/persistence in `preference_write_matrix_excludes_all_non_apply_reset_events`, `reset_and_restore_keep_preference_and_conversation_persistence_separate`, and `task_335_windows_host_driven_remembered_workspace_live_certification`. Transcript data is also inspected by live scenario `task_352_windows_active_repository_close_live_certification`. Those tests use retained state/API rather than this handler. No ignored/live scenario calls the handler.
- **Test-coupling counts:** Moved handler/type/helper/field references 0; direct ignored/live references 0; shared persistence presentation appears in ordinary and live state-level checks listed above.
- **Authority sensitivity:** Low for executable authority; it returns descriptive conversation data and grants no authority.
- **Persistence sensitivity:** Material. The handler calls namespace selection immediately before presentation. `DesktopAppState::select_persistence_namespace` itself remains the owner of the namespace operation, but relocating the command body also relocates the presentation path that chooses which durable namespace to read. This is a narrower but less clean persistence boundary than the selected candidate.
- **Minimum visibility delta:** Handler -> `pub(super)`; type/fields/helpers/constants 0 if alias and helpers remain root-owned.
- **Registration:** Direct module path is feasible (`conversation_transcript_commands::conversation_transcript`) at the existing entry. No wrapper, rename, or ordering change.

### 3. Commit identity status query

- **Responsibility:** Report whether a commit identity is configured.
- **Combined DTO-plus-handler range:** `main.rs:4928–4945`, 18 lines. Handler-only range: `main.rs:4935–4945`, 11 lines.
- **Handler:** `commit_identity` (one).
- **Request / response:** No request DTO; managed `State`; returns `CommitIdentityPresentation`, a one-field private DTO (`configured: bool`) at `main.rs:4928–4933`.
- **Helpers / constants:** None. Reads retained `DesktopAppState.commit_identity`; no constant moves.
- **Production callers:** Definition plus Tauri registration at `main.rs:9842`. Adjacent `set_commit_identity` is separately registered at line 9845 and owns validation, preference persistence, generation advancement, and revocation; it is not part of this candidate.
- **Tests:** No direct `commit_identity` handler or `CommitIdentityPresentation` reference. Tests and test helpers read or set the retained `DesktopAppState.commit_identity` and generation. References to the retained field occur in 18 distinct test/helper functions, including `authorize_test_commit`, `restored_identity_and_fresh_bound_review_serialize_authorize_presentation`, `binary_staged_review_is_not_authorizable_and_forged_selector_has_no_effect`, `authorize_then_refresh_revokes_pending_and_rotates_review_selector_without_commit`, `disconnect_revokes_pending_authorization_and_never_restores_old_review`, `connect_publication_currentness_includes_commit_identity_generation`, `desktop_successful_rename_revokes_reviewed_commit_authorization`, and `desktop_verified_directory_creation_revokes_reviewed_commit_authorization`; live scenarios also set/read it. No test requires access to a moved DTO if the type stays root-owned.
- **Test-coupling counts:** Moved handler/type/helper/field references 0; shared retained state references are broad (18 test/helper functions); several ignored/live setup paths use `set_commit_identity` or retained identity state, not the candidate handler.
- **Authority sensitivity:** Moderate. This handler only reads the configured flag, but that value is an input to reviewed Commit authorization. Its immediate neighbor owns mutation and authorization revocation. A read-only relocation is possible, but a module boundary named for Commit identity risks visually joining the query to authorization ownership.
- **Persistence sensitivity:** Low for the handler itself; persistence writes belong to `set_commit_identity` and preferences, which remain in `main.rs`/preferences module.
- **Minimum visibility delta:** Handler-only move: handler -> `pub(super)`; type/field/helper/constant 0. Moving its DTO too is not necessary and is excluded.
- **Registration:** Direct `commit_identity_commands::commit_identity` is feasible at the current slot.

### 4. Trusted profile selection query

- **Responsibility:** Present whether a trusted profile is remembered and the current trusted-profile presentation.
- **Range:** `main.rs:4749–4767`, 19 lines.
- **Handler:** `trusted_profile_selection` (one).
- **Request / response:** No request DTO; managed `State`; returns retained `TrustedProfilePresentation`.
- **Helpers / constants:** No explicit helper call; reads retained preference and trusted-profile state. No constants move.
- **Production callers:** Definition and registration at `main.rs:9836`; mutating siblings `choose_trusted_profile`, `restore_trusted_profile`, `forget_trusted_profile`, and `clear_trusted_profile` remain separately registered.
- **Tests:** No direct handler/response DTO/private-field reference. Lower-level retained selection functions are covered by `clear_selection_is_process_local_and_generation_is_exact`, `restore_rereads_current_source_without_persisting_or_spawning`, and `forget_removes_only_preference_and_save_failure_is_non_destructive`. No ignored/live scenario directly calls the handler.
- **Test-coupling counts:** Moved handler/type/helper/field references 0; retained state tests exercise lower-level ownership; direct ignored/live references 0.
- **Authority sensitivity:** Moderate to high adjacency. The result exposes the currently composed Trusted Profile presentation, which is a host authority-composition boundary, although this command itself does not compose or mutate authority.
- **Persistence sensitivity:** Moderate; it reads the remembered trusted-profile preference and active profile state, while writes/restoration remain owned by existing paths.
- **Minimum visibility delta:** Handler -> `pub(super)`; type/fields/helpers/constants 0.
- **Registration:** Direct module path is feasible. The candidate loses because it is closer to a frozen authority-composition boundary than the selected model read/presentation query.

### 5. Repository membership query

- **Responsibility:** Return descriptive workspace membership presentation.
- **Range:** `main.rs:7438–7444`, 7 lines.
- **Handler:** `repository_membership` (one).
- **Request / response:** No request DTO; managed `State`; returns `WorkspaceRepositoryMembershipPresentation`.
- **Helpers / constants:** Delegates to retained `repository_membership_presentation`; no helper or constant needs to move.
- **Production callers:** Definition plus Tauri registration at `main.rs:9856`; lifecycle siblings and the `repository_membership` state module own adjacent membership transitions/state.
- **Tests:** No direct `repository_membership` handler call found. Tests call retained presentation/lifecycle helpers; no moved type, helper, or field if handler-only. No direct ignored/live handler call found.
- **Test-coupling counts:** Moved handler/type/helper/field references 0; retained helper/state testing remains broad; direct ignored/live handler references 0.
- **Authority sensitivity:** Moderate. Membership presentation is descriptive, but it sits directly beside admission, activation, removal, and active repository authority composition.
- **Persistence sensitivity:** Low; the handler reads process-local membership, not persisted state.
- **Minimum visibility delta:** Handler -> `pub(super)`; types/fields/helpers/constants 0.
- **Registration:** Direct module path is feasible. It loses on authority adjacency and less distinct presentation ownership; moving only the adapter beside process-local membership state would mix Tauri presentation with the existing membership state module.

### 6. Effective authority snapshot query — excluded / closed high-risk

- **Responsibility:** Return the composed authority snapshot used by desktop workflows.
- **Handler range:** `main.rs:2215–2221`, 7 lines. Delegated helper `effective_authority_snapshot_for_state` is `main.rs:2224–2495` (272 lines).
- **Handler / response:** `get_effective_authority_snapshot`; no request DTO; returns `EffectiveAuthoritySnapshot` and retained authority presentation types.
- **Production callers:** Tauri registration plus the retained helper/state graph. No candidate extraction is proposed.
- **Tests:** Fresh `main_tests.rs` search found 20 direct handler references across eight ignored/live scenario functions: `windows_live_desktop_hostexplicit_create_file`, `windows_live_desktop_hostexplicit_delete_file`, `windows_live_desktop_hostexplicit_rename_file`, `windows_live_desktop_explicit_host_tool_invocation`, `windows_live_desktop_hostexplicit_repo_patch`, `windows_live_desktop_hostexplicit_multi_file_edit`, `task_324_c_windows_host_driven_two_repository_live_certification`, and `task_335_windows_host_driven_remembered_workspace_live_certification`. The tests also inspect snapshot types/fields and invoke retained snapshot calculation.
- **Authority sensitivity:** High by purpose; it presents composed repository, profile, runtime, registry, and review state. The helper owns a large amount of authority-adjacent composition.
- **Persistence sensitivity:** Low direct persistence ownership; some live contexts include remembered workspace state, which does not reduce the authority coupling.
- **Minimum visibility delta:** Handler -> `pub(super)` for registration; moving the helper or response types would introduce broader coupling. Existing test use strongly disfavors moving it and it remains closed under the Task 396 scope.
- **Registration:** Direct module path is possible in isolation but does not make the authority/test boundary coherent. Rejected.

## Candidate comparison

| Candidate | Responsibility / production ownership | Tests and private coupling | Authority / persistence | Registration and minimum visibility | Decision |
|---|---|---|---|---|---|
| `model_configuration` | Isolated read-only model/provider presentation assembly | No handler call; one test uses retained six-field DTO; DTO and fields stay in root | Low authority; no persistence | Direct path; 1 handler `pub(super)`, 0 types/fields/helpers/constants | **Selected** |
| `conversation_transcript` | Isolated read-only transcript presentation, but body selects durable namespace | No handler call; ordinary/live tests exercise retained persistence state, not the command | Low authority; material namespace/persistence coupling in command sequence | Direct path; 1 handler `pub(super)`, no other widening | Rejected in favor of no persistence coupling |
| `commit_identity` | Small configured-flag query adjacent to identity mutation | No handler call; 18 test/helper functions touch retained identity state | Moderate Commit authorization adjacency; identity writes remain elsewhere | Direct path; handler-only `pub(super)` | Rejected because of Commit authorization adjacency |
| `trusted_profile_selection` | Coherent read query | No handler call; lower-level tests cover retained selection functions | Trusted Profile composition and remembered preference adjacency | Direct path; handler `pub(super)` | Rejected because of authority-composition adjacency |
| `repository_membership` | Coherent descriptive query over membership | No handler call; retained helper/state coverage | Membership is descriptive but adjacent to repository admission/activation authority | Direct path; handler `pub(super)` | Rejected because the Tauri presentation module boundary is less distinct |
| `get_effective_authority_snapshot` | Thin adapter over a large authority calculation | 20 handler uses across 8 ignored/live scenarios; snapshot fields used in tests | High authority sensitivity | Direct path technically possible, but test/authority coupling is high | Excluded / remains closed |

`model_configuration` is selected for the combination of a distinct read/presentation responsibility, no command-specific test dependency, no persistence or authority ownership, and a handler-only visibility change. The retained DTO test's six private-field accesses are evidence to leave that DTO exactly where it is, not to widen its fields. The selection does not optimize for the number of lines moved.

## Explicit authority ownership comparison

| Candidate | Repository authority / admission / membership | `ToolRegistry` / `HostExplicit` / review tickets / Commit authorization | Runtime execution / provider lifecycle / runtime lifecycle | Mutation / network authority | Persistence authority |
|---|---|---|---|---|---|
| `model_configuration` | None owned or composed; no repository read | None owned or composed | Reads configured model/readiness presentation only; no activation or lifecycle action | None | None |
| `conversation_transcript` | None owned or composed | None owned or composed | None | None | Reads persisted presentation after asking existing state to select the namespace; owns no namespace policy or write lifecycle, but keeps the durable-data selection sequence inside the query body |
| `commit_identity` | None | Reads the configured-presence input used by Commit authorization; does not authorize, issue a review ticket, or revoke authorization | None | None | No persistence write; adjacent setter owns preference write |
| `trusted_profile_selection` | None | Does not compose Tools or permissions; observes the currently active Trusted Profile presentation | Reads active profile state; does not activate providers or own provider/runtime lifecycle | None | Reads remembered profile preference; does not write it |
| `repository_membership` | Reads descriptive process-local membership; does not admit, activate, remove, or select a repository | Does not build registry, HostExplicit, review ticket, or Commit authority | None | None | None |
| `get_effective_authority_snapshot` | Observes current repository state and composition | Observes composed registry, HostExplicit eligibility, review state, and Commit applicability; does not itself grant or mutate them | Observes connection/provider/runtime status; does not own lifecycle | Reports existing capability classes; no mutation/network action | No persistence policy |

## IPC/schema comparison

All six handlers can be relocated behind direct module-qualified registration without changing their current Tauri command identity, argument names/order, `State` use, response type, serialization, or error shape. No candidate requires an IPC redesign. The selected candidate preserves `model_configuration` and `ModelConfigurationPresentation` byte-for-byte at the boundary; only the Rust path used in `generate_handler!` changes. The same registration-only feasibility holds mechanically for the rejected queries, but their ownership/test sensitivities make them poorer Task 397 boundaries.

## Production-caller audit and direct registration

The selected handler has no production caller outside its definition and the Tauri registration. Existing command registration at `main.rs:9841` is currently unqualified `model_configuration`. Task 397 may replace that entry with exactly:

```rust
model_configuration_commands::model_configuration
```

Keep the entry at the same relative position. Keep the command identifier `model_configuration`, argument `State<'_, DesktopAppState>`, argument order, returned `ModelConfigurationPresentation`, serialization, frontend contract, and error behavior unchanged. Registration remains one of 47 handlers. No wrapper, re-export, rename, or registration reordering is allowed.

## Exact frozen Task 397 boundary

- **Responsibility:** Desktop model configuration read/presentation command.
- **Target module:** `crates/rah-desktop/src/desktop_model_configuration_commands.rs`.
- **Source range:** `crates/rah-desktop/src/main.rs:4832–4868` inclusive.
- **Exact moved symbols:** Existing `#[cfg(target_os = "windows")]`, `#[tauri::command]`, and `model_configuration` function body/signature only.
- **Tauri handler:** `model_configuration` (one).
- **Request types:** None beyond existing managed `State<'_, DesktopAppState>`.
- **Response type:** `ModelConfigurationPresentation`, retained in `main.rs`.
- **Helpers moved:** None. Keep `model_configuration_status`, `ProviderEndpointPresentation::from`, and endpoint conversion behavior where they are.
- **Constants moved:** None.
- **Shared state moved:** None. Keep `DesktopAppState`, model/connection locks, and all state fields in `main.rs`.
- **Exact retained symbols:** `ModelConfigurationPresentation` and all six private fields; `ProviderEndpointPresentation`; `DesktopModelProvider`; `ProviderEndpoint`, `ProviderScheme`, `ReadinessState`, `ConnectionState`; `DesktopAppState`; `model_configuration_status`; all setters/mutators including `set_model_configuration`; all test code.
- **Production callers:** Tauri registration only; update that path as above.
- **Affected tests:** None. Keep `model_configuration_presentation_is_closed_and_sanitized`, `model_and_repository_connection_generations_are_independent`, and `preference_write_matrix_excludes_all_non_apply_reset_events` unchanged. The first retains six private DTO field accesses; the latter two retain helper coverage.
- **Private-field coupling:** Six private fields are directly constructed in one test for the retained DTO. Frozen moved-field count is zero.
- **Ignored/live coupling:** Zero direct handler, DTO, helper, or field references to the proposed moved symbols.
- **Authority classification:** Descriptive model/provider/readiness observation only. No repository authority, repository admission/membership, `ToolRegistry`, `HostExplicit`, review tickets, Commit authorization, runtime/provider activation or lifecycle, mutation authority, or network authority is moved or composed.
- **Persistence classification:** None. No persistence object, namespace selection, or persistence policy moves.
- **Expected registration-path change:** One entry changes from `model_configuration` to `model_configuration_commands::model_configuration` in place; registered command count remains 47.
- **Expected source movement:** Remove 37 lines from `main.rs`; add a 1-line module declaration and a new module containing the two attributes and handler (37 lines). Net `main.rs` change is approximately −36 physical lines after accounting for the module declaration. No tests move.

## Frozen visibility budget

Task 397 has this hard maximum:

- Handlers: **1** — `model_configuration -> pub(super)` for parent Tauri registration.
- Types: **0**.
- DTO fields: **0**.
- Helpers: **0**.
- Constants: **0**.
- Shared state: **0**.
- Test visibility accommodations: **0**.

If implementation requires anything beyond this budget, stop, roll back, and fail Task 397. Do not expand the budget interactively.

## IPC, authority, and dependency delta

Expected changes for the frozen extraction:

- **IPC/schema:** None. Command name, argument, argument order, `State` use, result type, serialization, frontend contract, and error shape remain the same.
- **Permission/capability:** None. Do not edit `build.rs`, `capabilities/default.json`, or `permissions/`.
- **Authority:** None. No authority ownership, composition, admission, lifecycle, dispatch, or mutation moves.
- **Persistence:** None.
- **Cargo/dependencies:** None.
- **Frontend:** None.

## Implementation non-goals

Task 397 must not move the DTO or its fields, helpers, state, tests, setters, endpoint logic, authority composition, or persistence behavior. It must not change permissions, capabilities, generated command metadata, IPC, frontend code, Cargo files, or any unrelated issue. It must not begin another extraction.

## Recommended next task

**Task 397 — Mechanical Extraction of the Model Configuration Query Command**

Task 397 must use exactly this source range, symbol set, module path, single registration-path delta, test inventory, authority and persistence constraints, and frozen visibility budget. Do not start Task 397 automatically.
