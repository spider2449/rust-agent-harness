# Changelog

## v0.24.0 — release preparation (2026-09-10)

RAH v0.24.0 is **PREPARED, NOT PUBLISHED**. v0.23.0 remains the current
immutable published release. This preparation is based on Task 286's accepted
verdict:

> RAH v0.24 REVIEWED DELETION MILESTONE READY FOR RELEASE PREPARATION

The preparation baseline is `4197d90cd04493b562db7ae9be315074f9320433`.
The workspace version moves from `0.23.0` to `0.24.0` across 13 packages;
all remain on Rust edition 2024. No dependency, public API, authority, or
production behavior change is part of this release preparation.

### Release theme

**HostExplicit Reviewed File Deletion (`repo.delete-file`)** is the reviewed
human capability added by the accepted v0.24 milestone. Its route is:

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

ADR 0017 remains the sole underlying repository file-deletion mutation
authority. ADR 0021 remains the generic HostExplicit
coordinator/currentness/ticket/D2/provenance boundary. ADR 0025 remains the
capability-specific reviewed human deletion HostExplicit boundary. Human
confirmation, model output, frontend state, `PermissionLevel::Execute`, Tool
presence, provider metadata, Trusted Profile metadata, MCP, and Process Plugin
metadata do not create deletion authority.

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

`repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools, Process
Plugin Tools, fixture/diagnostic Tools, and unknown Tools remain ineligible.
There is no wildcard, prefix, effect-category, permission-derived, or
provider-derived admission.

The closed human request is `{path}`. Its bounds are a canonical serialized
request of at most 8192 bytes, a logical path of 1..=1024 UTF-8 bytes, a
reviewed source of 0..=65536 raw bytes with strict UTF-8 and no NUL, a complete
serialized review of at most 262144 bytes, and retained private preparation of
at most 524288 bytes. The complete escaped source is the review surface;
truncation is not permitted. The ordinary ADR 0017 Tool remains broader: its
input remains `path`, `expected_file_sha256`, and
`expected_file_byte_length`, with a 1 MiB ordinary-file bound and binary
ordinary deletion still allowed.

Prepare has zero Tool/native effects. Confirm uses the retained preparation,
currentness, exact definition/permission membership, and D2 before Started,
then reaches the current registry through `authorized_tool_dispatch`. A
verified result means exactly one reviewed worktree file is absent and one
unstaged Git deletion is present. It does not Stage, Unstage, Commit, mutate
the index, HEAD, branch, refs, or history; it does not rename, move, restore,
clean up, or recursively delete.

The exact deletion statuses remain `deleted_verified`, `known_no_effect`,
`invalid_input`, `precondition_failed`, and `uncertain`. `deleted_verified`
requires valid Tool output plus independent confirmed-absence proof.
`known_no_effect` requires valid Tool output plus independent exact-original
preimage proof. Malformed/contradictory output and post-Started failures remain
`uncertain` when safe proof is unavailable. The ticket is opaque, process-local,
in-memory, capability-specific, single-use, nonpersistent, nonresumable, and
currentness-bound; elapsed time `< 300s` is valid and `>= 300s` is expired.
The ticket ID is distinct from the non-authority activity ID, and Confirm and
Cancel are ticket-only. Generic activity is status-only and excludes the
source-bearing review and private authority values. Effectful Confirm
invalidates reviewed Commit authorization immediately before Started; no
automatic Commit occurs.

Windows semantics are one native `DeleteFileW` attempt followed by one
immediate reviewed proof pass: no polling, sleep/recheck loop, second delete,
retry, replay, cleanup, restore, or recovery. `DeleteFileW` success is the
effect commit point, not proof of final absence. Possible effects remain
uncertain. No race-free TOCTOU, OS sandboxing, network isolation, or rollback
claim is made; process supervision is not an OS sandbox.

### Evidence and limitations

Task 285's accepted Windows evidence is carried forward without rerunning the
destructive test. It used Windows 10 Professional `10.0.19045` x64, Rust/Cargo
`1.96.0`, Git `2.54.0.windows.1`, Codex `0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, model
`gpt-5.6-terra`, and medium reasoning. The fixture was
`src/rah-hostexplicit-live-delete.txt`, 80 bytes, SHA-256
`cdb8ee496bafc4f41bfdac75d37132cb6c1e561703c46b45ede3bfd21396d9fe`.
Prepare was `0 / 0` Tool/native; Confirm was `1 / 1`; the result was
`deleted_verified`; the HostActivity sequence was `prepared`, `started`,
`tool_completed`; the marker was `RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_OK`.
Index, HEAD, branch, refs, and unrelated staged state were preserved, Commit
authorization was invalidated before Started, duplicate and activity-ID
operations were rejected, model/MCP/Process Plugin counts were zero, and
owned Codex cleanup passed.

The live-certified code/test SHA was `e6cb63436541fe653ba0ea4165466aaa822bc206`;
the final Task 285 docs evidence SHA was
`3a465d3b5010f86e606e3e3a0281d20774aacbc8`. Exact CI identities are retained
in the v0.24 release gate. The evidence is a connected-current human/host
success path, not model-selected deletion, Linux/macOS parity, or all
Windows failure modes. v0.24 does not claim generic filesystem deletion,
recursive/directory/wildcard deletion, untracked cleanup, rename/move,
HostExplicit directory creation or commit, automatic Stage/Unstage/Commit,
backup/restore/Trash semantics, retry/replay/rollback/compensation, model or
provider deletion, generic shell/process authority, or broader platform
isolation. Timeout, cancellation, disconnect, crash, or lost response after a
possible effect is not rollback.

## v0.23.0 — released (2026-09-09)

RAH v0.23.0 is released and is the current immutable published release.
v0.22.0 is the prior immutable published release. The workspace version is
`0.23.0` across 13 packages, all edition 2024.

### Release record

- Immutable release source: `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Annotated tag: `v0.23.0`.
- Tag object: `5a27d84c269a0d57a8a6ad5f0fca89c06379e711`.
- Peeled target: `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Release-preparation exact-head CI: `34326184721` PASS.
- Tag CI: `34330434792` PASS.
- GitHub Release: `385352229` — `RAH v0.23.0`.
- Published: `2026-09-09T08:43:12Z`.

### Historical release-preparation record

During Task 276, the v0.23 release gate was **PREPARED / NOT YET PUBLISHED**.
Task 275 authorized preparation with verdict **A — v0.23 NEW-FILE HOSTEXPLICIT
MILESTONE COMPLETE — RELEASE PREPARATION MAY BEGIN**. The preparation baseline
was `533b0769618d25c1b9673a27c3e21af7a48809ca`. Task 276 itself deliberately
created no `v0.23.0` tag, GitHub Release, artifact, or other publication.
Publication was completed later in Task 277. Task 278 records this state in a
later documentation-only descendant; it is not the v0.23.0 release source.

### Added

- **HostExplicit Reviewed New-File Authoring (`repo.create-file`)** through
  the existing capability:

  ```text
  typed human {path, content}
   -> zero-effect shared Prepare
   -> complete bounded backend-derived review
   -> opaque single-use ticket
   -> ticket-only Confirm
   -> connected-current / exact-definition / permission / preparer checks
   -> shared creation revalidation
   -> D2
   -> reviewed Commit authorization invalidation
   -> HostExplicit Started
   -> authorized_tool_dispatch
   -> current ToolRegistry
   -> existing repo.create-file
   -> ADR 0013 RepositoryFileCreationPolicy
   -> strict result classification
   -> descriptive repository refresh
  ```

