# Task 404 — Effective Authority Snapshot Seam Independent Audit

Date: 2026-09-24

## Verdict and checkpoint

**PASS — EFFECTIVE AUTHORITY SNAPSHOT SEAM INDEPENDENTLY VERIFIED**

- Starting HEAD: 5f152d1de5f4dc17f632f4f4fca45e5d07dd92a9 — docs: design effective authority snapshot seam.
- Initial git status matched the exact expected dirty state:
  - M crates/rah-desktop/src/main.rs
  - M crates/rah-desktop/src/effective_authority.rs
  - ?? docs/plans/2026-09-24-task-403-effective-authority-snapshot-seam-implementation.md
- Initial source diff touched only the two expected production files. No unrelated file, Cargo/dependency, frontend, ADR, or architecture change was present.
- Source was compared directly against the starting commit and frozen Task 402 design. Task 403 prose was treated as a claim to verify.

## Baseline root helper

At HEAD, get_effective_authority_snapshot(State<DesktopAppState>) simply calls private effective_authority_snapshot_for_state; both remain in main.rs. The baseline helper behavior was:

1. Clone the active repository Arc under the repository mutex, then capture repository generation, then model generation. Temporary guards drop at the end of each statement.
2. Acquire and retain the connection guard, then acquire and retain the repository workflow guard, then clone Trusted Profile selection under its mutex. That temporary guard drops at the statement end.
3. For Connected, while holding the connection guard, lock/read current profile generation, connection generation, and Commit-identity generation in that order. Compare captured/current repository, model, and profile generations for context_current; call shared root connection_publication_is_current with the same five repository/model/profile/connection/identity operands; compute selected || captured_repository_generation == 0; compare registry definition count to published Tool count; combine those four booleans into current; map current/context to ConnectedCurrent, Stale, or ReconnectRequired; and capture source and repository/model/connection generations.
4. Map other connection phases: Connecting → Connecting; Disconnecting → Stale; Error(_) → Unavailable; NotConnected → Disconnected when selected and otherwise NoRepository. Their advertisements and captured generations are absent/false.
5. Clone allowed permissions while connection is held, then explicitly drop the connection guard. Clone published Tools, acquire the HostInvocation coordinator mutex, call reap_expired(Instant::now()), read coarse CoordinatorState, and drop the coordinator guard at the end of that block.
6. Derive branch-authority presence; clone published unavailable rows; derive configured BuiltIn/Trusted Profile summary and counts; derive a safe repository basename; then map Tool advertisements and HostExplicit descriptors, add configured external unavailable rows only when composition is absent (ProviderUnavailable for connection error, ProviderNotEffective otherwise), map repository identity/generation/display, map reviewed Commit presentation from workflow authorization plus selected status, and assemble schema version 1.

The workflow guard remains held through final return. Poisoned locks recover with PoisonError::into_inner; the helper returns the same infallible snapshot type. The snapshot is a best-effort observation gathered at multiple points, not a transactionally coherent snapshot.

## Carrier and ownership

Private carrier: main.rs::EffectiveAuthoritySnapshotInputs (main.rs:2220-2247), with private fields:

| Field | Exact type |
|---|---|
| selected | bool |
| repository_display_name | Option<String> |
| current_repository_generation | u64 |
| captured_repository_generation | Option<u64> |
| connection_state | ConnectionBindingState |
| runtime_source | Option<CodexExecutableSource> |
| captured_model_generation | Option<u64> |
| captured_connection_generation | Option<u64> |
| context_current | bool |
| publication_current | bool |
| repository_context_matches | bool |
| registry_tool_count_matches | bool |
| composition_present | bool |
| allowed_permissions | Vec<PermissionLevel> |
| effective_tools | Vec<EffectiveToolEntry> |
| unavailable_capabilities | Vec<UnavailableCapability> |
| configured | ConfiguredSummary |
| configured_external_tools | Vec<ExternalToolDescriptor> |
| branch_authority_present | bool |
| patch_preparer_present | bool |
| multi_file_edit_preparer_present | bool |
| create_file_preparer_present | bool |
| delete_file_preparer_present | bool |
| rename_file_preparer_present | bool |
| coordinator_state | CoordinatorState |
| commit_authorization | CommitAuthorizationPresentation |

