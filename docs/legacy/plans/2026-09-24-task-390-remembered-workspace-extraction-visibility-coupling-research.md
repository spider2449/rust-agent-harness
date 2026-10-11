# Task 390 — Remembered Workspace Extraction Visibility and Coupling Research

**Verdict: REJECT Remembered Workspace as the first production extraction in the planned self-contained form.** The six adapters remain behaviorally descriptive and authority-free, but the planned DTO/presentation group is coupled to private field access in the crate-root test child. A sibling module cannot preserve those test accesses with function-only `pub(super)` changes. Keeping the DTO group in the root makes a very narrow handler-only move mechanically possible, but leaves command policy, presentation mapping, and parsing split across the module boundary. The coherent options are therefore either a test relocation with meaningful fixture coupling or a visibly incomplete adapter extraction. Select another production domain for the next implementation task.

This is research only. No Rust code, visibility, tests, permissions, authority, IPC, Cargo files, or frontend behavior changed.

## 1. Baseline and evidence

- Starting HEAD: `e35214a2678af22cde3f1888511abb1947626fa0`.
- Initial worktree: clean.
- Task 389 supplied result: `FAIL — EXTRACTION NOT SEMANTICALLY MECHANICAL`; its attempted extraction was reverted completely.
- Current tree contains no `remembered_workspace_commands.rs` and no Task 389 research artifact. This report therefore uses the Task 389 final summary supplied in the task, the Task 388 plan, and current source. It cannot attribute individual compiler diagnostics or count exact edits in the reverted attempt where no log/diff was retained.
- Current source locations: `crates/rah-desktop/src/main.rs` lines 6,828–7,103, with `RememberedAdmissionResult` at 6,908–6,912; Tauri registration at 9,850–9,900. Lines shifted slightly from Task 388's estimate.

Task 389's supplied failure summary has two distinct classes: (A) child tests access private fields of selected request/presentation types; (B) moved handlers and their command-associated types must be reachable from root registration. The source audit below identifies the symbols and access paths. It does not reconstruct the unavailable failed compiler output.

## 2. Selected symbol inventory

The selected self-contained command adapter unit contains 19 named symbols (including the six handlers):

| Symbol | Kind | Current visibility / owner | Root or test access |
| --- | --- | --- | --- |
| `RememberedCandidatePresentation` | response item struct | private, crate root | `main_tests.rs` reads candidate `candidate_id` and `label` fields through catalog presentation |
| `RememberedWorkspaceCatalogPresentation` | response struct | private, crate root | tests inspect `candidates` and catalog response fields |
| `RememberedLocationPresentation` | response struct | private, crate root | reveal behavior test reads `candidate_id` |
| `RememberedCandidateAddRequest` | request struct | private, crate root | serde command input; no direct test construction found |
| `RememberedCandidateUpdateRequest` | request struct | private, crate root | request deserialization test constructs by serde and reads `location_hint` |
| `RememberedLocationHintUpdate` | request-state enum | private, crate root | same test checks `Missing` and `Clear` variants |
| `RememberedCandidateReorderRequest` | request struct | private, crate root | serde command input; no direct test construction found |
| `deserialize_remembered_location_hint_update` | serde helper | private, crate root | referenced by the request's `deserialize_with`; serde-generated code remains local to the type/module |
| `remembered_catalog_presentation` | conversion helper | private, crate root | called from root registration path indirectly through commands; directly called by tests and live harnesses |
| `remembered_mutation_error` | error conversion helper | private, crate root | used by catalog mutation handlers only |
| `parse_remembered_candidate_id` | parser helper | private, crate root | used by catalog and admission handlers; admission remains in root |
| `parse_remembered_location_hint` | parser helper | private, crate root | used by add/update handlers |
| `reveal_remembered_location` | presentation/helper function | private, crate root | directly called by behavior tests and live harnesses |
| `remembered_workspace_catalog` | Tauri handler | private, crate root | registered in root `generate_handler!` |
| `reveal_remembered_workspace_location` | Tauri handler | private, crate root | registered in root `generate_handler!` |
| `remember_workspace_candidate` | Tauri handler | private, crate root | registered in root `generate_handler!` |
| `update_remembered_workspace_candidate` | Tauri handler | private, crate root | registered in root `generate_handler!` |
| `delete_remembered_workspace_candidate` | Tauri handler | private, crate root | registered in root `generate_handler!` |
| `reorder_remembered_workspace_candidates` | Tauri handler | private, crate root | registered in root `generate_handler!` |

