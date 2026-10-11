# Task 313 — RAH v0.26 Repository Nested-Boundary Independent Re-Audit

Status: Audit complete; correction still required

## 1. Authoritative checkpoint

| Item | Value |
| --- | --- |
| Branch | `master` |
| Current master / `HEAD` | `de8a9ed526814e19d800762fe1302094a22e0e0c` |
| `origin/master` | `de8a9ed526814e19d800762fe1302094a22e0e0c` |
| Final exact-head CI | `34673368341` — PASS |
| Task 312 outcome | Outcome A — CORRECTION CLOSED |
| Accepted ADR | ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary |
| Current published release | RAH v0.25.0 |
| Immutable v0.25 source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| Annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| GitHub Release | `387406579` |
| Workspace | 13 packages, version `0.25.0`, Rust edition 2024 |
| HostExplicit eligible set | exactly 11 names |

The audit began with `git status --short`; the worktree was clean. The stated
checkpoint and `origin/master` were verified before inspection. This task is
audit-only: no production code, test, frontend/Tauri, ADR, version, dependency,
tag, Release, or workspace-membership change was made.

The historical v0.25 Windows live certification remains certification of the
immutable v0.25 release source only. It is not live certification of Task 312 or
current master. Task 313 ran deterministic validation only and no connected
Codex live test.

## 2. Task 311 findings revisited

Task 311 identified the accepted invariant from ADR 0027: every
Repository-A-bound path capability must fail closed when its target ancestry
crosses a distinct nested Git repository boundary, whether or not Repository B
is a workspace member. It found material gaps in `fs.read`, file-info,
whole-repository observers, patch, multi-file edit, create-file, delete-file,
create-directory, Stage, Unstage, and Commit's staged-path admission. Rename
was the strong reference; create-branch and `host.git.status` were outside the
path-target scope.

The required distinction remains:

- ordinary same-repository nested directories are valid;
- a child worktree with an observable `.git` directory or file is a protected
  boundary;
- a `.git` symlink or reparse marker is an ambiguous protected boundary;
- a submodule/gitlink (`160000`) remains a separate existing contract; and
- a bare child repository without an observable child `.git` marker is not
  inferred or newly supported.

## 3. Task 312 exact change and commit map

| Change | Commit | Independent result |
| --- | --- | --- |
| Shared repository nested-boundary enforcement | `13409e80e15e98f80bd5150416d2d73132784790` | Broadly closes ordinary-marker target and observer gaps, but retains the case-sensitive Windows probe gap documented below. |
| Preserve ordinary symlink observation | `4bbeeac98c0c3e4fd553616bd3cebf44edb4e49b` | Correctly keeps ordinary symlinks as Git leaves for the observer; it does not fix Stage/Unstage's separate full-tree snapshot. |
| Nested-boundary mutator regressions | `4a3a0a46a1bd5b52c001705fe64050a0237f57d8` | Adds real target/pre-effect coverage, but not the Windows case-sensitive form or Stage/Unstage unrelated-B snapshot read. |
| Nested-boundary lint | `85bdf14471ba8556d7aad9cb6758461e19874d09` | Test/lint completion only. |
| Final documentation/evidence head | `de8a9ed526814e19d800762fe1302094a22e0e0c` | Current audit checkpoint. |

The correction did not implement workspace membership or multi-repository
composition. `rah-sandbox::WorkspacePolicy` remains Git-agnostic.

## 4. Shared primitive audit

`rah_tools::repository_boundary::RepositoryNestedBoundaryPolicy` is a narrow
crate-private repository authority helper. It does not grant generic filesystem
authority and is not used to change `WorkspacePolicy`.

### `validate_existing`

The existing-target/creation-parent split is correct in the ordinary form:

- an existing directory is checked itself and then its ancestors;
- an existing file is checked from its parent ancestry;
- a missing target is checked from its immediate parent;
- a missing intermediate parent fails closed because ancestry observation fails;
- the Repository-A root is accepted before checking the root's own `.git`; and
- each non-root directory is checked for containment, link/reparse ambiguity,
  and a child `.git` marker.

The root special case does not allow a normal descendant path to terminate at
the root: component-wise equality is required, while containment is checked
separately. Repository-A root `.git` metadata therefore remains allowed.