- ADR 0013 remains the sole underlying file-creation mutation authority. ADR
  0024 owns only this capability-specific reviewed HostExplicit route. ADR
  0021 remains the generic HostExplicit coordinator, currentness, D2, ticket,
  lifecycle, and provenance boundary.
- The human request is closed to exactly `{path: String, content: String}`;
  unknown fields fail closed. It is exactly one file, with a 1..=1024 UTF-8
  byte path, 0..=262144 UTF-8 byte content, NUL rejection, canonical request
  size at most 327680 bytes, and complete serialized review at most 262144
  bytes. Empty content is valid. Content bytes are exact: no BOM or newline
  transformation, Unicode normalization, templating, append, overwrite, or
  automatic final newline.
- The reviewed route requires an existing safe parent, target absence from the
  worktree, HEAD, every index stage including intent-to-add, and Git admission.
  It rejects ignored targets, submodules, unsupported sparse-checkout state,
  unsafe Windows reserved/device/ADS/UNC/verbatim names, reviewed trailing-dot
  or trailing-space components, and symlink, junction, or reparse traversal.
  It creates no parent directory. Native exclusive acquisition is the commit
  point (`O_CREAT | O_EXCL` intent on Unix; `CREATE_NEW` / `FILE_CREATE` intent
  on Windows). Complete writing is not an atomic all-or-nothing transaction.

### Security

- The exact nine eligible HostExplicit Tools are `fs.read`, `repo.file-info`,
  `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`,
  `repo.patch`, `repo.edit-files`, and `repo.create-file`. `repo.delete-file`,
  `repo.rename-file`, `repo.create-directory`, `repo.commit`, MCP Tools,
  Process Plugin Tools, fixture Tools, and unknown Tools remain ineligible.
  There is no wildcard, category, or provider-metadata route.
- The exact ADR 0013 result classes remain `ok`, `invalid_target`,
  `precondition_failed`, `create_failed_known`, `write_failed_known`, and
  `uncertain`. `create_failed_known` requires bounded post-observation proving
  no RAH creation effect; `write_failed_known` means exclusive creation
  succeeded and an attributable empty or partial file may remain; `uncertain`
  preserves unknown absent, empty, partial, complete, or replaced state.
  Malformed or contradictory results are not upgraded to success.
- Prepare performs zero Tool executions and zero native creation attempts. The
  ticket is opaque, process-local, in-memory, capability-specific,
  single-use, exact-preparation/currentness-bound, and valid for an inclusive
  five-minute TTL. It is not durable authority and has no persistence, resume,
  or replacement path. Confirm and Cancel receive only the ticket ID.
- Generic activity has a distinct non-authority activity ID, and
  `ticket_id != activity_id`. The activity ID is not derived from the ticket
  and cannot Confirm or Cancel. Generic activity, persistence, and logging
  exclude the ticket, complete source content or source sentinel, complete
  source-bearing review, raw ToolInput/ToolOutput, native paths and parent
  identities, and private object identities. Successful generic terminal
  output is status-only.
- Effectful creation invalidates stale repository-bound reviewed Commit
  authorization. Refresh is descriptive only. The workflow never retries,
  replays, deletes, rolls back, restores, compensates, stages, or commits.
  A partial file may remain after `write_failed_known`; an uncertain external
  effect may remain. Timeout, cancellation, or disconnect is not rollback.

### Validation and evidence

- Task 274 evidence is carried forward and is not rerun for release
  preparation. It was host/human initiated, not model-selected: Windows 10
  Professional `10.0.19045` x64; Codex `0.149.0`; Codex SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
  model `gpt-5.6-terra`; reasoning `medium`.
- The guarded command was the exact `scripts/codex-live-gate.ps1` command
  recorded in the v0.23 live-certification plan. It proved target
  `src/rah-hostexplicit-live-created.txt`, source length 79, source SHA-256
  `b61494980e795ac47bfc4598d9e30176a1821c0eca4e3e213503ee67b8237314`,
  Prepare `Tool 0 / native create 0`, Confirm `Tool 1 / native create 1`,
  result `ok`, `HostActivity tool_completed`, generation tuple `[1, 0, 0, 1]`,
  Stage/Commit `0 / 0`, model request and lifecycle counts all zero, MCP and
  Process Plugin counts zero, and marker
  `RAH_CREATE_FILE_HOSTEXPLICIT_LIVE_OK`. It also retained the fresh
  disposable repository, ordinary parent, exact bytes/hash/length, regular
  non-symlink/non-reparse target, unchanged index/HEAD/branch/refs and
  unrelated staged state, distinct ticket/activity IDs, privacy checks,
  duplicate/activity-ID rejection, Commit invalidation, and Idle coordinator.
  The optional live Cancel-before-start subcase was not rerun; deterministic
  cancellation evidence was accepted by Task 275.
- This evidence does not claim model-selected HostExplicit execution. It does
  not claim generic `fs.write`, shell/process authority, arbitrary filesystem
  writing, parent mkdir, overwrite, append, delete, rename, automatic Stage or
  Commit, HostExplicit `repo.commit`, MCP or Process Plugin HostExplicit,
  ticket persistence/resume, retry/replay, rollback/recovery/compensation,
  race-free TOCTOU, process supervision as OS sandboxing, network isolation,
  or Linux/macOS production live parity.

## v0.22.0 — released (2026-09-09)

RAH v0.22.0 is released and is the current immutable published release.
v0.21.0 is the prior immutable published release.

### Release record

- Immutable release source: `76895b4067c38167f3c41a3536f6616cffa8293a`.
- Annotated tag: `v0.22.0`.
- Tag object: `eb91039521eac190efc85ef57ee2a5c28ca4e9fe`.
- Tag CI: `34301369809` PASS.
- GitHub Release ID: `385176161` (`RAH v0.22.0`).
- Published: `2026-09-09T02:02:09Z`.

### Historical release-preparation record

- Task 264 verdict: **A — v0.22 MILESTONE COMPLETE — RELEASE PREPARATION MAY
  BEGIN**.
- Preparation baseline: `43dce0c505a39975b28f9a0be25aeec4ade1a88b`.
- Workspace version: `0.21.0` -> `0.22.0`; all 13 packages remain edition 2024.
- No tag, GitHub Release, artifact publication, or live certification rerun is
  part of this preparation.

### Added

- **HostExplicit Reviewed Multi-File Edit Authoring** through the existing
  `repo.edit-files` capability:

  ```text
  typed bounded multi-file request
   -> zero-effect Prepare
   -> complete backend-derived ordered review
   -> opaque ticket-only Confirm
   -> shared revalidation / D2
   -> HostExplicit Started
   -> authorized_tool_dispatch
   -> ToolRegistry
   -> existing repo.edit-files / ADR 0014
   -> strict result classification / descriptive repository refresh
  ```

- ADR 0023's capability-specific reviewed HostExplicit workflow, reusing ADR
  0014's existing `RepositoryMultiFileMutationPolicy` and ADR 0021's general
  HostExplicit coordinator, currentness, and D2 boundaries.
