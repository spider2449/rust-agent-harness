# Task 395 — Desktop Application Status Command Extraction Independent Audit

## Verdict

**PASS — DESKTOP APPLICATION STATUS EXTRACTION INDEPENDENTLY VERIFIED**

## Starting checkpoint and scope

- Starting `HEAD`: `dbb343b1019e2d483f5d77d0a258b9fc597ea5dc` (`docs: select next bounded desktop extraction candidate`).
- Initial `git status --short` matched the required checkpoint exactly:
  - `M crates/rah-desktop/src/main.rs`
  - `?? crates/rah-desktop/src/desktop_status_commands.rs`
  - `?? docs/plans/2026-09-24-task-394-desktop-application-status-command-extraction.md`
- No unrelated dirty files were present.
- The production diff is limited to the Windows-gated `desktop_status_commands` module declaration, removal and relocation of `app_status`, and its qualified registration at the same position. No neighboring edits, format churn, renaming, comment changes, helper extraction, unrelated imports, or registration reordering were found.
- Exact implementation changes in `main.rs`: add `#[cfg(target_os = "windows")] mod desktop_status_commands;`; remove original `app_status`; replace the registration entry `app_status` with `desktop_status_commands::app_status`.

## Handler equivalence

- Original handler in `HEAD`: `crates/rah-desktop/src/main.rs:4577–4581` inclusive.
- Relocated handler: `crates/rah-desktop/src/desktop_status_commands.rs:4–8` inclusive; function at line 6.
- Original attributes: `#[cfg(target_os = "windows")]` and `#[tauri::command]`.
- Original signature: `fn app_status(state: State<'_, DesktopAppState>) -> AppStatus`.
- Relocated signature: `pub(super) fn app_status(state: State<'_, DesktopAppState>) -> AppStatus`.
- The body in both locations is exactly `state.status()`. It remains synchronous, takes the same single Tauri `State` argument, returns the same `AppStatus`, and retains the same attributes and control flow. It has no additional locking, mapping, or error behavior.

**BODY EQUIVALENCE: PASS**

## Visibility and module boundary

- Frozen visibility budget: one handler (`app_status`) to `pub(super)`; zero types, fields, helpers, or constants.
- Actual visibility delta: exactly `app_status` to `pub(super)`. No `pub(crate)` or `pub`, extra `pub(super)`, field or DTO widening, helper widening, or other handler visibility change was found.
- The new module contains only imports required by the handler (`AppStatus`, `DesktopAppState`, and `tauri::State`) and the attributed handler. It adds no helper, wrapper, re-export, alias, DTO, abstraction, constant, or unrelated command.
- Status construction and runtime state remain owned by `DesktopAppState` and `AppStatus` in `main.rs`; the extracted command only delegates to `state.status()`.

## Tauri registration and test coupling

- Registration changed in place from `app_status` to `desktop_status_commands::app_status` at `main.rs:9835`.
- Independent parse of the full `generate_handler!` list found 47 entries at `HEAD` and 47 currently. The moved command is first in both lists, appears exactly once, and the last entry remains `host_cancel_tool_invocation`.
- In `main_tests.rs`, direct `app_status` handler references: 0; `desktop_status_commands` references: 0; `AppStatus` references: 0; explicit `DesktopAppState::status` references: 0. Three existing `state.status()` calls remain at lines 17693, 17922, and 20081; they exercise status state/presentation behavior, not the Tauri handler. No tests changed. No new test-access requirement or moved-field coupling was introduced.

The three retained status tests were unchanged:

- `status_contains_only_the_desktop_application_state` checks the desktop-level status values for a disconnected, repository-unselected state and checks serialization for path separators. It does not exercise the Tauri command or establish a complete privacy proof for every status state.
- `status_reflects_connection_transitions_without_exposing_runtime_details` checks connecting and error mappings and absence of runtime version details in those cases. It does not invoke the relocated command or exercise every connection state.
- `repository_status_is_dynamic_without_exposing_repository_details` checks selected repository status and rejects a serialized `git.exe` detail. It does not test repository authority or command registration.

## IPC, schema, and semantic impact

- Command identity remains `app_status`; frontend calls remain `invoke("app_status")` with no parameters (at `frontend/status.js:2058` and `:2213`).
- Parameter count/order, Tauri `State` usage, return type, `AppStatus` serialization and field names, and error behavior are unchanged. The handler still returns `AppStatus` directly without a new fallible path.
- No frontend file changed.

**IPC/SCHEMA DELTA: NONE**

Production behavior changes only in the Rust module path used to resolve the registered command. The command name and behavior are preserved.

**PRODUCTION SEMANTIC DELTA: NONE**

## Authority, permissions, and dependencies

- The moved handler remains a descriptive read-only desktop application-status query. It reads the existing `DesktopAppState::status` presentation and performs no repository selection/admission/identity/membership operation, tool registry operation, `HostExplicit` action, review or commit authorization, runtime/provider execution or lifecycle operation, mutation, network action, or persistent authority-state update.

**AUTHORITY DELTA: NONE**

- No change to `build.rs`, `capabilities/default.json`, or `permissions/` was present. The known missing permission coverage for `host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and `host_prepare_repo_delete_file` remains outside this task and was not changed.

**PERMISSION/CAPABILITY DELTA: NONE**

- No `Cargo.toml`, `Cargo.lock`, workspace dependency, crate dependency, or feature file changed.
- No frontend file changed.

**CARGO/DEPENDENCY DELTA: NONE**

**FRONTEND DELTA: NONE**

## Line counts

- `main.rs` at `HEAD`: 9,919 physical lines.
- Current `main.rs`: 9,915 physical lines (net −4, from relocating a five-line handler and its surrounding blank lines while adding the two-line module declaration; the registration replacement is line-for-line).
- `desktop_status_commands.rs`: 8 physical lines.

## Focused validation

- `cargo fmt --check`: **PASS**.
- `cargo test -p rah-desktop -- --test-threads=1`: **PASS**, 315 passed, 18 ignored, 0 failed, 333 discovered; test harness reported 679.70 seconds. The ignored tests are host/live-gated and were not run.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings`: **PASS**.
- `git diff --check`: **PASS** before staging; the final staged diff was also checked before commit.
- Workspace-wide check/test/clippy were not run. The audited change is confined to the Windows-gated desktop crate, adds no dependency or shared API edge, and focused crate validation passed, so no cross-crate uncertainty required escalation.
- Windows live certification was not run. This task audits a mechanical command relocation, and no live certification was requested; host/live-gated tests remained ignored under the command used.

## Changed files and commit

The complete intended commit contained exactly:

- `crates/rah-desktop/src/main.rs`
- `crates/rah-desktop/src/desktop_status_commands.rs`
- `docs/plans/2026-09-24-task-394-desktop-application-status-command-extraction.md`
- `docs/plans/2026-09-24-task-395-desktop-application-status-command-extraction-independent-audit.md`

Commit: `refactor: extract desktop application status command` (recorded after the audit artifact was created and the exact staged scope and diff check passed).

## Completion

- Final worktree: clean.
- Push/tag: none.
- Commit decision: commit the audited Task 394 extraction with this independent audit record, as required by the passing-audit policy.
- Recommended next task: **Task 396 — Select and Freeze the Next Bounded Desktop Production Extraction Candidate**. It should be research-only and reassess the remaining production tree from the new checkpoint; do not start it automatically.
