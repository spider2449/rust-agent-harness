# Known issues

Open problems that are not fixed yet, with what is known about them, so work
can continue on another machine. Remove an entry when it is fixed, and say
in the commit which entry it closes.

## Graphify 0.9.107: UTF-8 BOM Cargo manifests need a compatibility reader

- **Seen:** 2026-11-13 in image-viewer. A leading EF BB BF in Cargo.toml
  makes upstream cargo_introspect's binary tomllib.load fail at line 1,
  column 1; the separate manifest-ingestion path also silently omits pkg_ nodes.
- **Mitigation:** the managed extraction child uses examples/graphify/extract.py
  to read Cargo manifests with utf-8-sig and strip a leading BOM for package
  ingestion. Default child-local PYTHONHASHSEED=0 prevents Graphify re-executing
  itself without the compatibility reader. No source or cached package is edited.
- **Verified:** BOM root/member parser tests, retained pkg_/crate: nodes, the
  RAH managed integration (including invalid TOML rejection), and an installed
  launcher query against image-viewer. Upstream direct CLI calls remain affected;
  recheck both paths before removing the shim after a package upgrade. See
  [the repair report](plans/2026-11-13-graphify-workspace-compatibility.md).

## Graphify 0.9.107: unchanged extraction drops package metadata

- **Seen:** 2026-11-12 during the optional AST-only Graphify evaluation on
  Windows. This is an external evaluation-tool issue, not a RAH runtime failure.
- **Symptom:** repeating `graphify extract` with `--code-only --cargo
  --no-cluster` against unchanged RAH source removed 39 `pkg_*` nodes and
  reduced the edge count by 105. The separate 39 `crate:*` nodes and their
  104 `crate_depends_on` edges remained present.
- **Evidence:** retained exploratory graph snapshots and the result field
  `unchanged_build_drift` are documented in
  [the evaluation report](plans/2026-11-12-graphify-evaluation.md).
  The standalone Python runner has been replaced by
  [native RAH evaluation](plans/2026-11-12-graphify-rah-evaluation.md).
- **Minimal reproduction:** a one-package Cargo fixture loses its `pkg_*`
  node on the second unchanged scan with `--cargo`; the control without that
  flag preserves it. Function replacement and its call edge still update.
- **Open:** determine the affected manifest metadata and upstream cause before
  relying on incremental graphs for dependency analysis. Use source and
  existing architecture tests as the authority; do not infer removed
  dependencies from this graph shrinkage.
- **RAH mitigation:** the managed Graphify MCP launcher builds in fresh staging
  directories and publishes only validated graphs. Native RAH tests confirm
  package metadata survives updates. The upstream incremental issue remains
  reproducible when using Graphify's incremental extraction directly. See
  [automatic indexing](plans/2026-11-12-graphify-auto-index.md).

## Graphify status: current desktop visual verification unavailable

- **Seen:** 2026-11-12 while adding the automatic indexing status page.
- **Environment limit:** Windows Computer Use returned a missing native pipe
  (OS error 2), and no connected browser surfaces were available. An isolated
  background start/stop verification command was rejected by automatic approval
  review (`blocked by policy`) and did not execute.
- **Verified instead:** Node status/protocol tests and the native RAH integration
  test pass, including the authenticated frontend page API, progress, errors,
  notifications, workspace switching and stopped-publisher state. This is not
  a known frontend failure, but current-window pixel verification is pending.
- **Status-bar follow-up:** the real headless Edge test now verifies busy/ready/error
  footer updates, visible click-through and provider removal, with an inspected
  screenshot. Direct inspection of the user's existing desktop window remains
  unavailable. See [the footer report](plans/2026-11-12-graphify-status-bar.md).
- **Follow-up:** restart the existing app to load the installed status-view
  mount, then inspect **Plugins > Plugin pages > Graphify** when UI control is
  available. See [the status implementation report](plans/2026-11-12-graphify-status.md).

## Local repo evaluation: JSON formatting and bounded task completion

- **Seen:** 2026-11-12 in the native RAH Graphify comparison with local
  `ornith-1.5-35b-Q4_K_M.gguf`, thinking enabled, a 1024-token per-request
  output limit and a 120-second turn limit.
