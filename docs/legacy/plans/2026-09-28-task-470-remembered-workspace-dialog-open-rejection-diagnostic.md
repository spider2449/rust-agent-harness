# Task 470 — Remembered Workspace `dialog.open` rejection diagnostic

## Starting state and preserved scope

The authoritative checkout was `F:\coding\otherPrj\rah-task-466` at HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`. Initial `git status --short` showed modified `crates/rah-desktop/frontend/remembered_workspace_test.js`, `status.js`, and `styles.css`, plus the untracked Task 462, 463, 466, 467, 468, and 469 plan reports. This was the preserved Task 462–469 WIP. No product correction, permission change, commit, push, release, or earlier report rewrite was made.

## Exact source path

In `crates/rah-desktop/frontend/status.js`, `openRememberedEditor` installs the Choose Location click handler. It awaits `pickRememberedLocation(dialog)`, which calls `dialog.open({ directory: true, multiple: false, title: "Choose remembered workspace location" })`. The handler catches the rejected value and calls `showRememberedError(error)`. That function uses `errorMessage(error)`, whose default return is `Desktop frontend unavailable`. Task 469 established this source route; this task captured the missing live rejection.

## Single live diagnostic

A temporary diagnostic-only addition in that exact catch handler displayed `typeof error`, `error?.name`, `error?.message`, and `String(error)` in the existing `#remembered-catalog-error` element after the original fallback call. The original `status.js` bytes were saved at `F:\Temp\rah-task470-status-js-original.bin`. `cargo build -p rah-desktop --bin rah-desktop --offline` succeeded in the checkout. The resulting real Tauri Desktop executable, `target\debug\rah-desktop.exe`, was launched with that checkout as working directory; it displayed `Desktop UI ready` and the existing remembered entry `Document worker capacity and update output label`. The editor was opened using that entry's Add Location control. Exactly one click was made on its Choose Location button. No native picker appeared. The editor was then canceled only to expose the error text behind it. The captured screenshot is `F:\Temp\rah-task470-raw-visible.png`; `F:\Temp\rah-task470-rejection.png` shows the error appearing behind the open editor.

The exact displayed diagnostic was:

```text
TASK470 type=string name= message= value=dialog.open not allowed.
Permissions associated with this command: dialog:allow-open,
dialog:default
```

The caught rejection was a JavaScript string. It had no exposed `name` or `message` property. The rejection itself names `dialog.open` and identifies `dialog:allow-open` and `dialog:default`. This is direct runtime evidence of Tauri command permission denial for the existing invocation. It does not establish why those permissions were absent from the effective capability beyond the static inventory in Task 469.

## Classification and disposition

**A — DIALOG PLUGIN PERMISSION DENIAL PROVEN.** The raw rejection directly establishes the dialog permission/capability layer as the responsible layer for this live failure. A separate subsequent correction task may assess and add the narrow dialog permission, then validate picker behavior. That correction is not authorized by Task 470 and was not performed.

After capture, the diagnostic Desktop process was stopped and the original `status.js` bytes were restored. The restored file and backup had identical SHA256 `B966586A1C5EE9314875494BA846A6DBA4B77D4C40CA13146080C88BD466CF01`. Final `git diff --stat` showed only the pre-existing three production/test files: 8 additions in `remembered_workspace_test.js`, 1 in `status.js`, and 24 in `styles.css`. The temporary diagnostic code is absent from the final diff. The Task 462 production/test WIP is substantively unchanged. `git diff --check` passed. Final status retains those three modified files and the earlier six untracked reports, with this Task 470 report as the sole new persistent file. Historical Task 463–469 verdicts remain unchanged.
