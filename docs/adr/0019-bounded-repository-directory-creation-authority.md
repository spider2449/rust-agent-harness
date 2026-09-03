# ADR 0019 — Bounded Repository Directory Creation Authority

Status: Accepted

## Context

RAH has separate host-owned authorities for repository observation, worktree
content mutation, file creation, file deletion, file rename/move, index
mutation, and reviewed commit/history mutation. A directory entry is a
persistent structural worktree effect. It is not an incidental consequence of
file creation, file movement, Execute, or any other existing authority.

Task 181 research established that the smallest useful and defensible new
capability is one directory leaf under one already-existing validated parent.
This ADR accepts that research as the implementation-ready architecture and
security contract. It authorizes no Rust, public API, Tool registration,
Trusted Profile or Desktop change, Generic Codex Tool Bridge change, Git
operation, or live validation.

## Decision

RAH introduces a separate, private, opaque, host-owned
`RepositoryDirectoryCreationPolicy` for the future `repo.create-directory`
capability.

One authorized operation may create exactly one new ordinary directory entry
at one explicitly named logical repository-relative path in one host-selected
canonical repository. The immediate parent must already be an ordinary
directory inside that repository. The authority creates no other entry and
does not create missing ancestors.

The authority planes remain distinct:

```text
directory creation != repository read
                   != worktree content mutation
                   != file creation
                   != file deletion
                   != file rename/move
                   != directory deletion or rename/move
                   != index mutation
                   != reviewed commit/history mutation
                   != Execute
```

In particular:

```text
RepositoryFileCreationPolicy
  != RepositoryDirectoryCreationPolicy
RepositoryFileRenamePolicy
  != RepositoryDirectoryCreationPolicy
RepositoryFileDeletionPolicy
  != RepositoryDirectoryCreationPolicy
```

No existing authority implies directory-creation authority. Multiple existing
capabilities must not be treated as an equivalent composition. `Execute` is
only an outer dispatch permission and does not grant this authority.

## Authority boundary

The trusted host selects the repository, repository identity, runtime and
repository generation, policy authority, and mutation serialization state. It
constructs the opaque short-lived authorization and binds it to the selected
context. Model output is a request, never authorization.

Provider metadata, a model request, a Tool definition, ToolRegistry
registration, Codex approval, frontend state, or configuration cannot
manufacture, elevate, or widen the authority. Trusted Profile may later
compose an explicitly host-supplied directory authority, but it cannot create
one from nothing or broaden its path, platform, or effect rules.

This authority grants no repository selection, absolute-path access, generic
filesystem mutation, generic `fs.mkdir`, shell/process/network access, Git
authority, credential authority, rollback, durability, or replay guarantee.

## Request and effect contract

The future public Tool name is:

```text
repo.create-directory
```

The closed v1 request contains only:

```json
{"path":"existing-parent/new-directory"}
```

`path` is one normalized logical repository-relative path. The request must
not contain a repository root, repository identity, cwd, absolute destination,
executable, argv, environment, recursive flag, overwrite flag, mode, ACL,
owner, permissions, retry control, or expected hash/length. The model cannot
select any native path or process parameter.

The effect shape is exactly:

```text
validate
 -> acquire the established repository mutation lease
 -> immediately independently revalidate all bound preconditions
 -> make exactly one bounded native final-directory creation attempt
 -> observe the filesystem and protected repository state
 -> return one sanitized classified result
```

The operation is not `mkdir -p`, an ensure-directory operation, or a directory
tree operation. It creates exactly one absent final leaf. Missing intermediate
parents fail before the native attempt. An existing target is never accepted
as idempotent success and is never overwritten or replaced.

The intended platform effect is one host-selected `CreateDirectoryW` call on
Windows and one bounded `mkdirat`-style call relative to a validated parent
directory descriptor on Unix/Linux, or an equivalent platform primitive that
proves the same contract. The implementation must not use a shell, a mkdir
subprocess, a recursive helper, a template/placeholder primitive, Git, or a
generic model-facing filesystem operation. Unsupported platform semantics
fail closed.

