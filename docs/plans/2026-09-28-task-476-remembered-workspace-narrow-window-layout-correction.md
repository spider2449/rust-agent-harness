# Task 476 — Remembered Workspace narrow-window layout correction

## Starting state and diagnosis

Authoritative checkout: `F:\coding\otherPrj\rah-task-466`, branch `task466-task462-reentry`, HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`. The worktree began intentionally dirty with the five Task 462/471 tracked edits and untracked Task 462–475 reports. They were preserved. The original `rust-agent-harness` checkout was untouched.

Task 475 had already passed the corrected 198-character Windows picker/path contract against committed fixture HEAD `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`. Its 600 × 700 production capture then showed the editor at `211,199,560,757` inside a window at `100,100,600,700`: right and bottom edges exceeded the window and Save/Cancel were clipped. Task 475 remained **C — NARROW-WINDOW ACCEPTANCE FAILED**. No later gate or publication ran.

The production editor is a native modal `dialog` containing one grid-column `form`: title, label/input, checkbox label, two location buttons, status, Save, and Cancel. Global buttons have `margin-top: 1.5rem`, and the two location buttons are deliberately stacked at usable widths. There is no non-wrapping row or long visible path in this editor; the text input already has `min-width: 0`, the checkbox has native width, and buttons already wrap text. The dialog's prior `width: min(28rem, calc(100% - 2rem))` based its narrow fallback on a percentage of the containing block rather than explicitly on the visible viewport. The underlying page has horizontal scrolling at narrow size even with the editor closed. In the Task 475 live state, the dialog was centered and sized beyond the visible right edge. This task changed only the editor's width fallback to `calc(100vw - 2rem)`; the rebuilt live dialog at the same 600 × 700 outer window measured `119,148,544,615` and ended inside the right edge. The remaining page scrollbar was visible after Cancel and therefore was not introduced by the editor.

Vertically, the editor had no viewport height limit. The stacked form and button margins are taller than the narrow viewport, so Save and Cancel fell below the window. The scoped editor now has `max-height: calc(100dvh - 2rem)` and `overflow-y: auto`. The modal stays inside the window and its own scrollbar exposes the actions. No persistence, picker, path, authority, or repository lifecycle logic changed.

## Files and regression coverage

- `frontend/styles.css`: viewport-relative width and height bound with internal vertical scrolling, on the existing editor class only.
- `frontend/remembered_workspace_test.js`: updated the scoped style contract.
- `frontend/remembered_workspace_layout_test.js`: loads the production shell, stylesheet, and editor construction function in Chromium at roughly normal, 600 × 700, and nearby narrower viewports. It checks viewport containment, no additional page horizontal overflow, horizontal bounds of every action, enabled location/Save/Cancel controls, picker callback behavior, and Save/Cancel reachability after scrolling. This Windows Desktop test uses the installed Microsoft Edge Chromium engine.
- Accumulated Task 462 `frontend/status.js` class assignment and Task 471 `dialog:allow-open` with its permission inventory test remain unchanged in behavior.

## Validation and disposition

Focused `remembered_workspace_test.js`, browser layout test, `node --check` for `status.js`, `tauri_permission_test.js` (47 application commands and exactly `dialog:allow-open`), and `git diff --check` passed. `cargo build -p rah-desktop --bin rah-desktop --offline` rebuilt the normal production Desktop.

Normal-width production smoke: the editor opened inside the 1518 × 1047 outer window with Replace Location, Clear Location, Save Changes, and Cancel enabled and visible. Narrow live run: outer window `100,100,600,700`; dialog `119,148,544,615`, wholly inside the window. Replace Location and Clear Location were actionable and horizontally contained. The dialog's UI Automation ScrollPattern reported vertical scrolling and no horizontal scrolling; one large vertical increment brought Save and Cancel to `140,586,171,56` and `140,686,104,56`, both inside the dialog and window. A page horizontal scrollbar was present before opening and after closing the editor; the editor did not introduce it.

Long-path picker smoke: Replace Location opened the native `Choose remembered workspace location` picker; selecting the established committed fixture returned to the operational editor with `New location selected — save to remember it`. The selection was canceled without saving. Disconnected chooser: Choose Repository enabled. Connected-ready, no active turn: `Chat ready`, Choose Repository disabled, and `Disconnect before changing repositories.` shown. The runtime was then disconnected and the Desktop process stopped.

Final source review found the accumulated functional diff limited to the Task 462 editor class/style and frontend tests, Task 471 `dialog:allow-open` and permission inventory test, and this Task 476 responsive correction/test. There is no product path-handling or Task 474 diagnostic code. No repository authority, HostExplicit, provider lifecycle, persistence, admission, ToolRegistry, Codex certification, version metadata, or dependency file changed. `git diff --check` passed.

Deterministic commands/results:

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo build -p rah-desktop --bin rah-desktop --offline`: PASS, production live binary.
- `node crates/rah-desktop/frontend/remembered_workspace_layout_test.js`: PASS, three browser window sizes including the narrow equivalent and a smaller neighbor.
- `node crates/rah-desktop/frontend/remembered_workspace_test.js`, `status_authority_test.js`, `repository_membership_test.js`, `tauri_permission_test.js`, and `node --check` for `status.js` and the new browser test: PASS.
- `git diff --check`: PASS.
- `scripts/windows-desktop-test-gate.ps1`: **HARNESS_ERROR, exit 70**. Helper preparation exited 0. Its Desktop test had no exit status after the 10-minute watchdog. The harness's cleanup `taskkill` emitted `ERROR: The process with PID 10308 (child process of PID 1968) could not be terminated`, causing a harness exception. The status file reports `watchdogTimeout: false` because the exception prevented the normal timeout disposition; this does not establish a completed Desktop test. Evidence: `F:\Temp\rah-windows-desktop-gate\20260928-202314-834-2b1688a89d11405ca12837f90ac9335c\status.json`, `harness-error.log`, `desktop.stdout.log`, `desktop.stderr.log`, and process snapshot. The stdout had incomplete test execution. Several test panics were captured in stderr before cleanup, so no aggregate passing claim is made from this gate. The precise cause is not attributed to the layout edit or to concurrent validation without further evidence.
- `cargo test --workspace`: was already running when the canonical gate failed. Its Desktop binary completed **324 passed, 0 failed, 20 ignored** in 265.07 seconds; subsequent package tests were still executing. The run was interrupted after the canonical failure to honor the stop rule. It has no workspace-wide PASS or aggregate count.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: not run after the required gate failed.

**E — DETERMINISTIC VALIDATION FAILED.** Live acceptance passed, but the canonical Desktop gate did not. Per the task stop rule, there was no commit, push, exact-head CI, tag, release, or Task 462 closure. The worktree remains intentionally dirty with the accumulated WIP; RAH remains 0.32.0, HostExplicit remains 11, and v0.33 capability remains NONE SELECTED.
