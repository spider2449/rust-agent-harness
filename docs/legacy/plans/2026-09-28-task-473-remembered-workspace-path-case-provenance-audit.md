# Task 473 — Remembered Workspace path-case provenance audit

## Checkpoint and historical result

Authoritative checkout: `F:\coding\otherPrj\rah-task-466`; starting HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`. Starting state was intentionally dirty: modified `crates/rah-desktop/capabilities/default.json`, `frontend/remembered_workspace_test.js`, `frontend/status.js`, `frontend/styles.css`, and `tauri_permission_test.js`; untracked Task 462, 463, and 466–472 reports. All were preserved. Task 472 remains **B — LIVE ACCEPTANCE FAILED** under its exact raw-string gate.

Task 472 intended this 198-character fixture string:

```text
F:\Temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

`Show Location` revealed:

```text
F:\temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

## Production source route

`frontend/status.js` `openRememberedEditor` calls `pickRememberedLocation`, which awaits Tauri `dialog.open({ directory: true, multiple: false, title: ... })`. A string result is returned without path transformation; the editor assigns it directly to local `selectedLocation`. The editor does not show the raw selection. Save sets `request.locationHint = selectedLocation` for `update_remembered_workspace_candidate`. Tauri IPC transports the string to `main.rs`, where `parse_remembered_location_hint` constructs a `PathBuf` and calls `RememberedLocationHint::parse`. That parser validates absolute native form, length, controls, and components; it does not access the fixture or call `canonicalize`. The update replaces the member hint and serializes it via `PathBuf::to_str` and `serde_json::to_vec` to `remembered-workspace.json`. Reload parses the JSON string into `PathBuf` through the same validator. `remembered_workspace_catalog` exposes only `hasLocationHint`, not the path. `reveal_remembered_workspace_location` reads the published hint, converts it with `to_str`, returns `location`, and the frontend assigns it to `textContent` without formatting. This path has no repository-root reconstruction, `git rev-parse`, drive-letter folding, separator normalization, or explicit canonicalization. `PathBuf` and JSON transport preserve the observed spelling in this route; neither is used here for filesystem resolution. Repository admission is a separate, explicit action with its own identity verification and does not run during Save or Show Location.

ADR 0028 explicitly defines persisted hints as descriptive locator text and prohibits canonicalization or filesystem probing during persistence. Its repository identity rules apply only on fresh admission. The path-casing change is therefore not established as a deliberate RAH persistence normalization.

## Ground truth and stored value

Both exact strings passed `Test-Path -PathType Container`. `fsutil file queryfileid` returned the same directory file ID for each: `0x00000000000000000008000000005885` on drive `F:`. `GetFinalPathNameByHandleW` with an open directory handle returned the same final name from either spelling, beginning `\\?\F:\temp\...\committed-repository-root`. Enumerating the root directory reported its actual component name as `temp`. PowerShell `Get-Item` and `Resolve-Path` retained the *input* spelling, so their displayed paths are not authoritative casing evidence. Both `git -C <path> rev-parse --show-toplevel --verify HEAD` invocations returned the same lowercase-spelled top level and commit `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`. The fixture worktree had no short-status entries. The Windows file ID and handle final-name evidence establish that these inputs identify the same directory object at audit time.

The current application store, `C:\Users\morefunfun\AppData\Local\org.rust-agent-harness.desktop\remembered-workspace.json`, contains `F:\temp\...\committed-repository-root` for candidate `a0b1c4b8-4321-4cf6-8024-079e64fd2dc0` (`Document worker capacity and update output label`). This is read-only inspection of the existing saved record. It agrees with Task 472's Show Location result. The current store cannot by itself prove the original picker return or Save IPC input; no before-Save raw path was recorded in Task 472. No Desktop process was running at audit time. No new runtime picker sequence or temporary diagnostic source edit was used.

## Provenance boundary and classification

The earliest **observed RAH value** with different casing is the persisted `location_hint`; the Show Location presentation matches it. The earliest proven source of the lowercase *filesystem spelling* is the native Windows directory entry/final-path query. The exact transition from Task 472's operator string to the picker return was not observed. Source review strongly constrains the possibilities: if the dialog returned lowercase, the frontend, persistence, readback, and Show Location preserve it; if it returned uppercase, a mutation inside the transport or storage path remains to be demonstrated. The existing evidence does not prove which occurred during Task 472.

**F — PROVENANCE REMAINS INDETERMINATE.** The two representations identify the same committed fixture, and `Show Location` does not independently change the saved presentation. Persistence currently holds lowercase, but whether it changed the picker-supplied value is unproven. Therefore raw case-sensitive equality is not established as a valid representation of the existing Windows Remembered Workspace path contract, nor is it disproven as a requirement to preserve the actual picker-returned text. Task 472's historical gate failure stands. Do not revise its result or its acceptance contract on this audit alone.

Recommended next task: capture the raw `dialog.open` return and Save `locationHint` once in a bounded production diagnostic, then compare them with the persisted JSON and Show Location. Use that evidence to choose between a picker-presentation finding and a narrowly scoped product correction or acceptance-contract revision. No product source, tests, fixture, stored catalog, or acceptance criterion was changed in Task 473.

Final dirty state: the preserved Task 462–472 modified files and untracked reports remain, plus this untracked Task 473 report. `git diff --check` passed. No broad suites, commit, push, tag, or release were run.
