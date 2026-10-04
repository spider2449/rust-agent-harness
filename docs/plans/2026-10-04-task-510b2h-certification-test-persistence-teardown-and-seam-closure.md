# Task 510B2H — certification test persistence teardown and seam closure

Reference-only follow-up: Task 510B2I recovered the full unexpected SHA and classified artifact identity as **G — INDETERMINATE**. See `2026-10-04-task-510b2i-codex-candidate-transient-sha-diagnosis.md`.

Status: stopped — E, later deterministic validation failed.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.

## Scope and diagnosis

Task 510B2G identified the exact retained owner as the RAH Desktop managed
`DesktopAppState.persistence` field, whose `Persistence` contains the SQLite
connection for `conversation-transcript.sqlite3`. Production runtime shutdown
was not implicated. This follow-up changed only the certification test teardown.

## Correction

After bounded smoke and `disconnect_codex`, the test replaces the managed
`Persistence` with an inert persistence rooted outside the temporary directory,
drops the old persistence value explicitly, then drops the state handle and the
Tauri app before `remove_dir_all(root)`. No production API, runtime shutdown,
retry, sleep, forced termination, or ignored cleanup error was added.

The pre-validation inspection showed the existing Task 510B–510B2G production
WIP plus this test-only lifetime correction; no production runtime behavior was
changed. The candidate path, expected version, expected SHA, production
baseline, feature graph, manifest linkage, typed causes, and presentation
mapping were untouched by this correction.

## Focused evidence

The initial selector used the unqualified test name and ran zero tests (`0
passed / 0 failed / 380 filtered`), so it is not certification evidence. The
correct selector is
`certification_tests::task510b2_exact_candidate_desktop_smoke`.

The first correctly selected smoke reached the real candidate process, real
adapter, neutral runtime, Desktop composition, shutdown, and `inference=0;
Tool execution=0`, but cleanup failed with Windows error 32 at
`remove_dir_all(root)`. The first correction attempt still created the inert
replacement under the cleanup root and failed with the same lock. The source
was then corrected so the inert replacement is outside the cleanup root.

The next run reached the exact candidate checks and then failed the required
post-launch identity assertion before cleanup. Pre-launch identity was:

- path: `C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`
- expected/pre SHA: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`
- observed post SHA: `3e4520220a642fbd7c80dad7c99579afb5bb515b83fd2a0127d47b811d7f8f91`

The run reported the exact descriptor, wrong path, wrong hash, wrong version,
and production rejection checks before the post-identity failure. Because the
candidate changed, the run does not prove corrected cleanup, pre/post SHA
stability, or certification-seam completion. The installed artifact was not
restored or modified by this task.

## Gates not run after the stop

Repeat stable smoke, presentation regression, typed verification regression,
feature graph reconfirmation, wrong-hash/version/path controls as a complete
closure record, full deterministic validation, canonical Desktop, Tauri
inventory, metadata, HostExplicit, commit, push, and exact-head CI were not
run after this stop. Task 510B remains deferred.

ADR 0005 and ADR 0030 remain satisfied by the prior evidence. No authority or
security boundary changed. Final classification: **E — LATER DETERMINISTIC
VALIDATION FAILED**.
