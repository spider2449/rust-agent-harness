# Task 384 — rah-desktop/main.rs Structural Decomposition Research

## Verdict

**The primary size cause is test/certification-harness placement (yes); production has meaningful cross-domain coupling but the line count does not prove that its architecture is monolithic.** The safest first extraction is the existing Windows-only tests child into src/main_tests.rs, unchanged. This artifact documents source structure at Task 383 and does not authorize or perform code movement.

## 1. Authoritative checkpoint

- HEAD: 9e2e4c2f05ae877ba5739f364fdd3db3824b4736 — fix: harden repository listing boundary validation.
- Prior commits: ca96d1e feat: add bounded repository structure listing; 093d156 docs: define repository structure listing implementation contract.
- Initial git status --short was empty.
- Task 383 / Task 382 remains the corrected, independently audited, Windows-live-recertified repo.list checkpoint and A/B baseline for later extraction. A future extraction is a descendant; Task 383 does not certify it.

## 2. Exact measurement and method

Commands run from repository root:

    git status --short
    git rev-parse HEAD
    git log -3 --oneline
    $s=[IO.File]::ReadAllText('crates/rah-desktop/src/main.rs')
    $a=$s.Split("`n")
    $a.Length
    @($a | Where-Object { $_.Trim().Length -gt 0 }).Length
    rg -n '^#\[cfg\(all\(test, target_os = "windows"\)\)\]|^mod tests|^#\[tauri::command\]|^#\[cfg\(test\)\]' crates/rah-desktop/src/main.rs
    rg -n '^    #\[(tokio::)?test|^    #\[ignore' crates/rah-desktop/src/main.rs

Direct byte measurement found 35,532 LF bytes (physical lines), 34,350 nonblank lines, no CR bytes, and a final LF. PowerShell Split returns 35,533 elements because the final LF creates a trailing empty element. This reconciles the reported editor count. A regex comment count was not used because Rust raw strings/fixtures make a simple count misleading.

The Windows-only test child is cfg at line 9,929, begins mod tests at 9,930 and closes at EOF (35,532). Lines 1–9,928 are the root prefix. Classification charges all child lines to test-only and also charges the nine scattered cfg(test) items: five test-hook structs at 443–478 and four helpers at 5,797–5,817, 6,008–6,047, 8,803–8,812, 8,976–8,991. No line is counted twice.

| Classification | Physical lines | File share | Rule |
| --- | ---: | ---: | --- |
| Production-only | about 9,805 | 27.6% | Prefix through 9,928 less scattered test-only spans |
| Test-only | about 25,727 | 72.4% | Child 9,929–35,532 (25,604 lines) plus about 123 scattered test-only lines |
| Total | 35,532 | 100% | Physical lines |

Anchored header scan of lines 1–9,928 counted 347 root declaration headers: 209 fn, 114 struct/enum/type/static/const, 14 impl, 10 mod. Nine headers are cfg(test), leaving 338 production-only headers by this convention. At test-module indentation, lines 9,931 onward contain 356 headers: 315 functions (231 attributed tests, 84 helpers), 24 type/constant declarations, 17 impls. Thus 365 test-only headers including scattered items. This is a regex declaration-header count, not a syn AST count; use declarations, fields, methods, imports and attributes are excluded.

There are 231 test attributes: 214 deterministic entries and 17 explicitly ignored host/live/certification entries.

## 3. Test-only and live harness allocation

