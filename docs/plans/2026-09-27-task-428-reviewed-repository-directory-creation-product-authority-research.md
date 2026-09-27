# Task 428 — Reviewed Repository Directory Creation Product and Authority Research

## 1. Starting checkpoint

Verified before research:

- `HEAD`: `df36cfe85cc634f9ccf871e8db3c12ff020102ef`
- `origin/master`: `df36cfe85cc634f9ccf871e8db3c12ff020102ef`
- Task 427 commit: `df36cfe85cc634f9ccf871e8db3c12ff020102ef`
- Worktree: clean.
- Task 427 exact-head CI: run `36288234150`, reported PASS for formatting, workspace check, workspace tests, lint, and overall result.

The Task 427 source plan records the verdict `NO FEATURE — DISCONNECT / CONNECT REMAINS THE CLOSED TRUSTED PROFILE RECOMPOSITION BOUNDARY`. This research does not reopen that subject.

## 2. Task 426/427 context

Task 426 selected no v0.33 primary capability because no candidate yet established the combination of material product benefit, narrow closed authority, deterministic testability, and acceptable lifecycle/currentness complexity. Its candidate table already recorded HostExplicit `repo.create-directory` as a possible narrow extension, but said product leverage was not established and deferred it.

Task 427 examined the strongest operational candidate and rejected live Trusted Profile recomposition. The next candidate examined here is a human-started, reviewed route to the existing ordinary `repo.create-directory` Tool. This task changes neither scope nor any accepted decision.

## 3. Existing ordinary directory authority

Accepted ADR 0019 defines `RepositoryDirectoryCreationPolicy` and the closed request `{"path":"existing-parent/new-directory"}`. The ordinary effect requires one absent final leaf below one already-existing validated parent. It is not recursive, does not make files or markers, does not invoke Git for mutation, and grants no stage, commit, ref, delete, rollback, retry, or replay authority.

The current implementation at `crates/rah-tools/src/repository_create_directory.rs` follows the authority contract: validates a closed request and bound context, serializes repository mutations, revalidates parent/repository/target and protected Git observations before one native create attempt, observes the filesystem and protected state afterward, and returns sanitized classified outcomes. The statuses include `invalid_input`, `precondition_failed`, `directory_created_verified`, `known_no_effect`, and `uncertain`. A verified result proves a filesystem effect plus unchanged protected Git state; Git status is not the creation proof.

ADR 0019 already owns the underlying structural mutation. This research neither redesigns that contract nor changes ordinary availability.

## 4. Existing Windows evidence

The Task 186 Windows live-validation plan records production-path PASS for the existing ordinary Tool using Codex `0.149.0` and a disposable repository. It records one exact `{"path":"parent/new-directory"}` request and exactly one Requested / Started / Finished lifecycle. The created directory was ordinary and empty (child count zero), with no `.gitkeep` or `.keep`; fixture file hashes were preserved; status and cached/worktree diffs were empty; HEAD, branch, and refs were unchanged; no stage, commit, retry, replay, compensation, delete, or rollback occurred.

That certification proves the existing model/runtime Tool path works on Windows. It does not prove a human-reviewed HostExplicit path or establish user demand for one. Task 428 performs no new live validation.

## 5. Current HostExplicit architecture

`crates/rah-desktop/src/host_invocation.rs` defines exactly eleven `HostInvocationKind` variants and `host_kind` explicitly maps the eleven production names. `repo.create-directory` is not mapped, so the current descriptor is ineligible (`NotSupported`) even though the ordinary Tool may be present in the selected repository composition. `main_tests.rs` asserts that HostExplicit remains exactly eleven and `host_kind("repo.create-directory")` is `None`.

