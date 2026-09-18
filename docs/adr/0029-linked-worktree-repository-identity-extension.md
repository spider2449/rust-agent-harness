# ADR 0029 — Linked Worktree Repository Identity Extension

Status: Accepted

Date: 2026-09-18

## Decision

**Authority classification: NARROW EXTENSION.** A linked worktree is a
distinct repository identity target within ADR 0027's existing repository
authority category. It adds no `PermissionLevel`, mutation authority, Tool
authority, HostExplicit category, model or provider routing authority, generic
Git authority, or generic filesystem authority.

ADR 0027 continues to define repository identity, membership, and the
zero-or-one-active authority composition. This ADR adds one closed supported
linked-worktree identity form within that model. It supersedes only the prior
support limitation that linked worktrees were unsupported for future
conformance. It does not supersede ADR 0027's authority model or make the
current production implementation support linked worktrees. Until a later
implementation and its required certification pass, production continues to
fail closed on linked worktrees.

The executable target is:

```text
one explicitly selected worktree root
  + that root's validated private Git identity
```

The common Git directory is relationship and shared-state evidence. It is not
an executable target, workspace member, generic Git handle, or authority for
sibling worktrees. In particular:

```text
same common Git directory != same RepositoryMemberId
```

and common Git identity cannot authorize access to a sibling worktree.

## Context

`RepositoryAdmissionIdentity::capture` currently supports only a top-level
real `.git` directory. A normal linked worktree instead has this shape:

```text
selected root/
  .git                 regular gitfile naming the private GIT_DIR

private GIT_DIR/
  commondir            relative relationship to the common Git directory
  gitdir               backlink to selected root/.git

common Git directory/
  worktrees/<id>/      private registration directory
```

The `.git` file, private Git directory, `commondir`, backlink, common
directory, and Git worktree registration describe one relationship. A Git
recognition result alone does not prove that relationship. Task 357 observed
that copied and fabricated gitfiles, submodules, and repositories created
with `--separate-git-dir` can be Git-recognized without being the supported
linked-worktree form. Current production therefore correctly fails closed.

Task 357, at exact-head CI run `35301502551` (PASS), reached Decision B,
**NARROW IDENTITY EXTENSION**. It found that linked worktrees require richer
identity and capability-specific shared-state currentness inside the
existing authority boundary. They do not require another authority category.
This conclusion formally supersedes Task 356's provisional
**NEW AUTHORITY CATEGORY** classification. Task 356 remains historical
roadmap evidence and is not rewritten.

## Authority composition and membership

The existing ADR 0027 cardinality remains exact:

```text
0 or 1 active repository members
```

A main worktree, linked worktree A, and linked worktree B may each be
explicitly admitted as separate inert members. Only one selected member may
supply current executable repository composition. Switching withdraws the
old composition before validating and freshly composing the next member.
There is no union `ToolRegistry`, parallel active repository, repository
selector in ordinary Tool input, model-selected worktree, or provider-selected
worktree.

Human selection and admission are independent for every worktree. Git's
worktree list is validation evidence only; it never discovers, adds, or
activates workspace members. Model output, Tool input, provider metadata,
repository content, and remembered candidates cannot select or admit a
worktree.

Membership relation is classified as follows:

- **Same:** the canonical root or root filesystem object is the same, or the
  validated private worktree target identity is the same. Reject duplicate
  admission.
- **Nested:** canonical selected roots have a parent/child relationship.
  Reject co-membership under ADR 0027.
- **Distinct:** canonical roots differ and private Git target identities
  differ. This includes main, linked A, and linked B even when all share one
  common Git directory.

Common Git directory equality alone is never `Same` and never a reason to
reject otherwise valid distinct members.

## Supported main-worktree identity

The existing ordinary form remains supported without regression. Admission
requires all of the following:

1. The selected root exists, is canonical, and is a real non-reparse working
   tree directory.
2. The root filesystem identity and all existing ancestor, alias, nested
   repository, and Windows path checks pass.
3. Top-level `.git` is a real non-reparse directory.
4. Fixed Git semantic probes report the selected top-level root, a non-bare
   ordinary worktree, and private gitdir equal to common gitdir equal to the
   root's `.git` directory.