Fields are copied facts, owned strings/vectors, or existing closed presentation enums/records. There is no DesktopAppState, DesktopRepository, repository path, membership handle, authority/preparer object, Trusted Profile/provider/runtime owner, ToolRegistry, workflow owner, HostInvocation coordinator/reference, prepared ticket/payload, authorization handle, dispatch closure, or persistence handle. No mutable owner crosses.

**LIVE OWNER CROSSING: NONE**

## Visibility and root ownership

The exact new production visibility is effective_authority::compose_effective_authority_snapshot -> pub(super) at effective_authority.rs:633. The carrier and fields remain private; no pub(crate) or public API was added. Test imports of RepositoryKind and SnapshotStatus in main.rs are separately gated by cfg(test) and cfg(target_os = "windows"). Existing module APIs were not widened by this diff.

**Carrier visibility: PASS. Compose visibility: PASS.**

Root still owns the Tauri command and registration, DesktopAppState access, locks and poison recovery, repository/model/profile/provider live reads, all generation acquisition/comparison, repository-context and registry-count checks, safe display-name derivation, configured-profile summary and sanitized descriptor capture, capability/preparer-presence observations, expiry cleanup, carrier construction, and compose invocation. effective_authority.rs has no live-state access or lifecycle ownership.

## Lock topology and temporal consistency

| Sequence / release | Baseline at HEAD | Current implementation |
|---|---|---|
| Initial observations | repository → repository generation → model | Same |
| Long-lived guards | connection → workflow → profile selection clone | Same; profile mutex guard drops after clone |
| Nested connected observations | current profile generation → current connection generation → current identity generation, while connection held | Same |
| Connection release | after permission capture and before Tool clone/coordinator access | Same explicit drop(connection) point |
| Expiry/coordinator | coordinator lock → reap_expired(Instant::now()) → state(); lock drops at block end | Same call, owner, ordering, and drop point |
| Workflow release | implicit at helper return after final DTO assembly | Retained through compose and return |
| Reacquisition/new nesting | None | None |

No lock order, lifetime, drop point, or observation order changed. Gathering remains split across multiple reads, with existing generation checks as stale/current detection. No atomic snapshot claim, giant lock, or post-carrier reread was added.

**LOCK TOPOLOGY: SEMANTICALLY UNCHANGED**

**TEMPORAL CONSISTENCY SEMANTICS: UNCHANGED**

## Currentness and expiry

| Comparison | Baseline | Current |
|---|---|---|
| Context generations | [captured repository, model, profile] == [current repository, model, profile] under connection lock | Same operands/expression remain in root at main.rs:2321-2329 |
| Published composition currentness | Shared connection_publication_is_current on captured/current repository, model, profile, connection, identity generations | Same root helper and operands at main.rs:2330-2344; helper unchanged |
| Repository context | selected || captured_repository_generation == 0 | Same root expression at main.rs:2346 |
| Registry inventory | composition.registry.definitions().len() == composition.tools.len() | Same root expression captured as registry_tool_count_matches at main.rs:2347-2348 |
| Reported current | conjunction of the four facts above | Same four-fact conjunction in compose at effective_authority.rs:638-641 |

No comparison moved out of the root gathering point. Compose derives only final snapshot-level status/current and presentation bits from the same closed facts.

**CURRENTNESS SEMANTICS: UNCHANGED**

Baseline and current expiry remain inside the root helper after dropping connection and before capturing coordinator state, under the same mutable coordinator lock. The call remains coordinator.reap_expired(std::time::Instant::now()), followed immediately by coordinator.state(). effective_authority.rs contains no expiry call and receives only copied CoordinatorState; compose cannot mutate workflow/coordinator state.

**EXPIRY SEMANTICS: UNCHANGED**

## Compose purity and branch equivalence