`effective_authority.rs` independently classifies the ordinary Tool as `EffectClass::RepositoryMutation`, `AuthorityCategory::RepositoryDirectoryCreation`, `PermissionLevel::Execute`, and `repository_bound = true`. That is descriptive underlying authority metadata. It is not an eligibility rule. HostExplicit requires an explicitly admitted kind, a first-party eligible source, a current composed Tool, currently allowed permission, current repository/context, and capability-specific preparation. Provider metadata cannot make a provider Tool eligible. Therefore ordinary Tool availability, Execute permission, Tool definition, Effective Authority visibility, and frontend state do not imply human HostExplicit authorization.

## 6. Product problem

The possible workflow gap is: a human wants a folder to exist, the ordinary Tool can create one if model dispatch selects it, but there is no human-only Prepare/review/Confirm route inside RAH. Source proves the ordinary Tool and the missing explicit route. Task 426 documentation says a direct human empty-folder action could help some authoring flows, but also records no source evidence of a frequent blocker. No usage or user-research evidence establishes frequency or severity.

Creating a new file with `repo.create-file` requires the parent to exist. `repo.rename-file` changes a file path and is not directory creation. An ordinary model request for `repo.create-directory` already covers automated folder creation. A user can also create a folder in the OS or editor outside RAH; that path has its own UI but no RAH lifecycle, ticket, or repository-currentness binding.

The incremental product proposition is specifically: “I explicitly want RAH to create this folder, independently of model choice.” That is plausible workflow value, not established demand. The ordinary Tool has already been live-certified, so the case cannot rest on capability availability alone.

Evidence classification:

| Claim | Classification | Evidence |
| --- | --- | --- |
| Ordinary Tool creates one empty directory through the selected repository composition | Source-proven and Windows-live-proven | ADR 0019; Tasks 185/186 plans and current Tool/compose source |
| There is no HostExplicit route today | Source-proven and documentation-proven | `host_kind`, `HostInvocationKind`, exact-eleven assertion; ADR 0021 and Task 426 |
| A person can request an explicit in-RAH folder action | Workflow-inferred | Human HostExplicit patterns for other capabilities; no directory-specific request evidence |
| Users often need this or are blocked by model choice | Not established | No frequency, telemetry, interview, or incident evidence identified |
| Manual folder creation remains available outside RAH | Workflow-inferred | OS/editor workflow; not a RAH source capability |

## 7. Empty-directory/Git visibility issue

Git does not track an empty directory as an independent tree entry. A successful creation can leave `git status --short`, `git diff`, and `git diff --cached` empty while HEAD, refs, and index remain unchanged. The Task 186 ordinary live run observed exactly this. Clean status must not be presented as “nothing happened” or used as success proof.

This does not make informed review technically impossible: a person may intentionally want an empty directory for a later workflow. It does reduce visible payoff and review-to-commit continuity. Until separately authorized content exists, the folder has no Git-visible persistence for a checkout/clone recipient. No claim that empty directories survive Git checkout should be made.

## 8. Proposed review semantics

A future backend-derived review could say, in bounded sanitized terms:

```text
Operation: Create directory
Path: docs/generated
Effect: Creates one empty ordinary directory.
Parent: Existing validated repository directory.
Git effect: None expected. Git does not track empty directories.
Non-effects: No files or markers; no recursive parent creation; no stage or commit.
```

It must not expose the absolute root, native handles, executable paths, Git-private paths, environment, filesystem IDs, or raw errors. The review should state the exact normalized logical path and one-leaf effect, not claim that Git will retain the folder.

This is less evidential than a content preimage/diff, but it can still be accurate and intelligible consent for a deliberately structural action. Whether that low-information review is useful enough to justify product surface is a product-value question, and current evidence does not establish that need.

## 9. Prepare semantics

A proper route needs a new zero-effect capability-specific preparation object; the ordinary Tool request is not a human review or a ticket. Prepare would validate and privately retain the selected repository identity/generation, exact normalized logical path, immediate parent existence and identity, ordinary-directory status, all ancestry/boundary checks, nested-repository exclusion, target absence and aliases, current supported Git-layout identity, and the review summary. It must perform no native mutation and issue no ticket if any proof is incomplete.

