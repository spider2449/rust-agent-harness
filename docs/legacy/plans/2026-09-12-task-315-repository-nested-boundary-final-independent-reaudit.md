# Task 315 - RAH v0.26 Repository Nested-Boundary Final Independent Re-Audit

## 1. Authoritative checkpoint

- Current master: `8ee429839ef32b901625a5d022a86c64b6888272`
- Direct parent: `59f85f662eea5018e502e22a515ecc771ff41411`
- Task 314 production correction: `1756b3a1f5848280017914169c648fb0efce76dc`
- Task 314 docs-only evidence commits: `1d219a70134e1a26cc4ef5531a47176966f5ebc5`, `59f85f662eea5018e502e22a515ecc771ff41411`, and `8ee429839ef32b901625a5d022a86c64b6888272`
- Task 314 exact-head CI: `34682628073` - PASS
- Task 314 outcome: Outcome A - CORRECTION CLOSED
- Accepted ADR: ADR 0027 - Workspace/Repository Identity and Authority-Composition Boundary
- Published release: RAH v0.25.0
- Immutable release source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`
- Annotated tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`
- GitHub Release: `387406579`
- Workspace: 13 packages, version `0.25.0`, Rust edition 2024
- Exact production HostExplicit set: 11 names

This is an audit-only record. Task 315 changes no production code, tests,
ADR, Cargo file, frontend, Tauri code, or CI workflow.

## 2. Prior findings and corrections

Task 311 found that repository-bound paths were not uniformly isolated from a
descendant repository. `fs.read` used generic `WorkspacePolicy`; file-info
could directly inspect a descendant; whole status/diff/diff-staged lacked a
pre-Git boundary scan; patch, multi-file edit, create-file, delete-file, and
create-directory lacked complete shared target/parent enforcement; Stage and
Unstage lacked target isolation; and Commit did not validate every staged path.
Rename was the previously conformant reference surface, while create-branch
and host.git.status had no model-selected repository-relative target.

Task 312 introduced `RepositoryNestedBoundaryPolicy` in `rah-tools`, with
`validate_existing` for target/parent ancestry and `validate_observation` for
whole-repository observers. It bound repository Desktop `fs.read` through
`FsReadTool::new_repository`, added lifecycle checks to the affected mutators,
validated staged Commit paths, and preserved rename's stronger identity,
alias, reparse, mount, one-attempt, and proof rules.

Task 313 independently confirmed the ordinary-marker correction but found two
material residual blockers:

1. `validate_existing` still used `directory.join(".git")`, so a distinct
   `.GIT` in a Windows case-sensitive directory could evade target validation.
2. Stage/Unstage `WorktreeSnapshot::capture` recursively read every regular
   file below A except exact `.git`, so an A target could cause a proof read of
   `nested-b/secret.txt`.

Task 314 changed exactly the relevant production surfaces. It made directory
entry enumeration plus a shared platform-gated marker predicate authoritative,
made WorktreeSnapshot boundary-aware, added test-only read and native-attempt
instrumentation, and removed rename's duplicate weaker marker helper. It did
not implement workspace membership.

## 3. Final shared marker audit

`crates/rah-tools/src/repository_boundary.rs:121-167` contains the only shared
marker decision. `reject_nested_marker` enumerates actual directory entries and
passes their names to `reject_marker_entries`; it no longer constructs a
lowercase `.git` child and treats `NotFound` as proof of absence.

`is_dot_git_name` is explicitly platform gated:

- Windows uses `OsStr` text with `eq_ignore_ascii_case(".git")`, which is the
  required ASCII-insensitive semantic for `.git`, `.GIT`, `.Git`, and `.gIt`.
- Unix-like targets compare exactly with `OsStr::new(".git")`.
- No locale-dependent folding, Unicode normalization, or Unicode-equivalence
  claim is made.

The matcher unit test independently exercises the four required Windows
spellings and the three non-matching forms, and the enumeration test supplies
names directly rather than relying on filesystem lookup aliasing. The actual
Windows run passed 13 repository-boundary tests. The implementation therefore
does not depend on a case-insensitive directory resolving `.git` to `.GIT`.

