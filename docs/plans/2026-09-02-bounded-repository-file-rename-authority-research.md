# Task 169 — Bounded Repository File Rename/Move Authority Research

**Status: research and documentation only.** This document does not accept an
ADR, implement Rust, add a Tool or policy, change a public protocol, modify
Trusted Profile or Desktop behavior, change dependencies or versions, or
authorize live validation. Its purpose is to define the smallest contract that
Task 170 could decide whether to accept.

## 1. Starting point and evidence reviewed

The authoritative starting state supplied for Task 169 is:

- `HEAD` and `origin/master`: `571e04b1d711d476f2ed2bc495e28f16f71583bd`;
- clean worktree;
- exact-head CI PASS, run `33594885777`;
- released RAH `v0.13.0` and closed Task 168 roadmap; and
- selected v0.14 headline: bounded one-file repository rename/move.

The source review covered the v0.14 roadmap, ADRs 0010, 0012, 0013, 0014,
0016, and 0017, the v0.13 deletion research/implementation/live-validation
documents, and the current implementations of repository worktree patch,
file creation, file deletion, stage/unstage, repository observation, Trusted
Profile composition, the Generic Codex Tool Bridge, and Desktop selected-
repository generation handling.

The existing implementation establishes several relevant facts. Repository
mutation tools use a RAH-owned per-canonical-root lease; repository root and
`.git` identities are revalidated; Git observations bind HEAD, index entries,
and refs; bounded tools use logical repository-relative paths; and results are
sanitized. `repo.delete-file` already demonstrates the clean HEAD-tracked
regular-file baseline and one native deletion attempt. Stage/Unstage remains a
separate Git/index operation. The Desktop rebuilds a registry for the selected
repository and increments a repository generation when context changes. The
bridge advertises canonical RAH names while routing through private aliases
when a Codex function name cannot use the dotted public name.

Those facts are foundations, not rename authority. In particular, the current
path helpers and `FileIdentity` implementations do not by themselves establish
a no-overwrite, cross-directory, handle-bound rename contract. The future
implementation must choose and test a platform-specific primitive instead of
assuming that `std::fs::rename`, `MoveFileExW`, or a Git command has the needed
semantics.

## 2. Decision: one authority, two paths, one effect

### 2.1 Rename and move are one authority in v1

Same-directory rename and cross-directory move both change one existing file's
directory entry from one logical pathname to another. They have the same
security questions: source eligibility, destination nonexistence, path
containment, alias/reparse checks, identity preservation, one-attempt
uncertainty, and unchanged Git/index state. Splitting them into
`repo.rename-file` and `repo.move-file` would duplicate the authority boundary
without reducing the effect for same-repository destinations.

The v1 authority is therefore one bounded **repository file rename/move**
authority with exactly one source path and exactly one destination path. The
public semantic name proposed for the eventual Tool is `repo.rename-file`.
“Rename” is the filesystem operation name and includes moving the directory
entry within the selected repository; “move” is not a license for arbitrary
filesystem movement.

### 2.2 Cross-directory movement is included, with a narrow prerequisite

V1 should allow both:

- `src/module.rs -> src/old/module.rs` when the destination parent already
  exists; and
- `src/module.rs -> tests/fixtures/module.rs` when the destination parent
  already exists and passes the same validation.

Cross-directory movement has materially more parent identity and same-volume
proof burden, but not a different authority semantic when both parents are
ordinary directories beneath one selected repository. Excluding it would make
the capability too weak for the common module/test move that motivates it.

The authority must reject the operation when the native primitive cannot prove
one same-filesystem, no-overwrite directory-entry effect. It must never fall
back to copy plus delete. A destination parent is not created by this v1
capability.

## 3. Source eligibility

The source must be the narrowest useful repository-owned preimage:

1. It is one explicit logical repository-relative path beneath the host-
   selected canonical repository root.
2. It resolves to an existing ordinary regular file, not a directory,
   symlink, junction, mount redirection, other reparse point, device, FIFO,
   socket, or special file.
3. It is present in the current HEAD tree as one ordinary blob and has exactly
   one normal stage-0 index entry equal to that HEAD entry.
4. Its worktree bytes are exactly equal to the HEAD blob bytes, including raw
   newline bytes and any BOM. The source is not merely clean according to a
   timestamp or Git's cached stat data.
