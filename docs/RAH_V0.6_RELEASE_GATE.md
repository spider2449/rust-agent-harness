# RAH v0.6 historical release gate

Status: **RELEASED**

Date: 2026-08-24

## Release record

- Version: `v0.6.0`
- Release-preparation commit:
  `6326c18937bbcfd1e515001692a2c88c6884d552`
- Immutable annotated tag object:
  `0a31db7ede796051a026e79187417c7759d349d3`
- Peeled tag target:
  `6326c18937bbcfd1e515001692a2c88c6884d552`
- Release-preparation CI: `32685119256` (`completed / success`)
- Tag CI: `32685443380` (`completed / success`)
- Codex baseline: exactly `codex-cli 0.149.0`
- GitHub Release: published — [RAH v0.6.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.6.0)
  (release ID `375426125`, published `2026-08-24T03:25:30Z`)

The GitHub Release names the existing immutable `v0.6.0` tag. Its authoritative
release target is the tag's peeled commit above; the release does not own a
separate mutable commit pointer.

## Milestone scope

v0.6 is repository-aware read-only workflow inspection through four ordinary
RAH Tools:

- `repo.file-info`
- `repo.status`
- `repo.diff`
- `repo.diff-staged`

The existing guarded `repo.patch` capability remains available, but it is a
separate worktree-content mutation authority under ADR 0012. Observer authority
does not merge with `repo.patch` authority.

## Authority and security contract

Observers use fixed commands against a host-selected, revalidated Git executable
and host-selected trusted repository. The host selects executable, repository,
argv, cwd, and environment. Model input cannot select arbitrary Git commands,
argv, cwd, environment, repository, revision, diff baseline, or executable.
Their environment is cleared and fixed by the host; output and execution are
bounded.

Paths are UTF-8 or base64 tagged without lossy decoding. Diff observers suppress
binary payloads, and all successful results state only `best_effort`
consistency. Conflicts and contradictory observations fail closed.

The observers are fixed-command host capabilities, not a generic Git API. They
allow no generic Git execution, arbitrary executable/argv/cwd/env, external
diff, textconv, pager/helper escape, or intentional repository mutation. They
provide no new mutation authority. `PermissionLevel::Execute` remains only the
outer host subprocess gate and does not grant mutation authority; a model
request is never authorization. Hardened execution and process supervision are
not OS sandboxing.

## Deterministic evidence

Tasks 059 through 064 established the deterministic observer foundation,
status, diff, trusted-profile composition, and Generic Tool Bridge verification.
Task 064 exercises all four canonical observers through the real composition
chain with deterministic aliases, permission admission, lifecycle behavior,
deduplication, cancellation-before-entry, no replay, response redaction, and
`repo.patch` route isolation.

The Task 066 milestone audit records required Ubuntu CI run `32684390117` as
`completed / success`. Ubuntu provides deterministic and cross-platform
coverage, not Unix live Codex validation.

## Live evidence

Task 065 ran the native trusted-profile observer bridge on Windows using exactly
`codex-cli 0.149.0`. Three fresh Windows runs each invoked `repo.file-info`,
`repo.status`, `repo.diff`, and `repo.diff-staged` exactly once, with one
requested/started/finished lifecycle per observer, terminal completion, and
cleanup. Each run preserved HEAD, refs, the raw index, tracked and untracked
fixture bytes, and staged and unstaged semantic state. The observers make no
intentional repository mutation. The exact marker was:

```text
RAH_REPOSITORY_OBSERVERS_LIVE_OK
```

Windows live gate: passed. Ubuntu deterministic validation: passed. Unix live
Codex validation is not claimed.

`repo.patch` is not rerun as a v0.6 live release gate: current release policy
requires the new milestone's critical live path, while v0.5.1 retains its prior
live evidence and deterministic regression coverage. This is regression
evidence separation, not shared authority.

## Historical audit and release checklist

Task 066 concluded **RELEASE READY** for this scope before publication. The
following records the completed release state.

| Check | Status |
| --- | --- |
| Workspace packages resolve to `0.6.0` | Passed: 11 packages |
| Local deterministic and focused release gates | Passed |
| Fresh Windows live observer gate at exact Codex baseline | Passed: 3 fresh runs |
| Release-preparation commit created | Passed |
| Required CI for release-preparation commit | Passed: `32685119256` |
| `v0.6.0` annotated tag created | Passed: immutable tag object `0a31db7ede796051a026e79187417c7759d349d3` |
| Tag CI | Passed: `32685443380` |
| GitHub Release published | Passed: `RAH v0.6.0` |

## Platform scope

- Windows live validated.
- Ubuntu deterministic validated.
- Unix live Codex validation is not claimed.

## Consistency limitations

Observer results retain `best_effort` consistency only. There is no
transactional snapshot guarantee; detectable contradictions fail closed, and
external actors may race with observations. The release makes no claim that
repository observation performs zero filesystem writes.
