# Task 181 — Repository Directory-Creation Authority Research

Research date: 2026-09-03
Task type: research, authority design, and documentation only
Validated baseline: `HEAD == origin/master == 9e951df035a351f224e94e22fd39764065e6108b`
Release state: v0.14.0 remains immutable at `52506521bdf838784dd45bb54df2d6bcff8bcd08`, annotated tag `v0.14.0`, tag object `9193423e96dd0cda2fd8f5ed5619ab2b58483acc`

## 1. Executive decision

The research supports a new, separate, host-owned authority for a future
`repo.create-directory` capability. The smallest useful v1 is:

> Create exactly one absent ordinary directory leaf at one validated logical
> repository-relative path, where its immediate parent is already an ordinary
> directory inside the selected repository.

The authority creates only that directory entry. It does not create files,
parents, placeholders, Git entries, index entries, commits, refs, or process
effects. It is a structural worktree mutation and therefore requires a new
ADR before implementation. The provisional private policy name is
`RepositoryDirectoryCreationPolicy`.

This conclusion validates Task 180's selected capability and its core product
gap. No equivalent safe directory-create authority exists in the current
implementation. Task 182 should write the ADR; Task 181 does not.

## 2. Evidence reviewed

The repository baseline, README, changelog, architecture and security
documents, the v0.15 roadmap, ADRs 0010–0018 (especially 0012–0014 and
0017–0018), current file-create/delete/rename tools, shared mutation lease,
path and reparse checks, Desktop repository generations and review refresh,
Trusted Profile composition, Generic Codex Tool Bridge, and existing live
evidence contracts were inspected.

The current code has reusable patterns but no directory capability:

- `crates/rah-tools/src/repository_create_file.rs` has a closed `{path,
  content}` request, requires an existing safe parent, performs exclusive
  file creation, and explicitly does not create directories.
- `crates/rah-tools/src/repository_rename_file.rs` requires an existing
  destination parent and excludes directory movement; its one native effect
  is a file rename/move, not directory creation.
- `crates/rah-tools/src/repository_delete_file.rs` is a separate destructive
  regular-file authority.
- `git_stage::repository_lease` is the established per-canonical-repository
  RAH mutation serialization boundary and is shared by worktree/index
  operations.
- Existing logical-path parsing rejects absolute, traversal, backslash,
  colon/ADS, empty, `.git`, and Windows reserved-name forms; parent checks
  reject symlink/reparse traversal. These helpers are useful implementation
  material, but must be reviewed for directory-specific final-entry and
  alias races before reuse.
- Desktop already binds selected repositories and runtime generations and
  refreshes workflow state after repository mutations. Its current registry
  registers create-file, delete-file, and rename-file only; no directory
  authority is present.

The roadmap's earlier baseline hash is stale relative to this task's required
baseline; this document uses the required current hash above.

## 3. Current product gap

The post-v0.14 workflow can inspect repository state, edit existing tracked
content, create a file under an existing parent, delete an eligible tracked
file, move an eligible tracked file into an existing destination directory,
stage/unstage through host actions, review staged state, and commit a fresh
reviewed snapshot.

It cannot safely perform the first structural step when a source or test
layout needs a new directory. `repo.create-file` cannot fill the gap because
its parent is required to exist and its content authority is unrelated to
directory entries. `repo.rename-file` cannot fill it because its destination
parent is required to exist and it is restricted to one eligible regular-file
move. Combining file creation, rename, deletion, or Execute would be an
unbounded or semantically incorrect authority composition.

The useful workflow is therefore two explicit operations when both are
authorized:

```text
repo.create-directory({path})
    -> repo.create-file({path, content})
```

Neither operation implicitly grants the other or gains a `create parents`
option.

## 4. Authority classification

`RepositoryDirectoryCreationPolicy` is a new private, opaque,
host-constructed authority. It grants one bounded native directory-entry
creation in one host-selected canonical repository and one runtime/repository
generation.

The authority planes remain distinct:

```text
directory creation != file creation != file deletion != file rename/move
                  != content mutation != index mutation != reviewed commit
                  != Execute
```

`PermissionLevel::Execute` may remain the outer Tool dispatch permission, as
it is for existing bounded repository tools. Execute alone is not directory
authority. A model request, provider metadata, Tool definition, ToolRegistry
registration, frontend state, Codex approval, or Trusted Profile declaration
cannot manufacture, elevate, or widen this authority.