5. Its host-captured SHA-256 and byte length equal the request precondition and
   the independently observed HEAD blob. The request values are checked
   preconditions, never authority.
6. The file has no ambiguous hard-link state. V1 should require link count one
   where the platform can reliably observe it and refuse when the required
   observation is unavailable.
7. It is not a submodule/gitlink, nested repository boundary, sparse or
   skip-worktree ambiguity, or repository metadata path.

The repository must be an ordinary supported non-bare checkout with an
attached current branch, a valid HEAD, a normal index, and no merge, rebase,
cherry-pick, revert, sequencer, bisect, or other state that makes the source
ownership or index interpretation ambiguous. Linked worktrees, alternate
indexes, malformed/unreadable indexes, and unsupported Git states are refused
in v1.

Unrelated dirty paths may remain. Requiring a globally clean worktree would
unnecessarily seize authority over unrelated user work. The target itself,
its relevant parents, repository identity, HEAD, index, branch, and refs must
remain bound and supported. The capability must not read or expose unrelated
file contents merely to enforce this exception.

The clean-HEAD requirement is intentional. Moving a locally modified file
could be useful, but it lacks the simple immutable repository-owned preimage
and risks structurally relocating unreviewed work. Supporting dirty, staged,
untracked, ignored, or conflicted sources is a separate authority decision.

## 4. Destination eligibility

The destination is a second, equally explicit logical repository-relative path.
It is not a model-selected native path or a destination assembled by the host
from an arbitrary directory option. V1 requires all of the following:

- normal `/`-separated nonempty components only;
- no absolute, drive-relative, drive-qualified, UNC, verbatim, device, or
  colon/alternate-data-stream syntax;
- no `.` or `..`, empty components, glob/wildcard syntax, unsupported Unicode
  normalization, or rejected case/alias forms;
- containment beneath the same selected canonical repository root;
- no `.git` or other repository metadata component, case variant, metadata
  indirection, submodule boundary, or nested repository boundary;
- an existing parent directory chain, with every parent verified as the
  intended ordinary directory and no symlink, junction, mount redirection, or
  reparse point in the chain;
- destination currently nonexistent, including no regular file, directory,
  link, special entry, or alias-equivalent entry; and
- source and destination parent on the same supported filesystem/volume.

The parent is an existing-directory precondition, not a directory-creation
request. A missing parent is `precondition_failed`. Directory rename is not
implicitly enabled by accepting a destination directory; the destination final
entry must be absent and the source must be a regular file.

The operation rejects all of these rather than interpreting them as
convenience cases: existing untracked destination, existing tracked
destination, existing directory, destination parent missing, destination
through link/reparse ancestry, nested repository destination, overwrite,
replace, merge, and source/destination alias collision. There is no overwrite
boolean because overwrite is never valid in v1.

On case-insensitive Windows, a case-only logical change such as `Foo.rs` to
`foo.rs` is rejected entirely in v1. It can be indistinguishable at the native
name layer, and a safe implementation would require a dedicated algorithm and
proof that its multiple namespace observations still satisfy one effect. A
future capability may reconsider it only after deterministic coverage for
NTFS case-sensitive directories, Unicode case/normalization equivalence,
trailing dot/space aliases, reserved device names, ADS/colon syntax, 8.3
aliases, verbatim and UNC namespaces, and exact native behavior. No current
checkout configuration is a sufficient reason to admit it.

## 5. Authority binding and preimage

The trusted host, under the existing per-repository mutation lease, constructs
an opaque, short-lived operation binding. Conceptually it contains:

1. selected canonical repository identity, including root identity and the
   validated `.git` identity;
2. host-selected runtime identity and repository/runtime generation;
3. attached branch identity, exact HEAD OID, and supported repository-state
   fingerprint;
4. source logical path and source canonical/native identity;
5. source HEAD tree/blob identity, normal stage-0 index entry, exact raw
   worktree SHA-256, and exact byte length;
6. destination logical path and destination canonical parent identity;
7. independent proof that the destination final entry did not exist;
8. raw index bytes or an equivalent collision-resistant index fingerprint,
   plus the semantic source entry; and
9. relevant refs and repository-state fingerprint sufficient to prove no
   branch/ref/history mutation.