- Typed Desktop review over 1–4 existing clean HEAD-tracked regular strict-
  UTF-8 files, with 1–16 exact literal replacements per target and at most 64
  replacements total. The host derives preimages, postimages, hashes, lengths,
  and canonical path order; caller order is not execution order.

### Security

- The exact HostExplicit allowlist is `fs.read`, `repo.file-info`,
  `repo.status`, `repo.diff`, `repo.diff-staged`, `repo.create-branch`,
  `repo.patch`, and `repo.edit-files`. `repo.create-file`,
  `repo.delete-file`, `repo.rename-file`, `repo.create-directory`,
  `repo.commit`, MCP Tools, Process Plugin Tools, and unknown Tools remain
  ineligible.
- Preparation is zero-effect and review is complete and backend-derived;
  `review_too_large` fails closed. Confirm/Cancel receive only a process-local,
  in-memory, single-use opaque ticket with an inclusive five-minute TTL,
  exact-change/currentness binding, and no persistence or resume.
- Generic HostExplicit activity uses a separate RAH-generated activity ID. It
  is not the ticket, is not derived from the ticket, cannot Confirm or Cancel,
  and the actual ticket and source-bearing review content are absent from
  generic activity. Task 263 Attempt 1 discovered the ticket-under-
  `invocationId` leak before Confirm with zero effect; Attempt 2 corrected it
  and passed on a fresh repository.
- ADR 0014's six result classes remain distinct: `ok`, `invalid_target`,
  `precondition_failed`, `failed_known_no_effect`, `partial_effect`, and
  `uncertain`. The operation is explicitly non-atomic: only a verified
  committed prefix is reported, uncertainty is preserved, and there is no
  retry, replay, prefix continuation, rollback, restore-preimage, transaction,
  automatic Stage, or automatic Commit.
- Model output is never authorization. Frontend input and presentation,
  permission classification, Trusted Profile composition, and provider
  metadata cannot create mutation authority or enable HostExplicit.

### Validation and evidence

- Task 262 provides deterministic fault, privacy, result, currentness,
  invalidation, and no-replay evidence. Task 263's final Attempt 2 provides
  the connected-current Windows production certification without a model
  prompt or model lifecycle.
- Task 263 evidence: Windows 11 IoT Enterprise LTSC x64; Codex `0.149.0`;
  SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
  model `gpt-5.6-terra`, reasoning `medium`; caller order `d.txt, b.txt,
  a.txt, c.txt`; backend/effect order `a.txt, b.txt, c.txt, d.txt`; Prepare
  `0` Tool/native attempts; Confirm `1` Tool and native attempts `1/1/1/1`;
  final generations `[1, 0, 0, 1]`; coordinator and chat Idle; model lifecycle,
  MCP, and Process Plugin counts all zero; marker
  `RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK`.
- During Task 265, the v0.22 release gate was `PREPARED / NOT YET PUBLISHED`.
  Unix deterministic or platform-gated testing is not Windows-equivalent live
  certification.

### Limitations and nonclaims

- `repo.edit-files` is non-atomic; `partial_effect` and `uncertain` may occur.
  There is no race-free TOCTOU, network isolation, OS sandbox, rollback,
  replay, or automatic recovery guarantee; process supervision is not OS
  sandboxing.
- No generic `fs.write`, `shell.exec`, structural HostExplicit authoring,
  HostExplicit `repo.commit`, MCP or Process Plugin HostExplicit, model-
  selected HostExplicit certification, authority-ticket persistence/resume,
  or automatic Stage/Commit is claimed.

## v0.21.0 — 2026-09-08

RAH v0.21.0 is released and is the current immutable published release.
v0.20.0 is the prior release.

### Release record

- Release commit: `7faa13a16425cc9d3bdb0dc540923eae8685ba90`.
- Annotated tag: `v0.21.0`.
- Tag object: `aa178f05d12d881745932800e8fc40a7431ce7b9`.
- Task 253 exact-head CI: `34198526278` PASS.
- Tag CI: `34199221802` PASS.
- GitHub Release ID: `384519493`.
- Published: `2026-09-08T07:27:35Z`.

### Added

- HostExplicit reviewed single-file `repo.patch` authoring workflow:
  `inspect -> typed human repo.patch Prepare -> exact bounded review ->
  ticket-only Confirm -> existing repo.patch -> inspect diff -> existing Stage /
  Unstage -> existing reviewed Commit`.
- Shared non-effectful `RepositoryPatchPreparer` and opaque revalidation for the
  host-derived canonical patch input.
- Exact bounded escaped review and ticket-only confirmation for the H1 fields
  `path`, `expectedOldText`, and `replacementText`.
- Frontend reviewed patch UX using typed fields and no generic Tool/JSON route.

### Security

- ADR 0022 adds the reviewed HostExplicit worktree-authoring boundary around
  the existing ADR 0012 `repo.patch` authority; ADR 0021 remains the general
  HostExplicit dispatch/currentness/D2 boundary.
- Host-derived SHA/length and canonical `ToolInput`, shared non-effectful
  preparation, exact R4 review, opaque process-local single-use five-minute
  tickets, strict pre-start revalidation/D2 ordering, and exactly-once mutation
  are retained.
- Confirm receives only a ticket ID; `authorized_tool_dispatch` reaches the
  existing `repo.patch` Tool after `Started`; strict result classification,
  source-review privacy, and conservative malformed/uncertain/post-start
  ToolError handling are required.
- No retry, replay, rollback, or restore-preimage is performed. Patch source
  text is absent from generic activity and persistence. Effectful patch start
  invalidates old reviewed-commit authorization; refresh may create a new
  `ReadyToAuthorize` review but never automatically authorizes it. Content
  mutation alone does not increment `repository_generation`.

### Validation

- Task 252 Verdict A: milestone complete; release preparation may begin.
- Deterministic workspace validation: 672 passed, 10 ignored, 0 failed.
- Task 251 final Windows connected-current HostExplicit reviewed `repo.patch`
  certification through the production backend, with Prepare `0 Tool / 0
  replacement`, Confirm `1 Tool / 1 replacement`, `ChangedVerified`, exact
  postimage, protected repository state unchanged, model lifecycle `0/0/0`,
  and MCP/Process Plugin providers `0`.

### Limitations

- No GUI mouse automation certification; no model-selected `repo.patch`
  certification; no Linux/macOS live certification.
- No other HostExplicit authoring Tool enablement and no external-provider
  HostExplicit.
- No generic filesystem write, shell, or generic process authority; no
  automatic Stage or Commit.
- No rollback guarantee, race-free TOCTOU guarantee, network isolation claim,
  or OS sandbox claim; process supervision is not OS sandboxing.
- No staged, binary, new-file, delete, rename, directory, or multi-file
  HostExplicit authoring, and no claim that a refreshed review is automatically
  authorized.

## v0.20.0 — 2026-09-07

At publication, RAH v0.20.0 was the current immutable published release.
v0.19.0 was the prior release.

### Release record

- Release commit: `0d2b4c46be5d879b70c660501f9b7236040a7a61`.
- Annotated tag: `v0.20.0`.
- Tag object: `bc3addb4f41914a97a91521eb01c4726ae3c2c73`.
- Task 242 exact-head CI: `34090581488` PASS.
- Tag-triggered CI: `34091553715` PASS.
- GitHub Release: `RAH v0.20.0` (numeric ID `383875194`).

