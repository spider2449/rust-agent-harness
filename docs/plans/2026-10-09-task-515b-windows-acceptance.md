# Task 515B — redesigned Desktop Windows acceptance

Starting checkpoint: `0818efbf999f3592bf7195fcd05578e2bb69526a`.
HEAD, origin/master and live GitHub master verified equal before changes.

1. Inventory and hash every dirty/untracked file; verify Task 515A backups and
   preserve Task 514/514C/515/515A reports and original failure evidence.
2. Inspect the implemented UI and existing private Windows acceptance scripts.
   Correct only observed defects; do not redesign the existing features.
3. Build the normal release executable and exercise its owned Windows WebView:
   pointer/keyboard resizing, collapse/restore, focus/reset, real close/restart
   persistence, isolated corrupt preferences and measured narrow layouts.
4. Exercise native llama.cpp chat continuity, model-selected read-only Tool,
   streaming cancel/recovery, reconnect and missing-OpenAI-key recovery.
5. Run required frontend, inventory, canonical Desktop, formatting, check,
   strict Clippy and final build gates; reuse unchanged exact-source evidence.
6. Publish only after actual acceptance and quality gates pass. Review exact
   staged paths, commit the requested message, push and require natural exact-head CI.

Evidence: ignored `target/task515b`; nine starting paths copied and SHA-256
verified. All eight paths in the Task 515A manifest and their retained backups
match. Historical evidence directories remain untouched.

The Task 515A full workspace result is accepted evidence for its tested source:
1,078 passed, zero failed, 18 ignored; Desktop 339 passed, 13 ignored; native
adapters 23 and neutral runtime 18 passed. The original StagedDiffExecution
failure remains preserved and unexplained. No permanent resolution or production
defect is claimed. No staged-diff investigation is planned without a new failure.

No release, tag, version bump, legacy compatibility work or authority expansion.
OpenAI live classification remains `OPENAI_LIVE_NOT_VERIFIED`.

## Preserved checkpoint and changes

Starting and final HEAD: `0818efbf999f3592bf7195fcd05578e2bb69526a`.
Local HEAD, origin/master and live GitHub master agree at closure verification.
Nothing is staged, committed or pushed; no new exact-head CI applies to the WIP.

`target/task515b/starting-manifest.json` inventories all nine starting paths.
Their copies in `backup/` were verified, as were the original eight Task 515A
backups. Task 514/514C/515/515A reports are byte-identical at closure. Historical
failure evidence and all previous evidence directories remain untouched.

Task 515's three tracked frontend edits and its new layout module/test remain.
Task 515B changes only `layout.js`, `workspace_layout_test.js` and this report:
Focus Chat now toggles back to the prior collapse state, displays Exit Chat Focus,
and focuses the Chat region when the disconnected composer is disabled. Reset
and explicit panel toggles clear focus mode. Tests cover enabled and disabled
composer focus, exit restoration and toggle presentation. No dependency, ADR,
backend, provider, command, authorization or persistence-schema change remains.

## Actual production Windows evidence

Normal `cargo build -p rah-desktop --release` passed, retained exit 0, first for
the preserved UI and then for the focus correction. The accepted corrected
executable was `target/release/rah-desktop.exe`, length 21,174,272 bytes, SHA-256
`ED7AA535C69B021148F57807E0EC47C240B8324D3C520A6913AC0989B3FA063E`.
Builds emitted the existing three dead-code warning groups.

The executable launched as a real native RAH window, not standalone HTML.
Private loopback CDP listener ancestry was verified live through WebView2 to
each retained Desktop process. CDP input exercised the rendered production DOM;
it did not substitute backend IPC for model turns, Tool results or authorization.
Normal WM_CLOSE exercised application shutdown; all recorded Desktop exits are 0.

Corruption/restart layout checks used the separate WebView data directory
`target/task515b/isolated-webview`. The LOCALAPPDATA/APPDATA launch overrides do
not relocate Tauri's Windows known-folder backend storage: native admission and
conversation history use normal application storage. The disposable Task 515B
candidate was added through the production API; it remains remembered, ID
`c269ab1d-a5ed-4bc4-9493-3a8bb41f5e07`. No existing candidate was deleted, no
backend configuration was corrupted, and no real repository was mutated.
The disposable repository is the retained `target/task515/fixture-checkout`.

