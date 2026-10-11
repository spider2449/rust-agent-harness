# RAH v0.3 Mutating Execute Authority Security Research

Status: Research complete; implementation is not authorized by this document

Date: 2026-08-22

## Executive answer

Intentional mutation of a host-authorized repository is a distinct authority
from read-oriented `PermissionLevel::Execute`. The existing Execute foundation
is a necessary process-construction boundary, but it does not say *which
repository state may change*, prove the actual effects, serialize competing
writers, or resolve an operation whose external side effect is uncertain.

The appropriate authority stack is:

```text
untrusted model ToolCall
  -> registered capability-specific Tool
  -> PermissionLevel::Execute
  -> immutable HostExecutionPolicy
  -> host-owned RepositoryMutationPolicy
  -> repository mutation lease
  -> pre-state capture and authorization binding
  -> supervised direct process / deterministic fixture
  -> post-state verification and host audit record
  -> bounded, relative-path ToolOutput
```

`PermissionLevel::Execute` should remain the broad public permission. A
repository mutation policy should initially be a private, host-owned policy in
`rah-tools`, with the repository-specific process details kept behind its tool.
`rah-sandbox` may provide an internal primitive later if several tools need it,
but it must not become a generic public mutation authority or a claim of OS
isolation.

The final recommendation is **B. PROCEED WITH DETERMINISTIC MUTATION FIXTURE
FIRST**. The first fixture should use a host-selected isolated temporary
repository/workspace, one host-selected `marker` target, and exact deterministic
content. It must prove the authorization, pre/post verification, audit,
serialization, no-replay, cancellation, and uncertain-effect rules before RAH
uses real Git mutation. It must not be registered as a general model-visible
write capability outside its controlled test/experimental composition.

After that validation, the first real Git capability should be `host.git.stage`,
but only for a host-owned symbolic target mapped to one exact, existing,
regular, tracked, non-submodule repository-relative file. It should execute a
literal-pathspec Git invocation selected by the host. `git commit`, all ref and
network mutation, arbitrary model paths, file-content writing, and support for
submodules, linked worktrees, or nested repositories remain out of scope.

## 1. Current read-oriented Execute foundation

ADR 0009 establishes that `PermissionLevel::Execute` is necessary but not
sufficient. `HostExecutionTool` and its immutable `HostExecutionPolicy` already
provide the following material guarantees for a capability-specific process:

- A runtime or Generic Codex Tool Bridge separately checks the tool's required
  `PermissionLevel::Execute`; a model request never grants it.
- Trusted host construction selects an absolute canonical native executable and
  captures its identity. The policy revalidates the executable before spawn.
- The argv policy is exact or typed. Model input cannot select a program, raw
  argv, shell string, cwd, environment, or timeout.
- The cwd is host-owned, canonicalized beneath a host-owned canonical root, and
  revalidated. Child environment is cleared and reconstructed only from trusted
  entries. Stdin is closed.
- Timeout is fixed by policy. Stdout/stderr are concurrently drained under
  individual and combined bounds, while serialized output is separately bounded.
- The supervised process layer owns the direct child, attempts termination on
  timeout, overflow, cancellation/drop, and reaps it. Windows Job Object and
  Unix process-group support are best effort, not OS isolation.
- `host.cargo.version` is an exact no-input capability. `host.git.status` is
  likewise fixed to `status --porcelain=v1`, with a trusted repository root and
  `.git` representation captured and revalidated before direct execution.
- `host.git.status` clears environment, disables system/global config and
  prompting/optional locks, and overrides fsmonitor and the untracked cache.
  It deliberately does not call generic shell execution.
- The Generic Codex Tool Bridge snapshots definitions, checks current tool and
  permission again before dispatch, correlates calls privately, emits RAH tool
  lifecycle events only for RAH execution, and deduplicates/reuses one response
  for duplicate dynamic requests. Its configured Codex approval policy remains
  `never`; Codex approvals are not authorization.

These controls intentionally constrain *process construction*. They do not
constrain the consequences of an allowed executable with access to a repository:
an exit status of zero cannot show that only authorized paths changed; a timed
out or disconnected caller cannot know whether Git already changed the index;
and repository state can change between a validation and a later spawn.
`host.git.status` is read-oriented and even it does not claim side-effect-free
behavior or protection from repository-local configuration. Mutation therefore
needs a second host-owned policy, state observations, and effect verification.

## 2. Mutation authority model

`RepositoryMutationPolicy` is the smallest needed abstraction. It is immutable,
constructed only by the trusted host, associated with one capability, and never
created from model JSON. It adds these decisions to `HostExecutionPolicy`:

- repository identity and allowed repository layout;
- mutation class and exact authorized targets;
- whether preconditions must match a captured state;
- required pre/post observations and permitted deltas;
- mutation lease/serialization scope; and
- audit retention and model-visible redaction rules.

