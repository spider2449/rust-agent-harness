# RAH v0.13 Release Gate

**RELEASE PREPARATION — NOT YET RELEASED**

## Release identity

- Release candidate: `RAH v0.13.0`.
- Starting HEAD: `0de2b2839fbd03d648ebbf2ba3f7d470d94ce794`.
- Starting exact-head CI: run `33586765389` — PASS.
- Task 164 milestone audit: PASS; v0.13 release preparation may begin.
- Release-preparation commit: pending.
- Exact-head CI for the release-preparation commit: pending.
- Tag: not created.
- GitHub Release: not published.

## Completed milestone scope

| Task | Status | Scope |
| --- | --- | --- |
| 157 | COMPLETE | v0.13 scope roadmap |
| 158 | COMPLETE | Deletion authority research |
| 159 | COMPLETE | Accepted ADR 0017 |
| 160 | COMPLETE | Bounded `repo.delete-file` core implementation |
| 161 | COMPLETE | Trusted composition and Generic Codex Tool Bridge |
| 162 | COMPLETE | Desktop selected-repository integration |
| 163 | COMPLETE | Windows live validation |
| 164 | COMPLETE | v0.13 milestone audit — PASS |
| 165 | RELEASE PREPARATION | Version, documentation, gate, validation, commit, push, exact-head CI |

## v0.13 authority and capability boundary

`repo.delete-file` is a separate ADR 0017 host-owned authority. It deletes
exactly one explicitly named repository-relative clean HEAD-tracked regular
file whose raw worktree bytes match the exact authorized HEAD blob preimage,
including SHA-256 and byte length. It performs one native deletion attempt and
does not auto-stage. It does not grant rename/move, directory or recursive
deletion, arbitrary untracked deletion, generic `fs.write`/`fs.unlink`,
generic shell/process, generic Git, automatic staging or commit, branch/ref/
history/network Git, or rollback/replay authority.

The public model request is not authorization. Execute is only an outer
dispatch permission. Trusted Profile composition cannot manufacture the
separate deletion authority, and the frontend is not authority. The Generic
Codex Tool Bridge uses the canonical public name `repo.delete-file`; a private
alias is only a provider translation detail. The bridge discoverability fix
for aliased tools is generic and does not widen authority.

## Windows live-certified evidence

Task 163 recorded the following successful live facts using exactly
`codex-cli 0.149.0`:

- Public tool: `repo.delete-file`.
- Private alias observed in this run: `rah_tool_4`; this is not a stable API or
  guaranteed alias.
- `ToolRequested = 1`, `ToolStarted = 1`, `ToolFinished = 1`.
- The exact intended target was deleted; the sentinel was unchanged.
- The deletion was unstaged; the Git index was unchanged.
- `HEAD`, refs, and history were unchanged; no replay occurred.
- `RAH_REPO_DELETE_FILE_LIVE_OK` was observed.

The chronology also preserves the initial aliased-tool discoverability
failure, which occurred before a tool call and led to the generic canonical
public-name description fix. It preserves the later CRLF/raw-byte
`precondition_failed` observation as a successful fail-closed refusal, not as
a deletion.

In the successful evidence, `tool_finished.result` may appear `null` because
the helper did not capture `ToolContent::Json` on that live path. The JSON
result was not captured. Task 164 classified this as non-blocking observability
technical debt; it is not fixed in release preparation.

Evidence terminology is precise: deterministic validation is established on
Windows and Ubuntu/Linux where CI/tests provide evidence; Windows is the
live-certified platform; Linux live certification is not yet established.
Equivalent Linux or macOS live validation is not claimed.

## Preserved limitations

- Task 120 remains **DEFERRED / NOT VALIDATED**.
- Transport confinement remains **NOT CLAIMED**; no network isolation claim is
  made.
- Process supervision is not OS sandboxing.
- Uncertain external effects are not automatically replayed or rolled back.
- Rename/move, directory/recursive deletion, arbitrary untracked deletion,
  generic filesystem/shell/process/Git authority, staging/commit automation,
  and branch/ref/history/network Git remain out of scope.

## Release-preparation checklist

- [x] Workspace packages are `0.13.0`; edition remains `2024`.
- [x] Cargo.lock contains only corresponding workspace-local version changes.
- [x] Release-specific local checks pass, including the Desktop release build;
  the exact native `codex-cli 0.149.0` baseline test/verify was not runnable
  because only native `0.152.1` is installed (the recorded Task 163 baseline
  remains the live evidence).
- [ ] Release-preparation commit created and pushed.
- [ ] Local `HEAD == origin/master` after push.
- [ ] Exact-head CI for the release-preparation commit passes.

## Not yet done

- v0.13.0 tag creation.
- GitHub Release publication.
- Post-release cleanup.

Task 166 is the separate tag and release-publication task after the exact-head
CI pass. No tag, GitHub Release, or release artifact publication is part of
Task 165.
