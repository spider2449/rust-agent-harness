# Task 388 — `rah-desktop` Production Domain Decomposition Research

## Verdict

**Select a smaller third domain first: move only the descriptive remembered-workspace catalog presentation and six catalog/reveal Tauri adapters into a focused `remembered_workspace_commands` module.** Do not extract repository activation/lifecycle or HostExplicit first. Both large candidates cross many shared state fields, lock-order/currentness rules, and frontend command surfaces. The remembered catalog adapter group is smaller, has no repository execution authority, and already has a lower-level owner in `remembered_workspace.rs`; a separate command-edge module preserves that module's intentionally frontend-free boundary.

This is research only. No Rust source was moved or changed.

## 1. Starting checkpoint and scope

- Starting `HEAD`: `1f770c62e0e2c6a8f98f70591cd5f1ce6a3c34b5` — `refactor: extract desktop test module`.
- Parent: `a1d240da2568e61ff117d8728307891a6e04b826`.
- `git status --short` was empty before this document was created.
- The starting checkpoint matches the requested Task 387 checkpoint. `origin/master` was not queried or used in this task; the existing remote push is not attributed to Task 387.
- Task 387's audited extraction left production lines 1–9,929 in `main.rs`, followed by the external test-child declaration at lines 9,930–9,931.
- This task is documentation-only. No Rust/Cargo validation was run or is required.

## 2. Current production-file measurements

Measured from the current `crates/rah-desktop/src/main.rs` using its LF-delimited source and anchored top-level declaration scan:

| Measure | Result | Method / boundary |
| --- | ---: | --- |
| Physical lines | 9,931 | `Get-Content` source-line count; file ends with the Windows-only test child declaration. |
| Nonblank lines | 9,531 | Lines whose trimmed contents are non-empty. |
| Root item headers, total | 341 | Anchored declarations: 208 `fn`, 62 `struct`, 31 `enum`, 14 `impl`, 13 `const`, 2 `static`, and 11 `mod`. |
| Production root item headers | 331 | Total less nine root-level `#[cfg(test)]` hooks/helpers and the one `#[cfg(all(test, target_os = "windows"))] mod tests` declaration. Modules and `impl` blocks count as headers. |
| `#[tauri::command]` attributes | 47 | Current source scan; registration inventory is at lines 9,850–9,898. |
| Production source | Lines 1–9,929 | Includes root module wiring, production declarations, setup and registration. |

Counts are source-header counts, not a `syn` AST census. They exclude fields, methods nested inside `impl`, imports, attributes, and declarations inside the test child. Nonblank count likewise follows trimmed physical source lines.

## 3. Source-order production map

Ranges are inclusive responsibility envelopes based on the current file. They include adjacent types and helpers needed by each workflow and do not imply every line belongs to one semantic operation.