It should remain internal to `rah-tools` in the initial design because it is Git
and repository-semantics specific, not a universal RAH boundary. A later common
internal `rah-sandbox` helper is reasonable only after more than one tool needs
the same host-side lease or file-identity implementation. Neither choice
requires modifying `Tool`, `ToolRegistry`, `ToolContext`, `AgentRuntime`,
`Sandbox`, or protocol DTOs today.

`Write` and `Execute` remain separate permissions. A filesystem-content tool
that edits a host-authorized file requires `Write` and its own workspace policy.
A Git-index/ref tool invokes a process and requires `Execute` plus this mutation
policy. A workflow that first writes content and then stages it needs both
independently authorized tools; `git add` only stages pre-existing content and
must not implicitly authorize creating or editing it. Do not redefine either
permission or add a new public `PermissionLevel` for this spike.

## 3. Repository and mutation scope

The policy must name one canonical repository identity, not merely a path. It
must revalidate the root and `.git` directory-or-file representation using the
existing `GitStatusTool` design before every mutation phase. Its scope must
separately state whether it authorizes:

| State plane | Initial fixture | First `host.git.stage` | Future separate authority |
|---|---|---|---|
| Worktree content | One fixture-owned marker only | None | file mutation policy |
| Git index | Fixture may simulate/verify only | One symbolic target | deletions, intent-to-add, bulk staging |
| Repository metadata | None | Git's necessary index lock only | config, worktree metadata |
| Refs/history | None | None | commit, merge, rebase, reset |
| Object database/reflog | None | None | commit and ref capabilities |
| Remote/network | None | None | fetch/pull/push policy |

The initial real capability must reject a repository containing a submodule,
gitlink, nested repository in the target subtree, linked-worktree `.git` file,
or unsupported sparse-checkout/worktree arrangement. It should also reject a
target with a symlink/junction/reparse-point component. These restrictions are
intentional narrowing, not a claim that all Git repositories are supported.

The policy must make separate boolean decisions for new files, deletions,
tracked-only scope, worktree mutation, index mutation, refs/history, local
configuration, object database writes, and network access. An omitted decision
is deny. The first real staging capability permits only index mutation for one
pre-existing tracked regular file; it must not permit new files, deletions,
renames, mode changes, submodules, refs, config, object writes beyond the index,
or network access.

## 4. Path and pathspec authorization

Model input must never be an absolute path or raw Git pathspec. When a path-like
input is eventually accepted, it must be repository-relative, separator
normalized, non-empty, NUL-free, reject `.`/`..` traversal and absolute/drive,
UNC, verbatim (`\\?\\`) and alternate-data-stream forms, and be compared using
the filesystem's case behavior. Existing parents must be canonicalized and
proved within the canonical repository root. A non-existing final component
requires parent canonicalization plus an explicit policy decision; the first
real prototype avoids this risk by allowing only pre-existing regular files.

Windows must treat case-folded aliases, short/long names, junctions, symlinks,
and reparse points as possible escapes. RAH must compare canonical identities,
not only strings, while presenting normalized repository-relative names. A
colon in a Windows path must not be accepted as a Git pathspec or NTFS ADS.

Git pathspec is command-language input: magic prefixes, leading `:`, globbing,
attributes, excludes, `--pathspec-from-file`, and option-like values can change
the selected set. The model must not supply it. The host should resolve a
symbolic target such as `tracked-file-1` to a single audited literal relative
path and construct the fixed argv itself. Use `--` and Git literal-pathspec
controls (for example `--literal-pathspecs` in the fixed host environment or
equivalent fixed command behavior) so the exact host path is not parsed as
pathspec syntax. Never use model strings with pathspec expansion.

## 5. Pre-state, post-state, and mutation boundary

Before obtaining the mutation lease, capture an opaque host-side record of:

- canonical repository/root and `.git` identities;
- HEAD symbolic/ref and object state, relevant refs, and worktree arrangement;
- index identity/state and any relevant lock-file state;
- a porcelain status snapshot with paths parsed as data, not displayed raw;
- authorized target file identity, mode, type, tracked/blob/index metadata, and
  a content hash where practical; and
- executable identity, fixed argv, environment profile, timeout, and policy
  version/identity.

Hold a host-owned per-repository mutation lease from pre-state capture through
post-state verification. This serializes RAH mutations to that repository. It
cannot stop an external process from changing it; compare post-state against the
pre-state and report an external-concurrency/uncertain violation instead of
claiming a reliable authorization decision. Read-only status should not run
concurrently with a mutation by default: it can observe an index lock or mixed
state and its current `GIT_OPTIONAL_LOCKS=0` setting only reduces its own locks.
Permit concurrent reads only after their result is explicitly tagged as an
unverified observation and never used as the mutation proof.