### Added

- Explicit Desktop Host Tool invocation for a closed first-party Tool set.
- Exact six-Tool first-release HostExplicit eligibility:
  `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`, `repo.diff-staged`,
  and `repo.create-branch`.
- Typed Host actions, branch Prepare/review/Confirm, and private
  `host_explicit` activity/provenance.

### Security

- ADR 0021 keeps host explicit dispatch, runtime/model dynamic dispatch, and
  capability authorization separate.
- HostExplicit reuses the shared D2 current-definition/permission gate and
  the Codex bridge now applies equivalent admission hardening.
- Connected-current composition, backend-owned eligibility, typed input,
  single-use branch tickets, and no generic ToolName/JSON route are retained.
- No replay, rollback, or authority amplification is introduced.

### Validation

- Task 239 deterministic evidence covers the six-Tool route and security
  boundaries.
- Task 240 certified Windows connected-current `repo.status` and
  `repo.create-branch` through the production HostExplicit backend path.
- The live branch effect was verified as `rah-host-explicit-live-18d2efa09d330900-2`
  at OID `e6b376c26b0d97e12c1be2c7981aecc32974e29c`.
- The live run recorded model lifecycle `runtime.start = 0`, `AgentRequest = 0`,
  `prompt = 0`, `ToolRequested = 0`, `ToolStarted = 0`, and `ToolFinished = 0`.

### Limitations

- `fs.read`, `repo.file-info`, `repo.diff`, and `repo.diff-staged` are
  deterministic-only HostExplicit release claims; they were not separately
  live certified.
- Real GUI mouse-click automation and Linux/macOS HostExplicit live behavior
  were not certified.
- Model-selected `repo.create-branch` remains not certified under Task 229;
  Task 207 remains unchanged.
- External provider Tools, `repo.commit`, and worktree-authoring HostExplicit
  routes remain deferred. Post-start HostExplicit cancellation is not offered.

## v0.19.0 — 2026-09-07

RAH v0.19.0 released for the Bounded Local Branch Creation at Captured
Attached HEAD milestone.

### Release record

- Release commit: `88463787d14e32a3f6692385161b59474bff1dea`.
- Annotated tag: `v0.19.0`.
- Tag object: `24fff2dbae77d6fbd281bf9f4ac34b855c116e3a`.
- GitHub Release: `RAH v0.19.0` (numeric ID `383770772`).
- Task 231 exact-head CI: `34040345951` PASS.
- Tag-triggered CI: `34071020347` PASS.

### Added

- `repo.create-branch`, backed by private host-owned branch authority.
- Exact captured-HEAD creation of one absent ordinary local branch ref.
- Explicit Desktop composition and the Effective Authority local-branch
  category.

### Security / authority

- `Execute` remains the outer gate only; the model supplies only a validated
  logical name.
- A fixed `update-ref` expected-absence CAS and Git-owned fixed reflog create
  the local ref without switching, tracking, or generic Git authority.
- Each call has one possible mutating attempt. Uncertain effects are not
  replayed, retried, rolled back, or compensated.
- Repository, executable, identity, hooks, configuration, and output bounds
  remain host-controlled and hardened.

### Validation and limitations

Windows host-driven Desktop repo.create-branch authority/effect path is certified. Model-selected repo.create-branch dispatch was not observed in two bounded Codex live attempts and is not certified.

- The fresh host-driven branch name and OID were internally asserted but were
  not printed on the successful output path; this is a nonblocking evidence-
  capture limitation and the effectful gate was not rerun.
- Linux live branch certification was not established.
- Task 207's prior external-provider model-selection limitation remains
  unchanged.

## v0.18.0 — 2026-09-06

RAH v0.18.0 released for the Inert Trusted Profile Persistence and Explicit
Restore milestone.

- Release commit: `bd0d237b8cda8f4cebdf56d2e3a792b5dd79ba2e`.
- Annotated tag: `v0.18.0` (tag object
  `e2ac5affc58984dd8ffad5a4d0b9f0de4780d8f0`).
- GitHub Release: [RAH v0.18.0](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.18.0)
  (numeric ID `383437699`).
- Task 219 exact-head CI: `34006233351` PASS.
- Tag-triggered CI: `34006623521` PASS.

### Added

- Desktop persists one host-selected Trusted Profile source path as inert local
  preference. Restart is remembered-not-restored: no selection, validation,
  composition, provider spawn, Tool advertisement, or authority restoration
  occurs implicitly.
- Explicit Restore fresh-loads and statically validates the current source
  without spawning. Connect/reconnect fresh-loads it again and remains the only
  provider activation boundary. Forget removes only the durable preference.
- Profile generation participates in currentness. The certified Codex 0.149.0
  baseline store has an explicit host-only repair workflow for legacy/invalid
  stores; it is not automatic repair, download, or migration.

### Validation and limitations

- Task 217 initially remained **INCONCLUSIVE** because the local baseline store
  was legacy v1. Task 217A repaired that host prerequisite; the resumed Task
  217 Windows lifecycle result is **PASS**. This is supporting baseline
  maintenance, not v0.18 product authority.
- Task 207 remains **INCONCLUSIVE / externally blocked for the model-selected
  external Tool execution sub-gate**. v0.18 makes no model-selected external
  Tool, Linux Desktop lifecycle, ambient-effect absence, OS-sandbox, network
  isolation, or rollback claim.

## v0.17.0 — 2026-09-05

Released as `RAH v0.17.0` for the Desktop Host-Selected Trusted Profile
External Provider Integration milestone.

### Added

- A Desktop host-selected, provider-only Trusted Profile overlay. Static
  profile selection is inert and non-spawning; local MCP stdio and Process
  Plugin providers activate only on explicit Connect or reconnect.
- A shared `rah-profile-composition` path that performs exact Tool-set/schema
  admission, preserves host-selected Trusted Profile permissions, merges
  admitted external Tools with the first-party Desktop Tool registry, and
  fails closed on duplicate public Tool names.
- Provider-owned lifecycle cleanup and sanitized Effective Authority external
  descriptors, with Configured, Effective, Advertised, and Current state kept
  distinct.

### Security / authority

- External effects use conservative lifecycle handling: ToolStarted revokes
  reviewed state, ToolFinished refreshes repository state, and uncertain
  effects are never replayed or presented as rolled back.
- The trusted host remains the authority boundary. Provider metadata,
  PermissionLevel, Tool advertisement, `repositoryBound=false`, and frontend
  presentation do not prove absence of ambient provider effects or grant
  generic shell, process, filesystem, or Git authority.

### Validation and limitation

- Hardened live-certification infrastructure fails closed unless it observes
  the complete hidden-nonce-backed Tool lifecycle and cleanup.
- Task 207 remains **INCONCLUSIVE / externally blocked for the model-selected
  external Tool execution sub-gate**. Windows provider selection, admission,
  composition, effective inventory, dynamic Tool advertisement, lifecycle
  ownership, and cleanup were verified, but actual model-selected MCP/Process
  Plugin execution was not established with the tested current ChatGPT-auth
  Codex model/runtime combinations. No RAH product defect was found and
  baseline migration was not justified.
- Real external-effect review invalidation and repository refresh remain
  deterministically verified but not live-certified through a real external
  provider model call. This is the accepted non-blocking Task 207C / Task 208
  limitation.