| Lines | Responsibility | Key symbols / dependencies | Authority sensitivity | Candidate module |
| --- | --- | --- | --- | --- |
| 1–385 | Imports, counters, app/connection/chat status and cancellation primitives | `AppStatus`, `ConnectionState`, `ChatState`, `TerminalOwnership`, cancellation helpers; Tauri/runtime primitives | Medium: top-level runtime and cancellation ownership | Keep core wiring in `main.rs` |
| 386–1,188 | Managed state and conversation/session state | `DesktopAppState`, `DesktopConversationState`, `ActiveChat`; persistence, preferences, Codex state | High: shared locks, lifecycle coordination and shutdown | Keep state aggregate and lifecycle owner visible |
| 1,189–1,531 | Repository, commit and workflow state types | `DesktopRepository`, `DesktopCommitCapability`, `RepositoryWorkflowState`, index-effect records | High: active repository and reviewed Commit/index state | Types remain root-owned initially |
| 1,532–2,202 | Provider/model endpoints, presentation and connection publication currentness | `DesktopModelState`, endpoint validation, `PendingConnectedPublication`; provider composition and Codex adapters | High: provider publication and generation currentness | Existing `provider_composition` remains lower-level |
| 2,203–4,576 | Effective Authority snapshot and HostExplicit orchestration | `CurrentHostComposition`, `run_host_tool`, prepare/confirm/cancel handlers, ticket/currentness and output/privacy mappers | Very high: trusted dispatch and reviewed mutation | `host_explicit` is a possible future command-edge module |
| 4,577–5,208 | App status, Trusted Profile/model/commit preferences and endpoint readiness | `app_status`, profile/model preference handlers, `LlamaCppReadinessProbe`; existing profile/preferences/provider modules | Low–medium, with provider configuration and UI-visible errors | Keep handlers grouped until an independently bounded preference extraction is defined |
| 5,209–6,008 | Repository observer, snapshot and staged-review workflow | `desktop_snapshot`, `desktop_repository_snapshot*`, `PreparedRepositoryWorkflow`, `refresh_repository_workflow`; Git and `rah-tools` | High: active-root reads and commit review invalidation | Repository observer/workflow candidate |
| 6,009–6,821 | Admission, activation, repository generation and index-effect reservations | `construct_repository_for_admission`, `admit_repository*`, `ActivationTransaction`, `activate_admitted_member`, Stage/Unstage reservation helpers | Very high: zero/one active member, currentness and active-only composition | Repository lifecycle candidate |
| 6,822–7,163 | Remembered candidate presentation/CRUD/reveal plus explicit remembered admission | `RememberedCandidate*`, catalog mapping, six descriptive handlers, `admit_remembered_workspace_candidate` | Low for catalog CRUD/reveal; admission crosses into repository identity and authority | New `remembered_workspace_commands` for six descriptive adapters only; admission stays in root |
| 7,164–7,687 | Membership presentation, Close, inactive removal, activation/switch and direct repository choice | `close_repository_transition`, selectors, membership commands; `repository_membership` | Very high: Close, membership, fresh active composition and host selection | Repository lifecycle candidate |
| 7,688–8,011 | Repository snapshot command, reviewed Commit authorization and Stage/Unstage commands | `repository_authorize_commit_review`, `repository_index_action`, Stage/Unstage handlers | High: ticket/currentness, Git index effects and Commit capability | Repository workflow candidate |
| 8,012–8,731 | ToolRegistry construction and Codex Connect/Disconnect lifecycle | `desktop_tool_registry`, `desktop_tool_composition_from_registry`, `connect_codex`, `disconnect_codex` | Very high: closed active-only registry and provider/runtime lifecycle | Keep composition and high-level runtime ownership visible |
| 8,732–9,841 | Chat turns, activity/events, runtime cancellation and conversation commands | `run_chat`, `send_chat`, `cancel_chat`, conversation lifecycle, event mappers | High: runtime ownership, event schemas and persistence boundaries | Future session/conversation study only |
| 9,842–9,929 | Tauri Builder, managed state, setup/shutdown and invoke registration | `main`, `Builder`, `.manage`, `generate_handler!`; non-Windows stub | High: composition and registration are deliberate root responsibilities | Retain in `main.rs` |

This yields 13 source-order production responsibility groups. The two large candidate envelopes remain approximately 2.8k repository lines and 2.4k HostExplicit lines. The primary extraction recommendation below is deliberately smaller.

## 4. Repository lifecycle/workflow candidate

### Exact size and included work

For like-for-like comparison with Task 384, the current broad repository candidate is lines **5,209–8,010 inclusive: 2,802 lines**. This envelope begins with selected-Git/repository observer support and ends with Stage/Unstage handlers. It contains:

- Observer and review state: `RepositoryObservationStage`, `RepositorySnapshot`, status/diff/staged-review presentation types, `desktop_snapshot`, `desktop_repository_snapshot_with_review`, `desktop_repository_snapshot`, `TargetObservation` helpers, and staged-review digesting (5,209–5,602).
- Prepared/published workflow refresh and Commit-context invalidation: `PreparedRepositoryWorkflow*`, `prepare_repository_workflow`, `publish_repository_workflow`, `install_repository_workflow`, `refresh_repository_workflow`, and revoke/invalidate helpers (5,603–6,008).
- Repository admission and active publication: selected repository replacement, construction/admission/identity, `publish_active_repository`, generation advancement and activation transaction helpers (6,009–6,421).
- Stage/Unstage reservation, currentness, refresh, activation publication and active-member activation (6,422–6,821).
- Remembered catalog plus admission adapters (6,822–7,163); only catalog CRUD/reveal is selected for the first extraction, not remembered admission.
- Membership, Close, inactive removal, activation/switch, direct choose, and snapshot handlers (7,164–7,694).
- Commit review authorization and Stage/Unstage action handlers (7,695–8,010).

