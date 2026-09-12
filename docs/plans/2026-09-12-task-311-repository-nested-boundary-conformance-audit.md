# Task 311 — RAH v0.26 Repository Nested-Boundary Conformance Audit

Status: Audit complete

## 1. Authoritative checkpoint

| Item | Value |
| --- | --- |
| Branch | `master` |
| Current master / HEAD | `3aa30494c344846b57fd416f0039116f58a12aa0` |
| Direct parent | `f3de3ebb96f17d56195ff72ac74591e08d2c7119` |
| ADR decision commit | `f3de3ebb96f17d56195ff72ac74591e08d2c7119` |
| Task 310 final verification commit | `3aa30494c344846b57fd416f0039116f58a12aa0` |
| Task 310 exact-head CI | `34668435629` — PASS |
| `origin/master` at audit start | `3aa30494c344846b57fd416f0039116f58a12aa0` |
| Current release | RAH v0.25.0 — RELEASED |
| Immutable v0.25 source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| Annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| GitHub Release | `387406579` |
| Workspace | 13 packages, version `0.25.0`, Rust edition 2024 |
| HostExplicit eligible set | exactly 11 names, unchanged |

The audit began with `git status --short`; the worktree was clean. The current
checkout and `origin/master` matched the supplied checkpoint. The repository
has `home` as a private network remote and `origin` as the GitHub remote; this
audit targets `origin/master` for delivery.

This document is the only intended Task 311 change. No Rust, test,
frontend/Tauri, Tool schema, ADR, version, dependency, tag, or Release change
is part of the audit.

## 2. ADR 0027 invariant

ADR 0027 is accepted and remains the authority for this audit. Its relevant
invariant is:

> For Repository A authority, every repository-bound path capability fails
> closed when the requested target/path ancestry crosses a distinct nested Git
> repository boundary, even when the nested repository is not a workspace
> member.

The invariant covers reads, observation, worktree mutation, creation,
deletion, rename, and index-target validation. Workspace membership is
descriptive and cannot be the protection mechanism. ADR 0027 also preserves
the existing capability-specific contracts and says that uniform nested
boundary enforcement is a prerequisite for later implementation.

Relevant decision text: `docs/adr/0027-workspace-repository-identity-authority-composition.md:118-132`,
`216-231`, `280-287`, and `289-311`.

## 3. Audit method and capability inventory

The audit inspected the current first-party implementation in `rah-sandbox`,
`rah-tools`, and `rah-desktop`, the relevant accepted ADRs, repository identity
and Git support, Desktop workflow target derivation, and deterministic tests.
No destructive or live certification test was run.

The complete scoped capability inventory is 16 capability surfaces:

1. `fs.read`;
2. `repo.file-info`;
3. `repo.status`;
4. `repo.diff`;
5. `repo.diff-staged`;
6. `repo.create-branch`;
7. `repo.patch`;
8. `repo.edit-files`;
9. `repo.create-file`;
10. `repo.delete-file`;
11. `repo.rename-file`;
12. `repo.create-directory`;
13. `repo.commit`;
14. `host.git.stage`;
15. `host.git.unstage`; and
16. `host.git.status`, the existing host-selected fixed Git status capability.

`fixture-marker` repository mutation, generic `ShellExecTool`, provider Tools,
and Git executable discovery were reviewed as supporting or out-of-scope
surfaces. They do not add another repository-relative target capability to
this matrix. The fixture marker is fixed host configuration rather than a
model-selected repository path.

The exact production HostExplicit set remains the 11 names recorded by ADR
0027. `repo.create-directory`, `repo.commit`, and Stage/Unstage remain outside
that set; ineligibility does not exempt an underlying repository authority from
the ADR 0027 boundary.

## 4. Per-capability source map