| Actual Windows acceptance | Result |
| --- | --- |
| Both pointer splitters | PASS: widths 240/340 to 280/380 |
| Both keyboard splitters | PASS: widths then 290/390 |
| Workspace and Inspector collapse/restore | PASS: custom widths restored |
| Chat usability / unintended selection | Composer visible; selection empty in measurements |
| Focus / exit focus | PASS after correction, including disabled-composer fallback |
| Reset Layout | PASS |
| Close/relaunch custom layout | PASS: 250/350, Workspace collapsed, Authority tab restored |
| Reset, close/relaunch | PASS: 240/340, both visible, Runtime tab |
| Invalid isolated localStorage | PASS: malformed JSON, negative width, excessive width, version 2 fall back |
| Original production DOM IDs | PASS: every original ID still present |
| Inspector/navigation/dialog access | PASS in bounded rendered checks |

The version-1 closed layout schema still stores only bounded widths, collapsed
flags and a fixed Inspector tab. It stores no credential, repository authority,
Tool authorization or runtime configuration. Focus restoration state is transient.
Narrow-mode launch intentionally closes both drawers while retaining widths/tab.

Rendered CSS pixel measurements from the actual Desktop (viewport height 803):

| Width | Page scroll width | Chat width | Composer bottom |
| --- | --- | --- | --- |
| 1600 | 1600 | 1008 | 790.40 |
| 1200 | 1200 | 608 | 790.40 |
| 626 | 626 | 626.40 | 790.40 |
| 476 | 476 | 476 | 790.40 |

No unusable horizontal page overflow. Send and runtime connection controls remain
within the viewport. At narrow widths Workspace/Inspector drawers can be opened,
and runtime configuration and dialog buttons are reachable. The native width
request needed DPI-rounding calibration: request 476 rendered 478; request 475
rendered exactly 476. No browser viewport emulation was used for these results.
Active-stream Cancel at narrow widths remains unverified.

Screenshots, all under ignored `target/task515b`:

- `layout-final/resized-restored.png`
- `layout-final/width-1600.png`, `width-1200.png`, `width-626.png`, `width-476.png`
- `layout-final/saved-before-close.png`
- `layout-restart/restored-after-restart.png`
- `layout-reset-restart/reset-after-restart.png`
- `functional/first-turn.png`, `second-turn.png`, `tool-result.png`
- `functional/effective-authority.png`, `repo-status-authority.png`

## Native functional and feature parity evidence

Native llama.cpp Connect passed. First streamed marker and second-turn recall
passed. The model invoked `repo.status` once; actual Activity showed Requested,
Running, Completed. Host evidence recorded one requested/started/finished sequence
in session generation 3 followed by normal completed terminal events. The assistant
continued with the exact unseen fixture filename
`task515-29570b2ca0794baab720d8aa9d1e2daa.txt`. Screenshot and the supplemental
read-only progress snapshot retain that answer. This is model-selected execution,
not a manual Host action. No mutation was requested.

A long-response turn started but acceptance was stopped at the failed quality
gate and the owned window was closed normally. Active-stream Cancel, the exact
Chat was cancelled message, another successful prompt, disconnect/reconnect and
actual OpenAI missing-key recovery are NOT VERIFIED in Task 515B. Normal window
shutdown is not Cancel acceptance. `OPENAI_LIVE_NOT_VERIFIED` remains unchanged.
The acceptance helper subsequently exited 1 with a CDP Runtime.evaluate timeout
after the deliberate normal window closure. Its final JSON records multiTurn,
presentation and Tool PASS. This post-closure helper timeout is not evidence of
a new production runtime timeout. No owned Desktop process remains at closure.

Actual bounded checks covered fixture admission/selection, remembered candidate
admission, tabs, native provider configuration, Effective Authority details,
per-Tool Host action forms and Activity. Broader admission/switching, remembered
maintenance, Trusted Profile, host reviews, repository status/diff/stage/commit,
identity and history behavior retain deterministic coverage. The canonical suite
failure prevents an overall parity PASS; destructive real-data operations were
not performed. A visible literal `null` notice remains in production screenshots;
its element/source was not diagnosed after the gate stop and needs follow-up.

## Required gates and new failures

