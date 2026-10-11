# Task 158 — Bounded Repository File Deletion Authority Research

**Research and plan only.** This document recommends a contract for a later ADR
0017. It neither accepts that ADR nor authorizes implementation, a Tool, profile
schema, bridge, Desktop control, Cargo change, or release work.

## Current-state evidence

- Task 157 selects tracked-file deletion as the first v0.13 structural-authoring
  candidate and requires a new authority.
- ADR 0010 requires a repository lease, pre/post evidence, redacted results, and
  no replay after uncertain effects; it does not grant worktree deletion.
- ADR 0011 permits Trusted Profile only to compose already approved,
  host-constructed capabilities into a fresh ToolRegistry; it cannot create an
  authority class.
- ADRs 0012, 0013, and 0014 separate tracked content replacement, exclusive
  creation, and multi-file replacement. Each excludes deletion, links/reparse
  points, unsupported Git state, auto-stage, and replay.
- ADR 0016 separately reserves history/ref authority for a host-reviewed,
  one-shot repo.commit. Its index and commit authority cannot be inferred from
  authoring authority.
- docs/ARCHITECTURE.md and docs/SECURITY.md preserve the sequence worktree
  authoring -> human Stage/Unstage -> staged review -> human authorization ->
  repo.commit, under the shared repository mutation lease.
- The present tools contain repo.patch, repo.create-file, and repo.edit-files;
  no RepositoryFileDeletionPolicy or repo.delete capability exists. Existing
  creation and patch code demonstrates canonical-root, Git-observation,
  reparse rejection, and shared-lease boundaries, but is not deletion authority.

## Threat and authority analysis

Deleting a pathname is persistent removal of its directory entry. It can erase
unreviewed bytes even though it neither writes a replacement nor changes the
index. It is therefore a separate destructive worktree authority:

```text
RepositoryWorktreeMutationPolicy  != content replacement
RepositoryFileCreationPolicy      != exclusive pathname allocation
RepositoryFileDeletionPolicy      == one protected pathname removal
RepositoryMutationPolicy          != index mutation
RepositoryCommitPolicy            != history/ref mutation
PermissionLevel::Execute          != any of the above authorities
```

The authority must be host-owned and additive. A model call only supplies a
bounded request; it cannot select the repository, native primitive, cwd,
environment, Git argv, policy generation, parent directory, or recovery. The
shared per-repository mutation lease serializes RAH operations but cannot
exclude editors, antivirus, Git, or another local process. Every contradiction
therefore fails closed.

### Exact capability boundary

“Delete one tracked repository file” means: during one accepted call, remove
the worktree directory entry for exactly one explicitly named, existing,
ordinary, non-link regular file under one host-selected non-bare repository.
The file must be tracked by current HEAD, clean in index and worktree, and still
be the exact pre-authorized source. The only intended durable repository effect
is an unstaged deletion of that one worktree path.

It does not delete a directory, remove contents recursively, follow a link,
remove an untracked path, mutate .git, stage, unstage, commit, change a ref, or
invoke Git to effect the removal.

## Decision matrix: target and repository state

| State at preflight/final revalidation | Decision | Reason |
| --- | --- | --- |
| HEAD-tracked ordinary regular file; index equals HEAD; worktree equals HEAD | Admit | Minimum useful protected deletion. |
| HEAD-tracked file with unstaged modification | Refuse | Would erase unreviewed work. |
| Any staged modification or index divergence | Refuse | Preserve the human index boundary and staged work. |
| Already staged deletion | Refuse | Target is absent or index is divergent; deletion is not stage reconciliation. |
| Untracked or ignored file | Refuse | No repository-owned source proof; no arbitrary deletion. |
| Intent-to-add | Refuse | Not a normal HEAD-tracked stage-0 entry. |
| Conflict/unmerged index entry | Refuse | Target ownership and source are ambiguous. |
| Sparse checkout/index, skip-worktree, assume-unchanged | Refuse | Worktree and index semantics are not proven for v1. |
| Submodule/gitlink or nested repository boundary | Refuse | It is not a regular file and has independent repository authority. |
| Detached/unborn HEAD, linked worktree, bare repo, merge/rebase/cherry-pick/revert/sequencer/bisect state, alternate index or unreadable/malformed Git observations | Refuse | v1 uses the ordinary selected-repository identity model and fails closed. |