The model-provided source digest and length may be required in the request to
make a stale request explicit, but the host must independently obtain and
compare the bytes, HEAD blob, stage-0 entry, and identities. No model field can
select the repository, generation, branch, native path, parent handle,
primitive, retry behavior, or recovery behavior.

For a native move, the binding does not promise that the source's native file
identity will change or remain unchanged in every platform implementation. It
does require that the destination is the same protected file where the
platform's handle/identity observation can prove that fact. At minimum the
exact bytes and length must remain equal, and the operation must not create a
second file through copying.

Immediate final revalidation must independently re-check, without trusting a
stale capture:

- root and `.git` identity, repository and runtime generation;
- current HEAD, attached branch, supported repository state, and relevant refs;
- raw index/fingerprint and the exact normal stage-0 source entry;
- source path ancestry, source identity, regular-file type, link count, bytes,
  SHA-256, and length;
- destination logical path, destination absence, both parent identities, all
  parent types, and same-volume/filesystem support; and
- no unsupported alias, case collision, link, reparse, mount, metadata, or
  nested-repository condition.

Any contradiction fails closed before the native effect. If post-observation
cannot prove the required result, the operation remains uncertain; the host
must not repair, move back, stage, or retry it.

## 6. Race model and its limits

The lease serializes RAH-owned mutation calls for the repository. It does not
exclude an editor, antivirus, indexer, Git process, filter driver, or another
privileged local process. The contract therefore mitigates, but does not
eliminate, cross-process TOCTOU races.

Required outcomes include:

| Race | Required handling |
| --- | --- |
| Source observed, then modified | Final identity/bytes/HEAD check refuses before effect. |
| Source replaced with another file | Final native identity and bytes check refuses; an unprovable contradiction is not success. |
| Destination absent, then created | Atomic no-replace primitive fails without overwrite; classify as known no effect only after fresh proof. |
| Parent replaced by junction/reparse | Parent identity/handle revalidation refuses; never follow the replacement. |
| HEAD/index/branch/refs changes | Final repository fingerprint mismatch refuses before effect. |
| Repository switched/reconnected | Generation or canonical identity mismatch refuses; old authority is not revived. |
| Native effect or result is lost | Post-observation may prove `renamed_verified`; otherwise result is `uncertain`, with no replay. |

The implementation must not claim complete TOCTOU elimination, cross-process
locking, malicious-local-actor protection, or OS sandboxing. A successful
final observation is evidence about the bounded operation, not a universal
filesystem integrity guarantee.

## 7. Native effect primitive

### 7.1 Rejected primitives

`git mv` is rejected. It intentionally updates the Git index and would collapse
worktree structural movement into the human-controlled index authority. The
future operation must leave the index unchanged.

Copy plus delete is rejected. It creates two durable effects, can leave a
partial copy or both names after a failure, can change file identity and
metadata semantics, and creates an unjustified rollback/compensation problem.
`MoveFileExW` with `MOVEFILE_COPY_ALLOWED` is specifically not acceptable
because Microsoft documents that it simulates a cross-volume move with copy and
delete. No generic filesystem rename Tool or shell command is acceptable.

### 7.2 Preferred Unix/Linux direction

Use a descriptor/parent-bound native rename with no-replace semantics, with
`renameat2(..., RENAME_NOREPLACE)` as the preferred Linux primitive where
available. Linux documents that ordinary `rename()` atomically replaces an
existing destination, while `RENAME_NOREPLACE` refuses an existing destination;
ordinary rename also fails across mounted filesystems. Those distinctions are
central to this contract. If the supported target does not provide a tested
atomic no-replace primitive, v1 should refuse rather than emulate it with
check-then-rename or link-plus-unlink.

The exact implementation may use a carefully bounded `renameat` variant on
platforms where its no-replace behavior is independently proven, but Task 170
must specify the supported platform matrix and deterministic tests. Parent
directory descriptors and `AT_*`-style operations reduce path re-resolution
windows; they do not make external races impossible.

### 7.3 Preferred Windows direction

Use a tested Unicode, handle-oriented Windows rename operation, preferably
`SetFileInformationByHandle` with `FileRenameInfo` or the appropriate tested
`FileRenameInfoEx` no-replace form. The source handle, validated parent
identity, destination name, and `ReplaceIfExists = FALSE`/equivalent must be
bound to the operation. The final implementation must reject reparse traversal,
unsupported namespace forms, cross-volume movement, and any behavior that
could replace an existing destination.