The material defect is in `reject_nested_marker` at
`crates/rah-tools/src/repository_boundary.rs:121-129`. It probes only
`directory.join(".git")`. On supported Windows configurations with a
case-sensitive directory, a distinct `.GIT` entry is not necessarily found by
that path probe. The walker at `:89-94` does compare enumerated names with
Windows ASCII-case-insensitive semantics, so the same filesystem can produce
different results for observation and target validation.

Windows supports per-directory case sensitivity on NTFS; Microsoft documents
that case-sensitive directories treat `FOO` and `foo` as distinct and that this
mode is available from Windows 10 build 17107. This is therefore not a safe
“default NTFS only” assumption: <https://learn.microsoft.com/en-us/windows/wsl/case-sensitivity>.

Concrete attack case:

```text
Repository A/
  .git/                 # A metadata
  nested/               # case-sensitive Windows directory
    .GIT/               # distinct child marker
    secret.txt
```

For `validate_existing(Repository A/nested/secret.txt)`, the constructed
`.git` probe can report `NotFound`, allowing the target-bound path to continue.
For `validate_observation`, enumeration sees `.GIT` and rejects it. The
supported Windows path-equivalence contract requires consistent fail-closed
behavior; the current inconsistency is a material gap affecting direct reads,
file-info, and target-bound repository policies. No Unicode normalization is
assumed.

### `validate_observation`

The bounded full-tree walk is correctly placed before fixed status/diff Git
observation. It:

- allows only Repository-A root `.git`;
- recognizes child `.git` names before following their entry type;
- treats ordinary symlinks as Git leaves and does not recurse through them;
- rejects directory reparse points and non-regular ambiguous reparse entries;
- rejects read-directory, entry, metadata, and other filesystem failures;
- rejects an observed tree over `MAX_OBSERVED_DIRECTORIES = 100_000`; and
- returns sanitized boundary errors without exposing filesystem error details.

The conservative scan includes ignored, untracked, and generated directories.
That can reduce availability for large or unreadable ignored trees, but it is a
bounded fail-closed precondition needed to establish a whole-tree boundary and
does not alter the Tool schemas or result taxonomy. Ordinary symlink leaves do
not cause that regression after `4bbeeac`.

The walker has no race-free guarantee. A concurrent removal, replacement, or
creation after a successful scan remains outside the claimed TOCTOU guarantee.

## 5. Windows audit

The following remain sound in the supported ordinary forms:

- `paths_equivalent` and `is_beneath` use component-wise ASCII-insensitive
  comparison;
- root, `.git`, target, parent, executable, and relevant destination identities
  remain capability-specific;
- Windows reparse ancestry is rejected in the applicable target and parent
  policies;
- `.git` symlink/reparse entries are treated as protected or ambiguous; and
- rename retains its stronger alias, destination-collision, identity, and
  post-effect proof.

The direct `.git` construction defect above means target-based validation is not
correct on every supported Windows filesystem configuration. The existing
Windows unit test creates `.GIT` on the default case-insensitive test directory;
it does not establish behavior in a case-sensitive directory. Therefore that
test is not independent evidence for the required contract.

The audit found no evidence that a junction or non-symlink directory reparse
point is traversed by the shared observer. Ordinary symlink leaves are skipped,
while target ancestry symlinks remain rejected. This preserves the narrow
symlink correction but does not cure the case-sensitive marker issue.

## 6. Unix audit

On Unix, `validate_existing` recognizes exact `.git` names, rejects symlink
components in target ancestry, and fails closed on filesystem errors. The
observer uses `symlink_metadata`, recognizes exact `.git`, does not recurse
through symlink leaves, and rejects incomplete directory observations.

Device/inode identity checks, hard-link restrictions, and capability-specific
mount checks remain in their existing policies. Rename retains its Linux mount
observation; the shared helper itself does not claim general nested-mount
detection. Deterministic Unix CI evidence is not Unix live certification.

Invalid UTF-8 names remain byte-safe in the Git observers and are not converted
into a false `.git` marker. No race-free device/inode replacement or mount
claim is made.

## 7. Bare nested repository decision

A bare repository layout at `Repository A/bare-child/` has Git internals in the
child root and no child `.git` marker. The current policy intentionally
recognizes observable child markers only and does not infer a bare repository
from names such as `HEAD`, `objects`, or `refs`.

This is compatible with ADR 0027 as an explicit nonclaim, not as positive
support: the accepted ADR makes nested `.git` boundary enforcement the current
conformance gate, rejects automatic discovery and nested-repository support,
and does not authorize heuristic Git-layout detection. A bare child is not a
supported ordinary worktree target and its detection cannot be made reliable by
an unowned filename heuristic without a new architectural decision.