Unrelated dirty worktree files are permitted only if the target, index, HEAD,
and required repository identity observations remain exact. The capability must
not claim whole-worktree stability or inspect/expose unrelated content.

## Recommended deletion contract

### Binding and preconditions

Construct a private host-owned RepositoryFileDeletionPolicy with the canonical
repository root and identity, fixed native deletion implementation, fixed
limits, and the existing shared repository mutation lease. Bind one accepted
operation to:

1. selected canonical repository identity and policy generation;
2. current runtime and generation, so a stale runtime/tool registry cannot use a
   prior repository context;
3. an attached current branch and exact HEAD OID (reject detached/unborn);
4. the exact normal stage-0 index entry and a raw/canonical index observation
   proving it equals the target's HEAD tree entry;
5. one canonical logical repository-relative path; and
6. raw source byte length and SHA-256 supplied as request preconditions, freshly
   compared with the worktree source and HEAD blob bytes.

The strong v1 decision is **target identical to the HEAD blob**, not a merely
model-supplied expected worktree preimage. The request digest/length makes a
stale call fail deterministically; the independent HEAD equality rule prevents
the model from authorizing deletion of newer human edits by echoing their hash.
A later “explicitly authorized expected worktree preimage” variant would need a
separate host-captured, opaque, short-lived authorization object and new ADR
scope; model-provided hashes alone cannot supply it.

Capture target, parent, root, Git/index/HEAD/ref observations under the lease;
validate them again immediately before the native effect; then verify
deterministic postconditions. No claim of cross-process atomicity or TOCTOU
freedom follows.

### Narrow Tool schema and result

The recommended model-visible request is exactly:

```json
{
  "path": "src/obsolete.rs",
  "expected_file_sha256": "lowercase-64-hex-sha256",
  "expected_file_byte_length": 123
}
```

It rejects unknown/missing fields, NUL, non-UTF-8 or oversized serialized input,
invalid digest/length, and any second target. The Tool name is proposed as
repo.delete-file (final label is for ADR 0017), with a description limited to
deleting one clean HEAD-tracked repository file. There are no force, recursion,
glob, cwd, environment, Git argv, shell command, or native-path fields.

Results are bounded and redacted: status, accepted logical path when safe, and a
minimal effect classification only. They never expose absolute/native paths,
raw source bytes/digest, parent identities, handles, temporary data, Git
diagnostics, or OS error text.

| Status | Meaning |
| --- | --- |
| invalid_input | Request did not satisfy the closed schema. |
| precondition_failed | No native deletion attempt; identity, source, Git state, or supported-state proof failed. |
| deleted_verified | Exactly the authorized path is absent; index/HEAD/branch/ref observations remain unchanged and index still records the HEAD entry. |
| known_no_effect | One attempt may have been reached, but fresh proof establishes the protected source remains intact. |
| uncertain | A deletion may have happened but required proof is incomplete or contradictory. |

### Path, file-type, and Windows containment

The model path uses / separators and nonempty normal components only. Reject
absolute, drive-relative/drive-qualified, . and .., empty, backslash, colon,
ADS, UNC, verbatim (\\?\\), device namespace, wildcard/glob characters, and
case-insensitive .git components. Do not rely on raw string equality for
identity or case distinction.