### Release record

- Release commit: `dc9ae03598f1ac48a571bb118ae9fd971250a2b7`.
- Annotated tag: `v0.17.0`.
- Tag object: `8bf0bbeb14f3d7e42f1f53f2ee5c6098d561a4aa`.
- Tag-triggered CI run `33959105671`: PASS.
- GitHub Release: published.

### Release limits

- v0.17 does not add network or Streamable HTTP MCP, provider
  download/install/update, PluginManager expansion, profile hot reload,
  active-provider auto-restore/persistence, generic shell/process,
  filesystem, or Git/branch/ref/history authority, OS sandboxing, network
  isolation, rollback, or Linux external-provider live certification.

## v0.16.0 — 2026-09-04

Released as `RAH v0.16.0` for the host-owned Effective Authority Review UX.
The immutable release commit is
`509a5ba8daefeabbf91da50853402a1661099668`; annotated tag `v0.16.0` has tag
object ID `c6ada41ed3c5edc677e392597c7d65dd5e9e69de` and peels to that commit.
The GitHub Release was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.16.0>.

### Added

- A read-only, backend-sanitized Desktop authority inventory showing public
  Tool names, host-derived effect/authority/permission/source classifications,
  and bounded unavailable-capability reasons.
- Clear configured, effective, and runtime-advertised state, with
  generation-aware Current, stale, and reconnect-required classification.
- Presentation of reviewed-commit state without exposing review authority or
  handles.

### Security / authority

- Backend sanitization is applied before frontend serialization/rendering;
  unknown schema and status values fail closed.
- Refresh Authority is observational and has zero Tool, lifecycle,
  repository, chat, or authority side effects.
- Tool visibility or advertisement is not unconditional execution
  authorization. Requests still pass ToolRegistry lookup, PermissionLevel,
  host policy, repository/workspace constraints, generation checks, and any
  one-shot reviewed-commit authorization.
- No new model-accessible authority or ADR was added.

### Validation

- Deterministic cross-layer security hardening and Windows live certification
  PASS using the certified `codex-cli 0.149.0` baseline.
- Linux live certification is not established.

### Release record

- Release commit: `509a5ba8daefeabbf91da50853402a1661099668`.
- Annotated tag: `v0.16.0`.
- Tag object: `c6ada41ed3c5edc677e392597c7d65dd5e9e69de`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.16.0>.
- Exact-head release-source CI run `33848123910` passed.
- Tag-triggered CI run `33848713329` succeeded.

## v0.15.0 — 2026-09-04

Released as `RAH v0.15.0` for the bounded repository directory-creation
milestone. The immutable release commit is
`6b66a357cacea4b1fcf21131cbc9e72fab90d59c`; annotated tag `v0.15.0` has tag
object ID `6ca031e66972b5e04dcade6766d6156a9c3e1a9b` and peels to that commit.
The GitHub Release was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.15.0>.

### Added

- Public `repo.create-directory` and separate host-owned
  `RepositoryDirectoryCreationPolicy` authority.
- Exactly one ordinary directory leaf at an explicit repository-relative path;
  the parent must already exist and the destination must be absent.

### Security / authority

- Directory creation is separate from file create/delete/rename and Execute;
  parent and destination validation, immediate pre-effect revalidation, and
  handle-/descriptor-relative native effects are preserved.
- One possible-effect attempt is allowed: no recursive mkdir, retry/replay,
  rollback/compensation, placeholder files, or Git mutation.

### Composition

- ToolRegistry composition and Generic Codex Tool Bridge integration preserve
  the public name, exact request/schema, structured result, and private alias
  boundary. Without host authority, the Tool is omitted.

### Desktop

- Host-owned selected-repository and repository-generation binding, stale
  runtime protection, repository refresh, reviewed-commit revocation, and
  Git-clean empty-directory handling are preserved.

### Validation

- Deterministic core, composition, Desktop, metadata, and release validation.
- Windows live `repo.create-directory` certification PASS using the certified
  `codex-cli 0.149.0` pair.
- Task 187 v0.15 milestone audit PASS.

### Limitations

- No recursive/tree creation, multiple directories per request, directory
  deletion or rename/move, overwrite, symlink/junction/reparse creation,
  implicit file or placeholder creation, generic `fs.mkdir`, shell/process,
  staging, commit, rollback, or replay authority.
- Windows is live-certified; Ubuntu/Linux has deterministic validation only,
  and Linux live certification is not established.

### Release record

- Release commit: `6b66a357cacea4b1fcf21131cbc9e72fab90d59c`.
- Annotated tag: `v0.15.0`.
- Tag object: `6ca031e66972b5e04dcade6766d6156a9c3e1a9b`.
- GitHub Release:
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.15.0>.
- Task 188 exact-head CI run `33829088735` passed.
- Windows `repo.create-directory` live certification passed in Task 186.

## v0.14.0 — 2026-09-03

Released as `RAH v0.14.0` for the bounded repository file rename/move milestone.
The immutable release commit is
`52506521bdf838784dd45bb54df2d6bcff8bcd08`; annotated tag `v0.14.0` has tag
object ID `9193423e96dd0cda2fd8f5ed5619ab2b58483acc` and peels to that commit.
Task 177 exact-head release CI run `33727731967` passed. The GitHub Release was
published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.14.0>.

### Added / changed

- A separate host-owned bounded repository file rename/move authority through
  the public `repo.rename-file` Tool.
- Same-directory rename and same-repository cross-directory move for exactly
  one clean HEAD-tracked regular file, guarded by exact source SHA-256 and
  byte-length preconditions and no-replace destination semantics.
- Host-owned authority composition, Desktop integration, and Generic Codex
  Tool Bridge integration for the canonical public capability.

### Security / hardening

- Immediate pre-effect repository, source, destination, HEAD, index, branch,
  runtime, and repository-generation identity revalidation followed by one
  native no-replace effect.
- Possible-effect uncertainty is not replayed. There is no `git mv`,
  copy-delete fallback, automatic staging, or rollback guarantee.
- Repository-generation lifecycle hardening, stale runtime/connection
  publication protections, and reconnect requirements across repository
  switches.
- Complete live-evidence request, advertisement, marker, and structured-result
  observations, with process-wide atomic JSONL evidence serialization.

### Validation

- Deterministic validation on Windows and Ubuntu/Linux where CI/tests provide
  evidence.
- Windows live `repo.rename-file` certification PASS using the certified
  `codex-cli 0.149.0` pair (`codex.exe` and `codex-code-mode-host.exe`).
- Task 176 v0.14 milestone audit PASS.

This is distinct from the v0.13 `repo.delete-file` capability.

### Limitations

- No directory or recursive move, overwrite/replacement, Windows case-only
  rename in v1, untracked-file rename, dirty-file rename, or cross-volume
  copy-delete fallback.
- No generic `fs.rename`, generic filesystem mutation, shell/process authority,
  generic Git authority, or network Git.
- No rollback or transaction guarantee; timeout, cancellation, or disconnect
  does not imply rollback, and possible effects are not replayed.
- Process supervision is not OS sandboxing. Network isolation is not claimed.
- Windows is live-certified; Ubuntu/Linux has deterministic validation only,
  and Linux live certification is not established.