The authority grants none of the following: arbitrary `mkdir`, recursive
creation, generic filesystem mutation, file write/create/delete/rename/copy,
directory delete/rename/copy/replacement, symlink/junction/reparse/mount or
special-file creation, mode/ACL mutation, shell/process execution, Git
commands, staging, commit, branch/ref/history mutation, repository selection,
network access, rollback, durability, or replay.

## 5. Proposed v1 Tool semantics

The future public name should be:

```text
repo.create-directory
```

The smallest model-supplied request is a closed object containing only:

```json
{"path":"existing-parent/new-directory"}
```

`path` is one normalized logical repository-relative path. The request must
not contain repository identity, root, cwd, executable, environment, absolute
path, mode, ACL, owner, permissions, recursive flag, overwrite flag, retry
control, or expected hash/length. A directory has no stable content preimage
analogous to a file, so a directory hash contract would be invented rather
than useful. Host-owned repository identity and generation remain outside the
request.

The conceptual verified result should be bounded and sanitized, for example:

```json
{
  "path":"existing-parent/new-directory",
  "status":"directory_created_verified",
  "uncertain":false,
  "git_metadata_changed":false
}
```

The final public status vocabulary belongs in the ADR and implementation, but
must distinguish `invalid_input`, `precondition_failed`, verified creation,
known no-effect, and `uncertain`. Results must not expose native paths, raw OS
errors, credentials, or policy internals.

## 6. Product and destination semantics

V1 answers the critical product questions as follows:

1. It creates exactly one leaf directory entry per call.
2. The immediate parent must already exist.
3. Any missing intermediate parent is rejected; `new-parent/new-child` does
   not silently create `new-parent`.
4. An already-existing directory is a failure, not idempotent success.
5. Any existing target object is a failure: directory, regular file, symlink,
   junction/reparse point, socket/device, or other filesystem object.
6. The repository root is an allowed parent after ordinary-directory,
   identity, metadata, and confinement validation.

This is “create exactly one directory entry,” not “ensure this directory tree
exists.” Recursive `mkdir -p` would authorize several effects, introduce
partial-success accounting and cleanup temptation, and materially broaden the
model-selected namespace operation.

The destination must be non-empty, bounded, normalized, slash-separated, and
repository-relative. Reject absolute and drive-relative paths, traversal,
empty or `.`/`..` components, backslashes, colon/ADS syntax, UNC,
verbatim/device namespaces, wildcard/glob forms, trailing path tricks, `.git`
and metadata paths, nested repository boundaries, and Windows reserved device
names. Case-insensitive or Unicode-normalization-equivalent collisions and
trailing-dot/space aliases must fail closed whenever the supported identity
model cannot distinguish them deterministically.

Every component from the selected repository root through the immediate parent
must be validated as the intended ordinary directory, with no symlink,
junction, mount-like redirection, or Windows reparse traversal. The final leaf
must be checked with no-follow metadata and must remain absent at immediate
revalidation. A target that appears in the race window is not RAH's successful
effect and is never overwritten.

## 7. Windows and Unix filesystem model

The intended Windows primitive is one host-selected `CreateDirectoryW` call
for the final path, with no recursive helper and no template/placeholder
behavior. Microsoft documents that it creates only the final directory,
reports an existing target as `ERROR_ALREADY_EXISTS`, and reports missing
intermediate directories as `ERROR_PATH_NOT_FOUND`:
<https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectoryw>.
The implementation must use the repository's validated path model and
revalidate parent/root identity immediately before the call. It must not use a
shell, `mkdir` subprocess, `CreateDirectoryExW` template, or a fallback that
creates ancestors. If the chosen Windows handle/path technique cannot give a
defensible no-reparse and no-replacement contract, the operation fails closed;
the ADR must not claim that ordinary path revalidation eliminates every
external TOCTOU race.

The intended Unix/Linux primitive is one `mkdirat`-style call relative to a
host-opened directory descriptor for the already-validated parent. Linux
documents that `mkdirat` interprets a relative name relative to `dirfd`, that
`EEXIST` includes any existing object including a symlink, and that only the
final component is created:
<https://www.man7.org/linux/man-pages/man2/mkdirat.2.html>.
Parent descriptors should be acquired by a no-follow, bounded component walk
(or an equivalent stronger platform primitive such as `openat2` where
available). A plain path walk followed by `mkdir` is not sufficient evidence
against component replacement races. Unsupported Unix platforms or platforms
where equivalent confinement cannot be proven should fail closed; this task
does not certify Linux live behavior.

