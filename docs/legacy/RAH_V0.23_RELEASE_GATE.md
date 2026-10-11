# RAH v0.23.0 Release Gate

## Status

RELEASED — HISTORICAL RECORD

This gate records the accepted Task 275 milestone, Task 276 release
preparation, the immutable Task 277 publication, and the Task 278
documentation-only cleanup. RAH v0.23.0 is the current immutable published
release. v0.22.0 is the prior immutable published release.

## Release chronology

- Task 275: v0.23 milestone audit accepted.
- Task 276: release preparation.
- Immutable release source:
  `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Task 276 exact-head CI: `34326184721` PASS.
- Task 277: immutable publication.
- Annotated tag: `v0.23.0`.
- Tag object: `5a27d84c269a0d57a8a6ad5f0fca89c06379e711`.
- Peeled target: `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Tag CI: `34330434792` PASS.
- GitHub Release: `RAH v0.23.0`, ID `385352229`.
- Published: `2026-09-09T08:43:12Z`.
- Draft/prerelease: `false / false`.
- Assets: `0`.

Task 276 itself stopped before tag and Release creation. Publication happened
later in Task 277. Task 278 is documentation cleanup only and does not change
the release source, tag, or GitHub Release identity. Its cleanup commit is a
later documentation-only descendant, not the v0.23.0 release source.

## Decision and release identity

- Task 275 verdict: **A — v0.23 NEW-FILE HOSTEXPLICIT MILESTONE COMPLETE —
  RELEASE PREPARATION MAY BEGIN**.
- Preparation baseline: `533b0769618d25c1b9673a27c3e21af7a48809ca`.
- Task 275 exact-head CI: run `34323593082`, `master`, push event,
  completed, success, exact baseline SHA.
- Target workspace version: `0.23.0`.
- Workspace packages: 13; all use Rust edition 2024.
- v0.23.0 local tag: absent before preparation.
- v0.23.0 remote tag: absent before preparation.
- v0.23.0 GitHub Release: absent before preparation.
- Current immutable v0.22.0 annotated tag object:
  `eb91039521eac190efc85ef57ee2a5c28ca4e9fe`.
- Current immutable v0.22.0 peeled release target:
  `76895b4067c38167f3c41a3536f6616cffa8293a`.
- Current immutable v0.22.0 GitHub Release ID: `385176161`.
- The v0.22.0 objects and Release are not modified or replaced.

## Exact milestone

RAH v0.23.0 delivers **HostExplicit Reviewed New-File Authoring** through the
existing `repo.create-file` Tool:

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

ADR 0013 is the sole underlying file-creation mutation authority. ADR 0024 is
the capability-specific reviewed HostExplicit route. ADR 0021 is the generic
HostExplicit coordinator/currentness/D2/ticket/provenance boundary. HostExplicit
itself is not generic filesystem authority.

## Exact HostExplicit eligibility