`MoveFileExW` is a possible implementation seam only if the ADR and tests
prove that the chosen flags provide the required no-overwrite, same-volume,
non-copy behavior and do not re-resolve an unsafe path. `MOVEFILE_REPLACE_EXISTING`,
`MOVEFILE_COPY_ALLOWED`, delayed-until-reboot, and hard-link-related modes are
not allowed. The safer research direction is a handle-bound rename using
`SetFileInformationByHandle`; the final choice remains an ADR implementation
detail that must be proven on the certified Windows target.

Microsoft documents that `SetFileInformationByHandle` changes information for
an opened handle and that `FILE_RENAME_INFO` supplies rename data. This is a
better authority boundary than an ambient shell or Git process, but it does
not itself prove reparse safety, no-overwrite behavior, same-volume behavior,
or post-effect visibility. Those properties must be checked and tested.

## 8. Effect commit point and result taxonomy

The sole filesystem effect commit point is the one native no-replace rename
attempt after immediate final revalidation. The operation shape is:

```text
host validation
 -> immediate source/destination/repository revalidation
 -> exactly one native no-replace rename attempt
 -> filesystem effect commit point
 -> post-effect observation
```

The taxonomy should remain small:

| Result | Meaning |
| --- | --- |
| `invalid_input` | Closed request schema or logical path form is invalid; no authority evaluation or native attempt. |
| `precondition_failed` | Host identity, source, destination, repository state, generation, or supported-platform precondition failed before the native attempt. Destination-exists, missing parent, dirty source, and alias collision belong here. |
| `renamed_verified` | Exactly the authorized source entry is absent, the destination entry is present as the protected unchanged file/content, and index, HEAD, branch, refs, and repository identity remain unchanged. |
| `known_no_effect` | An attempt may have been approached or reported failed, but fresh proof establishes that the protected source remains intact and the destination has not become the authorized effect. |
| `uncertain` | The native effect or required observation cannot prove either `known_no_effect` or `renamed_verified`. |

There is no separate `destination_exists` result in v1. It is a bounded
`precondition_failed` reason, redacted from model-visible details as needed.
OS return status alone is not sufficient. A sharing error, timeout,
cancellation, disconnect, crash, lost return, or observer failure after the
effect boundary is uncertain unless independent post-observation proves one
of the two safe classifications.

Before the effect point, a refusal is definitely no-effect only when the
source preimage and destination absence/intended state are freshly proven. A
reported native failure after the call is not automatically known no-effect.

## 9. Successful postconditions

`renamed_verified` requires proof of all of the following:

- source logical pathname is absent;
- destination logical pathname exists as one ordinary regular file;
- destination bytes and byte length equal the captured source bytes;
- where available, destination identity is the source identity or the native
  operation's documented preserved identity, with no extra hard-link effect;
- source and destination ancestry remain beneath the selected repository and
  satisfy link/reparse/alias rules;
- index bytes/fingerprint and semantic entries are unchanged;
- HEAD OID, attached branch, refs, and relevant repository state are unchanged;
- no Git process was used to perform the move, no stage/unstage occurred, and
  no commit/history/ref operation occurred;
- no overwrite, replacement, directory creation, copy, or second path effect
  occurred; and
- unrelated sentinel evidence, when present in deterministic/live fixtures,
  is unchanged.

On Unix, an inode/device identity is normally observable and a native rename
preserves the inode while changing directory entries, subject to filesystem
behavior. On Windows, volume serial plus file index can identify an opened
file, but identity observations and sharing/filter behavior are platform
specific. A successful API return does not prove that a name is immediately
visible to every observer. The contract should require exact bytes and
deterministic source-absent/destination-present observation, and should state
precisely which Windows identity evidence is required rather than promise
universal metadata preservation. Timestamps, ACLs, extended attributes,
compression/encryption flags, and arbitrary alternate streams are not to be
silently repaired or transformed; unsupported cases should refuse.

## 10. Git semantics and presentation

