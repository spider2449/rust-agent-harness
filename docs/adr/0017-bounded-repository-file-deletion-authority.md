# ADR 0017 — Bounded Repository File Deletion Authority

Status: Accepted

## Context

RAH has separate host-owned boundaries for repository worktree content
mutation, file creation, bounded multi-file editing, index mutation, and
commit/history mutation. Deleting a directory entry is a persistent and
destructive worktree effect even when it does not change file content or the
Git index. It therefore cannot be inferred from an existing editing, creation,
index, commit, or Execute authority.

Task 158 concluded that the first useful deletion capability must be limited to
one clean HEAD-tracked regular file and must bind the requested source to the
exact HEAD blob. This ADR accepts that conclusion as the architecture contract.
It authorizes no implementation, public API, Tool registration, Trusted
Profile schema, Desktop integration, bridge change, or live validation.

## Decision

RAH introduces a separate, additive, private host-owned
`RepositoryFileDeletionPolicy` for bounded repository file deletion. The
conceptual authority planes remain distinct:

```text
worktree content mutation != RepositoryFileCreationPolicy
RepositoryFileDeletionPolicy != RepositoryMutationPolicy
RepositoryMutationPolicy != RepositoryCommitPolicy
PermissionLevel::Execute != any of these authorities
```

`PermissionLevel::Execute` is an outer dispatch permission only. It MUST NOT
grant deletion authority. A model request, provider metadata, Tool definition,
ToolRegistry registration, Codex approval, or Desktop state is not authority.
The trusted host selects the repository, constructs the policy, composes its
limits, and grants the underlying authority.

### Exact capability boundary

One accepted operation may delete exactly one explicitly named
repository-relative file in one selected repository. The initial authority
requires the target to be:

- beneath the selected canonical repository root;
- an existing regular file;
- tracked by the current `HEAD` through one normal stage-0 index entry;
- clean, with the index and worktree equal to the authorized HEAD state;
- not conflicted, a submodule/gitlink, a nested repository boundary, or
  repository metadata; and
- in an otherwise supported ordinary non-bare repository state.

The operation does not imply directory or recursive deletion, wildcard or glob
selection, arbitrary filesystem deletion, untracked-file cleanup, rename/move,
directory creation, staging, commit, ref/history mutation, or network Git. It
does not invoke Git to perform the deletion.

Unrelated dirty paths may remain untouched when all target and repository
observations required by this contract remain exact. RAH must not claim that
the lease excludes editors, antivirus, Git, or other external local actors.

### Host binding and exact preimage

The host binds each operation to an opaque, short-lived authorization under the
existing per-repository mutation lease. The binding includes, as applicable to
the supported repository model:

1. selected canonical repository identity and policy generation;
2. runtime and runtime-generation identity;
3. attached branch and exact `HEAD` OID;
4. one canonical logical repository-relative path;
5. the exact normal stage-0 index entry and evidence that it equals the HEAD
   tree entry; and
6. the target's exact HEAD-blob identity, including raw byte length and
   SHA-256, together with the relevant index and worktree observations.

The narrow v1 source rule is that the current worktree bytes must equal the
authorized HEAD blob. A model-supplied digest or length is only a request
precondition; it is not authorization and cannot authorize a newer worktree
version. The host must independently compare the current HEAD blob, index
entry, and worktree source. A target changed after observation, a changed HEAD,
branch, repository identity, runtime generation, index state, or path identity
fails closed before deletion.

This binding explicitly prevents:

```text
model observes file A
 -> user modifies file A
 -> stale model deletion removes the newer user version
```

The stale request fails because the final host-owned preimage and HEAD checks no
longer match. The contract makes no claim of complete cross-process TOCTOU
elimination; contradictions or unsupported identity observations fail closed.

### Eligibility and path security

The logical path is one nonempty repository-relative path using `/` separators
and normal components. The host rejects absolute or drive-relative paths,
`..`, `.`, empty components, backslashes, drive-qualified paths, UNC paths,
verbatim (`\\?\\`) and device namespaces, ADS or colon components, wildcard or
glob characters, case-equivalent ambiguity, and unsupported normalization.
`.git` and all repository metadata, including case variants and supported
metadata indirections, are ineligible.

Root, every parent, and the target must be validated as the intended objects
beneath the selected repository. Symbolic links, junctions, Windows reparse
points, mount-like redirection, hard-link ambiguity, directory targets, nested
repositories, special files, and aliasing or replacement that cannot be
deterministically rejected fail closed. Windows validation must use the tested
Unicode handle-based identity and delete primitive appropriate to the supported
platform subset. RAH must not claim stronger filesystem isolation than these
checks provide.

### Effect boundary and outcomes

Execution has this fixed shape:

```text
validate
 -> immediate independent pre-effect revalidation
 -> exactly one native filesystem deletion attempt
 -> filesystem effect commit point
 -> deterministic post-effect observation
```

