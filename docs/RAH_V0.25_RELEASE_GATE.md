# RAH v0.25.0 Release Gate — released historical record

## Status

RELEASED — HISTORICAL RECORD

RAH v0.25.0 was prepared at the Task 305 baseline and later published
immutably. v0.25.0 is the current immutable published release; v0.24.0 is the
prior immutable published release. Task 307 records documentation-only
post-release cleanup. Preparation baseline:
`b3532f52b79be9575f3f9d8e818efa2096da4e12`.

Task 304's decision was **Verdict A - READY FOR RELEASE PREPARATION**. This
task records only version/release documentation and Cargo workspace metadata;
it does not authorize product functionality, authority/security behavior,
ADR contracts, provider protocols, frontend behavior, tagging, publication,
or post-release cleanup.

The Task 305 release-candidate scope was exactly:

```text
Cargo.toml
Cargo.lock
CHANGELOG.md
README.md
docs/ARCHITECTURE.md
docs/SECURITY.md
docs/RAH_V0.25_RELEASE_GATE.md
docs/plans/2026-09-12-v0.25-release-preparation.md
```

The workspace is `0.25.0` across 13 packages, remains on
Rust edition 2024, and has no dependency drift. The certified Codex baseline
remains `codex-cli 0.149.0` with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

## Milestone and publication checklist

- [x] Task 302 preserved: **Verdict B - NOT READY; CORRECTION REQUIRED**.
- [x] Task 303 focused Windows case-equivalent Git collision correction
  completed.
- [x] Task 304 preserved: **Verdict A - READY FOR RELEASE PREPARATION**.
- [x] Task 305 prepared the immutable v0.25.0 release source.
- [x] Task 306 created the annotated `v0.25.0` tag and GitHub Release with no repository commit.
- [x] Task 307 completed documentation-only post-release cleanup.
- [x] Annotated tag and peeled source independently verified.
- [x] Tag CI and GitHub Release independently verified.
- [x] Publication checklist complete.

The immutable Task 305 release source and exact-head CI are recorded below.
The later Task 307 descendant is not the release source.

## Immutable publication record

- Release source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.
- Annotated tag: `v0.25.0`.
- Tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`.
- Peeled source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`.
- Tag message: `RAH v0.25.0`.
- Tagger timestamp: `2026-09-12T00:44:21Z`.
- Preparation CI: `34662233490` PASS.
- Tag CI: `34662635116` PASS.
- GitHub Release ID: `387406579`.
- GitHub Release name: `RAH v0.25.0`.
- Published: `2026-09-12T00:47:29Z`.
- Draft: `false`.
- Prerelease: `false`.
- Assets: `0`.

Task 305 prepared the immutable source and stopped before publication. Task
306 created the annotated tag and GitHub Release with no repository commit.
Task 307 is documentation-only cleanup and is not the v0.25.0 release source.

## Complete milestone history

- Task 302: Verdict B — NOT READY; CORRECTION REQUIRED.
- Task 303: Windows case-equivalent Git collision correction.
- Task 304: Verdict A — READY FOR RELEASE PREPARATION.
- Task 305: release preparation at the immutable release source.
- Task 306: immutable publication with no repository commit.
- Task 307: documentation-only post-release cleanup.

## Historical Task 304 milestone audit

## Verdict

**Verdict A — READY FOR RELEASE PREPARATION**

Task 302 remains historically **Verdict B — NOT READY; CORRECTION REQUIRED**
because it found a material ordinary ADR 0018 Windows case-equivalent Git
collision defect. Task 303 completed the separate focused production
correction. Task 304 independently re-audited the corrected boundary and found
it closed, with no new material release-readiness defect. This gate authorizes
only release preparation; it does not authorize version changes, tagging,
publication, or a GitHub Release.

## Milestone and release boundary

The v0.25 theme is **HostExplicit Reviewed File Rename/Move** through the
existing ordinary `repo.rename-file` Tool. At the historical Task 304
checkpoint, v0.25 was an unreleased candidate milestone with no tag or GitHub
Release. It was later published as recorded above.

Task 304 corrected checkpoint:

- master: `b2e170cc6f8226891f572dce6ce453abba836aaa`
- direct parent and Task 303 production correction: `1aa4d3277bd7ba24dec1caa3d52cc57c2aa2584a`
- supplied exact-head CI: `34602139743` — PASS
- Task 303 corrected live marker: `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`

The audited sequence is Task 290 v0.25 scope and authority roadmap, Task 291
contract research, Tasks 292/294/296 ADR 0018 conformance corrections, Task
297 ADR 0026 acceptance, Task 298 reviewed rename preparation, Task 299
HostExplicit coordinator integration, Task 300 Desktop/Tauri integration, and
Task 301 Windows connected-current live certification. The audit inspected
current code and documentation in addition to those historical records.

## Architecture and authority chain

The current reviewed route is:

```text
human typed source/destination
 -> zero-effect Prepare
 -> complete backend-derived review
 -> opaque process-local single-use ticket
 -> Confirm with ticket only
 -> connected-current / generation / composition / registry currentness
 -> capability-specific preparation revalidation
 -> exact ToolDefinition / explicit permission validation
 -> D2
 -> reviewed Commit authorization invalidation
 -> HostExplicit Started
 -> exactly one authorized_tool_dispatch
 -> current ToolRegistry
 -> existing repo.rename-file
 -> ADR 0018 RepositoryFileRenamePolicy
 -> strict ToolOutput classification
 -> independent ADR 0026 reviewed post-effect proof
 -> status-only terminal activity
 -> descriptive repository refresh
```

ADR 0018 is the sole ordinary file rename/move mutation authority. ADR 0021
owns the generic HostExplicit coordinator, currentness, opaque ticket, D2, and
provenance boundary. ADR 0026 owns the capability-specific human-reviewed
rename boundary. The audit found one ordinary ADR 0018 proof defect described
below; the reviewed route does not cure it.

There is one production `repo.rename-file` implementation. Desktop does not
provide a second native rename path: the frontend prepares and confirms the
host route, which dispatches through the current ToolRegistry to the existing
ordinary Tool.

Model requests are untrusted requests, not authorization. Frontend
confirmation, `PermissionLevel::Execute`, ToolRegistry membership, Trusted
Profile metadata, and provider metadata cannot create rename authority. MCP
Tools and Process Plugin Tools remain HostExplicit-ineligible. No generic
filesystem rename/write authority, shell shortcut, `git mv`, or copy-delete
implementation exists for this route.

## Ordinary Tool contract

The public ordinary Tool remains `repo.rename-file` with
`PermissionLevel::Execute` and this exact closed input schema:

```json
{
  "source_path": "string, 1..=1024",
  "destination_path": "string, 1..=1024",
  "expected_source_file_sha256": "lowercase 64-hex string",
  "expected_source_file_byte_length": "integer, 0..=1048576"
}
```

All four fields are required and unknown fields are rejected. The ordinary
contract retains the ADR 0018 repository, source, destination, and path
constraints, including supported `.git` form and identity, canonical root,
Git executable identity, exact source HEAD/index/worktree equality, supported
mode and link count, destination absence, parent identities, reparse/symlink/
junction rejection, nested repository rejection, supported volume constraints,
and Windows alias/case protections.

The ordinary result statuses are `renamed_verified`, `known_no_effect`,
`precondition_failed`, `invalid_input`, and `uncertain`. The native operation
is one no-replace attempt at most. There is no retry, replay, rollback,
reverse rename, copy-delete, `git mv`, or shell path.

## Ordinary security foundation

The audit confirms the Task 292/294/296 corrections and the Task 303 fix for
repository identity, supported `.git` form, Git executable identity, source
exact HEAD/index/worktree equality, source mode and link count, destination
worktree and Git absence checks, parent identity, reparse/symlink/junction
rejection, nested repository rejection, supported volume constraints,
known-no-effect exact preimage proof, independent post-effect proof, uncertain
classification, and one native attempt maximum.

Task 303's ordinary correction is independently confirmed closed.
`destination_git_absent` checks HEAD and the index independently. Windows index
discovery uses bounded `--icase-pathspecs`; HEAD uses bounded
component-by-component `ls-tree -z HEAD --` discovery that follows actual tree
spellings at each relevant depth. Both are narrowing mechanisms only. Strict
parsing and host-owned component-wise `paths_equivalent` comparison remain the
final collision authority. All index stages, intent-to-add, conflict forms,
malformed/ambiguous records, and observation timeout/overflow/failure fail
closed. The tracked-but-missing `README.md` versus requested `readme.md`
regression returns `precondition_failed` with zero native attempts and no
effect, before native rename.