`validate_existing` uses the shared enumeration while walking target or parent
ancestry (`repository_boundary.rs:37-65`). `validate_observation` uses the
same predicate for every enumerated child (`:71-117`). Stage/Unstage snapshot
traversal uses the same predicate at the entry point (`git_stage.rs:357-402`).
Rename now calls the shared policy from `validate_ordinary_directory_ancestry`
(`repository_rename_file.rs:1687-1717`) and no longer has a competing weaker
direct-probe helper.

The root's own known `.git` metadata identity remains safely probed directly
by repository identity/policy constructors. Audited root-only probes include
`repository_observer.rs:380-404`, `git_stage.rs:120-139` and `220-235`,
`repository_worktree_patch.rs:1066-1079`, `repository_create_file.rs:657-675`,
`repository_delete_file.rs:699-721`, `repository_create_directory.rs:182-201`,
`repository_commit.rs:119-140`, and `repository_branch_create.rs:242-253`.
These establish Repository A's own supported metadata identity; they do not
decide whether a descendant contains a nested marker. The old rename direct
descendant probe was removed. The ignored host-only Git discovery test's
ancestor search is host root discovery, not a repository-boundary security
decision.

## 4. Stage/Unstage snapshot confidentiality

`GitIndexMutationPolicy::capture_state` revalidates first and calls
`WorktreeSnapshot::capture(&self.root, &self.boundary)` at
`git_stage.rs:199-217`. `capture_tree` checks the root marker, rejects a
non-root matching marker, rejects links/reparse entries, validates a directory
with the shared policy before recursion, and only then reaches a regular-file
read.

The test-only read audit is `#[cfg(test)]` only (`git_stage.rs:404-435`). Its
`record` call is immediately before the actual `fs::read(&path)` at line 386;
it creates no production API or production observable state. The attack tests
clear the audit before execution, construct:

```text
Repository A/
  .git/
  target.txt
  nested-b/
    .git/                 # .GIT on Windows test fixtures
    secret.txt
```

and separately execute Stage and Unstage. Both tests assert: execution fails;
the complete Git index bytes are unchanged; the mutation-attempt counter is
zero; and the recorded reads do not contain `nested-b/secret.txt`.

The test-only mutation counter is incremented immediately before
`mutation.execute_process` (`git_stage.rs:174-188`), the actual index-effect
boundary. It is not a constructor or observation counter. The four
post-construction tests create the nested boundary only after policy
construction and assert rejection, unchanged index bytes, and zero attempts
for Stage and Unstage separately. Direct targets inside B remain rejected at
construction and revalidation. Root metadata is skipped without recursion;
case-equivalent Windows root markers use the same matcher, while a matching
nested marker rejects before descendant content is read.

## 5. Windows analysis

The current audit ran on Windows. Deterministic evidence is sufficient for the
marker decision because the security input is the enumerated actual entry name,
the matcher is explicitly Windows-only and ASCII-insensitive, and the tests
exercise differently cased names independently of lookup aliasing. There was
no real per-directory case-sensitive NTFS fixture run. No live case-sensitive
NTFS certification is claimed.

Windows reparse-point checks remain in the shared boundary policy and the
capability-specific ancestry walkers. Root and `.git` identities use Windows
volume/file-index identity; path aliases use `paths_equivalent`; rename keeps
source and parent identity, destination absence, case-equivalent HEAD/index
collision, reparse/junction ancestry, one no-replace attempt, and post-effect
alias proof. These are deterministic checks, not a race-free TOCTOU claim.

## 6. Unix analysis

The non-Windows matcher remains exact `.git`. Boundary enumeration and all
filesystem enumeration errors fail closed. Observer traversal treats an
ordinary symlink as a Git leaf and does not recurse through it; a `.git`
symlink/reparse marker remains protected or ambiguous. Existing Unix device/
inode identity checks and rename Linux mount-point ancestry checks remain in
place. The tests are deterministic platform tests and do not constitute Linux
or macOS live certification.

