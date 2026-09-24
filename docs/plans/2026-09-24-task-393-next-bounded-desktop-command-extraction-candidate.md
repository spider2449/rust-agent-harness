# Task 393 — Next Bounded Desktop Command Extraction Candidate

## Verdict

**PASS — NEXT BOUNDED DESKTOP EXTRACTION CANDIDATE FROZEN**

Freeze the `app_status` Tauri command as the next mechanical extraction. Move only the command adapter into `desktop_status_commands.rs`. Keep the `AppStatus` response type, `DesktopAppState::status`, and all status composition helpers in `main.rs`.

This is a research and selection artifact. No Rust source was changed.

## Starting checkpoint

- Expected and observed `HEAD`: `1c06b7eb2105e61b5659f0208cedadf916678561` (`refactor: extract desktop preferences warning command`).
- `git status --short` was empty.
- `crates/rah-desktop/src/main.rs`: **9,919 physical lines**, measured from UTF-8 text with `splitlines()`. PowerShell's line measurement over `Get-Content` records reported 9,520; that was a measurement-method artifact, confirmed by both the tracked `HEAD` content and current file text.
- The current source includes `desktop_preferences_commands.rs` and its qualified registration from Task 391/392.
- `Remembered Workspace Catalog Commands` remains closed by Tasks 389/390 and was not reconsidered. The three named HostExplicit permission findings were not inspected for remediation and are outside this task.

## Method and source scope

Inspected the current production declarations, candidate command bodies, registration list, `main_tests.rs` symbol references, Task 388 decomposition research, Task 392 independent audit, and the architecture/security guardrails. Source ranges below are inclusive and refer to the verified starting `HEAD`.

The test inventory distinguishes references to a proposed moved symbol from tests that exercise a retained helper, retained DTO, or neighboring command. Test fixtures and production `DesktopAppState` use are recorded separately from private-field coupling to moved types.

## Candidate inventory

### 1. Application status query — selected

- **Responsibility:** Return the existing descriptive desktop application status.
- **Range:** `main.rs:4577–4581` (5 lines; command attributes and body).
- **Tauri handler:** `app_status` (1).
- **Request / response:** No request DTO. Returns retained `AppStatus`.
- **Private types / enums:** `AppStatus` and its nested presentation values remain in `main.rs`.
- **Helpers:** None moved or called directly by the command; it delegates to `DesktopAppState::status()`.
- **Shared state:** Managed `DesktopAppState` supplied as `State`; no field-level access in the handler.
- **External dependencies:** `tauri::State` and parent-module types only.
- **Tests:** No direct reference to `app_status`; no moved-type, moved-field, or moved-helper reference; no ignored/live reference. Three ordinary tests cover the retained status mapping: `status_contains_only_the_desktop_application_state`, `status_reflects_connection_transitions_without_exposing_runtime_details`, and `repository_status_is_dynamic_without_exposing_repository_details`. They call `current_app_status`, not the Tauri handler. Those tests also inspect `AppStatus` fields, which is why the response type stays in `main.rs`.
- **Authority sensitivity:** Low. The result describes current application, connection, profile, repository-tool, and model status. It does not compose, grant, or mutate authority.
- **Estimated visibility delta:** `app_status` becomes `pub(super)` for the parent registration. No type, field, helper, or constant visibility changes.

### 2. Commit identity status query

- **Responsibility:** Report whether a commit identity is configured.
- **Range:** `main.rs:4935–4949` (15 lines; `CommitIdentityPresentation` plus `commit_identity`).
- **Tauri handler:** `commit_identity` (1).
- **Request / response:** No request DTO. One private response DTO with one private `configured` field.
- **Private types / enums:** `CommitIdentityPresentation`; no enum.
- **Helpers:** None.
- **Shared state:** Reads `DesktopAppState.commit_identity` under its mutex.
- **External dependencies:** `serde::Serialize`, `tauri::State`, and parent-module state.
- **Tests:** No direct call to the proposed `commit_identity` handler and no `CommitIdentityPresentation` reference. No helper or moved-DTO-field reference. The shared state field is accessed by test support `authorize_test_commit` and six ordinary tests: `restored_identity_and_fresh_bound_review_serialize_authorize_presentation`, `binary_staged_review_is_not_authorizable_and_forged_selector_has_no_effect`, `authorize_then_refresh_revokes_pending_and_rotates_review_selector_without_commit`, `disconnect_revokes_pending_authorization_and_never_restores_old_review`, `desktop_successful_rename_revokes_reviewed_commit_authorization`, and `desktop_verified_directory_creation_revokes_reviewed_commit_authorization`. The state field remains in `main.rs`, so those accesses need no visibility change. No ignored/live test references the proposed handler or DTO; adjacent `set_commit_identity` use is not part of this candidate.
- **Authority sensitivity:** Moderate. The command reads only configuration presence, but the configured identity is an input to reviewed commit authorization.
- **Estimated visibility delta:** If moving the DTO with the handler, `commit_identity` and `CommitIdentityPresentation` need parent visibility; no DTO field needs widening. If only the handler moves, only the handler needs parent visibility. This is still mechanically viable, but it is more closely tied to commit authorization state than the selected status adapter.

