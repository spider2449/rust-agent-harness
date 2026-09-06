# ADR 0020: Repository Local Branch Creation Authority

Status: Accepted

Date: 2026-09-06

## Context

RAH has distinct host-owned authorities for observation, worktree content and
entry mutation, index mutation, and reviewed commit/history mutation.  None
authorizes creation of a persistent local branch ref.  Task 223 established
the bounded v0.19 contract below.  This ADR records that decision only; it
does not authorize implementation, public composition, or a UI change.

## Decision

RAH introduces `RepositoryBranchCreationPolicy`, a private, host-owned,
reusable authority for creating exactly one previously absent ordinary local
branch at the selected repository's exact current attached `HEAD` commit.
The future first-party capability is `repo.create-branch`.

This is local branch/ref **create** authority only.  It is not generic Git,
arbitrary ref mutation, commit/history, checkout or branch-switch, index,
worktree, repository-selection, remote, or network Git authority.

The established authority planes remain separate:

```text
observation
  != worktree content mutation
  != file create/delete/rename
  != directory create/tree mutation
  != index mutation
  != reviewed commit/history mutation
  != local branch creation
  != checkout/HEAD/worktree transition
  != remote/network Git
```

In particular:

```text
RepositoryCommitPolicy            != RepositoryBranchCreationPolicy
RepositoryMutationPolicy          != RepositoryBranchCreationPolicy
worktree file/directory policies  != RepositoryBranchCreationPolicy
PermissionLevel::Execute          != RepositoryBranchCreationPolicy
Tool registration                 != RepositoryBranchCreationPolicy
Trusted Profile/provider metadata != RepositoryBranchCreationPolicy
model request                     != RepositoryBranchCreationPolicy
frontend state                    != RepositoryBranchCreationPolicy
```

The policy exists only where the trusted host explicitly composes it for the
selected repository.  `Execute` is the future Tool's outer dispatch gate; it
does not manufacture the policy.

## Ownership and public contract

The model may supply exactly one logical branch `name`.  It may not select the
repository, Git executable, cwd, environment, full ref or namespace, new/old
OID, start point or revision, checkout target, tracking/upstream, force flag,
reflog message, hooks path, Git config, raw argv, or remote.  The host binds
every authority-bearing field.  The frontend may display or invoke the
host-composed capability but is never its authority source.

The closed v1 Tool schema is:

```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string", "minLength": 1, "maxLength": 128 }
  },
  "required": ["name"],
  "additionalProperties": false
}
```

Its permission is `PermissionLevel::Execute`; its Effective Authority
presentation is sanitized and repository-bound (`repositoryBound: true`) with
a first-party local-branch-creation classification.  It exposes neither paths,
raw argv, environment, hook location, policy generation, nor internal
diagnostics.

The Tool creates one new local branch at the repository's current committed
HEAD without switching branches.  Existing staged and unstaged changes remain
in the current worktree and are not incorporated into the new branch.  It must
not be described as checkout, switch, “start working on”, or moving work onto
a branch.

`RepositoryBranchCreationPolicy` is reusable for individually bounded create
requests in one current host-composed repository context; it is not one-shot
human review authorization.  Each request has one validated name, a
host-built `refs/heads/` namespace, a freshly captured attached HEAD target,
an absence requirement, CAS protection, fresh admission, and final
revalidation.  The policy becomes stale or unusable if its repository/runtime
identity or generation is no longer current, or its executable/policy identity
is invalid.  Reuse never conveys unlimited generic ref authority.

## Exact effect and branch-switch separation

The only authorized business effect is:

```text
refs/heads/<validated-logical-name> -> <host-captured-current-attached-HEAD-commit>
```

The only permitted secondary effect is Git-owned reflog metadata for that new
ref under the fixed reflog contract.  No mutation of symbolic HEAD, current
branch, HEAD OID, index, worktree, tracking configuration, existing refs,
tags, remotes, remote-tracking refs, repository config, conversation state, or
provider state is authorized.

Creating `refs/heads/new-name` does not grant authority to change HEAD,
checkout, switch, restore files, update an index for another branch, or
create-and-switch.  A branch switch requires separate authority and a separate
ADR; later implementation must not add it for convenience.

## Repository and name admission

V1 supports a normal non-bare repository with a real `.git` directory at the
selected root, attached existing HEAD, SHA-1 or SHA-256 OIDs, loose or packed
refs, dirty/staged state, shallow clones, sparse checkout, and alternate object
storage where Git/repository identity remains valid.