### Tauri commands

The broad envelope contains **16 Tauri commands**. Command names are also their Rust handler names and permission identities; request → response is taken from each handler signature. Frontend callers are in `crates/rah-desktop/frontend/status.js`; membership/authority test references include `repository_membership_test.js` and `status_authority_test.js`.

| Command / handler | Request → response | State / authority dependency |
| --- | --- | --- |
| `remembered_workspace_catalog` | none → `RememberedWorkspaceCatalogPresentation` | descriptive catalog snapshot |
| `reveal_remembered_workspace_location` | `candidateId: String` → `RememberedLocationPresentation` | descriptive location hint only |
| `remember_workspace_candidate` | `RememberedCandidateAddRequest` → catalog / `FrontendError` | descriptive catalog mutation |
| `update_remembered_workspace_candidate` | `RememberedCandidateUpdateRequest` → catalog / error | descriptive catalog mutation |
| `delete_remembered_workspace_candidate` | `candidateId: String` → catalog / error | descriptive catalog mutation |
| `reorder_remembered_workspace_candidates` | `RememberedCandidateReorderRequest` → catalog / error | descriptive catalog mutation |
| `admit_remembered_workspace_candidate` | `candidateId: String` → `RememberedAdmissionResult` / error | fresh Git validation and repository admission; keep root-owned |
| `close_repository` | `CloseRepositoryRequest` → `CloseRepositoryResult` | active lifecycle withdrawal; runtime-disconnect/currentness rules |
| `repository_membership` | none → membership presentation | membership and zero/one active status |
| `remove_repository_member` | member selector → removal result | inactive-only removal and busy-state checks |
| `activate_repository_member` | member selector → activation result | fresh active-only composition and generation |
| `choose_repository` | root path/request → `RepositoryActivationResult` | host admission and activation; host-owned selection |
| `repository_snapshot` | none → `RepositorySnapshot` | active repository observation |
| `repository_authorize_commit_review` | review request → authorization result | Commit capability and currentness |
| `repository_stage_action` | index action request → result | selected repository Git index effect |
| `repository_unstage_action` | index action request → result | selected repository Git index effect |

`admit_remembered_workspace_candidate` is an important seam: it reads descriptive state and then explicitly requests fresh host admission. It must not be swept into a catalog-only extraction.

### Shared state, locks, helpers, and invariants

Within lines 5,209–8,010, source scanning found direct `state.<field>` access to **20 distinct `DesktopAppState` fields** and **112 syntactic `.lock()` call sites**. The field set includes `workspace_membership`, `membership_coordination`, `lifecycle_coordination`, `repository`, `repository_generation`, `repository_workflow`, `repository_index_effect_reservation`, `next_repository_index_effect_token`, `commit_capability`, `connection`, `host_invocation`, `provider_activation`, `active_chat`, `chat`, `conversation`, `remembered_workspace`, `model`, and generation counters. Some functions accept a lock guard or captured value rather than `state`, so these are source references, not a count of distinct runtime lock acquisitions.

Direct lower-level owners/helpers include `repository_membership` and `remembered_workspace`; the range also composes Git discovery/invocation, `rah-tools` repository tools and root lifecycle helpers. Responsibilities include linked-worktree identity, repository generations, staged review/Commit revocation, index-effect exclusion, membership state, runtime disconnection checks, and publication barriers.

Authority-sensitive decisions remain host-owned in `main.rs`: repository root validation/admission, active-member selection and publication, generation/currentness checks, ToolRegistry withdrawal/composition, Commit capability invalidation, and Stage/Unstage execution binding. The module does not own model-selected repository choice. Inactive members must remain without an executable repository `ToolRegistry`; there must be zero or one active member; registries must not be unioned; linked-worktree identity must remain distinct; remembered workspace remains descriptive until a fresh explicit admission.

