# Task 398 — Model Configuration Query Command Extraction Independent Audit

## Checkpoint and scope

- **Starting HEAD:** `e9bd34f93724eaaa704d856033a8d93e1dbb6e75` (`docs: select next bounded desktop production extraction`).
- **Initial dirty files:** `crates/rah-desktop/src/main.rs` (modified); `crates/rah-desktop/src/desktop_model_configuration_commands.rs` (untracked); `docs/plans/2026-09-24-task-397-model-configuration-query-command-extraction.md` (untracked). No unrelated dirty path was present.
- **Exact implementation delta:** added the Windows-gated `desktop_model_configuration_commands` module declaration; removed the original `model_configuration` command; replaced its Tauri registration in place with `desktop_model_configuration_commands::model_configuration`; added a child module containing only imports and the moved command. No neighboring production edits, logic cleanup, comment churn, renaming, DTO changes, helper/status changes, registration reorder, or unrelated formatting churn were found.
- **Task 396 naming inconsistency:** **CONFIRMED**. The committed Task 396 artifact names `desktop_model_configuration_commands.rs` as the target, but gives `model_configuration_commands::model_configuration` in the registration examples at lines 42, 149, and 173.
- **Canonical module resolution:** Task 397 consistently uses `desktop_model_configuration_commands.rs`, declares `mod desktop_model_configuration_commands;`, and registers `desktop_model_configuration_commands::model_configuration`. The Task 396 inconsistency did not alter the frozen responsibility, symbol, or visibility boundary.

## Handler and DTO audit

- **Responsibility:** Desktop model configuration read/readiness presentation query.
- **Original handler region in `HEAD`:** `crates/rah-desktop/src/main.rs:4832–4868` inclusive. The function declaration begins at line 4834.
- **Relocated handler:** `crates/rah-desktop/src/desktop_model_configuration_commands.rs:8–45` inclusive.
- **Body equivalence:** **PASS**. The `#[cfg(target_os = "windows")]` and `#[tauri::command]` attributes, name, synchronous status, `State<'_, DesktopAppState>` argument, return type, locking, model selection/generation/readiness reads, endpoint conversion and insecure-transport mapping, connected-generation match, status helper call, branch structure, and returned fields are unchanged. Differences are relocation/import qualification, `pub(super)` for parent registration, and rustfmt layout including the multiline one-argument signature's trailing comma.
- **Retained DTO:** `ModelConfigurationPresentation` remains defined in `main.rs`; DTO moved: **NO**; DTO visibility widened: **NO**.
- **Six private fields:** `provider: DesktopModelProvider`, `model: Option<String>`, `endpoint: Option<ProviderEndpointPresentation>`, `insecure_transport: bool`, `readiness: ReadinessState`, and `status: &'static str`. None has a `pub` visibility modifier; private fields widened: **0**.
- **Module boundary:** the new module contains only the six required parent imports, `tauri::State`, and `model_configuration`. It contains no DTO definition/re-export, wrapper, helper, abstraction, constant, setter, lifecycle logic, provider/runtime ownership, or tests. It observes existing parent-owned in-memory state and does not acquire model/runtime lifecycle ownership.
- **Frozen visibility budget:** maximum `model_configuration -> pub(super)`; types 0; fields 0; helpers 0; constants 0; shared state 0; tests 0.
- **Actual visibility delta:** exactly `model_configuration -> pub(super)`. There is no `pub(crate)`, `pub`, or other `pub(super)` addition. The DTO, endpoint presentation types, app state, status/readiness helpers, model/provider state, setters, constants, and other handlers retain their existing visibility.

## Registration and test coupling