### 3. Conversation transcript query

- **Responsibility:** Return the existing persisted conversation transcript presentation.
- **Range:** `main.rs:9816–9827` (12 lines; command attributes and body).
- **Tauri handler:** `conversation_transcript` (1).
- **Request / response:** No request DTO. Returns the imported `ConversationTranscriptPresentation` alias for the persistence module's presentation type.
- **Private types / enums:** No candidate-owned DTO or enum; `Persistence` and `ResumeError` remain imported in `main.rs`.
- **Helpers:** Calls `DesktopAppState::select_persistence_namespace()` and `Persistence::presentation()`; neither is proposed to move.
- **Shared state:** `DesktopAppState.persistence` and its repository-selected namespace.
- **External dependencies:** `conversation_persistence` presentation and persistence APIs.
- **Tests:** No direct handler, presentation-alias, private-field, or helper reference; no ignored/live reference. Tests share the desktop persistence/state fixture and cover persistence behavior through the persistence API, rather than this command.
- **Authority sensitivity:** Low for authority; it reads persisted conversation content and selects the current persistence namespace, so it has more persistent-data and shared-state coupling than the selected handler.
- **Estimated visibility delta:** `conversation_transcript` becomes `pub(super)` for registration. No type, field, helper, or constant visibility changes if the presentation alias remains in `main.rs`.

### 4. Model configuration query

- **Responsibility:** Build the current model/provider endpoint and readiness presentation.
- **Handler range:** `main.rs:4836–4872` (37 lines).
- **Related response DTO:** `ModelConfigurationPresentation` at `main.rs:1740–1748`; its fields remain outside the handler range.
- **Tauri handler:** `model_configuration` (1).
- **Request / response:** No request DTO; returns `ModelConfigurationPresentation`.
- **Private types / enums:** DTO uses `DesktopModelProvider`, `ProviderEndpointPresentation`, and `ReadinessState`.
- **Helpers:** `model_configuration_status`, plus `ProviderEndpointPresentation::from` and `ProviderEndpoint::insecure_transport`.
- **Shared state:** Locks model and connection state and reads model selection, generation, readiness, and connection generation.
- **External dependencies:** `tauri::State`, provider endpoint/configuration types, and parent-module state.
- **Tests:** No direct call to the handler. `model_configuration_presentation_is_closed_and_sanitized` constructs the retained DTO and reads its serialized shape; its test source initializes all 6 private DTO fields. Tests also call the retained `model_configuration_status` helper in `status_contains_only_the_desktop_application_state`, `model_and_repository_connection_generations_are_independent`, and `preference_write_matrix_excludes_all_non_apply_reset_events`. No ignored/live reference to the handler or DTO was found.
- **Authority sensitivity:** Low to moderate. It reports configured provider and readiness state, without activating a provider or granting authority.
- **Estimated visibility delta:** For handler-only extraction, `model_configuration` needs `pub(super)` and the DTO/helper remain in `main.rs`, with zero field changes. Moving the DTO would require widening its 6 fields for the sibling test, which is outside a narrow extraction budget. The handler also has more state and helper coupling than `app_status`.

### 5. Effective authority snapshot query

