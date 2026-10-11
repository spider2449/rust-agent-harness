# Task 515C — Final integration and publication continuation

Authoritative starting HEAD: `0818efbf999f3592bf7195fcd05578e2bb69526a`.

Continue the accepted Task 515 UI and navigation WIP. Preserve historical
reports and evidence. Starting files are copied and hash-verified under
`target/task515c-integration/backup`, with `starting-manifest.json`.

1. Inspect each of the seven retained Desktop failures; reproduce with bounded
   private diagnostics, then use baseline/concurrency controls only if needed.
   Fix demonstrated defects without retries, changed deadlines or assertions.
2. Identify the literal null notice in the actual production DOM and source;
   correct its rendering with a focused regression.
3. Verify actual Windows streaming Cancel, cancelled terminal, next prompt,
   disconnect/reconnect and missing-OpenAI-key recovery to llama.cpp.
4. Verify owned Desktop normal exit before rebuilding. Run final frontend and
   browser suites, Desktop/workspace/native/neutral tests, Tauri inventory,
   fmt/check/strict Clippy, release build and diff integrity.
5. Review all Task 515 changes and publish only after quality and Windows gates
   pass; verify local/origin/live master equality and exact-head CI success.

No new UI feature, version, tag or release preparation. HostExplicit stays 11.
No authority/runtime change without demonstrated source defect.
`OPENAI_LIVE_NOT_VERIFIED` remains unchanged.

## Preserved failure identities and bounded diagnosis

The original Task 515B invocation (`cargo test -p rah-desktop`, default parallelism)
exited 101: 332 passed, seven failed, 13 ignored, 343.81 seconds. Full original
stdout/stderr remain under `target/task515b/new-observer-failure/`.

| Exact test identity (`tests::` prefix) | Original failure | Current bounded reproduction |
| --- | --- | --- |
| `repository_snapshot_matrix_isolated_repositories_and_replacements` | `main_tests.rs:2246`: selected B snapshot returned `StagedDiffExecution`; child error absent | Full diagnostic suite failed at line 2263 on second A; private staged observer error: `repository observation exceeded its total timeout` |
| `task_320_stale_target_after_final_preparation_preserves_active_state` | line 846: `commit review should be observed: StagedDiffExecution`; child error absent | Full diagnostic suite failed at line 860: `commit review should be available`; the first diagnostic did not log commit-review fallback errors, so its underlying review error remains unproven |
| `task_320_successful_switch_invalidates_old_commit_and_workflow_state` | line 846: `commit review should be observed: StagedDiffExecution`; child error absent | Passed in full diagnostic suite |
| `task_321_e_stage_reservation_wins_over_activation` | line 11542: `A should expose a Stage action` | Passed in full diagnostic suite |
| `task_321_e_unstage_reservation_wins_over_activation` | line 11598: `A should expose an Unstage action` | Passed in full diagnostic suite |
| `task_321_i_real_stage_reservation_rejects_real_connect_admission` | line 9138: `workflow should expose the requested real index action` | Passed in full diagnostic suite |
| `task_321_i_real_unstage_reservation_rejects_connect_admission` | line 9138: `workflow should expose the requested real index action` | Passed in full diagnostic suite |

Missing action selectors do not establish StagedDiffExecution. Source inspection
shows workflow observation can produce an unavailable, empty snapshot after an
observer failure; action construction also has its own eligibility checks. The
original logs do not distinguish these paths for the four selector failures.

One unfiltered diagnostic invocation retained assertions, default parallelism,
policy and deadlines: 337 passed, two failed, 13 ignored, 339.06 seconds; exit 101.
Test-only private diagnostics were then extended to log commit-review fallback
errors. One bounded invocation selected exactly the original seven identities
together, still using default parallelism: seven passed, 345 filtered, 68.24
seconds; exit 0. It emitted no observer/review failure diagnostic. This is a
reduced-suite control, not a serial/baseline proof or permanent repair. No causal
claim about load, Git, external processes or the UI follows from it. No further
diagnostic repetition or backend fix was made without a demonstrated defect.