| Gate | Evidence / disposition |
| --- | --- |
| Initial ten frontend suites | PASS, exit 0 |
| Tauri command inventory | PASS: 49 runtime/manifest/generated/default/frontend |
| Corrected workspace layout browser suite | PASS, exit 0 at all four widths |
| Final all-frontend run | FAIL, exit 1: unchanged remembered-workspace Edge test timed out at 476x633 |
| Formatting | PASS, exit 0 |
| Workspace check | PASS, exit 0 |
| Strict all-target/all-feature Clippy | PASS, exit 0 |
| Canonical Desktop | FAIL, exit 101: 332 passed, 7 failed, 13 ignored, 343.81s |
| Full workspace | Retain Task 515A 1078/0/18 for its tested state; no new final-state full run after failure |
| Native adapters / neutral runtime | Retain Task 515A 23 / 18 PASS; no backend correction |
| Corrected production build | PASS, exit 0; actual accepted artifact above |
| Later final rebuild | FAIL, exit 101: Windows denied replacement of the running executable |
| Diff integrity | PASS after diagnostic removal |
| Publication / exact-head CI | NOT RUN; acceptance conditions unsatisfied |

The final rebuild was started while the accepted executable was still running.
The OS access-denied replacement error is a runner sequencing mistake, not a
demonstrated compilation defect. No rebuild retry was used to hide that failure.
The final frontend timeout remains unexplained; no bound/assertion was changed.
Initial frontend success does not erase the final failure.

Canonical failure names:

- `repository_snapshot_matrix_isolated_repositories_and_replacements`
- `task_320_stale_target_after_final_preparation_preserves_active_state`
- `task_320_successful_switch_invalidates_old_commit_and_workflow_state`
- `task_321_e_stage_reservation_wins_over_activation`
- `task_321_e_unstage_reservation_wins_over_activation`
- `task_321_i_real_stage_reservation_rejects_real_connect_admission`
- `task_321_i_real_unstage_reservation_rejects_connect_admission`

The matrix failed with StagedDiffExecution at main_tests.rs:2246, observing the
first selected B snapshot. Two commit-review cases also reported StagedDiffExecution;
four cases lacked expected Stage/Unstage actions. Original full logs, retained
exit evidence and executable hash are preserved in `new-observer-failure/`.
No underlying child error was retained by the uninstrumented failing invocation.
Do not attribute these failures to load, concurrency, the UI patch or Git without
the missing control evidence.

Because this is new reproducible failure evidence, one bounded diagnostic
invocation added the previously retained test-only/private error logging while
keeping assertions, default policy and deadlines unchanged. Exactly the matrix
test executed: 1 passed, 351 filtered, 41.88s, retained exit 0. It emitted no
staged-error diagnostic because it passed. This does not repair or invalidate the
fresh canonical failure. No repeat full suite, baseline comparison, timeout
extension, process killing or retry-to-green followed.

Both temporarily instrumented Rust files were restored from verified copies;
`new-observer-failure/restored.json` proves identical hashes. No backend diagnostic
change remains. All failure evidence remains. HostExplicit stays 11; both exact
allowlist and eleven-kind presentation tests passed in the failing canonical run.
Repository/worktree/Tool authority and native credential/endpoint boundaries are
unchanged. No authority/security regression was demonstrated.

## Final disposition

**D — REPRODUCIBLE REPOSITORY OBSERVER FAILURE**, with additional **F** acceptance
blockers (browser timeout, final build sequencing and incomplete lifecycle checks).
The historical Task 515 failure is not claimed permanently resolved. The new
failure was investigated once without an established root cause or product fix.

Task 515B remains uncommitted and unpublished. The redesigned layout/restart
acceptance advances, but A is not justified. Preserve this checkpoint. Next work
must resolve the fresh observer/browser failures with diagnostic evidence, clear
the visible null notice, and finish actual Cancel/recovery/reconnect/missing-key
acceptance before final exact-state gates and publication. No v0.34.0 preparation.

## Task 515C navigation addendum

The frontend-only side panel navigation extension and its separate production
acceptance are recorded in [Task 515C](2026-10-09-task-515c-side-panel-navigation.md).
Its navigation PASS does not supersede the failures or broader acceptance
nonclaims in this historical Task 515B record. No staged-diff diagnosis resumed.