Stage/Unstage retain their established stricter non-following worktree-entry
policy. That snapshot policy is distinct from the observer's ordinary-symlink
leaf policy and was not conflated by Task 314. The repository observer's
ordinary symlink tests and status/diff symlink coverage passed.

## 7. Bare-repository nonclaim

The shared policy recognizes observable child `.git` markers only. A nested
bare repository with no child marker remains unsupported and non-inferred.
There is no heuristic detection from `HEAD`, `objects`, `refs`, or similar
filenames. This preserves ADR 0027's explicit nonclaim and does not weaken the
closed supported marker contract or silently broaden Task 314.

## 8. Lifecycle and capability checks

- `fs.read`: generic `FsReadTool::new` remains WorkspacePolicy-only; repository
  Desktop construction at `rah-desktop/src/main.rs:5987` uses
  `FsReadTool::new_repository`. Repository target validation precedes opening
  the file, and nested B is rejected.
- `repo.file-info`: `validate_target` precedes index, HEAD/tree, status,
  metadata, digest, and worktree facts (`repository_file_info.rs:59-112`).
- `repo.status`, `repo.diff`, and `repo.diff-staged`: observer boundary scan
  precedes the corresponding Git command in `RepositoryObserver::run`
  (`repository_observer.rs:308-363`); output is not parsed as a partial result
  before the scan.
- `repo.patch`: target and repository checks remain at preparation,
  revalidation, immediate pre-effect, and post-proof points.
- `repo.edit-files`: the complete target set is preflighted before any native
  replacement; all remaining targets are revalidated before effects. Mixed A/B
  targets reject the whole operation.
- `repo.create-file`: existing parent boundary is checked at preparation,
  revalidation, and proof; no parent creation or overwrite is added.
- `repo.delete-file`: target/repository boundary and identity checks remain at
  preparation, revalidation, immediate pre-delete, and proof; stale nested B
  rejection has zero delete attempts.
- `repo.create-directory`: existing parent boundary remains checked at
  capture, revalidation, and proof; ordinary same-A nested directories remain
  allowed and HostExplicit eligibility remains absent.
- Commit: `capture_snapshot` validates every staged path before `write-tree`
  (`repository_commit.rs:319-364`). The stale-after-review nested-boundary
  test returns `PreconditionFailed`, records zero commit attempts, and leaves
  HEAD/ref state unchanged. Gitlink mode 160000 remains separately rejected.
- Stage/Unstage: target validation, boundary-aware snapshot capture, immediate
  pre-effect revalidation, actual zero-attempt counter, and unchanged-index
  assertions all remain in place.
- `repo.rename-file`: source and destination ancestry, source/parent identity,
  destination absence, Windows aliases and HEAD/index collisions,
  reparse/junction and Linux mount checks, one native no-replace attempt,
  post-proof, uncertainty, and no retry/replay/rollback remain. The A to B,
  B to A, and B to B through A rejection directions were rerun and rejected
  before native rename.
- `repo.create-branch` and `host.git.status`: fixed host-owned ref/status
  surfaces have no model-selected repository-relative target and were not
  broadened.

## 9. Full 16-capability closure matrix

