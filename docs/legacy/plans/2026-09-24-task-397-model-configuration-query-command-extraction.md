# Task 397 — Model Configuration Query Command Extraction

## Checkpoint and frozen boundary

- **Starting HEAD:** `e9bd34f93724eaaa704d856033a8d93e1dbb6e75` (`docs: select next bounded desktop production extraction`).
- **Starting worktree:** clean; `git status --short` produced no entries.
- **Task 396 responsibility:** Desktop model configuration read/presentation command.
- **Original source range:** `crates/rah-desktop/src/main.rs:4832–4868` inclusive.
- **Moved symbol:** `model_configuration`, including its Windows cfg and Tauri command attributes, signature, and body.
- **Canonical target:** `crates/rah-desktop/src/desktop_model_configuration_commands.rs`; module identifier `desktop_model_configuration_commands`.
- **Task 396 naming consistency:** The committed artifact's target filename specifies `desktop_model_configuration_commands.rs`, while its registration examples at lines 42, 149, and 173 specify `model_configuration_commands::model_configuration`. This is a naming inconsistency only. The filename-consistent path `desktop_model_configuration_commands::model_configuration` is used here; no frozen boundary was changed.

## Extraction result

- Added the private Windows module declaration `mod desktop_model_configuration_commands;` in the existing module declaration block.
- Relocated only `model_configuration`. The relocated handler occupies lines 8–45 of the new module; its exact line span includes rustfmt's multiline signature.
- **Imports in new module:** retained parent-owned state, DTO, endpoint, connection, and status symbols, plus `tauri::State`.
- **Retained in `main.rs`:** `ModelConfigurationPresentation` and all six private fields (`provider`, `model`, `endpoint`, `insecure_transport`, `readiness`, `status`); `ProviderEndpointPresentation`; model/provider/endpoint/readiness/connection types; `DesktopAppState`; `model_configuration_status`; endpoint conversion; model setters; lifecycle logic; and all tests.
- **Tauri registration:** `model_configuration` → `desktop_model_configuration_commands::model_configuration`, at the same registration position. It is registered exactly once; total registered handlers: **47**.
- No wrapper, re-export, rename, or registration reorder was added.
- **Visibility budget:** maximum one handler `pub(super)`; zero changes to types, fields, helpers, constants, shared state, and tests.
- **Actual visibility delta:** exactly one handler, `model_configuration -> pub(super)`; all other visibility remains unchanged.
- **Test coupling:** zero tests directly call the moved handler; **0 tests affected or moved**. The unchanged `model_configuration_presentation_is_closed_and_sanitized` test constructs the retained DTO through all **six** private fields. No field visibility was widened. The other named retained tests remain unchanged.
- **Ignored/live coupling:** zero direct references to the moved handler or moved symbols in ignored/live test paths.
- **Body equivalence:** **PASS**. Compared the starting-HEAD handler with the relocated handler. Attributes and handler tokens match after removing `pub(super)` and normalizing rustfmt whitespace/its trailing comma on the multiline one-argument signature. Mapping, locking, readiness, endpoint sanitization, status matching, and return shape are unchanged.

## Boundary analysis

- **Production semantic delta:** mechanical module relocation and direct registration path only. The handler still reads model selection/generation/readiness and connection generation, maps the same presentation fields, and computes status with the same helper.
- **IPC/schema delta:** **NONE**. Command name, argument and order, Tauri `State`, returned `ModelConfigurationPresentation`, serialization/field names and meanings, error shape, and frontend invocation contract are unchanged.
- **Authority delta:** **NONE**. This remains a read/presentation endpoint. No repository admission or membership, registry, HostExplicit, review ticket, Commit authorization, runtime/provider lifecycle, model activation, mutation, or network authority moved or was added.
- **Persistence delta:** **NONE**. The handler only reads in-memory model and connection state. It does not select a durable namespace, read or write persisted state, restore/forget state, or own storage lifecycle.
- **Permission/capability delta:** **NONE**. No build, capability, or permission files changed; the unrelated permission findings were untouched.
- **Cargo/dependency delta:** **NONE**. No manifest, lockfile, feature, workspace dependency, or crate dependency changed.
- **Frontend delta:** **NONE**.

## Size and validation

- `main.rs` before: **9,915 lines**.
- `main.rs` after: **9,879 lines** (net −36 lines: 37-line original handler removed and one module declaration added).
- New module: **45 lines**, containing imports and only the moved handler.
- **Focused validation:**
  - `cargo fmt --check` — PASS.
  - `cargo test -p rah-desktop -- --test-threads=1` — PASS; **315 passed, 18 ignored, 0 failed; 333 discovered**.
  - `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` — PASS.
  - `git diff --check` — PASS (final check below).
- **Workspace validation:** not run. The change is confined to the Desktop crate's command registration and a private child module; focused package validation passed and no cross-crate uncertainty arose.
- **Windows live certification:** not run. This is a mechanical read/presentation handler relocation; no live provider/runtime behavior or certification claim is in scope.

## Final scope and verdict

- **Exact changed files:**
  - `crates/rah-desktop/src/main.rs`
  - `crates/rah-desktop/src/desktop_model_configuration_commands.rs`
  - `docs/plans/2026-09-24-task-397-model-configuration-query-command-extraction.md`
- **Final worktree:** these three files only; implementation intentionally remains dirty for independent audit.
- **Commit:** none.
- **Push/tag:** none.

**PASS — BOUNDED MODEL CONFIGURATION QUERY EXTRACTION COMPLETE**

**Recommended next task:** Task 398 — Model Configuration Query Command Extraction Independent Audit. Do not start it automatically.