The native effect changes only worktree directory entries. It must not invoke
`git mv`, `git add`, `git rm`, `git restore`, or another index mutator. The
index continues to contain the old stage-0 path and no new destination entry.

Accordingly, Git should ordinarily expose the result as an unstaged deletion
of the old path plus an untracked destination path until a human stages the
change. Git may display a rename only when its status/diff rename detection
heuristically pairs the delete/add content; that presentation is not an index
rename and is not authoritative. Git's status documentation distinguishes
worktree changes, untracked paths, and rename detection; RAH must preserve
that distinction.

Human Stage/Unstage remains the index authority. The RAH repository UI may
describe a verified operation semantically as “renamed old/path to new/path”
and show both logical paths, while also showing raw Git status as delete plus
untracked destination when that is what Git reports. It must not claim that the
index contains a rename before staging.

## 11. Separation from creation and deletion authority

The native rename necessarily allocates a new directory entry at the
destination and removes the old directory entry. Those are intrinsic parts of
one authorized filesystem effect, not separate model-visible create and delete
calls. The operation does not grant general file creation or deletion:

- `RepositoryFileCreationPolicy` authorizes an exclusive new file with
  model-provided content under its own contract; it does not authorize moving
  an existing file.
- `RepositoryFileDeletionPolicy` authorizes removal of one protected existing
  file; it does not authorize relocating that file or allocating a new name.
- `create authority + delete authority` is not capability algebra. Combining
  them would create a two-call gap, lose the atomic/no-overwrite contract,
  permit mismatched content, and invite partial failure.

Rename/move must therefore be a distinct host-constructed authority even when
the effect visibly includes one old-entry removal and one new-entry creation.
No automatic staging follows; the index remains a separate human authority.

## 12. Proposed future request and authority shape

The eventual narrow model-visible request should be:

```json
{
  "source_path": "src/module.rs",
  "destination_path": "src/old/module.rs",
  "expected_source_sha256": "lowercase-64-hex-sha256",
  "expected_source_byte_length": 1234
}
```

Unknown fields, missing fields, malformed digest/length, NUL, oversized
serialized input, non-logical paths, and source/destination alias forms are
invalid. No native absolute path, cwd, flags, overwrite boolean, recursive
option, destination-creation option, retry control, shell command, Git argv,
or call-selected filesystem is exposed. The host supplies the operation
primitive, root, parent handles, generation, and all authority.

The conceptual private objects are `RepositoryFileRenamePolicy` and
`RepositoryFileRenameAuthority`, or equivalent names chosen by Task 170. The
authority should be opaque, host-created, generation-scoped, non-serializable
as authority, and consumable only by a Tool registered through `ToolRegistry`.
It must bind the selected repository and share the existing repository lease.
Its lifetime must not outlive the host-selected context in a way that permits
stale use.

The final public Tool name should be `repo.rename-file`, because it describes
the one semantic operation while covering same-directory and same-repository
cross-directory cases. `repo.move-file` would be understandable but is more
easily misread as arbitrary filesystem movement. The name is a proposal only;
Task 170 must finalize it.

## 13. Trusted Profile, bridge, and Desktop implications

Trusted Profile may eventually expose the capability only by composing an
already host-constructed, already accepted rename authority for the selected
repository. Profile configuration cannot select an arbitrary root, create the
authority, change source/destination eligibility, enable overwrite, relax
case/reparse rules, revive a stale generation, or derive rename by composing
creation and deletion entries.

Generic Codex integration should use ordinary `ToolRegistry` registration and
the existing generic bridge route. It should advertise the canonical public
name and use the normal private alias translation where necessary. No
rename-specific bridge bypass, model-visible native path, or provider-owned
authorization is justified. The bridge must preserve exact-once request
handling and must not replay an uncertain call. The separate
`ToolContent::Json` lifecycle evidence formatting debt from Task 163 should be
fixed before relying on structured rename success evidence, but it is generic
observability work, not rename authority.

Future Desktop integration should construct the capability from the current
selected repository authority, bind it to the current repository generation,
refresh repository status/diffs/file information after any terminal result,
and invalidate stale presentation/action/review state after a verified
structural worktree change. A successful move should revoke any commit review
whose bound snapshot is no longer current. The frontend can show old and new
paths and raw Git delete/untracked presentation, but it cannot grant or relax
authority. A repository switch, reconnect, or generation mismatch must make
the old capability unusable.