| Capability | Task 311 original gap | Task 312 correction | Task 313 residual | Task 314 correction | Task 315 final verdict |
| --- | --- | --- | --- | --- | --- |
| `fs.read` | Generic workspace read could open B | Repository constructor bound `RepositoryNestedBoundaryPolicy` | Lowercase probe could miss Windows `.GIT` | Enumerated shared marker validation | Conformant; B target rejected before file read |
| `repo.file-info` | Direct metadata/digest/worktree facts could inspect B | Target validation before Git/direct facts | Same Windows marker gap | Shared enumeration flows through target validation | Conformant; target checked before facts |
| `repo.status` | Whole A observation had no boundary preflight | Full scan before Git | Case-sensitive marker gap | Shared enumerated scan | Conformant; scan precedes Git |
| `repo.diff` | Whole A diff had no boundary preflight | Full scan before each diff command | Case-sensitive marker gap | Shared enumerated scan | Conformant; no partial Git result |
| `repo.diff-staged` | Cached diff had no boundary preflight | Full scan before cached diff | Case-sensitive marker gap | Shared enumerated scan | Conformant; no partial Git result |
| `repo.patch` | B target could reach replacement | Capture/revalidate/pre-effect/proof checks | Shared marker case gap | Shared predicate used by all checks | Conformant; one-effect rules preserved |
| `repo.edit-files` | B or mixed A/B targets could be admitted | Entire-set preflight and per-effect revalidation | Shared marker case gap | Shared predicate used by all target checks | Conformant; mixed set rejects before effects |
| `repo.create-file` | File could be created under B parent | Parent checks at lifecycle points | Shared marker case gap | Shared predicate used by parent checks | Conformant; nested parent rejected |
| `repo.delete-file` | A-tracked file under B could be deleted | Target checks and proof | Shared marker case gap | Shared predicate used by target checks | Conformant; stale B has zero attempts |
| `repo.create-directory` | Partial local marker check | Parent checks at lifecycle points | Shared marker case gap | Shared predicate replaces duplicate semantics | Conformant; ordinary A parent remains allowed |
| Stage | A target could cause proof read of B; target gap | Target checks only | Snapshot read blocker plus marker case gap | Boundary-aware snapshot, read audit, zero-attempt counter | Conformant; B sentinel not read and index unchanged |
| Unstage | Same target/snapshot gap | Target checks only | Snapshot read blocker plus marker case gap | Same boundary-aware snapshot correction | Conformant; B sentinel not read and index unchanged |
| Commit indirectly | Staged paths lacked nested validation | Validate all staged paths before `write-tree` | Shared marker case gap | Corrected shared predicate and stale test | Conformant; zero commit attempts on stale B |
| `repo.rename-file` | Reference surface already had stronger checks | Shared boundary integration retained | Shared direct-marker semantics needed alignment | Duplicate weaker helper removed | Conformant; A->B, B->A, B->B reject pre-effect |
| `repo.create-branch` | No repository-relative target path | Fixed ref-only scope | None | No change required | Conformant/no-scope |
| `host.git.status` | No model-selected target path | Fixed host-owned status scope | None | No change required | Conformant/no-scope |

The matrix is a boundary-closure result, not a claim that any live external
effect or cross-platform certification was performed.

## 10. Public-contract comparison

The independent diff from Task 313 checkpoint
`336491926b5eeff0477eea4b0b0b15f073c0b367` to final Task 314 head
`8ee429839ef32b901625a5d022a86c64b6888272` contains only the Task 314
production changes in `repository_boundary.rs`, `git_stage.rs`, and
`repository_rename_file.rs`, plus the Task 314 evidence document. No changes
were found in protocol, sandbox, ADR, Desktop authority, provider, or frontend
contract files.

The following remain unchanged:

- public Tool names, inputs, outputs, and status taxonomy;
- `PermissionLevel` and provider contracts;
- Trusted Profile schema and frontend/Tauri authority boundary;
- repository selector surface;
- exact HostExplicit set of 11:
  `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`,
  `repo.diff-staged`, `repo.create-branch`, `repo.patch`, `repo.edit-files`,
  `repo.create-file`, `repo.delete-file`, and `repo.rename-file`; and
- ineligible `repo.create-directory`, `repo.commit`, MCP, Process Plugin,
  fixture/diagnostic, unknown, and provider-defined Tools.

`rah-sandbox::WorkspacePolicy` remains Git-agnostic (`workspace.rs:43-158`),
with no `.git` or repository semantics. Repository isolation remains the
narrower `rah-tools` layer.

## 11. Workspace membership absence

No workspace object, membership list, active-repository switch, repository
selector Tool input, union ToolRegistry, multi-repository Effective Authority,
workspace persistence, or multi-repository frontend UX exists in Task 314 or
Task 315. Task 316 is not started by this audit.