| Lines | Responsibility | Evidence/class |
| --- | --- | --- |
| 443–478 | Startup activation, authorization, connect publication, index effect, workflow refresh test hooks | Test-only structs adjacent to state and barriers |
| 5,797–5,817 | Test workflow installer | cfg(test) helper |
| 6,008–6,047 | Selected-repository test replacement | cfg(test) helper |
| 8,803–8,812 | Activity event test helper | cfg(test) helper |
| 8,976–8,991 | Empty composition test helper | cfg(test) helper |
| 9,929–10,325 | Test module cfg, imports, Connect/model fixtures | Test-only; broad super import evidences private coupling |
| 10,326–12,161 | HostExplicit create/delete tests | Includes ignored Windows live tests at 10,876 and 11,505 |
| 12,162–13,400 | Repository snapshot/review, Stage/Unstage, currentness, chat/cancel tests | Deterministic private-state tests |
| 13,401–17,196 | Host runtime probe; HostExplicit rename, branch, patch, multi-file and branch-create certification | Ignored live entries and Git/event/identity helpers |
| 17,197–18,570 | Connection currentness, membership, linked-worktree deterministic tests | Deterministic including barriers/race cases |
| 18,571–18,982 | Inactive-member removal certification | task_343; RAH_RUN_V028_INACTIVE_MEMBER_REMOVAL_LIVE=1 |
| 18,983–28,537 | Activation barriers, Close, Commit authorization, membership and repository Tool tests | Primarily deterministic; shared private fixtures |
| 28,538–30,199 | Task 324 process ownership and two-repository live certification | PowerShell process census and Codex ownership |
| 30,200–30,638 | Task 335 remembered-workspace live certification | RAH_RUN_V027_REMEMBERED_WORKSPACE_LIVE=1 |
| 30,639–32,727 | Task 352 active-repository Close live evidence | Repository captures and toolchain/report helpers |
| 32,728–34,322 | Task 361 shared linked-worktree live fixture/scenarios/layout cases | Task361LiveFixture, capture, cleanup, junction/reparse cases |
| 34,323–34,824 | Task 369 repo.search live certification | RAH_RUN_V031_REPOSITORY_SEARCH_LIVE=1; main/A/B, sparse, reparse/privacy |
| 34,825–35,496 | Task 379 repo.list live certification | RAH_RUN_TASK379_REPOSITORY_LIST_LIVE=1; Task 383 lineage and boundary cases |
| 35,497–35,532 | Task 361 linked-worktree live entry | RAH_RUN_V030_LINKED_WORKTREE_LIVE=1 |

The shared live/certification harness envelope is about 12.5k lines (about 35% of the file), counting live test and supporting fixture/helper ranges once. It is a bounded range estimate, not a compiler function-size measure. Remaining test-only source is deterministic tests and shared fixture/helper code.

The harness invokes native git; Codex CLI/codex.exe on applicable gates; Windows powershell, cmd.exe (junction/symlink attempts), attrib; and rustc/cargo for one toolchain probe. It requires Windows and explicit environment gates; older Codex gates can require pinned 0.149.0/configuration or authentication. Git fixtures create isolated repositories/worktrees and own process cleanup. main.rs does not reference rah-mcp-echo-server, CARGO_BIN_EXE fixtures or a Cargo-built fixture executable. Task 381-B's rah-mcp-echo-server.exe prerequisite belongs to separate profile-composition tests.

## 4. Production source-order map

Ranges are inclusive responsibility envelopes, not claims that every line is semantically identical.

| Lines | Class | Responsibility | Key symbols / coupling / candidate |
| --- | --- | --- | --- |
| 1–385 | Production | Imports, constants, startup counters, status, connection/chat/cancellation primitives | ConnectionState, ChatState, TerminalOwnership, await_*_cancel; Tauri/Tokio/runtime; retain top-level orchestration |
| 386–1,188 | Production | Managed Desktop state and conversation/session lifecycle | DesktopAppState, DesktopConversationState, ActiveChat, shutdown; persistence/preferences/Codex; broad shared aggregate |
| 1,189–1,531 | Production | Active repository and Commit/workflow state types | DesktopRepository, DesktopCommitIdentity/Capability, RepositoryWorkflowState; rah-tools and state; authority-sensitive |
| 1,532–2,202 | Production | Provider/model endpoints, presentation, publication currentness | ProviderEndpoint, DesktopModelSelection/State, PendingConnectedPublication; provider_composition/Codex/preferences |
| 2,203–4,575 | Production | Effective Authority and HostExplicit reviewed workflows | get_effective_authority_snapshot, CurrentHostComposition, run_host_tool, host prepare/confirm/cancel wrappers, ticket validation; Tauri, state, host_invocation, rah-tools; high authority risk |
| 4,576–5,221 | Production | Status, Trusted Profile, model/Commit preferences and llama.cpp readiness | app_status, profile handlers, model config, LlamaCppReadinessProbe; preferences/profile/provider modules and Reqwest |
| 5,222–6,047 | Production + test helpers | Repository snapshot/status/review workflow | RepositorySnapshot, desktop_snapshot, desktop_repository_snapshot*, PreparedRepositoryWorkflow, refresh; active repo, Git, rah-tools; cfg(test) installer/replacer embedded |
| 6,048–6,821 | Production | Admission, activation, index-effect reservations and Commit invalidation | construct/admit/publish/activate, ActivationTransaction, begin/complete index effect; membership, Git, state, registry; lifecycle-sensitive |
| 6,822–7,687 | Production | Remembered catalog, membership, Close/removal/switch | remembered presentations/commands, close_repository_transition, selectors; remembered_workspace, membership and state |
| 7,688–8,011 | Production | Snapshot, Commit authorization, Stage/Unstage commands | authorize_repository_commit_review, repository_stage_action, repository_unstage_action; selected private index/currentness |
| 8,012–8,731 | Production | Closed ToolRegistry composition and Codex Connect/Disconnect | desktop_tool_registry, composition, prepare/connect/disconnect; rah-tools, profile composer, provider activation and runtime |
| 8,732–9,841 | Production | Chat/runtime turns, activity/status events, conversation persistence commands | run_chat/send/cancel/new/clear/resume/transcript; runtime, managed state, event schemas/persistence |
| 9,842–9,928 | Production | Tauri Builder/setup/shutdown and command registration | Windows Builder, managed state, invoke_handler, non-Windows stub |