| Capability | Primary source map | Relevant current behavior |
| --- | --- | --- |
| `fs.read` | `crates/rah-tools/src/fs_read.rs:12-111`; `crates/rah-sandbox/src/workspace.rs:43-157` | Uses generic `WorkspacePolicy::resolve_existing`; no nested Git check. |
| `repo.file-info` | `crates/rah-tools/src/repository_file_info.rs:22-108`, `143-166`, `363-458` | Accepts a repository-relative path and directly observes the worktree path; no nested boundary check. |
| `repo.status` | `crates/rah-tools/src/repository_status.rs:26-79`; observer command construction at `crates/rah-tools/src/repository_observer.rs:143-158` | Whole-Repository A fixed Git status; no nested boundary preflight. |
| `repo.diff` | `crates/rah-tools/src/repository_diff.rs:39-135`; fixed commands at `crates/rah-tools/src/repository_observer.rs:159-217` | Whole-Repository A worktree/index diff; no nested boundary preflight. |
| `repo.diff-staged` | `crates/rah-tools/src/repository_diff_staged.rs:22-64`; staged commands at `crates/rah-tools/src/repository_observer.rs:218-280` | Whole-Repository A index/HEAD diff; no nested boundary preflight. |
| `repo.create-branch` | `crates/rah-tools/src/repository_branch_create.rs:242-275`, `374-460`, `621-670` | Validates a branch/ref name and fixed Repository A ref state; no filesystem target path. |
| `repo.patch` | `crates/rah-tools/src/repository_worktree_patch.rs:446-515`, `649-705`, `854-930`, `1051-1127`, `2057-2078` | Validates containment, identity, Git state, and preimage, but `validate_existing_target` does not inspect nested `.git`. |
| `repo.edit-files` | `crates/rah-tools/src/repository_multi_file_edit.rs:17-97`; shared policy at `crates/rah-tools/src/repository_multi_file_preflight.rs:792-940`, `974-1035`, `1109-1134`, `1244-1345`, `1728-1800` | Every target gets canonical/identity/currentness checks, but no nested Git boundary check. Mixed A/B targets are not rejected as a class. |
| `repo.create-file` | `crates/rah-tools/src/repository_create_file.rs:254-361`, `389-510`, `644-770`; native helper at `crates/rah-tools/src/native_repository_create.rs:67-187` | Validates parent identities, links/reparse points, absence, Git paths and gitlinks; does not reject an ordinary nested `.git` parent. |
| `repo.delete-file` | `crates/rah-tools/src/repository_delete_file.rs:278-465`, `520-590`, `675-845` | Existing-target and clean HEAD/index proof; no nested Git boundary check. |
| `repo.rename-file` | `crates/rah-tools/src/repository_rename_file.rs:715-925`, `947-1058`, `1686-1723`, `1771-1808` | Capability-specific source/destination ancestry checks reject `.git` metadata, links/reparse points, and observed Linux mount points. |
| `repo.create-directory` | `crates/rah-tools/src/repository_create_directory.rs:77-155`, `180-290`, `319-331` | `has_nested_metadata` rejects an existing `.git` entry in existing parent ancestry, but is a local check and does not identify every distinct nested repository form. |
| `repo.commit` | `crates/rah-tools/src/repository_commit.rs:118-168`, `198-233`, `236-363`, `796-819` | No arbitrary path input. It snapshots/commits A staged state and rejects gitlink mode `160000`, but does not reject ordinary staged paths whose worktree ancestry later contains nested B. |
| `host.git.stage` | `crates/rah-tools/src/git_stage.rs:23-70`, `80-219`, `275-321`, `424-437` | Host derives one absolute target, canonicalizes it beneath A, and uses literal Git pathspecs; no nested boundary check. |
| `host.git.unstage` | `crates/rah-tools/src/git_unstage.rs:18-65`; shared `GitIndexMutationPolicy` above | Same target admission and missing nested boundary check as Stage. |
| `host.git.status` | `crates/rah-tools/src/git_status.rs` and its fixed host-selected repository construction | No repository-relative target path; fixed A status observation, therefore scope-only for this path-boundary audit. |

Shared foundations inspected include `WorkspacePolicy`, `RepositoryIdentity`,
`RepositoryObserver`, `FileIdentity`, `paths_equivalent`, `is_beneath`, native
descriptor/handle-relative creation, multi-file preflight, Desktop staged
target derivation, and repository workflow selection.

## 5. Known strong reference: `repo.rename-file`

`repo.rename-file` is the current strong reference for the supported ordinary
worktree boundary contract.

### Detection location and coverage

`validate_ordinary_directory_ancestry` walks each existing source and
destination parent upward toward Repository A (`repository_rename_file.rs:1686-1715`).
At every directory it calls `reject_nested_repository_boundary`, which rejects
any existing `.git` entry using `symlink_metadata` (`1686-1723`). This means an
ordinary `.git` directory and a `.git` file are treated as a boundary marker;
the helper does not require `.git` to be a directory.

The source parent is checked in `capture` at `715-815`; the destination parent
is checked by `destination` at `838-847`. Therefore the ordinary supported
cases are covered:

- source A to destination below nested B: destination ancestry rejects;
- source below nested B to destination A: source ancestry rejects;
- source below nested B to destination below nested B: source and destination
  ancestry reject; and
- source/destination parents crossing the boundary: the upward walk rejects.

`destination_git_absent` separately checks A HEAD and index collisions,
including Windows case-equivalent forms (`1028-1108`). It is not itself the
nested-boundary validator and must not be copied as a generic replacement.

### Prepare, currentness, effect, and proof

The reviewed preparer captures source/destination, parent identities, content,
Git state, and index state (`756-815`), and `revalidate` repeats the complete
capture before the native rename (`849-861`). The ordinary Tool also performs
precondition capture, immediate revalidation, one native rename attempt, and
independent post-effect proof (`repository_rename_file.rs:496-608`). Post-proof
rechecks both parent ancestries and destination identity/content (`869-930`).

This places the nested check in admission, retained-preparation revalidation,
immediate pre-effect validation, and post-effect verification. A nested
boundary created after Prepare is therefore stale/rejected before effect in
the existing reviewed path; the tests record this at
`crates/rah-tools/tests/repository_rename_preparation.rs:556-564`.

### Platform and form limitations

The implementation rejects source/destination symlink or Windows reparse
ancestry (`reject_link_or_reparse`), compares Windows paths case-insensitively
through component-wise `paths_equivalent`, and checks observed Linux mount
points (`1686-1715`, `1725-1768`). Windows post-proof also counts aliases in
the parent directory (`1771-1808`). It does not claim race-free TOCTOU.

The supported top-level repository contract requires a real directory-form
`.git`; linked worktrees and `.git` indirection are rejected by
`validate_supported_dot_git` (`1203-1210`). A nested `.git` file, malformed
`.git` entry, or reparse `.git` entry is conservatively treated as an existing
boundary marker when it is observable. A nested bare repository directory has
no `.git` child marker and is not positively identified by this helper. That is
an unsupported-form evidence limitation, not linked-worktree support.

Conclusion: rename conforms for the existing ordinary-worktree nested `.git`
boundary contract and is not evidence that other capabilities conform.

## 6. Nested-boundary matrix