Before capture, immediately before effect, and after observation as applicable,
verify canonical root and every parent as a real directory beneath that root.
Reject symbolic links, junctions, all reparse points, mount-like redirection,
special files, directories, and metadata paths. Use handle-based native identity
checks on Windows and reject aliases/case-fold collisions rather than choosing
one. Reject hard-linked targets when link count is observable and not exactly
one: deleting one link would preserve bytes through another name and defeats the
simple “one file removed” result claim. Unsupported or unreliable link-count
observation fails closed.

The future Windows implementation must use a Unicode, handle-based native
delete-disposition primitive on the validated target handle, not shell, Git,
std::fs::remove_file by a re-resolved string, or a generic filesystem tool. The
tested primitive must reject reparse-point following and define behavior for
sharing violations, read-only attributes, ACL denial, ADS, filter drivers, and
delete-pending handles. No attribute/ACL repair is authorized.

### Effect and commit-point semantics

The native delete-disposition call on the fully revalidated target handle is
the sole filesystem commit point. The policy makes exactly one such native
deletion attempt. On Windows a successful disposition can make deletion pending
while another handle delays name disappearance; the operation is not
deleted_verified until deterministic post-observation proves the intended
pathname absent. That delayed state is a possible effect, never an excuse to
retry.

Before the commit point, cancellation, validation failure, temporary/handle
preparation failure, or failure to acquire a safe primitive is precondition_failed
or known_no_effect only if fresh observation proves the complete protected
preimage remains. At and after the commit point, timeout, cancellation,
disconnect, crash, lost native result, sharing/filter failure, observer failure,
or any mismatch is uncertain unless post-observation proves either intact
preimage (known_no_effect) or exact removal (deleted_verified). A native error
code or return alone is insufficient.

There is no automatic retry, replay, restore, rollback, cleanup deletion, or
compensating Git operation after a possible effect. A subsequent deletion
request needs fresh repository observation and host authorization.

### Index, Trusted Profile, and Desktop

Deletion never invokes git add, git rm, restore, reset, checkout, or any other
index operation. It leaves the index entry intact, so Git presents an unstaged
deletion. Human Stage / Unstage remains the only index authority; existing
reviewed-snapshot and repo.commit rules remain unchanged.

Trusted Profile may add a closed symbolic binding for an already approved and
implemented deletion constructor, then compose it atomically into a fresh
ToolRegistry. It cannot manufacture RepositoryFileDeletionPolicy, choose a raw
root/path, relax preconditions, revive a prior generation, or make Execute
permission authorize deletion.

After deleted_verified, Desktop must immediately refresh host-owned repository
status, worktree diff, staged diff/review state, file information, and available
Stage/Unstage actions. It must revoke/invalidate stale repository action IDs and
any commit-review authorization whose source index/repository generation no
longer matches; the UI shows sanitized host state and does not grant deletion or
commit authority. known_no_effect and uncertain also trigger a safe
refresh/invalidation because presentation may be stale. No Desktop commit
authority or automatic Stage is added.

## Deterministic test and fixture matrix