Private retained state should include the capability preparation needed to revalidate those facts, exact reconstructed Tool call/definition binding, and any opaque preparer identity. The human sees only sanitized review fields. Filesystem handles, canonical roots, IDs, raw errors, and Git internals remain private.

## 10. Currentness/staleness

Generic HostExplicit generations, composition identity, repository identity, registry identity, permission policy, Tool definition, and capability-preparer identity can cover context replacement. Directory-specific revalidation must cover path and filesystem state:

- If the target appears after Prepare, Confirm fails before native effect; an existing target is never idempotent success.
- If the immediate parent is replaced, its identity mismatch fails before effect.
- If any parent becomes a symlink, junction, or reparse point, boundary revalidation fails before effect.
- If repository generation/context changes, generic currentness rejects the ticket before effect.
- If Git layout identity or required protected Git observations become stale/contradictory, capability revalidation rejects before effect.

External actors can race after revalidation. RAH can detect specified changed identities and postconditions but cannot provide a global external-process lock or eliminate every TOCTOU window. A target altered immediately after creation may make ownership or postcondition proof impossible; that result must be uncertain.

## 11. Exact ToolDefinition binding

Confirm must bind and recheck the exact ordinary first-party `repo.create-directory` definition observed at Prepare: canonical name, permission, closed input schema, description/definition identity, current registry, and repository-bound source classification. The prepared call is reconstructed from the reviewed path and is not replaced by Confirm input. ADR 0021 D2 re-resolves the Tool and requires exact expected/current definition identity plus current permission before dispatch. Same-name provider Tools are not eligible: MCP/Process Plugin source kinds remain provider-ineligible and cannot self-declare host eligibility.

## 12. Permission/effective-authority analysis

The existing classification remains `RepositoryMutation` / `RepositoryDirectoryCreation` / `Execute` / `repository_bound = true`. Execute is only the dispatch permission; neither it nor this classification grants authority. The ordinary `RepositoryDirectoryCreationPolicy` remains the sole underlying mutation authorization.

Adding a reviewed route changes who can explicitly dispatch the existing Tool under a typed human ticket. It does not change EffectClass, AuthorityCategory, PermissionLevel, Tool schema, or ordinary Tool authority. HostExplicit eligibility is an additional trusted routing/review decision, not a new mutation capability and not a synonym for Execute.

## 13. Generic HostExplicit reuse

The current coordinator already serializes one model turn or one HostExplicit preparation/running action at a time. A HostExplicit directory preparation can reuse this exclusion, the opaque process-local single-use expiring ticket, host activity provenance, current registry, allowed permissions, four generation values, repository/composition identity, exact definition validation, and `authorized_tool_dispatch -> ToolRegistry` path. No parallel model mutation route is needed or acceptable.

Generic currentness is not enough by itself: it does not prove target absence, parent object continuity, no-link ancestry, nested-boundary status, or Git-layout facts. A new capability-specific preparer and retained payload should supply these, and the existing generic confirmation path should invoke its revalidation before D2. Avoid duplicating generic generation/registry checks in a second authority mechanism.

## 14. Capability-specific ADR precedent

The established pattern is: ordinary Tool authority + ADR 0021 generic HostExplicit coordination/currentness/ticket/provenance + a capability-specific reviewed workflow contract. ADRs 0024, 0025, and 0026 separately define reviewed new-file, deletion, and rename workflows; ADR 0022 provides the worktree authoring boundary. They preserve the existing ordinary mutation authority and ToolRegistry dispatch, while adding typed Prepare, backend-derived review, ticket-bound currentness, conservative results, and independent capability proof.

If directory review were selected, ADR 0019 would remain unchanged and a separate capability-specific reviewed-directory HostExplicit ADR would be required before implementation. Generic ADR 0021 does not provide directory review completeness, retained target/parent proof, or structural post-effect semantics. No ADR is written or status-changed here.

## 15. Post-effect proof