There are 12 production responsibility groups. Largest production spans: repository admission/lifecycle/workflow (5,222–8,011, about 2.8k lines); HostExplicit orchestration (2,203–4,575, about 2.4k); provider/model/status/profile settings split over 1,532–2,202 and 4,576–5,221 (about 1.4k total).

## 5. Existing src modules

| Module | Existing responsibility | What remains in root |
| --- | --- | --- |
| codex_baseline.rs | Windows Codex baseline executable discovery/version/hash/config | Connect orchestration and frontend mapping |
| conversation_persistence.rs | Private SQLite transcript persistence/migration/resume bounds | Namespace selection and chat lifecycle integration |
| desktop_preferences.rs | Closed inactive Desktop preference persistence | Command policy and live state publication |
| effective_authority.rs | Closed Desktop Effective Authority projection | Active registry and snapshot composition |
| git_discovery.rs | Closed lazy Git-for-Windows discovery | Git invocation/use in repository workflows |
| host_invocation.rs | HostExplicit request/review/coordinator machinery | Tauri wrappers, current composition and dispatch |
| provider_composition.rs | One-Connect Trusted Profile/provider composition lifecycle | Selection and Connect/Disconnect orchestration |
| remembered_workspace.rs | Descriptive candidate catalog; no repository/Git/provider/runtime/frontend dependency | Presentation, commands, reveal and fresh admission |
| repository_membership.rs | Process-local inert membership IDs/state | Admission/activation/Close/removal and active composition |
| trusted_profile_selection.rs | Closed profile selection/validation/provider-only load | Command policy and preferences integration |

These private domain modules and root wiring are the existing pattern. No command-domain module already duplicates the proposed future responsibility; no evidence root reimplements internals of these modules. Keep host orchestration visible and use one-way edges.

## 6. Major symbol inventory

- App/chat state: DesktopAppState, DesktopConversationState, ActiveChat, ConnectionState, ChatState, TerminalOwnership, AppStatus; methods for connect, turn ownership, cancellation and shutdown.
- Repository/Commit: DesktopRepository, DesktopCommitIdentity/Capability, RepositoryWorkflowState, RepositoryIndexAction/Reservation, TargetObservation, StagedReviewDescriptor, admission/activation/close/index-effect records.
- Provider/model: ProviderEndpoint/Input, DesktopModelSelection/State, ReadinessState, ModelConfigurationPresentation, PendingConnectedPublication/RejectedProviderPublication.
- Presentation/events: ChatEvent, SendChatResult, ActivityEvent, CommitActivityPresentation, HostActivityEvent/State, RepositorySnapshot and status/diff/review presentation structures.
- HostExplicit: CurrentHostComposition, output classification enums, run_host_tool, host_prepare_*, host_confirm_tool_invocation, host_cancel_tool_invocation, ticket/currentness validators and privacy mappers.
- Lifecycle: remembered request/presentation types, membership/close outcomes, ActivationTransaction, admission/activation/member selectors, Commit authorization and Stage/Unstage reservation functions.
- Runtime/session: ConnectionResult, PreparedCodexConnection, desktop_tool_registry, connect/disconnect preparation/publication, run_chat and conversation handlers.
- Scattered test-only: five hooks and four cfg(test) helpers in §3.

## 7. Exact Tauri command inventory (47)