### Relocation assessment

Pure relocation is **low-to-medium** for isolated presentation/observer helpers, **medium-to-high** for the whole 2,802-line candidate, and **high risk** for activation/Close/index/Commit workflows. The span contains 16 IPC commands and many serialization/error/currentness paths. Its tests provide strong deterministic coverage, but a production move touching activation or registry composition would require targeted Windows live smoke. An attempted single-module move would either expose many root state types/fields or introduce a callback/facade surface that obscures ownership. Extraction should be staged by boundary rather than treated as a contiguous 2.8k-line block.

## 5. HostExplicit orchestration candidate

### Exact size and commands

Current broad envelope: lines **2,203–4,576 inclusive: 2,374 lines**. It includes Effective Authority snapshot composition at the start, HostExplicit activity/output presentation and result classifiers, host dispatch helpers, prepare handlers, confirmation currentness and confirm/cancel endpoints. The HostExplicit command count is **9**; the envelope's tenth command, `get_effective_authority_snapshot`, is a host-composed observation command and should remain separately classified if the domain is later extracted.

| Command / handler | Request → response | Relevant owners |
| --- | --- | --- |
| `host_invoke_read` | `HostReadRequest` → read result / `FrontendError` | active `ToolRegistry`, `host_invocation` coordinator and current composition |
| `host_prepare_repo_create_branch` | branch request → prepared/review result | zero-effect prepare, current repository and review-ticket coordinator |
| `host_prepare_repo_patch` | patch request → prepared result | exact currentness/preimage and ticket state |
| `host_prepare_repo_edit_files` | multi-file request → prepared result | review ticket, currentness, repository-bound mutation |
| `host_prepare_repo_create_file` | create request → prepared result | review ticket and create-file mutation binding |
| `host_prepare_repo_delete_file` | delete request → prepared result | review ticket and delete-file mutation binding |
| `host_prepare_repo_rename_file` | rename request → prepared result | review ticket and rename mutation binding |
| `host_confirm_tool_invocation` | ticket-only confirmation → outcome | consumes/revalidates ticket and dispatches at most once through current registry |
| `host_cancel_tool_invocation` | ticket/cancel request → cancellation result | invalidates prepared action |

The host caller for these frontend workflows is `frontend/status.js`; the three handlers with the recorded permission-inventory discrepancy are called there at lines 1,155, 1,165 and 1,173. Existing Windows deterministic and gated coverage is in `main_tests.rs` under the corresponding private parent imports.

`get_effective_authority_snapshot` (line 2,213) is separately typed as none → `EffectiveAuthoritySnapshot`; it projects host-composed state and should not be silently bundled with mutating HostExplicit flows.

### State and decision ownership

Within lines 2,203–4,576, direct `state.<field>` scanning found **11 distinct `DesktopAppState` fields** and **47 syntactic `.lock()` call sites**: connection, next connection generation, lifecycle coordination, repository, repository generation/workflow, commit identity generation, Trusted Profile and generation, HostInvocation coordinator, and model state. `effective_authority` is the explicit existing helper module called in this envelope; other imported owners include `host_invocation` and `rah-tools` types/functions.

The trusted host composes effective permission and the active registry. The prepare handlers create no mutation authority; they validate current repository/currentness and place bounded review state behind process-local tickets. Confirm consumes and revalidates that ticket before an applicable dispatch through the current active `ToolRegistry`. Provider/model metadata is not authorization. Moving these wrappers must not make request data or a ticket itself appear to own permission.

### The three prepare handlers

`host_prepare_repo_edit_files` (3,823), `host_prepare_repo_create_file` (3,949), and `host_prepare_repo_delete_file` (4,154) remain part of this envelope and retain the Task 384/386 permission discrepancy. A HostExplicit extraction would put these handlers in the same structural patch as the known gap unless an independent permission-inventory investigation closes first. Do not fix or reclassify that finding as part of extraction.

### Relocation assessment