After a staging operation, recapture repository identity, HEAD/ref state, index
state, status, and authorized target metadata. The required postcondition is:

```text
authorized target's index entry changed in the requested way
AND no non-authorized index path changed
AND no worktree path changed by the operation
AND HEAD and refs did not move
AND repository identity remains valid
```

The verifier must compare complete relevant path sets, not infer success from
Git's exit code. An unauthorized path, moved HEAD/ref, changed worktree, lost
identity, leftover lock, incomplete observation, or conflicting external change
is a mutation-boundary violation. Return a bounded failure/uncertain result,
preserve the audit record, and do not automatically roll back.

## 6. Rollback, uncertainty, and cancellation

RAH must not promise rollback. Git may atomically replace an index only where
Git itself guarantees that behavior, but index restoration can race another
writer; worktree restoration can overwrite newer user edits; and ref/object or
reflog restoration is a separate destructive operation. Lock files, partial
writes, a crash, or a process-tree escape make inferred rollback unsafe.

The terminal state must distinguish:

- `ok`: postconditions were fully verified;
- `rejected`: no spawn occurred because policy/precondition validation failed;
- `failed_known`: process result and post-state were observed, but no permitted
  mutation was verified (or an observed violation was detected);
- `uncertain`: a side effect may have occurred but completion/postcondition
  verification is incomplete or contradictory; and
- `partial`: an observed state changed but does not meet the authorized complete
  postcondition. `partial` and `uncertain` may both be true.

Exactly-once bridge dispatch only limits RAH's own submission. It does not prove
exactly-once external effect. The following all become `uncertain` unless a
complete post-state check proves otherwise: spawn succeeds but response is lost;
the child exits then bridge disconnects; a timeout fires after Git completed;
cancellation races index mutation; a host crashes after `ToolStarted`; a
duplicate arrives after retained bridge state is lost; or the model asks a
semantically equivalent new call. None may be automatically replayed. A new
attempt requires a new host-authorized call, a new lease, fresh preconditions,
and an operator-visible audit trail.

Cancellation before spawn is a safe `rejected/cancelled` outcome. During or
after spawn it means only a termination attempt. The current `ToolContext` has
no cooperative cancellation token; runtime/bridge task abortion relies on the
supervised process layer's drop cleanup. That is acceptable for the fixture
only if its result is conservatively `uncertain` when the mutation boundary was
crossed or cannot be checked. It is not confirmation that repository state was
reverted, including where descendants were spawned.

## 7. Approval, audit, and output

For v0.3 use host-preauthorized, capability-specific mutation only. Per-call
interactive approval is deferred because current runtime approval response
semantics do not provide a complete controlled approval transaction, and Codex
`approvalPolicy=never` must not be weakened. Codex approval cannot authorize a
RAH mutation under any model.

The host audit record should include capability and policy identity, repository
identity, symbolic targets and authorized relative paths, pre/post-state digests,
canonical executable identity, exact argv, environment profile identifier,
lease timings, start/finish time, exit/termination metadata, changed paths,
timeout/cancellation flags, verification outcome, partial/uncertain flags, and
host-only diagnostics. It should avoid secrets and raw sensitive absolute paths
in both audit exports and model-visible content.

Existing `ToolOutput` plus `ToolContent::Json` is sufficient. A successful
bounded model-visible result can be:

```json
{
  "status": "ok",
  "authorized_paths": ["tracked.txt"],
  "changed_paths": ["tracked.txt"],
  "partial": false,
  "uncertain": false
}
```

Failures must use the same neutral shape with a safe reason class; sensitive
paths, raw Git configuration, credentials, and host diagnostics stay host-side.

## 8. Git-specific analysis

### `git add`

`git add -- <literal host-selected paths>` is materially narrower than generic
Git, but it still mutates the index and can interact with ignored files,
attributes/filters, sparse checkout, submodules, nested repositories, file mode
changes, deletions/renames, index extensions, and repository configuration.
`git add` does not normally invoke commit hooks, but that fact does not make it
safe: filters and other config can execute or transform data. The first real
prototype must avoid `-u`, intent-to-add, all/pathspec-from-file options, and
new/deleted/mode-changing files. It must disable system/global config, prompts,
optional locking, fsmonitor, untracked cache, and behavior that can invoke
external helpers. It must not accept aliases (direct canonical Git executable
with fixed `add` argv avoids alias lookup).

Repository-local configuration remains untrusted authority. The policy must
explicitly neutralize or reject relevant local settings, including includes and
includeIf, `core.hooksPath`, filters/attributes, fsmonitor, credential helpers,
signing, external commands, and any configuration that can invoke another
program, change the index semantics, or trigger network/credential use. Keeping
Git safe.directory checks enabled is correct: a dubious ownership failure must
fail closed, not be bypassed by a model or a broad `safe.directory` setting.