- **Responsibility:** Return the current authority snapshot used by desktop workflows.
- **Handler range:** `main.rs:2213–2219` (7 lines).
- **Tauri handler:** `get_effective_authority_snapshot` (1).
- **Request / response:** No request DTO; returns `EffectiveAuthoritySnapshot`.
- **Private types / enums:** Snapshot/status and composition types remain in `main.rs`.
- **Helpers:** Delegates to `effective_authority_snapshot_for_state` (`main.rs:2222–2495`, approximately 274 lines).
- **Shared state:** Reads repository, model, connection, profile, workflow, and generation state through the snapshot helper.
- **External dependencies:** Parent-module authority and repository presentation types.
- **Tests:** `main_tests.rs` has 20 direct handler references across 8 ignored/live scenarios: `windows_live_desktop_hostexplicit_create_file`, `windows_live_desktop_hostexplicit_delete_file`, `windows_live_desktop_hostexplicit_rename_file`, `windows_live_desktop_explicit_host_tool_invocation`, `windows_live_desktop_hostexplicit_repo_patch`, `windows_live_desktop_hostexplicit_multi_file_edit`, `task_324_c_windows_host_driven_two_repository_live_certification`, and `task_335_windows_host_driven_remembered_workspace_live_certification`. Tests also call the retained state helper directly. The response type and fields are inspected by the same authority-focused scenarios.
- **Authority sensitivity:** High. The handler is read-only, but it exposes the composed authority state used to gate and describe repository/runtime workflows.
- **Estimated visibility delta:** Parent registration needs `pub(super)`. Existing sibling tests call the handler directly, so preserving those calls without wrappers or test movement needs broader test visibility (`pub(crate)`) or another API change. Moving snapshot calculation would additionally move a large state-coupled helper. This is materially less bounded than the selected candidate.

## Candidate comparison

| Candidate | Coherence / production ownership | Test coupling | Visibility | Authority / IPC risk | Implementation size |
|---|---|---|---|---|---|
| `app_status` | One adapter delegates to the existing root-owned status builder; clear command boundary | No direct command use; three tests cover retained builder and DTO | One `pub(super)` handler; zero other changes | Descriptive status; existing command contract is unchanged | 5 source lines |
| `commit_identity` | One coherent query and a tiny response DTO | No moved-symbol references; tests touch the retained state field | One handler plus response type if moved; zero field widening | Reads a commit-authorization prerequisite | 15 lines with DTO |
| `conversation_transcript` | One coherent query, but selects persistence namespace and reads durable transcript | No direct command coupling; persistence tests use lower-level API | One `pub(super)` handler | Persisted user content and persistence namespace are involved | 12 source lines |
| `model_configuration` | Coherent query with more state assembly and conversion helpers | One test directly constructs/serializes the DTO and its six private fields | Handler-only move is narrow; moving the DTO requires six field visibility changes | Provider configuration/readiness presentation | 37 source lines plus retained DTO/helper dependencies |
| `get_effective_authority_snapshot` | Thin handler over a large authority-state calculation | 20 direct references in 8 ignored/live scenarios, plus helper use | Tests require broader visibility or a wrapper; helper boundary is large | Authority state is central to its purpose | 7-line handler; roughly 274-line helper |

`app_status` is selected for its combined ownership, low test coupling, and narrow visibility boundary. The comparison does not rank candidates by line count alone.

## Test-coupling audit for selected boundary

Proposed moved symbol: `app_status` only.

| Reference class | Count | Findings |
|---|---:|---|
| Handler references in `main_tests.rs` | 0 | No test directly calls `app_status`. |
| Type references to moved types | 0 | No type moves. `AppStatus` remains in `main.rs`. |
| Private-field accesses to moved types | 0 | No moved type or field. Status tests continue using the root-owned `AppStatus`. |
| Helper references to moved helpers | 0 | `DesktopAppState::status` and all status calculation functions remain in `main.rs`. |
| Ignored/live references | 0 | No ignored or live test calls the moved handler. |
| Shared test fixture dependencies | 0 handler-specific | No test fixture invokes this command. Existing status tests use the root helper and DTO. |

Affected tests: **none**. The retained tests `status_contains_only_the_desktop_application_state`, `status_reflects_connection_transitions_without_exposing_runtime_details`, and `repository_status_is_dynamic_without_exposing_repository_details` remain unchanged and continue testing `current_app_status` and `AppStatus`.

## Tauri registration audit

Current entries are unqualified in `tauri::generate_handler!`. Each candidate can use a direct module path at its existing list position; the test and ownership constraints differ.

| Candidate | Current registration | Direct module path | Visibility finding |
|---|---|---|---|
| Application status | `app_status` | `desktop_status_commands::app_status` | `pub(super)` is sufficient. |
| Commit identity status | `commit_identity` | `commit_identity_commands::commit_identity` | `pub(super)` is sufficient if DTO is retained or also visible to parent. |
| Conversation transcript | `conversation_transcript` | `conversation_transcript_commands::conversation_transcript` | `pub(super)` is sufficient for registration; tests do not call it. |
| Model configuration | `model_configuration` | `model_configuration_commands::model_configuration` | `pub(super)` is sufficient for handler-only move; DTO stays in root for its test. |
| Effective authority snapshot | `get_effective_authority_snapshot` | `effective_authority_commands::get_effective_authority_snapshot` | Parent needs `pub(super)`; sibling tests directly call it, so preserving them requires broader visibility or an impermissible wrapper. |