The Windows production invoke_handler at lines 9,850–9,898 registers 47 names. Each is the Rust function name; no rename annotation appears. build.rs explicitly requests generation for 44 command permissions and there are 44 checked-in autogenerated TOML files and 44 allow entries in the default capability. The invoke_handler registers 47 handlers. Three registered handlers are not in the build.rs generation list: host_prepare_repo_edit_files, host_prepare_repo_create_file, host_prepare_repo_delete_file. This inventory records the existing distinction without judging or changing it. Preserve the exact 47-handler set, 44-name generated-permission set, current capability contents and command-to-permission relationships. Task 384 edits none.

Handlers consume managed State<DesktopAppState> directly or via AppHandle and return typed presentations/results or existing FrontendError vocabulary. Request and response Rust/serde types at each function are authoritative; this compact inventory records dependency families, not a new schema.

| Command / Rust function | Input/output dependency family | State / authority |
| --- | --- | --- |
| app_status | none → AppStatus | status presentation |
| trusted_profile_selection | none → TrustedProfilePresentation | configured selection |
| choose_trusted_profile | selected-profile request → presentation/error | human intent, not authority |
| restore_trusted_profile | none → Result<(), FrontendError> | reread selection, no activation |
| forget_trusted_profile | none → result/error | preference only |
| clear_trusted_profile | none → result/error | process-local selection |
| model_configuration | none → ModelConfigurationPresentation | desired provider/model |
| set_model_configuration | closed config request → result/error | host validation |
| reset_model_preferences | none → result/error | preferences reset |
| commit_identity | none → CommitIdentityPresentation | identity display |
| set_commit_identity | identity request → result/error | commit author preference |
| desktop_preferences_warning | none → optional warning | presentation |
| test_llama_cpp_endpoint | endpoint/readiness request → readiness result | bounded GET only |
| choose_repository | root/admission request → repository result | host admission/activation |
| remembered_workspace_catalog | none → catalog presentation | descriptive only |
| reveal_remembered_workspace_location | candidate ID → reveal result | explicit reveal, no admission |
| remember_workspace_candidate | candidate request → mutation result | descriptive persistence |
| update_remembered_workspace_candidate | update request → mutation result | descriptive persistence |
| delete_remembered_workspace_candidate | candidate ID → mutation result | catalog only |
| reorder_remembered_workspace_candidates | ordered IDs → mutation result | catalog only |
| admit_remembered_workspace_candidate | candidate ID → admission result | fresh host validation |
| repository_membership | none → membership presentation | zero/one active member |
| remove_repository_member | member selector → removal result | inactive-only policy |
| activate_repository_member | member selector → activation result | fresh active-only composition |
| close_repository | expected member/generation → close result | withdraw composition, retain member |
| connect_codex | Connect request → ConnectionResult | profile/provider/registry composition |
| disconnect_codex | none → ConnectionResult | runtime/provider lifecycle |
| repository_snapshot | snapshot request → RepositorySnapshot | active-worktree observer |
| repository_authorize_commit_review | review/currentness request → authorization result | Commit capability/currentness |
| repository_stage_action | stage action request → index result | selected private index |
| repository_unstage_action | unstage action request → index result | selected private index |
| send_chat | prompt/request → SendChatResult | session/conversation lifecycle |
| cancel_chat | cancellation request → result | runtime terminal ownership |
| new_conversation | none → result | epoch/history boundary |
| clear_conversation_history | none → Result<(), FrontendError> | persistence policy |
| resume_previous_conversation | resume request → result | bounded replay |
| conversation_transcript | none → transcript presentation | Windows-only read |
| get_effective_authority_snapshot | none → EffectiveAuthoritySnapshot | host-composed observation |
| host_invoke_read | HostReadRequest → result | HostExplicit read dispatch |
| host_prepare_repo_create_branch | branch request → prepared result | zero-effect preparation |
| host_prepare_repo_patch | patch request → prepared result | ticket/currentness |
| host_prepare_repo_edit_files | multi-file request → prepared result | bounded reviewed edit |
| host_prepare_repo_create_file | create request → prepared result | reviewed mutation |
| host_prepare_repo_delete_file | delete request → prepared result | reviewed mutation |
| host_prepare_repo_rename_file | rename request → prepared result | reviewed mutation |
| host_confirm_tool_invocation | ticket-only request → result | revalidate before effect |
| host_cancel_tool_invocation | ticket/cancel request → result | invalidates prepared action |