Legend: **A** = conforms; **B** = indirectly conformant through an applicable
shared primitive; **C** = no repository-relative target path and not applicable;
**D** = material conformance gap. “No” in the child access column means the
current implementation rejects the ordinary child `.git` case; “Yes” means a
target can reach B under the stated current contract. “Conditional” identifies
the whole-repository observers whose fixed Git output normally collapses an
untracked nested repository to its boundary but can expose A-tracked paths
whose ancestry later becomes a nested repository.

| Capability | Repository-relative target? | Current nested boundary check? | Shared primitive used | Prepare-time check | Pre-effect revalidation | Post-effect relevance | Nested child readable/mutable today? | Windows alias/reparse coverage | Unix symlink/mount coverage | ADR 0027 verdict | Correction required? | Public-contract impact | Test gap |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fs.read` | Yes | No; `WorkspacePolicy` only | `WorkspacePolicy` | N/A | No repository-specific recheck | Read has no post-effect | **Yes**: `Repo A/nested/secret.txt` resolves and reads if it is inside canonical A | Generic canonicalization; no Git boundary; symlink escape handling is generic | Generic symlink containment; no nested Git or mount boundary | **D** | Yes | Same `{path}` input and Read permission; stricter repository composition only | No nested `.git` fixture |
| `repo.file-info` | Yes | No | `RepositoryObserver` plus direct `observe_worktree` | Admission only validates A identity | Observer revalidates A identity, not target ancestry | No native effect; final direct observation can cross | **Yes**: direct worktree metadata/digest can observe `nested/secret.txt` | Rejects target links/reparse components; no nested Git check or Windows nested aliases | Rejects symlink ancestry for observed components; no mount/nested Git check | **D** | Yes | No schema change | No nested repository target test |
| `repo.status` | No; whole A observation | No | `RepositoryObserver` | N/A | A root identity only | No native effect | **Conditional**: ordinary untracked B is summarized at `nested/`; A-tracked paths under a later B can be reported | Root/reparse identity only | Root identity only; no nested traversal policy | **D** for boundary-complete observation | Yes | Same empty input/output contract; stricter filtering/rejection | No nested B status behavior test |
| `repo.diff` | No; whole A observation | No | `RepositoryObserver` + diff parser | N/A | A root/HEAD revalidation only | No native effect | **Conditional**: Git normally ignores untracked B internals, but A-tracked worktree paths under B can enter diff output | No nested Git check; root identity/reparse only | No nested Git or mount check | **D** | Yes | Same empty input/output contract | No nested B diff test |
| `repo.diff-staged` | No; whole A index/HEAD observation | No | `RepositoryObserver` + diff parser | N/A | HEAD and A identity revalidation | No native effect | **Conditional**: staged A entries beneath a later B can expose B worktree-origin data; gitlinks are represented separately | No nested Git check; gitlink parser support is not boundary enforcement | No nested Git or mount check | **D** | Yes | Same empty input/output contract | No nested B staged-diff test |
| `repo.create-branch` | No | Not applicable; fixed refs only | `RepositoryIdentity` / fixed Git policy | Ref admission | `revalidate_static` and ref observations | Ref post-proof | **No path reachability**; branch name cannot select B | Repository identity/reparse checks; no target path | Repository identity checks; no target path | **C** | No | None | No path-boundary test needed; ref-scope test exists |
| `repo.patch` | Yes | No | Capability-specific patch policy; `validate_existing_target` | Prepare captures target/parent/Git state but no nested check | Repeats same missing check immediately before replacement | Post-proof checks target identity/content, not nested ancestry | **Yes** if target is an A-tracked file below B; ordinary containment does not stop it | Rejects symlink/reparse target components and Windows aliases; no nested Git marker | Rejects symlink components; no nested Git or mount check | **D** | Yes | No schema/status/permission expansion | No nested B patch or zero-attempt test |
| `repo.edit-files` | Yes, multiple | No | `RepositoryMultiFileMutationPolicy` / `SafeTarget` | Per-target preflight, no nested check | Per-target and global revalidation, same omission | Per-target replacement certification, same omission | **Yes**; one target can be B, and mixed A/B targets are not rejected | Identity, link/reparse, alias checks; no nested Git check | Link checks; no nested Git or mount check | **D** | Yes | No schema change; retain bounded target list | No mixed A/B nested fixture |
| `repo.create-file` | Yes; parent must exist | No; gitlink-only ancestor check | `NativeParent` and file-creation policy | Parent handles/identity and absence; no `.git` check | Repeats parent and repository snapshot; same omission | Created object/parent/Git snapshot proof; no nested check | **Yes**: `nested/new.rs` can be created below an existing ordinary B parent | Native Windows handle walk rejects reparse parents; no nested Git check | Native Unix directory walk rejects symlinks; no nested Git/mount check | **D** | Yes | No schema/status/permission expansion | No create-under-B test |
| `repo.delete-file` | Yes | No | Deletion policy + `validate_existing_target` | Clean HEAD/index review, no nested check | Repeats same existing-target/Git proof | Deletion proof checks absence and A state, not nested ancestry | **Yes** for a clean A-tracked file below B | Rejects links/reparse and identity aliases; no nested Git check | Rejects symlinks/hard links; no nested Git/mount check | **D** | Yes | No schema change | No delete-under-B test |
| `repo.rename-file` | Yes, source and destination | **Yes for supported `.git` marker forms** | Local `validate_ordinary_directory_ancestry` | Source/destination parent walk | Full capture repeated immediately before effect | Parent ancestry and destination identity rechecked | **No** for ordinary `.git` child boundaries | Case-equivalent paths, reparse/symlink rejection, alias post-proof; no bare-root recognition | Symlink rejection and observed Linux mounts; no race-free claim | **A*** | No for supported contract; test/evidence gap for nested bare/unknown forms | None | Missing complete cross-direction ordinary nested matrix; existing tests cover destination and appearance-after-prepare |
| `repo.create-directory` | Yes; existing parent | Partial local `.git` entry test | `has_nested_metadata`, `NativeParent` | Capture checks each existing parent and Git snapshot | Repeats partial check | Post-proof has parent identity and A snapshot | **Conditional/Yes** for unsupported bare nested root; ordinary `.git` parent is rejected | Reparse parent rejection; `.git` marker test uses `exists`, not a shared identity-aware validator | Symlink/reparse parent rejection; no mount or bare-root detection | **D** for complete invariant | Yes | No public expansion; remains ineligible HostExplicit | No nested `.git`, file-form, bare, or post-prepare tests |
| `repo.commit` | No arbitrary target; staged snapshot | No nested path-boundary check | `RepositoryIdentity`, `RepositoryObserver`, staged snapshot | Review/authorization of A staged state | Snapshot/currentness revalidation; does not inspect nested ancestry | Commit proof validates A commit state, not B ancestry | **Indirectly yes** if Stage/A index contains a path below later B; gitlink mode itself is rejected | A root/.git identity only; no nested path walk | A root identity only; no nested path/mount walk | **D (indirect)** | Yes, together with Stage/Unstage boundary correction | No message/schema/permission change; stricter staged-state admission | No commit-after-nested-boundary test |
| `host.git.stage` | Host-selected absolute target derived from A | No | `GitIndexMutationPolicy` | Desktop action catalog constructs target from A status | Root/.git/target identity revalidation; no nested check | Index/worktree snapshot proof, no nested check | **Yes** when target remains A-tracked below B; Desktop selection does not add boundary proof | Case-insensitive containment and reparse/symlink rejection; no nested Git check | Symlink rejection; no nested Git/mount check | **D** | Yes | Same empty schema and host-owned target contract | No Stage-under-B fixture or zero-attempt rejection |
| `host.git.unstage` | Host-selected absolute target derived from A | No | `GitIndexMutationPolicy` | Desktop staged review derives target | Same missing check | Index/HEAD proof, no nested check | **Yes** for an A-tracked target below B | Same as Stage | Same as Stage | **D** | Yes | Same empty schema and host-owned target contract | No Unstage-under-B fixture |
| `host.git.status` | No | Not applicable; fixed A status | Fixed host Git policy | N/A | A repository identity revalidation | No native effect | No model-selected path; outside this path-target invariant | Host repository identity/reparse policy | Host repository identity policy | **C** | No | None | No nested-specific test required for path scope |

`A*` means conformant for the current supported ordinary-worktree `.git`
boundary contract, while the matrix records the separate unsupported nested
bare/unknown-form evidence limitation. This does not change the final verdict:
the other rows already establish material production gaps.

## 7. Direct answers for required cases

### `fs.read`

For:

```text
Repo A/.git/
Repo A/nested/.git/
Repo A/nested/secret.txt
```

`WorkspacePolicy::resolve_existing` canonicalizes the candidate and accepts it
when it remains beneath A (`workspace.rs:76-89`, `144-157`). `FsReadTool` then
opens and reads the resolved file (`fs_read.rs:61-109`). There is no nested
`.git` check. Therefore A authority can read `nested/secret.txt` today. This
is a material ADR 0027 gap even though the operation is non-mutating.

### `repo.file-info`

The path parser rejects traversal, separators, NUL, and any path component
named `.git` case-insensitively, but it does not reject a normal component whose
ancestor contains `.git` (`repository_file_info.rs:143-166`). The tool runs
fixed A Git observations and then directly calls `observe_worktree` on the
requested path (`59-108`, `363-458`). A nested B file therefore receives direct
worktree metadata and, for a regular file under the digest limit, a content
digest. This is independently a gap, not merely an `fs.read` alias.

### `repo.patch`

The reviewed path captures exact content, A Git state, and target/parent
identity, and revalidates them before replacement. The target admission path
only walks path components for links/reparse points and canonical containment
(`repository_worktree_patch.rs:2057-2078`). It does not inspect `component/.git`.
If an A-tracked file is below an ordinary nested B, the path can pass both
Prepare and the ordinary Tool policy. The UI/review restriction cannot repair
the underlying Tool path.

### `repo.edit-files`

`SafeTarget::capture` and `revalidate` enforce canonical target, parent and
file identities, links/reparse points, hard-link restrictions, bytes, Git
stage-0 state, and worktree cleanliness. They do not inspect nested Git
metadata (`repository_multi_file_preflight.rs:1728-1800`, `1304-1336`). The
policy loops over each target and permits a change set containing one target
below B and another in A (`974-1035`). The whole capability is therefore
affected; current per-target proof is not a boundary proof.

### `repo.create-file`

The creation policy opens the existing parent using the native descriptor or
handle-relative helper, checks absence, and calls `require_git_absent`.
`require_git_absent` rejects a Gitlink ancestor (`160000`) but does not inspect
an existing nested `.git` entry (`repository_create_file.rs:686-770`). The
native helper protects against links/reparse redirection and parent replacement;
it does not own Git boundary semantics (`native_repository_create.rs:67-105`).
Consequently `nested/new.rs` can be admitted when `nested/.git` exists.

### `repo.delete-file`

Both ordinary execution and reviewed preparation eventually use
`validate_existing_target`, clean A HEAD/index state, and target identities
(`repository_delete_file.rs:756-845`). No source-parent ancestry walk checks
for `.git`. The reviewed clean-HEAD subset is narrower than the underlying
authority, but both the preparer and ordinary Tool share the omission.

### `repo.rename-file`

The strong-reference result above is confirmed for ordinary `.git` directory
and file markers, source/destination parent ancestry, immediate revalidation,
and post-effect proof. The helper is capability-specific because it also
contains rename-only destination Git collision and Windows alias logic. It is
not a generally reusable semantic primitive without separating the shared
boundary predicate from rename-specific collision/proof logic.

### `repo.create-directory`

This ineligible capability is not exempt. Its `has_nested_metadata` walk
rejects an existing `.git` entry in each existing parent (`319-331`) and the
capture/revalidate/post-proof lifecycle repeats that check. It is stronger than
most current paths for ordinary `.git` markers, but it does not recognize a
nested bare repository root or establish a shared fail-closed unsupported-form
policy. It therefore requires correction before it can be claimed as complete
ADR 0027 conformance.

### Stage / Unstage

Desktop status/staged review produces an A-relative entry path, calls
`observe_regular_target`, and constructs `GitStageTool` or `GitUnstageTool`
with `repository.root.join(entry.path)` (`crates/rah-desktop/src/main.rs:5316-5383`).
The action is later reconstructed from the stored target and executed after a
target-currentness check (`5815-5872`). Neither layer checks nested Git
ancestry.

`GitIndexMutationPolicy` canonicalizes the target, requires it to be a regular
file beneath A, uses literal pathspecs, and snapshots A HEAD/refs/index and
the worktree (`git_stage.rs:94-219`). It can reject links, reparse points,
identity replacement, and unrelated index changes, but not a new or existing
nested `.git`. Git's automatic treatment of an untracked nested repository is
not the RAH boundary: an A-tracked path can remain in A's index while a child
repository appears beneath it. Stage and Unstage therefore have a material
gap, and the Desktop derivation cannot be treated as a compensating control.

### Commit

`repo.commit` has no arbitrary repository-relative path input. It is therefore
not a direct target selector. It is nevertheless indirectly affected: its
staged snapshot accepts ordinary stage-0 entries and rejects only conflicts,
gitlinks, and intent-to-add (`repository_commit.rs:796-819`). It does not
walk staged path ancestry for nested repositories. Until Stage/Unstage and the
staged-state admission are corrected, Commit cannot be claimed to preserve the
boundary for a staged A path below B.

### Status / Diff / Diff-staged

These tools take empty input and execute fixed whole-A Git commands. They are
not arbitrary path selectors. Git normally reports an untracked ordinary B as
the nested directory rather than recursively exposing B, and the commands use
`--ignore-submodules=all`. That behavior is not a RAH authority proof. A
tracked A path whose ancestry becomes B can still be represented in status and
diff output. The observer has no nested boundary admission or output-path
filter. They are therefore included as D for boundary-complete repository
observation, with the ordinary untracked-B behavior recorded as a non-gap
observation fact.

### `repo.create-branch`

The model supplies only a branch name. The policy validates ref syntax,
observes Repository A HEAD/local heads/reflogs, and executes fixed ref
operations (`repository_branch_create.rs:294-460`). No filesystem path can
select B. It is C, not a claimed implementation of path rejection.

## 8. Shared primitive and `WorkspacePolicy` placement analysis

### Current shared foundations

`WorkspacePolicy` canonicalizes a root and validates lexical/canonical
containment, existing ancestors, and generic links. It is intentionally a
filesystem sandbox primitive (`crates/rah-sandbox/src/workspace.rs:43-157`).
It does not know Git roots, `.git` forms, repository identity, submodules, or
repository leases.

`RepositoryIdentity` validates A's root, A's own `.git` entry, filesystem
identity, and revalidation (`crates/rah-tools/src/repository_observer.rs:346-390`).
It does not traverse A descendants. `validate_existing_target`, `SafeTarget`,
the create native helper, and Git index target handling each implement related
but capability-specific path safety.

### Options

| Option | Assessment |
| --- | --- |
| A — extend generic `WorkspacePolicy` with Git awareness | Rejected. It couples a reusable generic workspace/filesystem primitive to Git topology, repository identity, submodule semantics, and platform-specific repository policy. It would also make non-repository `fs.read` unexpectedly Git-aware. |
| B — add a repository-specific descendant validator above `WorkspacePolicy` | Better than A for read composition, but insufficient as the single location because most repository Tools do not use `WorkspacePolicy`; it would leave duplicate mutation/index implementations. |
| C — duplicate capability-specific `.git` checks | Rejected. It repeats boundary semantics, form handling, revalidation, and platform behavior, and would recreate the current drift. |
| D — shared repository-boundary primitive used by repository policies while `WorkspacePolicy` remains filesystem-only | Recommended. Keep the primitive crate-private/narrow, parameterized by the already host-owned Repository A identity and supported-form policy, and call it from target admission, Prepare, immediate pre-effect revalidation, and relevant observation/index/commit paths. |

### Recommended correction boundary

Task 312 should add one narrowly scoped repository-boundary validation layer in
the repository-authority implementation, not a workspace-root authority. The
primitive should:

- walk the requested existing target ancestry and the relevant destination or
  creation-parent ancestry from A toward, but not through, A's own root;
- treat an observable `.git` directory, `.git` file, symlink/reparse `.git`,
  malformed metadata, and other unsupported ambiguous repository markers as a
  fail-closed nested boundary according to the existing support contract;
- preserve the existing rejection of links, junctions, reparse points, path
  aliases, and identity replacement;
- make platform-specific path-equivalence and filesystem identity checks part
  of the repository primitive, without claiming race-free TOCTOU;
- provide explicit observation/index output-path admission for fixed whole-A
  observers where an A-tracked path can expose B data; and
- be invoked again at the current/pre-effect point rather than only at UI
  review or first Prepare.

The primitive must remain distinct from rename's Git collision search,
multi-file replacement ordering, create parent handles, deletion proof, and
Stage/Unstage index snapshots. Those capability-specific proofs remain in
their existing policies.

The nested bare-repository question must be resolved as part of the correction
contract. If the current supported form cannot safely recognize a bare child,
the conservative result is rejection of the ambiguous form, not admission of
the path and not expansion of bare-repository support.

## 9. Windows analysis

Current code provides useful but non-uniform Windows protections:

- `paths_equivalent` and `is_beneath` compare path components
  case-insensitively on Windows (`crates/rah-tools/src/host_execute.rs:377-409`);
- repository and target policies use Windows file identities where implemented;
- target and parent link/reparse checks inspect `FILE_ATTRIBUTE_REPARSE_POINT`;
- native create uses handle-relative `NtCreateFile`/reparse rejection
  (`native_repository_create.rs:408-478`); and
- rename post-proof enumerates aliases in source/destination parents.

The missing nested checks therefore affect, at minimum:

- case-equivalent `.GIT`/`.git` marker spellings;
- case-equivalent aliases of nested roots and target paths;
- junction/reparse ancestry where the path policy permits the ordinary
  directory but does not perform a Git-boundary walk;
- drive-letter and supported canonical aliases;
- trailing-dot/space and ADS ambiguity according to each existing path
  contract; and
- replacement of a validated descendant or `.git` marker after Prepare.

Rename has the strongest current alias/reparse handling, but its helper is not
shared. The missing paths cannot claim Windows conformance merely because
their canonical containment checks are case-insensitive. No current code
claims device/UNC/verbatim support; those forms remain rejected or unsupported
according to each existing parser. No race-free protection is claimed.

## 10. Unix analysis

Current generic and repository path policies reject ordinary symlink traversal
in many mutation paths, and Unix identities use device/inode pairs where
implemented. Rename additionally compares observed Linux mount points while
walking ordinary directory ancestry (`repository_rename_file.rs:1691-1708`,
`1725-1768`).

The missing paths do not uniformly reject a nested `.git` directory or file,
and do not uniformly check nested mounts. `fs.read` uses canonical containment
and can read a nested B file. File-info performs direct `symlink_metadata` and
digest operations but has no mount or nested-Git boundary policy. Patch,
multi-edit, delete, create-file, and Stage/Unstage have local symlink and
identity defenses but no nested Git walk. These are deterministic policy gaps,
not claims about Linux/macOS live certification.

The correction must retain conservative behavior for device/inode replacement
and mount ambiguity. It must not claim that a check-then-effect sequence is
race-free or that a mount disappearing/reappearing is automatically safe.

## 11. Nested `.git` forms and submodule/gitlink distinction

| Form | Current observation | Audit treatment |
| --- | --- | --- |
| Ordinary child `.git` directory | Rename and create-directory reject; most other paths do not | Required boundary; affected paths are production gaps. |
| Child `.git` file / linked-worktree form | Rename's marker walk and create-directory's existence check reject an existing marker; most ordinary repository policies reject linked-worktree form only for A itself | A nested marker is a boundary/unsupported form, not permission to follow it. Preserve the existing no-linked-worktree admission contract. |
| Symlink/reparse `.git` | Rename and create-directory can reject when the marker is observable; coverage is not shared; other paths omit the check | Fail closed as an ambiguous boundary. Do not follow it. |
| Malformed `.git` entry | No uniform detection | Recognition/uncertainty is a correction requirement; unsupported evidence must fail closed. |
| Nested bare repository directory | No `.git` child marker; no uniform detection | Distinct Git root is not currently recognized by most paths, and is an unsupported-form test/correction gap. Do not broaden bare repository support. |
| Submodule / gitlink (`160000`) | Several parsers explicitly identify or reject gitlinks; create-file rejects a gitlink ancestor; commit rejects staged mode `160000` | A gitlink is not an ordinary nested worktree. Existing submodule handling remains separate, but it cannot be used as proof that an ordinary nested B is safe. |

An ordinary nested repository physically below A is not converted into a safe
submodule merely because Git summarizes it as a directory. Conversely, a
submodule/gitlink is an A index entry representing a separate repository and
must retain its existing explicit rejection/handling. ADR 0027 requires both
the ordinary nested boundary and gitlink distinction to remain explicit.

## 12. Prepare/currentness/pre-effect analysis

The required lifecycle distinction is:

```text
static construction/admission
  -> zero-effect Prepare/review
  -> currentness/Confirm checks
  -> immediate pre-effect validation
  -> native/Git effect
  -> post-effect proof or conservative uncertainty
