# ADR 0018 — Bounded Repository File Rename/Move Authority

Status: Accepted

## Context

RAH has separate host-owned authorities for worktree content mutation, file
creation, bounded multi-file editing, file deletion, index mutation, and
reviewed commit/history mutation. Moving a file changes directory entries and
has two explicit path identities, collision rules, and uncertain-effect
semantics. It is therefore a distinct structural worktree authority. It is
not a safe composition of creation and deletion.

Task 169 research and the v0.14 roadmap identify one tracked-file
rename/move as the smallest useful capability. This ADR accepts that contract
only; it authorizes no Rust, public API, Tool registration, Trusted Profile or
Desktop change, Generic Codex Bridge change, Git operation, or live validation.

## Decision

RAH introduces a separate, private, host-owned conceptual
`RepositoryFileRenamePolicy` for one bounded repository file rename/move.
Same-directory rename and same-repository cross-directory movement use this
one authority.

The authority planes remain separate:

```text
RepositoryFileRenamePolicy
  != RepositoryFileCreationPolicy
  != RepositoryFileDeletionPolicy
  != RepositoryWorktreeMutationPolicy
  != RepositoryMultiFileMutationPolicy
  != RepositoryMutationPolicy
  != RepositoryCommitPolicy
```

`PermissionLevel::Execute` is an outer dispatch permission only. It does not
grant rename authority. A model request, provider metadata, Tool definition,
ToolRegistry registration, frontend state, Codex approval, or Trusted Profile
configuration cannot create, elevate, or broaden this authority. The trusted
host selects the repository and constructs the opaque authority and its
short-lived operation binding.

### Exact v1 capability

One authorized operation may move exactly one explicitly named eligible file:

```text
source_path -> destination_path
```

Both paths are logical repository-relative paths in one selected canonical
repository. V1 permits a same-directory rename and a cross-directory move
within that repository. It permits neither multiple sources or destinations,
directory or recursive movement, wildcard/glob selection, arbitrary
filesystem movement, nor cross-repository movement.

The source and destination are not native absolute paths, and the destination
parent is not created by this authority.

### Source eligibility

The source must, at authorization and immediately before the effect, be:

- beneath the selected canonical repository root;
- one explicit valid logical repository-relative path;
- an existing regular file;
- tracked by current `HEAD` through exactly one normal stage-0 index entry;
- clean against `HEAD`, with exact worktree bytes equal to the authorized HEAD
  blob;
- non-conflicted and not a submodule or gitlink;
- outside repository metadata, including `.git` and supported metadata
  indirections;
- free of symlink, junction, reparse-point, mount-like, or other path
  redirection ancestry;
- not hard-link ambiguous under the supported identity model; and
- in an otherwise supported ordinary non-bare repository state.

Any dirty or changed source fails closed. Unrelated dirty paths may remain
untouched only when all repository, source, and index invariants needed by this
authority remain provable. The authority does not silently reset, restore, or
move an unreviewed version.

V1 refuses staged changes, missing or special sources, unmerged entries,
intent-to-add, sparse or skip-worktree ambiguity, detached or unborn HEAD,
linked worktrees, alternate or malformed indexes, and active merge, rebase,
cherry-pick, revert, sequencer, bisect, or similarly unsupported state.

### Destination eligibility

The destination must be one explicit logical repository-relative path beneath
the same selected repository. It must be:

- currently nonexistent, including no file, directory, link, special entry,
  tracked content, untracked content, or alias-equivalent object;
- beneath an already existing parent directory;
- reached only through parent directories validated as the intended ordinary
  directories, with no symlink, junction, or reparse ancestry;
- outside repository metadata and nested repository boundaries;
- on a supported same-volume/filesystem boundary with the source; and
- neither the source itself nor an equivalent path alias.

The parent identity and proof of final-entry absence are bound and freshly
revalidated. A destination that exists as tracked or untracked content, a
directory, or any other object; a missing or replaced parent; overwrite or
replace; path escape; `.git` targeting; nested repository targeting; or
unsupported alias ambiguity is a precondition failure. V1 has no overwrite
flag.

Logical paths use the repository path-security model: normal `/`-separated
components only, with no absolute or drive-relative paths, traversal, empty or
`.`/`..` components, backslashes, UNC, verbatim/device namespaces, ADS or
colon syntax, wildcard/glob characters, or unsupported normalization. Windows
case-insensitive and Unicode case/normalization equivalence, trailing
dot/space aliases, DOS device names, UNC/verbatim/device namespaces, reparse
ancestry, and other path-equivalent aliases are rejected when they cannot be
deterministically distinguished.

### Windows case-only behavior