`RememberedAdmissionResult` is deliberately excluded from the 19: it is the response for `admit_remembered_workspace_candidate`, remains in `main.rs`, and crosses into repository admission. `parse_remembered_candidate_id` is shared with that root-owned admission command, which is a real dependency to account for if the parser moves.

The planned group covers about 269 physical source lines in the current file: lines 6,825–6,903 and 6,914–7,103, including cfg attributes and blank lines. Task 388 estimated 271 lines at its earlier offsets.

## 3. Visibility blockers and Rust privacy

The failed full-group relocation has two access directions:

| Access | Symbol(s) | Why relocation breaks | Narrowest plausible visibility if kept in the child module |
| --- | --- | --- | --- |
| Root registration to child handler | six Tauri handlers | `main`'s `generate_handler!` path originates in the parent module; a private child item is inaccessible to its parent | `pub(super)` on each handler; visible to root and root descendants only |
| Root-owned admission to moved parser | `parse_remembered_candidate_id` | `admit_remembered_workspace_candidate` remains in root and calls the parser | `pub(super)` on the parser, or retain a root-owned parser wrapper/helper |
| Root test child to moved conversion/reveal helpers | `remembered_catalog_presentation`, `reveal_remembered_location` | `crate::tests` currently sees them as descendants of their defining root; after move tests are siblings | `pub(super)` on each helper, or relocate the associated tests into the child module |
| Root test child to moved private types | presentation/request structs and enum | sibling tests cannot name child-private items | `pub(super)` on each directly referenced type, or relocate tests |
| Root test child to moved private fields | presentation fields; update request `location_hint` | field privacy is checked at the field's defining module; making only the containing type visible does not grant sibling access | `pub(super)` on directly read fields, or relocate tests / rewrite test accesses (rewrites were out of scope and forbidden) |
| Serde helper | deserializer and request field annotation | generated serde implementation belongs to the type's defining module | remains private; no external visibility needed for serde itself |
| Tauri wrapper/signature types | request/response types | macro expansion references command wrappers at the registration site; request/response types are used by handler signatures and wrapper generated for the command | keep private to child unless the actual expansion/compiler requires parent naming; no evidence that serde requires wider visibility |

`main_tests.rs` is declared as the root's test child (`crate::tests`/`super` imports). Rust privacy grants a child access to private items of its ancestors. It does not grant a sibling module access to another sibling's private items or fields. After moving production items to `crate::remembered_workspace_commands`, `crate::tests` is a sibling of that module. A `pub(super)` item in the new module is visible to the root and, through the root, its descendants, so it can serve narrow handler/helper/type/field exposure where root-level reach is sufficient. `pub(crate)` would expose those implementation details to every module in the crate and is broader than the audited registration/test paths require.

The source scan found the `RememberedCandidateUpdateRequest.location_hint` field and `RememberedLocationHintUpdate::{Missing, Clear}` read/matched directly in `task_333_update_request_distinguishes_missing_and_explicit_clear_hint`. Presentation output fields are read directly by catalog/reveal behavior and live harness tests. Fields that are not directly read by tests need not be widened solely because their containing type moves.

Task 389's supplied summary calls out two blocker classes, not a symbol-by-symbol failed-build log. Source analysis shows the minimum viable full-group widening as at least six `pub(super)` handler declarations, two test-called helpers, the moved symbols/types named from tests, and `pub(super)` on directly test-read fields; `parse_remembered_candidate_id` also needs parent access if admission keeps calling it. This is materially more than a handful of handler changes. The exact count depends on whether test code imports module paths or retains root-visible aliases, and on the command macro expansion. Do not freeze a count by guessing at generated diagnostics.

