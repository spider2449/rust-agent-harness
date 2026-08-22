# RAH v0.3: next repository-mutation authority

Status: Research and implementation recommendation only
Scope: after verified `host.git.stage`; no capability is implemented here.

## Decision

**Recommend `host.git.unstage` for one host-owned target.**

It is the smallest useful new mutation authority: replace exactly one
host-bound target's index entry with its `HEAD` entry. It preserves worktree
bytes, `HEAD`, refs, reflogs, and the object database. It corrects an
accidental prior stage without granting file overwrite or history creation.

The existing layers remain unchanged:

```text
PermissionLevel::Execute
  + HostExecutionPolicy (canonical Git, fixed environment/cwd/argv/timeout)
  + private RepositoryMutationPolicy (host target, lease, state proof, audit)
```

The tool call remains a request. The host supplies the repository, executable,
symbolic target, literal relative path, and every command argument.

## Baseline

`GitStageTool` already provides the required local pattern: empty schema,
per-repository lease, identity revalidation, complete index snapshot,
worktree snapshot, `HEAD`/ref observation, and conservative
`ok`/`failed_known`/`policy_violation`/`uncertain` results. Reuse that private
pattern; do not introduce a generic Git abstraction or public policy API.

Git documents that `restore --staged` restores the index and defaults to
`HEAD`; the proposed argv makes `HEAD` explicit. `restore` without `--staged`
instead restores the worktree. See the [official git-restore
documentation](https://git-scm.com/docs/git-restore).

## Candidate comparison

| Candidate | Exact additional authority | State planes it may mutate | Decision |
| --- | --- | --- | --- |
| **`host.git.unstage`** | Set one target index entry to `HEAD`. | Index and temporary index lock. | **Recommend.** Smallest useful delta; preserves user bytes and history. |
| `host.git.restore-worktree` | Overwrite one target's worktree bytes from index/`HEAD`. | Worktree; transient metadata/locks. | Defer: destructive content authority. |
| `host.git.commit` | Record a snapshot and advance a branch. | Object DB, refs/`HEAD`, reflogs, commit metadata; hooks may add effects. | Defer: new history and ambient-process authority. |
| `host.git.intent-to-add` | Add one intent-to-add index entry. | Index. | Defer: requires new/nonexistent target policy; little corrective value. |
| `host.git.refresh-index` | Refresh stat cache for a target. | Index metadata. | Reject: technically smaller but not meaningful model-facing functionality. |

### `host.git.unstage`

**Authority and state.** Trusted construction maps one symbolic target to one
existing regular file inside one canonical repository. The target must be
tracked in both the index and `HEAD`, with one normal stage-0 entry. The tool
may change only that index entry and Git's necessary temporary index lock. It
cannot select a revision, deletion, other path, worktree file, ref, remote, or
configuration override. If the entry already equals `HEAD`, it is a verified
no-op.

**Rollback, uncertainty, replay.** No rollback is promised. A later stage is
not rollback because it stages current worktree bytes, which may differ from
the discarded staged bytes. Post-spawn timeout, cancellation, bridge loss, or
process-result loss is `uncertain` unless complete post-state proves otherwise.
Never retry or replay; a later attempt needs a new call, lease, and pre-state.

**Verification and target feasibility.** The existing symbolic target remains
ideal: no model path or pathspec crosses the boundary. Pre/post capture adds a
`HEAD` tree entry for the fixed path. Success requires the post-index target to
equal that captured entry, not merely differ from pre-index. Complete index,
worktree, `HEAD`, refs, repository, and executable observations make a strong
single-target proof feasible.

**Configuration, hooks, credentials, and network.** `git restore --staged` is
local and needs neither remote nor credentials. No `git-restore` hook is
documented in Git's hooks reference, but that is not an OS sandbox claim.
Preserve the cleared, fixed Git environment; keep system/global config and
prompting disabled and `safe.directory` fail-closed; reject unsupported layouts
or configuration-dependent conditions. No generic shell/process is exposed.

**Windows.** Retain canonical identity and case-aware comparison; reject
symlinks, junctions, reparse points, absolute/UNC/verbatim/ADS model paths, and
non-native Git executables. Empty input means pathspec magic and separator
aliases never cross the model boundary. Tests must cover sharing/lock errors
and identity changes conservatively.

**Testing/live validation.** Deterministic temporary repositories can stage the
target, preserve independent staged/unstaged files, execute once, and prove
target index=`HEAD` while worktree bytes are unchanged. A separate opt-in live
Codex fixture is feasible with one `{}` tool, an owned local repository, and no
network or credentials.

### `host.git.restore-worktree`

`git restore` without `--staged` writes the working tree from the index by
default, or from an explicit source. Even fixed-source/fixed-target use does
not move history, but it overwrites local user bytes and can remove a tracked
file missing in its source. Git documents those destination and removal
semantics in [git-restore](https://git-scm.com/docs/git-restore).

It has worse rollback and uncertain-effect semantics: restoring previous local
bytes would be a second destructive, race-prone write and cannot be automatic.
It needs a deletion policy, source policy, byte/identity proof, read-only-file
handling, and a recovery/audit design. Windows adds sharing violations,
antivirus locks, case aliases, junctions, and atomic-replace issues. It has no
inherent network/credential requirement, but it is a larger worktree-content
authority increment. Defer; no ADR is needed merely to defer it.

### `host.git.commit`

Commit is not the natural follow-up to stage. It creates commit/tree objects,
moves the current branch and `HEAD`, writes reflogs, and records identity and
time. It also brings author/committer policy, message/template/editor behavior,
signing and agents, exact staged-set policy, hooks, ref locks, and object
retention. Git's [commit](https://git-scm.com/docs/git-commit) and
[hooks](https://git-scm.com/docs/githooks) documentation is the authoritative
future design source.

Hooks can run arbitrary host programs and may use credentials or the network;
therefore a local Git executable does not contain commit authority. Although
pre/post observations can prove parent-to-child ref/tree transitions, timeout
or cancellation cannot make object/ref writes reversible. Windows also adds
hook interpreter, signing agent, executable-resolution, file-lock, and
credential-manager exposure. Defer. A new ADR is required if commit is later
proposed because it adds history/ref/object and hook security authority.

### Smaller alternatives

Prefer `restore --staged` over a path-limited `git reset HEAD -- <path>`:
Git documents that path-limited reset updates staged versions, while its other
forms can move `HEAD` and affect index/worktree. A fixed argv could constrain
it, but `restore` communicates the narrower intent directly. `intent-to-add`
stays index-only but introduces absent/new-target handling; `refresh-index`
writes only stat data and adds no useful agent workflow.

## Proposed exact model and argv

Tool name: `host.git.unstage`.

```json
{"type":"object","properties":{},"additionalProperties":false}
```

Trusted constructor only:

```text
GitUnstageTool::new(git_executable, repository_root, symbolic_target, target_path)
```

The model never supplies a revision, pathspec, argv, cwd, environment, timeout,
or configuration override. The sole mutating argv is:

```text
git --literal-pathspecs restore --staged --source=HEAD -- <host-owned-relative-target>
```

It uses the existing canonical native Git executable, canonical repository cwd,
closed stdin, fixed Git environment, output bounds, and timeout supervision.

## Required pre/post invariants

Hold the existing per-repository mutation lease across all of this work.

Before spawn:

1. Revalidate canonical root, `.git`, executable, target/parent/file identities,
   and absence of links/reparse points.
2. Reject target absent from `HEAD` or index, unmerged/multiple-stage entries,
   gitlinks/submodules, non-regular files, sparse/skip-worktree entries,
   linked-worktree layout, lock contention, or incomplete/stale observation.
3. Capture complete index; `HEAD`; all refs; target's exact `HEAD` tree entry;
   bounded worktree snapshot and target bytes; relevant lock state; and
   host-only policy/executable/environment audit data.

After spawn, recapture and accept success only when:

```text
post index[target] == pre HEAD-tree[target]
AND every other index entry is unchanged
AND complete worktree snapshot and target bytes are unchanged
AND HEAD and every ref are unchanged
AND repository, executable, and target identities remain valid
AND no residual lock or incomplete observation remains
```

`ok` requires a changed target index and all conditions. `ok` plus
`no_op=true` is allowed only when target already equalled `HEAD` and every state
plane remained equal. Any unapproved observed delta is
`policy_violation`/`partial`; incomplete/contradictory post-observation or a
post-spawn timeout/cancel/lost result is `uncertain`. Neither is retried,
replayed, or automatically rolled back.

## Authority delta from stage

`host.git.unstage` retains Execute, host-owned Git/repository/target, literal
pathspec, lease, verification, and no-network/no-replay guarantees. The only
new authority is **discarding one existing staged index entry by replacing it
with `HEAD`, while retaining the worktree**. It adds no worktree write,
ref/history, reflog, object DB, remote, credential, or generic-process
authority.

## ADR decision and implementation recommendation

**ADR 0011 is not warranted.** This is a capability-local index-only inverse
of ADR 0010 staging with no public-boundary or security-model change. Add an
implementation plan and update `docs/SECURITY.md` when work begins. Write an
ADR before worktree replacement, refs/history, objects/reflogs, hooks,
signing/identity, credentials, or network authority—especially commit.

Implement `GitUnstageTool` as a sibling of `GitStageTool`; extract private
state-observation helpers only when that reduces duplicated proof logic without
changing public APIs. First cover success, verified no-op, malformed input,
stale identity, unrelated staged/unstaged preservation, target-worktree
preservation, lease serialization, injected unrelated delta, and
timeout/lost-result uncertainty in deterministic temporary-repository tests.
Put live Codex bridge validation in a later, separate opt-in change.