5. The selected Git executable identity is host-owned and current.

Existing supported main-worktree admission, observation, mutation, Stage,
Unstage, and reviewed Commit behavior must be preserved by implementation.

## Supported linked-worktree identity

ADR 0029 authorizes exactly one additional `.git` class: a registered,
standard linked worktree whose complete host filesystem identity and fixed
Git semantic evidence agree. All requirements below are conjunctive. Any
missing, malformed, ambiguous, inaccessible, stale, or disagreeing evidence
fails closed and publishes no member.

1. The selected root exists and is a canonical, non-reparse working-tree
   directory. Existing ancestor, nested-root, alias, and repository-boundary
   checks pass.
2. Root `.git` is one non-reparse regular file, not a directory, link,
   reparse point, or other special object.
3. The `.git` bytes are bounded and strictly parsed as exactly one supported
   Git `gitdir:` record. No extra record, redirection form, malformed bytes,
   or unbounded content is accepted.
4. The parsed gitdir target resolves to the same canonical private gitdir
   reported by fixed Git semantic probes.
5. The private gitdir exists as a non-reparse directory and is an immediate
   registered child of `<common-git-dir>/worktrees/`.
6. The private `commondir` file is a bounded, strictly parsed relative
   relationship resolving exactly to Git's reported common directory. The
   accepted v0.30 relationship is the standard `../..` form from
   `<common-git-dir>/worktrees/<id>`.
7. The private `gitdir` backlink resolves exactly to the selected root's
   `.git` file. A copied file pointing back to another root is rejected.
8. Fixed Git semantic probes report the same selected root/private/common
   relationship. `git worktree list --porcelain -z` contains exactly one
   matching registration for that canonical selected root and relationship.
9. The candidate is non-bare, registered, not prunable or stale, not a
   submodule, not `--separate-git-dir`, and has no unsupported nested
   repository relation.
10. The root, `.git` file, private gitdir, common gitdir, common `worktrees`
    parent, registration entry, and every relevant path component satisfy
    the existing symlink, reparse, filesystem identity, alias, and Windows
    ambiguity checks.
11. The Git executable path and filesystem identity are host-selected and
    current. Git probes use fixed executable and arguments, selected-root
    working directory, controlled environment, bounded output, timeout, and
    owned process lifecycle.
12. Admission repeats identity and relationship validation before atomically
    publishing the inert member. A failed capture publishes nothing and has
    no activation, provider/runtime spawn, or filesystem/Git mutation side
    effect.

A present and coherent locked worktree may be admitted. Lock status alone is
not authority and does not grant or revoke a target. Missing roots,
inaccessible or inconsistent registrations, and stale or prunable records
are rejected. RAH never unlocks or prunes a worktree.

## Rejected repository forms

Git recognition alone is insufficient:

```text
Git recognizes .git file -> admit
```

is rejected. Admission requires both Git semantic classification and host
filesystem identity plus the complete closed relationship above.

The following remain unsupported:

- **Copied or fabricated gitfiles:** Git may resolve them, but the backlink,
  private/common relationship, or unique registration does not prove the
  candidate root. A manually fabricated file pointing at an ordinary `.git`
  directory is not a linked worktree.
- **Submodules:** modern submodules can have `.git` files, often with metadata
  below a superproject `modules` hierarchy, and old-form submodules may have
  `.git` directories. Reject through complete superproject, gitlink, nested
  repository, private/common, backlink, and registration classification,
  including fixed superproject-root evidence such as
  `git rev-parse --show-superproject-working-tree` and host-side gitlink
  checks where applicable. File versus directory shape alone is insufficient.
- **`--separate-git-dir`:** an external Git directory is not accepted unless
  it has the exact private registration, standard commondir, backlink, and
  common/worktrees relationship; the separate-git-dir form does not.
- **Bare repositories:** without a selected worktree root, there is no
  repository target. RAH does not synthesize a worktree.
- **Other indirection:** arbitrary, oversized, malformed, multiline,
  inaccessible, reparse-mediated, or otherwise unsupported gitfiles fail
  closed.

No additional Git-recognized gitfile form is accepted by this decision.

## Stable admission identity and mutable currentness