Pure relocation is **medium** for isolated presentation functions and **high** for prepare/confirm dispatch. There are nine HostExplicit IPC handlers (plus one separate Effective Authority observation), with typed frontend schemas, privacy mappers, ticket lifecycle, errors, activity events and currentness dependencies. Existing tests are strong, but all registered handler paths and gate-specific live claims need exact identity comparison. Because three moved handlers have an open permission inventory discrepancy, extracting this group before a separate permission audit risks mixing structural and permission uncertainty. The move would also make the discrepancy less visible if handler bodies move while `invoke_handler`, `build.rs`, and permission inventory stay distributed.

## 6. Candidate and shared-state comparison

Counts use direct `state.<field>` references intersected with the current `DesktopAppState` declaration, plus textual `.lock()` call sites inside the stated envelope. These are comparable source indicators, not runtime lock acquisition counts.

| Candidate | Source size | Shared state fields touched | `.lock()` call sites | Commands in envelope | Main coupling |
| --- | ---: | ---: | ---: | ---: | --- |
| Repository lifecycle/workflows | 2,802 lines | 20 | 112 | 16 | Admission, membership, active publication, Git index, Commit capability, currentness and runtime exclusion |
| HostExplicit orchestration | 2,374 lines | 11 | 47 | 9 HostExplicit + 1 Effective Authority observation | Active registry, effective authority, tickets, currentness, reviewed dispatch and activity |
| Descriptive remembered catalog/reveal adapters | 271 moved lines (6,828–6,907 and 6,915–7,105) | one field: `remembered_workspace` | no direct `.lock()` call sites | 6 | Descriptive-only catalog IO/presentation; no repository admission or runtime authority |

For the six selected catalog/reveal handlers, `DesktopAppState` is only read to reach its descriptive remembered-workspace store. Repository selection, `ToolRegistry`, runtime/session, preferences/provider, review-ticket, and activity/event mutation state are not touched. Command schemas are frontend-visible, but can be checked directly against the unchanged names and signatures. The lower-level `remembered_workspace` module explicitly has no frontend dependency, so it should remain unchanged; add a sibling `remembered_workspace_commands.rs` edge module instead of widening that module's remit.

## 7. Authority ownership comparison

| Question | Repository lifecycle/workflows | HostExplicit | Selected remembered catalog adapters |
| --- | --- | --- | --- |
| Who owns authority? | Host/root composes repository admission and active-only executable state. | Trusted host composes permission, current active registry and dispatch. | Host owns a descriptive catalog; no repository authority is granted. |
| Does `main.rs` merely orchestrate? | No. It validates/adopts roots, changes membership/active identity, currentness and effect bindings. | No. It composes currentness and dispatch around lower-level coordinator machinery. | Mostly yes: translate typed requests/results and errors to/from the descriptive owner. |
| Constructs authority? | Yes, active repository publication and registry lifecycle are adjacent. | Does not grant generic authority, but gates eligibility and binds reviewed operations to existing authority. | No. A path hint is descriptive; admission is deliberately excluded. |
| Validates currentness? | Yes, repository generation, identity, review and index-effect currentness. | Yes, repository/current registry/profile generations and ticket binding. | No repository currentness. Catalog ID and descriptive store validation only. |
| Mints/revalidates review tickets? | Commit review/capability paths; not all lifecycle operations. | Yes, for reviewed prepare/confirm flows. | No. |
| Mutates repository state? | Yes: membership, activation, index/Commit-related state and Git operations. | Confirm may dispatch existing repository mutation tools after review. | No. Mutates descriptive catalog persistence only. |
| Extraction risk to visible ownership | High; risks hiding zero/one active and active-only composition invariants. | High; risks hiding host dispatch/ticket and permission boundaries. | Low; keeping `remembered_workspace` lower-level and composition in root preserves ownership. |

## 8. Existing-module reuse

The ten existing private Desktop modules remain: `codex_baseline`, `conversation_persistence`, `desktop_preferences`, `effective_authority`, `git_discovery`, `host_invocation`, `provider_composition`, `remembered_workspace`, `repository_membership`, and `trusted_profile_selection`.

