# Task 472 — Remembered Workspace final live acceptance and Task 462 closure

## Preflight

- Authoritative checkout: `F:\coding\otherPrj\rah-task-466`, branch `task466-task462-reentry`, HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`.
- Starting worktree was intentionally dirty: modified `crates/rah-desktop/capabilities/default.json`, `frontend/remembered_workspace_test.js`, `frontend/status.js`, `frontend/styles.css`, and `tauri_permission_test.js`; untracked Task 462, 463, and 466–471 reports. The Task 462–471 WIP was present. The original checkout was untouched.
- Production/test diff remained the Task 462 editor layout correction and regression checks plus Task 471's single `dialog:allow-open` grant and exact permission inventory check. No unexpected production/test drift was found in preflight.
- Task 471 had already classified the permission correction as validated: the native folder picker opened and returned successfully, with the editor still operational. Its selection was intentionally not saved.
- `git diff --check` passed in preflight. Git emitted only the existing LF-to-CRLF working-copy warnings for the capability and permission test files. The Task 467 line-ending audit was not repeated.

## Live continuation

The Task 471 Desktop process was no longer running. Launched the existing `target\debug\rah-desktop.exe` from the authoritative checkout as the normal production Tauri Desktop. It reported `Desktop UI ready` and version `0.32.0`. The existing remembered entry `Document worker capacity and update output label` was present. Opening its editor showed `Replace Location` because it already had a saved hint, plus the label field, `Save a location hint`, `Clear Location`, `Save Changes`, and `Cancel`. At the normal window size these controls were visible, enabled, and contained within the editor.

The intended fixture was the clean, committed, 198-character repository path below. Before selection it existed and `git rev-parse --verify HEAD` returned `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`:

```text
F:\Temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

Activated `Replace Location`. The native `Choose remembered workspace location` picker opened. The human selected the fixture and returned from the picker. The editor remained operational and displayed `New location selected — save to remember it`; the location and sibling controls remained visible, enabled, and contained. No Task 470 `dialog.open not allowed` or `Desktop frontend unavailable` error recurred. Activated `Save Changes`; the editor closed and the catalog displayed `Location saved — hidden`. Activated `Show Location` for that entry. It revealed:

```text
F:\temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

The intended and revealed path strings differ at the casing of `Temp` versus `temp`. The remaining path text matches, but the Task 472 exact-string comparison does not pass. This report does not infer truncation, wrong repository selection, or a persistence defect from that case difference. The required exact-path gate failed, so the instructed stop applied.

## Gates and disposition

| Gate | Result |
| --- | --- |
| Native picker opens and returns with valid committed long fixture | PASS |
| Normal-width editor usable before and after selection | PASS |
| Save succeeds | PASS |
| `Show Location` equals intended fixture path exactly | FAIL: `F:\Temp` versus `F:\temp` |
| Practical narrow-window editor acceptance | NOT RUN: stopped at exact-path gate |
| Task 460 disconnected and connected-ready chooser smoke | NOT RUN: stopped at exact-path gate |
| Final accumulated production/test review and broad deterministic validation | NOT RUN: stopped at exact-path gate |
| Commit, push, exact-head CI | NOT RUN: classification B forbids closure |

**B — LIVE ACCEPTANCE FAILED.** The exact stored-path comparison failed. Task 462 remains open and the accumulated maintenance work remains uncommitted. No tag or release was created. Task 463–471 reports retain their historical outcomes without revision.
