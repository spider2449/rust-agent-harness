# Task 223 — Local Branch Creation Authority Contract Research

## Result

**COMPLETE.** v0.19 should add one new, reusable, host-owned
`RepositoryBranchCreationPolicy`, then expose it as the first-party Tool
`repo.create-branch`.  One call creates one previously absent ordinary local
branch at the exact attached `HEAD` commit captured by the host.  It is a ref
creation authority, not Git, checkout, switch, history, index, worktree, or
remote authority.  Task 224 should record this as a new ADR before any code.

## Authoritative starting checkpoint

Research began from clean `faf079f8754b50832826e84ca166b6e9b8d28dc2`, equal
to `origin/master`, after v0.18.0's immutable release
`bd0d237b8cda8f4cebdf56d2e3a792b5dd79ba2e` and checkpoint
`930f4eb1e3f8ef04dd685d8581e391080ce8bace`.  Task 222 exact-head CI was
`34007599182` PASS.  The source audit covered README, architecture,
guardrails, security, ADRs 0010–0019 (especially 0010, 0011, and 0016), the
v0.19 roadmap, `git_support`, stage/unstage, commit, repository identity and
observer code, mutation leasing, Desktop composition/activity/currentness,
conversation persistence, and Effective Authority.

## Selected v0.19 scope

The sole persistent business effect is creation of exactly one absent direct
ref:

```text
refs/heads/<validated-logical-name> -> <captured-attached-HEAD-commit>
```

There is no checkout, switch, `HEAD` change, index or worktree mutation,
arbitrary revision/start point, ref namespace, tracking setup, overwrite,
rename, deletion, remote Git, or generic Git command execution.  A later A2
branch switch is a separate authority and must not be inferred from this work.

## Existing authority boundaries

ADR 0010's index Stage/Unstage authority changes one host-bound index entry;
ADR 0016's reviewed commit authority consumes a human authorization to move
the current attached branch from a reviewed staged snapshot; ADRs 0012–0019
cover worktree content and entry effects.  None implies creating a persistent
name in `refs/heads`.  In particular, reviewed commit, index mutation, file
mutation/create/delete/rename, directory creation, `PermissionLevel::Execute`,
Tool registration, a model request, frontend state, Trusted Profile, and
provider metadata do **not** imply branch creation authority.

## Existing Git hardening foundation

The private reviewed-commit policy provides the applicable implementation
shape: `RepositoryIdentity` binds canonical root and `.git` identity;
`HostExecutionPolicy` captures/revalidates the exact native executable;
`repository_lease` serializes RAH-owned mutations per canonical root; and a
unique host-owned empty temporary hooks directory is identity- and
emptiness-checked before use.  `git_environment()` already clears the child
environment, disables system/global config, terminal prompts, fsmonitor and
untracked cache, and adds only the exact selected `safe.directory`.

The later implementation may extract a small private neutral hook-confinement
helper from the reviewed-commit implementation.  It must not introduce a
generic Git executor, RefManager, or public policy abstraction.

## New authority definition

`RepositoryBranchCreationPolicy` is a private opaque **reusable** policy.
It binds one canonical selected non-bare repository identity, the current
Desktop repository/runtime generation at composition time, one canonical
host-approved native Git executable and executable identity, a policy
generation/instance, the shared per-canonical-repository mutation lease,
fixed child environment/config, one unique host-owned empty hooks directory
outside the repository with captured identity, and the closed name limits.

It is reusable for multiple separately validated calls while its Desktop
repository/runtime binding remains current.  This is deliberately unlike
ADR 0016's one-shot human-reviewed commit authorization: no human snapshot
is being consumed and each create is independently bounded to one new absent
name and freshly captured current `HEAD`.  Reuse cannot turn into arbitrary
ref mutation because every call has the same fixed namespace, one closed
logical name, fresh absence/CAS checks, and no model-supplied OID or command
field.  Repository replacement, runtime/repository generation change,
identity change, executable change, or policy disposal makes the policy unusable.

## Exerciser / model / host ownership

The model may request one logical name only.  The host alone selects and
binds repository root, Git executable, cwd, environment, config, hooks root,
lease, ref namespace, target OID, zero old OID, fixed reflog message, timeout,
output limits, and observation commands.  The frontend displays state only;
it cannot authorize.  Trusted Profile and provider metadata cannot construct
or widen this authority.  Provider profiles gain no branch capability in
v0.19.

## Proposed repo.create-branch contract

The public first-party Tool is model-facing:

```text
repo.create-branch
```

