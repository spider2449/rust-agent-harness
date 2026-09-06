# Task 229C — Host-Driven Windows Desktop repo.create-branch Live Effect Certification

## Result

Status: LIVE PASS — AWAITING EXACT-HEAD CI

Attempt 1 reached the real registry effect and produced the expected
branch/ref evidence, but exited 101 in the post-effect tracking assertion.
The assertion incorrectly included the expected new local branch in the
pre-existing-branch tracking comparison. No retry or rollback was performed;
that fixture remains preserved.

Attempt 2 used a fresh temporary repository, a fresh generated branch, fresh
host authority, fresh Desktop repository/ToolRegistry, and fresh connected
current state. The corrected harness completed the full host-driven live gate.

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`
- `HEAD == origin/master`: `bd3bdc861aa6111cd8ba495bd280be52e6878e4b`
- Starting commit: `docs: decide v0.19 branch live certification path`
- Worktree: clean at task start
- Exact-head CI: `34035336919` PASS
- Workspace: 13 packages, all `0.18.0`, edition 2024, no dependency drift

## Relationship to Task 229

Task 229 remains INCONCLUSIVE after two bounded model-driven attempts. Each
attempt observed `ToolRequested = 0`, `ToolStarted = 0`, and `ToolFinished = 0`;
the target was absent and no possible branch Tool effect occurred. This task
does not send a third model prompt and does not reinterpret host dispatch as
model dispatch. The final evidence must retain: MODEL-SELECTED TOOL DISPATCH
NOT ESTABLISHED.

## Environment portability

The test resolves the native Git executable, temporary fixture, Desktop
storage, and generated branch name dynamically. It does not require a fixed
drive letter, checkout path, username, profile directory, temporary root,
machine name, or Git installation path.

## Certified connected Desktop environment

The ignored live test reuses the certified Codex preparation and connection
path. The live gate is expected to provide Codex `0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning. No model turn is started by this test.

## Disposable repository

The test creates a disposable native Git repository with one committed,
attached HEAD and repository-local Git identity. It creates one unstaged
tracked-file modification and one staged tracked-file modification, then
captures semantic Git-owned state: symbolic HEAD, current branch, full HEAD
OID, porcelain status, index entries, unstaged and staged raw diffs,
tracking/upstream state, pre-existing local heads, tags, and remotes. Raw
`.git/index` bytes are not compared.

## Authority composition

The test constructs `RepositoryBranchCreationAuthority`, stores it through
`DesktopRepository::new_with_authorities`, selects that repository through the
Desktop seam, and requires the stored authority to be present.

## ToolRegistry dispatch

The test composes `desktop_tool_registry` and requires `repo.create-branch` to
be present. The live effect is invoked exactly once through the composed
registry using `ToolContext::default()`. It does not construct a standalone
branch Tool, call the private policy, call `git branch` to mutate, or invoke
`update-ref` directly.

## Effective Authority

The connected publication path is required to be current, and the actual
composed entry must remain public `repo.create-branch` from
`repository_host` / `desktop_repository`, with effect class
`repository_mutation`, authority category
`repository_local_branch_creation`, Execute permission, and repository
binding.

The live attempt emitted the authority, registration, and advertisement
markers before dispatch. It did not emit the final PASS markers because the
test stopped conservatively after the aggregate post-effect assertion failed.

Blocked-attempt chronology:

- host-driven registry dispatch count: 1;
- model request: none; no `runtime.start(...)` and no model lifecycle event;
- Tool output: `status = branch_created_verified`, `uncertain = false`,
  `is_error = false`;
- branch: `rah-host-live-18d2bea08400da38-2`;
- captured/new OID: `61a91f03c91f76941bd88876d756b28ef30b596c`;
- reflog identity/message: `RAH Host <rah-host@example.invalid>` /
  `RAH create local branch`;
- failure: test-only post-effect tracking comparison included the expected new
  branch instead of comparing only pre-existing branch mappings;
- effect known: the branch creation effect was verified before the assertion;
- retry before checkpoint: none;
- rollback: none;
- deletion: none;
- preserved fixture: yes.

## Reviewed commit authorization

The existing Desktop repository-review and `RepositoryCommitControl` path
must establish a valid pending authorization for the staged fixture state.
The test never invokes `repo.commit`, and branch creation must preserve the
pending authorization.

## Tool output

The actual registry `ToolOutput` is passed to the existing strict
`branch_result_classification` helper. The required result is
`branch_created_verified`, `uncertain: false`, `is_error: false`, the exact
generated name, and the captured pre-dispatch committed HEAD OID.

The live attempt reached this safely verified result before the later
aggregate assertion stopped the test. Observed values were:

- status: `branch_created_verified`
- uncertain: `false`
- is_error: `false`
- name: `rah-host-live-18d2bea08400da38-2`
- oid: `61a91f03c91f76941bd88876d756b28ef30b596c`

## Exact branch effect

After a safely verified result, native Git observation must show exactly one
`refs/heads/<generated-name>` pointing to the captured HEAD OID. Symbolic HEAD,
the checked-out branch, HEAD OID, semantic index state, staged and unstaged
diffs, status, tracking, pre-existing heads, tags, and remotes must be
unchanged.

