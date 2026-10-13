# Known issues

Open problems that are not fixed yet, with what is known about them, so work
can continue on another machine. Remove an entry when it is fixed, and say
in the commit which entry it closes.

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
  `/sessions`) never completing.
- **Diagnostics in place:** a timed-out wait now prints `session`
  (`current`, `running`, `starting`, `workspace`, header title), the session
  list and every page error (uncaught exceptions, console errors, failed
  loads). The next failure's output should show which of the suspects it is.
- **To reproduce:** run the full suite repeatedly and keep the output, for
  example
  `cargo test --workspace --no-fail-fast 2>&1 | Out-File run.txt`
  with `RAH_TEST_BROWSER` set, then search `run.txt` for `timed out waiting`.

## The terminal REPL rejects `/call`

- **Seen:** 2026-10-13. The README and `model-mock` describe
  `/call <tool> <json>`, but `frontend-cli` treats every line that starts
  with `/` (except `/notool`) as a REPL command. It answers
  `unknown command /call; try /help`
  (`plugins/frontend-cli/src/lib.rs`, the `/plugins` match arm's fallback).
- **Workaround:** pass the line as `--prompt '/call <tool> <json>'`, which
  skips the REPL command parsing. In Git Bash, set `MSYS_NO_PATHCONV=1`, or
  it rewrites `/call` into a Windows path.

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