| Area | Required deterministic cases |
| --- | --- |
| Success | Clean HEAD-tracked regular file is removed once; only its worktree pathname changes; index/HEAD/branch/refs remain unchanged; status reports unstaged deletion. |
| Request/schema | Unknown/missing fields, second target, bad hash/length, oversized/NUL input, no model-selected cwd/env/argv/flags. |
| Staleness | Runtime/policy generation, selected repo identity, root/parent/target identity, branch, HEAD, index raw/semantic entry, source hash/length, or target content changes after capture and before final check all refuse before effect. |
| Dirty target | Unstaged change, staged change, staged deletion, staged-plus-unstaged change, intent-to-add, conflict stages, and non-HEAD index entry all refuse and preserve bytes/index. |
| Target class | Untracked, ignored, absent, directory, FIFO/device/special file, executable regular file if otherwise permitted, gitlink/submodule, nested repository, sparse/skip-worktree/assume-unchanged all have explicit expected admission/refusal tests (v1 refuses every unsupported state). |
| Path attacks | Absolute, .., ., empty, backslash, drive-relative, drive-qualified, UNC, verbatim/device prefixes, ADS/colon, .git case variants, wildcard/glob, normalization and case-equivalent aliases refuse. |
| Links/reparse | File symlink, parent symlink, root link, Windows junction, reparse point, mount-like traversal, target replacement race, parent replacement race, and hard-link count >1 refuse or become uncertain without outside effect. |
| Unsupported repository | Bare, linked worktree, detached/unborn HEAD, alternate index, malformed/unreadable index, merge/rebase/cherry-pick/revert/sequencer/bisect state refuse. |
| Native fault/effect | Inject failure before disposition; successful disposition plus verified absence; reported failure with intact source; reported success with failed observer; lost result/timeout/cancel/disconnect at/beyond commit point; delete-pending/sharing violation; all prove the prescribed taxonomy and exactly one native attempt. |
| Isolation/concurrency | Shared lease serializes delete with patch/create/edit/stage/unstage/commit; unrelated dirty paths remain byte-identical; external mutation contradictions fail closed; no retry/replay/restore occurs. |
| Integration | Direct Tool, ToolRegistry permission refusal, Trusted Profile static/effective composition, Generic Tool Bridge validation/redaction/lifecycle count, and Desktop refresh/revocation behavior. |

Fixtures must be disposable repositories outside the checkout, offline,
credential-free, and inspect both filesystem and controlled Git observations.
Windows and Linux need deterministic native seams; unsupported platform semantics
must remain refusal rather than inferred support.

## Future Windows live Codex gate

Only after ADR acceptance, deterministic implementation, bridge composition, and
Desktop integration, certify the smallest disposable Windows repository fixture
with exactly one clean tracked target and one unrelated protected file. Pin the
certified full Codex package/baseline and exact release binary before launch;
keep fixture, app-local state, and host Git evidence isolated.

The durable gate record must include the exact public Tool label, one intended
logical target, and exact counts: ToolRequested = 1, ToolStarted = 1, and
ToolFinished = 1. It must prove the target was present before the call and absent
after it; the unrelated protected file and index/HEAD/ref state are unchanged;
Git reports only the intended unstaged deletion; and no second native deletion
or replay occurred after terminal handling. Capture terminal state,
postconditions, cleanup, exact binary/revision identity, and an unambiguous
success marker. Build/launch/model prose alone is not acceptance evidence.

## Explicit non-goals

- Directory or recursive deletion; wildcard/glob deletion; arbitrary untracked
  file deletion; rename/move; mkdir.
- Generic fs.unlink/fs.write, shell execution, generic process execution,
  generic Git, branch/ref/history authority, network Git, credentials, or
  auto-stage.
- Content replacement, attribute/ACL repair, backup/restore, rollback,
  recovery, retry/replay, OS sandbox, or network-isolation claims.
- Expansion of commit authority, accepted ADRs 0010–0016, or a new public RAH
  abstraction.

## Proposed subsequent ADR 0017 scope

The ADR task should decide whether to accept this proposed
RepositoryFileDeletionPolicy and one repo.delete-file capability. It should
normatively fix the one-file clean-HEAD eligibility rule, opaque host authority,
request/result schemas, repository/runtime/preimage binding, path and
Windows-handle security requirements, one-attempt commit-point taxonomy,
shared-lease participation, no-auto-stage/replay semantics, Trusted Profile
composition limits, and Desktop refresh/invalidation obligations. It should
authorize neither implementation nor rename/move; those need separately tasked
work after ADR acceptance.

## Strong recommendation

**Adopt only a clean-HEAD-tracked, single-regular-file deletion contract with an
exact request preimage and a final independent HEAD equality check.** Reject all
dirty, staged, sparse, linked/reparse, submodule, and uncertain cases in v1.
This preserves unreviewed user work and the human index/commit boundaries while
adding the smallest useful structural-authoring capability.