## Destination and parent requirements

The final destination must be absent as every filesystem object before the
attempt, including an ordinary directory, regular file, symlink, junction,
reparse point, socket, device, or other special entry. A target appearing in a
race window is not automatically RAH's successful effect and must not be
overwritten.

The immediate parent must already exist and be an ordinary directory. The
repository root is a valid parent after the same ordinary-directory,
repository-identity, metadata, and confinement checks. No missing parent is
created implicitly.

Every component from the selected repository root through the immediate parent
must be validated as the intended ordinary directory, without symlink,
junction, mount-like redirection, or Windows reparse traversal. The final
component must be checked with no-follow semantics where supported. The parent
identity and final-entry absence proof are bound and freshly revalidated.

The path is one non-empty, slash-separated, normalized logical path beneath
the selected repository. Reject absolute and drive-relative paths, traversal,
empty or `.`/`..` components, backslashes, colon or ADS syntax, UNC,
verbatim/device namespaces, wildcard/glob forms, trailing-dot/space tricks,
`.git` and other metadata paths, nested repository boundaries, and Windows
reserved device names. Case-insensitive or Unicode-normalization-equivalent
collisions and other path aliases fail closed when the supported identity
model cannot distinguish them deterministically. RAH does not claim that
ordinary validation eliminates every external TOCTOU race.

A successful effect creates only an ordinary directory. It does not create a
symlink, junction, reparse point, file, placeholder, `.gitkeep`, `.keep`,
other marker, or child content. V1 exposes no model-selected Unix mode or ACL
input. The host applies the platform-native fixed policy: on Unix/Linux a
conventional `0o777` subject to process umask and applicable default ACLs,
with no follow-up chmod/chown/ACL mutation; on Windows the host/native
inherited security policy.

## Repository identity, generation, and serialization

The host binds the operation to the selected canonical repository root and
metadata identity, repository/runtime generation, exact normalized destination
identity, root and immediate-parent filesystem identities, each validated
parent component, final-entry absence, repository confinement, metadata and
nested-repository exclusion, and the Git observations needed to prove that no
Git state was mutated. A directory content preimage is not required.

The operation reuses the established per-canonical-repository mutation lease
from pre-effect validation through result construction. The lease serializes
RAH-owned repository mutations only; it does not exclude external processes,
sync tools, antivirus, Git, or privileged actors.

After acquiring the lease and immediately before the native attempt, the host
independently revalidates the selected repository/root identity, generation,
path normalization and containment, metadata and nested-repository boundary,
all parent identities and ordinary no-link/no-reparse properties, destination
absence including equivalent aliases, and the relevant index, HEAD, branch,
and refs observations. Any changed identity, generation, parent, absence
proof, or contradictory Git observation fails before effect.

Unlike the tracked-file deletion and rename authorities, directory creation
has no content preimage and does not inherently require a clean worktree,
clean index, attached HEAD, or branch identity. It may operate in a valid
unborn non-bare repository when all selected-root, parent, confinement, and
Git-observation requirements are satisfied. Bare repositories, malformed
metadata, unsupported worktree layouts, and contradictory repository state
remain precondition failures.

## Git semantics

Git does not track an empty directory as an independent committed tree entry.
A verified directory creation therefore means:

- the ordinary filesystem directory exists;
- no file or marker was created;
- the index is unchanged;
- `HEAD` is unchanged when present;
- the attached branch and refs/history are unchanged; and
- `git status --short` may produce no output and may remain clean.

This clean Git result is expected and does not prove that no filesystem effect
occurred. Filesystem postconditions are authoritative for proving directory
creation. Git metadata preservation is a safety postcondition, not a
requirement that Git display the empty directory.

The operation must not invoke Git, modify the index, stage anything, create a
commit, mutate refs, or manufacture Git visibility with `.gitkeep`, `.keep`,
or another marker. A separately authorized `repo.create-file` operation may
later create the first Git-visible content.

## Results and failure classification

The implementation must use the existing sanitized outcome vocabulary and
distinguish these classes:

- `invalid_input`: the closed request is structurally invalid; no authority
  evaluation or native attempt occurs.
- `precondition_failed`: authority, repository/generation, path, parent,
  target absence, identity, or other validation fails before a native attempt.
- `directory_created_verified`: the native attempt occurred and filesystem
  and required unchanged-state postconditions prove the authorized creation.
- `known_no_effect`: an attempt may have been reached, but fresh observation
  proves the target remains absent and protected state remains intact.
- `uncertain`: the native effect or required observation cannot prove either
  verified success or known no-effect.

The conceptual verified result is:

```json
{
  "path":"existing-parent/new-directory",
  "status":"directory_created_verified",
  "uncertain":false,
  "git_metadata_changed":false
}
```

Results are bounded and sanitized. They must not expose native paths, raw OS
errors, credentials, authorization objects, or policy internals.

Verified success requires the requested destination to exist as an ordinary
directory, remain inside the selected repository, and retain validated parent
and ancestry properties without an unexpected target type. Observation must
also prove the target was absent before the attempt, index bytes or its bound
fingerprint are unchanged, `HEAD` and branch state when present are unchanged,
refs are unchanged, and RAH made no file, marker, staging, commit, or second
effect. Verification makes no crash-durability, global-exclusivity, rollback,
or privileged-external-actor claim.

An existing target, missing or unsafe parent, invalid path, or failed
immediate revalidation is `precondition_failed` and has no native attempt. A
native error may be `known_no_effect` only when independent fresh observation
proves the target is still absent and protected state is intact. An
`ERROR_ALREADY_EXISTS` or `EEXIST` target is RAH failure, never idempotent
success. Contradictory state, lost observation, timeout, cancellation,
disconnect, crash, or inability to establish ownership of a resulting
directory is `uncertain`.

## Replay, rollback, and compensation

There is exactly one native directory-creation attempt. No automatic retry or
replay is permitted after any native attempt that may have taken effect,
including timeout, cancellation, disconnect, crash, ambiguous OS failure, or
provider/runtime response loss.

RAH must not delete the destination as cleanup or rollback after an uncertain
attempt. Another actor may own or have modified that path, and directory
creation does not imply directory-deletion authority. No compensation
guarantee exists. A successfully created empty directory may remain after a
later separately authorized file-creation failure; that is a persistent
partial workflow effect, not rollback failure. Recovery requires fresh
observation and a new authorization.

## Composition with existing repository authorities

`repo.create-file` remains one explicit file effect and must continue requiring
an existing parent; it must not gain implicit parent creation. Directory
authority alone cannot create a file, and file authority alone cannot create a
missing directory. With both authorities, the workflow is two explicit Tool
calls and two distinct host authorizations:

```text
repo.create-directory -> repo.create-file
```

Likewise, a later workflow may create a directory and then move a tracked file
into its existing parent with `repo.rename-file`. Those are two explicit
effects and two separate authorities. Rename must not silently create a
directory.

Directory deletion remains separate and deferred. This ADR does not add or
imply `repo.delete-directory` for symmetry. No automatic cleanup is authorized
after a successful or uncertain creation.

## Desktop, runtime, and Trusted Profile implications

Future Desktop integration must bind the authority to the host-selected
repository context and repository/runtime generation. Repository switching
revokes the old context and prevents a stale runtime from acting against the
newly selected repository. Registration occurs only after the host constructs
the authority, and Generic Codex Tool Bridge routing remains ordinary
`Tool -> ToolRegistry` dispatch with the canonical public name.

After verified creation, Desktop must refresh repository/workflow presentation
even if Git status remains clean. It must not imply that Git tracks the empty
directory. The structural filesystem mutation must also revoke outstanding
reviewed commit authorization and refresh the review generation under the
existing fail-closed workflow rule. These are future integration requirements,
not changes authorized by this ADR task.

Trusted Profile may later contain only a closed symbolic declaration that
composes an already host-supplied authority. It may not accept a repository
path, generation, mode, ACL, recursive flag, or policy settings that construct
or broaden authority. Profile configuration and provider metadata cannot
elevate file creation into directory creation.