### `git commit`

Commit is a separately dangerous capability and must be deferred. It writes
objects, alters refs and reflogs, can alter index state, and depends on author/
committer identity, timestamps, templates, editor behavior, signing, config,
GPG/SSH agents, and hooks. `pre-commit`, `prepare-commit-msg`, `commit-msg`, and
`post-commit` can run arbitrary host code; related checkout/merge/rewrite hooks
become relevant in later history-changing workflows. A future commit policy
must explicitly disable or specifically host-authorize hooks and must never
inherit arbitrary `core.hooksPath`. It requires distinct ref/history, object,
identity, message, signing, and hook authority; staging authority is insufficient.

Network Git (`fetch`, `pull`, `push`) is not local mutation and is excluded.
It needs another policy for remote identity/URL, credentials and agents, branch
and ref scope, force behavior, proxy, and network containment. Do not combine
it with the first mutation phase.

## 9. Deterministic fixture and later real prototype

Design `host.fixture.stage-marker` as a repository-owned test capability, not a
general file editor. Host setup creates an isolated temporary repository or
workspace, selects a marker file and an exact marker value, and registers the
capability with no model-controlled path or content. A single invocation may
perform one bounded deterministic mutation. Its tests must demonstrate the full
pre/post records, authorized-path-only proof, no replay, cancellation/timeout
uncertainty, audit redaction, lease behavior, and detection of an injected
unauthorized mutation. It needs the same minimum private mutation-policy shape
defined above, rather than a special-case bypass.

The later `host.git.stage` should expose symbolic targets, not `paths`:

```json
{ "target": "tracked-file-1" }
```

The host maps that immutable target identity to one exact allowed relative path
captured during policy construction. This avoids path parsing, case equivalence,
and pathspec ambiguity at the model boundary. A multi-path array can be evaluated
later only after all of those rules are proved for each element and the complete
set has an atomic, verifiable postcondition.

## 10. Required implementation test plan

The eventual fixture and staging implementation need deterministic tests for:

- denied target, traversal, absolute/UNC/verbatim/ADS forms, separator and
  Windows case normalization, symlink/junction escape, and pathspec magic;
- pre-state capture, stale-state rejection, repository/executable revalidation,
  only-authorized mutation, unauthorized-delta detection, and result redaction;
- fixture timeout/cancellation after side effect, dropped bridge response,
  duplicate dynamic call, no automatic replay, and uncertain/partial outcomes;
- per-repository serialization, lock contention, and externally injected
  concurrent mutation;
- literal pathspec behavior, ignored files, sparse checkout, mode changes,
  submodules, nested repositories, linked worktrees, and `.git` file layouts;
- disabled/not-relevant hooks, minimized configuration/environment, no network,
  no credentials, and no secret leakage; and
- audit record completeness and no public-contract changes.

The normal suite must use temporary repositories and fixture executables; it must
not require a live model, credentials, network access, or a user repository.

## 11. Public contract and ADR decision

No listed public contract needs to change for the fixture or first narrow staging
capability: `AgentRuntime`, `Tool`, `ToolRegistry`, `ToolContext`,
`ToolDefinition`, `ToolCall`, `ToolOutput`, `ToolError`, `PermissionLevel`,
`AgentEvent`, and `Sandbox` are adequate when the policy, lease, verification,
and audit plumbing remain capability-internal. Current cancellation limitations
require conservative uncertainty reporting, not a public API change.

Before implementation, add `docs/adr/0010-repository-mutation-policy.md`. It
should record the layered authority, scope planes, symbolic-target rule,
per-repository serialization, mandatory pre/post verification, no rollback
promise, uncertain-effect/no-replay semantics, and the fixture-first sequence.
This research task does not create that ADR.

## 12. Final recommendation

**B. PROCEED WITH DETERMINISTIC MUTATION FIXTURE FIRST**

Minimum abstraction before that fixture: a private immutable
`RepositoryMutationPolicy` associated with a capability, plus a host-owned
per-repository lease and a verifier that produces `ok`, `rejected`,
`failed_known`, `partial`, or `uncertain` outcomes from captured pre/post state.
It must compose with the existing `PermissionLevel::Execute`,
`HostExecutionPolicy`, supervisor, and bridge deduplication; it must not add a
new permission level or alter public RAH contracts.

Remaining risks are residual OS ambient authority, races with non-RAH processes,
platform-specific path and file-identity behavior, and Git's broad local
configuration surface. The fixture is intended to validate RAH's authority and
reporting model before any attempt to narrow those risks for real Git staging.
