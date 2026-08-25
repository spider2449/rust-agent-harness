# Task 094A — Multi-file edit preflight and postimage foundation

## Scope

Task 094A adds a crate-private preparation model for the future `repo.edit-files`
capability. It is not a `Tool`, is not registered, and has no profile or bridge
integration. The module contains no target replacement, rename, or `MoveFileExW` call.

## Implementation

`repository_multi_file_preflight` owns the closed request parser and fixed limits:
256 KiB serialized input, one through four targets, 1,024-byte logical paths,
one through sixteen replacements per target, 64 KiB replacement strings, 1 MiB
per-image limits, and checked 4 MiB/64-item aggregate limits. The request has
only `targets`; target and replacement objects reject unknown or missing fields.

The private `RepositoryMultiFileMutationPolicy` binds the Git executable,
canonical repository root, `.git` identity, and the existing
`git_stage::repository_lease` lock. It rejects logical path traversal, aliases,
links/reparse points, non-regular files, hard links, duplicate logical/canonical
paths, and duplicate file identities. It observes raw index, HEAD, refs, and
regular HEAD/stage-0 Git entries before snapshotting strict UTF-8, NUL-free
content and checking requested SHA-256 and length.

All literal replacement ranges are resolved in the original snapshot, must be
unique and non-overlapping, and produce one bounded postimage in byte-offset
order. Prepared targets are sorted by canonical logical path UTF-8 bytes.

For the non-commit preparation path, same-parent host-named exclusive temporary
postimages are written, identity/content checked, revalidated with repository
observations and target snapshots, then safely removed before return. The prepared
result cannot commit a target.

## Deferred to Task 094B

Task 094B owns retained temporary preparation for a commit attempt, native
replacement, post-commit classification, partial/uncertain outcomes, and broader
fault/race coverage. ADR 0014 remains **Proposed**. Windows uses native volume/
file-index identity and reparse rejection; Unix uses device/inode identity. No
crash-durability claim is made.

## Deterministic evidence

The private module has 20 focused deterministic tests. Test-only hooks select an
exact canonical-root target index and phase. They are not compiled into production
and do not appear in a tool, profile, bridge, or release surface.

Three-target real-Git fixtures prove first, middle, and final temporary failures
leave requested targets, raw index, HEAD, refs, and sentinel unchanged. Earlier
verified owned temporaries are cleaned, later temporaries are not prepared, and a
changed or replacement temporary is never blindly deleted. A dedicated global
hook mutates a target after all temporary preparation and before the actual
whole-plan revalidation; the real preparation path rejects it before return.

The retained private prepared-plan seam covers middle-target content and
same-byte identity replacement, parent-identity replacement, raw-index, HEAD,
ref-only, and temporary content/identity races. The shared existing repository
lease is structurally tested by holding that exact lease before spawning
preparation.

Real-Git fixtures reject clean violations, staged, untracked, ignored, unmerged,
and mode-160000 gitlink states, as well as linked-worktree `.git` file form.
Sparse handling has lower-level deterministic fail-closed evidence: only exact
`H ` index tags are admitted; sparse and other tags reject. A sparse-checkout
filesystem fixture is intentionally omitted because output differs across Git
versions.

| Phase | First | Middle | Final | Target effect |
| --- | --- | --- | --- | --- |
| parse/bounds | covered | n/a | n/a | zero |
| target validation | covered | covered | covered | zero |
| SHA/length | covered | covered | covered | zero |
| replacement planning | covered | covered | covered | zero |
| temp creation/write/verify | covered | covered | covered | zero |
| global revalidation | covered | global | global | zero |
| target/parent race | covered | covered | global | zero |
| index/HEAD/ref race | covered | global | global | zero |
| temporary race | covered | covered | covered | zero |

| Evidence | Windows | Ubuntu/Linux | Result |
| --- | --- | --- | --- |
| logical paths, aliases, ordering, Git observations | covered | covered | fail closed |
| unmerged, gitlink, linked worktree | covered | covered | reject |
| sparse tag state | lower-level deterministic | lower-level deterministic | reject |
| junction/reparse ancestry | Windows-only fixture | n/a | reject |
| FIFO and temporary mode | n/a | Unix-only fixture | reject / preserve mode |
| unsupported Windows attributes | reparse attribute branch | n/a | no separate mask exists |

Unix symlink, FIFO, and prepared temporary mode evidence is platform-gated. The
Windows junction fixture covers the production reparse-point attribute rule;
there is no distinct unsupported-attribute mask. Native target replacement,
native fault injection, partial/uncertain outcomes, retries, rollback, and all
commit-point tests remain deferred to Task 094B. All target files remain
uncommitted by 094A; ADR 0014 remains **Proposed**.