V1 MUST reject a case-only or equivalent-path Windows rename, such as
`Foo.rs -> foo.rs`. No temporary-name or other multi-step workaround is
authorized. The source and destination must be distinct under the supported
native path identity model; unsupported case or normalization ambiguity fails
closed.

### Host authorization binding

The trusted host binds the operation to an opaque, short-lived authorization
under the existing per-canonical-repository mutation lease. As applicable to
the supported repository model, the binding includes:

1. selected canonical repository identity, root identity, and repository/runtime
   generation;
2. runtime identity and generation, attached branch, and exact `HEAD` OID;
3. exact source logical path and source filesystem identity;
4. source HEAD tree/blob identity, exact normal stage-0 index entry, and proof
   that the entry agrees with the HEAD tree;
5. source SHA-256, byte length, and exact authorized worktree bytes;
6. exact destination logical path and destination-parent filesystem identity;
7. independent proof that the destination final entry was absent;
8. raw index state or an equivalent collision-resistant index fingerprint; and
9. relevant repository/ref state sufficient to prove that no branch, ref,
   history, or index mutation is part of this authority.

A model-supplied digest or length is only a request precondition. It is not
authorization and cannot authorize a newer or different worktree version.

### Immediate revalidation and effect shape

Immediately before the native effect, the host independently revalidates both
path sides and all bound repository state. It fails closed if the source
changed, identity changed, or disappeared; the destination appeared; either
parent changed; HEAD, branch, index, relevant refs, repository identity, or
runtime/repository generation changed; or any path/security observation is
contradictory. It rechecks ordinary types, metadata exclusion, nested
repository boundaries, alias/case/normalization rules, source bytes and
identity, destination absence, parent identity, and supported same-volume
semantics.

Execution has exactly this shape:

```text
validate
 -> immediate independent source/destination revalidation
 -> exactly one native rename/move attempt
 -> filesystem effect commit point
 -> deterministic post-effect observation
```

There is no second rename attempt after an effect may have occurred. These
checks mitigate, but do not eliminate, cross-process TOCTOU races.

### Native effect primitive

The effect is one host-selected native filesystem rename/move operation with
no-replace semantics, on a supported same-volume/filesystem boundary. If the
platform cannot provide the required no-overwrite and cross-directory
semantics, or reports unsupported cross-device behavior, the operation fails
closed. It never falls back to copy plus delete.

The operation MUST NOT invoke `git mv`, `git add`, `git rm`, `restore`,
`reset`, `checkout`, `commit`, a shell move command, generic process
execution, or a generic model-facing `fs.rename` capability. `git mv` is
rejected because it mutates the index. Copy plus delete is rejected because it
creates two persistent effects and partial-failure semantics. Native
same-volume rename is the only v1 effect primitive.

### Outcomes

The bounded result taxonomy is:

- `invalid_input`: the closed request is structurally invalid; no authority
  evaluation or native attempt occurs;
- `precondition_failed`: host authorization, repository/source/destination
  state, identity, preimage, or supported-platform validation fails before a
  native attempt;
- `renamed_verified`: post-observation proves the authorized move and all
  required unchanged-state invariants;
- `known_no_effect`: an attempt may have been reached, but fresh observation
  proves the source remains intact at its authorized preimage and no
  authorized move effect occurred; and
- `uncertain`: the effect or required observation cannot prove either
  `known_no_effect` or `renamed_verified`.

Destination-exists is ordinarily `precondition_failed`, never an overwrite or
recovery mode. A native error or success return alone is not enough to choose
an outcome. Before the effect commit point, failure is known no-effect only
when fresh observation proves the protected preimage intact. After that point,
timeout, cancellation, disconnect, crash, lost native result, sharing failure,
observer failure, or contradictory state is `uncertain` unless independent
post-observation proves a permitted outcome.

### Verified postconditions

`renamed_verified` requires deterministic postconditions, not merely an OS
success return. To the extent the supported platform can reliably observe
them, the host proves:

- the source path is absent;
- the destination path is present and contains exactly the authorized bytes;
- the destination corresponds to the protected source object/identity where
  the platform can reliably prove that fact;
- the index and its bound fingerprint are unchanged;
- HEAD, attached branch, and relevant refs are unchanged;
- no staging, commit, second filesystem effect, or history mutation occurred.

The ADR makes no stronger file-identity or durability claim than the supported
platform can prove.

### Git and index semantics

The native operation leaves the Git index unchanged and does not invoke any
Git mutation. Before a human stages the result, Git may report the structural
worktree effect as, for example:

```text
D old/path
?? new/path
```

This is not Git index rename semantics. Human Stage/Unstage remains the
separate ADR 0010 index authority. Rename authority does not grant staging,
commit, ref, or history mutation.

Moving one directory entry naturally removes one name and creates another as
one filesystem rename effect. That intrinsic effect does not mean creation
authority or deletion authority grants rename, and creation authority plus
deletion authority never composes into rename authority.