## Observability

Future normal Generic Tool Bridge integration should provide bounded,
redacted lifecycle evidence using the canonical public name:

```text
tool_advertised
tool_requested
tool_started
tool_finished
```

Structured results may be preserved subject to existing redaction and bounds.
Evidence is observability only. It cannot grant or broaden authority, and a
failure to record evidence does not authorize a filesystem effect.

## Platform considerations

Deterministic validation may establish Windows and Ubuntu/Linux behavior where
CI and tests provide evidence. Windows is the live-certified platform for the
project's later live gate. Linux live certification is not established. This
ADR makes no formal supported-platform promise, no network-filesystem
guarantee, no OS-sandbox claim, and no claim that all external races can be
eliminated.

## Consequences

RAH gains a narrow structural authoring plane that fills the missing-directory
step while preserving host-owned repository selection, generation binding,
shared mutation serialization, Git/index/commit boundaries, and conservative
uncertain-effect handling. Empty-directory creation can be useful as the
first explicit step before file creation, but it can also remain invisible to
Git and can remain after a later workflow failure.

Implementation and deterministic-test work must separately prove closed input,
parent and target identity, traversal and metadata rejection, symlink/
reparse and alias behavior, one-attempt semantics, unchanged Git metadata,
verified filesystem postconditions, conservative uncertainty, and no replay
or cleanup. Those proofs are not performed by this documentation task.

## Non-goals

This ADR explicitly rejects or defers:

- recursive directory creation, `mkdir -p`, multiple directories per request,
  and ensure-directory/idempotent semantics;
- directory deletion, rename/move, copy, replacement, overwrite, and
  cross-volume behavior;
- automatic file creation, implicit parent creation, placeholders,
  `.gitkeep`, `.keep`, or other marker files;
- symlink, junction, reparse-point, mount, device, special-file, ACL, chmod,
  owner, or model-selected Unix mode creation;
- generic filesystem mutation, generic `fs.mkdir`, shell/process fallback,
  provider-selected executable/cwd/environment, and network authority;
- Git mutation, automatic staging, commit, branch/ref/history mutation,
  checkout, reset, clean, stash, merge, rebase, fetch, pull, push, or network
  Git;
- rollback, compensation, recovery journaling, durability guarantees,
  automatic retry, and replay after a possible effect;
- Trusted Profile schema or implementation changes, Desktop implementation,
  public protocol expansion, Generic Codex Bridge implementation, and live
  validation in this ADR task; and
- stronger OS sandbox, network-isolation, all-race-elimination, or Linux
  live-certification claims.

## Validation expectations

Task 182 is documentation-only. Future implementation tasks must use
deterministic tests for successful one-leaf creation under the repository root
and an existing nested parent; missing parents and all existing target types;
path, metadata, nested-repository, traversal, alias, reserved-name, symlink,
junction, and reparse rejection; repository/generation and parent identity
changes; one native attempt and conservative uncertain results; unchanged file
bytes, index, `HEAD`, branch, and refs; and no Git, shell, process, replay,
rollback, or cleanup effect.

Validation must distinguish deterministic Windows and Ubuntu/Linux evidence
from any later Windows live certification. Generic Tool Bridge, Trusted
Profile, and Desktop lifecycle evidence remain separately authorized future
work and must retain host-owned authority and generation semantics.

## Relationship to existing ADRs

ADR 0019 does not supersede or broaden:

- ADR 0010, repository index mutation;
- ADR 0011, Trusted Profile authority composition;
- ADR 0012, worktree content mutation;
- ADR 0013, file creation;
- ADR 0014, bounded multi-file editing;
- ADR 0016, reviewed commit authority;
- ADR 0017, file deletion; or
- ADR 0018, file rename/move.

It adds one orthogonal structural worktree authority. All existing
host-ownership, sanitization, generation, mutation-lease, no-replay, and
non-sandbox boundaries remain in force.