```

Current coverage is uneven:

- rename repeats nested ancestry at capture, Prepare revalidation,
  pre-effect validation, and post-proof;
- patch, multi-edit, create-file, and delete repeat identity, bytes, Git state,
  and parent checks, but repeat no nested boundary check;
- create-directory repeats its local `.git` existence check, but not a shared
  complete form policy;
- file-info and observer tools revalidate A identity around fixed Git calls but
  do not validate requested/output path ancestry;
- Stage/Unstage revalidate A/target identities and Desktop action currentness,
  but not nested ancestry; and
- Commit revalidates its staged snapshot/currentness but not nested path
  ancestry.

A nested B can appear after Prepare and before effect in every affected
mutation/index route whose final check omits the boundary. Task 311 does not
fix that race. Task 312 must place the shared check at target admission and
again at the immediate pre-effect/currentness boundary. If observation after a
possible effect becomes ambiguous, existing known-no-effect/uncertain result
semantics remain authoritative. No rollback, retry, or replay is recommended.

## 13. Existing deterministic-test inventory

The current test suite contains useful adjacent evidence, but not a complete
nested-repository matrix:

| Evidence class | Existing tests | What it proves | What it does not prove |
| --- | --- | --- | --- |
| Actual nested `.git` fixture | `repository_rename_preparation.rs:292-301` | Rename rejects a destination below an ordinary child `.git` boundary. | Other capabilities, source-below/destination-above, B-to-B, or mixed targets. |
| Nested boundary created after Prepare | `repository_rename_preparation.rs:556-564` | Rename revalidation becomes stale before effect. | Other Prepare/Confirm routes. |
| Ordinary same-repo nested directory | `repository_create_file.rs:178-197`, `repository_create_directory.rs:476-495`, multi-file nested target tests | Ordinary nested directories are valid same-repository paths. | These are not nested-repository tests and must not be counted as such. |
| Submodule/gitlink | `repository_create_file.rs:457-491`; multi-file preflight `3419-3514`; commit `796-819`; status/diff parser tests | Gitlink forms are separately recognized/rejected/represented. | Ordinary independent nested repositories. |
| Symlink/reparse/junction | `repository_file_info.rs:278-300`; `repository_diff.rs:292-310`; `repository_diff_staged.rs:359-`; `git_stage.rs` and `git_unstage.rs` link tests; native create `716-753`; multi-file `3518-3549` | Generic link/reparse safety and no-follow behavior in selected paths. | `.git` boundary semantics, nested mounts, and all capabilities. |
| Generic outside-root/traversal | `fs_read.rs:180-216`; workspace tests `205-280`; repository mutation tests | Canonical containment/traversal rejection. | Nested Git authority boundary inside the canonical root. |
| Rename Windows aliases | `repository_rename_preparation.rs:524-554` and source implementation `887-910` | Rename-specific case/alias behavior. | Other repository policies. |

No existing deterministic test directly covers `fs.read`, file-info, patch,
multi-edit mixed A/B, create-file, delete-file, Stage, Unstage, status, diff,
diff-staged, commit, or create-directory against a fresh Repo A plus nested
Repo B ordinary `.git` fixture. No existing test establishes zero native
mutation attempts for each affected pre-effect rejection. No complete Windows
case/reparse nested matrix or Unix mount nested matrix exists.

## 14. Future minimum test matrix

Task 312 must add deterministic, fresh disposable fixtures without replaying
an uncertain effect. At minimum:

1. Repo A root with Repo B nested beneath A, where B is not a workspace member.
2. `fs.read` of a B file through A — reject before open.
3. `repo.file-info` of a B file through A — reject before direct observation.
4. `repo.patch` of a B file through A — `precondition_failed` and zero native
   replacement attempts.
5. `repo.edit-files` with one A target and one B target — reject the complete
   set before any native replacement; also test B-only.
6. `repo.create-file` below an existing B parent — reject before native create.
7. `repo.delete-file` of a B file through A — reject before native delete.
8. rename A→B, B→A, and B→B through A — reject before native rename.
9. `repo.create-directory` below B — reject before native directory create.
10. Stage of a B path through A and Unstage of a B path through A — reject
    before Git index mutation.
11. Commit after a staged A path becomes descendant of B — reject or prove the
    boundary according to the corrected staged-state contract.
12. observer/status/diff/diff-staged behavior for untracked B and A-tracked
    paths that become B descendants; assert no B internals are exposed.
13. B created after Prepare and before the effect for every applicable
    capability; assert stale/precondition rejection and zero attempts.
14. `.git` created/replaced after validation; assert fail-closed behavior.
15. Windows case-equivalent `.git` and nested-root spellings, reparse/junction
    ancestry, supported aliases, trailing-dot/space and ADS ambiguity according
    to existing contracts, and filesystem identity replacement.
16. Unix symlink ancestry, device/inode replacement, nested mount, and mount
    observation ambiguity cases according to existing contracts.
17. Submodule/gitlink versus ordinary nested repository distinction, including
    A gitlink rejection without treating it as proof for ordinary B.
18. For every pre-effect rejection, assert zero native mutation attempts and no
    automatic retry/replay. For every possible-effect ambiguity, preserve the
    existing uncertain classification and fixture for diagnosis.

The future tests must distinguish an ordinary nested directory from a real
nested repository and must inspect actual target effects/state, not merely
error strings.

## 15. Public contract / ADR impact analysis

The recommended correction is stricter implementation of an already accepted
Repository A boundary:

| Surface | Required change? | Finding |
| --- | --- | --- |
| Public Tool schemas | No | Existing path fields and empty observer/index inputs remain sufficient. |
| Tool status/result taxonomy | No planned expansion | Existing `precondition_failed`, invalid-target, stale, known-no-effect, and uncertain classes are sufficient. Boundary rejection should use the existing pre-effect classes. |
| `PermissionLevel` | No | Execute remains separate from narrower repository authority. |
| New authority | No | The validator is a narrower enforcement primitive inside existing host-owned repository policies. |
| HostExplicit eligibility | No | The exact 11-name set remains unchanged. |
| Frontend/Tauri | No | UI review cannot be the security boundary; backend correction is sufficient. |
| `WorkspacePolicy` contract | No | Keep generic filesystem containment Git-agnostic. Repository-aware composition belongs above it or beside repository policies. |
| Accepted ADRs | No amendment identified | ADR 0027 explicitly requires this later conformance correction and preserves existing ADRs. |
| Version/dependency/release | No | Task 311 is audit only; Task 312 should also avoid expansion unless independently authorized. |

If implementation discovers that an existing accepted Tool contract requires
authority expansion rather than fail-closed hardening, it must stop and create
an ADR conflict-resolution task. This audit found no such conflict.

## 16. ADR consistency and verdict

ADR 0027 remains internally coherent. It expressly records that current v0.25
nested `.git` enforcement is not uniform and makes uniform enforcement a
prerequisite. The observed gaps do not require changing ADR 0027, ADR 0016,
ADR 0018, or any other accepted authority ADR. They require implementation
hardening before workspace membership/admission.

The material production gaps are:

- `fs.read` can read a nested B file through A;
- `repo.file-info` can inspect a nested B file through A;
- `repo.status`, `repo.diff`, and `repo.diff-staged` have no nested output
  boundary and can expose A-tracked paths whose ancestry becomes B;
- `repo.patch` can admit a nested B target;
- `repo.edit-files` can admit a B target and mixed A/B change set;
- `repo.create-file` can create below an existing B parent;
- `repo.delete-file` can delete a B file through A;
- Stage and Unstage can derive/admit A-contained targets below B and lack a
  nested boundary check; and
- `repo.commit` is indirectly affected because its staged snapshot does not
  reject ordinary nested-B paths, even though it has no arbitrary path input.

`repo.rename-file` is the only current strong reference and is conformant for
the supported ordinary `.git` marker contract. `repo.create-directory` has a
partial ordinary-marker check but lacks complete shared unsupported-form
coverage. `repo.create-branch` and `host.git.status` are path-scope C cases.

## Final verdict

The audit chooses the correction-required outcome recorded at the end of this
document.

## 17. Exact next task recommendation

**Task 312 — Repository Nested-Boundary Conformance Correction**

Task 312 must correct only the audited boundary gaps, add the deterministic
evidence in Section 14, and preserve existing public contracts, authority
separation, result semantics, HostExplicit eligibility, and no-replay rules.
The preferred correction is a shared narrowly scoped repository-boundary
primitive used by repository policies and repository-bound read/observation
composition, while `WorkspacePolicy` remains Git-agnostic. The correction must
not add workspace membership, workspace-root authority, generic filesystem
authority, new Tool inputs, new HostExplicit eligibility, or linked-worktree
support.

Workspace membership and explicit admission must not begin until Task 312 is
independently closed and, if warranted by its scope, re-audited.

## Validation and delivery record

The mandated audit-only validation was run sequentially after writing this
document:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS |

No destructive live test, Rust implementation, test change, frontend/Tauri
change, Tool schema change, HostExplicit change, version bump, dependency
change, tag, or Release was performed.

## Closure report

| Item | Result |
| --- | --- |
| Final verdict | B — correction required |
| Audited capability count | 16 scoped first-party capability surfaces |
| Conformant list | `repo.rename-file` for supported ordinary `.git` markers; `repo.create-branch` and `host.git.status` by non-path scope |
| Affected list | `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.patch`, `repo.edit-files`, `repo.create-file`, `repo.delete-file`, `repo.create-directory`, `repo.commit` indirectly, `host.git.stage`, `host.git.unstage` |
| Material production gaps | Nested B reachability in read/file-info, observation output, patch/edit/create/delete, and Stage/Unstage; Commit depends on the staged-state gap |
| Test-only gaps | Complete per-capability nested fixtures, mixed A/B edit, all rename directions, create-after-Prepare, Windows aliases/reparse, Unix mount, observer output, and zero-attempt assertions |
| Recommended shared validation location | Narrow repository-specific boundary primitive in `rah-tools` repository policies/composition; not generic `WorkspacePolicy` |
| WorkspacePolicy | Remains Git-agnostic |
| Windows findings | Existing case/reparse/identity defenses are uneven; missing nested-Git checks remain material |
| Unix findings | Symlink and selected mount protections are uneven; missing nested-Git checks remain material |
| Submodule/gitlink findings | Explicitly distinct and partially rejected/represented; not a substitute for ordinary nested-B rejection |
| Public schema/API impact | None expected; fail-closed hardening of existing contracts |
| ADR conflict | No |
| Exact next task | Task 312 — Repository Nested-Boundary Conformance Correction |
| HostExplicit eligible count | 11, unchanged |
| Implementation performed | No |
| v0.25 immutable release identity | Unchanged: source `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`, tag object `ea3c31aaf5190b632d7ef86387f7aff6004ae664`, Release `387406579` |
| Expected changed-file scope | This one audit document |

The final commit, push, exact-head CI, clean worktree, and `HEAD ==
origin/master` verification are recorded below after delivery.

Verdict B — CORRECTION REQUIRED