For a full self-contained move with unchanged test bodies/import semantics and warnings treated as errors, the source-level minimum is **16 `pub(super)` items plus 7 `pub(super)` fields**: six handlers; six request/response structs (`RememberedCandidatePresentation`, `RememberedWorkspaceCatalogPresentation`, `RememberedLocationPresentation`, and the Add/Update/Reorder request types); `RememberedLocationHintUpdate`; `parse_remembered_candidate_id` (root admission caller); `remembered_catalog_presentation`; and `reveal_remembered_location`. The seven fields are catalog `status`/`candidates`, candidate `candidate_id`/`label`, location `candidate_id`/`location`, and update request `location_hint`. This count assumes handler signatures remain `pub(super)` and their associated DTO types are raised to the same effective visibility to avoid a private-interface mismatch. `pub(super)` reaches only the parent module subtree; none of these changes requires `pub(crate)`. If all seven test functions move into the new module, the two test-only presentation/reveal helpers, the directly named update request/enum, and the seven test-read fields can remain private; the six handlers, signature DTO types, and root-shared candidate-ID parser remain parent-visible (12 items total).

## 4. Exact test coupling

Direct selected-item references occur in seven unique test functions in `main_tests.rs`:

| Test | Access | Classification |
| --- | --- | --- |
| `task_333_catalog_actions_are_durable_ordered_and_privacy_safe` | calls private presentation converter; validates observable serialized catalog behavior | deterministic behavior test |
| `task_334_reveal_is_explicit_exact_and_authority_free` | calls private reveal helper; reads returned `candidate_id` | deterministic behavior/privacy test |
| `task_334_reveal_rejects_unavailable_catalog_without_path_access` | calls private reveal helper | deterministic behavior test |
| `task_333_update_request_distinguishes_missing_and_explicit_clear_hint` | names request type; reads private `location_hint`; matches enum variants | DTO/parser structural behavior test |
| `task_335_windows_host_driven_remembered_workspace_live_certification` | calls presentation/reveal helpers and reads private catalog presentation fields | live Windows certification harness |
| `task_352_windows_active_repository_close_live_certification` | calls catalog presentation helper in live harness assertions | live Windows certification harness |
| `task361_run_live_scenarios` | calls catalog presentation helper in live scenario evidence | live scenario/certification harness |

Count: **7 functions directly coupled to selected private helpers/types; 3 functions read private fields or variants/types directly**. The seven are not seven ordinary unit tests: three are live certification/scenario functions. No selected command handler is directly invoked from tests; current tests exercise conversion/helper behavior and the actual host-driven surface separately.

The test child imports selected helpers in its root `use super::{...}` list and uses `super::...` at call sites. Updating import paths alone can identify a new module, but cannot grant access to its private fields. The tests also rely on root-level fixtures and state such as `DesktopAppState`, `TestRepository`, `PathBuf`, and shared certification setup. They are not presently organized as an independent remembered-workspace test file.

## 5. Tauri registration and IPC visibility

Root registration currently uses `tauri::generate_handler![remembered_workspace_catalog, ...]` with bare identifiers in a fixed order. Tauri 2.11.5's `generate_handler!` parser accepts Rust `Path` values and derives the generated wrapper path from each command path. Therefore registration can refer naturally to `remembered_workspace_commands::remembered_workspace_catalog` and peers. This is an ordinary Rust module visibility requirement: a private child handler is not nameable by its parent, while `pub(super)` is enough for root. The macro does not establish a reason for `pub` or `pub(crate)`.

No evidence in the current code or Tauri macro parser indicates that serde requires request/response types to become public. Serde derive/helper code is generated at the type definition site. Tauri command wrappers are generated alongside each command function and called through the supplied command path. Keep DTOs private unless a compiler check of a concrete implementation proves otherwise; Task 390 does not run such a check.

Keep command function names, wrapper identities, serde casing/default/deny-unknown-fields semantics, registration order, and response shapes unchanged. Module paths in Rust registration are not frontend IPC command names; the Tauri command's final identifier remains the registered command identity.

## 6. Candidate A — planned group with minimum visibility widening

Candidate A is the original self-contained movement of handlers plus their DTO/presentation/parser/error helpers (19 symbols, about 269 lines), with only minimum visibility changes.