## v0.13.0 — 2026-09-02

Released as `RAH v0.13.0` for the completed bounded repository file-deletion
milestone. The immutable release commit is
`a432d7ecc4a5a564288e2bd50b550055b94920cf`; annotated tag `v0.13.0` has tag
object ID `4a40d8fd0f6065c771dd1a78e4808df0cd02c8e7` and peels to that commit.
Exact-head release CI run `33588660637` passed, and the GitHub Release was
published.

### Added

- `repo.delete-file`, a bounded capability under separate ADR 0017 deletion
  authority. It accepts one explicitly named repository-relative regular file
  only when the clean HEAD-tracked target matches the exact authorized HEAD
  preimage, including raw bytes, SHA-256, and byte length.
- Generic Codex Tool Bridge integration and canonical public tool-name
  discoverability for aliased tools. Provider-private aliases remain
  implementation details.
- Desktop selected-repository integration for the host-created deletion
  authority.

### Verified

- No automatic staging: successful deletion remains an unstaged worktree
  deletion with an unchanged Git index. No commit, ref, or history operation
  is included.
- Trusted Profile composition cannot manufacture deletion authority; model
  requests, provider metadata, Execute permission, tool definitions, and the
  frontend are not authority.
- Windows live validation using `codex-cli 0.149.0` observed public
  `repo.delete-file`, private alias `rah_tool_4` for that run only, and exactly
  `ToolRequested = 1`, `ToolStarted = 1`, `ToolFinished = 1`. The intended
  target was deleted, the sentinel and index were unchanged, the deletion was
  unstaged, HEAD/refs/history were unchanged, no replay occurred, and
  `RAH_REPO_DELETE_FILE_LIVE_OK` was observed.

### Limitations

- The capability does not provide rename/move, directory or recursive
  deletion, arbitrary untracked deletion, generic `fs.write`/`fs.unlink`,
  generic shell/process or Git authority, automatic staging or commit, or
  branch/ref/history/network Git authority.
- The initial aliased-tool discoverability failure is preserved as a failed
  observation that led to the generic canonical-name description fix. The
  CRLF/raw-byte `precondition_failed` observation is preserved as fail-closed
  behavior, not a successful deletion.
- In Task 163 evidence, `tool_finished.result` may be `null` because the helper
  did not capture `ToolContent::Json`; the JSON result was not captured. Task
  164 classified this as non-blocking observability technical debt.
- Deterministic validation is established on Windows and Ubuntu/Linux where
  CI/tests provide evidence. Windows is live-certified; Linux live
  certification is not yet established, and equivalent macOS live validation
  is not claimed.
- Task 120 remains **DEFERRED / NOT VALIDATED** and transport confinement
  remains **NOT CLAIMED**. Process supervision is not OS sandboxing, and
  uncertain effects are not automatically replayed or rolled back.

## v0.12.0 — 2026-09-01

Released as `RAH v0.12.0` for the audited Desktop repository-authoring
milestone. The immutable release commit is
`d1c1cd470fd337f141abb9675fb4642ccd2e00b0`; annotated tag `v0.12.0` has
object ID `4d002a8bc67b1877e692bb0aafd764fc5eb47b65` and peels to that commit.
Task 153 candidate CI run `33479033004` and Task 154 tag CI run `33479751261`
passed. The GitHub Release was published on `2026-09-01T06:58:38Z`.

### Added

- Windows Desktop end-to-end bounded repository workflow: model bounded
  repository authoring, human Stage / Unstage, host-observed staged review,
  human reviewed-snapshot authorization, message-only `repo.commit`, verified
  Git commit result, and Desktop repository refresh.

### Verified

- The existing authority boundaries are productized through Desktop; v0.12
  introduces no new authority. Model request is not authorization; Execute
  permission is not commit authorization; human Stage / Unstage are host
  actions; human Authorize is the reviewed-snapshot authorization event; and
  the frontend does not own authorization.
- `RepositoryCommitReview` remains opaque and Rust-only. `repo.commit` remains
  message-only, does not auto-stage, and uncertain external effects are not
  replayed.
- Windows live validation reused the Task 151 certified `codex-cli 0.149.0`
  bundle with closed manifest schema v2. The demonstrated complete pair is
  `codex.exe` and `codex-code-mode-host.exe`, each with a closed identity and
  SHA-256; the Task 151 hardening closes only the Desktop verifier gap that
  could accept a directory missing the code-mode host.
- The bounded repository-safe authoring path was live-proven. At
  `D:\\rah-task151-clean`, exactly one independently verified Git commit effect
  was observed: `90683f5eaab129a75e815879e69586ff75de5e86`, with no second
  commit or replay. This fixture commit is not the RAH v0.12 release commit.

### Security and limitations

- No generic Git, shell/process, or `fs.write` authority is introduced. There
  is no branch/ref, network, credential, or rollback authority.
- The exact live edit Tool label and exact live `repo.commit` activity-event
  counts were not durably retained. They remain documented non-blocking
  observability gaps; no unsupported lifecycle counts are asserted.
- Windows is live-certified. Unix/macOS live validation is not claimed. Task
  120 remains **DEFERRED / NOT VALIDATED** and transport confinement remains
  **NOT CLAIMED**.

## v0.11.0 — 2026-08-30

Released as `RAH v0.11.0`. The immutable annotated tag `v0.11.0` has object
ID `3fd37807f382c2c0c61328e72d7542984db05983` and peels to release commit
`44a2ee3c6580b862fd0a71b9e773984de757dc15`. Tag CI run `33300410414`
passed, and the GitHub Release was published.

### Added

- `repo.commit`, a bounded host-reviewed repository commit capability that
  creates one ordinary commit from one exact reviewed staged snapshot in the
  exact trusted-profile-selected repository.
- Trusted Profile composition for the exact repository, exact native Git
  executable, explicit trusted host identity, Execute outer permission, and
  separate host-only per-operation authorization.

### Verified

- Deterministic commit-policy hardening, Trusted Profile composition, and
  Generic Tool Bridge verification.
- Windows certified live Codex validation at exactly `codex-cli 0.149.0`; the
  complete same-version official code-mode host was required for the certified
  dynamic-tool path.
- The disposable live fixture completed lifecycle `1 / 1 / 1` with
  `committed_verified` at `13c200c5c772b3e4a0eceb0a2364981c849313e0`.
  This is fixture evidence, not a RAH repository release commit. There was no
  automatic staging, retry, replay, approval, or synthetic tool call.

### Security and limitations

- `repo.commit` is not generic Git authority. Execute alone is insufficient:
  every commit requires fresh host-reviewed authorization; the model controls
  only the message.
- No automatic staging, branch creation/switching, arbitrary ref mutation,
  detached/unborn commit, amend, merge, rebase, cherry-pick, reset, clean,
  stash, tag, remote/network Git, credential Git, linked worktree, or
  submodule/gitlink commit is supported.
- Uncertain effects are never retried or replayed, and no rollback guarantee is
  made. Windows is live-certified; Ubuntu is deterministic evidence only.
- Task 120 remote llama generation remains **DEFERRED / NOT VALIDATED**.
  Transport confinement remains **NOT CLAIMED**.

## v0.10.0 — 2026-08-29

