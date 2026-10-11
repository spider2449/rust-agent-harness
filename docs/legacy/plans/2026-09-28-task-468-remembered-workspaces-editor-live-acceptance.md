# Task 468 — Remembered Workspaces editor live acceptance

## Checkpoint and authorization

The Task 466 worktree started at exact HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`. The three Task 462 production/test files were already modified and Task 462/463/466/467 documentation was untracked. `git diff --check` passed. Task 467's classification, **A — Git line-ending normalization only; repository-canonical content identical**, authorizes this specific re-entry. The raw-byte audit was not repeated and no line endings were changed.

## Source navigation and distinct picker flows

The Repository section's `#choose-repository` button calls IPC `choose_repository` from its click handler in `frontend/status.js`. That command selects an active repository; it is the flow captured in Task 467.

The Remembered Workspaces section is `.remembered-workspaces` in `frontend/index.html`. Its `#remembered-catalog` click handler recognizes a candidate button with `data-remembered-action="edit"` or `"edit-location"` and calls `openRememberedEditor`. That function creates the `.remembered-editor` modal dialog, titled `Edit remembered workspace` for an existing candidate. The editor contains `Save a location hint`, `Choose Location` or `Replace Location`, `Clear Location`, `Save Changes`, and `Cancel`. Its `Choose Location` click handler calls `pickRememberedLocation`, which invokes the Tauri dialog plugin's `dialog.open({ directory: true, multiple: false, title: "Choose remembered workspace location" })`. Saving later invokes `update_remembered_workspace_candidate`; it does not invoke `choose_repository`.

The editor has no path-valued input or other control that displays the selected location before saving. On picker return, the handler stores the path in local `selectedLocation` and changes the status text to `New location selected — save to remember it`. The existing catalog `Show Location` action reveals a saved path after a successful save. Consequently the requested A3 observation of the selected path *in the editor's location control* cannot be established from the present product behavior.

## Fixture and live navigation

The existing Task 467 fixture at `F:\Temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root` remained a committed repository: `git -C <fixture> rev-parse --verify HEAD` returned `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`, and `git -C <fixture> rev-parse --show-toplevel` returned the same root (with Git's normalized slashes). Task 467's repository-admission identity and semantic validation had passed for this exact fixture; no new fixture was created.

The normal release Desktop binary built from the canonically identical original Task 462 WIP was launched with the Task 466 worktree as working directory. The UI reported `Desktop UI ready`. Normal navigation was: scroll to **Remembered Workspaces**, locate the existing entry `Document worker capacity and update output label`, then select its **Add Location** action. This opened the **Edit remembered workspace** modal. The label input, location checkbox, `Choose Location`, `Clear Location`, `Save Changes`, and `Cancel` were all visible inside the dialog before picker use. Evidence was captured outside the repository at `F:\Temp\rah-task468-editor2.png`.

One click was made on that modal's `Choose Location` button. No native folder picker appeared. The modal stayed open and a red `Desktop frontend unavailable` error appeared in the Remembered Workspaces section. Evidence was captured outside the repository at `F:\Temp\rah-task468-picker.png`. No folder was selected and no retry was made.

## Gates and disposition

| Gate | Result | Evidence |
| --- | --- | --- |
| A1 correct editor | PASS | The named modal, location control, and sibling controls were visible before the picker click. |
| A2 correct picker source | FAIL | The editor button was clicked, but no native picker opened; the UI showed `Desktop frontend unavailable`. |
| A3 result returned to editor | UNVERIFIED | No selection occurred. The current editor does not display a selected path. |
| A4 clipping acceptance | UNVERIFIED | A3 was not reached. |
| A5 full path inspectable | UNVERIFIED | No location was selected or saved. |
| B narrow-window | UNVERIFIED | Stopped at A2. |
| C chooser smoke | UNVERIFIED | Stopped at A2. |

**Task 462 remains PARTIAL — UNCOMMITTED.** No product or test code changed in Task 468. No commit, remote fetch, push, CI, tag, or release followed. The next narrowly scoped task should diagnose why the production editor's dialog-plugin picker call reports `Desktop frontend unavailable`, while preserving the single-attempt evidence and without treating an active-repository selection as a remembered-location result.

Task 469 records the subsequent bridge and acceptance-contract audit; see `2026-09-28-task-469-remembered-workspace-picker-bridge-and-acceptance-contract-audit.md`. This reference does not revise Task 468's historical gates.