The nonclaim must remain explicit. If a future supported authority needs to
admit arbitrary paths inside such a child, it must first add a bounded identity
and layout decision. Task 313 does not infer that layout and does not treat
bare-child non-detection as clearance for workspace membership.

## 8. Observer confidentiality and semantics

`RepositoryObserver::run` calls `validate_observation` before each `repo.status`,
`repo.diff`, and `repo.diff-staged` fixed Git command. The full operation fails
closed, so there is no intended sequence in which Git output is parsed and only
then filtered. File-info validates its requested target before Index, HEAD,
tree, status, direct metadata, digest, and worktree facts are collected.

The real Repo-A/Repo-B integration fixture confirms ordinary child `.git`
rejection for `fs.read`, file-info, status, diff, and diff-staged. Its helper
asserts only that execution returns an error. It does not independently inspect
all output fields, native counters, or A/B state; this is evidence of ordinary
marker rejection, not complete confidentiality certification.

### Stage/Unstage snapshot finding

`GitIndexMutationPolicy::capture_state` calls `WorktreeSnapshot::capture`.
`capture_tree` at `crates/rah-tools/src/git_stage.rs:348-380` recursively walks
every non-`.git` directory and reads every regular file. It does not call
`validate_observation` or stop at a nested Repository-B boundary. With:

```text
Repository A/
  .git/
  a.txt
  nested-b/
    .git/
    secret.txt
```

Stage or Unstage of an otherwise valid A target can read `nested-b/secret.txt`
while building its pre/post proof. With a case-sensitive Windows child marker
`.GIT`, it can also recurse into the marker directory and read Git internals.
The target-only nested-B tests reject a B target before the Git index mutation,
but they do not cover an A target plus an unrelated B tree or assert that this
internal snapshot reads no B data. Under ADR 0027's Repository-A authority
invariant, this is a second material production gap.

## 9. Per-capability closure matrix

| Capability | Task 311 finding | Task 312 correction site | Deterministic evidence at current head | Task 313 independent verdict |
| --- | --- | --- | --- | --- |
| `fs.read` | Generic workspace read could open B | `FsReadTool::new_repository`; boundary after `resolve_existing`; Desktop binding | 8 unit tests; nested fixture rejection | **B**: `.GIT` case-sensitive target probe can miss B |
| `repo.file-info` | Direct metadata/digest could inspect B | `RepositoryObserver::validate_target` before Git/direct facts | 4 integration tests; nested fixture rejection | **B**: same target probe gap |
| `repo.status` | Whole status had no preflight | Observer full-tree validation before Status | 4 integration tests; nested fixture rejection | Conditional ordinary-marker closure; **B overall** due shared policy defect |
| `repo.diff` | Whole diff had no preflight | Observer validation before each raw/numstat/patch call | 4 integration tests; nested fixture rejection | Conditional ordinary-marker closure; **B overall** due shared policy defect |
| `repo.diff-staged` | Staged diff had no preflight | Observer validation before each cached diff call | 7 integration tests; nested fixture rejection | Conditional ordinary-marker closure; **B overall** due shared policy defect |
| `repo.create-branch` | No repository-relative target path | No change; fixed ref-only scope | Existing branch unit coverage | Conformant/no-scope for this boundary audit |
| `repo.patch` | Target preflight omitted nested boundary | Shared validation at capture, revalidation, and pre-effect | 49 unit tests, including nested ordinary marker | **B** on case-sensitive marker miss |
| `repo.edit-files` | B and mixed A/B targets could be admitted | Shared validation per target and before each replacement | 33 preflight tests; mixed-boundary unit test | **B** on case-sensitive marker miss |
| `repo.create-file` | Existing B parent could receive a new file | Shared validation at capture, revalidation, and post-proof | 11 integration tests; nested-parent unit test | **B** on case-sensitive marker miss |
| `repo.delete-file` | Clean A-tracked file under B could be deleted | Shared validation at capture, revalidation, and proof | 11 integration tests; nested-target unit test | **B** on case-sensitive marker miss |
| `repo.rename-file` | Reference path; retain stronger checks | Shared validation plus existing ancestry/collision/identity checks | 42 unit tests; 24 preparation tests; serial rerun passed | **B** because shared/direct marker integration is not Windows case-sensitive complete |
| `repo.create-directory` | Partial local marker check | Shared parent validation at capture/revalidation/proof | Unit coverage includes nested parent and zero create attempt | **B** on case-sensitive marker miss |
| Stage | Target B could reach index; no nested check | Shared target validation in construction/revalidation | 5 shared unit tests; 6 integration tests; B-target rejection | **B**: full worktree snapshot can read unrelated B |
| Unstage | Same target/index gap | Shared target validation in construction/revalidation | 5 shared unit tests; 6 integration tests; B-target rejection | **B**: full worktree snapshot can read unrelated B |
| `repo.commit` | Staged paths lacked nested validation | All staged paths checked before `write-tree` and commit | 15 unit tests; stale nested-boundary review test | **B** until shared case form and any staged-state broad reads are closed |
| `host.git.status` | Fixed host-selected status, no model path | No change; outside path-target scope | 4 integration tests plus host tests | Conformant/no-scope as previously classified |

