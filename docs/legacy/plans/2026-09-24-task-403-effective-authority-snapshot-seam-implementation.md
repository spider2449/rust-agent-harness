# Task 403 — Effective Authority Snapshot Seam Implementation

## Checkpoint and scope

- Starting HEAD: `5f152d1de5f4dc17f632f4f4fca45e5d07dd92a9` (`docs: design effective authority snapshot seam`).
- Starting worktree: clean.
- Task 402 frozen design: root owns state access, locking, generation comparisons, expiry cleanup, and gathering; `effective_authority.rs` owns deterministic closed snapshot derivation and presentation. One root-private owned carrier and one `pub(super)` compose function are permitted.
- Production files changed: `crates/rah-desktop/src/main.rs` and `crates/rah-desktop/src/effective_authority.rs` only.
- No IPC, schema, permission, dependency, lifecycle, or authority-owner changes are in scope.

## Baseline

`get_effective_authority_snapshot` remains the Tauri command and delegates to private `effective_authority_snapshot_for_state`. Before this change, the helper gathered owner state and also assembled connection/repository bindings, reported status, advertisement flags, HostExplicit descriptors, configured and unavailable capabilities, reviewed Commit state, and the schema-version-1 snapshot.

The preserved lock and observation sequence is repository → repository generation → model → connection → workflow → profile selection. While connection remains held, the connected branch reads profile, connection, and identity generations in that order and computes context, publication, repository-context, and registry/tool-count facts. It copies allowed permissions, then drops connection. While workflow remains held, root clones classified Tool/unavailable observations, locks HostInvocation, calls `reap_expired(Instant::now())`, copies coarse coordinator state, and releases that lock. Root then captures repository capability/preparer presence, safe repository display name, profile summary/descriptors, and Commit presentation before composing; workflow remains held through return.

The existing serialized type remains `EffectiveAuthoritySnapshot` schema version 1 with fields `schemaVersion`, `status`, `repository`, `connection`, `configured`, `effectiveTools`, `unavailableCapabilities`, and `reviewedCommit`. Existing nested field names, closed enum values, labels, and omission rules are unchanged.

## Carrier and responsibility split

The private root-owned `EffectiveAuthoritySnapshotInputs` at `main.rs:2220` has these private fields containing only copied or owned closed observations:

| Field | Type | Captured fact |
|---|---|---|
| `selected` | `bool` | Active repository presence |
| `repository_display_name` | `Option<String>` | Root-sanitized basename presentation |
| `current_repository_generation` | `u64` | Current repository generation |
| `captured_repository_generation` | `Option<u64>` | Published connection generation |
| `connection_state` | `ConnectionBindingState` | Closed connection phase |
| `runtime_source` | `Option<CodexExecutableSource>` | Closed executable-source variant |
| `captured_model_generation` | `Option<u64>` | Published model generation |
| `captured_connection_generation` | `Option<u64>` | Published connection generation |
| `context_current` | `bool` | Root comparison of repository/model/profile generations |
| `publication_current` | `bool` | Existing five-generation publication comparison |
| `repository_context_matches` | `bool` | Existing selected-repository context rule |
| `registry_tool_count_matches` | `bool` | Existing publication/registry consistency check |
| `composition_present` | `bool` | Published composition presence |
| `allowed_permissions` | `Vec<PermissionLevel>` | Published allowed permissions |
| `effective_tools` | `Vec<EffectiveToolEntry>` | Classified closed Tool presentation |
| `unavailable_capabilities` | `Vec<UnavailableCapability>` | Closed unavailable entries |
| `configured` | `ConfiguredSummary` | Profile/BuiltIn summary |
| `configured_external_tools` | `Vec<ExternalToolDescriptor>` | Sanitized configured descriptors |
| `branch_authority_present` | `bool` | Existing branch authority presence |
| `patch_preparer_present` | `bool` | Existing patch preparer presence |
| `multi_file_edit_preparer_present` | `bool` | Existing multi-file preparer presence |
| `create_file_preparer_present` | `bool` | Existing create-file preparer presence |
| `delete_file_preparer_present` | `bool` | Existing delete-file preparer presence |
| `rename_file_preparer_present` | `bool` | Existing rename-file preparer presence |
| `coordinator_state` | `CoordinatorState` | Coarse state after expiry cleanup |
| `commit_authorization` | `CommitAuthorizationPresentation` | Copied closed Commit presentation |

No live owner/reference crosses the carrier: it contains no `DesktopAppState`, repository object/path, profile/provider/runtime object, registry, preparer, workflow/coordinator reference, authority handle, prepared ticket/payload, dispatch closure, or persistence handle.

