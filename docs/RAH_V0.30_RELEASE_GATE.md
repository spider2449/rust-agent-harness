# RAH v0.30.0 Release Gate

Status: **READY FOR PUBLICATION - NOT YET RELEASED**

Task 362 is the final v0.30 milestone audit and release preparation. It adds
no feature, Rust behavior, test/harness behavior, frontend behavior,
permission, ADR, persistence, or schema change. Publication is Task 363.

## Release identity and theme

Release theme: **Linked Git Worktree Repository Support**.

RAH Desktop can explicitly admit ordinary and supported linked Git worktrees as
distinct repository members, even when they share one Git common directory.
Exactly one repository member remains active at a time. Repository effects stay
bound to the selected worktree's identity and capability-specific Git state.

The current immutable published release is RAH v0.29.0. No `v0.30.0` tag or
GitHub Release is created by Task 362.

## Milestone audit: Tasks 356-361

| Task | Accepted milestone result |
| --- | --- |
| 356 | Product roadmap selected linked Git worktree membership. Its provisional `NEW AUTHORITY CATEGORY` label is historical and was reconsidered. |
| 357 | Git/identity research reached Decision B: **NARROW IDENTITY EXTENSION**; shared common state is not sibling authority. |
| 358 | ADR 0029 was accepted as one closed linked-worktree identity form under ADR 0027, with no new executable authority category. |
| 359 | Production implementation established one `RepositoryGitLayout` model across admission, observation, Stage, Unstage, Commit, branch, and authoring conformance. |
| 360 | Independent audit passed with narrow hardening: relative linked records, production/test admission parity, root-owned Stage hooks, and activation stale-publication protection. |
| 361 | Windows live certification passed with explicit nonclaims and emitted `RAH_V030_LINKED_WORKTREE_LIVE_OK`. |

No unresolved mandatory milestone blocker was found. The accepted v0.30
product decision is not reopened.

## Authority and ADR status

Authority classification: **NARROW EXTENSION**, not `NEW AUTHORITY CATEGORY`.
ADR 0027 remains authoritative for repository identity, membership, one-active
composition, switching, and repository-bound authority. ADR 0029 is Accepted
and adds one closed linked-worktree identity form. ADR 0028 restart and
remembered-candidate rules remain unchanged. Existing mutation and Commit ADRs
remain authoritative. No ADR amendment is required in Task 362.

HostExplicit remains exactly 11. No PermissionLevel, worktree mutation
authority, Stage/Unstage authority, Commit authority, generic Git authority,
generic filesystem authority, Tool authority, model routing authority, or
provider authority was added.

## Supported identity and behavior

An ordinary main worktree has a real directory at `root/.git`. A supported
linked worktree has a bounded `.git` gitfile, private worktree Git directory,
standard `commondir` relation to shared common state, backlink to the selected
root's `.git`, and exactly one matching native registration. Absolute and
supported relative link spellings are accepted only when all canonical,
filesystem, and fixed read-only Git semantic evidence agrees.

Same common Git directory is not duplicate identity and does not merge
authority. Duplicate identity is the same selected root identity or the same
validated private worktree target. Main, linked A, and linked B can remain
distinct members. There are zero or one active members, one active-only
ToolRegistry, and no model/provider worktree selector.

Admission and activation are host-owned, fail-closed, and revalidate identity
and currentness before publication. Observation is selected-worktree scoped.
Stage/Unstage target only the selected private index. Reviewed Commit binds the
selected identity, private index, HEAD, attached branch, and expected selected
branch OID. Unrelated sibling common-state changes do not globally stale a
selected preparation; selected-ref movement does stale the reviewed Commit,
with zero native Commit spawn and no retry or rebinding.

RAH recognizes an existing valid worktree identity. It does not create,
remove, prune, repair, move, lock, or unlock Git worktrees. External native
move/removal or root `.git`, backlink, `commondir`, or registration mutation
makes retained identity stale and requires fresh explicit admission.

## Explicitly unsupported and privacy-bounded forms

Arbitrary, copied, fabricated, or malformed gitfiles; stale/prunable
registrations; submodules; `--separate-git-dir`; bare repositories; unsupported
nested relationships; and unsupported symlink/reparse-mediated identity are
rejected. Relative spelling does not imply generic gitfile support.

Private linked evidence remains host-private: private/common paths,
registration paths, gitfile/backlink/commondir bytes, registration IDs, and
filesystem identities are not exposed through Tool input/schema, Effective
Authority, generic Activity, model/provider state, remembered persistence, or
unbounded errors. Restart restores no executable repository membership or
authority.

## Certification record

| Record | Identity / result |
| --- | --- |
| Hardened production behavior | `a684405ecd0143093fba669b9b52e84bc32f7590` |
| Final deterministic/audit tree | `3771a52227d2ef85944e971a2a0dc9c0e5d5856f`; CI `35352376409` - PASS |
| Windows-certified source | `93a522c2a42b438d539d296ba92cbb9dd1eea297`; CI `35420903926` - PASS |
| Certification docs head | `1ed6380c6b5ebb6e57baa9c253a0923289c05470`; CI `35421216647` - PASS |
| Task 362 release-preparation source | This Task 362 commit; its SHA is reported after commit and is not embedded in its own commit. |
| Marker | `RAH_V030_LINKED_WORKTREE_LIVE_OK` |

Windows environment: Windows 11 IoT Enterprise LTSC, build 26100, x64; rustc
`1.98.1`; Cargo `1.98.1`; Git `2.55.0.windows.5`; 13 workspace packages,
all `0.29.0` at certification time, Rust edition 2024; HostExplicit exactly
11.

The project Codex lifecycle baseline remains `codex-cli 0.149.0`. That
executable was unavailable during Task 361; ambient `0.155.1` was not
substituted, Codex was not run, and no model inference occurred. Task 361 does
not establish model/tool routing as live-certified.

## Task 361 verdict and required nonclaims

Task 361 is **PASS WITH EXPLICIT NONCLAIMS**. Retained nonclaims are:

- GUI automation, model-selected worktree routing, and Codex inference were
  not executed.
- Linux and macOS live certification and cross-platform live parity are not
  claimed.
- No OS sandbox, network isolation, race-free TOCTOU, machine-wide external-
  Git lock, rollback/replay/compensation, or uncertain-effect guarantee is
  claimed.
- No submodule or `--separate-git-dir` support is claimed.
- No worktree lifecycle authority, multiple active repositories, or persistent
  executable repository membership is claimed.

Existing broader RAH security limitations remain unchanged.

## Version and release-preparation gate

Task 362 transitions the 13 workspace packages from `0.29.0` to `0.30.0`.
The Rust edition remains `2024`. Cargo.lock may change only in the 13 internal
RAH package version records; external versions, checksums, dependency graph,
and source behavior must remain unchanged.

The prepared changelog is explicitly not marked released. Task 362 creates no
annotated tag, tag push, GitHub Release, publication timestamp, or release
object. Task 363 independently re-checks this exact candidate, creates
`v0.30.0` at exactly its SHA, requires tag CI, and publishes the release.

## Closure requirements

Before Task 362 closes, record the exact changed-file scope, package metadata,
HostExplicit count, full Rust and frontend/static validation, focused linked
carry-forward tests, release-build identity `rah-desktop v0.30.0`, exact-head
Task 362 CI PASS, `HEAD == origin/master`, a clean worktree, and the absence of
the `v0.30.0` tag and GitHub Release. Preserve all v0.29 immutable artifacts.