Task 303's corrected production candidate also passed the exact-head Windows
reviewed live route with marker `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.
Prepare Tool/native was `0/0`, Confirm Tool/native was `1/1`, final proof was
`ReviewedSuccess`, and terminal status was `renamed_verified`. HEAD, index, and
refs were preserved; Commit authorization was invalidated at the effect
boundary; model, MCP, and Process Plugin activity were zero; and Codex cleanup
was reaped successfully. This carried evidence was independently reviewed by
Task 304 and remains applicable because Task 304 changes documentation only.
It does not claim model-selected rename certification.

## Reviewed preparation

The Task 298 preparation foundation remains coherent in current code:

- human input contains exactly `source_path` and `destination_path`;
- Prepare performs no filesystem rename or other effect;
- source review is strict UTF-8, NUL-free, and bounded to 65,536 raw bytes;
- complete review is backend-derived, bounded, and never truncated;
- ignored destinations fail closed;
- ordinary ToolInput is canonical and host-derived;
- private preparation binds repository, filesystem, Git/index, ToolDefinition,
  ToolInput, review identity, and preparer evidence;
- revalidation detects content, identity, repository, definition, and currentness
  drift; and
- private preparation is retained in process memory and is absent from generic
  activity serialization.

## Proof and coordinator ordering

Ordinary Tool status alone cannot establish reviewed success. `renamed_verified`
requires independent fresh post-effect proof. `known_no_effect` requires an
independent exact preimage proof, and same-byte identity replacement cannot be
accepted as proof of no effect. Malformed, contradictory, or lost results
after a possible effect remain `uncertain`. Uncertainty does not retry,
replay, reverse, roll back, or compensate.

Confirm consumes the ticket and checks connected-current generations,
composition, repository identity, current ToolRegistry, exact ToolDefinition,
explicit permission, and capability-specific preparation revalidation before
D2. Reviewed Commit authorization is invalidated after D2 and before
`Started`; only then can the one authorized dispatch occur. Prepare, Cancel,
and pre-effect rejection preserve Commit authorization. After `Started`, an
uncertain outcome never restores it.

## HostExplicit eligibility

The current exact eligible set contains 11 capabilities:

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

`repo.rename-file` requires a connected current `RepositoryHost` source and
the current rename preparer. `repo.create-directory`, `repo.commit`, MCP
Tools, Process Plugin Tools, fixtures/diagnostics, unknown Tools, and
provider-defined Tools are ineligible. No wildcard, prefix, effect-category,
permission-derived, or provider-derived admission exists.

## Desktop and Tauri boundary

The Desktop frontend exposes rename only when backend Effective Authority says
the connected HostInvocation is eligible. It accepts exactly two human path
fields and calls only `host_prepare_repo_rename_file` for Prepare. It displays
the complete backend-derived review and does not recompute hashes or security
state. Confirm and Cancel send only the opaque ticket through existing
commands. The frontend does not access the filesystem and provides no generic
retry, force, overwrite, create-parent, Stage, or Commit control.

The statuses `renamed_verified`, `known_no_effect`, `precondition_failed`,
`invalid_input`, and `uncertain` render conservatively. The Tauri command
manifest contains `host_prepare_repo_rename_file`; the generated permission is
exactly `allow-host-prepare-repo-rename-file`, and the default capability opts
into that permission. No generic filesystem permission was added. Existing
Confirm/Cancel commands and permissions remain in use.

## Privacy and provenance

Generic activity does not persist the ticket, complete source review/content,
source hash, ToolInput, native filesystem paths, FileIdentity, Git/index
evidence, repository identity, preparer identity, or authority-bearing private
preparation. The frontend may display the complete reviewed source content in
the direct review response, but that display does not become persistent
authority. Host activity uses status-only terminal data and separate
non-authority activity correlation.

## Deterministic validation

Task 304 reran the focused and workspace validation sequentially. The complete
Task 304 execution record is:

| Check | Result |
| --- | --- |
| `rah-tools` reviewed rename preparation suite | PASS: 24 passed |
| `rah-tools` ordinary rename suite, serial | PASS: 41 passed |
| `rah-desktop` deterministic suite | PASS: 219 passed, 10 ignored |
| Frontend JavaScript syntax and authority tests | PASS: syntax, authority, and permission checks |
| Tauri permission test | PASS |
| Desktop release build | PASS |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS: all executed crate and integration tests passed; live/host-only tests remained ignored |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS |
| `cargo metadata --no-deps --format-version 1` | PASS: 13 packages, version `0.24.0`, edition 2024 |

The four Windows correction cases passed within the 41-test ordinary suite,
including tracked-but-missing HEAD, index-only, intent-to-add, and conflict
stages. Each collision was rejected before a native attempt. Ignored live tests
are host-only evidence gates, not deterministic failures.

## Windows live certification carried forward

Task 303's corrected production live evidence remains valid because Task 304
makes documentation-only changes and does not rerun the destructive live test.
The corrected production commit is the direct parent
`1aa4d3277bd7ba24dec1caa3d52cc57c2aa2584a`; checkpoint `b2e170c` contains
validation-documentation changes only. The certified environment was Windows
10 Professional build 19045 x64, Rust/Cargo 1.96.0, Git 2.54.0.windows.1,
and `codex-cli 0.149.0` with executable SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

Prepare recorded zero Tool executions and zero native rename attempts.
Confirm recorded exactly one Tool execution and one native rename attempt.
The final proof was `ReviewedSuccess` and terminal status was
`renamed_verified`. HEAD, index, and refs were preserved. Commit authorization
was pending before and after Prepare and invalidated after Started/effect.
Model lifecycle, MCP, and Process Plugin counts were zero. Cleanup reaped
Codex successfully, protected HEAD/index/refs were preserved, and the marker
was `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.