The table separates ordinary-marker deterministic closure from the two
remaining production findings. It does not turn the Windows live or Unix
deterministic limitations into live certification claims.

## 10. `FsReadTool` construction audit

Both constructors remain present:

- `FsReadTool::new` uses only `WorkspacePolicy` and remains Git-agnostic for
  generic workspace use;
- `FsReadTool::new_repository` composes `WorkspacePolicy` with
  `RepositoryNestedBoundaryPolicy`; and
- Desktop repository composition uses `new_repository` at
  `crates/rah-desktop/src/main.rs:5987`.

All production constructor sites were searched. CLI, Trusted Profile, and
runtime/example generic construction remains intentionally generic; no public
input schema changed. Repository-A Desktop `fs.read` rejects an ordinary nested
B target in deterministic tests, while ordinary same-repository nested
directories and outside-root behavior remain covered. The case-sensitive marker
gap prevents full closure.

## 11. Mutator, index, and Commit lifecycle audit

The Task 312 lifecycle placements are correct for ordinary marker forms:

- patch performs admission, retained revalidation, immediate pre-effect
  validation, one replacement attempt, and conservative post-proof;
- multi-file preflights all targets before effects, including mixed A/B sets,
  then revalidates all remaining targets before each native replacement;
- create-file validates the existing parent at preparation and immediately
  before native create, without parent creation or overwrite;
- delete validates at capture, retained revalidation, and proof, preserving
  zero-attempt stale rejection and no replay;
- rename retains source/destination ancestry, destination absence, identity,
  one no-replace attempt, and post-effect proof;
- create-directory validates the existing parent before native creation and
  remains HostExplicit-ineligible;
- Stage/Unstage validate the target before index mutation and immediately
  before the Git action; and
- Commit parses and validates every staged path before `write-tree`/commit and
  rejects a nested boundary appearing after review before the commit attempt.

The independent verdict is nevertheless B because target validation is not
case-sensitive-directory complete, and Stage/Unstage's pre/post worktree
snapshot is a broad internal read that bypasses the boundary walk. No
production repair is made in this audit.

## 12. Public-contract and authority comparison

Compared with the immutable v0.25 source and the current Task 312 head:

| Contract | Finding |
| --- | --- |
| Tool names | Unchanged. |
| Tool input schemas | Unchanged. `FsReadTool::new_repository` is an additive host construction path, not a model input. |
| Output schemas/status taxonomy | Unchanged; existing precondition/stale/uncertain/known-no-effect classes remain sufficient. |
| `PermissionLevel` | Unchanged. `fs.read` remains `Read`; repository Tools retain their existing levels. |
| HostExplicit set | Unchanged exact 11: `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`, `repo.patch`, `repo.edit-files`, `repo.create-file`, `repo.delete-file`, `repo.rename-file`. |
| Excluded set | `repo.create-directory`, `repo.commit`, MCP, Process Plugin, fixture/diagnostic, and unknown/provider-defined Tools remain excluded. |
| `WorkspacePolicy` | Still generic and Git-agnostic. |
| ADRs/provider contracts | No contract amendment or provider bypass was introduced. Existing capability ADRs remain authoritative. |

The stricter preconditions are within ADR 0027's accepted hardening boundary,
but the remaining case-sensitive and snapshot findings prevent claiming that
the implementation satisfies that boundary. No public authority expansion or
schema drift was observed.

## 13. Actual deterministic test evidence

