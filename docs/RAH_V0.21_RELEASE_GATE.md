# RAH v0.21.0 Release Gate

## Status

**RELEASED — HISTORICAL RECORD**

v0.21.0 is the current immutable published release. v0.20.0 is the prior
release.

## Release identity

- Task 252 audit commit: `563301f2627b50c5c256ad221c34a931b28aa1e2`.
- Task 252 exact-head CI: `34196931238` PASS.
- Task 252 verdict: **A — MILESTONE COMPLETE — RELEASE PREPARATION MAY BEGIN**.
- Task 253 release commit: `7faa13a16425cc9d3bdb0dc540923eae8685ba90`.
- Task 253 exact-head CI: `34198526278` PASS.
- Annotated tag: `v0.21.0`.
- Tag object: `aa178f05d12d881745932800e8fc40a7431ce7b9`.
- Peeled target: `7faa13a16425cc9d3bdb0dc540923eae8685ba90`.
- Tag CI: `34199221802` PASS.
- GitHub Release ID: `384519493`.
- Published: `2026-09-08T07:27:35Z`.
- Draft: `false`.
- Prerelease: `false`.

## Completed publication checklist

- [x] Task 252 audit completed with Verdict A and passing exact-head CI.
- [x] Task 253 release-preparation commit pushed with passing exact-head CI.
- [x] Annotated `v0.21.0` tag published with the recorded tag object.
- [x] Tag CI completed successfully for the immutable release tag.
- [x] GitHub Release `RAH v0.21.0` published as non-draft and non-prerelease.
- [x] Release source remains the recorded peeled target.

## Release scope

The v0.21 milestone is **HostExplicit Reviewed Single-File Patch Authoring**:

```text
inspect
 -> typed human repo.patch Prepare
 -> exact bounded review
 -> ticket-only Confirm
 -> existing repo.patch
 -> inspect diff
 -> existing Stage / Unstage
 -> existing reviewed Commit
```

ADR 0012 remains the sole underlying `repo.patch` worktree mutation authority.
ADR 0021 remains the general HostExplicit dispatch/currentness/D2 boundary. ADR
0022 adds only the reviewed workflow around that existing capability.

The exact seven-Tool HostExplicit set is:

1. `fs.read`
2. `repo.file-info`
3. `repo.status`
4. `repo.diff`
5. `repo.diff-staged`
6. `repo.create-branch`
7. `repo.patch`

The following are not HostExplicit eligible: `repo.create-file`,
`repo.edit-files`, `repo.delete-file`, `repo.rename-file`,
`repo.create-directory`, `repo.commit`, MCP Tools, and Process Plugin Tools.

## v0.21 security summary

- H1 human input is typed and limited to `path`, `expectedOldText`, and
  `replacementText`.
- The host derives SHA-256/length bindings and canonical `ToolInput`.
- Shared `RepositoryPatchPreparer` is non-effectful and performs no Tool
  execution or replacement.
- Review is exact R4, bounded, escaped, and contains no hidden
  mutation-relevant content.
- The ticket is opaque, process-local, single-use, and valid for five minutes.
  Confirm receives only the ticket ID.
- Shared preparation revalidation occurs before D2; D2 preflight occurs before
  `Started`; `authorized_tool_dispatch` occurs after `Started`.
- Existing `repo.patch` performs the mutation, with exactly one possible Tool
  execution and native replacement attempt.
- Tool results use a strict classifier. Malformed, uncertain, or post-start
  `ToolError` is handled conservatively.
- There is no retry, replay, rollback, restore-preimage, compensation, or
  automatic second confirmation.
- Patch review source text is absent from generic activity and persistence.
- Old reviewed-commit authorization is invalidated at the effectful boundary.
  Refresh may create a new `ReadyToAuthorize` review but never automatically
  authorizes it.
- Content mutation alone does not increment `repository_generation`.

## Task 251 Windows evidence

The certified claim is:

> Windows Desktop connected-current HostExplicit reviewed repo.patch was
> certified through the production backend.

- Prepare: 0 Tool executions, 0 native replacements.
- Confirm: 1 Tool execution, 1 native replacement.
- Result: `ChangedVerified`.
- Target: exact expected postimage verified.
- Protected repository state: semantic index, `HEAD`, branch, refs, and
  unrelated staged sentinel unchanged.
- Review: old authorization cleared; fresh `ReadyToAuthorize` review
  generated; no pending automatic authorization.
- Model involvement: `runtime.start = 0`, `AgentRequest = 0`, `prompt = 0`,
  `ToolRequested = 0`, `ToolStarted = 0`, `ToolFinished = 0`, lifecycle
  `0/0/0`.
- External providers: MCP `0`, Process Plugin `0`.

Certified baseline:

- Windows 10 build 19045.
- Git `2.54.0.windows.1`.
- `codex-cli 0.149.0`.
- SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Model `gpt-5.6-terra`, reasoning `medium`.

This Task 251 evidence is carried forward and is not rerun during Task 253.

## Task 251 chronology

1. Attempt 1 exposed a real pre-effect coordinator defect.
2. Task 251A fixed async Prepare reservation/finalization.
3. Attempt 2 performed the successful mutation but hit an over-strict test
   assertion; no retry or rollback occurred and the fixture was preserved.
4. Task 251C corrected only that assertion.
5. Attempt 3 was a fresh disposable-repository live run and passed.

Detailed evidence remains in the Task 251 plan and the Task 252 audit.

## Nonclaims

This gate claims no GUI mouse automation, model-selected `repo.patch`,
Linux/macOS live certification, other HostExplicit authoring Tool enablement,
external-provider HostExplicit, generic filesystem write, shell or generic
process authority, automatic Stage or Commit, rollback guarantee, race-free
TOCTOU guarantee, network isolation, or OS sandboxing. Process supervision is
not OS sandboxing. Staged, binary, new-file, delete, rename, directory, and
multi-file HostExplicit authoring are not claimed, and a refreshed review is
not automatically authorized.

## Publication boundary

The later Task 255 documentation cleanup is not the v0.21.0 release source and
does not change the tag, GitHub Release, or release commit. No live gate rerun
is part of Task 255.