- `repository_membership.rs` owns process-local inert membership IDs/state. It intentionally does not own root admission, activation, Close/removal transactions, currentness, or active ToolRegistry composition. Putting the 2.8k-line workflow into it would overload that narrow owner.
- `host_invocation.rs` owns coordinator/request/review machinery. It intentionally does not own Tauri wrappers, current Desktop composition or dispatch; moving all HostExplicit into it would reverse the current separation and add Desktop/frontend coupling.
- `remembered_workspace.rs` owns durable descriptive catalog persistence and has no frontend/repository/Git/provider/runtime dependency. Put command DTO/presentation and Tauri adapter wrappers in a separate `remembered_workspace_commands.rs` sibling, which depends down on this existing owner. This adds one focused module, not a duplicate catalog abstraction.
- Do not put remembered admission in the new module: `admit_remembered_candidate` deliberately crosses into selected Git discovery and repository admission.

## 9. Visibility impact and dependency directions

### Large candidates

The currently private parent `DesktopAppState`, many root state types, `FrontendError`, presentation/event enums and helper functions are directly referenced by both large groups. As sibling modules, they cannot access each other's private child items from the parent side. A relocation would require multiple `pub(super)` exports or a newly designed façade/callback boundary; the counts above make broad visibility growth a design warning. Do not make internal types `pub` or `pub(crate)` merely to satisfy the move. Exact symbol-by-symbol visibility must be audited from the chosen narrow extraction's actual identifiers before implementation.

### Selected narrow candidate

Move source lines 6,828–6,907 and 6,915–7,105 (271 total physical lines): catalog request/presentation types, deserialization/parser and error mappers, reveal helper, and exactly these six `#[tauri::command]` handlers. Leave `RememberedAdmissionResult` at lines 6,908–6,914 with the admission command:

- `remembered_workspace_catalog`
- `reveal_remembered_workspace_location`
- `remember_workspace_candidate`
- `update_remembered_workspace_candidate`
- `delete_remembered_workspace_candidate`
- `reorder_remembered_workspace_candidates`

Keep `admit_remembered_candidate` and `admit_remembered_workspace_candidate` in `main.rs` (starting at line 7,106), along with `RememberedAdmissionResult`, because admission crosses to repository authority and registration remains root-owned.

Proposed target: `crates/rah-desktop/src/remembered_workspace_commands.rs`. Expected dependency direction is:

```text
main.rs (state, composition, invoke registration)
  ↓
remembered_workspace_commands.rs (typed Tauri adapters)
  ↓
remembered_workspace.rs (descriptive catalog persistence)
```

Root-to-child references from `generate_handler!` require the six moved handlers to be visible to the parent. The current test child also refers directly to `remembered_catalog_presentation`, `reveal_remembered_location`, `RememberedCandidateUpdateRequest`, and `RememberedLocationHintUpdate`; make those four moved symbols visible to the parent subtree or update only their test import paths. The expected ceiling is ten `pub(super)` changes, never `pub`/`pub(crate)`; reducing this through narrow test import updates is preferred. Child code can continue using private ancestor state/types without widening those fields. Imports move with their use; state fields, command registration and catalog lower-level types remain in `main.rs`/existing module respectively.

This narrow module owns frontend DTO/presentation adapters, not catalog persistence rules or host admission. Avoid a circular dependency: the module calls into `super::DesktopAppState` and `super::remembered_workspace`; the parent only references the six handlers in registration.

## 10. Frontend, IPC and permission surface

The selected six command names have visible callers in `frontend/status.js` and are asserted in `frontend/remembered_workspace_test.js`. Their request/response contracts are closed serde structs and typed frontend results. Preserve exact names, serde field names, `FrontendError` variants, unavailable/error presentation, command registration order and timing. The frontend caller remains unchanged.

All six selected commands are represented in current `build.rs` generation and permission/capability inventory. The known 47 registered vs 44 generated/default permission inventory discrepancy belongs to the three HostExplicit prepare handlers:

```text
host_prepare_repo_edit_files
host_prepare_repo_create_file
host_prepare_repo_delete_file
```

The selected extraction does not contain these handlers, does not alter permissions, and does not resolve or redefine that finding.

## 11. Pure relocation feasibility