V1 exposes no mode or ACL input. On Unix the host should use a fixed native
default, conventionally `0o777` subject to the process umask and applicable
default ACLs, with no follow-up chmod/chown/ACL operation. Windows should use
the host/native inherited security policy without model-selected security
attributes. These defaults must be stated as host policy, not as a model
authority.

## 8. Repository identity and mutation serialization

The policy should reuse the existing per-canonical-repository mutation lease
from the pre-effect validation through result construction. The lease
serializes RAH operations only; it does not exclude external processes,
antivirus, sync tools, or privileged actors.

The authority must bind and immediately revalidate:

- selected canonical repository root identity and repository metadata identity;
- host-owned repository/runtime generation;
- exact normalized logical destination identity;
- repository-root and immediate-parent filesystem identity;
- every parent component's ordinary-directory/no-link/no-reparse status;
- destination final-entry absence, including case/alias-equivalent absence;
- selected repository confinement and metadata/nested-repository exclusion; and
- the relevant index, HEAD, branch, and refs state needed to prove that this
  operation did not mutate Git state.

The revalidation must occur after acquiring the lease and immediately before
the one native attempt. A changed root, generation, parent identity, target
absence proof, or Git observation fails before effect. A lease is a useful
shared serialization boundary, not a substitute for these identity checks.

Unlike tracked-file deletion/rename, directory creation has no content
preimage and does not inherently require a clean worktree, clean index, or
attached HEAD. Therefore v1 should not impose global clean-state or branch
identity requirements merely by copying ADR 0017/0018. It may operate in a
valid unborn non-bare repository, provided the selected root/metadata, parent,
path confinement, and index/refs observations are valid. Bare repositories,
malformed metadata, unsupported worktree layouts, and contradictory Git state
remain precondition failures. An implementation may reject a narrower
unsupported repository mode only with an explicit ADR reason.

## 9. Git semantics

Git records files and tree entries needed by tracked contents; it does not
record an empty directory as an independent commit object. Git's own directory
listing documentation explicitly distinguishes an untracked empty directory
from tracked entries and can hide empty directories:
<https://git-scm.com/docs/api-directory-listing>.

Consequently, successful `repo.create-directory("src/new_module")` means:

- the ordinary filesystem directory now exists;
- no file was created;
- the index is unchanged;
- `HEAD` is unchanged when present;
- refs and branch history are unchanged; and
- `git status` may show no output and may remain clean while the directory is
  empty.

The capability must not create `.gitkeep`, `.keep`, marker files, or any other
placeholder. It must not stage or commit. The later `repo.create-file` call,
if separately authorized, supplies the first Git-visible content effect.
Git metadata checks are still valuable postconditions because they detect an
unexpected effect or observer contradiction; they are not a claim that Git
must report the directory.

## 10. Effect and failure classification

The future contract should use these classes:

### Failed before effect

Use this for invalid input, missing authority, invalid repository/generation,
unsafe path, missing/non-directory/replaced parent, existing target, or any
pre-effect revalidation failure. No native create attempt is made. A native
error can also be classified as known no-effect only when independent fresh
observation proves the target is still absent and protected state is intact.

### Verified success

Require all of the following after the one native attempt:

- the requested final path exists;
- it is an ordinary directory, not a symlink/reparse/special object;
- its parent and ancestry remain within the selected repository and retain the
  validated identity/no-redirection properties;
- the target was absent before the attempt and the postcondition is consistent
  with this operation's one creation;
- index bytes/fingerprint, HEAD, attached branch state when present, and refs
  are unchanged; and
- no file, marker, staging, commit, or second effect was made by RAH.

The result is filesystem success even if Git status is clean. “Verified” does
not promise crash durability, global exclusivity, rollback, or protection from
privileged external actors.

### Possible-effect uncertainty

Return `uncertain` when the native attempt may have occurred but post-observation
cannot prove verified success or known no-effect. Examples include timeout,
cancellation, disconnect, crash, lost result, contradictory target/parent
identity, inability to observe the postcondition, or an external actor racing
the target such that ownership of the resulting directory cannot be proven.

An `ERROR_ALREADY_EXISTS`/`EEXIST` result with independent proof that another
target existed is a failed-before-effect result for RAH, never RAH success. If
the target state is contradictory or observation is lost, classify
conservatively as uncertain.

## 11. Replay and rollback policy

There is exactly one native directory-create attempt. There is no automatic
retry after any native attempt that may have taken effect, including timeout,
cancellation, disconnect, or ambiguous OS failure. There is no replay from a
provider retry or runtime reconnect.