Its entire v1 JSON schema is:

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

`maxLength` is a schema guard; the authority boundary is the stricter ASCII
byte validator below.  Permission remains `PermissionLevel::Execute`, as for
existing mutation Tools, but `Execute != RepositoryBranchCreationPolicy` and
Execute alone must never create ref authority.  Effective Authority marks the
tool `repositoryBound: true`.

Tool wording is: “Create one new local branch at the repository's current
committed HEAD without switching branches. Existing staged and unstaged
changes remain in the current worktree; they are not incorporated into the
new branch.”  Do not say “create and use” or “start working on” a branch.

## Repository admissibility

V1 admits only a normal, non-bare repository with a real `.git` directory
inside the canonical selected root.  It rejects `.git` indirection/linked
worktrees, bare repositories, malformed metadata, nested/replaced repository
identity, and submodule working trees selected as the root.  A parent
repository may have a submodule elsewhere; this effect does not enter it.
Sparse checkout, shallow clone, and alternate object storage are admitted:
this operation resolves one existing commit and changes neither index nor
worktree, and Git itself owns object/ref access.  No raw `.git/refs/heads`
filesystem reads or writes are permitted.

V1 admits only the conventional `files` ref storage (loose and packed refs).
It rejects a detected `extensions.refStorage` other than absent/`files`,
including `reftable`, until deterministic evidence proves all required
observation, create-only, reflog, and collision semantics on that backend.
This is a product admission restriction, not a claim that `update-ref` is
inherently incompatible with reftable.

## Attached HEAD contract

Before authorization and again immediately before mutation, Git-owned
observations must prove: symbolic `HEAD` exists and is exactly
`refs/heads/<current>`; the exact `HEAD` OID resolves to a commit object; and
that attached branch resolves to precisely the same OID.  Detached and unborn
HEAD fail closed.  The OID is host-captured, canonical full SHA-1 or SHA-256
hex, never a request value or revision expression.  No HEAD-file,
packed-refs-file, index, or worktree fingerprint is a production precondition:
those mutable implementation details neither bind this authority nor improve
the final Git CAS.

## Dirty and staged state

Ordinary unstaged, staged, and mixed changes are allowed.  The operation does
not stage, unstage, write the index, update `HEAD`, or update the worktree, so
they do not change the exact committed target.  User/model text must state:
“The new branch points at committed HEAD. Existing staged and unstaged changes
remain in the current worktree; they are not incorporated into the new
branch.”

The policy rejects merge, cherry-pick, revert, squash-message, bisect, rebase,
and sequencer state using the same conservative markers as reviewed commit.
This is product-clarity and workflow-safety conservatism, not a claim that Git
cannot create a ref during those states: accepting it would make a later
recovery workflow appear branch-safe without an explicit product decision.

## Branch-name language

The host runs its own closed validator before any Git compatibility check.  It
accepts exactly an ASCII logical name of 1–128 bytes with 1–8 slash-separated
components, each 1–48 bytes, matching:

```text
[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?
```

Each component must match that expression.  Thus the validator rejects empty
names/components, leading dash or slash, trailing slash/dot/space, repeated
slash, dot/dotdot or any leading-dot component, `.lock` suffix, `..`, `@{`,
single `@`, controls/DEL, spaces, `~`, `^`, `:`, `?`, `*`, `[`, backslash,
non-ASCII/Unicode, and all Windows device/alias spellings by virtue of the
language (and explicitly rejects an ASCII case-insensitive `CON`, `PRN`, `AUX`,
`NUL`, `COM1`–`COM9`, or `LPT1`–`LPT9` component).  It rejects any input
beginning `refs/`, including `refs/heads/...`; the caller supplies no ref
prefix.  It preserves accepted spelling exactly and never normalizes.

