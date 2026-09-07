# Task 240 — Windows Live Explicit Host Tool Invocation Certification

## Status

COMPLETE — WINDOWS LIVE CERTIFIED

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

Initial harness commit: `4e162987f139910baee19d4741a8c367e4ff9f5b`,
`test: add Windows host invocation live gate`; exact-head CI `34082521267`
passed. Test-only corrections were then committed as `791317f`, `0270fd1`,
and `a8c0658c590fa8f0ce1606a5262e6ac947076697`; the final harness head used
for live certification is `a8c0658c590fa8f0ce1606a5262e6ac947076697`.

## Harness exact-head CI

The final harness exact-head CI was `34083260988`, completed successfully for
the exact live-certified harness head. Earlier correction CI runs were
`34082788768` and `34083024251`, also successful before their respective
fresh pre-effect attempts.

## Live run

Pending the single certified command through `scripts/codex-live-gate.ps1`
with the pinned Codex version, SHA-256, model, and reasoning effort.

The first certified harness attempt reached the test binary but stopped before
Desktop connection publication: the real Tauri builder rejected Windows event
loop initialization from the Tokio test thread. No Codex connection, model
request, HostExplicit action, or branch effect occurred. The test-only harness
was corrected to enable Tauri's explicit any-thread test construction. This
requires a correction commit and exact-head CI before a fresh live attempt;
there is no automatic retry.

The next fresh attempt reached the real connected Desktop path but stopped at a
test-only baseline presentation assertion. The production status contract
reports `codex-cli 0.149.0` while the harness compared only `0.149.0`; no
eligibility, HostExplicit action, model request, or branch effect occurred.
The assertion is corrected without production changes and requires another
exact-head CI before the next fresh live attempt.

The following fresh attempt proved connection-current composition, both
eligible Tools, deferred Tool unavailability, the complete `repo.status`
HostExplicit lifecycle, and zero-effect branch Prepare. It stopped before
branch confirmation because the harness inverted the pre-prepare eligibility
assertion. No branch Started event or branch Tool effect occurred. The
test-only assertion is corrected and requires another exact-head CI before a
fresh effectful attempt.

The corrected fresh attempt passed the complete certification. It used the
exact command below once on Windows after correction commit CI passed:

```powershell
& .\scripts\codex-live-gate.ps1 `
  -Version '0.149.0' `
  -ExpectedSha256 '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00' `
  -Model 'gpt-5.6-terra' `
  -ReasoningEffort 'medium' `
  -Command {
      cargo test -p rah-desktop tests::windows_live_desktop_explicit_host_tool_invocation -- --ignored --exact --nocapture
  }
```

## Live evidence

The test emits bounded connection, eligibility, read, prepare, branch,
HostExplicit provenance, zero-model, provider-absence, branch-name, and OID
markers without absolute paths, environment, credentials, tokens, profile
paths, Codex home, or Git executable paths.

Observed successful markers included:

- `RAH_HOST_EXPLICIT_LIVE_CONNECTION_CURRENT=1`
- `RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_ELIGIBLE=1`
- `RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_STARTED=1`
- `RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_COMPLETED=1`
- `RAH_HOST_EXPLICIT_LIVE_BRANCH_ELIGIBLE=1`
- `RAH_HOST_EXPLICIT_LIVE_BRANCH_PREPARED=1`
- `RAH_HOST_EXPLICIT_LIVE_BRANCH_EFFECT_BOUNDARY=1`
- `RAH_HOST_EXPLICIT_LIVE_BRANCH_STARTED=1`
- `RAH_HOST_EXPLICIT_LIVE_BRANCH_COMPLETED=1`
- `RAH_HOST_EXPLICIT_LIVE_MODEL_TURN_STARTED=0`
- `RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_REQUESTED=0`
- `RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_STARTED=0`
- `RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_FINISHED=0`
- `RAH_HOST_EXPLICIT_LIVE_OK`

The successful fixture values were branch
`rah-host-explicit-live-18d2efa09d330900-2` and OID
`e6b376c26b0d97e12c1be2c7981aecc32974e29c`. The live test independently
verified the exact target ref, no-upstream state, ADR 0020 reflog identity and
message, unchanged protected Git planes, preserved reviewed authorization,
unchanged generations and conversation namespace, no refresh event, and
connection currentness. Cleanup succeeded after all assertions.

## Live result

PASS. The certified environment reported Windows NT `10.0.19045.0`, Git
`2.54.0.windows.1`, Codex `0.149.0`, the pinned executable SHA-256, model
`gpt-5.6-terra`, and medium reasoning. The production connected-current
Desktop path certified `repo.status` and `repo.create-branch` through typed
HostExplicit commands, D2 revalidation, normal ToolRegistry dispatch, and
existing capability authority. No model turn, AgentRequest, prompt, model Tool
lifecycle, fake AgentEvent lifecycle, provider activation, retry, rollback, or
branch deletion occurred.

HostExplicit provenance was `Started -> tool_completed` for `repo.status` and
`prepared -> Started -> tool_completed` for `repo.create-branch`; each Tool
executed exactly once.

## Closure

Live-pass documentation commit: `8f0efce5d37f7e133ea7386b81d960f7d9a7ec83`,
exact-head CI `34083496689` passed. Final closure commit:
`docs: close Task 240 Windows host invocation certification`.

The allowed certification claim is: Windows Desktop connected-current explicit
Host Tool invocation is certified for `repo.status` and `repo.create-branch`
using the production HostExplicit backend path. No model request or model Tool
lifecycle was used.

Explicit nonclaims remain: model-selected Tool dispatch was not exercised by
Task 240; real GUI mouse-click automation was not certified; arbitrary eligible
Tool, external provider, `repo.commit`, and Linux/macOS live behavior were not
certified. Task 229's model-selected `repo.create-branch` limitation remains
historically unchanged, as does Task 207. Task 241 has not started.

## Next task

Task 241 — RAH v0.20 Explicit Host Invocation Milestone Audit; not started.
