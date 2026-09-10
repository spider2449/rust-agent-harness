# RAH v0.24.0 Release Gate

## Status

PREPARED — NOT PUBLISHED

RAH v0.24.0 is prepared from the independently accepted Task 286 milestone
audit. No v0.24.0 tag, GitHub Release, artifact publication, or post-release
cleanup is part of this gate. v0.23.0 remains the current immutable published
release.

## Release identity and checkpoint

- Release candidate: `0.24.0`.
- Preparation baseline: `4197d90cd04493b562db7ae9be315074f9320433`.
- Baseline subject: `docs: audit v0.24 reviewed deletion milestone`.
- Direct parent of the baseline: `3a465d3b5010f86e606e3e3a0281d20774aacbc8`.
- Final release-preparation commit: `TO BE RECORDED AFTER COMMIT`.
- Workspace: 13 packages, all version `0.24.0`, all edition 2024.
- No dependency, resolver, edition, member, public API, or authority change.

Task 286 accepted the following verdict:

> RAH v0.24 REVIEWED DELETION MILESTONE READY FOR RELEASE PREPARATION

## Task chronology

- Task 279 — scope and authority roadmap: `31d2c5a9e927979ef1ba5ca5d9c579db2cd96bf7`.
- Task 280 — reviewed deletion contract research: `19f60079f36d53b2afc4151a64fc6a11cf441e7f`.
- Task 281 — ADR 0025 acceptance: `b7bf5db0a43c37a133820b2861d0916b3b0913d7`.
- Task 282 — shared deletion preparation foundation: `599d4dcdf833d0bf7cc791f8ae73bf4d18b7b09a`.
- Task 283 — deterministic reviewed deletion backend and corrections:
  `76f1298ba4a865b40f096ba7cfdf950516e9ad74`,
  `fe8d7048cb5b7f294341d424d741f5232553fda9`,
  `4b46370fba127fd479592166b0ec70979ad90aae`.
- Task 284 — Desktop reviewed deletion workflow: `247980a786c44439ec8c0a0d7b04fc5047b68946`.
- Task 285 — Windows certification corrections through live code/test head:
  `748d379738bf114509cb7ddf0e1f42e0b1dd4991`,
  `53b4a3068b7c12dd2650846209d755f79abdf45d`,
  `0ac1370b075f35ca255021e278bca848c827dabb`,
  `aab6e82ed58dab841bbc4b94849012f99287c23f`,
  `0b5b1912048534eccf1596c9945d5bab6dc4d19a`,
  `e6cb63436541fe653ba0ea4165466aaa822bc206`.
- Task 285 final docs-only evidence head: `3a465d3b5010f86e606e3e3a0281d20774aacbc8`.
- Task 286 — milestone audit: `4197d90cd04493b562db7ae9be315074f9320433`.

## Architecture and authority contract

The release theme is **HostExplicit Reviewed File Deletion
(`repo.delete-file`)**. The relationship is fixed:

```text
ADR 0017 = sole underlying repository file-deletion mutation authority
ADR 0021 = generic HostExplicit coordinator/currentness/ticket/D2/provenance boundary
ADR 0025 = reviewed human deletion HostExplicit boundary
```

The production route is:

```text
typed human {path}
 -> zero-effect deletion Prepare
 -> complete bounded backend-derived destructive review
 -> opaque process-local single-use ticket
 -> ticket-only Confirm
 -> connected-current/currentness checks
 -> retained deletion-preparer revalidation
 -> exact ToolDefinition / permission membership
 -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started
 -> authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.delete-file
 -> ADR 0017 RepositoryFileDeletionPolicy
 -> strict five-status parsing
 -> independent reviewed-route effect proof
 -> status-only terminal HostActivity
 -> descriptive repository refresh
```

The shared `RepositoryDeleteFilePreparer` is zero-effect and retains the
private exact preimage, identities, Git/index/HEAD/ref observations, canonical
Tool input, exact ToolDefinition and permission membership, review identity,
and currentness-bound private state. The frontend is presentation-only. There
is no Desktop direct delete path, generic deletion service, provider manager,
or provider-derived eligibility.