Exactly these nine Tools are eligible:

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
```

The following remain ineligible:

```text
repo.delete-file
repo.rename-file
repo.create-directory
repo.commit
MCP Tools
Process Plugin Tools
fixture Tools
unknown Tools
```

Eligibility has no wildcard, category, or provider route. Provider metadata,
Tool visibility, permission classification, frontend state, Trusted Profile
composition, human review, and model text cannot enable HostExplicit authority.

## Authority and closed review contract

The human Prepare input is exactly:

```text
{ path: String, content: String }
```

Unknown fields fail closed. The bounds are exactly one file; path
1..=1024 UTF-8 bytes; content 0..=262144 UTF-8 bytes; NUL rejected; canonical
serialized request at most 327680 bytes; and complete serialized review at
most 262144 bytes. Empty content is allowed. Content is exact: no BOM or
newline transformation, Unicode normalization, templating, append, overwrite,
or automatic final newline. If the complete mutation-relevant review cannot
fit, preparation fails before ticket issuance. No truncated or ellipsized
review may authorize mutation.

The parent must already exist and pass safe-root, containment, ordinary
directory, identity, and non-link/non-reparse checks. The target must be absent
from the worktree, HEAD, and every index stage, including intent-to-add.
Ignored targets, submodule paths, unsupported sparse-checkout state, Windows
reserved/device/ADS/UNC/verbatim forms, reviewed trailing-dot/space components,
and symlink/junction/reparse traversal fail closed. No parent directory is
created. Direct exclusive name acquisition is the commit point:
`O_CREAT | O_EXCL` intent on Unix and `CREATE_NEW` / `FILE_CREATE` intent on
Windows. Complete writing is not an atomic all-or-nothing transaction.

Prepare performs zero Tool executions, zero native creation attempts, and no
filesystem, repository, index, HEAD, ref, history, Stage, Commit, model,
provider, or durable-authority effect. Confirm performs connected-current,
exact-definition, permission, preparer, and shared creation revalidation,
then D2, before reviewed Commit invalidation, HostExplicit Started, and one
authorized dispatch through the current ToolRegistry.

## Exact ADR 0013 results

The six result classes are exactly:

```text
ok
invalid_target
precondition_failed
create_failed_known
write_failed_known
uncertain
```

- `ok`: one exact new regular non-reparse file is verified with the reviewed
  bytes, SHA-256, and length. It remains untracked; index, HEAD, and refs are
  unchanged.
- `invalid_target`: rejected before valid creation authority can effect a
  target.
- `precondition_failed`: repository, path, admission, or currentness
  preconditions fail before native create.
- `create_failed_known`: bounded post-observation proves no RAH creation
  effect. A native error category alone is insufficient.
- `write_failed_known`: exclusive creation succeeded and a known empty or
  partial invocation-attributable file may remain. This is not no-effect.
- `uncertain`: a possible effect cannot be safely classified; absent, empty,
  partial, complete, or externally replaced target state may remain unknown.

Malformed or contradictory output is not upgraded to success. Every
potentially effectful result consumes the ticket and prohibits retry, replay,
automatic delete, rollback, restore, compensation, Stage, or Commit. Timeout,
cancellation, disconnect, or crash is not rollback. No race-free TOCTOU claim
is made.

## Ticket, currentness, and privacy

The ticket is opaque, process-local, in-memory, capability-specific,
single-use, exact-preparation/currentness-bound, and valid for an inclusive
five-minute TTL. It is not serializable as durable authority. There is no
persistence, resume, replay, or replacement ticket. Confirm and Cancel receive
the ticket ID only.

Generic activity uses a distinct non-authority activity ID and requires
`ticket_id != activity_id`. The activity ID is not the ticket, is not derived
from the ticket, and cannot Confirm or Cancel. Generic activity, persistence,
and logging exclude the authority ticket, complete source content or source
sentinel, complete source-bearing review, raw ToolInput, raw source-bearing
ToolOutput, native repository path, native parent identity, and private object
identities. Successful generic terminal output is status-only.

An effectful or potentially effectful create route invalidates stale
repository-bound reviewed Commit authorization. Repository refresh is
descriptive only and cannot fabricate new Commit authorization. A successful
create means one new untracked worktree file, not `git add`, Stage, index
mutation, HEAD mutation, ref mutation, history mutation, or Commit. Direct
content bytes/hash/length verification is required because ordinary `git diff`
does not expose untracked contents as a normal tracked diff.

## Task 274 Windows evidence carried forward

Task 276 does not rerun the Windows live gate or submit a model prompt. The
accepted Task 274 evidence is:

- Windows 10 Professional `10.0.19045` x64.
- Codex `0.149.0`.
- Codex SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- Model `gpt-5.6-terra`; reasoning `medium`.
- Guarded command:

  ```text
  & .\scripts\codex-live-gate.ps1 -Version '0.149.0' -ExpectedSha256 '14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00' -Model 'gpt-5.6-terra' -ReasoningEffort 'medium' -Command { cargo test -p rah-desktop --features rah-tools/live-test-support tests::windows_live_desktop_hostexplicit_create_file -- --ignored --exact --nocapture }
  ```

- Target `src/rah-hostexplicit-live-created.txt`; source length 79 bytes;
  source SHA-256
  `b61494980e795ac47bfc4598d9e30176a1821c0eca4e3e213503ee67b8237314`.
- Prepare: Tool 0, native create 0. Confirm: Tool 1, native create 1.
- Result `ok`; HostActivity `tool_completed`; generation tuple `[1, 0, 0, 1]`.
- Stage / Commit `0 / 0`; AgentRequest 0; model ToolRequested 0;
  model ToolStarted 0; model ToolFinished 0; MCP 0; Process Plugin 0.
- Marker `RAH_CREATE_FILE_HOSTEXPLICIT_LIVE_OK`.
- The fresh disposable repository, ordinary existing parent, exact target
  bytes/hash/length, regular non-symlink/non-reparse target, untracked result,
  unchanged index/HEAD/branch/refs and unrelated staged state, distinct
  ticket/activity IDs, privacy checks, duplicate/activity-ID rejection,
  reviewed Commit invalidation, no replacement authorization, and Idle
  coordinator/chat were retained as accepted evidence.
- The optional live Cancel-before-start subcase was not rerun. Deterministic
  cancellation evidence was accepted by Task 275.

This is human/host-initiated connected-runtime evidence, not model-selected
`repo.create-file` evidence. A connected process is not a model turn; zero
model lifecycle counts do not certify model execution.

## Security nonclaims

This release preparation does not claim generic `fs.write`, shell/process
authority, arbitrary filesystem path writing, parent mkdir, overwrite, append,
delete, rename, automatic Stage, automatic Commit, HostExplicit `repo.commit`,
MCP HostExplicit, Process Plugin HostExplicit, model-selected HostExplicit,
ticket persistence/resume, retry/replay, rollback/recovery/compensation,
race-free TOCTOU, process supervision as OS sandboxing, network isolation, or
Linux/macOS production live parity. A partial file may remain after
`write_failed_known`; an uncertain external effect may remain.

## Validation checklist

All required validation commands below passed sequentially before the
preparation commit. The commands intentionally exclude the ignored Windows
live test and any model prompt.

The sequential validation set is:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
node --check crates/rah-desktop/frontend/status.js
node crates/rah-desktop/frontend/status_authority_test.js
cargo build -p rah-desktop --release
cargo test -p rah-tools
cargo test -p rah-desktop
```