The live-test-support instrumentation is feature-gated observability only. It
adds counters and test diagnostics under `cfg(feature = "live-test-support")`
and cannot create a production mutation path. The evidence is host-driven
connected-current certification; it does not claim model-selected reviewed
rename dispatch.

## Documentation and dependency audit

README, CHANGELOG, ARCHITECTURE, SECURITY, ADR 0018, ADR 0021, ADR 0026, and
the relevant plans were checked. The release-facing documents record v0.25.0
as the current published release and v0.24.0 as prior. Task 302 remains
historical Verdict B, Task 303 remains the focused correction, and Task 304 is
the independent readiness decision. No unrelated historical record was
rewritten.

The workspace remains on Rust edition 2024 and version `0.25.0`; no version
change is part of this cleanup. Metadata reports 13 packages. No Cargo.toml or
Cargo.lock dependency drift was found between the supplied parent and
checkpoint or cleanup. There is no Trusted Profile schema change, MCP protocol
change, Process Plugin protocol change, or Codex baseline change.

## Known nonclaims

This milestone does not claim race-free TOCTOU, transactional rollback, OS
sandboxing, network isolation, Linux/macOS live parity, generic filesystem
mutation, HostExplicit `repo.commit`, HostExplicit provider tools,
cross-repository atomic mutation, or model-selected reviewed rename
certification.

## Publication and post-release checklist

- [x] Independently review the prepared source and exact-head preparation CI.
- [x] Create and verify the annotated `v0.25.0` tag.
- [x] Run and verify terminal exact-source tag CI `34662635116`.
- [x] Create and independently audit the GitHub Release.
- [x] Reconfirm v0.24.0 publication identity.
- [x] Complete Task 307 as documentation-only cleanup in the authorized
  seven-file scope.

The immutable v0.25.0 release source forever remains
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`. The later Task 307 documentation
cleanup commit is not the release source and does not modify any publication
object.

## Final decision

**Verdict A — READY FOR RELEASE PREPARATION**

Task 303's Windows case-equivalent tracked-but-missing Git destination defect
is independently confirmed closed. No new material ordinary ADR 0018,
reviewed-route, frontend/Tauri, privacy, dependency, version, or release
documentation defect was found. Task 305 prepared the source, Task 306
published it, and Task 307 records the later documentation-only cleanup.