- **Symptom:** six completed repo-inspection answers contained correct JSON
  fields inside Markdown fences despite a JSON-only prompt. Two baseline
  authorization turns timed out after reading the correct source. Graphify
  completed four of four turns and baseline two of four, but both methods
  scored zero of four on the strict answer/all-checks rubric. No tool errors
  were recorded; this is not an MCP transport failure.
- **Evidence:** [native evaluation results](plans/2026-11-12-graphify-rah-evaluation.md)
  and the ignored `live_graphify_workspace_evaluation` integration test.
- **Open:** improve output-format compliance and evaluate more representative
  bounded repo tasks before making Graphify a default. Successful harness
  execution is not a quality pass. Model settings and user defaults are unchanged.

## llama.cpp: disabling thinking can add unnecessary tool calls

- **Seen:** 2026-11-11 with ornith-1.5-35b-Q4_K_M.gguf and the optional
  Chat Completions `enable_thinking = false` configuration.
- **Symptom:** a synthetic read-only task explicitly asking for one `fs.read`
  and no other tools instead requested `echo` followed by `fs.read`. The answer
  contained the correct verification code, but the exact tool-plan assertion
  failed in `live_loopback_thinking_mode_comparison`.
- **Measurement:** the initial live comparison failed after 70.83 seconds at
  that assertion. The ignored comparison now records answer correctness and
  exact tool-plan adherence for every probe instead of stopping at a quality
  mismatch. A completed benchmark does not mean all quality probes passed.
- **Follow-up:** a complete rerun passed all four answer/tool-plan probes for
  both modes. This does not erase the first observed extra tool call; it shows
  that the behavior is not consistent across these limited live samples.
- **Broader evaluation:** four work-like synthetic tasks in on/off/off/on
  order (16 turns) passed all strict answer/tool/completion checks for 8/8
  enabled samples and 0/8 disabled samples. Disabled samples included an
  unnecessary `echo`, Markdown instead of required JSON, a wrong timeout
  calculation and one non-timeout incomplete turn. This is a small task set,
  not a population-wide failure rate. See
  `docs/plans/2026-11-11-llm-task-evaluation.md`.
- **Open:** measure representative user tasks before selecting this mode.
  Thinking remains unchanged by default, and existing user configuration is
  never automatically switched. Disabling it is not a guaranteed speedup.
  See `docs/plans/2026-11-11-optional-thinking-mode.md`.

## llama.cpp: model template changes cached prefixes across user turns

- **Seen:** 2026-11-11 with ornith-1.5-35b-Q4_K_M.gguf on the local
  llama-server, using RAH's Chat Completions plugin.
- **Symptom:** a follow-up request with 5363 input tokens reused only 2454
  tokens after the preceding tool-result request had 5331 input tokens.
  The follow-up took 13.03 seconds despite executing no tools.
- **Confirmed:** the served chat template inserts empty `<think>` blocks for
  assistant messages after `last_query_index`, and omits them for messages
  before it. Adding a user message moves this boundary. `/apply-template`
  reproduced a prefix change at an earlier assistant tool-call message with
  otherwise unchanged synthetic history. The exact share of elapsed time
  attributable to this template behavior has not been isolated.
- **Scope:** model-template/cache behavior, not evidence that RAH reordered
  the same history. Reasoning content is deliberately not persisted by RAH.
- **Open:** compare a user-selected template or an explicitly opted-in
  reasoning mode for both cache reuse and answer/tool quality. Do not change
  the server template, replay reasoning text or disable reasoning automatically.
  Measurement: `docs/plans/2026-11-11-rah-turn-latency.md`.

## Windows: sandbox test cannot create a low-integrity token

- **Seen:** 2026-11-11 in the managed Windows development environment while
  running `cargo test --offline -p rah-desktop -p rah-cli --no-fail-fast`.
- **Test:** `sandbox_os::shell_exec_cannot_write_outside_the_workspace`.
- **Symptom:** Windows returns error 87 while setting up the low-integrity
  token. The sandbox plugin fails to start, leaving the shell tool unavailable.
