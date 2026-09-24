# Task 391 — Bounded Desktop Preferences Command Extraction

## Checkpoint and scope

- Starting checkpoint: `bc3862ca16e5d5da011195c9570c48512453f49a`
- Starting `git status --short`: clean
- Starting `crates/rah-desktop/src/main.rs` line count: 9,931
- Previous Remembered Workspace extraction candidate: closed and not retried
- Task 390 was research-only and was not pushed
- No commit, push, or tag was created

## Phase A precheck

### Candidate envelope and selection

Task 388 mapped `main.rs:4577–5208` as an envelope for app status, Trusted
Profile/model/commit preferences, and endpoint readiness. It is not one safe
mechanical unit. The live source was audited at `main.rs:4577–5206` before any
Rust edits. The 6,828–7,103 Remembered Workspace range discussed in Task 390
was excluded.

The broad envelope contains these exact production symbols:

- Commands: `app_status`, `trusted_profile_selection`,
  `choose_trusted_profile`, `restore_trusted_profile`,
  `forget_trusted_profile`, `clear_trusted_profile`, `model_configuration`,
  `desktop_preferences_warning`, `set_model_configuration`,
  `commit_identity`, `set_commit_identity`, `reset_model_preferences`, and
  `test_llama_cpp_endpoint`.
- Private helpers: `trusted_profile_selection_allowed`,
  `trusted_profile_forget_allowed`, `ensure_trusted_profile_selection_allowed`,
  `ensure_trusted_profile_forget_allowed`, `profile_selection_error`,
  `publish_trusted_profile_selection`, `clear_trusted_profile_selection`,
  `save_trusted_profile_preference`, `restore_trusted_profile_selection`,
  `forget_trusted_profile_preference`, `apply_model_selection`,
  `readiness_transport_error`, and `publish_readiness_result`.
- Probe type and method: `LlamaCppReadinessProbe` and
  `LlamaCppReadinessProbe::check`.
- Private presentation type: `CommitIdentityPresentation`.
- Request/response or state types defined outside the envelope and used by
  these commands include `DesktopAppState`, `AppStatus`,
  `TrustedProfilePresentation`, `DesktopModelProvider`,
  `ProviderEndpointInput`, `ModelConfigurationPresentation`,
  `ProviderEndpointPresentation`, `CommitIdentityPresentation`,
  `DesktopCommitIdentity`, `FrontendError`, `ReadinessState`, and
  `PreferencesWarning`.
- No constants are defined in the inspected command range. Referenced shared
  constants include `READINESS_CONNECT_TIMEOUT`, `READINESS_TOTAL_TIMEOUT`,
  and `READINESS_BODY_LIMIT`.

The broad group was not selected. Trusted Profile selection and restore
interact with active selection/currentness; model configuration changes active
desired provider state and generations; commit identity changes revoke pending
commit authority; readiness performs bounded network observation and publishes
generation-bound active state. Those concerns and their test helpers are not
one low-coupling descriptive-preference unit.

The frozen unit is only `desktop_preferences_warning`, original
`main.rs:4872–4885` inclusive, including its Windows and Tauri attributes. It
consumes a warning from `DesktopAppState.preferences` and maps the existing
`PreferencesWarning` variants to existing string values. It has no request
DTO, response DTO, helper, or constant of its own.

### Tauri handler and registration inventory

The selected command is `desktop_preferences_warning`. Before the move,
`generate_handler!` registered the bare identifier at `main.rs:9859`. The
frozen registration is
`desktop_preferences_commands::desktop_preferences_warning`. Tauri accepts
the module-qualified handler path, requiring only that the function be visible
to the parent module.

The broad envelope's 13 command names are listed above. Only
`desktop_preferences_warning` moves; every other handler stays in `main.rs`.

### Test-coupling audit

For the selected production symbol, `crates/rah-desktop/src/main_tests.rs`
search results are:

| Category | Count | Exact test names |
| --- | ---: | --- |
| A. Tests referencing the candidate handler | 0 | None |
| B. Tests referencing a candidate request/response type | 0 | None; no DTO moves |
| C. Tests directly accessing fields of a moved private type | 0 | None; no type or field moves |
| D. Ignored/live/certification tests referencing the candidate | 0 | None |

`DesktopAppState` remains in `main.rs`; it is an ancestor-owned shared state
dependency, not a moved candidate type. `PreferencesWarning` also remains in
`main.rs`'s existing import/use graph. The adjacent test
`preference_and_conversation_warning_domains_do_not_cross` checks the warning
string mapping independently, but does not call or name
`desktop_preferences_warning`, access a moved field, or require relocation.
It remains unchanged.