Stable admission identity for either accepted form retains privately:

- canonical selected root and root filesystem identity;
- selected Git executable canonical path and filesystem identity; and
- supported worktree classification.

For a main worktree, it also retains the `.git` directory filesystem
identity. For a linked worktree, it retains:

- `.git` file filesystem identity and bounded exact contents, digest, or
  equivalent content-currentness evidence;
- canonical private gitdir and its filesystem identity;
- canonical common gitdir and its filesystem identity;
- common `worktrees` parent identity and registration directory identity;
- the private `gitdir` backlink and `commondir` content/identity relations;
- the exact validated registration for the selected root; and
- the validated private-to-common relationship.

All raw evidence and identifiers remain host-private. Filesystem object
identity is required in addition to path spelling and canonicalization.

Stable admission identity must not be overbound to ordinary mutable Git
state. These are not immutable admission identities:

- HEAD object ID;
- branch ref object ID;
- index contents or index file object identity;
- ordinary shared ref values; or
- object-store contents.

They are capability-specific currentness inputs when an operation depends on
them. Normal Git activity must not force needless fresh admission when the
selected root and validated private/common/registration relationship remain
the same.

The selected root/private/common relationship is part of authority identity.
Revalidate it at admission, before activation/composition, and before
publication wherever ADR 0027 requires fresh identity validation. If an
external process moves or removes a worktree, prunes or repairs its
registration, replaces its gitfile/private/common directory, changes a
backlink or commondir, or otherwise changes the relation, the old identity
becomes stale as appropriate. RAH never repairs or silently migrates an
admission. A changed stable identity requires fresh explicit admission.

## Common Git state and operation-specific currentness

The common directory can contain the shared object database, ordinary shared
refs, and shared configuration as defined by Git. Not all refs are shared.
HEAD, index, worktree-specific pseudorefs, and documented worktree-specific
namespaces such as `refs/bisect`, `refs/worktree`, and `refs/rewritten` have
per-worktree semantics. Future code should use Git semantic path resolution
where practical instead of hand-constructing metadata paths. RAH does not
use sibling worktree metadata paths as authority.

Do not treat every change to a common Git directory as invalidating every
sibling preparation. Each capability captures only the exact Git semantic
facts it uses:

- **Stage and Unstage** depend on the selected member/worktree identity, its
  current index snapshot, selected HEAD when required, and relevant selected
  target facts. An unrelated sibling branch update does not automatically
  stale Stage or Unstage.
- **Reviewed Commit** depends on the selected worktree identity, exact
  selected index snapshot, selected HEAD, attached branch, expected selected
  branch ref OID, and the existing Commit policy/currentness. A Commit on an
  unrelated sibling branch does not automatically invalidate the review. A
  change to the selected branch's expected OID does; the old review fails
  stale and is never rebound.
- **Observation and diff** capture only the Git facts required by that
  observation.

No global common-directory generation is authorized merely for convenience.
RAH does not own all Git clients and introduces no process-wide or
machine-wide shared Git lock. External Git remains possible. RAH uses exact
currentness checks and stale/precondition failure, and preserves uncertain
effects under the existing rules. It makes no claim of race-free concurrency
with external Git.

## Existing operation authority remains unchanged

ADR 0029 changes repository identity resolution and currentness only. It does
not broaden existing effect classes or authority.

### Worktree content

ADR 0012 and subsequent bounded mutation ADRs remain authoritative. File
operations target only the selected active worktree root. They cannot target
a sibling worktree root, private gitdir, common gitdir, or `.git` metadata.
There is no cross-worktree Tool and no worktree selector in Tool input.

### Stage and Unstage

Stage and Unstage retain their separate existing index authority. For a
linked worktree, the target is only that selected active worktree's
Git-resolved current index. It is never the main index by assumption, a
sibling index, or a synthetic common index. Stage A and Unstage A must leave
main and B indexes unaffected. No replay or cross-worktree recovery is
authorized.

### Reviewed Commit

