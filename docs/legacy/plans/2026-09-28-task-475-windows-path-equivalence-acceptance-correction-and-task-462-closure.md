# Task 475 — Windows path-equivalence acceptance correction and Task 462 closure attempt

## Preflight and acceptance contract

Authoritative checkout: `F:\coding\otherPrj\rah-task-466`, branch `task466-task462-reentry`, starting HEAD `c153b2727b73943b424e4ca154aac3a7f9313933`. The starting worktree was intentionally dirty with five tracked source/test edits and untracked Task 462, 463, and 466–474 reports. These were preserved. The original `rust-agent-harness` checkout was untouched. The tracked diff was limited to the Task 462 Remembered Workspaces editor CSS/class and frontend checks, plus Task 471's exact `dialog:allow-open` grant and permission inventory check.

Task 472 remains historically **B — LIVE ACCEPTANCE FAILED** under its then-current rule requiring raw case-sensitive equality between the operator-authored `F:\Temp\...` path and Show Location's `F:\temp\...` result. Task 473 proved both spellings identify the same Windows directory object and committed fixture: matching Windows file ID and final handle path, with fixture HEAD `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`. It classified picker provenance **F — PROVENANCE REMAINS INDETERMINATE**. Task 474 captured the raw picker return, Save input, persisted value, and Show Location value: all were the same `F:\temp\...` string. The spelling transition occurred before RAH received the picker result; Task 474 classified the native-layer cause **E — OPERATOR SPELLING DIFFERS FROM PICKER RETURN, BUT CAUSE REMAINS BELOW RAH**. Its temporary diagnostic source had been removed.

Task 475 supersedes only Task 472's raw operator-string criterion. For a path selected with the production Windows folder picker, acceptance requires successful selection; preservation of the picker-returned path through Save and persistence; unchanged Show Location presentation of the persisted value; resolution to the intended Windows filesystem object and committed repository fixture; and no truncation, lost segment, substitution, or different target. A different case spelling supplied by the picker for the same proved object is acceptable. Mere case-insensitive text comparison is insufficient. This is not an arbitrary normalization rule. No production path-handling code was changed.

## Corrected long-path live gate

The established 198-character committed fixture existed and had HEAD `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7` before selection. The first launch used an existing executable that still contained Task 474's temporary diagnostic UI, despite the restored source. That run was excluded from current acceptance. The Desktop process was stopped; `cargo build -p rah-desktop --bin rah-desktop --offline` passed; and the rebuilt normal production Desktop was launched from the authoritative checkout. It reported Desktop UI ready and version `0.32.0`; no Task 474 diagnostic UI appeared.

In the rebuilt Desktop, the existing `Document worker capacity and update output label` entry's editor opened and showed usable `Replace Location`, `Clear Location`, Save, and Cancel controls at normal width. `Replace Location` opened the native folder picker. The picker navigated to the established fixture, displayed its full path, and returned successfully. The editor displayed `New location selected — save to remember it`. `Save Changes` succeeded and the catalog showed `Location saved — hidden`. `Show Location` revealed:

```text
F:\temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

The revealed string is 198 characters, retains every intended segment, identifies an existing directory, and `git rev-parse HEAD` with that directory as its repository returned `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`. The existing Task 473 file-object identity and Task 474 picker-return evidence establish the other parts of the corrected contract; those audits were not repeated. **Corrected long-path gate: PASS.**

## Narrow-window gate and stop

The rebuilt Desktop window was resized to approximately 600 × 700 pixels. The existing entry's editor opened. Windows UI Automation reported the dialog at screen bounds `211,199,560,757` while the Desktop window began at `100,100` and measured 600 × 700. Thus the dialog's right edge was about 71 pixels beyond the window's right edge, and its bottom edge was about 156 pixels beyond the window's bottom edge. `Replace Location` and `Clear Location` remained enabled, but `Save Changes` and `Cancel` lay below the usable viewport. A visual capture confirmed that the editor's right side and lower actions were clipped. This is actionable clipping and unintended horizontal overflow at a realistic narrow size. **Narrow-window gate: FAIL.**

Per the task's stop rule, no UI redesign, Task 460 repository chooser smoke, final accumulated source review, canonical Desktop test gate, workspace deterministic gates, commit, push, or exact-head CI was performed. The preflight source review found no unexpected production/test scope drift, but it does not substitute for the final review. No tag or release was created.

**C — NARROW-WINDOW ACCEPTANCE FAILED.** The corrected path contract passes; Task 462 remains open because the original layout issue is still materially present at narrow width. The Task 472 historical failure is unchanged. RAH remains `0.32.0`; no evidence in this task changes HostExplicit 11, Codex certification, Remembered Workspace descriptive authority, repository lifecycle, or v0.33 capability selection.