It rejects bare repositories, detached or unborn HEAD, `.git` indirection,
linked worktrees, a selected submodule working-tree root, malformed/replaced
identity, unsupported ref storage (including reftable), and unsupported special
operation states.  Merge, rebase, cherry-pick, revert, bisect, sequencer, and
squash/recovery states represented by existing hardened markers fail closed for
workflow clarity and recovery semantics; this does not claim Git cannot create
a ref in those states.  These are v1 admission boundaries, not permanent
safety claims.

Dirty, staged, and mixed state are allowed: the new branch points at committed
HEAD; changes remain in the current worktree and are not incorporated.  There
is no Stage or Unstage side effect.

The closed v1 logical-name language is ASCII only, 1–128 bytes total, with
1–8 slash-separated components of 1–48 bytes each.  Every component is:

```text
[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?
```

Accepted spelling is preserved exactly; no normalization occurs.  The validator
rejects empty names/components, leading dash or slash, trailing slash, repeated
slash, leading-dot components, `.`/`..`, trailing dot or space, `.lock`
suffixes, `..`, `@{`, single `@`, ASCII controls, DEL, spaces, `~`, `^`, `:`,
`?`, `*`, `[`, backslash, non-ASCII/Unicode, and input beginning `refs/`.
It also rejects case-insensitive Windows reserved aliases `CON`, `PRN`, `AUX`,
`NUL`, `COM1`–`COM9`, and `LPT1`–`LPT9`.  The caller never supplies
`refs/heads/`; the host constructs it.

RAH's validator is the authority boundary.  A fixed
`git check-ref-format --branch <validated-name>` may be a secondary Git
compatibility check, but Git acceptance cannot widen RAH's language.  Shell
quoting is not an authority boundary.

Before mutation, bounded Git-owned observation of local heads rejects exact
targets, ancestor/descendant prefix collisions (`foo` versus `foo/bar`), and
ASCII case-fold collisions (`Feature` versus `feature`) uniformly on Windows
and Linux.  It covers loose and packed heads, uses bounded count/output, fails
closed on overflow, and does not enumerate unrelated namespaces.

## Native mutation, hooks, and configuration

The selected private primitive is fixed native `git update-ref`, not
`git branch`.  A host-built invocation has the semantic shape:

```text
git <host-fixed security config> update-ref --create-reflog \
  -m "RAH create local branch" refs/heads/<validated-name> <captured-oid> \
  <zero-oid-for-object-format>
```

It conveys the exact constructed ref, captured OID, and atomic expected absence
without checkout or tracking behavior.  The public API must not expose generic
`update-ref`, arbitrary refs/OIDs, `--stdin` transaction language, deletion,
symbolic-ref mutation, force, or relaxed retry.  Implementation may arrange
fixed `-c` arguments in the existing hardened HostExecutionPolicy shape.

The old value is a zero OID of the captured object format: 40 zeroes for SHA-1
or 64 for SHA-256.  It is mandatory compare-and-create CAS.  A concurrent
creator makes Git fail; RAH never overwrites, forces, delete-and-recreates, or
retries with a relaxed old value.  Observation alone is insufficient.

Reference hooks are a required confinement boundary.  The host must use the
reviewed-commit-equivalent hooks shape: a unique canonical initially empty
directory outside the repository, captured identity, identity and emptiness
revalidated immediately before mutation, absolute host-pinned
`core.hooksPath`, and cleanup only while it remains the captured safe object.
`--no-verify` is neither applicable nor sufficient.  A repository hook or
repository-local `core.hooksPath` must not gain execution authority.

System/global Git configuration is disabled under the hardened Git policy;
repository config remains untrusted ambient data.  The host pins security
critical values including exact `safe.directory`, `core.hooksPath`,
`core.logAllRefUpdates`, fsmonitor and untracked-cache behavior, terminal
prompting, and a fixed non-secret reflog identity if needed.  No network
credential configuration is supplied.

V1 uses `--create-reflog`, fixed message `RAH create local branch`, and
host-controlled identity.  Git-owned reflog state for only the new branch is
permitted.  No model-controlled message or manual `.git/logs` write/delete or
arbitrary reflog mutation is permitted.

## Serialization, revalidation, and outcomes

The existing per-canonical-repository mutation lease serializes RAH-owned
Stage, Unstage, reviewed commit, content mutation, file create/delete/rename,
directory creation, and branch-create calls.  It does not exclude external Git
or filesystem processes.  Thus fresh final checks, CAS, and post-observation
remain mandatory and no race-free claim is made.

Immediately before the sole mutating attempt, while leased, the host
revalidates policy/runtime binding; repository generation/context and identity;
`.git` topology and ref storage; executable identity; hooks identity/emptiness;
special-state exclusion; symbolic attached HEAD, branch, OID, and commit type;
branch name and constructed namespace; and local-head exact/prefix/case
collision state.  CAS is the final absence guarantee.