## 14. Directory, untracked, and content scope decisions

Directory rename/move is deferred. It recursively changes a namespace,
possibly crosses nested repositories, contains links/reparse children, can
have huge and poorly observable effects, and makes exact postconditions and
uncertainty much harder. A regular-file source check must reject it.

Untracked and ignored source moves are deferred. They have no HEAD-backed
immutable source reference, may contain local configuration, generated data,
or scratch work, and would turn the operation into broader local-filesystem
authority. A future untracked contract would need a new host-captured
preimage and separate authority decision.

The effect is structural only. It must not alter bytes, normalize line endings,
rewrite encoding, update imports/references, patch content, or perform
refactoring. Repository-aware refactoring is a future compound authority and
cannot be hidden inside rename.

## 15. Replay, cancellation, and rollback

Once the native call may have begun, the call is an external filesystem effect.
There is exactly one attempt per authorization. Timeout, cancellation,
disconnect, crash, or a lost response does not imply rollback. The host must
not automatically retry, replay the same call ID, move the file back, restore
the source, stage either path, or invoke Git compensation. A later attempt
requires fresh source/destination/repository observation and new host
authorization. This is true even if the first call was reported as a native
failure, because post-effect visibility may be delayed or incomplete.

The bridge's duplicate-request handling remains the call lifecycle boundary;
it is not a filesystem rollback mechanism. A conflicting reuse of a call ID
must be rejected according to existing bridge rules, and an uncertain result
must remain terminal for that authorization.

## 16. Deterministic test matrix for implementation work

Task 170 and later implementation work should build the following matrix
without requiring credentials, network, a live model, or the checkout itself
as a mutable fixture.

### Success

- clean tracked regular file, same-directory rename;
- clean tracked regular file, existing-parent cross-directory move;
- exact bytes and length preserved;
- source absent and destination present;
- destination identity/content proves the protected source where supported;
- index bytes/semantic state unchanged;
- HEAD, branch, refs, and history unchanged;
- unchanged sentinel and no second path effect;
- Git presentation is unstaged delete plus untracked destination, with rename
  detection treated as presentation only.

### Preconditions and races

- dirty source, staged source, staged deletion, intent-to-add, conflict,
  source untracked, source ignored, source missing, directory, submodule,
  nested repository, sparse/skip-worktree, and unsupported Git state;
- stale source SHA/length, source bytes changed, source identity replaced;
- destination existing untracked file, tracked file, directory, link, or
  alias-equivalent name;
- missing destination parent, parent identity replacement, link/reparse
  ancestry, `.git`/metadata target, path escape, unsupported alias or
  normalization;
- destination appears after initial observation;
- HEAD, branch, refs, index bytes, repository identity, runtime generation, or
  selected repository changes before final revalidation;
- same-volume proof unavailable, cross-volume/mount boundary, read-only or
  unsupported filesystem, sharing violation, and native no-replace primitive
  unavailable;
- unrelated dirty path remains untouched and does not authorize a target move.

### Windows-specific cases

- case-only rename decision (v1 rejects);
- NTFS case-sensitive directory behavior;
- Unicode case/normalization equivalence and trailing dot/space aliases;
- reserved device names, ADS/colon, verbatim and UNC forms, and 8.3 aliases;
- source/destination/parent reparse points and junction replacement;
- file sharing violations, open handles, filter-driver interference, and
  delayed visibility;
- volume/file identity observation, hard-link count, executable regular-file
  behavior, and exact byte preservation.

### Effect and bridge cases

- native failure with proven intact source and absent destination;
- successful effect with verified postconditions;
- reported success/failure with failed or incomplete post-observation;
- timeout, cancellation, disconnect, and lost result at or after the commit
  point;
- exactly one native attempt, no automatic retry, no move-back, and no replay;
- duplicate exact call and conflicting call-ID reuse;
- ToolRegistry Execute denial, Trusted Profile composition, canonical public
  name/private alias routing, sanitized result, and lifecycle evidence.

Linux tests should cover symlink, inode replacement, destination collision,
`RENAME_NOREPLACE`, mount boundary, and uncertain observation paths. Windows
must have deterministic seams for its native handle/rename behavior; unsupported
semantics remain refusal rather than inferred support.