Preserve exact serde fields/types, error tags/strings, registration set/order and permission mapping. The table groups payload type dependencies; it does not redefine schemas.

## 8. IPC and authority-preservation constraints

Production event names: preferences_warning, conversation_persistence_warning, host_activity_event, chat_event, activity_event, repository_snapshot_refresh. Preserve payload serialization/privacy, correlation and timing. Preserve command IDs, schemas, FrontendError values, activity/status vocabulary, selectors/review tickets, transcript boundaries, managed-state identity and shutdown timing.

| Area | Owner | Root role | Pure-relocation risk |
| --- | --- | --- | --- |
| Admission/membership | Host identity and repository_membership state | Validate and publish inert member | Never add model-selected roots; preserve identity and zero/one active |
| Switch/activation/Close/removal | DesktopAppState lifecycle | Currentness barriers, publication/withdrawal | Moving can break linearization or no-effect rejection |
| Registry/Effective Authority | Host composition | Closed active-only registry and authority projection | Preserve contents; metadata cannot grant permission |
| Trusted Profile | Host selection/provider_composition | Preference and activation orchestration | Keep intent distinct from effective profile/provider lifecycle |
| HostExplicit | Host coordinator/current composition/policy | Wrappers, tickets, confirm/cancel and events | Preserve allowlist, ticket binding, side-effect order |
| Commit | DesktopCommitCapability and selected identity/index/HEAD/branch | Authorize, index effects and invalidate reviews | Preserve private-index/currentness and invalidation |
| Runtime Connect/Disconnect | Host state/provider activation/runtime adapter | Compose and publish if generations remain current | Preserve ownership; no uncertain-effect replay |
| Provider/model | Host preference and closed validation | Configuration/readiness | Cannot grant repository/tool authority |
| Stage/Unstage | Active member private index | Reserve effects and refresh/revoke review | Preserve selected-index/currentness semantics |

## 9. Orchestration versus policy; shared state

Orchestration/wiring: Tauri wrappers, Builder/setup/shutdown, registration, calls into existing modules, event delivery, Connect/Disconnect sequencing.

Policy/authority/business logic: admission/currentness, registry contents, Effective Authority, HostExplicit eligibility/tickets, index reservations, Commit authorization/invalidation, closed provider/model inputs, transcript bounds and cancellation terminal ownership. Some functions do both because DesktopAppState is owned in this root. Keep root as readable composition boundary; do not target an arbitrarily tiny file.

DesktopAppState is a broad aggregate referenced across commands/tests. This is real coupling, but file size is chiefly test placement, not merely duplicated state access. Passing State<DesktopAppState> to child command handlers can preserve Tauri ownership if parent state and privacy remain; do not redesign state or create main ↔ repository_commands cycles.

## 10. Test coupling and privacy

The current cfg(all(test, target_os = "windows")) tests module is a child of crate root and uses super imports to access private functions/types/fields/constructors. Rust child modules can access private ancestor items. Moving its body unchanged to src/main_tests.rs while declaring the same child using #[path = "main_tests.rs"] mod tests under identical cfg preserves privacy and requires no pub/pub(crate) widening. These are private-path tests, not external tests/ without API expansion. Keep the five test hooks and four scattered cfg(test) helpers in root in phase one because state/barriers couple them to production impls.

## 11. Strategy comparison

Ranges reflect boundary choices. Moving authority-sensitive test text is not production semantics.

| Strategy | main.rs reduction | New files | Production moved | Test moved | Authority-sensitive production lines touched | Registrations | Evidence/risk |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A. Test child only | ~25,603 (72.1%) | 1 | 0 | ~25,604 | 0 | 0 | 214 deterministic tests; focused Windows suite; lowest risk |
| B. Command domains | ~1.2k–3.0k | 1–3 | same | 0 first | ~0.8k–2.5k lifecycle/HostExplicit | names reviewed, unchanged | Focused/full Desktop tests and domain smoke; medium/high |
| C. Lifecycle | ~1.8k–2.8k | 1–2 | same | 0 | ~1.2k–2.3k | command path reviewed | Windows admission/switch/Close/index/Commit proof; high |
| D. Tests adjacent to domains | similar to B/C | 2–5 | ~1.2k–2.8k | selected portions | domain-dependent | module wiring reviewed | broad suite repartition/privacy review; medium/high |
| E. Broad rewrite | up to ~9.8k production plus tests | many | up to ~9.8k | up to ~25.7k | nearly all | broad | full deterministic/live proof; very high, no justification |