### Cross-directory and deferred movement

Cross-directory movement is permitted only when both paths remain in the same
selected repository, the destination parent already exists, path security and
parent identity pass, native single-effect semantics apply, and no
cross-volume or copy/delete fallback is required. If volume or mount identity
cannot be safely established, the operation fails closed.

Directory movement is deferred: it can affect an arbitrary recursive
namespace, nested repositories, reparse children, and large topology, and is
not equivalent to moving one regular file. Untracked-source movement is also
deferred; v1 is HEAD-backed and does not provide generic local-file movement.

Rename preserves bytes only. It does not rewrite line endings or encoding,
edit content, update imports, rewrite references, patch dependent files, or
perform repository-aware refactoring.

### Request and tool boundary

The eventual conceptual closed request, without finalizing an implementation
API here, is:

```text
source_path
destination_path
expected_source_file_sha256
expected_source_file_byte_length
```

The research-selected semantic name for a future capability is
`repo.rename-file`; it describes both same-directory rename and bounded
cross-directory movement, not generic filesystem access. The request is not
authorization. It must not contain native absolute paths, cwd, environment,
overwrite/replace or recursive flags, shell or Git argv, retry/fallback
controls, or a model-selected temporary path.

### Trusted Profile, bridge, and Desktop boundaries

ADR 0011 remains authoritative. A Trusted Profile may eventually compose this
capability only when the trusted host has already supplied the underlying
rename authority. The profile cannot create it, select arbitrary roots,
enable overwrite, relax path rules, restore stale authority, or broaden the
same-repository scope.

Future exposure uses the ordinary:

```text
Tool -> ToolRegistry -> Generic Codex Tool Bridge
```

No rename-specific authorization bypass is permitted; private aliases remain
implementation details. Future Desktop integration remains host-owned: the
selected repository context may construct and store opaque authority, while
the frontend owns neither authority nor repository selection. A verified move
should eventually refresh repository presentation, revoke stale commit review
under existing mutation rules, and preserve repository/generation guards.
None of this is implemented by Task 170.

### Replay, cancellation, rollback, and security limits

There is exactly one native rename attempt. After an effect may have occurred,
there is no automatic replay, retry, move-back compensation, or rollback.
Timeout, cancellation, disconnect, and crash do not imply rollback. An
uncertain external effect remains uncertain. A later operation requires fresh
observation and fresh host authorization.

Process supervision is not OS sandboxing. This ADR makes no network-isolation
claim, no rollback guarantee, and no claim that bounded identity and
revalidation defeat every privileged or external filesystem race.

## Non-goals

ADR 0018 explicitly rejects or defers directory or recursive movement,
wildcard/glob movement, arbitrary filesystem rename, untracked-source movement,
overwrite/replace, case-only Windows rename, cross-repository movement,
cross-volume copy/delete fallback, generic copy, content editing,
import/reference rewriting, repository refactoring, automatic staging or
commit, `git mv`, generic Git, shell/process authority, generic `fs.rename`,
ref/history/network Git, rollback, compensation, automatic retry/replay, and
generic undo.

It also does not authorize public protocol expansion, Tool registration,
Trusted Profile implementation, Generic Codex Bridge changes, Desktop
integration, or live validation.

## Consequences and implementation direction

This adds a narrowly useful structural-authoring plane while preserving
host-owned repository selection, the shared mutation lease, exact preimage
binding, the human index boundary, and the reviewed commit boundary. The cost
is deterministic implementation and test burden for two-path identity,
destination collision, Windows reparse and alias behavior, same-volume native
semantics, postconditions, and uncertain outcomes.

The recommended sequence is: this accepted ADR, then a narrow deterministic
core policy and capability implementation, deterministic hardening, trusted
composition and ordinary bridge integration, Desktop integration, any
separately required observability cleanup, Windows live validation, a v0.14
milestone audit, and release preparation. Later task numbers remain subject
to repository conventions and separate authorization.

Evidence terminology remains unchanged: deterministic validation may be
established on Windows and Ubuntu/Linux by tests and CI; Windows is the live-
certified platform; Linux live certification is not established. This ADR
makes no new certification claim.

## Relationship to existing ADRs

ADR 0018 does not supersede or broaden:

- ADR 0010, repository index mutation;
- ADR 0011, Trusted Profile authority composition;
- ADR 0012, worktree content mutation;
- ADR 0013, file creation;
- ADR 0014, bounded multi-file editing;
- ADR 0016, reviewed commit authority; or
- ADR 0017, file deletion.

It adds a new orthogonal structural worktree authoring authority. All existing
separation, host-ownership, sanitization, generation, no-replay, and
non-sandbox claims remain in force.