- **Open:** Whether the failure reproduces in a regular Windows terminal.

## Intermittent: "edit and resend" times out in the session browser test

- **Test:** `crates/rah-cli/tests/web_browser.rs`,
  `sessions_are_named_renamed_branched_exported_and_deleted_in_the_browser`,
  step `the edited branch`.
- **Seen:** 2026-10-12, three times locally on Windows 11 with Edge, in about
  20 full runs (`scripts\check.ps1` or `cargo test --workspace`). Never on CI
  (Ubuntu, Chrome) so far. Running the test alone has not reproduced it.
- **Symptom:** the test clicks `edit` on the first user message, sets the
  text and clicks `Send`. The UI should fork before that message, open the
  fork and run the turn (2 entries in a third session). Instead, 30 seconds
  later the page still shows the previous branch (6 entries), no notice was
  shown (`notices: []`) and no turn is running.
- **Ruled out:**
  - Load: it also happened after the browser tests were serialized
    (`ONE_BROWSER` lock), with only one browser running.
  - An error reported by the UI: `resend` reports failures through
    `showNotice`, and no notice appeared.
  - The editor being re-rendered away: the step's `eval` would then have
    thrown (`editor` is null) instead of timing out.
- **Suspects, unconfirmed:** `resend()` in `plugins/frontend-web/assets/app.js`
  returning early (`state.running` still true, or `startTurn` refusing
  because `state.starting` is stuck), or one of its requests (fork, title,
  `/sessions`) never completing. The second fits the symptom better: a
  pending promise shows no notice and changes nothing. `idle()` reads the
  DOM that `setRunning` changes together with `state.running`, so a stuck
  `state.running` would also have failed the previous wait.
- **Diagnostics in place:** a timed-out wait now prints `session`
  (`current`, `running`, `starting`, `workspace`, header title), the session
  list, every page error (uncaught exceptions, console errors, failed
  loads) and `requests`: the page's last 30 requests with their status and
  time, or how long they have been pending. The next failure's output
  should show which of the suspects it is.
- **Not reproduced on 2026-10-13** (Windows 10, Edge): 30 full
  `cargo test --workspace` runs and 10 rounds of three parallel runs of the
  test alone, 60 passes in all. The UI is not changed until a failure shows
  the cause.
- **Not reproduced on 2026-10-14** (Windows 11, Edge, standard user): 11
  full `cargo test --workspace` runs with the browser tests. Those runs did
  find two other intermittent failures, fixed the same day: a `sandbox-os`
  pipe name collision and a refused `storage-fs` rename.
- **Seen again on 2026-11-09** (Windows 10, Edge, v0.45.7 full run), this
  time in `the_web_ui_works_in_a_real_browser`: a wait timed out with the
  page showing "turn failed", the "fork here" control and "cancelled". The
  change under test did not touch `frontend-web`; the test passed three
  times in a row alone right after. The full output was not kept.
- **To reproduce:** run the full suite repeatedly and keep the output, for
  example
  `cargo test --workspace --no-fail-fast 2>&1 | Out-File run.txt`
  with `RAH_TEST_BROWSER` set, then search `run.txt` for `timed out waiting`.

## Setting up another development machine

- **Browser tests** run only when `RAH_TEST_BROWSER` names Chrome, Chromium
  or Edge, for example
  `$env:RAH_TEST_BROWSER = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"`.
  `scripts\check.ps1` finds Edge or Chrome on its own.
  `RAH_TEST_SCREENSHOT_DIR` makes them save screenshots of the UI.
- **The first test run after a build** can be much slower on Windows (a
  scan of the new binaries); the browser tests allow 30 seconds per page
  condition and 90 seconds per DevTools command for that.
- **A running `rah` locks `target\debug\rah.exe`,** so `cargo test` fails to
  relink it ("Access is denied"). Stop `rah`, or build the tests elsewhere:
  `$env:CARGO_TARGET_DIR = "target/agent"`.
- **Repositories owned by another account** (for example after a Windows
  reinstall) need `git config --global --add safe.directory <path>`. RAH's
  Git tools read that setting since the sandbox passes the home directory.