A is viable because the source already has the needed child privacy relation. E mixes semantic boundaries and obscures relocation review.

## 12. Selected first extraction and target role

**Task 385 should mechanically extract lines 9,929–35,532, the entire Windows-only tests child, to crates/rah-desktop/src/main_tests.rs**, preserving module name/child relation using the same cfg and #[path]. Expected main.rs reduction is about 25,603 lines (72.1%); test content remains in the repository. Production semantic impact: **none**. Do not split tests, alter fixtures or widen visibility. Stop if production code/schema/state semantics change.

Long-term main.rs role: module declarations, Desktop state ownership, app/bootstrap/setup/shutdown, Builder/registration and high-level host composition where visible ownership aids audit. Retain additional orchestration where that is clearer; no arbitrary line limit.

## 13. Validation frozen for Task 385

1. Verify clean state and exact base HEAD; review full diff.
2. Make a mechanical move. Compare old/new module body byte-for-byte, accounting only for cfg/path declaration. Use git diff --color-moved=plain, --stat, direct body hash/compare and git diff --check.
3. Run cargo fmt --check.
4. On Windows run cargo test -p rah-desktop -- --test-threads=1. Confirm 214 deterministic tests and 17 ignored entries are unchanged; do not enable live gates.
5. Windows is required to compile this cfg-gated module; Linux-only package testing is insufficient.
6. Full workspace/live certification is not required for a pure test-file move unless review finds production composition changes or an existing gate requires it.
7. Confirm only main.rs path wiring and main_tests.rs changed; no build.rs, Cargo files, handlers, permissions or capabilities changed.

Later phases:
- Provider/preferences-only: fmt, Desktop check and serial Desktop suite on Windows; Connect/profile smoke if composition lifecycle moves.
- Repository snapshot-only: focused snapshot/currentness plus full Desktop suite; active-worktree/linked isolation smoke if observer wiring changes.
- Admission/membership/activation/Close/index/Commit: full serial Desktop tests and Windows host-driven admission/switch/zero-one/Close/removal/Stage/Unstage/Commit/currentness/linked-isolation evidence. If registry/composition changes, rerun relevant Windows live evidence including repo.list at the new descendant.
- HostExplicit: full serial Desktop tests and Windows backend proof of moved families, allowlist, tickets/currentness/effects/events/errors; relevant live gate if dispatch/composition/effects move.
- Runtime/session/chat: full serial Desktop tests and Windows smoke for connect/disconnect, send/cancel/terminal, persistence/resume/shutdown. No inference claim without its gate.
Cross-domain/behavior changes require applicable broader gates.

## 14. Roadmap and review

- Task 385 — mechanical tests-child move to main_tests.rs, no semantic edits.
- Task 386 — independent structural/behavior audit: body equivalence, privacy, production/command identity and Windows deterministic parity; fail/stop if production changed.
- Task 387 — research one production domain after audit, based on actual post-move dependency evidence.

385 and 386 should be separate commits: mechanical move, then independent audit record/closure. Preserve provenance. Review with git diff --color-moved=plain, git diff --stat, body comparison and git diff --check; inspect staged/unstaged paths. Commit only under later task authorization.

## 15. Stop conditions and non-goals

Stop if extraction requires authority redesign; public API/IPC/permission identity changes; HostExplicit or Tool semantics change; repository/provider lifecycle change; new dependency; version bump; ADR. Those need a separate product/architecture task.

Task 384 performs no code move/module creation, symbol rename, visibility change, import cleanup, test/certification edit, behavior/permission/authority/ToolRegistry/IPC/Tauri change, Cargo/dependency change, frontend change, version bump, ADR, Task 383 reinterpretation, push/tag or release preparation.

## 16. Repository facts and exact next task

Observed workspace: 13 packages, version 0.31.0, edition 2024. Task 384 is documentation-only; no Rust/Cargo changes are authorized.

**Exact next task: Task 385 — move the Windows-only tests child at main.rs lines 9,929–35,532 byte-for-byte to src/main_tests.rs, preserving cfg and direct child relation with #[path]. No other edits. Validate body equivalence, cargo fmt --check, git diff --check and Windows cargo test -p rah-desktop -- --test-threads=1; confirm 214 deterministic tests and 17 ignored live entries remain unchanged.**