- Root registration requires six `pub(super)` handler items.
- Root-owned admission needs `parse_remembered_candidate_id` available if that helper moves.
- Existing root-child tests need parent-visible helper/type access, and direct field reads need field visibility too. `pub(super)` is the narrowest plausible modifier; it reaches the root and the root's descendants, not unrelated crate siblings.
- These modifiers do not change Rust `pub` API, frontend IPC, permission, or authority ownership. They do widen private implementation visibility to the root subtree. Moving request/presentation data members to `pub(super)` solely for tests is a visibility delta larger than the module's command registration needs.
- Candidate A therefore does not satisfy the task's selection rule for a small, understandable visibility delta. The test coupling is the limiting part, not handler registration.

## 7. Candidate B — handlers and coherent DTO/presentation group together

This is the cleanest production ownership boundary: the six catalog/reveal handlers, request/response/presentation types, and local parse/serde/error/conversion helpers move together; `RememberedAdmissionResult`, remembered admission, repository admission/identity, and membership remain in root. It avoids a handler module whose logic relies on parent-owned parser and presentation routines.

- Estimated production movement: 19 symbols, about 269 physical lines.
- Expected visibility changes without test relocation: six handler declarations plus the parent-shared parser, test-called helpers/types and directly accessed fields. `pub(super)` is sufficient in principle; `pub(crate)` is unnecessary based on observed call graph. The exact number of field declarations is a mechanical implementation inventory item, not a reason to broaden whole structs.
- Tests affected: seven tests need new import/call paths; three contain direct private field/type/variant access and therefore need `pub(super)` field/type visibility or relocation. Test bodies cannot remain unchanged with the fields private.
- Review complexity: medium-high because pure production relocation would be mixed with many visibility edits whose only consumer is the existing test child.

Candidate B is not selected despite its cleaner production file boundary because the test visibility delta contradicts the original narrow-boundary premise.

## 8. Candidate C — relocate domain-local tests under the new module

Move the six tests with direct private item/field access and, for coherence, the seventh helper-calling test under `#[cfg(test)] mod tests;` within the new production module. This preserves private access without exposing DTO fields. The production module may use a child test module, which can access its ancestor's private items.

- Tests to move: **7 direct-reference tests** (including three live/certification harness functions); at minimum 3 need relocation to retain direct private field/type access without widening. Moving only six leaves the seventh helper caller requiring a root-visible helper or a path/import adaptation that still cannot reach a private sibling item.
- These tests are conceptually related to remembered workspace presentation/reveal, but they are not all domain-local. The first four use common `main_tests.rs` fixture infrastructure and shared application state; three are broad Windows live/certification scenarios tied to larger admission, repository-close, and scenario setup.
- Estimated moved test code: high hundreds of lines, exact count not used because test functions share surrounding helpers and fixtures. Extracting the tests would require either importing root test helpers into a child test module or duplicating/relocating common fixture infrastructure, which would blur the small production boundary.
- Classification: three deterministic behavior/DTO tests, one reveal behavior test, and three live/certification functions. This is not a clean small domain-test file.

Candidate C preserves private production visibility but transfers broad test scaffolding and certification ownership. It is not justified for a first extraction.

## 9. Candidate D — root re-export or wrapper

A root alias such as `use remembered_workspace_commands::handler;` can give `generate_handler!` a root name, but the underlying child item still must be visible to the parent. A `pub(super) use` re-export has the same narrow visibility effect as exposing the handler directly. A wrapper in `main.rs` adds a second Tauri command endpoint and delegates to the module, creating extra registration/wrapper indirection without reducing the need to reconcile private request/response types or test field access.

Candidate D adds indirection, not a smaller ownership boundary. Do not use it.

## 10. Candidate E — choose another production domain

Task 388's source map identifies the app-status/profile/model/commit preference and endpoint-readiness span (then lines 4,577–5,208) as low-to-medium sensitivity, with existing `desktop_preferences`, `trusted_profile_selection`, and `provider_composition` lower-level modules. It is a better next *research target* than HostExplicit or repository lifecycle: no repository admission/membership authority is inherent in its domain, and it is separate from the open three-command permission finding. However, Task 388 explicitly left those handlers grouped pending an independently bounded preference extraction; this Task 390 audit did not inspect every test/DTO/shared-state edge there. Recommend a small source-and-test coupling audit to freeze one preference subunit before moving code.