Artifacts and patches are retained under `target/task515c-integration`.
`diagnostic-artifact.json` and `review-diagnostic-artifact.json` bind executable
paths, lengths and SHA-256. Both copied diagnostic executables are preserved.
`main-restored.json` proves original/final main.rs SHA-256 equality:
`46BE4ED5AFCE8E3F98D3129B0A588391CBE32C3BDF731513A24143211780C72E`.
No diagnostic backend code remains; main.rs has no Git diff.

## Demonstrated null notice correction

Actual production `null-text-node` evidence records a type-3 text node containing
`null` directly in `.shell-notices`. The real `#connection-error` remained inside
`#runtime-section-0-content`. layout.js moved its ancestor into the detached
Inspector, then queried document for the missing element; DOM append converted
the null argument to a text node.

Retain the connection-error element before detaching Runtime, then move that
same element into the global notices. No duplicate IDs or changed handlers.
The focused browser regression checks its owning container, absence of stray
notice text nodes and independent notice visibility. It passed all four widths.
The complete final ten frontend suites and Tauri inventory exited normally with
code 0, including both required Edge browser suites. Inventory remains 49
runtime/manifest/generated/default/frontend commands. Historical Edge timeout
evidence remains preserved and is not described as PASS.

## Actual production Windows lifecycle acceptance

Corrected normal release build: exit 0. Before building, both owned diagnostic
windows had normal retained Process exits 0; the process census found zero
running rah-desktop executables. Evidence: `prebuild-lifecycle.json`.

Accepted executable: `target/release/rah-desktop.exe`, 21,178,368 bytes,
SHA-256 `CC149596E17B3D971C1AE7F19021A672977C313229F7FE7346465D66B80A48F1`.
Release build reports the existing three dead-code warning groups.

Actual native WebView2 session `lifecycle-final`: PID 9836, start
`2026-10-09T11:41:14.3166525+08:00`. Launch identity and live CDP listener ancestry
are retained in launch.json and ownership.json. Mouse input exercised production
controls; no backend IPC substituted chat, cancellation or provider recovery.

- Streaming Cancel: PASS, active llama.cpp response visibly streaming before
  Cancel; Cancel Turn fully inside the actual 476px viewport.
- Terminal: PASS, exact `Chat was cancelled`; host evidence records typed native
  cancellation and exactly one cancelled terminal for session generation 1.
- Subsequent prompt: PASS, `RAH515C_CANCEL_RECOVERY_OK`, session generation 2.
- Disconnect/reconnect: PASS, `RAH515C_RECONNECT_OK`, fresh connection generation 2.
- Missing OpenAI key: PASS, sanitized OPENAI_API_KEY rejection displayed in the
  global notice with Inspector closed; launcher removed the credential from its
  child environment. No OpenAI API inference was performed.
- Explicit recovery to llama.cpp: PASS, `RAH515C_MISSING_KEY_RECOVERY_OK`, connection
  generation 4, model generation 3, session generation 4; final Disconnect passed.
- Normal WM_CLOSE: PASS, same retained Desktop Process exit 0 in exit.json.

The first private acceptance helper stopped before Cancel because it requested
475px using an old DPI calibration but required 476px. Actual measurement was
475px; failure evidence and normal exit 0 remain preserved. Correcting the native
request to 476 retained the exact viewport assertion. During final recovery,
DOM hit-testing showed the open Inspector intercepted the Send click. Read-only
UI/backend evidence proved no turn had started and the prompt remained intact.
Closing the drawer and clicking Send through CDP continued the same pending
acceptance check, with no replay or duplicate submission. Diagnostic and input
continuation evidence are retained in the accepted session directory.

Accepted layout, navigation, restart persistence and model-selected Tool evidence
remain as recorded in Task 515B and the navigation addendum. These product tests
were not repeated. No repository was admitted or mutated in this lifecycle run;
no remembered entry was added or removed. Screenshots and UI/host-event evidence
remain under `target/task515c-integration/lifecycle-final`.