The exact ten production HostExplicit Tools are:

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
```

Ineligible: `repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP
Tools, Process Plugin Tools, fixture/diagnostic Tools, and unknown Tools. There
is no wildcard, prefix, effect-category, permission-derived, or
provider-derived admission. Model request/output, frontend state, human
confirmation, `PermissionLevel::Execute`, Tool presence, provider metadata,
and Trusted Profile metadata are not authority.

## Reviewed deletion contract

The human request is closed to `{path}`. Bounds are:

- canonical serialized request: `<= 8192` bytes;
- logical path: `1..=1024` UTF-8 bytes;
- reviewed source: `0..=65536` raw bytes, strict UTF-8, NUL rejected;
- complete serialized review: `<= 262144` bytes; and
- retained private preparation: `<= 524288` bytes.

The reviewed target is exactly one existing regular repository file, current
HEAD-tracked, with one normal stage-0 index entry, worktree bytes equal to the
current HEAD blob, index equal to the HEAD entry, mode `100644` or `100755`,
captured FileIdentity, link count 1, supported ordinary repository state,
strict UTF-8, no NUL, and no more than 64 KiB. The complete deterministic
escaped source is the review surface and is never truncated or treated as a
preview. It includes the exact path, clean state, mode, permanent intent,
preimage, byte length, SHA-256, BOM/newline/content facts, HEAD/blob/index
relationships, expected effect, post-delete Git meaning, non-effects, and
destructive warnings. Raw bytes remain private preparation state.

Prepare has zero Tool/native attempts and zero filesystem, index, HEAD/ref,
Stage, Unstage, Commit, model, MCP, Process Plugin, or durable-authority
effects. Confirm consumes an opaque RAH-generated, process-local, in-memory,
capability-specific, single-use, nonpersistent, nonresumable,
currentness-bound ticket. Elapsed `< 300s` is valid; elapsed `>= 300s` is
expired. Confirm and Cancel are ticket-only, and `ticket_id != activity_id`.
The activity ID cannot Confirm or Cancel, and generic activity is status-only
and excludes the ticket, source-bearing review, and private values.

D2 occurs before Started and effect. Effectful Confirm invalidates
repository-bound reviewed Commit authorization immediately before Started.
Refresh is descriptive and cannot fabricate replacement authorization. A
verified deletion means one reviewed worktree file absent and one unstaged Git
deletion; it does not Stage, Unstage, Commit, mutate the index, HEAD, branch,
refs, or history, and it does not rename, move, restore, clean up, or recurse.

The exact five statuses are `deleted_verified`, `known_no_effect`,
`invalid_input`, `precondition_failed`, and `uncertain`.

- `deleted_verified` requires valid underlying Tool output plus independent
  confirmed absence, retained parent identity, no same-name/case-equivalent
  replacement, and unchanged protected Git/index/HEAD/branch/ref state.
- `known_no_effect` requires valid underlying Tool output plus independent
  exact-original FileIdentity/link/bytes/hash/length and protected Git/index
  proof.
- `invalid_input` and `precondition_failed` have no verified deletion effect.
- `uncertain` covers possible effects or insufficient proof. Malformed or
  contradictory output and post-Started dispatch/runtime failure remain
  uncertain; no sixth status is added.

Windows semantics are one native `DeleteFileW` attempt followed by one
immediate reviewed proof pass: no polling, sleep/recheck loop, second delete,
retry, replay, cleanup, restore, or recovery. `DeleteFileW` success is the
filesystem effect commit point, not proof of final absence. This is mitigation,
not a race-free TOCTOU guarantee.

## Evidence summary

