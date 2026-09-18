# Task 359 — Linked Worktree Identity and Repository Policy Foundation

Status: deterministic implementation; Task 360 audit and Task 361 live certification remain separate

Parent checkpoint: `5ebf2ab843ed08ff3be1db7b5015c6a224a2bc28`

Task 358 exact-head CI: run `35302394722`, PASS

Accepted decisions: ADR 0027 and ADR 0029

Research: `docs/plans/2026-09-18-task-357-linked-worktree-authority-contract-research.md`

## Scope

Extend the existing process-local repository identity to the closed linked
worktree relationship accepted by ADR 0029. Keep one active selected member,
one repository workflow, and the existing authority categories. Main
worktrees retain their current `.git` directory form. Linked worktrees retain
their selected root, private Git directory, common-directory relation, and
registration as private host evidence.

This task does not add a PermissionLevel, Tool authority, HostExplicit
eligibility, persistence field, sibling selector, worktree-management
capability, or frontend worktree UI. It does not claim Windows live
certification. Task 360 performs the independent deterministic security and
currentness audit; Task 361 owns the live Windows gate.

## Production files changed

- `crates/rah-tools/src/repository_git_layout.rs` — new private layout
  classifier, relationship evidence, bounded fixed Git probes, and shared
  real-worktree test fixture.
- `crates/rah-tools/src/repository_admission_identity.rs` — extends the
  existing identity with layout evidence and private-target duplicate
  relation semantics.
- `crates/rah-tools/src/git_status.rs` — uses the canonical layout identity
  check rather than a separate `.git` classifier.
- `crates/rah-tools/src/repository_observer.rs` — retains and revalidates the
  selected layout before repository observations.
- `crates/rah-tools/src/git_stage.rs` — validates the selected layout and
  narrows Stage/Unstage state to selected HEAD, index, target, and worktree;
  unrelated shared-ref changes no longer invalidate the operation.
- `crates/rah-tools/src/repository_commit.rs` — permits validated linked
  layouts and binds reviewed Commit to the selected index, HEAD, attached
  branch, and branch OID.
- `crates/rah-tools/src/repository_branch_create.rs` — creates local refs
  through the validated common Git directory and preserves selected HEAD.
- `crates/rah-tools/src/repository_create_directory.rs` — binds the selected
  Git executable and layout and observes the selected private HEAD/index.
- `crates/rah-tools/src/repository_create_file.rs` — uses selected layout and
  index evidence.
- `crates/rah-tools/src/repository_delete_file.rs` — uses selected layout,
  index, and private operation-state paths.
- `crates/rah-tools/src/repository_rename_file.rs` — uses selected layout,
  index, and private operation-state paths.
- `crates/rah-tools/src/repository_worktree_patch.rs` — uses selected layout
  and validates the semantic Git paths.
- `crates/rah-tools/src/repository_multi_file_preflight.rs` — uses selected
  private HEAD/index paths, captures selected HEAD semantics, revalidates the
  layout through preparation and mutation, and ignores unrelated shared-ref
  changes.
- `crates/rah-tools/src/repository_boundary.rs` — blocks access to the active
  root `.git` file or directory while preserving nested repository checks.
- `crates/rah-tools/src/lib.rs` — registers the private layout module.
- `crates/rah-desktop/src/main.rs` — performs complete identity validation
  during production admission and before and after active composition
  construction; a real main/A/B fixture exercises admission, switching,
  Close, reactivation, and stale activation after external worktree removal.

The following changed files add deterministic test coverage or update an
existing test constructor for the new host-selected Git executable:

- `crates/rah-tools/src/fs_read.rs` — linked-root read isolation.
- `crates/rah-tools/src/repository_file_info.rs` — linked-root observation.
- `crates/rah-desktop/src/repository_membership.rs` — real three-worktree
  membership, duplicate, activation, removal, re-admission, and DTO privacy.
- `crates/rah-runtime-codex/src/bridge_tests.rs` — updates a test-only
  directory-creation authority constructor.
