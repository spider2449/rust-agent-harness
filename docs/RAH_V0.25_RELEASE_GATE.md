# RAH v0.25 Reviewed Rename/Move Milestone Audit and Release Readiness Gate

## Verdict

**Verdict B — NOT READY; CORRECTION REQUIRED**

The complete v0.25 milestone is not ready to enter release preparation. The
audit found a material ordinary ADR 0018 Windows case-equivalent Git collision
gap. Task 303 has completed the focused correction, but this gate remains
Verdict B until Task 304 independently re-audits the corrected boundary. This
gate does not authorize release preparation, version changes, tagging,
publication, or a GitHub Release.

## Milestone and release boundary

The v0.25 theme is **HostExplicit Reviewed File Rename/Move** through the
existing ordinary `repo.rename-file` Tool. v0.24.0 remains the current
immutable published release. v0.25 is an unreleased candidate milestone;
there is no `v0.25.0` tag and no GitHub Release.

Audit checkpoint:

- master: `3f82d1d894c8ccf804691ed04fb9745ae6e99a82`
- direct parent: `6f2637626248c2cabac2142357ef4e31e266ec4b`
- supplied exact-head CI: `34584375927` — PASS
- Task 301 live marker: `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`

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

The audit confirms the Task 292/294/296 corrections for repository identity,
supported `.git` form, Git executable identity, source exact HEAD/index/
worktree equality, source mode and link count, destination worktree and Git
absence checks, parent identity, reparse/symlink/junction rejection, nested
repository rejection, supported volume constraints, known-no-effect exact
preimage proof, independent post-effect proof, uncertain classification, and
one native attempt maximum.

One material gap remains. `destination_git_absent` currently asks Git for the
requested destination spelling and compares returned paths by exact bytes.
On Windows, a tracked `README.md` can remain in HEAD/index while absent from
the worktree, and a requested `readme.md` can evade that exact Git collision
check. The filesystem path-equivalence rules are case-insensitive on Windows,
so the native no-replace operation can create an alias that ADR 0018 requires
to be rejected. The existing exact-case tracked-destination and case-only
rename tests pass, but this tracked-but-missing case-equivalent collision is
not covered.

This was the material ordinary authority defect corrected by Task 303. The
correction compares independently queried HEAD and every index-stage candidate
under the existing Windows filesystem-equivalence rule, with bounded
case-insensitive index discovery and bounded component-wise HEAD tree
discovery. Deterministic real-Git coverage proves the tracked-but-missing
case-equivalent destination returns `precondition_failed` with zero native
attempts and no effect. Task 303 does not itself change this gate's Verdict B;
release readiness remains pending the independent Task 304 milestone
re-audit.

Task 303's corrected production candidate also passed the exact-head Windows
reviewed live route with marker `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.
Prepare Tool/native was `0/0`, Confirm Tool/native was `1/1`, final proof was
`ReviewedSuccess`, and terminal status was `renamed_verified`. HEAD, index, and
refs were preserved; Commit authorization was invalidated at the effect
boundary; model, MCP, and Process Plugin activity were zero; and Codex cleanup
was reaped successfully. This carried evidence does not change Verdict B or
replace the independent Task 304 milestone re-audit.

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

The reliable serial focused ordinary rename suite passed with 36 tests. The
Task 301 record reports 219 Desktop deterministic tests passed and 10 ignored,
frontend JavaScript and authority tests passed, Tauri permission tests passed,
and the Desktop release build passed. The complete Task 302 execution record
is:

| Check | Result |
| --- | --- |
| `rah-tools` reviewed rename preparation suite | PASS: 24 passed |
| `rah-tools` ordinary rename suite, serial | PASS: 36 passed |
| `rah-desktop` deterministic suite | PASS: 219 passed, 10 ignored |
| Frontend JavaScript syntax and authority tests | PASS: syntax, authority, and permission checks |
| Tauri permission test | PASS |
| Desktop release build | PASS |
| `cargo fmt --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS: all executed crate and integration tests passed; live/host-only tests remained ignored |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `git diff --check` | PASS before staging |
| `cargo metadata --no-deps --format-version 1` | PASS |

The initial parallel focused library run encountered a disposable fixture
setup collision; the required serial rerun passed all 36 tests. Ignored live
tests are not deterministic failures.

## Windows live certification carried forward

Task 301 evidence remains valid because this audit makes no production code
change and does not rerun the destructive live test. The certified evidence
records Windows 10 Professional build 19045 x64, Rust/Cargo 1.96.0, Git
2.54.0.windows.1, and certified Codex baseline 0.149.0 with SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

Prepare recorded zero Tool executions and zero native rename attempts.
Confirm recorded exactly one Tool execution and one native rename attempt.
The final proof was `ReviewedSuccess` and terminal status was
`renamed_verified`. HEAD, index, and refs were preserved. Commit authorization
was pending before and after Prepare and invalidated after Started/effect.
Model lifecycle, MCP, and Process Plugin counts were zero. Cleanup reaped
Codex, and the marker was `RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK`.

The live-test-support instrumentation is feature-gated observability only. It
adds counters and test diagnostics under `cfg(feature = "live-test-support")`
and cannot create a production mutation path. The evidence is host-driven
connected-current certification; it does not claim model-selected reviewed
rename dispatch.

## Documentation and dependency audit

README, CHANGELOG, ARCHITECTURE, SECURITY, ADR 0018, ADR 0021, ADR 0026, and
the relevant plans were checked. The release-facing documents correctly leave
v0.24.0 as the current published release; the new gate records v0.25 as
unreleased. The stale final line in the Task 301 plan was corrected to record
exact-head CI run `34584375927` as passed. No unrelated historical record was
rewritten.

The workspace remains on Rust edition 2024 and version `0.24.0`; no version
bump is part of this audit. No Cargo.toml or Cargo.lock dependency drift was
found between the supplied parent and checkpoint. There is no Trusted Profile
schema change or provider protocol change. Task 302 adds documentation and
plans only; it adds no public API or production capability.

## Known nonclaims

This milestone does not claim race-free TOCTOU, transactional rollback, OS
sandboxing, network isolation, Linux/macOS live parity, generic filesystem
mutation, HostExplicit `repo.commit`, HostExplicit provider tools,
cross-repository atomic mutation, or model-selected reviewed rename
certification.

## Remaining release prerequisites

Task 303 must correct and test the ordinary Windows case-equivalent Git
collision proof. A follow-up milestone audit must then confirm the ordinary
security foundation, rerun the required deterministic gates, and obtain
successful exact-head CI for the corrected master. Only a later explicit
release-preparation task may update release-facing version material. Tagging
and GitHub publication remain later explicit gates.

## Final decision

**Verdict B — NOT READY; CORRECTION REQUIRED**

The exact defect is the Windows case-equivalent tracked-but-missing Git
destination collision described in the ordinary security section. No release
preparation has started. No `v0.25.0` tag or GitHub Release was created.