## 17. Eventual Windows live gate design (not executed here)

Only after ADR acceptance, deterministic implementation/hardening, Trusted
Profile and bridge integration, Desktop integration, and any observability
prerequisite should a Windows live gate be designed around a disposable
repository outside the checkout. It should contain one clean tracked source,
one unchanged sentinel, and, for cross-directory coverage, one empty existing
destination directory. The repository index/worktree must be clean before the
call.

The gate must pin the certified release binary and complete Codex package, and
must not assume a future private alias. Evidence must record the canonical
public Tool name, the logical source/destination, pre/post hashes and lengths,
and exactly one `ToolRequested`, one `ToolStarted`, and one `ToolFinished`.
Post-live proof must show source absent, destination present with original raw
bytes, sentinel unchanged, index unchanged, HEAD/branch/refs/history unchanged,
no auto-stage/commit, Git's unstaged structural result, no replay, terminal
state, cleanup, and a durable completion marker. Host Git/filesystem evidence
must remain independent of model prose.

The Task 163 `ToolContent::Json` evidence formatting issue should be a small
prerequisite task before this gate if the existing generic evidence sink still
records structured results as `null`. It should not be folded into the rename
authority or allowed to change its result semantics. If the fix is generalized,
it belongs under generic observability hardening with its own deterministic
coverage.

## 18. Explicit v0.14 non-goals

The v0.14 rename/move slice must not include:

- directory, recursive, wildcard, glob, or arbitrary filesystem movement;
- untracked/ignored source movement unless a later explicit decision selects it;
- overwrite, replace, merge, destination directory creation, or collision
  resolution;
- cross-repository or cross-volume movement, or copy/delete fallback;
- copy, content edit, encoding/newline transformation, refactoring, or import
  updates;
- automatic stage, unstage, commit, branch/ref/history mutation, network Git,
  or `git mv`;
- generic `fs.rename`, `fs.write`, `fs.unlink`, shell/process, or generic Git
  authority;
- rollback, compensation, generic undo, automatic retry, or replay;
- frontend-owned authority, profile-created authority, provider metadata
  authorization, or a rename-specific bridge bypass; and
- claims of OS sandboxing, network isolation, race freedom, or guaranteed
  rollback.

## Primary references

- RAH `docs/adr/0010-repository-mutation-policy.md` — host-owned mutation,
  lease, pre/post proof, and uncertainty.
- RAH `docs/adr/0012-repository-worktree-content-mutation-authority.md` —
  tracked clean worktree content boundary and native replacement precedent.
- RAH `docs/adr/0013-repository-file-creation-authority.md` — exclusive
  creation and destination/link restrictions.
- RAH `docs/adr/0014-bounded-multi-file-repository-edit-authority.md` —
  multi-file exclusion and transactional/rollback restraint.
- RAH `docs/adr/0016-bounded-repository-commit-authority.md` — separate
  reviewed index/history authority.
- RAH `docs/adr/0017-bounded-repository-file-deletion-authority.md` — closest
  one-path structural effect and deletion/no-replay contract.
