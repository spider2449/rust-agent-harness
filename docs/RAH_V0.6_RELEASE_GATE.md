# RAH v0.6 release-preparation gate

Status: **RELEASE PREPARATION IN PROGRESS**

Date: 2026-08-24

## Release candidate

Target version: `v0.6.0`

Release-preparation commit: this commit; its full SHA is recorded in the Task
067 completion report.

Codex baseline: exactly `codex-cli 0.149.0`

The tag and GitHub Release are intentionally deferred to Task 068 after this
release-preparation commit is pushed and its required CI is green.

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

Observers use fixed commands against a host-selected, revalidated native Git
executable and trusted repository. Model input never selects argv, cwd,
environment, repository, revision, diff baseline, or executable. Their
environment is cleared and fixed by the host; output and execution are bounded.

Paths are UTF-8 or base64 tagged without lossy decoding. Diff observers suppress
binary payloads, and all successful results state only `best_effort`
consistency. Conflicts and contradictory observations fail closed.

The observers are fixed-command host capabilities, not a generic Git API. They
allow no generic Git execution, arbitrary executable/argv/cwd/env, external
diff, textconv, pager/helper escape, or intentional repository mutation.
`PermissionLevel::Execute` remains only the outer host-process gate and does
not grant mutation authority. Hardened execution and process supervision are
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
`codex-cli 0.149.0`. Three fresh fixtures each invoked `repo.file-info`,
`repo.status`, `repo.diff`, and `repo.diff-staged` exactly once, with one
requested/started/finished lifecycle per observer, terminal completion, cleanup,
and no repository mutation. The exact marker was:

```text
RAH_REPOSITORY_OBSERVERS_LIVE_OK
```

Windows is live validated. Unix live Codex validation is not claimed.

`repo.patch` is not rerun as a v0.6 live release gate: current release policy
requires the new milestone's critical live path, while v0.5.1 retains its prior
live evidence and deterministic regression coverage. This is regression
evidence separation, not shared authority.

## Audit and release checklist

Task 066 concluded **RELEASE READY** for this scope. It remains historically
accurate and is not rewritten by release preparation.

| Check | Status |
| --- | --- |
| Workspace packages resolve to `0.6.0` | Passed: 11 packages |
| Local deterministic and focused release gates | Passed |
| Fresh Windows live observer gate at exact Codex baseline | Passed: 3 fresh runs |
| Release-preparation commit created | Passed |
| Required CI for that exact commit | Pending |
| `v0.6.0` tag created | Deferred to Task 068 |
| GitHub Release published | Deferred to Task 068 |

## Platform scope

- Windows live validated.
- Ubuntu deterministic validated.
- Unix live Codex validation is not claimed.
