# Known issues

Open problems that are not fixed yet, with what is known about them, so work
can continue on another machine. Remove an entry when it is fixed, and say
in the commit which entry it closes.

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