At most one mutating Git process may run per Tool execution.  There is no
automatic mutation retry, fallback to `git branch`, replay after uncertainty,
or rollback.  Observation after uncertainty is allowed; `update-ref -d`,
branch deletion, reflog removal, switching, and worktree restoration are not.

The public sanitized taxonomy is:

| Outcome | Meaning |
| --- | --- |
| `invalid_input` | Closed schema/name validation failed; no attempt occurred. |
| `precondition_failed` | Authority, admission, identity, state, collision, or final validation failed before mutation. |
| `known_no_effect` | After a possible attempt, fresh observation proves target absence and protected HEAD state intact. A nonzero exit alone is insufficient. |
| `branch_created_verified` | One attempt and post-observation prove exact target/OID, unchanged symbolic HEAD/current branch/HEAD OID, and permitted reflog semantics. |
| `desired_state_observed_after_uncertain_attempt` | Desired target/OID is observed after an uncertain attempt, but RAH cannot attribute it causally. It is not verified success and never authorizes replay. |
| `uncertain` | Effect or required observation cannot establish any safer result. |

Timeout, cancellation, disconnect, child failure, lost result, and observer
failure do not imply rollback.  The desired-state-after-uncertain result is
conservatively error-like/non-verified unless the established result design has
a more precise non-success representation.  Verified success need not
recursively hash index/worktree/config, since the fixed primitive cannot alter
those planes; deterministic tests must prove index/worktree unchanged, no
upstream configuration, and unrelated refs unchanged.

## Review, currentness, and composition

Successful bounded branch creation does not revoke an otherwise valid reviewed
commit authorization: ADR 0016 binds attached branch, old HEAD, index
semantics, and tree, none of which this effect changes.  It neither consumes
commit authorization nor stages/unstages.  Any blanket future implementation
rule that revokes review for every repository effect must be refined for this
known first-party effect; final commit revalidation remains authoritative.

It does not increment `repository_generation` and introduces no branch/ref
generation.  Repository identity/context, active branch, HEAD, index, and
worktree are unchanged; fresh ref observation under lease plus CAS protects
against stale targets.  The repository/model/profile/connection currentness
tuple remains Current: no disconnect/reconnect, provider recomposition, or
runtime republication occurs.  Repository-scoped conversation identity remains
unchanged; create-only introduces no branch-specific namespace.

Trusted Profile/provider metadata cannot grant the policy; no profile
capability is added.  External MCP/Process Plugin metadata cannot widen it.
`repo.create-branch` is first-party host composition only in v0.19; Task 207
is unrelated.

## Consequences

RAH gains its first bounded local ref-create capability without a HEAD, index,
or worktree transition.  Expected-absence CAS, host-built fields, hook/config
confinement, one-attempt handling, and deterministic verification make its
effect narrow without claiming external race elimination or rollback.

Costs are a new persistent ref authority, ASCII-only names, normal attached
repositories only, deferred linked-worktree/reftable support, secured hooks
directory lifecycle, bounded local-head enumeration, conservative uncertain
UX, and no branch switching through this capability.

## Alternatives rejected

1. **Reuse reviewed commit authority** — a reviewed commit moves the existing
   attached branch from a reviewed index; it does not create a new ref.
2. **Reuse generic repository mutation or Execute** — both are broader and on
   the wrong effect plane.
3. **Use `git branch` porcelain** — it expresses the required expected-absence
   and fixed behavior less directly than private `update-ref`.
4. **Expose generic `update-ref`** — arbitrary ref mutation is vastly broader.
5. **Create and switch** — it combines ref creation with HEAD/index/worktree
   transition.
6. **Host/UI-only creation** — selected v0.19 permits a bounded model-requested
   name while retaining host policy as authority.
7. **One-shot human review authorization** — no reviewed content snapshot is
   being authorized; each narrow CAS-bound request is independently admitted.
8. **Automatic rollback/delete after uncertainty** — deletion is separate
   authority and uncertainty cannot justify compensation.

## Non-goals

This ADR excludes branch switch/checkout/create-and-switch; delete, rename,
force, or reset; arbitrary start commits or refs; tags; remotes; tracking;
fetch/pull/push; merge/rebase/stash; worktree creation; linked-worktree and
reftable support in v1; Unicode names; generic Git executor; shell/process
authority; and rollback.

## Relationship to existing ADRs

ADR 0020 adds a local-branch-create plane without superseding ADR 0010
(repository/index mutation), ADR 0011 (Trusted Profile host composition),
ADRs 0012 onward (worktree/file separation), ADR 0016 (reviewed commit), or
ADRs 0017–0019 (file delete, rename, and directory creation).  Existing
host-ownership, sanitization, repository selection, lease, no-replay, and
non-sandbox boundaries remain authoritative.