ADR 0016 remains the sole reviewed Commit authority. A linked-worktree
Commit binds the selected worktree identity and private/common relation, the
selected index snapshot, selected HEAD, existing attached-branch requirement,
expected selected branch ref/OID, repository/composition currentness, and Git
executable identity. It uses neither a sibling index nor sibling HEAD, cannot
retarget another member, and cannot silently migrate authorization. It may
create shared Git objects and advance the selected ordinary shared branch ref
under the existing reviewed contract; it does not change a sibling's private
HEAD or index. A detached-HEAD linked worktree may be admitted and used for
existing supported read/edit/index operations, subject to each operation's
policy. Detached HEAD remains unsupported for Commit wherever existing
policy requires an attached branch. This ADR does not broaden Commit
semantics.

### Branch creation

Existing local branch creation authority remains separate. Any future
linked-aware implementation must use the selected identity and exact shared
or current ref preconditions. ADR 0029 authorizes no checkout, switch,
`worktree add`, `worktree remove`, or `worktree prune`.

## Active lifecycle, Close, and inactive removal

Before active composition publication, the host revalidates stable worktree
identity and registration, verifies the selected current member, and checks
ordinary ADR 0027 generations/currentness. A switch follows:

```text
A active
  -> withdraw A executable composition
  -> validate B
  -> build fresh B composition
  -> publish B
```

No sibling registry is retained. Failed validation publishes no active
composition.

ADR 0027's one-active lifecycle applies equally to a linked member. Close
withdraws active executable authority and retains the member inert. It does
not call `git worktree remove`, prune, or otherwise change registration,
files, refs, or index. Removing an inactive member changes only process-local
membership; it does not delete the linked working directory, remove or prune
the Git registration, or modify common metadata. Later re-admission is fresh.

## HostExplicit, permissions, and providers

The exact HostExplicit production set remains 11:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
repo.delete-file
repo.rename-file
```

Linked-worktree support changes only repository target identity and
currentness. It adds no HostExplicit #12, eligibility category, permission,
Tool input field, worktree selector, sibling dispatch, or provider dispatch.
Model and provider routing boundaries remain unchanged.

## Persistence and privacy

ADR 0028 remains fully authoritative. Restart restores exactly zero admitted
members and zero active repository authority. A remembered linked-worktree
candidate may retain only the existing descriptive information permitted by
ADR 0028. Do not persist private/common gitdir paths, worktree registration
ID, gitfile contents, filesystem identities, `RepositoryMemberId`, active
state, `ToolRegistry`, Commit authorization, Stage/Unstage selector,
HostExplicit ticket, preparation, or currentness snapshot. Fresh explicit
admission rediscovers and revalidates all authority evidence; no persistence
schema change is authorized.

Linked-worktree identity details remain host-private, including canonical
roots, `.git` bytes, private/common paths, filesystem IDs, registration
directories, backlink and commondir data, and ref/currentness snapshots. Do
not expose them automatically to the model, Tool input, MCP, Process Plugin,
generic Activity, Effective Authority DTO, persistence, or unbounded error
messages. Existing user-approved path presentation is presentation, not
authority.

## Native Git, admission atomicity, and platform checks

Identity classification may use only fixed, host-owned, read-only Git
commands with fixed argv and bounded output/time. Examples include
`git rev-parse` (including fixed top-level, gitdir, common-dir, Git-path,
bare, and superproject probes), `git symbolic-ref`, and `git worktree list
--porcelain -z`. No shell, model-selected Git argument, generic Git authority,
or mutating worktree command is allowed during admission.

Conceptual admission is atomic:

```text
human selects root
  -> canonicalize and capture root
  -> classify supported .git form
  -> capture private evidence
  -> fixed Git semantic probes
  -> validate private/common/backlink/registration
  -> reject unsupported, submodule, separate-git-dir, nested, and reparse cases
  -> duplicate/relation check
  -> final revalidation
  -> atomically publish inert member
