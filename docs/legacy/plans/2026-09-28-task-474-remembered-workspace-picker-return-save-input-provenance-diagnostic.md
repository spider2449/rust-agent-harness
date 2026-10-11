# Task 474 — Remembered Workspace picker-return and Save-input provenance diagnostic

## Checkpoint and prior evidence

Authoritative checkout: `F:\coding\otherPrj\rah-task-466`. Starting HEAD: `c153b2727b73943b424e4ca154aac3a7f9313933`. Starting worktree was intentionally dirty: modified `crates/rah-desktop/capabilities/default.json`, `frontend/remembered_workspace_test.js`, `frontend/status.js`, `frontend/styles.css`, and `tauri_permission_test.js`; untracked Task 462, 463, and 466–473 reports. All were preserved. The original `F:\coding\otherPrj\rust-agent-harness` checkout was untouched.

Task 472 remains **B — LIVE ACCEPTANCE FAILED** under its raw case-sensitive operator-string comparison. Task 473 classified provenance **F — PROVENANCE REMAINS INDETERMINATE**. It established that `F:\Temp\...` and `F:\temp\...` resolve to the same Windows directory object and committed fixture (HEAD `f0554303c440680dc6ae2dfdf4fdc349ec5b2dd7`), Windows reports the directory component as `temp`, and the stored value and Show Location both contain `F:\temp\...`. This task did not repeat that identity audit.

## Exact source boundaries and mechanism

In `crates/rah-desktop/frontend/status.js`, `pickRememberedLocation` awaits the existing `dialog.open({ directory: true, multiple: false, title: "Choose remembered workspace location" })` at line 1778. The successful result is in `selected` immediately after the await, before the function's string/array handling at lines 1779–1780. `openRememberedEditor` receives that result at line 1847 and assigns it to `selectedLocation` at line 1849. On Save, line 1881 assigns `selectedLocation` to `request.locationHint`; lines 1883–1885 pass `{ request }` into the existing `update_remembered_workspace_candidate` IPC call. This is the final frontend Save-input boundary.

The backend receives the request in `crates/rah-desktop/src/main.rs` at lines 7017–7031; `parse_remembered_location_hint` is at line 6945. `crates/rah-desktop/src/remembered_workspace.rs` updates the candidate at lines 630–647 and serializes the catalog at line 793. The resulting per-user store is `C:\Users\morefunfun\AppData\Local\org.rust-agent-harness.desktop\remembered-workspace.json`. For Show Location, `main.rs` lines 6969 and 6991 read and return the stored hint; `frontend/status.js` lines 1951–1953 place `result.location` into `revealedRememberedLocations`, and the catalog rendering at lines 1725–1726 consumes that value. Task 473 traced the string-preserving details of this route and ADR 0028 forbids canonicalization during persistence.

One temporary `status.js` diagnostic copied the raw successful `selected` value into a diagnostic-only variable immediately after `dialog.open`. Immediately after `request.locationHint` construction, it displayed `JSON.stringify` of both values in a temporary on-screen `<pre>`. This exposed only the two required path values, did not transform either value, and did not change the picker, Save control flow, or backend/persistence behavior. Windows UI Automation read the exact diagnostic text. The original source bytes were backed up before the edit and restored after the live sequence; both before and restored SHA256 are `B966586A1C5EE9314875494BA846A6DBA4B77D4C40CA13146080C88BD466CF01`. No `TASK474` code remains in the final production/test diff.

`cargo build -p rah-desktop --bin rah-desktop --offline` passed with the temporary diagnostic. The resulting normal production Tauri Desktop executable, `target\debug\rah-desktop.exe`, was launched from this checkout. The existing candidate `Document worker capacity and update output label` (`a0b1c4b8-4321-4cf6-8024-079e64fd2dc0`) was edited. The single bounded sequence was: open its editor → Replace Location → native picker → navigate to the same committed fixture → Select Folder → editor reports `New location selected — save to remember it` → Save Changes once → inspect catalog JSON → invoke Show Location once. The native picker address showed the fixture with `F:\temp` before Select Folder. There was one picker selection and one Save; the sequence was not repeated.

## Exact values and transition

The operator-authored fixture spelling was:

```text
F:\Temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

The raw successful `dialog.open` result, the frontend Save `request.locationHint`, the persisted `location_hint` for the candidate, and the later Show Location text were each exactly:

```text
F:\temp\rah-task467-long-20260928\repository-fixture\long-location-for-remembered-workspaces\nested-directory-for-picker-validation\additional-path-segment-for-dialog-width\committed-repository-root
```

The diagnostic displayed `TASK474 picker_return=` and `TASK474 save_input=` with identical JSON string values containing that path. The stored value was read from the catalog after Save; Windows UI Automation read the Show Location text after the explicit reveal action. These are exact runtime and storage values, not visual estimates.

The earliest proven casing difference is **operator fixture spelling → raw successful `dialog.open` return**, specifically `Temp` → `temp`. Picker return → frontend Save input → persisted value → Show Location all preserve the same string. RAH's application-level frontend, Save, persistence, and Show Location route did not mutate the casing after the picker return in this sequence. This diagnostic does not isolate whether Windows dialog APIs, shell canonicalization, filesystem presentation, or the native dialog bridge chose that spelling before the JavaScript return.

**E — OPERATOR SPELLING DIFFERS FROM PICKER RETURN, BUT CAUSE REMAINS BELOW RAH.** The returned spelling agrees with Task 473's Windows-reported directory casing, but this diagnostic cannot attribute the native-layer choice more narrowly. The evidence is sufficient to reconsider Task 472's requirement for raw case-sensitive equality with the operator-authored `F:\Temp` fixture string in a later task. It does not rewrite Task 472's historical **B — LIVE ACCEPTANCE FAILED** result or change its acceptance contract here.

## Final state

The diagnostic Desktop process was stopped. Temporary diagnostic source was fully removed; the preserved Task 462/471 production/test diff remains the same five modified files and `git diff --stat` remains 37 insertions, 1 deletion. This report is the sole new persistent file. Final worktree is dirty with those five modified files and untracked Task 462, 463, and 466–474 reports. `git diff --check` passed. No broad test suite, commit, push, tag, or release was performed.

Recommended next task: explicitly reassess Task 472's raw case-sensitive operator-string acceptance gate against the observed picker return and Task 473's Windows directory identity evidence; only then decide whether to supersede the gate and resume Task 462 live acceptance.
