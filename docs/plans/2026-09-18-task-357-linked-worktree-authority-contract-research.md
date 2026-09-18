# Task 357 — Linked Worktree Identity and Authority Contract Research

Status: research complete; no production implementation authorized

Parent checkpoint: 9111bb40a914ce6a723e76c2f2aa5ee831813751

Task 356 exact-head CI: run 35298955522, PASS (the preceding roadmap checkpoint)

Current released baseline: RAH v0.29.0

## Decision

**Decision B — NARROW IDENTITY EXTENSION.**

Linked worktrees are an additional supported form of the existing
host-owned repository identity in ADR 0027. They do not create a new
PermissionLevel, mutation capability, active-member model, or authority
category. ADR 0027 already says that repository identity scopes authority,
that identity may include the supported top-level .git form and ordinary
worktree classification, that identity is private host evidence, and that
capability-specific target identity may be included where needed. It also
already assigns one active composition to one selected repository identity.
Its current linked-worktree rejection is a support limit, not a second
authority model.

The new identity form must bind one selected worktree root to its own private
Git directory and a validated relation from that private directory to the
common Git directory. The common directory is relationship and shared-state
evidence. It is never a target handle. Distinct roots with distinct private
directories remain distinct members even when their common directory is
identical.

Task 356's “NEW AUTHORITY CATEGORY” label is not supported by this research
and is not retained. The existing host-selected member, one-active rule,
active-only composition, capability-specific policies, and existing effect
authorities express the complete boundary. The needed change is identity
coverage and operation-specific shared-state currentness inside that boundary.

**Task 358 recommendation:** reframe as
**Linked Worktree Repository-Identity ADR/Conformance Decision**. An additive
ADR 0029 remains justified to state the closed linked-worktree identity form
and shared-state currentness explicitly, but it must characterize them as an
ADR 0027 identity clarification/conformance rule, not a new authority
category. Task 358 should accept that exact classification before any
implementation task begins.

## Scope and evidence

This task reviewed ADR 0027, ADRs 0012 and 0016, the Task 356 roadmap,
repository admission and activation source, the Git Stage/Unstage policies,
the reviewed Commit policy, branch creation, repository observation, and the
official Git manuals linked below. No Rust implementation, frontend work,
tests, permission change, persistence schema change, dependency change, or ADR
edit was made.

The only repository changes are this plan and the README v0.29 release-status
correction. The README correction is independent of the identity decision.

### Existing RAH identity boundary

At this checkpoint, RepositoryAdmissionIdentity retains the canonical root,
root filesystem identity, top-level .git filesystem identity, selected Git
executable path, and executable filesystem identity. Capture rejects a root
whose .git is not a directory. RepositoryAdmissionRelation returns Same for
the same root object or same top-level .git object, Nested for canonical
parent/child roots, and Distinct otherwise. It does not capture a private
gitdir or common-dir relationship.

Admission constructs and validates a temporary repository, revalidates the
captured identity, and only then publishes an inert member under membership
coordination. Activation revalidates the retained member identity before
building a fresh repository composition and again before publication. This
already provides the correct lifecycle places to validate the richer identity:
complete admission before inert-member publication, then complete fresh
identity revalidation both before active composition construction and before
composition publication.

The existing source has ordinary-.git assumptions in repository policies.
Reviewed Commit explicitly rejects linked worktrees and reads root/.git/index;
branch creation requires a real .git directory; Git index policies retain
root/.git identity and currently snapshot the complete refs list. These are
implementation gaps for later authorized work. They do not require a change
to the authority classification.

### Official Git model

Research used the official manuals:

- [git-worktree](https://git-scm.com/docs/git-worktree)
- [gitrepository-layout](https://git-scm.com/docs/gitrepository-layout)
- [git-rev-parse](https://git-scm.com/docs/git-rev-parse)
- [gitsubmodules](https://git-scm.com/docs/gitsubmodules)
- [git-init](https://git-scm.com/docs/git-init)

Git documents one main worktree for a non-bare repository and zero or more
linked worktrees. A linked root has a plain-text .git file resolving to a
private per-worktree gitdir. A private gitdir has a commondir file resolving
to shared Git state and a gitdir backlink to the selected root's .git file.
The standard registration is common-git-dir/worktrees/<id>. Git maintains
that registration and can report root, HEAD, branch or detached state,
locked state, and prunable state through worktree list --porcelain.

Git's own layout and rev-parse documentation warn against assuming whether a
path belongs to GIT_DIR or GIT_COMMON_DIR. The future RAH policies must use
fixed Git path-resolution commands and verify their answers against host
filesystem identity and the closed relationship below.

Shared state includes the common object database and ordinary refs such as
refs/heads and refs/tags. HEAD and index are per-worktree. Git also specifies
that pseudo-refs are generally per-worktree, while refs/bisect,
refs/worktree, and refs/rewritten are per-worktree exceptions among refs/*
namespaces. Therefore “all refs are shared” is false. Worktree-specific
access paths exist for other worktrees' per-worktree refs, but RAH must not
use them to select or access sibling authority.

### Windows Git fixture

The fixture used installed native Git 2.54.0.windows.1 and a disposable
directory under the Windows temporary directory, outside the RAH checkout.
It contained:

- main, initialized on branch main;
- linked-a, added with branch worktree-a;
- linked-b, added with branch worktree-b;
- a committed tracked file used for independent index checks; and
- a real file-protocol submodule used to characterize submodule gitfiles.

No fixture path or fixture state was committed. The fixture was removed after
the observations. A second minimal fixture confirmed the main .git directory
and linked .git regular-file forms and the absolute HEAD/index paths. It too
was removed. No fixture remains in the RAH tree.

The requested commands were run from each selected root:

- git rev-parse --show-toplevel
- git rev-parse --absolute-git-dir
- git rev-parse --path-format=absolute --git-common-dir
- git rev-parse --git-path HEAD
- git rev-parse --git-path index
- git symbolic-ref -q HEAD
- git rev-parse HEAD
- git worktree list --porcelain

For the path comparison, the follow-up run used --path-format=absolute with
--git-path HEAD and --git-path index. The normalized observations were:

The porcelain worktree list was also run with cwd set to main, linked-a, and
linked-b in a separate clean fixture. Each invocation returned three worktree
records and contained all three exact roots; the registry is common Git
metadata, not a per-root list.

| Worktree | .git form | private GIT_DIR | common Git directory | HEAD and index | symbolic HEAD |
| --- | --- | --- | --- | --- | --- |
| main | directory | main/.git | main/.git | main/.git/HEAD and main/.git/index | refs/heads/main |
| linked-a | regular file | main/.git/worktrees/linked-a | main/.git | private gitdir/HEAD and private gitdir/index | refs/heads/worktree-a |
| linked-b | regular file | main/.git/worktrees/linked-b | main/.git | private gitdir/HEAD and private gitdir/index | refs/heads/worktree-b |

In both linked roots the .git file named its private gitdir. The private
gitdir's gitdir file pointed back to that root's .git file; its commondir
contents were ../.. and resolved to main/.git. The linked HEAD, index, and
refs/heads/main paths reported by Git confirmed the private/common split.
The worktree list contained exactly main, linked-a, and linked-b, with one
record per root. All three initial HEAD OIDs happened to be equal because
each branch began at the same seed commit; their symbolic branch targets and
HEAD paths were different.

The main result confirmed the expected ordinary form:

- root/.git is a directory;
- Git's absolute gitdir resolves to root/.git; and
- Git's common directory resolves to the same directory.

The linked result confirmed:

- root/.git is a regular file;
- private GIT_DIR differs for A and B;
- common Git directory is equal for main, A, and B;
- HEAD and index paths are private per worktree; and
- a normal branch ref path is shared under the common directory.

A further fixed rev-parse --path-format=absolute --git-path probe from each
root confirmed that A and B resolve objects, packed-refs, config, and
refs/heads/main to the same common paths as main. A and B resolve HEAD,
index, config.worktree, refs/bisect, refs/worktree, and refs/rewritten to
their own private paths. Main resolves its own HEAD/index and per-worktree
configuration/ref paths under main/.git because for main the private and
common directory are the same. These path observations match Git's documented
sharing exceptions; they are not inferred from all refs being shared.

### Git effects observed

The fixture was clean before each experiment.

1. A's tracked file was modified and staged with native Git. SHA-256 of A's
   index changed. The main and B index hashes did not change. Git's stage-zero
   entry in A named the new object.
2. The file was unstaged in A using native Git. A's index hash changed again
   and its entry returned to the HEAD object. Main and B index hashes remained
   equal to their staged-experiment values. The worktree file was then restored
   in the disposable fixture only.
3. A's file was staged and committed on A. refs/heads/worktree-a advanced and
   A's HEAD resolved to that new OID. Main's HEAD and B's HEAD remained at their
   own values. B's index hash was unchanged. This proves that the object/ref
   effect is common while the selected worktree HEAD/index are private.
4. A separate synthetic descendant commit object was created in the fixture,
   then a compare-and-swap update-ref advanced only refs/heads/worktree-a from
   its recorded expected OID. The old expected OID no longer matched the
   selected branch's current value. A prepared operation bound to the old
   branch OID must fail stale; it must not rebind to the new OID.
5. An empty commit on B advanced worktree-b. A's HEAD, A's branch ref, and A's
   index stayed unchanged. This demonstrates why unrelated object creation or
   an unrelated sibling branch update must not invalidate all sibling
   preparations.
6. Locking B caused worktree list --porcelain to report its locked state while
   the root and registration remained present and coherent. Git worktree move
   changed the selected root and backlink relationship; after moving it back,
   Git again reported the original path. RAH itself must not move, lock,
   unlock, repair, remove, or prune worktrees.

### Gitfile discrimination observations

Git successfully resolved both a copied linked-A .git file at a second root
and a manually written .git file pointing at main/.git. In both cases
rev-parse --show-toplevel returned the candidate root even though Git's
worktree registration did not list that candidate. The copied file also
retained a private backlink to linked-a/.git, not the copied root. This is
direct evidence that Git recognition or rev-parse output alone is not enough
to admit a linked member.

A malformed gitfile failed rev-parse with exit 128 and an invalid-gitfile
diagnostic. A submodule's .git file pointed below the superproject common
directory's modules tree, not common/worktrees/<id>; its private and common
Git directories were the same; and --show-superproject-working-tree reported
the containing main root. Its git directory did not register as an independent
linked worktree. The submodule was also physically nested inside main.

git init --separate-git-dir created a Git-recognized .git file, but its
resolved private and common directories were the same external directory;
there was no linked-worktree commondir/backlink pair or matching registration
under the main repository's worktrees directory. Git's worktree list for
main did not include this separate-git-dir repository.

These observations support a closed classification rather than “any
Git-recognized gitfile.” A modern submodule, a separate-git-dir repository,
the copied root, and the manually fabricated file all fail the exact private
directory, backlink, common-dir, and registration relationship required of a
linked worktree. Old-form submodules can have a .git directory, so exclusion
must also check superproject/nested-root evidence rather than treating the
file-vs-directory distinction as a complete submodule test.

## Admission classification contract

Only the following two non-bare forms may be admitted.

### Main ordinary worktree

Preserve the v0.29 supported form:

1. Selected root is an existing canonical ordinary working-tree directory.
2. Top-level .git is a real directory, not a link or reparse point.
3. Fixed Git probes report the selected root as top-level, non-bare, and a
   registered main worktree.
4. Git's private gitdir and common directory canonicalize to root/.git and
   that directory's filesystem identity.
5. The root and every relevant ancestor pass the existing Windows reparse,
   alias, and repository-boundary checks.

No Task 357 finding requires changing this baseline form.

### Linked worktree

Only admit when all of these facts agree in one fresh host capture:

1. The selected root is an existing canonical ordinary working tree. Its
   top-level .git entry is one non-reparse regular file.
2. Its bounded, strict single-record gitfile has the Git gitdir form. The
   parsed target canonicalizes to Git's absolute private gitdir.
3. The private gitdir is an existing non-reparse directory whose canonical
   parent is the canonical common/worktrees directory. It is an immediate
   registered child; the child basename is only descriptive and contributes
   no authority by itself.
4. The private directory's commondir file is a bounded, strict relative
   relationship which resolves to Git's reported common directory. The
   v0.30 narrow form is the standard commondir value ../.. from
   common/worktrees/<id>.
5. The private directory's gitdir backlink resolves to the selected
   root/.git file, not a copied root, alias, or sibling .git file.
6. Git's fixed semantic classification and worktree list --porcelain -z show
   one registration for exactly the selected canonical root, with no bare or
   prunable state. The listed root, Git-reported private/common directories,
   and filesystem relation must agree.
7. Root, .git file, private gitdir, common gitdir, worktrees parent, and
   registration directory are captured with filesystem identities and are
   checked for symlink/reparse components.
8. No superproject identifies the candidate as a submodule, no candidate is
   physically nested under an admitted member, and the candidate has no
   unsupported nested repository relationship.
9. The selected Git executable identity is captured as in v0.29, and the
   semantic classification is performed with fixed native Git executable,
   argv, cwd, bounded output, timeout, and controlled environment.

If any source disagrees, is inaccessible, is malformed, or changes during
capture, admission fails and publishes no member. Git output is one proof
input; it is not filesystem identity proof.

### Rejected Gitfile forms

- Submodule: reject when the candidate is nested under another repository,
  --show-superproject-working-tree identifies a containing superproject, or
  host Git tree evidence identifies the path as a gitlink/submodule. Standard
  submodule private metadata also resolves below modules, not the exact
  common/worktrees registration form. A .git file alone is not the
  discriminator. Old-form submodules are rejected through superproject,
  gitlink, and nested-root checks even when their .git entry is a directory.
- Separate-git-dir: reject because its gitfile target is not the immediate
  registered common/worktrees/<id> private gitdir with the expected backlink
  and commondir relationship.
- Copied gitfile: reject because the registered path/backlink still names the
  original root. A successful Git top-level probe does not repair that
  mismatch.
- Fabricated file pointing at an ordinary .git directory: reject because
  private equals common and there is no exact linked-worktree registration.
- Malformed, oversized, multiline, redirected, inaccessible, or otherwise
  unsupported gitfile: reject before member publication.
- Bare repository: reject because no selected working-tree root exists; do
  not synthesize one.

No additional arbitrary Git-recognized gitfile form is accepted in v0.30.

## Stable identity and mutable currentness

### Stable admission identity

The private identity binding for either accepted member form contains:

- canonical selected worktree root and root filesystem identity;
- main-directory or linked-file .git form;
- top-level .git object filesystem identity;
- for linked members, bounded exact gitfile bytes at capture, their SHA-256,
  and the canonical parsed private-gitdir target;
- private gitdir canonical path and filesystem identity;
- common gitdir canonical path and filesystem identity;
- private/common relation, including canonical common/worktrees parent
  relation;
- for linked members, gitdir backlink and commondir file identities, bounded
  content SHA-256 values, and canonical resolved target values;
- registration directory identity and Git semantic evidence binding its
  entry to exactly the selected root;
- main-vs-linked classification; and
- selected Git executable canonical path and filesystem identity.

Windows FileIdentity should use volume serial plus file index for each
directory or file object, alongside canonical path and form. A directory name,
worktree ID, .git file path, or Git-reported path by itself is not sufficient.
The <id> part of common/worktrees/<id> is descriptive only.

Relationship files require both filesystem identity and content currentness.
For .git and the private gitdir backlink, read at most 64 KiB of exact bytes;
for commondir, read at most 256 bytes. Require strict UTF-8/ASCII-compatible
single-line syntax, no NUL, no extra records, and only the supported line
ending. Parse and retain only the hash and canonical resolved target, not the
raw content. Reopen without following links/reparse points and check the
filesystem identity before and after the bounded read. Re-capture the exact
facts during revalidation. This catches replacement and in-place content edits
to a retained inode. It does not claim race-free TOCTOU.

Git's worktree list output is bounded and parsed in porcelain NUL mode. Match
the selected canonical root and expected semantic record to the host-owned
filesystem proof. Do not persist the private/common paths, worktree ID,
identities, or parsed relationship.

### Mutable currentness

These facts are snapshots or per-operation dependencies, not immutable
admission identity:

- selected worktree HEAD symbolic target and resolved OID;
- selected worktree index object identity, raw digest, semantic entries, and
  tree as required by the capability;
- target file, parent, and worktree preimage facts used by a prepared edit;
- exact selected branch/ref OID and checked absence/presence of a named ref;
- selected worktree registration state and locked/prunable status;
- common objects and exact shared ref values used by an operation;
- operation-specific local configuration values that affect its semantics;
- active member, repository, composition, selector, observation, and
  capability generations.

Index is deliberately mutable currentness, never stable admission identity.
Git legitimately replaces the index object while staging and committing.
Capture its current object identity/content at a capability boundary, and
revalidate the exact semantic/raw facts the operation relies on immediately
before use. Never put a common-directory mtime or “any common-dir change”
token into admission identity or operation currentness.

An in-place change to the gitfile, gitdir backlink, or commondir content is
identity-relevant because it changes the relationship even if the inode is
unchanged. Replacing the root, relationship file, private gitdir, common
gitdir, or registration makes the retained member stale. Fresh explicit
admission is required; a retained member ID cannot refresh its own identity.

### Shared Git state is not a member relation

Freeze both statements:

- Same common gitdir does not mean Same member.
- A common gitdir cannot authorize another sibling worktree.

Main, A, and B can be three separately admitted members with one common
directory. The common identity is used only to prove the private/common
relationship, recognize shared refs/objects relevant to a selected
capability, detect a malformed or replaced relation, and revalidate exact
operation dependencies. It never creates a generic common-repository handle
or lets an A Tool target B.

## Membership and lifecycle rules

RepositoryAdmissionRelation should preserve its three member-comparison
outcomes:

- Same: equal canonical root/root filesystem identity, or equal validated
  private gitdir identity for a fully validated alternate spelling of the
  same worktree.
- Nested: one canonical worktree root is physically beneath the other.
- Distinct: separate valid roots and private gitdirs, whether their common
  gitdir is shared or not.

Do not return Same for equal common gitdirs. Keep the common relationship as
separate private evidence/helper data. Run complete classification before
duplicate comparison, so a copied .git file cannot become a member merely
because its private target matches an existing member.

| Candidate relation | Result |
| --- | --- |
| Same root through case, drive-letter, or supported canonical alias | Same; reject as already a member |
| Same private gitdir reached through alternate root spelling | Same; reject as already a member |
| Main plus linked worktree sharing common state | Distinct; both can be admitted |
| Linked A plus linked B sharing common state | Distinct; both can be admitted |
| Different linked roots, different private gitdirs, same common gitdir | Distinct; both can be admitted after complete validation |
| Candidate physically nested beneath another admitted worktree root | Nested; reject co-membership |
| Ordinary nested repository | Nested; reject co-membership; path capability still rejects nested .git boundaries |
| Copied root/gitfile targeting an existing private gitdir | Invalid classification; reject before relation comparison |

Distinct linked sibling roots are allowed. A linked root nested under another
admitted root remains rejected, preserving ADR 0027's nested co-membership and
per-path nested-.git protections. Git worktree placement does not justify
weakening path ancestry or turning a parent into authority over nested
repositories.

Lifecycle remains ADR 0027/0028:

- Admission is human-selected, validates fully, then atomically publishes one
  inert process-local member. Failed validation publishes nothing. No runtime
  or provider is spawned for admission.
- Activation revalidates all stable identity evidence before composition
  construction and repeats it before publication. It does not trust the
  retained member ID or stale registration.
- Switch withdraws the old active composition, validates the target, builds
  fresh target composition, and publishes only if the captured lifecycle
  transaction is still current. It performs no Git worktree mutation.
- Close withdraws active executable authority and leaves membership inert.
- Removing an inactive member changes process-local membership only. It runs
  no worktree remove, prune, unlock, filesystem deletion, or Git mutation.
- Restart restores zero admitted members and zero active authority.
  Remembered candidates are descriptive location hints only. Fresh admission
  repeats root, gitfile, private/common, registration, Git executable,
  duplicate, alias, and nested checks. No identity or authority proof is
  persisted.

## Capability target and currentness matrix

All rows assume the active host-selected member supplies exactly one root and
active-only composition. Tool input carries no RepositoryMemberId, worktree
selector, gitdir path, common path, branch, or authority token. “Sibling
stale” means an effect-prepared or review-prepared state for a sibling is no
longer safe to reuse. A switch independently invalidates active-bound state
under existing lifecycle rules.

| Capability | Target root | Private worktree state required | Shared state required | Prepared currentness | Possible shared effect | Sibling stale condition |
| --- | --- | --- | --- | --- | --- | --- |
| fs.read | Selected root and one permitted relative file | Root/member binding; no HEAD or index dependency unless the file is Git metadata, which is excluded | None for ordinary file read | Active root, path containment, file/parent identity, nested-boundary checks | None | Only active switch/revocation or overlapping physical target; common-dir change alone never |
| repo.file-info | Selected root and selected relative path | Selected HEAD, index, and file state as the observer requires | Object lookup for referenced entries; exact refs only if returned presentation needs them | Root/private relation; path/preimage facts; fresh observed HEAD/index result | None | Only if its exact selected observation dependency changes; unrelated ref/object additions do not |
| repo.status | Selected root | Selected worktree HEAD and index | Common object/ref facts used by Git to resolve current HEAD and branch presentation | Root/private relation; current selected HEAD target/OID; selected index semantic state; operation-relevant config | None | If B's own HEAD/index or relevant exact ref changes; not because A adds an object or updates A's unrelated branch |
| repo.diff | Selected root | Selected worktree index and worktree bytes; Git resolves selected index | Objects referenced by selected index | Root/private relation, index semantic state, worktree path/preimage, fixed diff config | None | Overlapping B target only; selected B index/worktree dependency change |
| repo.diff-staged | Selected root | Selected worktree HEAD and index | Objects for selected HEAD/index entries | Root/private relation, exact selected HEAD OID/ref, index semantic digest/tree | None | Selected B HEAD/ref/index change; unrelated branch or object creation does not |
| create-branch | Selected root; no branch switch | Selected HEAD and attached branch state | Common refs namespace and requested ref/reflog name | Root/private relation, attached selected branch and HEAD OID, exact requested name absent including Git collision semantics, repository/composition generation | Creates one common refs/heads entry and applicable reflog; Git may create metadata | A colliding target name or changed selected base/current branch dependency; unrelated refs need not stale it |
| patch | Selected root and authorized existing file | No Git metadata mutation; only the selected active root's Git boundary checks | None | Root/member/composition, target identities, bounded exact preimage and policy currentness | Worktree content only | Overlapping target or parent identity change; sibling commit alone does not |
| edit-files | Selected root and exact authorized files | No Git metadata mutation; nested-boundary checks | None | Root/member/composition, each target and parent identity, complete reviewed preimages | Worktree content only | Overlap, target replacement, or active switch; no global common-state invalidation |
| create-file | Selected root, absent destination and existing safe parent | No Git metadata mutation; exclude .git and nested repositories | None | Root/member/composition, absence, parent identity, path alias and nested-boundary proof | Worktree file creation only | Same physical destination/parent conflict or active switch |
| directory creation Tool | Selected root, absent child and existing safe parent | No Git metadata mutation; exclude .git and nested repositories | None | Root/member/composition, destination absence, parent identity | Worktree directory creation only | Same physical destination/parent conflict or active switch |
| delete-file | Selected root and reviewed existing file | Git HEAD/index only if its current deletion policy uses them; no common ref mutation | Exact selected tree/index facts required by deletion precondition | Root/member/composition, exact file and parent identity, review/currentness, operation-specific Git precondition | Worktree file deletion only | Overlapping target or selected dependency change; sibling's unrelated commit is not itself a dependency |
| rename-file | Selected root and source/destination | Git HEAD/index for tracked-source and destination-absence checks | Exact selected tree/index facts required by rename precondition | Root/member/composition, source/destination and parent identity, case/alias proof, review/currentness | Worktree path rename only | Overlap or selected precondition change; not any common-dir change |
| Stage | Selected root and one host-selected tracked file | Selected private HEAD, selected private index, selected target, registration relation | Read required HEAD objects; git add may add content-addressed objects to common object store | Active selector/member/composition and observation generations; stable relation; target identity; selected HEAD/ref dependency; selected index precondition; rerun protected post-observation | Selected private index plus possible shared unreachable blob objects; no refs | Same selected worktree dependency only. Do not stale B for object addition or unrelated branch update. If B's selected ref/current HEAD changes, only operations depending on that exact fact stale |
| Unstage | Selected root and one host-selected tracked file | Selected private HEAD tree and selected private index | Read selected HEAD objects | Active selector/member/composition and observation generations; stable relation; exact HEAD tree entry; selected index precondition; no sibling index effect | Selected private index only | B's own HEAD/index dependency changes; not an unrelated A branch/object change |
| Reviewed Commit | Selected root, selected staged snapshot | Selected private HEAD, attached branch, selected private index, worktree registration | Common object store, exact selected ordinary branch ref, relevant reflogs | Member/root/private-common/Git identity; composition/repository generations; exact attached branch, expected HEAD/ref OID, raw index digest, staged entries digest, tree, complete review proof; final one-shot revalidation | New shared objects; selected shared branch ref and its reflog; per-worktree HEAD reflog/required Git metadata; selected index must not be substituted | Selected branch/ref, selected HEAD, selected index/tree, or review proof changed. Unrelated sibling branch commit/object creation does not invalidate it; active switch still does |
| Branch presentation | Selected active root and its HEAD | Selected private HEAD | Shared branch list only if UI explicitly presents it | Read current symbolic HEAD/HEAD OID; refresh any shared branch inventory before presenting as current | None | Refresh branch-list view on any relevant list change; never migrate selected worktree identity or retain an old ref list as authority |

The table defines future target resolution; it does not certify every current
policy already supports linked roots. In particular, the direct .git/index
read in Commit and the normal-directory restriction in branch creation must
be replaced or adapted under later authorization.

### Authoring path boundary

repo.patch, repo.edit-files, repo.create-file, the ordinary directory-creation
Tool, repo.delete-file, and repo.rename-file target only paths beneath the
selected active worktree root. Their host policies must reject sibling roots,
private gitdir, common gitdir, the selected root's .git entry, nested .git
metadata, and any path whose canonical ancestry leaves the selected root.
Case-insensitive .git components, aliases, reparse points, and existing nested
repository boundaries retain the existing fail-closed rules. A common Git
directory is never treated as a workspace root or a permitted authoring
destination.

The read/status/diff Tools continue to use only the active repository binding.
fs.read is root-bounded file access; repo.file-info/status observe that
selected worktree; repo.diff compares that worktree against its own index; and
repo.diff-staged compares that worktree's index against its own HEAD. No Tool
input acquires a member selector or can ask to read a sibling.

## Stage and Unstage contract

Future Stage and Unstage use:

selected active worktree
→ validated private gitdir and common relationship
→ selected worktree's Git-resolved index path

They must never resolve the selected index as common-dir/index or as a path
selected by model/provider input. Admission and each effect must clear inherited
Git redirection variables such as GIT_DIR, GIT_WORK_TREE, GIT_COMMON_DIR, and
GIT_INDEX_FILE, then compare fixed Git path-resolution results with the
captured host relationship.

Before Stage/Unstage, revalidate the active member and its selector,
composition/repository generation, root and gitfile/private/common identities,
worktree registration, selected Git executable, target identity, and the
operation's HEAD/ref/index preconditions. Unstage must bind the exact selected
HEAD tree entry it will restore. Stage binds the exact selected worktree
content and index precondition that it will stage. Afterward prove the selected
index entry changed as authorized, no unrelated selected-index entry changed,
the selected worktree bytes and HEAD remained within the existing contract,
and neither sibling index changed in live certification.

Current Stage code snapshots every ref before and after the index effect.
That is too broad for linked members: a concurrent commit on an unrelated
sibling branch would change the common ref inventory even though Stage did not
touch it. The conformance implementation should use the exact selected
HEAD/ref facts the index action needs and separately verify that the native
Stage/Unstage effect is limited to the selected index. It must not require
global shared-state locking against unrelated Git clients. A relevant selected
ref update fails currentness. If an external change overlaps an effect and the
effect cannot be proved, retain uncertain classification; do not retry,
replay, roll back, or compensate.

Stage may create shared blob objects; their presence does not stale a sibling.
Unstage reads the selected HEAD tree and writes only the selected index. The
fixture proved A stage/unstage changed A's index and not main/B indexes.

## Reviewed Commit contract

ADR 0016 remains the Commit authority. It already authorizes a single reviewed
commit of one exact staged snapshot, advances only the already-attached branch,
and rejects detached HEAD, linked worktrees, and bare repositories in its
current v1 form. This task changes none of those effects.

For a later linked-worktree implementation, the selected member's identity
must accompany the existing Commit authorization and bind:

- exact selected root and filesystem identity;
- exact linked .git/private/common/registration relationship;
- selected Git executable identity;
- one active repository/composition/member generation;
- selected per-worktree HEAD attachment and old HEAD OID;
- exact already-attached refs/heads target and expected current OID;
- selected private index raw digest, semantic staged-entry digest, and
  staged tree;
- complete reviewed staged content/postcondition evidence; and
- one-shot authorization and existing uncertainty rules.

Final confirmation must re-read the selected worktree's Git-resolved private
index and selected HEAD/ref. It must not read common/index or another member's
index. It must verify the single created commit has exactly the reviewed tree
and expected one parent, and that only the selected attached branch advanced
from the reviewed old OID to the verified new OID. Git may write new objects
in the shared store and selected ref/reflog in common state; it must not commit
another worktree's index or change another worktree's symbolic HEAD.

The current source's raw root/.git/index reads, metadata state checks under
root/.git, and linked-worktree rejection must be replaced with selected
gitdir-aware fixed Git path resolution before this contract can be met.
Existing Commit remains attached-branch-only. Detached HEAD may use read,
status/diff, bounded worktree edit, Stage, and Unstage when all of their
existing tracked-target preconditions pass. The current Stage/Unstage policy
uses resolved HEAD and HEAD tree but does not require symbolic-ref, so its
policy layer does not require attachment. Branch creation and Commit both
explicitly require an attached local branch; detached branch creation and
Commit remain unavailable. No Task 357 evidence expands those conditions.

The current implementation was reviewed directly. Under its repository
lease, staged review captures a semantic snapshot, obtains the fixed
index-versus-HEAD diff presentation, then captures the snapshot again and
rejects a changed review. The opaque review binds policy generation, attached
branch, old HEAD OID, staged-entry SHA-256, staged tree OID, and a presentation
hash. That presentation hash proves correspondence to the displayed review;
it is not authorization. Review rejects unsupported/binary staged
presentation.

Authorization re-captures the snapshot and compares the review's generation,
branch, old HEAD, staged-entry digest, and tree. The host-only authorization
then additionally binds the raw index SHA-256. Final execution consumes the
pending authorization once, acquires the repository lease, revalidates
repository and Git executable identity, attachment, branch-ref OID, HEAD OID,
raw index, staged entries, tree, ordinary Git operation state, and review
generation immediately before one fixed Git commit process. The command uses
the fixed host identity, a host-owned empty hooks directory, disabled commit
signing, disabled hooks, explicit cleanup mode, timeout, and bounded output.
Post-observation proves the selected branch/HEAD moved, the commit has exactly
the reviewed tree and old HEAD as its sole parent, the message and host
identity match, and the resulting index tree remains the authorized tree.
Failure after the attempt is known-no-effect only when exact no-effect proof
succeeds; otherwise it remains uncertain. The authorization is never replayed.

### Sibling impact and shared-ref currentness

Fixture evidence and the exact dependencies above lead to these rules:

- Commit A advances A's shared ordinary branch ref and adds shared objects.
  A's private HEAD follows its attached branch; B's private HEAD and index stay
  unchanged.
- B must not be rebound to A's commit or made stale merely because A created
  objects or updated refs/heads/worktree-a.
- Any B operation that depends on refs/heads/worktree-b checks that exact
  ref/OID and its own HEAD/index facts. An external update of B's checked-out
  branch makes that prepared operation stale even if Git would normally stop a
  second checkout of the same branch.
- A prepared Commit review for A must fail if A's expected branch ref changes,
  even if the A index and tree are unchanged. The one-shot review is not
  silently updated to the new OID.
- A Commit on unrelated B branch leaves A's selected branch, HEAD, and index
  dependency current. Do not invalidate A because a new object appeared or
  another ordinary ref value changed.
- Branch create checks the exact selected base/current HEAD and requested
  namespace collision. An unrelated branch create/delete is not a global
  invalidation unless it collides with that exact requested name or changes a
  fact the operation actually uses.
- A packed-refs rewrite is not itself stale if the semantic selected ref value
  remains identical. Compare the required ref value and resolution, not storage
  location, timestamps, or common-dir mtime.
- Local/shared configuration is revalidated only to the extent the selected
  command consumes it. Use fixed environment/argv and explicit overrides for
  authority-sensitive settings. Do not use a blanket common config hash as a
  proxy for all Git currentness.
- If a UI presents the whole branch inventory, refresh that presentation when
  its relevant values change. A stale list is not permission and never selects
  another worktree.

Git normally prevents checking out one branch in multiple worktrees. RAH may
use this as an additional Git behavior check, but not as the authority proof:
external clients can update refs independently, so every effect still checks
the exact selected worktree and selected ref dependencies.

## Fixed admission command boundary

Admission may run only bounded, read-only native Git commands with the
host-selected canonical root as explicit cwd, fixed executable identity,
fixed argv shapes, controlled environment, timeout, and output bounds:

- git rev-parse --show-toplevel
- git rev-parse --absolute-git-dir
- git rev-parse --path-format=absolute --git-common-dir
- git rev-parse --path-format=absolute --git-path HEAD
- git rev-parse --path-format=absolute --git-path index
- git rev-parse --is-inside-work-tree
- git rev-parse --is-bare-repository
- git rev-parse --show-superproject-working-tree
- git worktree list --porcelain -z
- git symbolic-ref -q HEAD
- git rev-parse --verify HEAD

Any later exact-ref probe uses a strictly parsed Git-produced symbolic HEAD
target, host-fixed argv construction, and exact selected ref only. No model
supplies argv, executable, cwd, Git environment, ref path, or worktree path.
No shell, generic process authority, or mutating admission command is added.

The admission sequence is:

human selects root
→ canonicalize and bound the root and reject unsupported Windows aliases
→ inspect the top-level .git entry without following links/reparse points
→ capture root and entry filesystem identities
→ run fixed read-only Git classification with a sanitized environment
→ resolve private/common paths and HEAD/index using Git
→ validate standard private registration, backlink, commondir, and list record
→ validate selected Git executable identity
→ reject invalid, duplicate, nested, bare, submodule, or unsupported forms
→ revalidate all captured relationship evidence
→ under membership coordination, atomically publish an inert member

No member is visible until the last validation and relation checks pass. Any
failure publishes nothing. No provider/runtime creation is part of admission.

## Windows aliases, reparse points, and moves

Retain v0.29 fail-closed behavior for root ancestry reparse points, .git
symlinks, reparse points, alternate streams, ambiguous path forms, and
nested repository boundaries. Extend that same check to every private/common
gitdir, common/worktrees parent, and registration component. A reparse point
at any of those locations makes classification unsupported. Do not claim
race-free TOCTOU.

Drive-letter case aliases and case-equivalent roots canonicalize to one
candidate; root and private-directory filesystem IDs prevent a second member
through an alternate spelling. Reject colon/ADS, device paths, verbatim
namespace paths, and any Git environment redirect. For v0.30's narrow Windows
baseline, reject UNC/network-root candidates unless separately proven by a
future task to have consistent filesystem identity, Git path resolution, and
reparse behavior. No network-isolation claim follows from this policy.

| External change | Retained member result |
| --- | --- |
| git worktree move | Old canonical root/backlink no longer matches; stale. New root requires complete explicit admission. |
| Manual move without repair | Git registration/backlink or root relation is incoherent; stale/reject until externally repaired. |
| git worktree repair | Repair is external mutation. Fresh admission after repair may create a new member; old preparations remain invalid. |
| Private metadata replacement | Private directory/file identities or relationship digests change; stale. |
| Common directory replacement/move | Common canonical path, filesystem identity, or relation changes; all members relying on that common relationship become stale and need fresh admission. |
| gitfile rewrite or replacement | File identity, exact-byte digest, or resolved target changes; stale. |
| External worktree remove | Root and/or registration disappears; stale. RAH does not remove it. |
| External worktree prune | Missing/stale registration cannot pass Git list plus filesystem checks; stale. RAH does not prune it. |
| External sibling Commit | Revalidate only selected semantic facts. An unrelated branch/object change may remain current; selected branch/HEAD/index changes fail the dependent operation. |

Git documents locked as protection against pruning for worktrees on portable
or intermittently available storage; it does not establish an accessible
working tree. A present locked worktree may be admitted if its root and full
identity are currently accessible and coherent. A missing or inaccessible
locked root is rejected. Any prunable or stale record is rejected.

## Race and currentness matrix

| Race | Stable identity | Mutable currentness | Must fail stale | May remain valid | Effect handling |
| --- | --- | --- | --- | --- | --- |
| Admission vs external worktree move | Root, .git form/object, private/common IDs, backlink, registration | Root path and Git registration record | If any before/after capture disagrees, publish nothing | None for the candidate under validation | No Git mutation or replay |
| Activation vs gitfile replacement | Root and expected .git/private/common relationship | Exact gitfile file ID, bytes digest, resolved target | Fail before active composition; repeat check before publication | Other members remain inert | No effect is started |
| Activation vs private gitdir replacement | Root and expected private/common relation | Private directory and registration IDs | Fail target activation and publish no composition | Other members unaffected unless they share the replaced common relation | No effect is started |
| Activation vs common-dir replacement | Per-member root/private identity | Common path/object ID and relation | Fail activation for every member whose captured common relation changed | Inert membership may remain descriptive but not executable | Fresh admission required |
| Switch vs external sibling Commit | Each member's own stable identity; one-active transaction | Target's own HEAD/index/ref and shared exact facts | Fail if target relation or required exact currentness changes before publication | Unrelated sibling objects/refs that target does not consume | Old composition withdrawn only under existing transaction; no Git effect |
| Stage preparation vs sibling ref update | Selected root/private/common relation | Selected HEAD/ref/index; unrelated common refs not dependency | Fail if selected HEAD/ref/index changes; no global refs snapshot invalidation | Unrelated branch/object update | A started Stage remains one-shot; no replay |
| Commit review vs external selected-branch ref update | Selected member identity | Expected attached branch OID and HEAD OID | Review/authorization/final confirmation fails against old OID | Unrelated sibling branch and objects | Consume authorization once; no retry |
| Commit review vs sibling unrelated-branch Commit | Selected A root/private state | A branch/ref, HEAD, A index/tree | Fail only if A's own facts changed | B branch/ref, B HEAD/index, common object additions | A review may remain current; switch still invalidates |
| Close vs external Git effect | Member identity, already captured effect owner | External result and selected index/ref facts | Close follows current busy/uncertain owner rules; it cannot claim rollback | Inert membership may remain descriptive | Never replay or compensate |
| Inactive removal vs external unregister | Process-local member ID is selector only | Fresh root/private/common/registration state | Removal can remove only the inert process-local member; later activation must fail stale | Other members | No Git worktree command |
| Restart plus remembered stale location | No persisted stable identity | All root, .git, private/common, registration and executable facts | Fresh admission fails if any current relationship is stale | Descriptive candidate can remain visible | Restart restores zero authority; no implicit probing/admission |

The RAH process-local lease/currentness model is not a lock against external
Git clients. If an external effect overlaps a started RAH effect, preserve the
existing exact-once and uncertain-effect semantics. No outcome authorizes
retry, replay, rollback, compensation, or inferred absence.

## Deterministic acceptance matrix

All rows require zero publication on rejection, no leaked private evidence in
Tool/provider/activity/persistence output, and no unrequested Git mutation.
The final column names Windows evidence required at the later Task 361 gate;
Task 357 itself adds no tests.

| Case | Classification | Admit? | Identity evidence | Failure reason | Authority implication | Deterministic future test | Windows live requirement |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Main ordinary worktree | Supported main form | Yes | Root ID; .git directory ID; private=common=root/.git; registered main record | Any mismatch or unsupported path | Existing one-member target | Admit, activate, read, preserve v0.29 behavior | Main member admitted; exact identity and no ordinary-worktree regression |
| Linked worktree A | Supported linked form | Yes | Root/.git file digest+ID; private A ID; common ID; backlink, commondir and registration | Any relationship mismatch | A is one distinct selected member | Admit A and prove A private HEAD/index | A has distinct process-local member ID and actual private paths |
| Linked worktree B | Supported linked form sharing common | Yes | Same common ID as A but different root/private IDs and record | Incomplete registration or duplicate private ID | B remains distinct from A and main | Admit all three without union composition | Main/A/B all admitted; only one active |
| Same root via case/drive/canonical alias | Duplicate of same member | No second member | Canonical root and root filesystem ID match | already_member | No duplicate authority | Submit equivalent spelling | Alias rejected without focus or activation side effect |
| Same private gitdir via alternate root spelling | Duplicate/invalid alias | No second member | Private directory ID matches; backlink must still identify one root | already_member or invalid relationship | Common/private state cannot multiply members | Revalidate alternate path and backlink | Duplicate rejected; original member unchanged |
| Same common dir, different private gitdirs | Distinct linked members | Yes | Different valid root/private IDs, same common ID | None when every relation validates | Shared common state is not member identity | Admit A and B; assert relation is Distinct | Both remain separately selectable; one-active only |
| Copied gitfile from A | Invalid copied root | No | Root differs; backlink names A/.git; Git list omits copied root | backlink/registration mismatch | No sibling alias authority | Copy .git file; prove Git may resolve but host rejects | Rejection before member publication |
| Manual gitfile pointing to main/.git | Arbitrary recognized gitfile | No | private=common, no linked worktrees registration/backlink | Not registered linked form | No redirection to main authority | Git recognizes candidate; host rejects | No member; main unchanged |
| Malformed gitfile | Invalid metadata | No | Bounded strict parse fails | Invalid format, extra line, NUL, or unsupported bytes | No member or Git process target | Truncated/multiline/oversized fixture | Fail closed before effect |
| Replaced gitfile object | Stale linked relation | No for retained member | File ID changes; captured digest/target rechecked | Identity stale | Require fresh admission | Replace with same or different target; old identity fails | Old member cannot activate or dispatch |
| In-place rewritten gitfile inode | Stale linked relation | No for retained member | Same file ID but exact-byte SHA-256 and target change | Content relationship stale | FileIdentity alone is insufficient | Edit contents in place; assert digest mismatch | Fail before activation/effect |
| Replaced private gitdir | Stale linked relation | No | Private dir/file IDs and contents change | Private identity mismatch | Old worktree member withdrawn from execution | Replace registration/private directory | No active publication or native effect |
| Replaced common dir | Stale related members | No for old identities | Common directory ID/path differs | Common relationship stale | No reuse of old common relation | Substitute same-path common dir | A/B old members both fail activation |
| Broken gitdir backlink | Invalid linked form | No | Backlink bytes/ID do not resolve to selected root/.git | Registration points elsewhere | No copied-root authority | Rewrite backlink to sibling or absent root | Zero member publication |
| Broken commondir | Invalid linked form | No | commondir bytes/hash/resolution disagree with Git common path | Common relationship unsupported | No common state handle | Malform or retarget commondir | Zero member publication |
| Modern submodule | Nested submodule | No | Superproject evidence, gitlink, private under modules, no worktrees registration | Submodule/out-of-scope nesting | No submodule membership or traversal | Native submodule fixture and fixed probes | Rejected even though .git is a file |
| Old-form submodule | Nested submodule with .git directory | No | Superproject/gitlink and nested root evidence | Submodule/out-of-scope nesting | File-vs-directory cannot admit it | Fixture or deterministic boundary classifier | Reject; no nested path Tool access |
| Separate-git-dir repository | Git-recognized gitfile, not linked | No | private=common external directory; no standard registration/backlink relation | Not standard linked form | No arbitrary .git indirection | git init --separate-git-dir fixture | Git-recognized candidate remains unadmitted |
| Bare repository | Bare | No | is-bare=true; no selected top-level worktree | No working-tree root | No synthetic worktree authority | Native bare fixture | Rejected with zero authority |
| Nested linked root below admitted root | Nested linked member | No | Canonical path ancestry; root IDs | ADR 0027 nested co-membership | No parent/nested coauthority | Add linked root beneath main and attempt admission | Rejection and nested-.git path protection preserved |
| Ordinary nested repository | Nested repository | No co-member; target path rejected | Nested .git boundary beneath selected root | Nested membership/path boundary | Existing per-path nested repository boundary remains | Read/edit/stage nested target | No cross-boundary Tool access |
| Root junction or symlink | Reparse root | No | symlink_metadata/reparse attributes for every ancestor and root ID | Ambiguous alias | No alias-selected authority | Junction/symlink selected root | Windows gate proves fail closed |
| .git symlink/reparse point | Unsupported metadata | No | Entry form/reparse state before Git follows it | Not regular directory/file form | No .git indirection | Replace top-level .git with symlink/junction | Rejected before classification publication |
| Private gitdir reparse point | Unsupported private state | No | Private path components and final directory reparse state | Unsafe private relation | No private Git context | Replace private directory or ancestor with junction | Fail closed; no Stage/Commit attempt |
| Common gitdir reparse point | Unsupported common state | No | Common path components and common directory identity | Unsafe shared relation | Common directory grants no authority | Junction common path in disposable fixture | Fail closed for every member |
| worktrees/<id> registration reparse point | Unsupported registration | No | Worktrees parent, child ID, final directory object identity | Registration relation can be redirected | No registration ID trust | Junction one registration child | Rejected; no other registration followed |
| UNC, verbatim, device or ADS root | Unsupported Windows spelling for v0.30 | No | Canonical form and volume/path identity cannot meet narrow policy | Unsupported ambiguous root form | No generic path authority | Present each input spelling to admission validator | All unsupported forms rejected unless separately authorized |
| git worktree move | Changed root relationship | Old member: no; new explicit admission may pass | Root canonical path, backlink, registration and current IDs | Retained identity stale | No automatic member migration | Move in fixture, try old then new candidate | Old activation fails; new admission is explicit |
| Manually moved then unrepaired | Broken registration | No | Git semantic probes, .git target, backlink, root mismatch | Incoherent relation | No inferred repair authority | Move directory without Git command | Reject until external repair and fresh admission |
| External repair after move | Freshly repaired form | Old member: no; fresh admission: conditional | Re-capture every stable fact after repair | Old captured identity stale | Repair is not RAH authority | Repair only disposable fixture then readmit | No RAH worktree mutation; fresh ID after readmission |
| Externally removed/pruned worktree | Missing or unregistered | No | Root existence, private dir, list registration | Missing/prunable/stale | No automatic prune/remove | Remove/prune only in disposable fixture or simulated state | Activation fails; no recreation or retry |
| Locked, present, coherent worktree | Registered locked linked form | Yes | All normal identity evidence plus locked record and accessible root | Reject only if inaccessible or otherwise incoherent | Lock does not authorize or revoke target | Lock and inspect porcelain without unlocking via RAH | Present locked target can be admitted; RAH performs no unlock |
| Detached HEAD linked worktree | Valid linked identity, detached branch state | Yes for non-Commit uses | Normal linked identity; detached record; private HEAD OID | Commit/branch-create require attached branch | Read/edit/Stage/Unstage retain current narrow policy; Commit remains unavailable | Detach A; inspect each existing capability precondition | Commit attempts remain zero; no branch switch |
| Restart, remembered candidate | Descriptive candidate only | No auto-admission; yes after explicit fresh admission | Re-capture every path, file, private/common, registration, Git fact | Any stale evidence | Restart begins with zero members/active authority | Restart fixture/process and inspect state | Zero authority before explicit fresh admission |
| Refs packed or loose representation changes | Same semantic ref if OID unchanged | Yes if exact dependencies still agree | Selected ref value and expected target, not storage file identity | Reject only if required semantic OID/target changed | Packed-refs bytes/mtime are not member authority | Pack refs without changing selected semantic values | Prepared operation based on unaffected selected values remains valid |

## Windows live-gate contract for Task 361

Task 361 remains a release-blocking, host-driven backend certification using
real native Git 2.54.0.windows.1-or-approved-current-version disposable
main/linked-a/linked-b roots. It must exercise the production admission,
active-composition, existing Tools, Stage/Unstage, and reviewed Commit routes.
The fixture stays outside the RAH checkout and is removed only through its
attributable test owner. No marker is printed before every required assertion
passes.

### Admission and active lifecycle

- Admit main, A, and B; assert three fresh distinct member IDs and one shared
  common identity with three private target identities.
- Reject duplicate root/case/drive aliases, copied/fabricated/malformed
  gitfiles, submodule, separate-git-dir, bare, nested roots, and all tested
  reparse variants before member publication.
- Demonstrate same common state does not collapse or union member identity.
- Exercise main → A → B switch and prove only one active composition/registry
  exists at any instant. Close withdraws active authority and leaves members
  inert. Inactive removal changes membership only and makes zero Git
  worktree remove/prune/move/unlock calls.
- Restart with remembered candidates and prove zero membership, active
  repository, registry, Stage/Unstage state, Commit authorization, or
  executable identity until fresh human admission and separate activation.

### Per-worktree state, authoring, and effects

- Prove selected HEAD path/branch/OID for each member and per-worktree index
  paths from fixed Git commands.
- Stage A and prove only A's index changes; Unstage A and prove only A changes
  back. Main and B index snapshots remain unchanged.
- Edit a selected worktree file in each route and prove only that selected
  root changes. Attempt sibling-root and Git metadata paths and prove reject.
- Review and commit a staged A snapshot. Prove A's selected index/HEAD/ref
  facts were used, only the intended attached branch ref advanced, no B or
  main index changed, and B/main HEAD remained their selected branch state.
- Prepare A review, externally advance A's selected branch ref using a safe
  disposable forward update, and prove the old review fails currentness
  without dispatch/rebind/retry.
- Commit or otherwise update B's unrelated branch and prove A's selected
  currentness is not rejected solely for unrelated shared object/ref change.
- Prove no cross-worktree Tool dispatch, model/provider-selected root, or
  repository ID/worktree selector enters Tool input.

### Shared state, reparse, and accounting

- Confirm common relationship is revalidated at fresh activation and effect
  preparation/confirmation. Distinct roots remain separate while shared refs
  resolve from common state.
- Exercise root junction/symlink, .git symlink/reparse, private gitdir,
  common gitdir, and worktrees/<id> reparse variants; fail closed before
  effects.
- Count automatic worktree create/remove/prune/move/repair/unlock, branch
  switch, fetch/pull/push, network Git, provider routing, and model-selected
  worktree dispatch as zero.
- Preserve one-attempt/no-replay/uncertain-effect accounting for every
  started external effect.
- Print marker only after all assertions pass:
  RAH_V030_LINKED_WORKTREE_LIVE_OK

The planned gate is host-driven backend certification, not GUI automation.
Unless executed, it does not claim GUI automation, model-selected worktree
dispatch, Linux/macOS live parity, race-free TOCTOU, OS sandboxing, network
isolation, rollback, or automatic Git worktree lifecycle operations.

## Authority, privacy, and nonclaims

- Tool names and input schemas remain unchanged. HostExplicit stays exactly
  11: fs.read, repo.file-info, repo.status, repo.diff, repo.diff-staged,
  repo.create-branch, repo.patch, repo.edit-files, repo.create-file,
  repo.delete-file, repo.rename-file.
- No worktree selector, RepositoryMemberId, private/common path, filesystem
  identity, registration ID, currentness token, Git path, branch authority,
  or review ticket is added to Tool/provider/model input.
- Active membership selects one ordinary target. Provider metadata, model
  output, Tool input, frontend state, or remembered candidate cannot select or
  admit a worktree.
- No new PermissionLevel or external permission is needed. Existing
  capability-specific read, worktree, Stage/Unstage index, branch-create, and
  reviewed Commit authorities remain separate. If later implementation
  appears to require a new mutation permission, stop for a new decision.
- No persistence schema change. Never persist private/common paths, filesystem
  IDs, worktree IDs, RepositoryMemberId, generations, active authority,
  identity hashes, ref snapshots, tickets, preparations, Stage/Unstage
  actions, Commit authorization, or effect continuation.
- Keep relationship bytes, paths, filesystem IDs, Git registry IDs, ref
  snapshots, and currentness tokens host-private. User review may use the
  existing bounded label/basename and repository-relative paths.
- No generic filesystem read, Git, common-repository, process, ref, or
  provider authority is created. Admission's bounded metadata reads are
  private host identity checks, not a Tool capability.
- No RAH linked-worktree create/remove/move/repair/lock/unlock/prune; branch
  switch; remote Git; network operation; multiple active repository; shared
  registry; cross-worktree Tool route; or persisted authority.
- No claim of race-free TOCTOU, cross-process Git exclusion, rollback,
  compensation, automatic replay, OS sandbox, network isolation, or
  cross-platform live parity.

## ADR and sequencing recommendation

Task 358 should accept a short additive ADR 0029 or equivalent formal
conformance decision with a title such as **Linked Worktree Identity and
Shared Git State under ADR 0027**. Its purpose is to:

1. replace the current support nonclaim with the one accepted main and linked
   identity forms;
2. distinguish one root/private gitdir target from common shared Git state;
3. freeze exact gitfile, backlink, commondir, registration, filesystem
   identity, and content-currentness proofs;
4. preserve Same/Nested/Distinct membership semantics and forbid same-common
   deduplication;
5. state operation-specific HEAD/index/ref/object dependencies and no
   common-dir-mtime/global-ref-snapshot invalidation;
6. preserve ADR 0012 worktree mutation scope, ADR 0016 attached-branch Commit
   and one-shot effect semantics, and separate Stage/Unstage index authority;
7. preserve one active composition, process-local membership, descriptive
   persistence, consent, privacy, and fresh activation/readmission; and
8. state the explicit exclusions and nonclaims in this research.

This is an identity specialization within ADR 0027, not a new authority
category. Task 358 must make that classification explicit before implementation
planning is accepted. Task 359/360 implementation or conformance work must
wait for Task 358 acceptance; Task 361 remains separate live certification.

## Task 357 repository validation

Required docs-only validation after authoring:

- git diff --check
- cargo metadata --no-deps --format-version 1
- exactly 13 workspace packages, each version 0.29.0 and edition 2024
- Cargo.lock unchanged; dependency graph unchanged
- no Rust, frontend, test, permission, persistence, or ADR changes
- README changes only the v0.29 heading/status to released
- only README.md and this plan are changed

Task 356's exact-head CI run 35298955522 is the prior-roadmap PASS. The
Task 357 commit must separately be pushed and have a successful terminal
exact-head CI run before this task is closed.
