# Task 503B — Clippy correction and final validation

Starting HEAD: `904a781f928bc8fa5d592f5db92bd58cdcede400`.
Local HEAD and GitHub master independently matched. Existing Task 503 WIP is
preserved; no commit existed at entry. Historical 503/503A logs remain intact.

## Preserved evidence and plan

Phase B: 119 passed, 0 failed, 1 ignored, exit 0. Phase C: 3 passed,
0 failed, exit 0. Workspace: 1042 passed, 0 failed, 24 ignored, exit 0.
Formatting, workspace check and diff check passed. The accepted E0282
`JoinSet<((SessionId, String), Value)>` annotation remains unchanged.
These gates are not rerun merely to reproduce evidence.

Correct only the two reported collapsible_if sites; run warnings-denied
workspace Clippy first. Then canonical Windows Desktop, frontend/static,
Tauri inventory, metadata and static/executable HostExplicit gates. Only after
those pass, compare production and neutral fixture behavior for discovery,
start, streamed events, Tool round trip/correlation, cancellation, shutdown
and typed failures. Classification A alone permits commit and normal GitHub
master push, followed by natural exact-head CI. No Desktop migration.

## Exact corrections

At the original experimental.rs:435, combine `if let Some(route) =
state.active.take()` with `&& route.accepting.load(Ordering::SeqCst)`.
The outer expression does have an existing state-removal effect: it occurs
exactly once before the accepting check in both forms, including when false.
No effect is added, removed or reordered. The route borrow/move and cancel
await stay inside the same successful branch; thread clearing and errors stay
unchanged. This explicitly qualifies the requested outer-condition inspection.

At original :564, combine the successful JoinSet result match with
`&& let Some(entry) = calls.get_mut(&key)`. Matching has no external effect;
the mutable lookup still occurs only after a successful result. The borrow
remains scoped to the same body; shared upgrade, reply draining and result
retention are unchanged. No evaluation-order or lifetime change.

## Current results

Clippy: `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
exit 0; log `F:/temp/rah-task503b-clippy.log`. Source frozen during execution.
No unrelated Clippy findings or source changes.

Frontend: all five existing suites passed, including Edge layout and model
preflight; all actual frontend JavaScript files pass `node --check`.
An initial extra syntax-check invocation named nonexistent app.js; corrected
to actual files, without changing source or rerunning successful suites.
Tauri inventory: 47 runtime/manifest/generated/default allows/frontend, PASS.
`cargo metadata --no-deps --format-version 1`: exit 0, 13 packages;
JSON retained at `F:/temp/rah-task503b-metadata.json`.
Static HostExplicit: 11 enum variants and 11 host_kind entries, PASS.

Canonical Windows Desktop gate: PASS, exit 0; 329 passed, 0 failed,
20 ignored. Command: `scripts/windows-desktop-test-gate.ps1 -TargetDirectory
'F:/temp/rah-task503-target' -OutputDirectory 'F:/temp/rah-task503b-desktop'`.
Gate evidence directory:
`F:/temp/rah-task503b-desktop/20261003-123706-735-6515a9777e674016994c226c55c79ec6`.
The gate includes `host_invocation::tests::host_allowlist_is_exact`, PASS.
Separate explicit execution of that test: 1 passed, 0 failed, exit 0;
log `F:/temp/rah-task503b-hostexplicit.log`. HostExplicit = exactly 11.

## Post-gate parity comparison

Each command uses `cargo test -p rah-runtime-codex --lib FILTER`.
Logs: `F:/temp/rah-task503b-parity-FILTER.log`, with colons replaced by
underscores. All seven commands exit 0; aggregate 33 passed, 0 failed,
1 ignored (the existing optional live probe).

| Filter | Passed / ignored | Comparison |
| --- | --- | --- |
| `catalog::tests::` | 5 / 0 | Existing bounded catalog and typed discovery failure, also shared directly by neutral discovery |
| `runtime_tests::` | 18 / 1 | Production thread start/resume, streamed events, cancellation, shutdown and Task 498 typed failure envelopes |
| `string_and_integer_request_ids_route_echo_through_registry` | 1 / 0 | Production Tool round trip, live event counts and response IDs |
| `duplicate_call_executes_once_and_reuses_the_response` | 1 / 0 | Concurrent duplicate request identity, one effect and correctly reused replies |
| `cancellation_drops_pending_call_and_rejects_late_duplicate` | 1 / 0 | Production cancellation and late-call refusal |
| `disconnect_cancels_pending_execution_without_replay` | 1 / 0 | Production disconnect without replay |
| `experimental::tests::` | 6 / 0 | Neutral discovery/start/replay/native continuation/stream, concurrent distinct calls with duplicate correlation, cancellation/shutdown, stale handles and typed failures |

Comparison PASS: neutral uses existing catalog, restricted thread/input,
event-stream and Tool-output helpers. Production owns per-call spawned tasks;
neutral owns concurrent JoinSet workers. Neutral barrier fixture proves two
distinct calls execute concurrently, IDs 71/73 share call-a output and ID 72
gets call-b output, with exactly two effects and two of each live Tool event.
Production duplicate fixture proves two queued requests reuse one effect.
Provider IDs remain private; lifecycle adaptation uses neutral host identities
without changing production routing. Cancellation preserves completed uncertain
effects without replay; shutdown revokes retained handles. Typed sources remain
recoverable and sanitized at projection. This is deterministic production-path
fixture comparison, not a live paid-model or migrated Desktop certification.

## Final disposition and publication

**A — CODEX ADAPTER CONFORMS TO NEUTRAL RUNTIME CONTRACTS**

All required deterministic and parity gates pass. No workspace test rerun;
only the two source-equivalent lint edits occurred. `cargo fmt --check` and
`git diff --check` pass after correction. Task 503 is complete and authorizes
the coherent implementation/report commit and normal GitHub master push.
No tag, release, version bump, mirror contact or Desktop migration.
Commit SHA, push equality, natural exact-head CI run/result and clean worktree
will be recorded in the final return and `F:/temp/rah-task503b-publication.json`;
the committed report cannot contain its own final commit SHA.
Suggested next task only if separately authorized: Desktop migration assessment.

## Authority and production isolation

Task 503B changes only two source-equivalent conditionals and task reports.
No dependencies, ADRs, permission, authority, neutral contract or host port
changes. `git diff --quiet HEAD -- crates/rah-desktop Cargo.toml Cargo.lock`
passes. Production construction, connection lifecycle, repository switching,
Task 499 model preflight and Codex baseline admission remain unchanged.