The rejected broad group had direct coupling to model and profile helpers in
tests including `endpoint_normalization_controls_generation_and_llama_only_closure`,
`stale_readiness_result_cannot_overwrite_a_new_model_generation`,
`model_selection_changes_generation_only_when_effective_selection_changes`,
`model_selection_is_rejected_while_chat_is_running`,
`apply_and_reset_save_failures_do_not_roll_back_current_desired_state`,
`reset_is_idle_only_and_changes_no_conversation_state`,
`clear_selection_is_process_local_and_generation_is_exact`,
`restore_rereads_current_source_without_persisting_or_spawning`,
`forget_removes_only_preference_and_save_failure_is_non_destructive`,
`preference_write_matrix_excludes_all_non_apply_reset_events`,
`non_loopback_apply_changes_current_state_without_preference_filesystem_activity`,
and `reset_and_restore_keep_preference_and_conversation_persistence_separate`.
Commit identity helpers/state also occur in multiple commit and live HostExplicit
tests, reinforcing exclusion of that part of the envelope. No tests were moved
or edited.

### Shared-state and authority inventory

The selected handler reads only `DesktopAppState.preferences`, locks that
existing preference store, calls `Preferences::take_warning`, and maps
`PreferencesWarning::{RestoreFailed, SaveFailed}` to the unchanged strings
`preferences_restore_failed` and `preferences_save_failed`. It uses no
`AppHandle`, repository state, `ToolRegistry`, `HostExplicit`, review ticket,
commit authorization, runtime execution, or provider lifecycle state.

The value read is a stored preference warning. It does not configure or activate
a provider. Model selection and endpoint configuration elsewhere in the broad
envelope remain active desired runtime configuration owned by the existing
host lifecycle; those functions and owners stay in `main.rs`. Repository
authority, admission, identity validation, membership, ToolRegistry creation,
commit authorization, and runtime/provider lifecycle ownership do not move.

The change does not alter permissions, capability files, permission generation,
frontend files, IPC names/arguments/serialized fields/error behavior, Cargo
files, dependencies, or `HostExplicit` count. The known findings for
`host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and
`host_prepare_repo_delete_file` remain out of scope and unchanged.

### Frozen visibility budget and verdict

```text
Visibility budget:
- 1 Tauri handler: pub(super)
- request/response/type visibility changes: 0
- field visibility changes: 0
- helper visibility changes: 0
```

PRECHECK PASS — COHERENT EXTRACTION BOUNDARY FROZEN

## Phase B relocation

```text
Target module: crates/rah-desktop/src/desktop_preferences_commands.rs
Exact moved symbols: desktop_preferences_warning
Exact retained symbols: all other symbols in main.rs:4577–5208, including
  PreferencesWarning, DesktopAppState, emit_preferences_warning, and every
  model, commit identity, Trusted Profile, and readiness command/helper
Exact visibility budget: 1 handler pub(super); zero type, field, or helper
  visibility changes
Exact affected tests expected unchanged: none reference the moved handler or
  a moved private type; all rah-desktop tests remain unchanged
```

The implementation consists of a cfg-gated module declaration, the new module
file, the module-qualified `generate_handler!` entry, removal of the original
function body, and the one budgeted `pub(super)` on the relocated handler.

| Original symbol | Old `main.rs` range | New module range | Body-equivalent | Permitted differences |
| --- | --- | --- | --- | --- |
| `desktop_preferences_warning` | 4872–4885 | 4–17 | Yes | Module location, imports, and `pub(super)` required for parent Tauri registration |

The function signature remains
`fn desktop_preferences_warning(State<DesktopAppState>) -> Option<&'static str>`;
the Tauri command name, argument names, response shape, mapping strings, and
behavior are unchanged. No production semantic rewrite was made. No IPC,
authority, permission, capability, dependency, Cargo, or frontend changes were
made.

Actual visibility delta: one function changed from private to `pub(super)`;
zero types, fields, or helpers changed visibility. The budget was not exceeded.

## Validation and final state

- `cargo fmt --check`: passed
- `cargo test -p rah-desktop -- --test-threads=1`: passed; 315 passed, 18
  ignored, 0 failed; 661.18 seconds
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings`:
  passed
- `git diff --check`: passed
- Workspace validation: not run; the task requires focused package validation
  and forbids redundant workspace runs by default.
- Windows live certification: not run; it was outside the authorized focused
  validation.
- `main.rs` line count: 9,931 before; 9,919 after
- `desktop_preferences_commands.rs` line count: 17
- Exact changed files:
  - `crates/rah-desktop/src/main.rs`
  - `crates/rah-desktop/src/desktop_preferences_commands.rs`
  - `docs/plans/2026-09-24-task-391-bounded-desktop-preferences-command-extraction.md`
- Final `git status --short`:
  - ` M crates/rah-desktop/src/main.rs`
  - `?? crates/rah-desktop/src/desktop_preferences_commands.rs`
  - `?? docs/plans/2026-09-24-task-391-bounded-desktop-preferences-command-extraction.md`
- Commit created: no
- Push/tag: none

PASS — BOUNDED DESKTOP PREFERENCES EXTRACTION COMPLETE

Next task: Task 392 — Desktop Preferences Command Extraction Independent
Audit. Do not start it automatically.