| Candidate | Pure relocation potential | Authority risk | IPC risk | Shared-state coupling | Test impact | Windows-live recertification need | Review complexity |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Repository lifecycle/workflows | Medium for a staged observer-only slice; low for the entire range | High | High: 16 commands in envelope | High: 20 fields / 112 lock call sites | Strong deterministic membership/currentness/index/Commit tests; broad coupling | Targeted Windows admission/activation/Close/index smoke if those paths move; full live suite not automatic | High |
| HostExplicit | Medium for adapters, low-to-medium across complete 2,374 lines | High | High: 9 commands plus one adjacent authority snapshot; 3 handlers have open inventory gap | Medium-high: 11 fields / 47 lock sites | Strong tests for ticket/currentness/output privacy and handler behavior | Targeted Windows reviewed-flow smoke if dispatch/effects/composition are relocated | High |
| Remembered descriptive catalog/reveal adapters | High: two adjacent source groups totaling 271 lines, with admission excluded | Low | Medium: six stable command names/schemas | Low: one descriptive field, zero lock sites in span | Existing Windows deterministic catalog tests and frontend contract assertions | No live smoke for exact body relocation; compile and deterministic suite on Windows still required | Low-medium |

The third candidate wins on mechanical movement, explicit separation from authority and simple body/schema equivalence. It reduces `main.rs` less than either large candidate, intentionally.

## 12. Production-size reduction estimates

| Extraction | Current source | Approximate reduction | Estimated `main.rs` after adding one module declaration | Notes |
| --- | --- | ---: | ---: | --- |
| Repository lifecycle/workflows | 5,209–8,010 inclusive | ~2,802 lines | ~7,130 lines | Assumes one module declaration added; practical size depends on registration/import lines retained. |
| HostExplicit broad envelope | 2,203–4,576 inclusive | ~2,374 lines | ~7,558 lines | Keep Effective Authority observation separately if boundary review selects only nine HostExplicit commands. |
| Selected catalog/reveal adapters | 6,828–6,907 and 6,915–7,105 | ~271 lines | ~9,661 lines | Keep `RememberedAdmissionResult` and remembered admission in root; add one module declaration. |

These are source-range estimates, not promised final counts. Line reduction is secondary to preserving clear ownership.

## 13. Validation and Windows live-validation cost

### Task 389 selected extraction gate

For a pure Desktop Rust relocation with no behavior/schema/dependency changes, freeze:

```powershell
cargo fmt --check
cargo test -p rah-desktop -- --test-threads=1
cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings
git diff --check
```

The test run must be on Windows because the command group and main test child are Windows-gated. Do not run the workspace suite by default. Escalate to `cargo test --workspace -- --test-threads=1` only if the implementation changes cross-crate behavior or review exposes behavior uncertainty.

Prove before/after body equivalence for the moved source, compare command names/signatures/serde types and registration order, and inspect `git diff --check` and exact file scope. No executable semantics may change.

### Comparison gates for the two larger candidates

- A repository extraction limited to pure observer presentation could use the same Desktop gate. If admission, activation, ToolRegistry composition, Close/removal, index effects, or Commit binding moves, add a focused deterministic test selection plus targeted Windows host-driven smoke for the moved lifecycle boundary. Full Windows certification is not automatic.
- A HostExplicit extraction uses the Desktop gate plus command-name/schema and ticket/currentness/effect equivalence checks. Add targeted Windows host-driven reviewed-flow smoke if confirm dispatch, current registry composition, or repository mutation paths are relocated. No model/inference claim follows from such a smoke.
- No candidate justifies workspace-wide tests unless a cross-crate dependency or behavior change is actually made.

## 14. Permission-finding sequencing

Investigate the known three-command permission-generation discrepancy **after the selected first production extraction** and before any HostExplicit production move. The selected six handlers are all represented in permission generation and do not include the finding. A separate permission-only audit will therefore not block or contaminate the low-risk extraction; it should become an explicit prerequisite for later HostExplicit decomposition. Do not fix the discrepancy in Task 388 or Task 389.

## 15. Comparison and selection order

### Option A — repository lifecycle first

Rejected as the first move. Although it offers the largest reduction (~2,802 lines), its range combines admission, membership, active publication, linked-worktree identity, Commit invalidation, Stage/Unstage reservations, and 16 Tauri commands. The 20 state-field and 112 lock-site coupling plus active-only registry invariants make mechanical relocation and independent equivalence review substantially harder.

