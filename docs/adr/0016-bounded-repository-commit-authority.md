# ADR 0016 — Bounded Repository Commit Authority

Status: Accepted

## Context

RAH supports host-owned repository selection and inspection, separately bounded
worktree edits, and bounded index mutation. That workflow stops at the index.
Task 132 selected bounded repository commit authority; Task 133 established
that normal Git commit creates Git objects, advances the attached current branch,
and normally mutates HEAD and branch reflogs.

ADR 0010 is insufficient: it governs bounded repository index mutation. Commit
crosses a different authority plane:

    index
      -> Git history / current-branch ref

A normal commit also creates Git objects and reflog metadata. Thus stage
authority is not commit authority, and repo.commit cannot be an incidental
extension of stage/unstage. repo.edit-files does not imply stage authority;
commit authority does not imply branch-switch, arbitrary-ref, or remote-Git
authority.

## Decision

A trusted host may authorize exactly one normal Git commit that records one
specific reviewed staged index snapshot in one exact authorized repository and
advances only that repository's already-attached current branch from one exact
expected parent commit. The authority owner is the trusted host, not the model
and not the provider.

This authority is not implied by PermissionLevel::Execute, ADR 0010 index
mutation, repo.patch, repo.create-file, repo.edit-files, stage/unstage authority,
repository selection, Tool registration, provider metadata, a model request,
Codex approval, or Desktop UI state alone. PermissionLevel::Execute is an outer
permission gate only; it is not commit authority.

### Exact authorized effects

The bounded operation authorizes only:

- During reviewed-snapshot capture or validation, git write-tree may create
  otherwise unreachable tree objects needed to derive the exact staged tree.
- Creation of required Git tree object(s), if absent, and exactly one commit
  object for the successful operation.
- Advancement of exactly the already-selected attached branch ref.
- Normal HEAD and branch reflog mutation, including logs/HEAD and
  logs/refs/heads/<current-branch> where reflogs are enabled/applicable.
- Git-internal lock, temporary, cache, and metadata writes required by normal
  operation.

The symbolic .git/HEAD attachment remains the same branch. The operation does
not intentionally authorize worktree-content mutation, index/staging mutation,
another branch/ref update, tag mutation, remote mutation, or config mutation.
It must not claim zero incidental filesystem writes or absence of unreachable
objects after failures.

git write-tree is expressly allowed during authorization preparation because it
produces the semantic staged tree required to prove the future commit. It is not
purely read-only and may create unreachable tree objects, but it must never
update refs, stage files, or change worktree content. No generic object-writing
Git authority follows from this exception.

### Attached branch and index admission

v1 requires a non-bare ordinary selected repository with a normal .git directory
under the supported repository identity model; attached symbolic HEAD resolving
to exactly refs/heads/<validated-current-branch>; an existing parent commit; the
exact expected old HEAD OID; and the current branch ref equal to that old OID.
Canonical repository and native executable identity, attachment, branch/HEAD,
index state, tree, and admission are revalidated immediately before the one
attempt.

v1 rejects detached HEAD, unborn HEAD, arbitrary target ref, model-supplied
branch, branch creation, branch switching, linked worktrees, and bare
repositories. A v1 commit has exactly one parent, the authorized old HEAD.

The host-reviewed snapshot binds:

1. raw SHA-256 of the exact selected real index file;
2. canonical staged-entry semantic digest from controlled
   git ls-files --stage -z --no-abbrev; and
3. staged tree OID from git write-tree.

Raw index bytes catch byte races/replacement. The staged-entry digest provides
canonical semantic/index-entry evidence. The tree OID proves exactly what may
be committed. A tree alone loses relevant index-state distinctions; raw bytes
alone do not prove committed semantic content. All components must match in the
final pre-spawn revalidation.

Admission proves a real staged tree difference from HEAD, equivalent to
controlled git diff --cached --quiet HEAD reporting a difference. Empty commits
and --allow-empty are forbidden. Metadata/cache-only index changes do not
justify a commit; permitted tracked-entry tree and executable-bit differences
do.

v1 rejects unmerged entries, conflict stages, intent-to-add, sparse index,
sparse checkout unless safe semantics are proven, alternate index, redirected
index path through environment, malformed/unreadable index, and all unsupported
index states. It rejects staged gitlink/submodule entries. It also fails closed
on MERGE_HEAD, CHERRY_PICK_HEAD, REVERT_HEAD, rebase state, sequencer state,
bisect state, merge/squash message state, unresolved conflicts, and every state
that makes a normal one-parent commit ambiguous.

Linked worktrees and staged gitlinks/submodules are DEFERRED / REJECTED. They
add common-dir/index/ref or nested-repository semantics not required for the
minimum useful authority and need separately deterministic evidence.

Full worktree cleanliness is not required. Unrelated unstaged tracked files,
untracked files, and staged-and-unstaged changes to the same path are allowed:
Git records the exact authorized index. RAH must not intentionally alter those
worktree bytes and must not claim global worktree stability when external actors
may have changed it.

### Reviewed snapshot and public model input