Released as `RAH v0.10.0`. The immutable annotated tag `v0.10.0` has object
ID `d340120e5b316265d6a4cd83bdf08eb73d712d1a` and peels to release commit
`9f4947ce4e37e9ce5b1e49330ab5327c1bd61ffa`. Tag CI run `33248727210`
passed, and the GitHub Release was published.

### Added

- Desktop certified Codex baseline discovery and selection for exactly
  `codex-cli 0.149.0`, plus closed native Git executable discovery.
- One bounded host-selected llama.cpp provider endpoint under ADR 0015, inactive
  Desktop model-preference persistence, exact selected-repository observation,
  verified repository runtime-CWD binding, and launch-CWD/`AGENTS.md` isolation.
- Repository-scoped conversation persistence, explicit bounded Resume/replay,
  SQLite transcript storage, transactional V3-to-SQLite migration, A/B
  transcript isolation, and fail-closed SQLite corruption handling.

### Limitations

- Task 120 remote llama.cpp generation proof is **DEFERRED / NOT VALIDATED**.
  Transport confinement is **NOT CLAIMED**; ADR 0015 does not promise redirect,
  proxy, DNS, peer-identity, or effective-destination confinement.
- No llama.cpp process management or provider/model installation; no generic
  network Tool, network MCP/Streamable HTTP, generic shell/process authority,
  model-selected executable/cwd/endpoint, automatic authority restoration, Git
  commit/ref/history authority, or generic repository delete/rename authority.
- Repository move/rename intentionally changes the conversation-persistence
  namespace. SQLite is private Desktop storage, not generic SQL authority, and
  uncertain external effects have no rollback guarantee.

## v0.9.0 — 2026-08-25

Released as `RAH v0.9.0` on 2026-08-25. The immutable annotated tag `v0.9.0`
has object ID `fbb30c3787911bdb935417bf51d9c0c5f2bdf381` and peels to release
commit `d971790fd1de7df782a99d2274278a14f1f0066f`. Tag CI run `32824354008`
completed successfully, and the GitHub Release was published.

### Added

- `repo.edit-files` bounded multi-file repository edit authority for up to four
  existing, clean, tracked UTF-8 files, with exact original-snapshot
  replacements and deterministic host-owned commit order.
- Verified partial-effect and uncertain-outcome semantics, Trusted Profile v1
  composition, and Generic Tool Bridge integration.

### Verified

- Windows certified Codex live validation using exactly `codex-cli 0.149.0`
  emitted the structural marker `RAH_REPO_EDIT_FILES_LIVE_OK`.
- ADR 0014 is Accepted.

### Security and limitations

- `repo.edit-files` is not a cross-file transaction and provides no rollback
  or replay.
- It grants no generic filesystem write; it cannot create, delete, or rename
  files, and grants no staging, commit, history, ref, or network Git authority.
- Unix live Codex validation is not claimed.

## v0.8.0 — 2026-08-25

Released as `RAH v0.8.0` on 2026-08-25. The immutable annotated tag `v0.8.0`
has object ID `198eccd34a8ae76b9235736c3d1a64173692c351` and peels to release
commit `0b12d5448dcea89b158e4941e7b741b7539c8894`.

### Added

- Bounded repository file creation through `repo.create-file`: one
  host-authorized UTF-8 file at an existing parent directory per call.
- Native exclusive creation with no overwrite, composed through the Trusted
  Profile and the Generic Tool Bridge while retaining host-bound repository
  authority.

### Verified

- Deterministic Windows and Linux coverage, plus certified Codex live
  validation using exactly `codex-cli 0.149.0`.
- The release-preparation CI run `32804191964` and tag CI run `32804873958`
  completed successfully. The GitHub Release was published at
  <https://github.com/spider2449/rust-agent-harness/releases/tag/v0.8.0>.

### Limitations

- No overwrite, delete, rename, directory creation, binary creation, staging,
  commit/history authority, multi-file transaction, rollback, or replay.
- `repo.create-file` creates one file per call and requires its parent to
  already exist.

## v0.7.0 — 2026-08-24

Released as `RAH v0.7.0` on 2026-08-24. The immutable annotated tag `v0.7.0`
has object ID `b4df68290053f7dd8f6a2b45671fd7cdab8d128f` and peels to release
commit `9521fa4e5f5c184eabd0061eb71854422752b8f1`.

### Added

- `repo.patch` retains its legacy single-replacement request and additionally
  accepts `replacements[]` with one through sixteen exact replacements in one
  existing, HEAD-tracked, regular UTF-8 worktree file.
- Every replacement is resolved against the same original snapshot. Duplicate,
  overlapping, absent, and ambiguous matches are refused; accepted
  non-overlapping replacements are applied deterministically in one final
  single-file replacement.
- Full-file SHA-256 and byte-length preconditions remain mandatory. The
  operation does not automatically stage changes.

### Verified

- Deterministic Generic Tool Bridge validation, Windows native Codex live
  multi-replacement validation, and repository-observer verification cover the
  milestone. Reproducible certified baseline tooling uses isolated configuration
  and host-attested structural markers; the Codex platform-alignment audit
  remains part of the release evidence. Tag CI run `32706469848` completed
  successfully.
- The certified live runtime is exactly `codex-cli 0.149.0`. This release does
  not claim Unix live Codex validation.

### Security and limitations

- `repo.patch` is not arbitrary filesystem write authority. It does not create,
  delete, or rename files; provide a multi-file transaction or rollback; or
  grant Git commit, history, ref, or network authority.
- It does not grant generic shell or process authority. Model requests and
  Codex approvals remain non-authoritative; host policy and `ToolRegistry`
  checks remain required.

## v0.6.0 — 2026-08-24

Released repository-aware read-only workflow inspection milestone. The immutable
annotated tag `v0.6.0` peels to
`6326c18937bbcfd1e515001692a2c88c6884d552`. The GitHub Release, titled
`RAH v0.6.0`, was published at
<https://github.com/spider2449/rust-agent-harness/releases/tag/v0.6.0>.

### Added

- A repository-aware read-only observer toolkit: `repo.file-info`,
  `repo.status`, `repo.diff`, and `repo.diff-staged`.
- Trusted-profile composition for the four fixed host observer capabilities,
  deterministic Generic Tool Bridge verification, and Windows live Codex
  verification using exactly `codex-cli 0.149.0`.
- Ubuntu deterministic and cross-platform coverage for repository-observer
  behavior.

### Security

- No new mutation authority. Observers are fixed-command host capabilities;
  `PermissionLevel::Execute` is only their outer host-process gate.
- The observers do not provide generic Git execution or arbitrary executable,
  argv, cwd, or environment selection. They disable external diff and textconv
  behavior and make no intentional repository mutation.
- The existing guarded `repo.patch` worktree mutation capability remains
  separately governed by ADR 0012. ADR 0010 remains repository-index mutation
  only, and ADR 0011 remains trusted-profile authority composition.

### Verified

- Task 064 deterministically verified all four observers through the Generic
  Tool Bridge. Task 065 ran three fresh Windows live fixtures with exactly
  `codex-cli 0.149.0`; each observer was invoked once and the repository was
  unchanged.
- Release-preparation CI run `32685119256` and tag CI run `32685443380`
  completed successfully. This release does not claim Unix live Codex
  validation, transactional snapshot consistency, or zero incidental filesystem
  writes.

## v0.5.1 — 2026-08-22