RAH must not compensate an uncertain create by deleting the directory. A
delete could destroy another actor's directory or content added after the
attempt, and deletion is a separate authority. A created empty directory may
remain after a later file-create failure; that is a persistent partial workflow
effect, not rollback failure. Recovery requires fresh observation and a new,
separately authorized operation.

## 12. Desktop/runtime composition

Future Desktop integration must follow the existing host-owned lifecycle:

- the selected repository and canonical root remain host-owned;
- the authority is bound to the repository/runtime generation;
- repository switching revokes the old context and prevents stale calls from
  acting in the new repository;
- the Tool is registered only when the host has constructed the authority;
- Generic Codex Bridge routing remains ordinary `ToolRegistry` dispatch; and
- sanitized lifecycle/effect events use the canonical public name.

After a verified directory create, Desktop should refresh repository/workflow
presentation even when Git status remains clean, because filesystem structure
changed and the UI may need to show the new location. The refresh must not
pretend Git tracked the empty directory.

The operation should revoke existing reviewed commit authorization and refresh
the review generation under the same fail-closed workflow rule used for other
repository worktree mutations. Git cannot commit the empty directory itself,
but a structural filesystem mutation occurred; retaining a review token across
that mutation would weaken the existing reviewed-snapshot invariant. The
refresh/revocation behavior is a future integration requirement, not a Task
181 source change.

## 13. Trusted Profile and authority composition

If accepted by a later ADR, Trusted Profile may contain a closed symbolic
declaration for `repo.create-directory` only to compose an already host-supplied
directory authority. It must not accept a raw repository path, generation,
mode, recursive flag, absolute destination, or policy settings that construct
or broaden authority. Profile metadata cannot elevate file-create authority
into directory-create authority. Profile validation and effective composition
must retain the existing static/effective distinction, redaction, fresh
registry, and fail-closed admission rules.

No existing authority implies directory creation. In particular, the answer to
the critical composition question is unequivocally **no** for file creation,
file deletion, file rename/move, worktree content mutation, index mutation,
reviewed commit, Execute, provider metadata, and frontend state.

## 14. Interaction with existing authorities

`repo.create-file` remains one explicit file effect. It must continue rejecting
missing parents and must not gain implicit parent creation. A caller with only
directory authority cannot create a file; a caller with only file authority
cannot create the missing directory. With both, two Tool calls and two host
authorizations are required.

`repo.rename-file` remains a one-file move to an existing destination parent.
Creating a directory and moving a tracked file into it are two explicit
effects and two authorities. Rename must not silently create parents.

Directory creation grants no directory deletion. Empty-directory deletion is
deferred entirely; no `repo.delete-directory` should be added to v0.15 for
symmetry. No cleanup or compensation is authorized after an uncertain create.

## 15. Deterministic test matrix for implementation

These are future test requirements, not claims that the tests exist today.
Do not invent names until implementation selects the test modules.

| Area | Required case | Expected result |
|---|---|---|
| Success | One child under repository root | One ordinary directory; verified success |
| Success | One child under an existing nested parent | One ordinary directory; verified success |
| Success | Postcondition inspection | Directory exists; no file inside it |
| Git | Index, HEAD, branch/refs snapshots | Unchanged; no staging or commit |
| Authority | Missing authority/permission | Rejected before native attempt |
| Input | Empty, absolute, drive-relative, traversal, backslash, ADS, device/UNC, reserved, wildcard, trailing trick | Rejected |
| Target | Existing directory, regular file, symlink, junction/reparse, special object, case/alias-equivalent object | Rejected; never idempotent/overwrite |
| Parent | Missing parent, file parent, symlink parent, unsafe reparse/junction parent | Rejected; no ancestor creation |
| Scope | Nested repository or metadata path | Rejected |
| Identity | Repository identity/generation mismatch | Rejected before effect |
| Race | Target appears during revalidation | Rejected or conservative non-success; never RAH success/overwrite |
| Race | Parent identity/type changes | Rejected or uncertain according to effect boundary |
| Semantics | Missing intermediate parents | Rejected; exactly one leaf only |
| Effect | Native attempt counter | Exactly one attempt on an eligible call |
| Failure | Possible native effect with lost/failed post-observation | `uncertain` |
| Failure | Known absent target after failed native call | Known no-effect |
| Safety | Uncertain result | No retry, replay, delete, or cleanup |
| Isolation | File bytes and unrelated tracked content | Unchanged |
| Boundary | No Git/process/shell fallback | No subprocess effect beyond permitted observation |
| Composition | ToolRegistry, profile, bridge, duplicate delivery, cancellation | Host authority required; closed schema; no broadened dispatch |
| Desktop | Selected repository, refresh, generation invalidation, review revocation | Stale context rejected; workflow refreshed/revoked |

