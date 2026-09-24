# Task 392 — Desktop Preferences Command Extraction Independent Audit

## Audit scope and starting state

Starting HEAD: `bc3862ca16e5d5da011195c9570c48512453f49a` (`docs: reassess remembered workspace extraction boundary`). The initial `git status --short` contained exactly:

```text
 M crates/rah-desktop/src/main.rs
?? crates/rah-desktop/src/desktop_preferences_commands.rs
?? docs/plans/2026-09-24-task-391-bounded-desktop-preferences-command-extraction.md
```

The implementation audit used the parent version from `git show HEAD:crates/rah-desktop/src/main.rs`, current source, the complete tracked diff, and direct searches in `main_tests.rs` and the desktop crate. No unrelated dirty file was present.

## Exact Task 391 implementation delta

The production delta consists of:

1. A Windows-gated `mod desktop_preferences_commands;` adjacent to the existing desktop preference module declaration.
2. Removal of the original function from `main.rs`.
3. A `generate_handler!` entry path change from `desktop_preferences_warning` to `desktop_preferences_commands::desktop_preferences_warning` at the same list position.
4. A new 17-line module containing the two required imports and the relocated command.

No other production source change is present. The only other starting dirty file is the Task 391 plan record.

## Original and relocated handler comparison

Original symbol: `desktop_preferences_warning`, `main.rs:4872–4885` inclusive in starting `HEAD`.

Relocated symbol: `desktop_preferences_commands::desktop_preferences_warning`, `desktop_preferences_commands.rs:4–17` inclusive.

Original attributes: `#[cfg(target_os = "windows")]` and `#[tauri::command]`. The Windows cfg is now on the module declaration; the Tauri command attribute stays on the function.

Original signature and behavior:

```rust
fn desktop_preferences_warning(state: State<'_, DesktopAppState>) -> Option<&'static str>
```

The function synchronously locks `state.preferences`, recovers a poisoned mutex with `PoisonError::into_inner`, consumes exactly one warning through `take_warning()`, and maps `RestoreFailed` to `preferences_restore_failed` and `SaveFailed` to `preferences_save_failed`. It returns `None` when there is no warning. It has no await points, additional side effects, helper calls, or error conversion.

The relocated function preserves its attribute, name, argument name and order, `State<'_, DesktopAppState>` type, return type, lock/recovery sequence, one-shot consumption, match arms, strings, and control flow. Its only signature difference is rustfmt line wrapping. Added dependencies are only `super::{DesktopAppState, PreferencesWarning}` and `tauri::State`, both required at the new module boundary. `pub(super)` permits the parent module’s handler registration.

BODY EQUIVALENCE: PASS

## Module and visibility boundary

The new module is declared under the same Windows cfg as the existing preference module. It introduces no public crate API, DTO, field visibility change, helper, abstraction, re-export, or wrapper. The module contains only the required imports and the moved handler.

Frozen visibility budget:

```text
1 handler -> pub(super)
0 DTO visibility changes
0 field visibility changes
0 helper visibility changes
```

Actual visibility delta is exactly one handler changed from private to `pub(super)`. No other visibility changed.

## Tauri registration and IPC/schema

The command appears exactly once in `generate_handler!`, at its original position. Only its Rust path is module-qualified. The handler list contains 47 entries before and after; no registration was removed, added, duplicated, or reordered. The three separately known permission-coverage findings (`host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and `host_prepare_repo_delete_file`) were not changed.

The Tauri command name remains `desktop_preferences_warning`; its argument remains `state` with the same managed state type; its serialized result remains the same optional static string. Both output strings and the no-warning `None` case are unchanged. The frontend invocation and event handling remain unchanged.

IPC/SCHEMA: UNCHANGED

## Independent test-coupling audit

Direct search of `crates/rah-desktop/src/main_tests.rs` found zero references to `desktop_preferences_warning` and zero references to `desktop_preferences_commands`. No test file changed.

No request or response DTO or private type was moved. `DesktopAppState` and `PreferencesWarning` remain in `main.rs`; test references to those retained types do not constitute moved-type coupling. No tests access a moved private field, and no ignored/live test references the command or new module.

The adjacent unchanged test `preference_and_conversation_warning_domains_do_not_cross` (around `main_tests.rs:18146`) independently maps both preference warning variants to the same two strings and verifies conversation warning serialization does not cross into those strings. It does not call the command, check `take_warning`, or cover the command’s one-shot consumption. Since the moved match expression is body-equivalent and the enum remains in the same module, relocation does not change that mapping; the focused package suite also passed this test.

Coupling counts:

```text
handler test references: 0
moved private-type references: 0
private-field coupling to moved types: 0
ignored/live test coupling: 0
```

## IPC, authority, permissions, and dependencies

The handler only consumes and maps a stored Desktop Preferences warning. It does not own or change repository authority, admission, identity validation, membership, ToolRegistry, HostExplicit, review tickets, commit authorization, runtime execution, provider lifecycle, runtime lifecycle, mutation authority, network authority, or persistence authority. It does not configure or activate a provider.

AUTHORITY DELTA: NONE

No changes exist in `build.rs`, capability files, permissions, `Cargo.toml`, `Cargo.lock`, or frontend files. There is no permission-generation or capability change and no opportunistic repair of the three out-of-scope permission findings.

PERMISSION/CAPABILITY DELTA: NONE
CARGO/DEPENDENCY DELTA: NONE
FRONTEND DELTA: NONE

## Diff scope and line counts

The complete production diff contains only the module declaration, function removal, registration path adjustment, and equivalent relocated function. No whitespace churn, neighboring edit, rename, comment change, logic cleanup, or test rewrite was found.

Independent line counts (`splitlines()` on Git’s `HEAD` content and current files):

```text
main.rs at HEAD:                 9,931
main.rs current:                 9,919
 desktop_preferences_commands.rs:   17
```

The production line delta is -12 in `main.rs`; the new module is 17 lines.

## Focused validation

```text
cargo fmt --check                                      PASS
cargo test -p rah-desktop -- --test-threads=1         PASS (315 passed, 18 ignored, 0 failed)
cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings
                                                       PASS
git diff --check                                       PASS
```

Workspace validation (`cargo check --workspace`, `cargo test --workspace`, and workspace clippy) was not run: the audit found no cross-crate or dependency change, and the task specifies the focused desktop checks. Windows live certification was not run because it is outside the requested focused validation; the suite’s opt-in live tests remained ignored.

## Exact files and verdict

The intended and only changed files are:

```text
crates/rah-desktop/src/main.rs
crates/rah-desktop/src/desktop_preferences_commands.rs
docs/plans/2026-09-24-task-391-bounded-desktop-preferences-command-extraction.md
docs/plans/2026-09-24-task-392-desktop-preferences-command-extraction-independent-audit.md
```

PASS — DESKTOP PREFERENCES COMMAND EXTRACTION INDEPENDENTLY VERIFIED

Commit recommendation: commit these four files together with `refactor: extract desktop preferences warning command`. Do not push or tag.

Next task: Task 393 — independently select and freeze the next bounded desktop command extraction candidate; do not start it automatically as part of Task 392.