Task 285 evidence is carried forward and was not rerun. The live-certified
code/test SHA is `e6cb63436541fe653ba0ea4165466aaa822bc206`; the final Task 285
docs evidence SHA is `3a465d3b5010f86e606e3e3a0281d20774aacbc8`. The certified
host was Windows 10 Professional `10.0.19045` x64 with rustc/cargo `1.96.0`
and Git `2.54.0.windows.1`. Certified Codex was `0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, reasoning `medium`.

The fixture was `src/rah-hostexplicit-live-delete.txt`, 80 bytes, SHA-256
`cdb8ee496bafc4f41bfdac75d37132cb6c1e561703c46b45ede3bfd21396d9fe`.
Prepare Tool/native was `0 / 0`; Confirm Tool/native was `1 / 1`; the
HostActivity sequence was `prepared`, `started`, `tool_completed`; terminal
status was `deleted_verified`; and marker
`RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_OK` was observed. The index, HEAD,
branch, refs, and unrelated staged state were unchanged. Commit authorization
was invalidated before Started. Duplicate Confirm, activity-ID Confirm, and
activity-ID Cancel were rejected; model lifecycle, MCP, and Process Plugin
counts were zero; coordinator/chat were Idle; generation was `[1, 0, 0, 1]`;
and owned Codex cleanup passed. The temporary Task 285 evidence branch was
deleted.

## CI identities

- Task 285 live-certified code CI: run `34452123623`, branch
  `task-285-live-code-ci-evidence`, SHA
  `e6cb63436541fe653ba0ea4165466aaa822bc206`, event `push`, status
  `completed`, conclusion `success`.
- Task 285 final docs evidence CI: run `34451186015`, branch `master`, SHA
  `3a465d3b5010f86e606e3e3a0281d20774aacbc8`, event `push`, status
  `completed`, conclusion `success`.
- Task 286 audit CI: run `34454136984`, branch `master`, SHA
  `4197d90cd04493b562db7ae9be315074f9320433`, event `push`, status
  `completed`, conclusion `success`.

## Preparation validation checklist

The final Task 287 candidate must pass:

- [x] `cargo fmt --check`;
- [x] `cargo check --workspace`;
- [x] `cargo test -p rah-tools`;
- [x] `cargo test -p rah-desktop`;
- [x] `cargo test --workspace`;
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- [x] `node --check crates/rah-desktop/frontend/status.js`;
- [x] `node crates/rah-desktop/frontend/status_authority_test.js`;
- [x] `git diff --check`;
- [x] `cargo metadata --no-deps --format-version 1`;
- [x] `cargo build -p rah-desktop --release`;
- [x] Cargo diff audit shows only workspace package-version movement and no
  dependency drift;
- [x] changed paths are exactly the eight preparation files;
- [x] `v0.24.0` tag and GitHub Release remain absent; and
- [ ] exact-head Task 287 push CI is completed with conclusion `success`.

The destructive ignored Windows test is explicitly not part of this checklist.

## Immutable v0.23 checkpoint

- annotated `v0.23.0` tag object:
  `5a27d84c269a0d57a8a6ad5f0fca89c06379e711`;
- peeled release source:
  `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`;
- GitHub Release ID: `385352229`;
- v0.23.0 tag/release remain unchanged; and
- v0.24.0 tag and GitHub Release are absent.

## Publication checklist — intentionally incomplete

- [ ] independent review of the prepared commit and exact-head CI;
- [ ] create and verify the annotated `v0.24.0` tag;
- [ ] run terminal exact-source tag CI;
- [ ] verify the tag object and peeled source independently;
- [ ] create the GitHub Release with `--verify-tag`; and
- [ ] independently audit publication state.

No future v0.24.0 tag SHA is recorded here. Post-release cleanup and v0.25
work are outside this task.

## Explicit nonclaims

This gate does not claim generic `fs.write/delete/unlink`, recursive or
directory deletion, wildcard/glob deletion, untracked/ignored cleanup,
rename/move, HostExplicit directory creation or `repo.commit`, automatic
Stage/Unstage/Commit, backup/restore, Trash/Recycle Bin semantics, retry,
replay, rollback, compensation, recovery journal, model-selected deletion,
MCP or Process Plugin deletion, network/provider deletion, generic
shell/process authority, race-free TOCTOU, OS sandboxing, network isolation,
Linux/macOS production live parity, or all Windows failure modes live.
Timeout, cancellation, disconnect, crash, or a lost response after possible
effect is not rollback. Process supervision is not an OS sandbox.