compose_effective_authority_snapshot consumes the owned carrier and constructs a closed output. It performs no I/O, persistence, process/network/runtime dispatch, locking, ticket operation, coordinator/registry/repository mutation, or owner lookup. Its callees are existing pure label/unavailable/Commit mapping functions and host_descriptor_with_rename, which derives a descriptor from closed Tool and eligibility facts.

| Baseline root branch/mapping | Current location | Equivalence |
|---|---|---|
| Connection phase mapping and status | Root captures closed phase at main.rs:2362-2409; compose at effective_authority.rs:636-662 | Same five branches and labels |
| Connected current/stale/reconnect | Root currentness facts; compose at effective_authority.rs:637-649 | Same conjunction and context_current fallback |
| Connection binding, runtime/source labels, captured generations | effective_authority.rs:663-672 | Same codex label only for Connected; same mapper and values |
| Connection/Tool advertisement | effective_authority.rs:671, 673-676 | Same advertisement equals combined currentness |
| HostExplicit availability descriptors | effective_authority.rs:674-688 calls unchanged host mapper with same facts | Same predicates and inputs; presentation only |
| Configured external unavailable rows | effective_authority.rs:690-703 | Same only when composition absent; same reason and descriptor mapping |
| Repository identity/generation/display | effective_authority.rs:704-724 | Same selected/current/stale/unknown/not-selected branches and safe display input |
| Reviewed Commit presentation | effective_authority.rs:734 | Same existing mapper and copied authorization plus selected flag |
| Final schema-v1 snapshot assembly | effective_authority.rs:726-735 | Same fields and values; no schema change |

Changed root syntax captures closed inputs and the registry boolean, clones configured external descriptors, and invokes compose. No predicate changed. The sanitized descriptor clone comes from the already-cloned profile selection and is used for output only when composition is absent, as before.

**COMPOSE PURITY: PASS**

**BRANCH EQUIVALENCE: PASS**

## Authority, IPC, presentation, and error audits

- **HostExplicit:** host_kind still defines exactly 11 kinds before and after: fs.read, repo.file-info, repo.status, repo.diff, repo.diff-staged, repo.create-branch, repo.patch, repo.edit-files, repo.create-file, repo.delete-file, and repo.rename-file. Eligibility remains the existing host_descriptor_with_rename predicate chain. Compose issues/inspects/consumes no ticket and does not authorize dispatch or change permissions/kinds.
  - **HOSTEXPLICIT AUTHORITY DELTA: NONE**
- **Repository authority:** only selected/display/generation and existing presence facts cross; no repository/path/membership/authority object. Compose admits, activates, grants, or mutates nothing.
  - **REPOSITORY AUTHORITY DELTA: NONE**
- **Trusted Profile/provider:** only configured summary, sanitized external descriptors, published Tool/unavailable presentation, connection phase/source, and currentness/readiness facts cross. No Trusted Profile, provider, runtime, registry, or activation owner crosses.
  - **TRUSTED PROFILE / PROVIDER AUTHORITY DELTA: NONE**
- **Commit:** only copied CommitAuthorizationPresentation and selected bool cross. Workflow owner, token/ticket, capability, and identity mutation do not cross or move.
  - **COMMIT AUTHORIZATION DELTA: NONE**
- **IPC/schema v1:** DTO definitions, serialization attributes and field assembly remain schema version 1 with the same fields, enums/labels, unavailable rows, Tool/HostExplicit descriptor shapes, and Commit presentation. Safe repository display remains computed in root; no path/provider/runtime internals, tickets, handles, or aliases are introduced.
  - **IPC/SCHEMA DELTA: NONE**
- **Presentation/redaction:** basename sanitizer, configured summary, source-label mapper, sanitized external descriptor/unavailable mapping, phase/currentness labels, HostExplicit reason mapper, and reviewed Commit mapper retain their implementations and call semantics.
  - **PRESENTATION/REDACTION DELTA: NONE**
