# Task 514E — Native cancellation terminal semantics

Starting HEAD/local origin/live master: `44a039c02ff5486decc86603f7fe210efcbf1943`.
Tracked checkout clean; preserve both untracked reports and target/task514d.

1. Reproduce typed native cancellation through production Desktop composition,
   retaining the failed-before-fix result.
2. Correct Desktop terminal translation with typed source and generation-scoped
   accepted cancellation state; preserve terminal ownership and Tool uncertainty.
3. Test both orderings, stale/duplicate/late cancellation and genuine failures.
4. Run focused tests, native/neutral/canonical/frontend/workspace quality gates.
5. Build normal production Desktop and reuse owned WebView2 acceptance against
   the existing llama.cpp server; correlate frontend and backend terminals.
6. Only after deterministic and actual acceptance, commit/push and verify
   exact-head CI. No release, tag, version bump or Task 515 work.

ADRs 0032/0033 govern typed sources and revocable host lifetime. No neutral
category, dependency, transport, Tool or authority changes are planned.

## Deterministic correction and control

The starting production run_chat path projected a source-bearing
OpenAiAdapterError::Cancelled into generic ChatRuntimeFailed and claimed the
terminal before cancel_chat. The final production-composition regression fails
on the isolated starting checkpoint with the actual payload:
`{"kind":"failed","code":"chat_runtime_failed","diagnostic":{"kind":"operation","operation":"turn"}}`.
Evidence: target/task514e/unchanged-pre-fix.{stdout,stderr}.log and exit 101.
Only the regression test was added to that detached checkpoint; its body is
identical in both checkouts (SHA256
`3f1c3658532ee66f1e84a2de8e954940aa5dc0ca38dde54a181f40fa766697b9`).
The earlier reproduction and fixture-development failures also remain preserved.
The recovery fixture needed matching delta/final output under the existing strict
protocol contract; two later fixture assertions needed the existing serialized
credential code and repository refresh event name. No production protocol or
uncertainty assertion was weakened.

Desktop now recognizes only the retained native Cancelled source at Turn,
combined atomically with accepted cancellation on the current generation/runtime/
session and the existing one-shot terminal claim. Duplicate requests fail closed.
Native successful cancel replies defer to the runtime terminal event because
the reply may also mean AlreadyTerminal; this preserves queued completion and
genuine failure outcomes. Legacy fallback and hard recovery remain unchanged.
Typed sources and neutral diagnostic categories remain intact.

Six focused tests PASS (target/task514e/focused-complete.*): original production
failure/recovery, actual command return before terminal consumption, both claim
orderings/duplicates/late requests/stale reconnect and revoked old port, ten typed
source controls including source-free Operation and Shutdown, production network/
protocol/rejection after an accepted request, and uncertain Tool effect refresh/
retained ownership. Native cancellation emits ChatEvent::Cancelled without a
failure diagnostic; existing frontend text is already `Chat was cancelled`.
No frontend rewrite is needed. Terminal emission evidence records event kind,
session generation and connection generation for actual UI correlation.

## Validation and production acceptance

| Gate | Result |
| --- | --- |
| Focused final cancellation tests | 6 PASS, 0 failed |
| Native adapter | 23 PASS |
| Neutral runtime | 18 PASS |
| Canonical Desktop | 339 PASS, 13 ignored, no watchdog timeout |
| Frontend suites | All 9 PASS |
| Tauri permission inventory | PASS, 49 commands |
| Workspace | 1,078 PASS, 18 ignored, 0 failed across 60 suites |
| cargo fmt --check | PASS |
| cargo check --workspace | PASS |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS |
| Normal cargo build -p rah-desktop --release | PASS |
| git diff --check | PASS |

The first Clippy invocation stopped at question_mark in the new Option-returning
helper (exit 101). Its evidence remains in clippy.* and validation.*. Replacing
the equivalent let/else early return with `active.as_mut()?` corrected the lint
without suppression or behavioral change. All six focused tests, fmt, check and
Clippy passed again on final source (focused-after-lint.*, *-after-lint.*, and
final-validation.*). The previously passed broad suites remain preserved; no
assertion, category or runtime failure was weakened.

Actual Windows acceptance reused Task 514D's owned native-window/WebView2 CDP
method and the existing llama-server (PID 16436); no other automation framework,
server restart or server configuration change was introduced. Normal production
rah-desktop.exe SHA256:
`940A8B89E55A074296ECED1262D28C3F99CC42A2F8EB8C1D80FD978D3E2DF2F3`.
The prior Task 514D executable was copied and hash-verified before replacement:
target/task514e/task514d-checkpoint-desktop.exe matches its original launch hash
`69B6947A5991F5BE1A8FC923082D3ECCD3A1357E48E1ED04A9869C9ED155F2CB`.

Owned production PID 15324 displayed the actual RAH window. The long response
had streamed more than 100 characters and Cancel Turn was active before clicking.
The live frontend then recorded `Chat was cancelled`, chatRunning=false,
promptDisabled=false, Send restored, and Disconnect enabled. Backend identified
typed native cancellation and emitted exactly one cancelled terminal for session
1/connection 1. No desktop_failure or Failed terminal occurred. The next prompt
returned RAH514E_CANCEL_RECOVERY_OK (session 2/connection 1). Disconnect/reconnect
returned RAH514E_RECONNECT_OK (session 3/connection 2), followed by a successful
Disconnect with provider controls enabled. No deltas occurred between the
cancellation terminal and next Started event.

Evidence: target/task514e/actual-ui/ui-evidence.json (live rendered error text and
frontend events), ui-host-events.jsonl (backend terminal generations),
adjudication.json (independent matching/count/negative assertions), ui-launch.json
(path/hash/creation/window identity), ui-listener-ownership.json, cancelled.png,
cancel-recovery.png and reconnect-recovery.png. cancelled.png captures the
stopped-stream/Send viewport; cancellation wording is captured by the actual
frontend snapshot rather than that viewport image. No mocked terminal or model
output was injected into the UI. The verified process accepted normal close and
HasExited=true; no production Desktop remained. cleanup.json retains the null
Get-Process exit-code observation without claiming exit status zero.

## Boundary and publication review

Changed files: main.rs (generation-bound cancellation and terminal translation,
native reply ownership and terminal evidence), runtime_selection.rs (typed native
source identification), main_tests.rs (five focused composition/ownership tests),
and this plan/report. No dependency edge, public API, neutral error category, ADR,
transport, model selection, credential, Tool definition, permission, HostToolPort,
repository/worktree authority, release metadata or version changed. HostExplicit
remains exactly 11; the canonical host_allowlist_is_exact test passed. Accepted
Task 514D presentation, 17 Tool details, native identity and repo.status round-trip
evidence remain unchanged and are not claimed as newly repeated tests.

Deterministic and actual Windows acceptance PASS. Commit/push and exact-head CI
closure are recorded in target/task514e/publication-closure.json. The two existing
untracked reports and all failed-before-fix evidence remain preserved. OpenAI
live API remains NOT VERIFIED; this task made no live OpenAI request. Cancellation
does not imply rollback of remote or Tool effects. Task 515 remains unauthorized;
no release preparation, tag or version bump belongs to Task 514E.