- **Tauri registration:** exactly one entry changed from `model_configuration` to `desktop_model_configuration_commands::model_configuration`, at the same position. There is no wrapper, re-export, rename, duplicate, or reorder.
- **Registered handler count:** **47 at `HEAD`; 47 current**. The list was counted through its closing `])`, including its final entry without a trailing comma.
- **Test coupling counts:** direct test calls to the moved handler: **0**; affected/moved tests: **0**; direct references to the moved module/symbol in `main_tests.rs`: **0**; ignored/live moved-symbol references: **0**. Tests do refer to the retained DTO and status helper.
- `model_configuration_presentation_is_closed_and_sanitized`: constructs the parent-owned DTO using all six private fields, asserts its serialized JSON field names/values, and checks that sensitive provider/credential-related names are absent. It proves DTO serialization/presentation closure and sanitization; it does not invoke the relocated Tauri handler.
- `model_and_repository_connection_generations_are_independent`: exercises the model status mapping for absent, equal, stale, and updated generations alongside repository authority generation cases. It proves the retained status helper behavior and separation; it does not invoke the relocated Tauri handler.
- `preference_write_matrix_excludes_all_non_apply_reset_events`: exercises presentation/readiness/status and other state events, then asserts no preference write accounting occurred. It proves those tested non-apply/reset operations do not write preferences; it does not invoke the relocated Tauri handler or prove every persistence path.

## Contract and boundary audit

- **IPC/schema delta:** **NONE**. Tauri command identity remains `model_configuration`; the sole argument remains `state: State<'_, DesktopAppState>` in the same order; return type remains `ModelConfigurationPresentation`; serialization derives, camelCase names, optional endpoint omission, field meanings, and error/result shape remain unchanged. Frontend calls continue to invoke `model_configuration` by its command name.
- **Production semantic delta:** **NONE**. The implementation delta is a mechanical module relocation and its registration path. The body still reads current model and connection state and returns the same readiness/configuration presentation.
- **Authority delta:** **NONE**. The command does not compose repository authority/admission/membership, ToolRegistry, HostExplicit, review tickets, commit authorization, runtime execution authority, provider/runtime/model-activation lifecycle, mutation authority, or network authority.
- **Persistence delta:** **NONE**. The command selects no durable namespace and does not write preferences/conversation state, restore/forget state, or own persistence lifecycle.
- **Permission/capability delta:** **NONE**. No build script, capability, or permission file changed. The known missing coverage for `host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and `host_prepare_repo_delete_file` was not changed.
- **Cargo/dependency delta:** **NONE**. No manifests, lockfile, workspace/crate dependencies, or features changed.
- **Frontend delta:** **NONE**. No frontend file is dirty.

## Source size

- `main.rs` at `HEAD`: **9,915 lines**.
- Current `main.rs`: **9,879 lines**.
- New module: **45 lines**.
- `main.rs` net change: **−36 lines**. Its diff has 39 deletions and 3 insertions: the 37-line original handler region plus its two adjacent blank lines are removed; two lines add the Windows-gated module declaration; the one-for-one registration-line replacement contributes one deletion and one insertion. The new 45-line file consists of its imports/spacing and the relocated command with rustfmt's multiline signature. Thus the measured total follows the actual diff rather than a one-line module-declaration estimate.

## Validation and decision

- `cargo fmt --check` — **PASS**.
- `cargo test -p rah-desktop -- --test-threads=1` — **PASS**; 315 passed, 18 ignored, 0 failed, 333 discovered; finished in 697.07 seconds. The ignored entries require explicit host/live certification gates.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` — **PASS**.
- `git diff --check` — **PASS** before artifact creation; rerun after artifact creation and before commit.
- **Workspace validation:** not run. The production change is confined to the Desktop crate's private child module and Tauri registration; package formatting, tests, and Clippy passed, and no shared API or cross-crate uncertainty was found.
- **Windows live certification:** not run. This is a synchronous read/presentation command relocation with no provider/runtime behavior change; gated live tests were not enabled, and no live certification claim is needed for the audited extraction.
- **Exact intended commit files:** `crates/rah-desktop/src/main.rs`; `crates/rah-desktop/src/desktop_model_configuration_commands.rs`; `docs/plans/2026-09-24-task-397-model-configuration-query-command-extraction.md`; `docs/plans/2026-09-24-task-398-model-configuration-query-command-extraction-independent-audit.md`.
- **Commit decision:** PASS; commit those four files as `refactor: extract desktop model configuration command`. No push or tag.
- **Next task:** Task 399 — Reassess Remaining Desktop Production Decomposition Boundary. Research-only; do not start automatically.

**PASS — MODEL CONFIGURATION QUERY EXTRACTION INDEPENDENTLY VERIFIED**
