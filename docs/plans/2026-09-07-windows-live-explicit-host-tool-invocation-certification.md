# Task 240 — Windows Live Explicit Host Tool Invocation Certification

## Status

HARNESS READY — AWAITING EXACT-HEAD CI

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- `HEAD == origin/master == 42394778488ec04b6c146fb3661478cac569adcf`.
- Starting commit: `docs: record Task 239 final CI`.
- Task 239 exact-head CI: `34080431216` passed.
- Worktree was clean.
- Workspace baseline: 13 packages, all `0.19.0`, edition 2024, no dependency drift.

## Certification claim

This task certifies the production Windows Desktop connected-current explicit
Host Tool invocation path for `repo.status` and `repo.create-branch`.
It does not certify model-selected Tool dispatch, GUI mouse-click automation,
arbitrary eligible Tools, external provider invocation, `repo.commit`, or
Linux/macOS live behavior.

## Certified Codex environment

The live gate is pinned to Codex `0.149.0`, executable SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning effort.

## No-model contract

The test establishes the real Codex app-server connection and connected-current
Desktop composition only. It does not call `runtime.start`, construct an
`AgentRequest`, send a prompt, wait for model Tool selection, or replay model
behavior. Real model lifecycle counts remain zero, and HostExplicit activity is
not represented as `AgentEvent` Tool lifecycle.

## Live harness

One ignored Windows-only integration-style test uses one fresh Tauri Desktop
test instance, the actual `connect_codex` publication path, actual typed host
commands, and one fresh native Git fixture. The test records bounded
`host_activity_event` markers and uses no arbitrary ToolName/JSON command API.

## Fresh repository fixture

The fixture is dynamically created under the native temporary directory, is a
real Git repository with an initial committed attached branch and tracked
files, and receives one staged and one unstaged tracked-file modification.
Semantic Git observations include symbolic HEAD, current branch, HEAD OID,
index state, staged and unstaged diffs, porcelain status, local heads,
tracking, tags, and remotes. Raw `.git/index` bytes are not compared.

## Connected-current composition

The selected fixture is published through the production Desktop connection
path with no Trusted Profile, MCP, or Process Plugin activation. The test
requires the actual current repository, registry, allowed permission policy,
Effective Authority snapshot, and connection generations.

## Effective Authority eligibility

The live snapshot must show `repo.status` eligible with kind `repo_status`,
`repo.create-branch` eligible with kind `repo_create_branch`, and a registered
deferred Tool such as `repo.commit` unavailable for host invocation. The branch
entry must retain repository mutation, local branch creation, repository-bound,
and Execute classifications.

## repo.status HostExplicit path

The test invokes the production typed read command with the empty
`repo_status` request. It requires exactly one HostExplicit Started event and
one HostExplicit `tool_completed` event, exactly one normalized structured
status result, no prepared state, no second confirmation, and no Git,
currentness, review, conversation, or connection effect.

## repo.create-branch Prepare

Only after the read-only assertions pass, the test establishes a production
reviewed authorization where practical, generates a fresh valid branch name,
and invokes the production prepare command with `name` only. It requires an
opaque bounded ticket, exact sanitized review text, a still-absent target,
zero Tool execution, zero Git effect, and HostExplicit `prepared` activity.
The production model-start admission function is checked deterministically and
must reject while HostPrepared without starting a model request.

## repo.create-branch Confirm

The test records the possible-effect boundary and invokes production confirm
exactly once with the ticket ID only. It requires one HostExplicit Started and
one `tool_completed` event, the strict existing branch result classification,
the exact prepared name and pre-confirm HEAD OID, and a real native Git ref and
reflog observation. It never retries, confirms again, deletes the branch, or
rolls back.

## HostExplicit provenance

The observed sequence is `prepared -> Started -> tool_completed` for the
branch and `Started -> tool_completed` for the read. Event source remains
`host_explicit`; no model lifecycle event is synthesized or counted.

## Model lifecycle non-evidence

The live markers record `model_turn_started = 0`, `model_tool_requested = 0`,
`model_tool_started = 0`, and `model_tool_finished = 0`. This task does not
claim model-selected Tool execution.

## Branch effect / non-effects

The created ref must point exactly to the captured HEAD OID and carry the ADR
0020 identity/message. Symbolic HEAD, checked-out branch, HEAD OID, index,
staged and unstaged state, porcelain status, pre-existing heads and tracking,
tags, remotes, and upstream state remain unchanged; the new branch has no
upstream.

## Review / currentness

Verified branch creation must preserve reviewed authorization, report no review
invalidation or refresh reason, preserve all four generations and the
conversation namespace, retain the current connection, and keep the branch
Tool advertised and host eligible. The exact branch name is not invoked again.

## Provider non-effects

MCP active providers and Process Plugin active providers remain zero. No
Trusted Profile external provider child is spawned; host explicit invocation
does not activate providers.

## No-replay boundary

Before branch Started, a failure is a test-harness failure with no branch effect.
After branch Started, any failure preserves the disposable fixture and reports
the branch, known OID, observed result/ref/reflog state, and exact assertion;
there is no retry, rollback, or cleanup that removes evidence.

## Validation

Before harness commit, run the required sequential deterministic validation:
format, workspace check/tests/clippy, focused dispatch/runtime/Desktop tests,
frontend syntax/authority tests, Desktop release build, diff check, and
metadata. The ignored live test is not executed by these checks.

Completed successfully before the harness commit:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p rah-tools authorized_dispatch -- --nocapture` (15 passed)
- `cargo test -p rah-runtime-codex` (84 passed, 1 ignored)
- `cargo test -p rah-desktop` (185 passed, 5 ignored)
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `node --check crates/rah-desktop/frontend/status.js`
- `node crates/rah-desktop/frontend/status_authority_test.js`
- `cargo build -p rah-desktop --release`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1` (13 packages, all `0.19.0`,
  edition `2024`)

Scope audit passed: only the Windows test module in
`crates/rah-desktop/src/main.rs` and this plan are changed; `Cargo.toml` and
`Cargo.lock` are unchanged; no production Desktop module was modified; and no
effectful ignored live test was executed.

## Harness commit

Pending: `test: add Windows host invocation live gate`.

## Harness exact-head CI

Pending: push the harness commit and require successful CI for that exact head
before the live gate.

## Live run

Pending the single certified command through `scripts/codex-live-gate.ps1`
with the pinned Codex version, SHA-256, model, and reasoning effort.

## Live evidence

The test emits bounded connection, eligibility, read, prepare, branch,
HostExplicit provenance, zero-model, provider-absence, branch-name, and OID
markers without absolute paths, environment, credentials, tokens, profile
paths, Codex home, or Git executable paths.

## Live result

Pending. A full PASS requires all connected-current, eligibility, read,
prepare, confirm, exact effect, non-effect, review/currentness, provider,
provenance, zero-model, and cleanup assertions.

## Closure

Pending live-pass documentation commit and final documentation-only exact-head
CI. The final closure must state that model-selected Tool dispatch was not
exercised and that Task 229 and Task 207 limitations remain unchanged.

## Next task

Task 241 — RAH v0.20 Explicit Host Invocation Milestone Audit; not started.