The model must not self-authorize commit state merely by supplying observed HEAD
or index hashes. The host captures the staged snapshot, exact repository
identity, attached branch, expected parent, compound index identity, and staged
tree; explicitly authorizes that reviewed snapshot; and creates an opaque
internal authorization object.

Any handle is host-created, capability-scoped, repository-bound, snapshot-bound,
single-use or generation-bound, and not an authority source merely when echoed
by a model. Possessing arbitrary hash/token-looking strings creates no authority.

The narrow v1 public model-controlled input is commit message text only,
conceptually:

    { "message": "..." }

The model cannot choose repository, executable, CWD, branch, parent, index,
tree, author, committer, dates, hook path, Git config, environment, signing key,
message file, argv, remote, or ref. A host may provide a message override only
under the same validation policy.

Authorization is bound to one repository identity, attached branch, expected
parent, exact index/tree, and policy generation. It is short-lived/in-memory,
consumed by one attempt, not durable across restart, not reconstructed from
SQLite, and invalidated by relevant repository generation/context change.
Desktop may later show review or request/create authorization, but UI state is
not durable commit authority; restart must not restore old single-use authority.

### Fixed process, configuration, hooks, identity, and environment

Future implementation must use the exact native host-authorized Git executable,
exact canonical repository CWD, fixed command shape, minimized host environment,
no shell, and no arbitrary argv. Conceptually:

    git [host-fixed -c configuration] commit --no-verify --cleanup=verbatim
        -m <validated-message>

There is no pathspec, -a, patch/interactive mode, amend, allow-empty,
allow-empty-message, fixup, squash, signoff, signing, author/date/reset-author,
message file/template/reuse, trailer, or editor flow. repo.commit never stages
files and must never call git add or equivalent.

The message is exactly one valid UTF-8 value of at most 16 KiB. It rejects NUL,
empty, whitespace-only values, and an empty first line. Host policy owns further
newline/layout rules; --cleanup=verbatim permits exact comparison. No -F, -t,
-C, -c, reuse-message, fixup, squash, trailer, or signoff input is admitted.

System configuration is disabled; global/XDG configuration is disabled or
replaced with a host-controlled empty source; numbered environment configuration
is constructed only by the host; exact safe.directory if required, fixed
core.hooksPath, commit.gpgSign=false, user.useConfigOnly=true, explicit host
identity, and every other security-critical value are host-fixed. Repository
local config remains untrusted ambient input if Git reads it: security-sensitive
behavior must be overridden command-scoped or detected/rejected. Includes and
includeIf must not regain system/global authority. Model/provider input adds no
Git config.

Both --no-verify and command-scoped
core.hooksPath=<host-owned-empty-directory> are required. The empty directory is
host-created, canonical, empty, lifecycle-managed by host policy, not selected
by a model, and revalidated before execution. --no-verify alone is not the hook
security boundary because prepare-commit-msg and post-commit remain possible.
This neutralizes normal hook discovery but is not OS executable isolation.

Commit signing is forbidden. Fixed configuration sets commit.gpgSign=false, no
signing argv exists, and model input cannot select signing key/format/program,
SSH, X.509, or GPG. committed_verified proves no signature header.

Name and email are explicit host authority. Author and committer use the same
host-approved identity in v1. Repository/global identity, OS username/hostname
fallback, mailmap, and model values do not grant identity authority. If identity
is unavailable, preconditions fail with no guessing. Git supplies current
operation time; the model cannot choose author/committer date or timezone.
Deterministic commit OIDs are not required.

The child environment is cleared/minimized. It excludes GIT_DIR, GIT_WORK_TREE,
GIT_COMMON_DIR, GIT_INDEX_FILE, GIT_OBJECT_DIRECTORY,
GIT_ALTERNATE_OBJECT_DIRECTORIES, all inherited GIT_CONFIG_*, GIT_AUTHOR_*,
GIT_COMMITTER_*, editor/pager variables, GIT_SSH, GIT_SSH_COMMAND, GIT_ASKPASS,
SSH_ASKPASS, proxy/credential variables, HOME, and XDG_CONFIG_HOME.
GIT_TERMINAL_PROMPT=0 is fixed and inert editor behavior may be fixed defensively.
PATH is not inherited merely for convenience. Environment minimization is not an
OS sandbox.

This is offline local Git commit authority: it authorizes no fetch, pull, push,
remote lookup/modification, credentials, network Git protocol, SSH keys, HTTP
auth, or credential helpers. Neutralizing hooks, signing, editor, and helpers
prevents known Git-controlled external launch paths. RAH does not provide an
OS-level network sandbox; no network authority means no intentional network
operation, not that the Git process cannot make network system calls.

### Lease, one attempt, result taxonomy, and proof

repo.commit participates in the existing per-canonical-repository RAH mutation
lease. The lease covers snapshot revalidation, spawn, and postcondition
observation, serializing RAH stage, unstage, repo.patch, repo.create-file,
repo.edit-files, and repo.commit. It does not exclude external Git/process
actors. Exact snapshots, immediate revalidation, Git locking, post-observation,
and conservative uncertainty mitigate TOCTOU; they do not claim race freedom.

