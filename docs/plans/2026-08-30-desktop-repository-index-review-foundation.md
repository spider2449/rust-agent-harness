# Task 147 — Desktop Repository Index Review Foundation

## Starting point and boundaries

Starting `HEAD` and `origin/master`: `27e9247bd3653240c679f95d775a2884c47ec11b`.
Task 145 decisions are preserved: Stage and Unstage are `HUMAN_HOST_ACTION`s,
successful `repo.diff-staged` review is complete within its bounded contract,
overflow fails closed, and binary staged content is refused for v0.12 review
authorization. This task adds no commit authority, `RepositoryCommitControl`,
`repo.commit`, permission level, generic Git command, dependency, ADR, profile,
or Generic Tool Bridge change.

## Implementation route

Desktop retains a Rust-only in-memory repository workflow catalog. Every
snapshot replaces it with a new observation generation. An opaque `index-N`
selector is associated with the canonical selected repository generation, the
observation generation, one host-observed regular target, and its bounded
metadata identity. The frontend receives only that selector and submits it to
`repository_stage_action` or `repository_unstage_action`; it never supplies a
path that can create authority. Each command validates and consumes the
selector, rechecks target identity, constructs the existing public
`GitStageTool`/`GitUnstageTool`, executes exact `{}` once, invalidates all
workflow state, and refreshes observations.

Stage is offered only for a current tracked, non-conflicted, existing regular
file with worktree changes for which `GitStageTool::new` admits the target.
Unstage is offered only for a current tracked, non-conflicted staged regular
modification admitted by `GitUnstageTool::new`. There is no Stage All,
Unstage All, looped operation, generic argv, or model registration.

The existing authority cannot stage untracked files, including a file created
by `repo.create-file`, because that tool intentionally preserves the absent
index state. The UI displays this as non-actionable. A deleted tracked target
also receives no Stage action because the existing tool requires an existing
regular file.

## Review foundation

The snapshot displays normalized staged files and patches from the existing
`repo.diff-staged` observer. It has `no_staged_changes`, `review_available`,
`review_binary_unsupported`, and `review_unavailable` presentation states.
Binary metadata remains displayable but is not authorizable in v0.12. Failed,
overflowed, malformed, or otherwise unsuccessful observation creates only a
bounded unavailable review state, never a partial successful review.

For a complete textual review, Rust also retains a nonserialized descriptor:
repository/observation generations and a SHA-256 over the ordered normalized
staged review, with complete/binary flags. It is a stale-review descriptor,
not authorization material; it is absent from SQLite, transcript, restart,
and frontend data. Snapshot/action state is conservatively invalidated on
every refresh, selection replacement, and every index attempt.

## Verification plan

Focused Desktop coverage exercises one observed tracked stage and unstage,
worktree preservation, selector consumption, refreshed review states, and the
unchanged model registry. Local validation includes Desktop, rah-tools,
workspace, strict Clippy, metadata, frontend syntax, diff, and Windows manual
smoke gates. No commit is authorized or attempted by this task.