- **Errors:** handler/helper return type remains EffectiveAuthoritySnapshot; all root mutex poison recovery remains PoisonError::into_inner; user-facing errors/fallbacks are untouched; compose is infallible and adds no error path.
  - **ERROR CONTRACT: UNCHANGED**

## Test and caller audit

Four new deterministic module-local tests were added:

1. snapshot_composition_preserves_current_stale_and_reconnect_reporting — current, stale, reconnect status and advertisement.
2. snapshot_composition_keeps_host_explicit_availability_presentation — eligible current presentation and busy coordinator availability.
3. snapshot_composition_keeps_the_eleven_host_explicit_kinds — all 11 existing HostExplicit kinds with closed descriptor presentation.
4. composed_snapshot_keeps_closed_schema_and_safe_presentation — schema-v1 fields, labels, safe repository name, and redaction.

Fixtures construct only the closed carrier, Tools, coordinator enum values, and closed presentation rows. They do not construct DesktopAppState, a large root fixture, live owners, tickets, or executable authority. Test-only imports are cfg-bounded; no production visibility widening was needed.

Existing tests were not moved, renamed, deleted, rewritten, unignored, or newly ignored. main_tests.rs is unchanged; existing effective_authority module tests are unchanged and the diff only appends the four tests and their fixture. Test output reports 18 ignored tests, with the same live/certification identities visible. No ignored/live test caller or helper call was edited.

Production topology remains Tauri registration -> get_effective_authority_snapshot -> root gatherer -> compose_effective_authority_snapshot. Registration at main.rs:9827 still names the same handler. No production caller bypasses root gathering. Root tests continue calling the same command/private helper; only module-local compose tests call compose directly.

## Scope, counts, and validation

- Initial source diff was bounded to main.rs and effective_authority.rs; no unrelated formatting, neighboring refactor, rename, permission change, Cargo/dependency, frontend, or ADR change was found.
- Independently counted lines: main.rs 9,879 at HEAD → 9,870 current; effective_authority.rs 906 at HEAD → 1,203 current. Diff numstat: main.rs 133 added / 142 removed; effective_authority.rs 298 added / 1 removed.
- Moved source consists of connection/status/binding derivation, Tool advertisement and HostExplicit descriptor projection, unavailable external mapping, repository binding, reviewed Commit mapping, and final snapshot assembly. Root now captures closed inputs and existing booleans. Module growth also includes compose and four tests/fixture; line count alone is not evidence of equivalence.
- Baseline package count: 315 passed, 18 ignored, 333 discovered. Current independently run result: 319 passed, 0 failed, 18 ignored, 337 discovered. The diff adds exactly four tests and removes/edits none.
- cargo fmt --check: PASS (exit 0).
- cargo test -p rah-desktop -- --test-threads=1: PASS — 319 passed, 0 failed, 18 ignored, 337 discovered; finished in 694.21s.
- cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings: PASS (exit 0).
- git diff --check: PASS; rerun after this record and before commit.
- Workspace validation: NOT RUN. This is a Desktop-only private module seam with no shared crate API, public API, Cargo/dependency, or cross-crate type movement; focused validation passed.
- Windows live certification: NOT RUN. Tauri handler interface/registration, ignored/live tests, authority behavior, HostExplicit set, dispatch and ticket semantics are unchanged. The audit did not warrant live certification.

## Closeout

- Audit artifact: docs/plans/2026-09-24-task-404-effective-authority-snapshot-seam-independent-audit.md.
- Intended commit files only: crates/rah-desktop/src/main.rs, crates/rah-desktop/src/effective_authority.rs, docs/plans/2026-09-24-task-403-effective-authority-snapshot-seam-implementation.md, and this Task 404 audit record.
- Authorized commit message: refactor: separate effective authority snapshot composition.
- No push or tag was performed.
- Recommended next task: **Task 405 — Effective Authority Observation / Expiry Contract Clarification**, docs/research only. Resolve the narrow wording tension: snapshot inspection does not change executable authority decisions, but can reap an already-expired, unusable prepared ticket. Do not change expiry behavior or authority policy. Do not start Task 405 automatically.