- [Microsoft `MoveFileExW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw) — documents replacement flags and that `MOVEFILE_COPY_ALLOWED` simulates cross-volume copy/delete.
- [Microsoft `SetFileInformationByHandle`](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) and [`FILE_RENAME_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) — handle-based rename information APIs to be selected and tested by the ADR implementation.
- [Linux `rename(2)`/`renameat2(2)`](https://www.man7.org/linux/man-pages/man2/renameat.2.html) — atomic same-filesystem rename, replacement behavior, and `RENAME_NOREPLACE`.
- [Git `git-status`](https://git-scm.com/docs/git-status.html) — worktree/index/untracked state and rename-detection presentation.

## Recommended v1 authority contract

One new host-owned, opaque, generation-scoped `RepositoryFileRenameAuthority`
should authorize exactly one native no-replace rename of one clean HEAD-tracked
ordinary regular file from one explicit logical repository-relative path to one
currently nonexistent explicit logical repository-relative path. Same-directory
rename and cross-directory movement are both admitted only within the same
selected ordinary repository, with existing validated parents and a proven
same-filesystem/volume primitive. The source must equal its current HEAD blob
and normal stage-0 index entry; the destination must have no alias, collision,
metadata, link, reparse, nested-repository, or case-only ambiguity. The host
binds repository identity, runtime/repository generation, branch, HEAD, source
identity/blob/hash/length, index, refs/state, destination absence, and parent
identities, then independently revalidates immediately before one native
attempt.

The preferred effect is a platform-native descriptor/handle-oriented
no-overwrite rename: Linux `renameat2` with `RENAME_NOREPLACE` where supported,
and a tested Windows `SetFileInformationByHandle` rename form with no replace
and no copy fallback. The native attempt is the effect commit point. Results
are `invalid_input`, `precondition_failed`, `renamed_verified`,
`known_no_effect`, or `uncertain`; destination-exists is a precondition
failure. Exact bytes, source absence, destination presence, unchanged index,
HEAD, branch, refs, and no second effect are required for verified success.
No uncertain result is retried or replayed, and no compensation is attempted.

## Explicit rejected alternatives

- Two authorities (`rename-file` and `move-file`) for the same one-file,
  same-repository effect: rejected as redundant authority surface.
- Same-directory-only v1: rejected because existing-parent cross-directory
  moves have the same security semantics and are a core use case.
- Dirty, staged, untracked, ignored, conflicted, submodule, or directory
  sources: rejected because their ownership/preimage semantics are broader or
  ambiguous.
- Existing destination, overwrite, replace, merge, or implicit parent
  creation: rejected because they destroy no-overwrite and bounded-path proof.
- `git mv`: rejected because it mutates the index.
- Copy plus delete or cross-volume `MOVEFILE_COPY_ALLOWED`: rejected because
  it creates a two-effect partial-failure and file-identity problem.
- Generic path-based `fs.rename` or shell/Git subprocess: rejected because it
  does not establish the closed host authority boundary.
- Automatic case-only rename algorithm: rejected for v1 because Windows alias
  and namespace semantics require a separate proof.
- Composition of creation plus deletion authority: rejected because
  capabilities are not additive algebra and the two calls cannot be atomic.
- Automatic staging, commit, rollback, move-back, retry, or replay: rejected
  to preserve index/history separation and uncertain-effect safety.

## Proposed ADR scope

Task 170 should decide and normatively specify the new private authority,
one public semantic Tool name, closed request/result schemas, source and
destination eligibility, same-directory and same-repository cross-directory
scope, case-only rejection, path/reparse/alias rules, repository/runtime
generation binding, exact preimage and index/HEAD/ref evidence, shared lease,
native primitive and supported platform matrix, one-attempt commit point,
postconditions, uncertainty/no-replay behavior, Trusted Profile composition,
generic bridge routing, and Desktop refresh/review invalidation obligations.
It should authorize neither implementation shortcuts nor generic filesystem,
Git, creation, deletion, staging, commit, directory, untracked, rollback, or
refactoring capabilities.

## Proposed Task 170 implementation sequence

1. Accept ADR 0018 from this research, preserving ADRs 0010–0017 and the
   host-owned worktree/index/history boundaries.
2. Implement the private policy/authority, logical two-path request parsing,
   repository and generation bindings, source/destination validation, and
   bounded sanitized results without adding profile, Desktop, or live scope.
3. Implement one platform-native same-volume no-replace effect seam and the
   exact commit-point/uncertain-result handling; do not add copy fallback.
4. Add deterministic Linux and Windows tests for the full source,
   destination, identity, alias/reparse, repository-state, collision, race,
   postcondition, cancellation, and no-replay matrix.
5. Harden deterministic evidence and verify index/HEAD/refs remain unchanged;
   separately complete generic `ToolContent::Json` observability correction if
   still required for durable structured results.
6. Add host-owned Trusted Profile composition and ordinary ToolRegistry/
   Generic Codex Bridge routing with canonical `repo.rename-file` semantics.
7. Add Desktop selected-repository generation guards, repository refresh, old/
   new path presentation, and commit-review invalidation.
8. Pin the certified Windows baseline and run the disposable one-file live gate
   with independent filesystem/Git/lifecycle evidence and no replay.
9. Complete the v0.14 milestone audit, then prepare release evidence.

Each step is a separate authorization boundary. Implementation must not begin
from this research alone, and live or Desktop work must not be folded into the
ADR or deterministic policy task.