All checks were run on the exact current head. The full workspace test completed
with zero failures; pre-existing host-only tests remained ignored by their
environment gates. No connected Codex live test was run.

| Check/suite | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo metadata --no-deps --format-version 1` | PASS; 13 packages, 13 members, all `0.25.0`, edition 2024, zero manifest dependency changes from v0.25 source |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS; workspace tests reported zero failures; `rah-tools` 277 passed; desktop 219 passed / 10 ignored; all other reported suites passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS before this document |
| `rah-tools` lib | 277 passed |
| `repository_nested_boundary` | 1 passed |
| `repository_file_info` | 4 passed |
| `repository_status` | 4 passed |
| `repository_diff` | 4 passed |
| `repository_diff_staged` | 7 passed |
| `repository_worktree_patch::tests` | 49 passed |
| `repository_multi_file_preflight::tests` | 33 passed |
| `repository_create_file` | 11 passed |
| `repository_delete_file` | 11 passed |
| `repository_commit::tests` | 15 passed |
| `repository_rename_file::tests` | 42 passed with `--test-threads=1` after one earlier stale test-fixture lock artifact; no production failure remained |
| `repository_rename_preparation` | 24 passed |
| `git_stage` | 6 passed |
| `git_unstage` | 6 passed |

The focused tests use real temporary Git repositories where stated. The
Task-312 nested observer integration fixture creates real Repo A and Repo B,
but its assertions are error-only. Existing mutator counters are connected at
the native effect points in their dedicated suites; the nested fixture does not
provide a complete all-capability zero-attempt/state matrix.

Counts are evidence of executed tests, not proof that the Windows
case-sensitive-directory or Stage/Unstage snapshot attack cases are closed.

## 14. Remaining limitations and nonclaims

The following remain explicit nonclaims and are not themselves Task 313
failures:

- no race-free TOCTOU guarantee;
- no automatic retry, replay, rollback, cleanup, or compensation after an
  uncertain effect;
- no Unix/macOS live certification;
- no Windows current-master live certification;
- no OS sandboxing or network-isolation claim;
- no linked-worktree support;
- no bare nested-repository inference or support;
- no workspace-wide filesystem/Git authority;
- no workspace membership or explicit admission implementation;
- no provider, MCP, Process Plugin, or model-selected repository authority;
- no HostExplicit `repo.commit` or `repo.create-directory`; and
- no use of membership state as a nested-boundary security mechanism.

The remaining production blockers are not nonclaims:

1. `RepositoryNestedBoundaryPolicy::reject_nested_marker` must establish
   Windows path-equivalent marker detection even when a directory is explicitly
   case-sensitive. The correction must preserve ASCII-only comparison and must
   not introduce Unicode normalization assumptions.
2. Stage/Unstage's `WorktreeSnapshot::capture_tree` must not recursively read a
   supported nested Repository-B worktree or Git marker while proving an A
   index operation. It must retain ordinary same-repository behavior, symlink
   and reparse fail-closed semantics, and existing one-attempt/uncertain rules.

## 15. Verdict basis

The ordinary `.git` fixture correction is substantially wired and deterministic
tests pass, but the implementation is not conformant on every supported
Windows filesystem configuration and Stage/Unstage still performs a broad
internal read through a nested Repository-B boundary. Either finding is
material under ADR 0027. Workspace membership foundation work must not begin.

## 16. Exact next task

Task 314 — Windows case-sensitive nested-marker and Stage/Unstage snapshot
boundary correction.

Task 314 must remain narrowly scoped to:

- replacing the constructed-name-only child-marker check with a bounded,
  path-equivalence-correct, fail-closed marker observation for Windows
  case-sensitive directories;
- adding deterministic Windows case-sensitive-directory coverage where the
  environment supports it, or documenting an explicit fail-closed unsupported
  result at the constructor/policy boundary;
- making Stage/Unstage's worktree proof boundary-aware without broadening index
  authority or changing public schemas;
- adding real Repo A/Repo B tests for direct reads, target-bound policies,
  unrelated-A Stage/Unstage snapshots, mixed targets, zero native attempts,
  and unchanged A/B state; and
- rerunning the independent audit gates and preserving all no-replay,
  uncertainty, symlink/reparse, gitlink, rename, and public-contract rules.

It must not implement workspace membership, repository admission, workspace-wide
Tool composition, frontend polish, versioning, release work, or live
multi-repository certification. Task 313 performed no correction.

Verdict B — CORRECTION STILL REQUIRED