## 12. Deterministic evidence

All focused tests below ran against the current production head, sequentially;
the larger suites were rerun serially after an initial validation harness was
found to have overlapped live sessions. Final serial results were all PASS.

| Focused suite | Actual result |
| --- | ---: |
| repository boundary unit tests | 13 passed |
| `git_stage` integration | 6 passed |
| `git_unstage` integration | 6 passed |
| repository Commit unit tests | 15 passed |
| repository observer unit tests | 2 passed |
| `repository_status` | 4 passed |
| `repository_diff` | 4 passed |
| `repository_diff_staged` | 7 passed |
| `fs_read` unit tests | 8 passed |
| `repository_file_info` | 4 passed |
| repository patch unit tests | 49 passed |
| repository multi-file preflight unit tests | 33 passed |
| `repository_create_file` | 11 passed |
| `repository_delete_file` | 11 passed |
| repository rename unit tests | 42 passed |
| `repository_rename_preparation` | 24 passed |
| repository create-directory unit tests | 6 passed |
| real nested-boundary integration | 1 passed |

The boundary, Stage, Unstage, Commit, observer, and mutator tests use fresh
temporary fixtures as implemented. The Stage/Unstage tests specifically clear
the read audit and assert the absence of the B sentinel path, zero native
index attempts, and unchanged index bytes. Counts are supporting evidence, not
a substitute for the source-order audit.

## 13. Limitations and nonclaims

- No real per-directory case-sensitive NTFS fixture was run.
- No Windows current-master connected destructive certification was run.
- No Linux/macOS live certification was run.
- No race-free TOCTOU, OS sandboxing, network isolation, rollback,
  compensation, retry, replay, or uncertain-effect recovery claim is made.
- Linked worktrees and nested bare repositories remain unsupported/non-inferred.
- No Unicode normalization or locale-dependent marker semantics are claimed.
- No model-selected repository authority, provider/MCP/Process Plugin authority,
  workspace-wide filesystem/Git authority, or cross-repository operation is
  claimed.
- No HostExplicit `repo.commit` or `repo.create-directory` is claimed.
- The prior v0.25 Windows live rename evidence is not current-master
  certification; it remains historical release evidence.

## 14. V0.25 immutability and package state

The annotated `v0.25.0` tag resolves to tag object
`ea3c31aaf5190b632d7ef86387f7aff6004ae664` and points to immutable source
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`. The current Task 314 master is a
different source and is not the v0.25 release source. GitHub Release identity
remains `387406579` from the authoritative checkpoint.

`cargo metadata --no-deps --format-version 1` reports 13 packages and 13
workspace members, all version `0.25.0`, all edition 2024. No Cargo manifest,
dependency, or `Cargo.lock` drift exists in the Task 313-to-Task 314 change.

## 15. Validation record

The requested final validation is recorded for the final audit delivery head:

```text
cargo fmt --check                                      PASS
cargo check --workspace                                PASS
cargo test --workspace                                 PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
git diff --check                                       PASS
cargo metadata --no-deps --format-version 1           PASS
```

The full workspace test had zero failures; host-only tests retain their
existing ignored environment gates. No connected destructive Codex live test or
case-sensitive NTFS live certification was run.

## 16. Exact next task

With this closure, the next bounded task is **Task 316 - Inert Multi-Repository
Membership and Explicit Admission Foundation**. It may initially implement
only host-owned descriptive membership, process-local repository identities,
explicit host/human admission, duplicate/alias-equivalent rejection, nested
co-membership rejection, minimally necessary zero/one active state, and fresh
authority on admission/activation. It must not automatically implement a
workspace-wide ToolRegistry union, repository selectors in Tool inputs,
cross-repository operations, parallel HostExplicit execution, multi-repository
Commit, network/provider changes, full frontend polish, or live certification.

Verdict A — NESTED-BOUNDARY CONFORMANT; CLEAR FOR MULTI-REPOSITORY MEMBERSHIP FOUNDATION
