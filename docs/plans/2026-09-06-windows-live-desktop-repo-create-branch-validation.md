# Task 229 - Windows Live Desktop repo.create-branch Validation

## Result

Status: INCONCLUSIVE — MODEL-SELECTED TOOL DISPATCH NOT OBSERVED AFTER TWO BOUNDED ATTEMPTS

## Starting checkpoint

- Origin: `spider2449/rust-agent-harness`
- SHA: `a7a4a7e111619d0c665816eb6e8d1e1938aeb536`
- `HEAD == origin/master`: verified before implementation
- Worktree: clean before implementation
- Task 228: complete; ADR 0020 remains authoritative

## Environment portability

The repository root and Git executable were resolved dynamically. The live
fixture and Desktop persistence storage were created under dynamically resolved
temporary storage. No username, drive letter, repository path, profile path,
authentication path, or executable path is committed here.

## Certified Codex gate

Executed through the existing `scripts/codex-live-gate.ps1` without changes:

- version: `0.149.0`
- binary SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`
- model: `gpt-5.6-terra`
- reasoning effort: `medium`
- config fingerprint: `24725811da1d226ffb98cef0614024072c97db4874748e9968d3217be67814b0`
- home: isolated temporary
- authentication mode: ephemeral auth-file copy from the existing gate
- MCP servers: 0; plugins/apps/code mode: disabled

## Disposable repository fixture

The ignored Windows test created a disposable native Git repository with one
committed HEAD, then established one unstaged and one staged modification. The
branch target was generated independently of user, machine, and repository
path. The fixture was preserved locally after the conservative live-test stop;
its path is not committed.

## Desktop authority composition

The test used the selected repository, native Git discovery, a real
`RepositoryBranchCreationAuthority`, `DesktopRepository`, and
`desktop_tool_registry`. Live output proved:

- `RAH_DESKTOP_BRANCH_AUTHORITY_PRESENT=1`
- `RAH_DESKTOP_BRANCH_TOOL_REGISTERED=1`

## Effective Authority advertisement

The test used the existing Desktop composition builder and connected/current
generation publication machinery. Live evidence proved the repository-host,
repository-mutation, `repository_local_branch_creation`, Execute,
repository-bound classification and connected-current advertisement, with
`RAH_DESKTOP_BRANCH_TOOL_ADVERTISED=1`.

## Reviewed commit authorization

The live test established a real staged review through the Desktop repository
workflow and armed the existing `RepositoryCommitControl`. It did not invoke
`repo.commit` before model dispatch was observed.

## Model prompt

The test sent one direct live prompt requesting exactly one
`repo.create-branch` call for a generated logical name, forbidding branch
switching and other mutating Tools, and requesting `RAH_BRANCH_LIVE_DONE`.

## Tool lifecycle

The certified model produced no `repo.create-branch` request. Observed branch
lifecycle counts were `ToolRequested=0`, `ToolStarted=0`, and
`ToolFinished=0`. Read-only inspection of the preserved fixture showed only
the original `master` local head, unchanged symbolic/current HEAD and OID, and
the intended staged/unstaged semantic state. No branch was created. This is
classified as model-selected dispatch INCONCLUSIVE under the zero-request
rule. The prompt was not replayed.

The initial live execution also exposed an over-strict test-only raw index-byte
assertion. That assertion was corrected in the ignored validation test without
changing production Desktop behavior. The corrected prompt was not rerun
automatically.

## Checkpoint and authorized second attempt

Attempt 1 was preserved at checkpoint commit
`f12871a8a5e1b048e147df9c9b2e3d65a0e3bd43` with exact-head CI run
`34034804437` completed successfully. The checkpoint preserved the corrected
test and the INCONCLUSIVE result; it did not replay the prompt.

One explicitly authorized second attempt was then run through the same
certified gate, with Codex `0.149.0`, binary SHA
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning effort. It again observed:

- Attempt 1: `ToolRequested=0`, `ToolStarted=0`, `ToolFinished=0`; target absent.
- Attempt 2: `ToolRequested=0`, `ToolStarted=0`, `ToolFinished=0`; target absent.

Both attempts established no possible Tool effect. The second attempt also
observed no other effectful `ToolStarted` event and no repository mutation. No
further retry was performed.

## Exact branch effect

Not established because no model-selected branch Tool request occurred. No
target ref or target reflog was observed.

## HEAD/index/worktree non-effects

The test captured symbolic HEAD, current branch, HEAD OID, porcelain status,
index semantic evidence, unstaged and staged raw diffs, tracking/upstream state,
pre-existing local heads, and tags/remotes. Read-only post-run inspection
showed `master` still checked out, the same committed HEAD,
` M nested/ordinary.txt` plus `M  tracked.txt`, and no additional local head.
No branch was deleted or rolled back.

## Generation/currentness

The live test connected the runtime through the Desktop publication path and
captured currentness/generation and conversation namespace values before the
model turn. No branch effect occurred. Full post-effect generation proof is
not claimed because model dispatch was not observed.

## Conversation/provider non-effects

No Trusted Profile was activated; no MCP or Process Plugin authority was used.
The test used the first-party Desktop registry only.

## Cleanup

The connected Codex runtime was shut down through the existing Desktop exit
shutdown seam. No provider activation was present. The disposable fixture was
retained locally for diagnosis; no branch deletion or rollback was attempted.

## Live result classification

INCONCLUSIVE — MODEL-SELECTED TOOL DISPATCH NOT OBSERVED AFTER TWO BOUNDED
ATTEMPTS. Deterministic Desktop authority, registration, and advertisement
evidence is established separately. There is no evidence of RAH mutation
failure; full model-selected Windows live execution was not established. The
live model did not request the first-party Tool on either attempt, and no
further retry is authorized by this task.

## Deterministic regression validation

PASS: formatting, workspace check, focused `rah-tools` branch tests (29
passed), Desktop tests (180 passed, 3 ignored), workspace tests, clippy with
`-D warnings`, diff check, metadata (13 packages, all `0.18.0`, edition 2024),
frontend syntax and authority tests, and Desktop release build. The live test
remains ignored in ordinary test runs.

## Commit

The corrected INCONCLUSIVE checkpoint was preserved in commit
`f12871a8a5e1b048e147df9c9b2e3d65a0e3bd43` with message
`test: record inconclusive desktop branch live gate`. This checkpoint records
attempt 1 as `0 / 0 / 0`, target absent, and no possible Tool effect. No
automatic replay was performed.

## Exact-head CI

Checkpoint exact-head CI run `34034804437` completed successfully before the
second attempt. No closure commit or closure CI was run because the second
attempt was zero-dispatch.

## Limitations / nonclaims

This task does not claim Linux live certification, branch switching, generic
Git/ref mutation, remote Git, rollback, race-free external exclusion, OS
sandboxing, network isolation, Trusted Profile branch authority, or provider
branch authority.

## Next task

Task 229C — Host-Driven Windows Desktop repo.create-branch Live Effect
Certification.

Task 230 — RAH v0.19 Local Branch Creation Milestone Audit — not started.
Task 229 remains INCONCLUSIVE until the milestone audit decides how the
model-selected dispatch limitation is represented.