- `crates/rah-tools/tests/git_status.rs` — rejects unsupported generic
  gitfiles while retaining main-worktree process-boundary coverage.
- `crates/rah-tools/tests/repository_delete_file.rs` — verifies unrelated
  refs do not stale a selected delete review.

No Cargo manifest, lockfile, Tauri permission, or frontend file is changed.

## Internal identity and Git probe boundary

`RepositoryGitLayout` is a private, non-serialized host abstraction with
`Main` and `Linked` evidence. It provides the selected private Git directory,
common Git directory, selected index path, selected HEAD path, and
same-private-target relation. The existing `RepositoryAdmissionIdentity`
continues to retain canonical root and root filesystem identity, canonical
Git executable and executable filesystem identity, and now the layout proof.
No private/common path or raw identity getter is exposed to frontend, model,
provider, Tool input, or persistence state.

Semantic classification uses only `HostExecutionPolicy`, the exact retained
Git executable, fixed argv, selected-root cwd, the repository observer
environment, bounded output, and a five-second timeout. The fixed probes are
`rev-parse --show-toplevel`, `--absolute-git-dir`, `--git-common-dir`,
`--is-bare-repository`, `--show-superproject-working-tree`, semantic `--git-path`
for `index` and `HEAD`, and `worktree list --porcelain -z`. Branch creation
performs its own fixed semantic refs-storage check. No shell,
model-selected arguments, or mutating worktree command is used.

Metadata bounds are 4096 bytes for the root gitfile, 4096 bytes each for
`commondir` and the `gitdir` backlink, 16 KiB for each rev-parse response, 1 MiB
for worktree-list stdout, and 16 KiB for probe stderr. Parsers reject NUL,
invalid UTF-8 where paths require it, multiple or ambiguous records, unsupported
records, and oversized content. Gitfile, commondir, and backlink bytes and
filesystem object identities remain currentness evidence.

## Accepted and rejected layouts

| Layout | Result | Evidence |
| --- | --- | --- |
| Main worktree | Accepted | Root `.git` is a real non-reparse directory; Git reports it as both private and common Git directory. |
| Standard linked worktree | Accepted | One regular `.git` gitfile; canonical private directory under `<common>/worktrees/<registration>`; exact standard `commondir` and backlink; one matching non-bare, non-prunable registration; all semantic probes agree. |
| Modern submodule | Rejected | It lacks the accepted common `worktrees` registration/backlink relationship and reports superproject evidence. |
| `git init --separate-git-dir` | Rejected | It lacks the standard private registration and common/backlink relationship. |
| Copied or fabricated gitfile | Rejected | Selected-root backlink and unique registration do not agree. |
| Malformed gitfile | Rejected boundedly | Record count, NUL, byte bound, path, and backlink validation fail closed. |
| Detached linked worktree | Identity accepted | Read and content policies retain their existing behavior; reviewed Commit remains attached-branch-only. |

Locked but present registrations are accepted if all evidence agrees. Stale,
prunable, moved, removed, replaced, or rewritten identities fail revalidation;
RAH does not repair, prune, migrate, or remove worktrees.

## Membership and operation conformance

Membership equality is the same root filesystem object or same validated
private worktree target. Nested roots remain conflicts. Different roots with
different private Git directories remain distinct even when they share a
common Git directory. Admission never activates a member. Activation
revalidates the complete linked relationship before active composition
construction and before final publication.

The existing observer and repository policies use the selected worktree root,
private HEAD, private index, and private operation-state metadata. Git semantic
resolution verifies the selected index and HEAD locations. Branch creation
uses shared refs and does not switch worktrees. Its currentness snapshot retains
only namespace-colliding local heads, so unrelated branch updates do not stale
the requested branch creation. Content tools remain rooted in
the selected worktree, reject `.git` metadata paths, and preserve nested `.git`
boundaries.

Stage and Unstage mutate only the selected worktree index and no longer
snapshot all refs. Reviewed Commit binds the selected index snapshot, attached
branch, expected branch OID, and selected HEAD. An unrelated sibling branch
commit does not stale an unchanged selected review; movement of the selected
branch ref does. Shared object creation during Commit remains part of the
existing Commit effect.