Root continues to own Tauri dispatch, state access, lock/poison handling, generation comparisons, repository/profile gathering, safe display-name derivation, summary/descriptor capture, expiry cleanup, and carrier construction. The module owns final reported status/currentness, connection/repository presentation, HostExplicit descriptors, external-unavailable mapping, reviewed Commit mapping, and complete snapshot construction.

## Seam and invariant audit

- Compose function: `effective_authority::compose_effective_authority_snapshot` at `effective_authority.rs:633-736`, one `pub(super)` visibility increase, infallible and deterministic.
- Carrier and every carrier field remain private. No other visibility was widened.
- Generation equality expressions and their lock/comparison point remain in root. The module combines only the captured boolean facts using the existing current/stale/reconnect branches.
- Lock acquisition/release order is unchanged. Connection is still dropped before HostInvocation is locked; workflow remains held through composition/return.
- `HostInvocationCoordinator::reap_expired` remains in root at the prior coordinator lock location immediately before `state()` capture. Expiry policy/timing is unchanged.
- HostExplicit still reports the fixed 11 kinds. Eligibility predicates are passed unchanged to `host_descriptor_with_rename`; dispatch, ticket issuance/consumption, and authorization remain with their existing owners. `HOSTEXPLICIT AUTHORITY DELTA: NONE`.
- Repository membership/admission/activation and repository authority ownership remain unchanged. `REPOSITORY AUTHORITY DELTA: NONE`.
- Trusted Profile selection, provider activation, and runtime lifecycle remain unchanged. Only configured summary and sanitized descriptors cross the seam. `TRUSTED PROFILE/PROVIDER AUTHORITY DELTA: NONE`.
- Commit workflow and authorization ownership remain unchanged; the copied closed presentation maps through existing `reviewed_commit`. `COMMIT AUTHORIZATION DELTA: NONE`.
- Schema/redaction/status/source/tool mappings are mechanically relocated without a contract change. `IPC/SCHEMA DELTA: NONE`.
- Temporal semantics remain non-transactional; no atomicity claim or new lock was introduced.

## Branch movement

Moved into composition: connected current/stale/reconnect result, non-connected status mapping, connection binding and advertisement, per-Tool advertisement and HostExplicit descriptor derivation, configured external unavailable reason/mapping, repository binding identity/generation/display projection, reviewed Commit mapping, and final snapshot assembly.

Retained in root: all gathering and lock handling; generation equality/currentness facts; repository-context and registry/tool-count comparisons; expiry cleanup; permission and capability/preparer observation; safe display-name derivation; configured profile summary and sanitized descriptor capture; and private helper/handler entry points.

No predicate was changed. No generation comparison, lock, or `reap_expired` call moved. No serialized mapping changed.

## Tests and validation

New module-local deterministic tests cover:

- `snapshot_composition_preserves_current_stale_and_reconnect_reporting`
- `snapshot_composition_keeps_host_explicit_availability_presentation`
- `snapshot_composition_keeps_the_eleven_host_explicit_kinds`
- `composed_snapshot_keeps_closed_schema_and_safe_presentation`

Existing module classification/serialization tests remain in place. No root integration, ignored/live, or certification test was moved or edited. `main_tests.rs` and all direct handler/helper test callers remain unchanged.

Validation results:

- `cargo fmt --check`: PASS.
- `cargo test -p rah-desktop -- --test-threads=1`: PASS — 319 passed, 0 failed, 18 ignored, 337 discovered. Compared with baseline, four ordinary deterministic tests were added; all 18 ignored package-wide tests remain accounted for.
- `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- Workspace validation: not planned; no cross-crate API or dependency impact.
- Windows live certification: not run; existing ignored/live interfaces and executable-authority scenarios are unchanged.

## Files, line counts, and verdict

Expected dirty files are the two production files above and this plan. No commit, push, or tag is authorized.

- `main.rs` lines before/after: 9,879 / 9,870.
- `effective_authority.rs` lines before/after: 906 / 1,203.
- Exact dirty files: `crates/rah-desktop/src/main.rs`, `crates/rah-desktop/src/effective_authority.rs`, and `docs/plans/2026-09-24-task-403-effective-authority-snapshot-seam-implementation.md`.
- Final git status: those two modified Rust files and this untracked plan only.
- Verdict: **PASS — EFFECTIVE AUTHORITY SNAPSHOT SEAM IMPLEMENTED**.
- Recommended next task on success: **Task 404 — Effective Authority Snapshot Seam Independent Audit**. Do not start it automatically.