## Final-source quality gates

No backend correction remains. Final frontend source SHA-256 is retained in
`final-frontend-source.json`; `frontend-source-verified.json` confirms no source
drift after browser, release and production acceptance.

| Gate | Result |
| --- | --- |
| All ten frontend suites, both Edge browser suites | PASS, normal exit 0 |
| All frontend JavaScript syntax | PASS, exit 0 |
| Tauri command inventory | PASS, 49 matching commands |
| `cargo fmt --check` | PASS, retained exit 0 |
| `cargo check --workspace` | PASS, retained exit 0 |
| Strict workspace/all-target/all-feature Clippy | PASS, retained exit 0 |
| Native runtime adapters | PASS, 23 tests, retained exit 0 |
| Neutral runtime | PASS, 18 tests, retained exit 0 |
| Fixture helper build and file presence | PASS, retained exit 0 |
| Production OpenAI host Tool fixture | PASS, one test, retained exit 0; no real API call |
| Canonical Desktop invocation | PASS, 339 passed, zero failed, 13 ignored, 280.34 seconds; retained exit 0 |
| Corrected production release build | PASS, retained exit 0, artifact bound above |
| Diff integrity after correction | PASS |

The canonical invocation built/verified sibling fixtures before executing one
unfiltered `cargo test -p rah-desktop --bin rah-desktop -- --nocapture`, with
default parallelism and absolute `RAH_TEST_TARGET_DIR` pointing at this workspace's
target. Logs and same-object Process exits are retained for each gate. No retry,
serial substitution, changed deadline, weakened assertion or process kill was
used to obtain this final canonical result. It does not explain or permanently
repair the earlier suite-context failures.

Full workspace gate: PASS, 1,078 passed, zero failed, 18 ignored across 60
terminal test summaries; same retained Process exit 0. The workspace Desktop
suite also passed under its workspace feature composition. Complete logs and
computed counts remain in `final-workspace.*` and `workspace-summary.json`.

## Review and publication checkpoint

All required final-source gates and remaining actual Windows acceptance pass.
The review covers the five frontend paths and all Task 515 plan/evidence reports.
The bounded correction in this continuation changes only layout.js and its
existing browser suite. The accepted redesign/navigation WIP remains intact.
Historical Task 514/514C/515/515A/515B/navigation reports are byte-identical to
the preserved starting manifest; they are not rewritten to erase earlier failures.

No dependency, ADR, backend, runtime, ToolRegistry, command, permission or host
authority change. HostExplicit remains 11; its exact allowlist and eleven-kind
presentation tests passed in the final canonical gate. The layout storage schema
remains presentation-only version 1. OpenAI real API remains
`OPENAI_LIVE_NOT_VERIFIED`. No release preparation, version bump or tag.

Starting/local/origin/live master was rechecked at the authoritative
`0818efbf999f3592bf7195fcd05578e2bb69526a` immediately before publication.
Only the five frontend files and five Task 515 reports enter the task commit.
The two unrelated historical Task 514 reports remain untracked and preserved.
All diagnostic, failure, backup and actual Windows evidence remains under the
ignored target directories; no cleanup is performed.

This committed report is the verified source/Windows checkpoint. Publication
follows it with a normal push, local HEAD/origin/master/live GitHub master
equality, and natural CI success for that exact head. The resulting commit,
comparison and CI identifiers are retained in
`target/task515c-integration/publication.json` and the final completion response.
Classification A requires those checks to finish successfully; no publication
or exact-head CI result is claimed merely by preparing this report.

Remaining risk: the earlier suite-context observer/review failures are preserved
and not claimed permanently repaired or caused by a specific external condition.
Fresh final canonical and full workspace gates pass without changing test
parallelism, deadlines or assertions. No demonstrated remaining production UI,
security or authority defect remains from this continuation.