After host validation, a fixed `git check-ref-format --branch <name>` is an
additional compatibility/admission check, not the authority boundary.  Git
documents that `--branch` can accept branch shorthand more broadly, so RAH's
closed language remains controlling ([git-check-ref-format](https://git-scm.com/docs/git-check-ref-format)).

## Case / namespace / packed-ref collision rules

The host constructs only `refs/heads/<name>`.  It rejects every other
namespace, including tags, remotes, notes, stash, bisect, replace, meta,
`HEAD`, `ORIG_HEAD`, and merge pseudorefs, because none can be constructed.

While holding the lease, a bounded Git-owned enumeration of *local heads only*
checks exact and ancestor/descendant prefix collisions and ASCII-case-folded
collisions.  It compares full logical components, so existing `foo` rejects
`foo/bar`, existing `foo/bar` rejects `foo`, and `Feature` rejects `feature`
on every platform.  This deliberate uniform rule prevents loose-ref case
aliases on Windows and makes Linux results portable.  Enumeration has a fixed
output/count limit and fails closed if exceeded; it does not enumerate every
repository ref.  Git observations, not loose-ref paths, cover loose and
packed refs.  A concurrent external actor can still race; final CAS and
post-observation remain mandatory.

## Native Git primitive comparison

`git branch --no-track <name> <oid>` is porcelain with branch-related behavior
and does not express expected absence as explicitly as the desired contract.
It is therefore not selected.  `git update-ref` is generic plumbing in general,
but a private fixed invocation supplies a direct host-built ref, captured new
OID, and expected absent old value without checkout or tracking behavior.
Git documents compare-and-update with an old OID and zero-old create checks
([git-update-ref](https://git-scm.com/docs/git-update-ref)).

## Selected primitive

The selected private primitive is one fixed `git update-ref` invocation:

```text
git -c core.hooksPath=<host-empty-dir> -c core.logAllRefUpdates=false \
    -c user.name=RAH -c user.email=rah@invalid \
    update-ref --create-reflog -m "RAH create local branch" \
    refs/heads/<validated-name> <captured-oid> <zero-oid-for-captured-format>
```

The actual Rust argument vector contains those literal host values only; it
uses the canonical executable directly, explicit selected-root cwd,
`env_clear`, bounded closed stdin/output, timeout/cancellation ownership, no
shell and no PATH lookup.  The zero OID is constructed as zero repeated to
the validated captured full OID length (40 SHA-1 or 64 SHA-256), never
hard-coded to SHA-1.  Supplying it as `<old-oid>` is Git's atomic
compare-and-create: a target created by another actor makes this operation
fail without overwrite.  No force, delete/recreate, relaxed retry, generic
`update-ref`, `--stdin`, symref operation, or model-supplied argument is ever
exposed.

## Hook and config confinement

`reference-transaction` is invoked by any Git command performing reference
updates ([githooks](https://git-scm.com/docs/githooks)); neither `git branch`
nor `update-ref` may be assumed hook-free.  The policy must use the reviewed
commit shape: create a unique canonical empty directory outside the repository,
capture `FileIdentity`, revalidate canonical path/identity/emptiness before
each mutating spawn, pin absolute `core.hooksPath` through host-generated
command config, and remove it on policy drop only when still the captured
empty directory.  `--no-verify` is not relevant or sufficient for this ref
operation.

Child environment disables system/global config and prompts, pins only the
selected root as `safe.directory`, and disables fsmonitor/untracked cache.
The host command config overrides local `core.hooksPath` and
`core.logAllRefUpdates`; it sets the fixed non-secret reflog identity only to
avoid consuming repository-local identity.  `branch.autoSetupMerge`,
`branch.autoSetupRebase`, commit/signing/editor settings, aliases, fsmonitor,
and untracked cache have no selected primitive behavior; branch porcelain is
not used.  Local config otherwise remains untrusted ambient input, not an
authority source.  No network or credential environment is supplied.

## Reflog contract

V1 intentionally creates a reflog using `--create-reflog` plus the fixed
message `RAH create local branch`; host config pins `core.logAllRefUpdates=false`
so local config cannot add conditional logging behavior.  The permitted
persistent effects are the one new direct local-head ref and the Git-owned
reflog metadata for exactly that ref (including needed Git-owned reflog
parent directories).  The message and identity are host-fixed, not model
controlled.  No manual `.git/logs` access, rewrite, or deletion is permitted.
Git documents that `--create-reflog` creates a log even when it normally would
not, and that ordinary ref logging is config dependent
([git-update-ref](https://git-scm.com/docs/git-update-ref)).

## Mutation lease and final revalidation

The policy uses the established per-canonical-root lease and holds it through
result construction.  It serializes RAH Stage, Unstage, commit, content/file/
directory mutation, and future branch creation, but cannot exclude external
Git, editors, sync, antivirus, or privileged processes.

Immediately before the sole spawn, while leased, it revalidates policy and
Desktop repository/runtime generation; `RepositoryIdentity`; normal `.git`
topology/ref-storage admission; Git executable identity; hooks root identity
and emptiness; special-state exclusion; attached HEAD/ref/OID/commit identity;
target language and constructed namespace; and bounded local-head absence,
prefix, and case-collision observations.  The ref CAS, rather than a
pre-check alone, is the final absence guarantee.

## Atomic compare-and-create

There is exactly one `update-ref` process launch per Tool execution after a
possible effect point.  A zero old OID makes target absence a native atomic
precondition.  A pre-existing target is never idempotent success.  RAH reduces
races through identity, lease, fresh validation, CAS, and post-observation; it
does not claim race freedom against external actors.

## Post-effect verification

Verified success requires Git-owned observation proving the target exists as
the direct `refs/heads/<name>` ref and resolves exactly to the captured OID;
symbolic HEAD remains the same attached branch; that branch and HEAD OID
remain exact; and the target reflog has the one host-fixed creation entry.
The fixed plumbing invocation structurally has no checkout, index, worktree,
or branch-tracking config operation.  Production must not scan/fingerprint all
worktree, index, config, or refs merely to re-prove that implementation fact;
deterministic fixtures prove those non-effects.  A bounded local-head
observation detects unexpected namespace/case state relevant to this policy.

## Outcome taxonomy

The closed private/public-sanitized outcomes are:

| Outcome | Meaning |
| --- | --- |
| `invalid_input` | Schema or closed-name validation failed; no attempt. |
| `precondition_failed` | Authority, generation, identity, topology, state, collision, or final validation failed before spawn. |
| `known_no_effect` | Spawn could not begin or its result was non-successful, and fresh observation proves target absent with protected HEAD state intact. |
| `branch_created_verified` | One spawn occurred and exact target/HEAD/reflog postconditions prove the authorized result. |
| `desired_state_observed_after_uncertain_attempt` | An uncertain attempt occurred; later observation sees the desired target/OID and protected HEAD state, but cannot attribute creation to RAH. |
| `uncertain` | Effect/disposition or required observation cannot establish a safe classification. |

Only validated names and the captured OID may appear in verified/desired-state
results.  Invalid input returns a bounded generic status and never echoes raw
input.

## Timeout / cancellation / uncertain effects

Validation failure before spawn maps to `invalid_input` or
`precondition_failed`.  Spawn failure maps to `known_no_effect` only after
fresh proof of absence.  A successful child plus exact postcondition maps to
`branch_created_verified`.  A nonzero exit does not by itself prove no effect.
Timeout, cancellation, disconnect, lost result, output overflow, child crash,
or observer failure triggers observation and then the taxonomy above.  There
is no automatic retry/replay and no rollback/compensation: no `branch -D`,
`update-ref -d`, loose-ref/reflog removal, or restoration is authorized.

## Review and commit authorization interaction

A verified A1 creation preserves a valid reviewed-commit authorization.  ADR
0016 binds current attached branch, old HEAD, raw/semantic index, and staged
tree; it does not bind absence of unrelated local refs.  A1 changes none of
those.  It never stages/unstages or consumes commit authority.  If current
Desktop activity plumbing contains a blanket “any repository effect revokes
review” rule, later implementation must classify this known bounded ref effect
separately rather than revoke it; external-provider conservative invalidation
does not apply.

## Generation / currentness interaction

Successful A1 does not change `repository_generation` and needs no new
branch/ref generation: selected repository identity/context, active branch,
HEAD, index, and worktree do not change.  Fresh target/ref observation under
the lease handles stale name state.  The connected runtime therefore remains
Current; no disconnect/reconnect is required.  Later Desktop integration must
avoid generic repository-effect logic that would incorrectly mark it stale.

## Conversation / provider interaction

The repository-scoped conversation identity remains valid because A1 does not
switch the active branch; no branch-specific namespace is needed.  Model,
profile, provider composition, and connection generations are unchanged.
First-party branch authority is host-composed separately from v0.17/v0.18's
provider-only Trusted Profile overlay.  A future static profile composition
would require separate authority analysis.

## Effective Authority presentation

Future Effective Authority lists public name `repo.create-branch`, source
`first_party`, permission `Execute`, and `repositoryBound: true`, with a new
sanitized authority/effect classification such as `repository_local_branch_create`.
That minimal DTO/classification addition is preferable to labelling it generic
Git or generic repository mutation.  It exposes no executable/root/hooks path,
argv, environment, ref backend, or policy generation.  V1 needs no branch
picker or manager UI; activity/result presentation is sufficient.

## Privacy / diagnostics

Public output may contain a validated logical name, already-observable commit
OID, closed status, and uncertainty flag.  It must not contain a filesystem
path, executable/hook/lock path, raw argv/environment, or raw Git stderr.
Bounded stderr is internal diagnostic evidence only.  ToolRequested has no
effect; after ToolStarted a ref may be affected; ToolFinished reconciles the
classified result.  An uncertain finish must never claim rollback or invite an
automatic replay.

## Deterministic validation matrix

Later work must add deterministic seam-driven tests, without sleeps, covering:

1. Attached normal success: target equals captured HEAD; HEAD symbolic ref/OID,
   index, worktree sentinel, existing local/tag/remote refs, and branch config
   are unchanged; no upstream is added.
2. Unstaged, staged, and mixed changes: all allowed and target committed HEAD,
   never staged tree.
3. Closed input: empty, over-limit, dash, `refs/heads`, dot/dotdot, `.lock`,
   `..`, `@{`, space/control, backslash, Git special characters, trailing
   dot/space, repeated slash, Unicode, Windows aliases, and extra properties.
4. Collisions: exact; loose and packed parent/child prefix; packed exact; and
   Windows case-fold collision (the uniform policy is also tested on Linux).
5. Admission: detached/unborn, bare, linked worktree, unsupported ref storage,
   all listed special states, repository identity/generation change, executable
   replacement, and hooks-root replacement/non-emptiness.
6. Races: HEAD change after capture, target appearing before validation and
   between observation/CAS, external collision, and lease serialization.
7. Failure: spawn failure, nonzero exit, timeout before/after mutation,
   cancellation, lost result, observer failure, output overflow, each yielding
   the closed taxonomy and exactly zero/one mutating attempt as appropriate.
8. Hooks/config: malicious repository `reference-transaction`, malicious local
   `core.hooksPath`, and a marker prove the host empty root prevents execution.
9. Reflog: exactly the permitted target log and fixed message; no unrelated
   reflog mutation or model-controlled content.
10. Integration semantics: reviewed authorization survives verified A1;
    repository/model/profile/connection generations, currentness, and
    conversation identity remain unchanged.

Fixtures must cover loose and packed refs; tests may enumerate sentinel refs
to prove no unrelated changes, unlike production's bounded focused checks.

## Windows live validation design

The release gate is a controlled host fixture, not a model-selection proof:
host-select exact `git.exe`, create a temporary normal repository on attached
`main` at commit A, install malicious reference-transaction hooks and dirty/
staged sentinel state, invoke one host-composed request, then prove
`refs/heads/task223` equals A, `HEAD` remains `main`, index/worktree sentinels
remain unchanged, no hook marker ran, no upstream exists, and owned process/
temporary-hooks cleanup is correct.  A real model-selected Tool call is only
additional integration evidence; Task 207's external-provider limitation is
not a blocker for this first-party proof.

## Linux / portability status

The same deterministic semantic contract must run on Linux, including SHA-1
and SHA-256 fixtures where available, packed refs, and uniform case policy.
No Linux live certification is claimed by this research.  ASCII-only names and
Git-owned ref operations intentionally avoid Windows-only name semantics.

## ADR-ready decision

Task 224 should create **ADR 0020 — Repository Local Branch Creation
Authority** (confirm numbering at execution).  It should accept the new
reusable private policy; `repo.create-branch`; host-only ref/OID/process
construction; attached-HEAD-only target; create-only CAS; files ref storage;
closed ASCII names; hook/config/reflog confinement; allowed dirty state;
special-state/linked/bare rejection; no review invalidation or generation/
currentness/conversation change; and no replay, rollback, remote Git, or
generic update-ref authority.

## Proposed implementation sequence

1. 223 — authority contract research (this task).
2. 224 — ADR 0020 documentation decision only.
3. 225 — private branch-creation policy foundation.
4. 226 — `repo.create-branch` Tool and host composition.
5. 227 — deterministic hook/race/failure hardening matrix.
6. 228 — Desktop activity and Effective Authority integration.
7. 229 — Windows controlled live validation.
8. 230 — v0.19 milestone audit; 231 — release preparation.

## Explicit non-goals

No Rust/API/Tool/profile/Desktop implementation is authorized here.  No
checkout/switch/detach, arbitrary refs or revisions, tags/remotes, tracking,
force/overwrite/rename/delete, merge/rebase/cherry-pick, commit/index/worktree
mutation, generic shell/process/Git, remote/network Git, rollback/replay,
reftable support, branch UI, or Trusted Profile capability addition is in
scope.

## Next task

**Task 224 — ADR 0020: Repository Local Branch Creation Authority.** It is a
documentation/architecture decision only and must not implement
`repo.create-branch`.