A future route should not trust model-visible ToolOutput as the review proof. The ordinary Tool's strict output classifier is useful for dispatch result handling, while reviewed success should independently verify the retained exact path and structural outcome: target exists as one ordinary empty directory within the selected repository; parent identity remains the one prepared; no marker/file was created; and protected index, HEAD, branch/refs and repository/layout observations remain unchanged. The prepared absence proof plus one authorized ordinary Tool dispatch and postcondition establish the bounded effect as far as the implementation's identity model permits.

Only `directory_created_verified` may become reviewed terminal success. `invalid_input`, `precondition_failed`, `known_no_effect`, dispatch rejection, and stale state remain non-success classifications. They must not be converted to success from a clean Git status. A lost or contradictory postcondition remains possible-effect-unknown/uncertain.

## 16. Uncertainty/no-replay

The native operation remains exactly one attempt. Once the attempt may have occurred, no retry, replay, second create, cleanup, deletion, or compensation is allowed. A native failure can be known-no-effect only if fresh observation proves the target remains absent and protected state is intact. If target ownership, emptiness, parent identity, or relevant protected state cannot be established, retain `uncertain` / possible-effect-unknown presentation. Human confirmation does not justify a retry after uncertainty and does not imply rollback.

## 17. Commit authorization interaction

The Task 185 Desktop integration and current tests establish that a verified ordinary directory mutation refreshes repository presentation and invalidates outstanding reviewed Commit authorization even if Git status is clean. A structural worktree mutation makes the prepared Commit review's currentness assumptions stale even without a Git-visible diff.

A future reviewed route must preserve that invalidation. It may conservatively invalidate at the HostExplicit Started boundary immediately before dispatch, consistent with reviewed authoring patterns, so an uncertain possible effect cannot leave old Commit authorization usable. If it waits for terminal classification, it must at minimum invalidate on verified success and any possible-effect uncertainty. Prepare alone and failure known before effect should not consume an unrelated Commit authorization. The capability ADR must choose and state the precise point.

## 18. Frontend/ticket boundary

The minimum future UX is typed path entry, backend Prepare, display of backend-derived review, human Confirm, and ticket-only Confirm request. The frontend presents the returned path/effect/Git visibility; it does not construct authoritative facts, choose permissions, or decide currentness. Confirm resends no path or JSON. Existing HostExplicit tickets are opaque, process-local, single-use, bounded-lifetime, bound to the exact payload and context, and rejected when stale; they are not reconstructed from frontend state.

No full UI design, IPC command, permission, or frontend change is authorized by Task 428.

## 19. Windows considerations

Reuse ADR 0019's existing Windows path policy and native protection. Review and revalidation must account for junction/reparse ancestry, reserved names, trailing dot/space aliases, case-insensitive equivalence, path spelling, colon/ADS, UNC/device/verbatim forms, `.git`, nested repositories, and equivalent existing targets. Confirm must revalidate parent identity and target absence immediately before the existing one-call CreateDirectoryW path. Races after that point remain possible; post-effect proof must classify them conservatively. Do not weaken ordinary path handling for UI convenience.

The ordinary Windows production path is already live-certified, so a future reviewed-route live gate is practical in principle, but it would need to prove human Prepare/review/Confirm and HostExplicit lifecycle rather than rerun model Tool selection. This Task performs no GUI automation and makes no new live-cert claim.

## 20. Cross-platform claims

Existing deterministic Unix tests may support portable contract coverage, but the Task 186 evidence is Windows production live certification only. It does not establish Linux or macOS live parity. Keep deterministic portable tests, Windows live certification, and any later Linux/macOS live claim separate. Unix mode/umask/default ACL behavior and native directory creation remain platform-specific evidence.

## 21. Product comparison