### Option B — HostExplicit first

Rejected as the first move. Its 2,374 lines and nine HostExplicit handlers form a recognizable domain, and `host_invocation.rs` is a lower-level reuse point. However, root remains responsible for composition, dispatch/currentness and IPC registration; the move touches ticket/privacy/currentness paths and all three handlers in the open permission-inventory finding. Audit that discrepancy before preparing a later HostExplicit extraction.

### Option C — smaller descriptive catalog adapter first

**Selected.** Move only the six remembered catalog CRUD/reveal adapters and their DTO/presentation/error mapping, 271 lines across two adjacent ranges with the admission result type left in the gap. This is a descriptive-only slice with one shared state field, no `.lock()` sites in the moved code, no active repository state, and an existing lower-level catalog owner. A new sibling command adapter is preferable to expanding `remembered_workspace.rs`, whose no-frontend boundary is intentional.

## 16. Exact next task contract

### Task 389 — Mechanical Extraction of Remembered Workspace Catalog Commands

- **Starting checkpoint:** `1f770c62e0e2c6a8f98f70591cd5f1ce6a3c34b5`.
- **Exact source groups:** `crates/rah-desktop/src/main.rs` lines 6,828–6,907 and 6,915–7,105 (271 total lines), limited to the six descriptive catalog/reveal command adapters and their request/presentation/parser/error-mapping helpers. Leave lines 6,908–6,914 (`RememberedAdmissionResult`) with admission in the root.
- **Target:** new private sibling module `crates/rah-desktop/src/remembered_workspace_commands.rs`; declare it from `main.rs` and point the existing `generate_handler!` entries at the moved handlers without changing identities/order.
- **Remain in `main.rs`:** `admit_remembered_candidate`, `admit_remembered_workspace_candidate`, `RememberedAdmissionResult`, repository admission, all state construction, and Tauri registration/composition.
- **Allowed transformations:** move imports with their references; formatting needed for standalone module; the minimum `pub(super)` visibility on moved handlers/helpers/types required by parent registration and the sibling Windows test module. Update only test import paths needed to refer to moved private items; leave test bodies, fixtures, assertions and gates unchanged. Do not widen state-field or domain-type visibility absent a demonstrated compiler requirement.
- **Forbidden transformations:** command rename or schema/error/event change; permission/capability/build changes; authority/currentness/repository admission changes; lower-level `remembered_workspace.rs` edits; Cargo/dependency, frontend, tests, or runtime behavior changes; unrelated cleanup.
- **Required proof:** exact moved-body comparison modulo wrapper/import relocation and rustfmt; six command names/signatures/serde contracts and registration order unchanged; only `main.rs`, `main_tests.rs` import paths, and the new module file change.
- **Validation:** Windows `cargo fmt --check`, `cargo test -p rah-desktop -- --test-threads=1`, `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings`, and `git diff --check`. No workspace gate or live smoke unless implementation review finds behavior or composition changed.
- **Commit discipline:** leave the implementation uncommitted for an independent Task 390 structural/IPC/authority audit; checkpoint only after that audit. Do not push.

## 17. Short staged roadmap

- **Task 389** — mechanical remembered catalog command extraction.
- **Task 390** — independent structural, command identity and authority audit.
- **Task 391** — checkpoint the audited extraction.
- **Task 392** — research the next production domain, with the HostExplicit permission discrepancy audited before any HostExplicit move.

Keep `main.rs` responsible for app/bootstrap, managed state, module composition, command registration and visible top-level authority/lifecycle wiring. The purpose of decomposition is reviewable ownership, not minimum line count.

## 18. Explicit non-goals

Task 388 does not move Rust code, create Rust modules, rename symbols, change visibility, alter Tauri commands/registration, permissions/capabilities, authority, Cargo/dependencies, tests, frontend behavior or runtime behavior. It does not modify `*.rs`, `Cargo.toml`, `Cargo.lock`, `build.rs`, permissions, capabilities, frontend, changelog or release gates. It does not fix the permission discrepancy, version-bump, tag, push, or start Task 389 automatically.
