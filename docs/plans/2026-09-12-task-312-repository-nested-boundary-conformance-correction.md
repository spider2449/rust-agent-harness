# Task 312 — Repository Nested-Boundary Conformance Correction

Status: correction implemented; delivery evidence pending commit/CI

## Authoritative checkpoint

- Current master / `HEAD`: `3bdfcf8324bb8433489af5f85e978a9f80ab3e0a`
- Direct parent: `d46b4c4c47b9b0747fc1b6bfa35a1c399323f665`
- Task 311 audit-content commit: `d46b4c4c47b9b0747fc1b6bfa35a1c399323f665`
- Task 311 final verification commit: `3bdfcf8324bb8433489af5f85e978a9f80ab3e0a`
- Task 311 exact-head CI: `34669558520` — PASS
- Task 311 verdict: Verdict B — CORRECTION REQUIRED
- Accepted architecture: ADR 0027
- Current release: RAH v0.25.0 — RELEASED
- Immutable v0.25.0 source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`
- Tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`
- GitHub Release: `387406579`
- Workspace baseline: 13 packages, version `0.25.0`, Rust edition 2024
- HostExplicit eligible set: exactly 11 names

## Scope and design

Task 312 corrects repository-bound nested Git boundary enforcement without
implementing workspace membership, repository selection, new authority, or
public contract changes. `rah-sandbox::WorkspacePolicy` remains Git-agnostic.

The correction adds one repository-specific nested-boundary primitive in
`rah-tools`, distinguishes Repository A's own `.git` from descendant markers,
fails closed on observable or ambiguous marker forms, and reuses existing
capability-specific identity, link/reparse, gitlink, and result semantics.

Affected surfaces are `fs.read`, repository file-info, status, diff,
diff-staged, patch, edit-files, create-file, delete-file, rename-file,
create-directory, Stage, Unstage, and reviewed Commit. `repo.create-branch`
and `host.git.status` remain fixed-scope non-target capabilities.

## Lifecycle and observer strategy

Targeted capabilities validate at admission/Prepare and again at currentness
or immediate pre-effect boundaries. Whole-repository observers validate the
bounded Repository A tree before Git execution and fail the complete
observation when a safe boundary proof cannot be established; they do not
post-filter already observed sensitive content.

## Platform handling and nonclaims

Windows marker lookup uses host path semantics rather than locale lowercase
comparison and retains existing reparse, alias, identity, ADS/device, and
trailing-dot/space rejection behavior. Unix symlink, identity, and mount
defenses remain capability-specific; no race-free TOCTOU or Unix/macOS live
certification is claimed. Nested bare repositories without a reliable child
`.git` marker remain unsupported and are not heuristically inferred.

## Deterministic regression matrix

Add or extend fresh Repo A plus nested Repo B fixtures for direct reads and
observation, all repository mutators, Stage/Unstage, reviewed Commit, the
observer behavior, marker appearance after Prepare, marker form ambiguity,
ordinary same-repository nested directories, and unchanged gitlink behavior.
For every proven pre-effect rejection, assert the existing native/Git effect
counter is zero. Preserve rename cross-direction coverage and its stronger
checks.

## Validation and delivery record

Focused deterministic evidence includes the new real Repo A plus independent
nested Repo B observer test, repository-scoped `fs.read` rejection, ordinary
directory allowance, `.git` directory/file/symlink marker rejection, Windows
case-equivalent marker coverage, zero native directory-create attempts, and
the existing rename cross-direction/currentness/uncertainty tests. The
`rah-tools` suite passed 269 unit tests plus all package integration suites;
the new `repository_nested_boundary` integration test passed 1 test. The
full workspace test passed with 0 failures; 10 host-only tests remained
ignored by their existing environment gates. Clippy, format, metadata, and
diff checks passed.

The exact changed-file scope is:

- `crates/rah-tools/src/repository_boundary.rs`;
- the affected `rah-tools` repository/read modules and `src/lib.rs`;
- `crates/rah-tools/tests/repository_nested_boundary.rs`;
- `crates/rah-desktop/src/main.rs` only to bind repository-scoped `fs.read`;
- this plan file.

The shared primitive is `RepositoryNestedBoundaryPolicy` with
`validate_existing` and `validate_observation`. It is used at target
admission, retained/currentness or immediate pre-effect checks, observer
admission, Stage/Unstage index admission, and reviewed Commit staged-path
validation. Whole-repository status/diff/diff-staged fail the complete
observation before Git execution when a nested boundary or ambiguous
traversable filesystem evidence is found.

The correction preserves rename's stronger ancestry, identity, mount, alias,
and post-effect checks; create-directory's one-leaf/no-parent-creation
contract; gitlink mode-160000 handling; generic Git-agnostic WorkspacePolicy;
the unchanged public schemas and PermissionLevel values; and the exact 11
HostExplicit names. Bare nested repositories without a reliable child marker,
race-free TOCTOU, Unix/macOS live certification, linked-worktree support,
workspace membership, and new authority are explicit nonclaims.

Production correction commit: pending.
Later evidence commit: none planned.
Exact-head CI: pending.
Final Git state and immutable v0.25 identity recheck: pending.

Final verdict will be exactly one of:

- Outcome A — CORRECTION CLOSED
- Outcome B — BLOCKED

The next task after a clean closure is Task 313 — Repository Nested-Boundary
Independent Re-Audit. Workspace membership is not implemented by Task 312.