| Workflow | Ordinary model Tool | Proposed HostExplicit reviewed route | Manual external folder creation |
| --- | --- | --- | --- |
| Who initiates | Model request | Human | Human |
| RAH authority | Existing ordinary directory policy | Same ordinary policy, reached through a reviewed human route | None in RAH |
| Review before effect | Tool request/schema and ordinary execution checks | Backend Prepare review, then explicit Confirm | OS/editor UI |
| Git visibility | Usually none for an empty folder | Usually none for an empty folder | Usually none for an empty folder |
| Currentness binding | Ordinary execution preconditions | Prepare/Confirm ticket and revalidation | Outside RAH |
| Audit lifecycle | Tool Requested/Started/Finished | HostActivity/review lifecycle | Outside RAH |
| User benefit | Automated folder creation | Explicit RAH action independent of model choice | Already available through OS/editor |

The proposed route offers an in-RAH audit/currentness path and removes dependence on model selection for this one structural action. Manual creation already satisfies the basic human intent; ordinary model dispatch already satisfies automation. No evidence currently shows that either alternative is a material blocker.

## 22. Deterministic-test direction

If reconsidered, tests should cover valid root-level and nested-existing-parent creation; missing parent and existing target; target appearing after Prepare; parent replacement; symlink/reparse parent; nested repository; reserved/alias path; stale repository generation, composition identity, definition and ticket; duplicate Confirm; active model turn and HostExplicit busy state; unavailable permission; same-name provider spoof; verified success with clean Git status; reviewed Commit invalidation; target race/uncertainty; exact one native attempt; and no retry, cleanup, marker, stage, or commit. Use deterministic barriers/hooks, not sleeps. Failure and stale cases must prove zero native attempts where they occur before dispatch.

## 23. Windows live-cert direction

If reconsidered after contract acceptance, a separate Windows live gate should exercise human Prepare, rendered backend review, human Confirm, HostExplicit Started, exactly one authorized dispatch through the existing ToolRegistry, `directory_created_verified`, and independent reviewed post-effect proof. It should assert the exact logical path, empty ordinary directory, no marker, status possibly clean, unchanged index/HEAD/refs, no model Tool request causing the effect, no other mutation Tool, stale-ticket rejection, unchanged repository context, and one native attempt. Do not claim GUI automation unless performed.

## 24. HostExplicit 11→12 decision

The production count remains 11. A reviewed directory route would intentionally expand the trusted HostExplicit set from 11 to 12 and increase the human-reachable effect surface, even though the underlying ordinary directory authority already exists. Technically it can be a narrow extension: one typed path, one existing-parent leaf, one existing Tool, existing coordinator, and existing mutation policy. It is not architecturally trivial, because new eligibility, Prepare/review, payload, revalidation, independent proof, activity result mapping, and certification would be required.

The selection bar is not met because a credible human workflow need and material incremental value are not established. The clean-Git review limitation is manageable but decreases visible payoff. Therefore the 11→12 authority-surface expansion is not justified for v0.33 on current evidence.

## 25. ADR conclusion

ADR 0019 already covers the underlying one-directory mutation and remains unchanged. ADR 0021 supplies reusable generic HostExplicit dispatch and ticket boundaries. If later product evidence selects the route, a capability-specific reviewed-directory ADR would be required, following ADRs 0024–0026. Task 428 makes no ADR change and recommends no such ADR now.

## 26. Final outcome

**NO FEATURE — ORDINARY `repo.create-directory` REMAINS SUFFICIENT.**

“Sufficient” here is the scope decision supported by available evidence: ordinary model-driven creation is implemented and Windows-live-certified, and manual human creation remains available outside RAH. It is not a claim that a HostExplicit route is technically impossible. The proposed review can convey informed consent for one empty folder, and the existing coordinator/currentness model is reusable with a new capability-specific preparer. The missing element is credible evidence that independent human initiation inside RAH solves a material user problem strongly enough to justify adding a twelfth HostExplicit route.

## 27. Exact next task

Return to v0.33 candidate selection under the Task 426 roadmap. Do not start Task 429 reviewed-directory contract research unless new product evidence changes the selection decision. Task 427 remains closed at its stated boundary.