The preserved fixture independently showed exactly one target ref,
`refs/heads/rah-host-live-18d2bea08400da38-2`, at
`61a91f03c91f76941bd88876d756b28ef30b596c`. The checked-out branch remained
`master`, with the same HEAD OID and the intended ` M nested/ordinary.txt` /
`M  tracked.txt` status.

## Reflog

The target reflog is read through Git's reflog formatting command and must
show the ADR 0020 message `RAH create local branch` and identity
`RAH Host <rah-host@example.invalid>`.

The preserved target reflog showed exactly:
`RAH Host <rah-host@example.invalid> RAH create local branch`.

## HEAD/index/worktree non-effects

The branch operation must not switch or checkout, mutate the worktree or
index, alter tracking, or produce a remote effect. The only expected
persistent repository difference is the new local branch and its permitted
reflog.

The failed comparison included the newly created branch in the complete
tracking listing; the pre-existing `master` tracking entry was unchanged.
The test-only helper now excludes exactly the generated target when comparing
pre-existing tracking state, matching the semantic requirement. The live
assertion separately requires the generated branch to have no upstream.

## Generation/currentness

Repository, model, Trusted Profile, and connection generations are captured
after connected publication and before dispatch, then required to be exactly
unchanged after dispatch. The connection must remain Connected/current and
the branch Tool must remain advertised. The test does not reconnect to obtain
a passing snapshot.

## Conversation/provider non-effects

The conversation persistence namespace and presentation are captured and
must remain unchanged. Trusted Profile activation must remain absent; no MCP
child, Process Plugin child, provider recomposition, or profile-generation
change is permitted.

## Cleanup

Only after all assertions pass, the connected runtime is shut down through
the existing Desktop shutdown seam and the disposable fixture is dropped.
If a post-dispatch result is not safely verified, the test stops without
retry or rollback and reports the fixture path where practical.

The connected runtime was shut down through the existing Desktop shutdown
path after the failed assertion. The disposable fixture was preserved at
`F:\Temp\rah-desktop-tool-registry-1788701595462217300-0`; it was not deleted
and the created branch was not rolled back.

## Fresh-fixture Attempt 2 — host-driven live PASS

The certified gate completed with:

- Codex `0.149.0`;
- binary SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
- model configuration `gpt-5.6-terra`, reasoning `medium`;
- one host ToolRegistry dispatch;
- no model request and `RAH_DESKTOP_BRANCH_MODEL_DISPATCH_ESTABLISHED=0`;
- `branch_created_verified`, `uncertain = false`, `is_error = false`;
- strict classifier `BranchActivityClassification::ProvenSafe`;
- `invalidate_review = false`, `refresh_reason = None`;
- all required authority, registration, advertisement, effect, ref, reflog,
  HEAD, index, worktree, review, generation, connection-current, and
  `RAH_DESKTOP_BRANCH_HOST_LIVE_OK` markers.

The generated branch name and captured OID were asserted internally by the
test's exact Tool output, target-ref, OID, reflog, and no-upstream checks, but
the successful test path did not print those two values. The fresh fixture was
dropped only after every assertion passed. No value is inferred here and the
effectful test is not rerun; the missing success-path echo is an evidence
capture limitation to retain in the historical record.

## Deterministic regression validation

PASS: `cargo fmt --check`; `cargo check --workspace`; focused branch tests
(29 passed); `cargo test -p rah-desktop` (180 passed, 4 ignored);
`cargo test --workspace`; `cargo clippy --workspace --all-targets
--all-features -- -D warnings`; `git diff --check`; metadata (13 packages,
all `0.18.0`, edition 2024); frontend syntax and authority tests; and
`cargo build -p rah-desktop --release`. The new test remains Windows-only and
ignored during ordinary test runs. After the live attempt, the corrected
test-only tracking snapshot helper was revalidated with Desktop tests,
clippy, formatting, and diff checks.

## Security boundary

This certification remains host-driven. The trusted host composes the
repository-bound authority and dispatches through the Desktop ToolRegistry.
Model output and provider metadata do not grant branch authority.

## Model-selected dispatch nonclaim

This test emits no model lifecycle claim. It does not call `runtime.start`,
construct a live `AgentRequest`, send a model prompt, or wait for model
`ToolRequested` / `ToolStarted` / `ToolFinished` events. A successful result
certifies only the Windows host-driven Desktop authority/effect path.

## Phase-1 checkpoint commit

The corrected helper and blocked evidence were committed as:
`test: checkpoint blocked host-driven branch live gate` (`56619f8`).

## Exact-head CI

Checkpoint exact-head CI passed as run `34037651448` for the exact pushed
checkpoint head. The first live attempt did not authorize replay of its
preserved fixture.

## Phase-2 pass commit

The pass documentation is to be committed as:
`docs: record host-driven branch live pass`.

## Next task

Task 230 — RAH v0.19 Local Branch Creation Milestone Audit — not started.