Exactly one mutating commit process spawn attempt is allowed per authorization.
There is no automatic retry, replay, amend, reset, compensating commit, ref
restoration, or rollback, including timeout, cancellation, or disconnect. A
later operation needs new host inspection and authorization. Git owns its lock
and ref transaction mechanism; RAH never deletes index/ref locks, removes
another process lock, forces a ref, or resets after failure.

The result taxonomy is:

- invalid_input: structurally invalid request/message before authority evaluation.
- precondition_failed: authorized repository/snapshot/state/identity no longer
  matches, or is unsupported, before the mutating attempt.
- known_no_effect: an attempt may have launched or been refused, but sufficient
  post-observation proves the authorized branch remains at old HEAD and no
  authorized durable commit-ref effect occurred. It never proves no unreachable
  object exists.
- committed_verified: all required postconditions prove exactly one authorized
  commit became current on the authorized branch.
- uncertain: RAH cannot prove either known_no_effect or committed_verified.

Exit status alone is never sufficient. Spawn failure before process creation may
be known_no_effect only with fresh proof. Nonzero exit, timeout, cancellation,
disconnect, post-spawn observer failure, and postcondition mismatch are
uncertain unless independent state proof establishes known_no_effect. Uncertain
operations are never replayed.

committed_verified proves same canonical identity and attached symbolic branch;
old HEAD as authorized parent; new HEAD/branch differs; new commit has exactly
one parent equal to old HEAD; tree equals the authorized write-tree OID; message
equals authorized normalized/verbatim message; author and committer conform to
host policy; no signature; index semantic state corresponds to the authorized
committed tree; and no intentional RAH worktree mutation. Raw index bytes need
not stay identical because Git may rewrite index/cache bytes.

“One commit” means one mutating Git process spawn attempt and exactly one new
reachable commit becomes the authorized branch tip with authorized parent/tree/
message/identity. It does not claim no unreachable commit/tree object, no
incidental metadata touch, or filesystem atomicity. Reflogs are authorized
incidental history metadata, not authority to edit/delete reflogs or update
another ref.

### Relationships and explicit exclusions

ADR 0009 is the host execute-process foundation; it supplies bounded process
mechanics, not generic commit authority. ADR 0010 remains index mutation only.
ADR 0011 keeps trusted-profile composition host-owned. ADR 0012 governs
repo.patch, ADR 0013 file creation, and ADR 0014 multi-file worktree editing.
ADR 0015 bounds a model-provider endpoint and is unrelated network authority.
No ADR is superseded.

Explicit exclusions are branch creation/switching, checkout/switch/detach,
arbitrary ref updates, amend, merge/merge commits, rebase, cherry-pick, revert,
reset, clean, stash, tag creation, reflog editing, generic commit-tree or
update-ref, history filtering/rewriting, force operations, automatic staging,
generic Git/shell authority, and all remote Git/credential actions.

The design is cross-platform. Implementation must have deterministic Windows
and Linux coverage; macOS remains conservative until separately validated.
Windows remains the certified live platform for the milestone. This ADR itself
makes no live-support claim.

## Consequences

RAH can complete inspect -> edit -> diff -> stage -> commit without generic Git
or shell authority. Exact reviewed state becomes durable history while the host
remains the authority source. A future Generic Tool Bridge may expose the
bounded message schema, enforce ordinary permission routing, and dispatch to
ToolRegistry; it must not invent authorization, broaden message to argv, or
retry uncertain calls.

The costs are new durable history/ref authority, Git config/hook/signing/identity
hardening, exact review-state binding, external-race/uncertainty handling,
reflog/object side effects, cross-platform deterministic burden, and no rollback
after uncertain effects.

This ADR does not provide OS sandboxing, network isolation, rollback, generic
shell safety outside the fixed Git command, generic Git execution authority,
arbitrary filesystem safety for external processes, protection from a malicious
privileged local actor, or transactional rollback across object/ref/reflog state.

## Alternatives rejected

### Reuse ADR 0010 stage authority

Rejected because index mutation does not authorize history/ref mutation.

### Generic git commit subprocess tool

Rejected because argv, config, hooks, signing, identity, and ref scope are broad.

### Generic shell.exec

Rejected because it is massive unrelated authority expansion.

### Model-provided expected HEAD/index hashes

Rejected because model observations are not host authorization.

### Completely clean worktree

Rejected because exact reviewed index, not global worktree cleanliness, is target.

### Only --no-verify

Rejected because prepare-commit-msg and post-commit can still run.

### Repository/global identity

Rejected because ambient configuration must not grant identity authority.

### Commit signing

Rejected for v1 because it adds executable/configuration surface.

### Automatic retry after timeout

Rejected because uncertain effects must not be replayed.

### Rollback via reset/update-ref

Rejected because it expands ref/history authority and cannot guarantee compensation.

## Open implementation details

Task 135 may choose exact private Rust type names, observer/helper layout,
command-config encoding, opaque authorization-token representation, bounded
error-field names, and host-owned empty-hooks-directory placement. These do not
reopen authority ownership, ref scope, hook/signing/identity policy, reviewed
snapshot boundary, retry/rollback policy, or result semantics.
