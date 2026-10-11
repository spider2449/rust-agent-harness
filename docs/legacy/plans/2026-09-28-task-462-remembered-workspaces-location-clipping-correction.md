# Task 462 — Remembered Workspaces location clipping correction

## Starting state and evidence

The isolated checkout and `origin/master` both started at
`ca51c8896b889fcaa413ea22463c1277d4ebf0ec`; the worktree was clean.
Task 458 observed the remembered entry editor extending beyond its content
area and clipping the left side of `Choose Location`. Task 459 classified this
as an existing UI defect and noted that the original observation did not retain
viewport measurements or establish the specific CSS mechanism.

## Source-level cause and correction

`openRememberedEditor` appends a title, label input, location checkbox label,
location button, clear button, status text, and save/cancel buttons directly to
one form. The global `dialog form` rule makes that form a non-wrapping flex row.
Its minimum content width therefore exceeds the dialog's `28rem` maximum;
buttons later in the row extend beyond the dialog. Global `input { width: 100% }`
also applies to the checkbox, enlarging that row's required width. There is no
fixed-width location field or hidden-overflow rule in the remembered form.

The editor now has a scoped class and one-column grid form. Its width is
bounded by the viewport, text input can shrink with the grid, checkbox uses
its native width, and buttons stay within the editor. The location picker and
save actions retain their existing behavior. Revealed long paths remain text
in `.remembered-location`, which uses `overflow-wrap: anywhere`; the stored
location is deliberately hidden until the existing Show Location action.

Changed files: `frontend/status.js`, `frontend/styles.css`, and
`frontend/remembered_workspace_test.js`. The focused static regression asserts
the editor's class and scoped layout contract, including viewport width,
shrinkable grid track, checkbox sizing, and bounded buttons. It does not assert
browser pixels. No dependency, ADR, repository admission, ToolRegistry,
permission, persistence, or authority semantics changed. Remembered Workspaces
remain descriptive state only.

## Validation and current stop

The focused frontend test, `node --check` for `status.js`, authority frontend
test, Tauri permission test, `cargo fmt --check`, `git diff --check`, and
`cargo test -p rah-desktop --no-run` passed. The canonical Desktop gate ran
exactly once and passed: 324 passed, 0 failed, 20 ignored. The workspace
sequence passed: formatting, check, tests, clippy with warnings denied, the
three specified Node checks, diff check, and metadata. Metadata reported 13
packages and 13 members, all version 0.32.0 and edition 2024.

The normal release Desktop build passed and launched in a 1100 by 900 logical
window on a 3072 by 1728 Windows desktop. The UI reported ready. A preexisting
remembered entry with no location was opened in the normal Edit dialog. The
location button, checkbox, status, and save/cancel controls were visible and
contained within the editor; the row no longer extended outside the dialog.
The disconnected repository chooser was visible and enabled. A disposable
long path was created outside the repository at
`F:\Temp\rah-task462-live\some-long-workspace-name\subdirectory\project`,
but the native folder picker flow did not complete during this validation.
Therefore the long-path reveal, narrow-window, and connected Task 460 smoke
scenarios are **not validated**. This evidence does not close Task 458's full
live scenario. No production files or authority semantics were changed by the
validation attempt.

Task 462 is **PARTIAL / NOT COMPLETE**. Do not commit, push, tag, or claim
natural exact-head CI until the remaining live scenarios pass. The remote
master and exact committed-head publication gates have not been attempted.

## Task 468 editor-targeted live attempt

Task 468 source-identified the Remembered Workspaces editor path separately
from the active repository chooser. The production UI reached the existing
entry's `Edit remembered workspace` dialog with its location and sibling
controls visible (A1 PASS). One click on that dialog's `Choose Location`
button did not open a native picker; the UI showed `Desktop frontend
unavailable` (A2 FAIL). A3–A5, B, and C are UNVERIFIED. The editor currently
shows a selected-location status message rather than the selected path before
saving, so the requested A3 path-in-editor observation also requires a scope
decision after the picker failure is diagnosed. See
`2026-09-28-task-468-remembered-workspaces-editor-live-acceptance.md`.
Task 462 remains **PARTIAL — UNCOMMITTED**.

Task 469 audits the picker bridge and existing post-selection UI contract without changing this outcome; see `2026-09-28-task-469-remembered-workspace-picker-bridge-and-acceptance-contract-audit.md`.

## Task 467 re-entry and live attempt

Task 467 proved the Task 466 re-entered production/test WIP has identical Git
filtered blobs to the original Task 462 WIP. Its explicit EOL-only equivalence
audit supersedes Task 466's raw-byte gate for this re-entry. A 198-character
committed long-path fixture passed the current RAH admission validation before
the native picker. In the subsequent production UI attempt, a human reported
selecting the fixture, but the captured screen showed it as the active
repository and did not show the Remembered Workspaces editor or location
control. Long-path editor Gate A therefore remains **UNVERIFIED**. The required
narrow-window and Task 460 chooser gates were not attempted. The live stop
rule applied; Task 462 remains **PARTIAL / UNCOMMITTED**.