Released and published as `v0.5.1`, tagged at
`0ea648d84d6f48720c33e8b1bb07e1c24101c870`. This is the portability-only
recovery release for the published v0.5.0 repository-mutation milestone; it
adds no authority, behavior, public API, dependency, or feature expansion.

### Fixed

- Corrected a Linux/Ubuntu clippy portability defect by importing
  `std::fs::File` only for the Windows-native repository identity path that
  uses it. The Ubuntu `unused import: File` failure is removed without changing
  `repo.patch` replacement, policy, or test behavior.

### Verified

- The minimal recovery commit passed the required GitHub Ubuntu CI job,
  including formatting, workspace check, workspace tests, and clippy.
- The release-preparation CI run `32574019502` and tag CI run `32574129999`
  both completed successfully. v0.5.1 is the corrected, fully required-CI
  verified v0.5.x baseline.
- The Windows live `repo.patch` release gate remains valid using exactly
  `codex-cli 0.149.0`; this release makes no Unix live Codex validation claim.

## v0.5.0 — 2026-08-22

Published feature release, tagged at
`b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786`. It contains the complete v0.5
`repo.patch` feature milestone and passed Windows release validation. Its
required Ubuntu CI later failed only because `std::fs::File` was imported
unconditionally while used only by the Windows native-identity path. This was a
lint-only portability defect, not a `repo.patch` authority or runtime-semantics
defect. The public release and immutable tag were preserved unchanged; v0.5.1
supersedes v0.5.0 operationally as the fully verified v0.5.x baseline.

### Added

- `repo.patch`, a repository-aware capability that conditionally replaces one
  exact literal text occurrence in one bounded existing, HEAD-tracked, unstaged
  UTF-8 worktree file.
- ADR 0012, accepted: a separate private, host-owned
  `RepositoryWorktreeMutationPolicy` for worktree-content mutation. The existing
  `PermissionLevel::Execute` is only an outer runtime gate, not the authority.
- Whole-file SHA-256 and byte-length preconditions; exact single-match
  replacement; bounded request, source-file, and postimage sizes; and strict
  UTF-8 handling that preserves a leading BOM and CRLF/LF bytes exactly.
- Repository, path, link/reparse-point, and hard-link protections; a
  same-directory exclusive temporary complete postimage; one-attempt/no-replay
  behavior; and known-failure versus uncertain-effect classification.
- Trusted-profile composition of `repo.patch`, Generic Tool Bridge verification,
  and a Windows live validation using exactly `codex-cli 0.149.0`. Restricted
  Codex-owned filesystem, shell, process, MCP, and network-tool capabilities
  remain disabled in that path.

### Security

- Worktree content mutation, index mutation, and Git history/ref mutation are
  separately authorized state planes. `repo.patch` does not grant generic
  filesystem write, generic shell/process, Git command, or network authority.
- The policy accepts one request and one native replacement attempt only.
  Successful results require post-observation; failures are reported as known
  only when the preimage is proven intact. Uncertain outcomes are never replayed.

### Verified

- Deterministic repository-patch, trusted-profile composition, and Generic Tool
  Bridge coverage; the opt-in live gate observed the one-request/one-attempt
  path, preserved index/HEAD/refs and unrelated content, and cleaned its
  temporary repository and app-server child.
- Windows is the verified v0.5 release baseline. This release makes no Unix
  live-validation claim.

### Limitations

- No file creation, deletion, rename/move, binary edits, multi-file
  transactions, staged or untracked target mutation, or `restore-worktree`.
- No Git history/ref mutation, network Git, automatic rollback, complete TOCTOU
  elimination, or Unix live-validation claim.

## v0.4.0 — 2026-08-22

Released 2026-08-22. Tag `v0.4.0` targets release commit `ebd6358`; CI passed
and the GitHub Release was published.

### Added

- Trusted static capability profiles with strict versioned parsing, hardened
  explicit source loading, symbolic host resources, built-in composition, and
  redacted static/effective inventories.
- `rah profile validate` for non-spawning static validation and `rah profile
  validate-effective` for explicit effective provider composition.
- Trusted-profile composition for hardened local stdio MCP and Process Plugin
  providers, including exact expected tool/schema admission and explicit host
  permission mapping.
- ADR 0011, the trusted capability profile authority boundary.

### Changed

- Effective composition constructs a fresh `ToolRegistry`, preserves declared
  permissions, fails closed on duplicate registration, and retains provider
  lifecycle ownership. Staged providers are cleaned up after later failure.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Security

- Profiles configure existing host authority only; model requests and provider
  metadata remain non-authoritative.
- MCP and Process Plugin providers use native executable validation/revalidation,
  isolated cwd, minimized environment, bounded stdio/lifecycle resources, and
  atomic admission. These controls are not OS sandboxing or network isolation.

### Verified

- Deterministic mixed built-in + MCP + Process Plugin composition, permission
  preservation, redacted inventory, duplicate fail-closed behavior, and staged
  provider cleanup.
- Opt-in trusted-profile Generic Codex Tool Bridge validation using exactly
  `codex-cli 0.149.0`: one `plugin.test.echo` execution, Codex continuation,
  and child/app-server cleanup.

### Deferred

- Profile discovery, reload, editing, or mutation; generic provider and
  subprocess schemas; MCP Streamable HTTP/network MCP; PluginManager;
  provider/plugin installation or download; automatic restart; and hot reload.
- Generic shell/process authority, model-selected executable/argv/cwd/env,
  destructive worktree authority, Git commit/ref/history mutation, network or
  credential-bearing Git, OS sandboxing, network isolation, and rollback.

## v0.3.0 — 2026-08-22

Git tag `v0.3.0` was created at release commit `1968326`.

### Verified

- Generic Tool Bridge, `fs.read`, the MCP adapter, and the process-plugin
  adapter remain available through RAH-owned neutral tool boundaries.
- Hardened `HostExecutionPolicy` is verified through deterministic and opt-in
  live fixture validation.
- Host-owned Execute capabilities are `host.cargo.version`, `host.git.status`,
  `host.git.stage`, and `host.git.unstage`.
- `RepositoryMutationPolicy` is verified through deterministic and opt-in live
  repository-mutation fixture validation; `host.git.stage` and
  `host.git.unstage` have deterministic and opt-in live validation.
- The optional Codex adapter baseline is exactly `codex-cli 0.149.0`.

### Capability classification

`process.test.echo` is the hardened Execute validation fixture, and the
repository-mutation fixture validates mutation policy behavior. Neither is a
production/public host capability. In particular, v0.3.0 does not include
`host.fixture.echo`.

### Deferred

- arbitrary `shell.exec` and `process.exec`;
- model-selected executable, argv, cwd, or environment;
- worktree restore and arbitrary file mutation;
- Git commit, refs/history mutation, reset, clean, checkout, switch, stash,
  merge, rebase, push, pull, fetch, network Git, and credential-bearing Git
  execution.

Destructive worktree authority is deferred beyond v0.3 and requires ADR 0011.

### Security notes

Process supervision is not OS sandboxing. RAH makes no network-isolation or
rollback guarantee. Timeout or cancellation can leave uncertain mutation
effects, and uncertain mutations are never automatically replayed. On Windows,
Job Object assignment remains post-spawn; external OS processes can race
repository mutation, and Git configuration may still influence Git semantics.