Do not fall back to HostExplicit orchestration: it is authority-sensitive, includes ticket/currentness behavior, and intersects the unrelated known permission-generation discrepancy. The three commands `host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and `host_prepare_repo_delete_file` remain untouched and unresolved by this task.

## 11. Authority boundary

Every candidate considered here leaves `RememberedAdmissionResult`, `admit_remembered_candidate`, and `admit_remembered_workspace_candidate` in `main.rs`. Repository admission and identity validation, active repository membership/activation, ToolRegistry composition, and executable repository authority remain owned by the current root admission/composition flow. The remembered catalog remains descriptive state only; possession of a remembered candidate or location hint does not admit, activate, select, or authorize a repository.

No candidate changes authority, permissions, IPC behavior, or security ownership. The three HostExplicit permission inventory findings are explicitly separate.

## 12. Quantitative comparison

Line counts are estimates from current source ranges; test movement is the main uncertainty. A handler-only movement is shown for Candidate A's narrowest sub-option, but is not a preferred coherent boundary.

| Candidate | Lines moved | Visibility changes | Tests moved | Authority risk | Review complexity |
| --- | ---: | ---: | ---: | --- | --- |
| A. Planned group, narrow modifiers | ~269 | at least 6 handler `pub(super)` plus moved helper/type/field items needed by root admission and sibling tests; `pub(crate)` not indicated | 0, but 7 test bodies need paths and 3 require widened field/type reach | low | high |
| A0. Six handlers only (types/helpers stay root) | ~117 | 6 handler `pub(super)`; no DTO field widening | 0 | low | medium: adapters depend on root-owned parsers/mappers and split domain logic |
| B. Handlers + DTO/presentation group | ~269 | same as A; no visibility benefit over A | 0 | low | medium-high |
| C. Production + associated tests | ~269 production + high-hundreds test code | 6 handlers `pub(super)`; no DTO field widening; test module parent path changes | 7 | low | high due root fixtures and mixed live certification |
| D. Root wrapper/re-export | ~269 plus wrapper/alias lines | handler still parent-visible; types/tests still need same access | 0 | low | high for indirection without decoupling |
| E. New bounded preference subunit | TBD after research | TBD; likely narrow, but not audited here | TBD | low to medium | medium until exact subunit is frozen |

No artificial numeric scores are assigned. Candidate A's minimal route may be implemented as A0, but A0 is not the self-contained boundary originally selected and should not be represented as the same result.

## 13. Decision

Reject the original Remembered Workspace self-contained first extraction for now. The command boundary is authority-safe, but it is not narrow in the test tree: the tests rely on private presentation/request details from the crate root. Moving the full unit either widens those fields/types to the root subtree or relocates broad shared test infrastructure and certification harnesses. Handler-only movement is possible with six `pub(super)` handlers, but it leaves parsing/presentation/error conversion in the parent and is not a coherent ownership improvement.

Selected strategy: abandon Remembered Workspace as the first extraction and select the lower-sensitivity Desktop Preferences command group from the Task 388 source map for Task 391. The mapped 4,577–5,208 span is only a candidate envelope, not an instruction to move all 632 lines. Task 391 must freeze a smaller exact symbol/test/visibility/IPC/authority boundary before editing and stop if no coherent subunit exists. Do not start that implementation automatically from this report.

## 14. Next-task contract

**Recommended next task: Task 391 — Desktop Preferences Command Boundary Research.** This is the small prerequisite needed to name a genuinely low-coupling production unit; it should inspect the exact current preference/endpoint command ranges and tests, leave HostExplicit and all permission findings untouched, and produce a frozen extraction contract. If that research finds no narrow unit, select a different candidate rather than forcing extraction.

If task numbering instead requires Task 391 to be implementation-only, do not start it from this research: first authorize/resequence the bounded preference-boundary research, then define `Task 392 — Mechanical Extraction of <frozen preference subunit>`. No independent audit should be scheduled until a real extraction exists.

A future mechanical task must freeze the exact handler/types/helpers, allowed `pub(super)` edits, Tauri registration path/order, unchanged IPC schema, exact test/import treatment, body-equivalence proof, and focused Windows validation before code movement.

## 15. Validation and repository closeout

Docs-only checks requested and run after writing this artifact:

- `git status --short`
- `git diff --check`
- `git diff --stat`

No `cargo fmt`, `cargo check`, `cargo test`, or `cargo clippy` command is authorized or run. No push is performed.
