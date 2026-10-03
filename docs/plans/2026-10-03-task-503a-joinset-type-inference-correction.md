# Task 503A — JoinSet type inference correction

Starting HEAD: `904a781f928bc8fa5d592f5db92bd58cdcede400`.

Continue the existing dirty Task 503 worktree. All eight source hashes match
the preserved Task 503 inventory. Historical logs and inventory remain intact;
an additional tracked patch is retained at `F:/temp/rah-task503a-start-tracked.patch`.
No reset, clean, stash, rebase, implementation recreation or remote mutation.

## Inspection before correction

E0282 occurs at experimental.rs:548, `let mut workers = JoinSet::new()`.
There is exactly one worker spawn site. Its future returns
`((SessionId, String), serde_json::Value)` on every branch. The key is the
host session and private call identity; successful output_response and both
failure JSON branches all return Value. Worker output has no error parameter:
join_next supplies `Result<((SessionId, String), Value), tokio::task::JoinError>`.
The earlier response.clone() inside tokio::select! needs the unresolved second
tuple type before inference resolves the later spawn branch.

Intended annotation, recorded before editing:
`JoinSet<((SessionId, String), Value)>`.
This preserves existing host typed failures/events, provider response mapping,
worker concurrency, cancellation and abort/join shutdown. No result flattening
or neutral contract change is needed.

## Validation plan

Change only this annotation. Freeze source and run the exact Phase B command,
`cargo test -p rah-runtime-codex`, with isolated target
`F:/temp/rah-task503-target`; retain a new log. Stop on any failure.
If it passes, run Phase C host tests, then the original full deterministic and
production parity gates. Static HostExplicit enumeration remains 11.
Only classification A permits commit, normal GitHub push and exact-head CI.

## Results and classification

Applied only the recorded annotation at experimental.rs:548. No second type
annotation, formatting rewrite, contract or control-flow change was necessary.

| Gate | Result |
| --- | --- |
| `cargo test -p rah-runtime-codex` | Compilation complete (48.24s); exit 0; 119 passed, 0 failed, 1 ignored |
| `cargo test -p rah-runtime experimental_host::` | exit 0; 3 passed, 0 failed, 0 ignored |
| `cargo fmt --check` | PASS, exit 0 |
| `cargo check --workspace` | PASS, exit 0 |
| canonical Windows gate `-PrepareOnly` | PASS, exit 0; helper fixtures prepared, not a Desktop test gate |
| `cargo test --workspace` | exit 0; 1042 passed, 0 failed, 24 ignored |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | FAIL, exit 101 |
| closure `git diff --check` | PASS, exit 0 |

All six experimental adapter tests actually ran. Factory/instance, private
identity, discovery/default, continuation/replay, owned stream/control, live
Tool events, correlation, retained handles, cancellation/shutdown and typed
sanitized errors passed. Distinct concurrent calls have two effects; call-a
correlates to IDs 71/73 and call-b to 72, with two each Requested/Started/Finished
events. No artificial serialization was introduced.

Phase C proves revoked retained ports reject requests/admission, including after
scope drop; effect count stays at the prior admitted effect. Source checks
revocation before authorization/dispatch. Adapter stale-handle tests pass with
zero effects and no Started/Finished event. Host-owned execution survives a
dropped response future. Cancellation preserves the completed uncertain Tool
result, one effect, Cancelled/Stopped, and no automatic replay. Typed
CodexAdapterError and AuthorizedDispatchError sources pass assertions.

Exact later failure: `clippy::collapsible_if` at
`crates/rah-runtime-codex/src/experimental.rs:435:9` (conversation close) and
`:564:17` (router completion), promoted to errors by `-D warnings`.
Both nested conditionals existed in the preserved WIP. This is a deterministic
source lint/build failure, not infrastructure or a demonstrated runtime contract
or semantic failure. Stop applied; no fixes or retries followed.

All eight validation source hashes stayed stable. Inventory:
`F:/temp/rah-task503a-validation-source-sha256.json`. Historical log hash stays
`62F4312A1EAB799095E0BC5C8D18C9F7BF7FFB2D7B45F4C350327510F3BA1BC9`.
New preserved logs and SHA256:

- `F:/temp/rah-task503a-phase-b.log`:
  `08AD33421C4C1E554F801BA8C2FAD277D288123F120C78002008658FF0057721`.
- `F:/temp/rah-task503a-phase-c.log`:
  `3A17E33126DF545F3683AA9C00E9E21529995834BB5A893DDFACAF193791FD69`.
- `F:/temp/rah-task503a-workspace-test.log`:
  `42E76B9997C16BAF8A99FB026A960A7B0570EFA0037B7C2960FE568F1757B979`.
- `F:/temp/rah-task503a-clippy.log`:
  `CAFBDDFEC2DFF45757FA8822845A76E0DAB471D7A887388A5EEC42FD3FE80C86`.
- fmt/check: `F:/temp/rah-task503a-fmt.log` and
  `F:/temp/rah-task503a-workspace-check.log`.

Canonical Desktop test gate, frontend/static, Tauri permission inventory,
metadata sanity and later explicit executable HostExplicit verification:
NOT RUN after stop. Static HostExplicit is 11 variants and 11 host_kind entries.
Workspace Desktop tests passed; the later explicit gate is not declared
complete. Production regression tests passed alongside neutral fixtures;
separate post-gate production parity remains NOT RUN.

No Desktop production composition/model-preflight/baseline change, 0.160.0
admission or migration. No new dependency, ADR or authority change from 503A.
Adapters gain no registry mutation, repository selection, permission mutation
or HostExplicit authority.

**E — LATER DETERMINISTIC VALIDATION FAILED**

Task 503 remains incomplete. No commit, push, new exact-head CI, tag, release
or version bump. HEAD stays at the starting SHA. Worktree intentionally remains
dirty and unstaged with preserved WIP; build/log artifacts are outside the repo.
No cargo/rustc process remained after Clippy. Final clean worktree: NO.
Suggested next task: separately authorize disposition of the two exact Clippy
diagnostics and resume validation. No Desktop migration.

## Task 503B continuation — final current disposition

Historical stop classifications above remain preserved evidence. Task 503B
corrected only the two collapsible_if sites with Clippy's canonical let chains;
the accepted JoinSet annotation remains unchanged. Evaluation order, state
removal, mutable lookup, cancellation, response retention and typed errors are
preserved. No neutral redesign or Desktop migration occurred.

Clippy PASS (exit 0). Canonical Windows Desktop gate PASS: 329 passed,
0 failed, 20 ignored, exit 0. Five frontend suites and all actual frontend
JavaScript syntax checks PASS. Tauri inventory matches at 47 throughout.
Metadata PASS (13 packages). HostExplicit static variants/routes = 11/11;
canonical and explicit executable host_allowlist_is_exact PASS (1 passed).
Post-gate production/neutral parity: 33 passed, 0 failed, 1 ignored, all exit 0.
The previously successful Phase B/C/workspace evidence remains preserved;
workspace tests were not rerun. Formatting and diff checks pass after correction.

**A — CODEX ADAPTER CONFORMS TO NEUTRAL RUNTIME CONTRACTS**

Task 503 implementation is complete. No dependency, ADR, baseline, model
preflight, production composition or authority expansion. Classification A
authorizes the coherent Task 503 commit and normal GitHub master push; exact
commit/push/CI identity will be recorded in the final publication return and
external evidence, avoiding a second commit solely to record its own SHA.
Full corrections, commands and evidence paths:
[Task 503B](2026-10-03-task-503b-clippy-correction-and-final-validation.md).
Next task, if separately authorized: assess Desktop migration. It is not begun.