Metadata must show 13 packages, all version `0.23.0`, all edition 2024, and no
remaining workspace package version `0.22.0`. Cargo.lock may contain only the
13 internal workspace package version changes; external dependency versions,
sources, checksums, features, edges, additions, and removals must be
unchanged. The final changed-file list must be exactly the eight authorized
paths, with no Rust/frontend/test/workflow/ADR changes.

## Historical Task 276 preparation and exact-head CI

- Immutable release-preparation source:
  `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`.
- Required commit message: `docs: prepare RAH v0.23.0 release`.
- Required direct parent: `533b0769618d25c1b9673a27c3e21af7a48809ca`.
- Task 276 exact-head CI: `34326184721`, `master`,
  `05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`, push event, completed, success.
- Task 276 stopped after its exact-head CI; it did not create or push the tag,
  create the Release, upload assets, or begin post-release cleanup.

## Publication checklist

- [x] Task 275 milestone audit accepted.
- [x] Task 276 release preparation completed at the immutable source.
- [x] Task 276 exact-head CI completed successfully.
- [x] Task 277 created the annotated `v0.23.0` tag.
- [x] The tag object and peeled target match the immutable release identity.
- [x] Tag CI completed successfully for the exact v0.23.0 source.
- [x] GitHub Release `385352229` is published, non-draft, non-prerelease, and
  has zero assets.
- [x] Task 278 records the post-release state without mutating publication
  identity.

The immutable v0.23.0 release source forever remains
`05527ce10cc088bbaa09fc6792e0f26f6c85ac2b`. The later Task 278 cleanup commit
is not the release source.