For the selected candidate, handler count is 1. No wrapper, re-export, command rename, or registration reordering is needed. Keep the command name, state argument, and serialized `AppStatus` response unchanged.

## Authority ownership audit

The moved function only calls `DesktopAppState::status()` and returns its existing descriptive `AppStatus`. The status builder, authority composition, provider/runtime state, and repository state stay in `main.rs` and retain their present ownership.

The moved command does not own or modify repository authority, repository admission or identity validation, active membership, `ToolRegistry`, HostExplicit dispatch, review-ticket authorization, commit authorization, runtime/provider lifecycle, mutation authority, network authority, or persistent authority state. It adds no permission or capability behavior.

## Frozen extraction contract for Task 394

- **Responsibility:** Desktop application status query adapter.
- **Target module:** `crates/rah-desktop/src/desktop_status_commands.rs`.
- **Exact source range:** `crates/rah-desktop/src/main.rs:4577–4581` inclusive.
- **Exact moved symbol:** `app_status` and its existing `#[tauri::command]` attribute and body.
- **Exact Tauri handler:** `app_status`.
- **Request type:** None beyond the existing Tauri-managed `State<'_, DesktopAppState>` argument.
- **Response type:** `AppStatus`, retained in `main.rs`.
- **Helpers moved:** None.
- **Shared-state dependencies:** `DesktopAppState::status()` only; no field is moved or newly exposed.
- **Retained symbols:** `AppStatus`, its fields and serialization attributes, `DesktopAppState`, `DesktopAppState::status`, all status construction/mapping helpers, and all tests.
- **Affected tests:** None. Preserve the three listed status-mapping tests in place.
- **Ignored/live test coupling:** None.
- **Authority classification:** Descriptive application status query; no authority ownership or state mutation.
- **Visibility budget:**
  - handlers: **1 `pub(super)`** (`app_status`)
  - types: **0**
  - fields: **0**
  - helpers: **0**
  - constants: **0**
- **Expected registration change:** Replace `app_status` with `desktop_status_commands::app_status` at the same list position.
- **Expected line-count movement:** Remove 5 lines from `main.rs`; add a two-line Windows-gated module declaration and a 6-line module file. The registration stays one line. Expected `main.rs`: 9,916 lines (net −3, including the two-line Windows-gated module declaration); new module: 6 lines. The command registration remains one line.
- **Expected IPC delta:** None. Keep the Tauri command name `app_status`, the managed state argument, and the serialized response shape unchanged.
- **Expected permission/capability delta:** None.
- **Expected Cargo/dependency delta:** None.

## Rejected or deferred candidates

- **Remembered Workspace Catalog Commands:** Explicitly closed in Tasks 389/390 for sibling-test and private DTO coupling. Not reopened.
- **Commit identity status:** A viable small alternative, but it reports a value directly used by commit authorization. Keep it separate from the frozen status adapter rather than broadening this task.
- **Conversation transcript:** A viable small alternative, but its command selects a persistence namespace and reads durable conversation content.
- **Model configuration:** Handler-only extraction is possible, but it assembles a larger multi-field response from model and connection state; moving its DTO would require exposing six private fields to satisfy a sibling test.
- **Effective authority snapshot:** Rejected for this mechanical extraction because of authority-state sensitivity, the large calculation helper, and direct ignored/live sibling-test calls requiring broader visibility or wrappers.

## Implementation non-goals

Task 394 must not move or redesign any status DTO, state field, status builder, or authority calculation. Do not edit tests, frontend files, permissions/capabilities, Cargo files, or dependency declarations. Do not repair the three unrelated HostExplicit permission findings. Do not alter IPC names or payloads. Do not exceed the frozen visibility budget. If body-equivalent relocation cannot fit that exact boundary and budget, stop, roll back, and fail rather than expanding scope.

## Recommended next task

**Task 394 — Mechanical Extraction of the Desktop Application Status Command.** Use only this frozen range, symbol, target module, test inventory, and visibility budget. Do not start Task 394 automatically.

## Research validation

- Rust production source changed: no.
- Validation run: `git diff --check` only, after adding this document.
- Cargo checks, tests, clippy, workspace validation, and Windows live certification: not run, as required for this docs/research task.
- Push/tag: none.