```

Failure publishes nothing and has no activation side effect.

Preserve existing Windows fail-closed rules. Reject unsupported symlinks,
junctions/reparse points, device or verbatim ambiguity, alternate data stream
ambiguity, and alias/canonical conflicts across the root, `.git`, private
gitdir, common gitdir, `worktrees` parent, registration entry, and relevant
ancestors. No race-free TOCTOU claim is made.

Timeout, cancellation, disconnect, process failure, or a lost response does
not imply rollback. Do not retry, replay, or compensate uncertain external
effects. Linked-worktree support creates no cross-worktree rollback.

## Required future conformance tests

Implementation must add deterministic tests at the later implementation
gate. This ADR itself adds no tests. The future tests must cover at least:

- **Identity:** ordinary main, linked A and B, root aliases, same-private
  duplicate, same-common/different-private allowed, copied and fabricated
  gitfiles, malformed gitfile, submodule, separate-git-dir, bare, nested, and
  reparse variants.
- **Staleness:** changed or replaced gitfile; replaced private or common
  gitdir; changed backlink or commondir; removed/pruned registration; and
  externally moved linked worktree.
- **Capability isolation:** independent A/B HEAD and index; Stage and Unstage
  A affect only A; file mutation affects only the active root; Commit A uses
  A's index and HEAD; unrelated B branch Commit does not automatically stale
  A; and a selected A branch ref change stales A's review.
- **Lifecycle:** switching, Close, inactive removal, restart with zero
  authority, and remembered-candidate fresh re-admission.

Concurrency/currentness tests use deterministic coordination and do not rely
on sleeps.

## Windows live gate

Linked-worktree production support is release-blocked until the later
host-driven Windows live gate emits:

```text
RAH_V030_LINKED_WORKTREE_LIVE_OK
```

The disposable fixture contains `main`, `linked-a`, and `linked-b`. The gate
must prove complete closed admission, distinct member IDs despite a shared
common directory, one active repository, switching, Close, inactive removal,
restart behavior, selected-root file effects, selected-index Stage/Unstage,
and selected-worktree reviewed Commit. It must prove stale expected branch
OID rejection and rejection of duplicate/alias, copied/fabricated/malformed
gitfile, submodule, separate-dir, nested, bare, and reparse cases. It must
show zero worktree add/remove/prune/move/repair/lock/unlock, zero network Git,
and no provider/model repository selection.

Host-driven Windows evidence is acceptable. The gate does not imply GUI
automation, model-selected Tool dispatch, or cross-platform certification
unless those are separately executed.

## Relationship to existing decisions and explicit exclusions

ADR 0027 remains authoritative for workspace membership, repository
identity/authority composition, zero-or-one active repository, active-only
composition, restart semantics, and provider boundaries. ADR 0029 is an
additive clarification and extension of supported repository identity forms.
ADR 0027's statement that linked worktrees were unsupported remains
historically true of the v0.26 foundation; this ADR authorizes future
conformance work to add linked support. It does not change that foundation's
historical record or any ADR 0027 authority rule.

The existing mutation decisions remain authoritative: ADR 0012 for bounded
worktree content mutation; ADR 0016 for reviewed Commit; ADR 0019 for
ordinary directory creation authority; ADR 0020 for local branch creation;
and ADRs 0021–0026 for HostExplicit framework and capability-specific routes.
ADR 0029 changes target identity resolution/currentness only and does not
broaden their effects.

This ADR explicitly does not authorize arbitrary gitfiles, submodules,
separate-git-dir, bare repositories, automatic sibling discovery, worktree
creation/removal/prune/repair/move/lock/unlock, checkout/switch, multiple
active repositories, a union ToolRegistry, cross-worktree Tool calls,
cross-worktree Stage or Commit, network Git, persisted executable membership,
generic shell/process/filesystem authority, HostExplicit `repo.create-directory`,
network MCP, PluginManager, or profile hot reload.

## Implementation sequencing

The next implementation task is **Task 359 — Linked Worktree Identity and
Repository Policy Foundation**. It should proceed in bounded layers:

1. Build a reusable worktree-aware identity resolver.
2. Preserve ordinary-main compatibility.
3. Capture and revalidate linked identity and registration.
4. Add membership duplicate and relation support.
5. Adapt repository construction and path resolution.
6. Conform observation, index, and Commit policies to capability-specific
   currentness.
7. Add deterministic fixtures and tests for the accepted matrix.

Existing picker/admission frontend surfaces should need minimal or no
structural change if backend acceptance is sufficient. Any necessary frontend
change must remain explicit and narrow. Task 359 must not combine every later
Desktop flow, security audit, Windows live certification, or release phase
into an uncontrolled patch.