The native deletion disposition on the fully revalidated target is the sole
filesystem effect commit point. No second deletion attempt is permitted. A
successful native return is not by itself proof of absence, including when
Windows deletion is pending because another handle delays name disappearance.

The host result taxonomy is:

- `invalid_input`: the closed request schema is invalid;
- `precondition_failed`: no native deletion attempt occurred because identity,
  preimage, Git state, target state, or supported-state validation failed;
- `deleted_verified`: the one authorized path is absent and deterministic
  observation proves the index, HEAD, branch, and repository invariants remain
  as authorized;
- `known_no_effect`: an attempt may have been reached, but fresh observation
  proves the protected preimage remains intact; and
- `uncertain`: the effect or required observation cannot prove either result.

Before the commit point, a failure is definitely no effect only when the
complete protected preimage is freshly proven intact. At or after the commit
point, timeout, cancellation, disconnect, crash, lost native result, sharing or
filter failure, observer failure, or contradictory state is uncertain unless
post-observation proves `known_no_effect` or `deleted_verified`. Native error
codes and process/supervisor status alone are insufficient.

### Dirty and unsupported states

Any modified target fails closed. The operation must not silently reset,
overwrite, restore, or erase unreviewed user edits. Staged changes, a staged
deletion, intent-to-add, unmerged/conflicted entries, untracked or ignored
targets, missing targets, directories, submodules, sparse or skip-worktree
ambiguity, detached or unborn HEAD, linked worktrees, bare repositories,
alternate or malformed indexes, and merge/rebase/cherry-pick/revert/sequencer/
bisect state are refused in v1. Repository identity or generation changes and
all path escape, symlink, reparse, alias, or target replacement observations
are also fail-closed precondition failures when detected before the effect.

### Index and commit separation

Deleting the worktree directory entry does not invoke `git add`, `git rm`,
`restore`, `reset`, `checkout`, or any other index operation. The index remains
unchanged, so a verified deletion appears as an unstaged deletion. Human Stage
and Unstage remain authoritative for index mutation; this policy does not imply
`RepositoryMutationPolicy`.

ADR 0016 remains unchanged. Deletion authority neither grants, modifies, nor
prepares `RepositoryCommitPolicy`; the reviewed one-shot commit workflow stays
a separate history/ref authority boundary.

### Trusted Profile and model/tool boundary

ADR 0011 remains authoritative. A Trusted Profile may compose a deletion
capability only after the trusted host has already granted and constructed the
underlying deletion policy. Profile configuration cannot create, elevate,
relax, or revive deletion authority or select a raw repository path.

The eventual model-visible capability must accept one closed explicit logical
repository-relative target, with only the bounded preimage preconditions needed
by this contract. It must not expose arbitrary filesystem paths, native paths,
generic delete or recursive flags, Git argv, shell commands, model-selected
CWD/environment, recovery controls, or another target.

### No replay and external effects

Once execution may have crossed the filesystem effect boundary, an uncertain
outcome MUST NOT be retried or replayed automatically. Timeout, cancellation,
disconnect, and crash do not imply rollback. RAH does not restore the file,
reset Git, remove locks, compensate the effect, or claim rollback. A later
attempt requires fresh repository observation and new host authorization.

Process supervision is not OS sandboxing. This ADR makes no network-isolation
claim and does not claim protection from privileged external actors or all
filesystem races. Uncertain external effects are retained as uncertain rather
than guessed away.

## Consequences and implementation direction

This decision adds the smallest useful destructive structural-authoring
authority while preserving host-owned repository selection, the shared mutation
lease, the human index boundary, and the reviewed commit boundary. It creates a
new deterministic implementation burden for repository identity, HEAD/blob and
index observation, Windows handle/reparse behavior, exact postconditions, and
uncertain outcomes.

The next task should implement this ADR deterministically as a private
`RepositoryFileDeletionPolicy` and narrow repository deletion capability,
including focused Windows and Unix tests and bounded redacted results. That
implementation task must not broaden this authority or implement Desktop,
Trusted Profile, generic filesystem/process, staging, commit, or live-model
integration without separate authorization.

## Non-goals

This ADR defers and does not authorize:

- rename or move;
- directory or recursive deletion;
- arbitrary untracked deletion or cleanup commands;
- generic `fs.write` or `fs.unlink`;
- generic shell or process execution;
- model-selected Git, branch, ref, or history mutation;
- network Git;
- automatic staging or automatic commit;
- rollback, restore, backup, recovery, retry, or replay;
- stronger OS sandboxing or network isolation claims; and
- Desktop integration, Trusted Profile implementation, public API expansion,
  bridge changes, or live validation.

## Relationship to accepted ADRs

ADR 0010 remains the separate host-owned index mutation policy. ADR 0011
remains composition-only. ADRs 0012, 0013, and 0014 retain their separate
content replacement, creation, and multi-file edit contracts. ADR 0015 remains
the unrelated bounded provider endpoint authority. ADR 0016 remains the
separate reviewed commit/history authority. No accepted ADR is superseded or
broadened by this decision.