Windows-gated fixtures should cover reparse points, junctions, reserved names,
case-insensitive equivalent targets, trailing dot/space aliases, and native
error mapping. Unix-gated fixtures should cover symlink components,
directory-FD anchoring, mode/umask behavior, and unsupported primitive paths.
Deterministic Ubuntu/Linux evidence must not be described as Linux live
certification.

## 16. Future Windows live-validation gate

Task 181 does not execute live validation. A later task should pin the
certified `codex-cli 0.149.0` executable pair, exact release-derived clean
baseline, and a disposable repository outside the checkout:

```text
tracked: sentinel.txt
existing: parent/
absent: parent/new-directory/
```

The host should advertise only the authorized canonical tool set and ask the
model to invoke `repo.create-directory` exactly once with exactly
`{"path":"parent/new-directory"}`. Independent host assertions must prove:

- `parent/new-directory` exists and is an ordinary directory;
- no file appeared inside it and the sentinel bytes/hash are unchanged;
- index bytes/fingerprint, HEAD, and refs are unchanged;
- no staging, commit, shell, Git mutation, retry, or replay occurred;
- lifecycle counts are exactly `tool_requested = 1`, `tool_started = 1`, and
  `tool_finished = 1`;
- the structured result is `directory_created_verified` with
  `uncertain = false`;
- raw JSONL parses, required redacted events are present, and
  `marker_observed = true` for the task's marker; and
- Git status may remain clean, which is expected and cannot replace the
  filesystem postcondition.

Live evidence observes an already host-authorized effect; it never grants
authority. Linux remains deterministic-only unless separately certified.

## 17. Explicit v0.15 non-goals

- recursive trees, `mkdir -p`, multiple directories per request, or
  ensure-directory/idempotent semantics;
- directory deletion, rename/move, copy, replacement, overwrite, or
  cross-volume behavior;
- automatic file creation, `.gitkeep`/placeholder creation, staging, commit,
  branch/ref/history mutation, checkout, reset, clean, stash, or network Git;
- generic filesystem mutation, generic `fs.mkdir`, shell/process fallback,
  provider-selected executable/cwd/environment, or network authority;
- symlink, junction, reparse-point, mount, device, special-file, ACL, chmod,
  owner, or model-selected mode creation;
- rollback, compensation, recovery, durability, automatic retry, or replay;
- Trusted Profile schema changes, profile reload/hot watch, public profile
  policy construction, Desktop implementation, or live validation in Task
  181;
- stronger OS sandbox, network-isolation, or all-race-elimination claims; and
- Linux live certification.

## 18. Open questions

No authority-critical question remains unresolved for Task 182. The ADR and
implementation still need ordinary platform-detail decisions, without
broadening this contract:

1. Which existing path helper should be factored or reused without importing
   file-specific Git assumptions?
2. Which Windows handle/reparse observation sequence gives the strongest
   practical final-component and parent identity proof for the supported OS
   baseline?
3. Which Unix platforms can provide the required no-follow directory-FD walk;
   which must fail closed?
4. What exact sanitized result and event field names best match current Tool
   conventions while preserving the three outcome classes?
5. What host-fixed Unix mode policy should be documented and tested against
   umask/default ACL behavior?

These are implementation and platform-admission details, not permission to
add recursion, idempotence, placeholders, or another authority.

## 19. Recommendation for Task 182

Proceed with:

> Task 182 — ADR for bounded repository directory creation authority

The ADR should accept the one-leaf/existing-parent/no-placeholder contract,
the separate host-owned policy, shared mutation lease, immediate identity
revalidation, one native no-replace attempt, verified filesystem postcondition,
Git metadata preservation, conservative uncertainty, and no-replay/no-delete
rules described here. It must leave ADRs 0010–0018, Trusted Profile schema,
Desktop integration, Rust implementation, release metadata, and tags
unchanged.

After ADR acceptance, implementation should remain separately authorized and
follow the proposed sequence: deterministic core policy/tests, composition and
bridge integration, Desktop lifecycle/review invalidation, hardening, then a
separate Windows live gate.