Content mutation and Stage/Unstage currentness no longer compare all common
refs. They bind selected HEAD/tree, index, target, and repository identity as
needed by each operation. Multi-file editing captures both the selected HEAD
file relation and selected HEAD object ID; an unrelated ref update remains
current while advancing the selected branch makes the review stale.
Patch preflight follows the same selected-HEAD rule, and branch creation keeps
only the local-head namespace conflicts relevant to its requested ref.

Directory creation now receives the host-selected Git executable so it can
use the same validated layout. Its no-effect/postcondition snapshot covers the
selected private HEAD and index; unrelated common refs are not a global
invalidation source.

Reviewed delete currentness likewise binds the selected file, selected index,
HEAD and attached branch. A Commit in sibling B leaves A's retained delete
review current; advancing A's selected HEAD remains stale. Generic file and
process tools continue to receive only the selected active root through the
existing composition.

## Deterministic fixture and evidence matrix

The reusable real-Git fixture creates `main`, `linked-a`, and `linked-b` with
local deterministic identity and line-ending configuration. Tests cover:

- all three distinct members sharing a common Git directory, same-root and
  alias duplicates, copied-target rejection, no automatic activation, and
  membership-only inactive removal with unchanged Git registration/ref state;
- root, gitfile, private/common/registration, commondir, backlink, Git
  executable, moved, and removed identity staleness;
- real modern and old-form nested submodule, separate-git-dir, copied,
  fabricated, malformed, locked, detached, and bounded metadata cases;
- observer isolation; Stage/Unstage index isolation; Commit A/B index, HEAD,
  branch, and shared-object behavior; local branch creation without checkout;
- test-only Stage/Unstage fault hooks bind to their selected repository root,
  keeping concurrent test executions isolated;
- selected-root read, file-info, create, delete, rename, patch, multi-file
  preflight, and directory creation; active-root `.git` and nested-boundary
  rejection;
- Windows-gated junction rejection for root, active `.git`, private
  registration, and common Git directory.

Errors remain generic and bounded. Membership may continue to show only the
user-selected display path under the existing presentation contract.
Effective Authority remains active-member-only. No private/common path,
registration ID, identity bytes, Gitfile bytes, ticket, index snapshot, or
Commit state is persisted. Restart still begins with no members and no active
repository authority. HostExplicit remains exactly 11; no frontend structure
or Tauri permission change is expected.

## Validation record

Completed:

- `cargo fmt --check` — PASS.
- `cargo check -p rah-tools --all-targets` and `cargo check -p rah-tools` — PASS.
- `cargo clippy -p rah-tools --all-targets --all-features -- -D warnings` — PASS.
- `cargo check -p rah-desktop` — PASS.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` — PASS.
- `cargo test -p rah-runtime-codex -- --test-threads=1` — 83 passed,
  1 ignored.
- Isolated Desktop restart test after replacing its placeholder repository
  with a real Git fixture — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace -- --test-threads=1` — PASS. Desktop: 312 passed,
  15 ignored. `rah-tools`: 316 unit tests passed, all integration tests
  passed; one environment-gated Git status test was ignored. All other
  workspace integration and documentation tests passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  PASS.
- `cargo test -p rah-tools repository_commit::tests::spawn_failure_lock_refusal_and_post_spawn_observer_failure_are_classified_by_state -- --test-threads=1` — PASS after aligning the test-only index-lock fault with the selected worktree index path.
- `cargo metadata --no-deps --format-version 1` — 13 packages, all
  `0.29.0`, all edition 2024.
- No Cargo manifest, dependency, or `Cargo.lock` change.
- HostExplicit remains exactly 11; frontend and Tauri permission surfaces are
  unchanged.
- `git diff --check` — PASS.
- No Task 359 live-certification marker was emitted. No Windows live
  certification is claimed.

Commit, push, `HEAD == origin/master`, exact-head CI, and clean Git state are
recorded in the closure report after publication.
