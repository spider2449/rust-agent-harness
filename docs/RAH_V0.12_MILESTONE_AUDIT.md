# RAH v0.12 Milestone Audit

**Task 152 — research, audit, and documentation only**

## Decision

**READY FOR v0.12 RELEASE PREPARATION**

The approved v0.12 outcome is complete: Windows Desktop provides bounded repository authoring, human Stage/Unstage, complete staged review, explicit host-owned reviewed-snapshot authorization, and a one-shot message-only `repo.commit` through the existing Generic Tool Bridge. This productizes accepted authority and introduces no new authority.

## Audit basis and method

Starting SHA and `origin/master` were both `1021b1fb1f2662f6c59002a14119ab3cde37cba3` (`fix: validate complete Codex Desktop runtime bundle`). The initial worktree was clean and `.vscode/` was untouched. This audit inspected the Task 144 roadmap; Task 145 research; the Task 147/147A index-review implementation; Task 148 authorization; Task 149 bridge integration; Task 150 deterministic proof; Task 151 Windows live record and prerequisite fix; committed source and tests; README, CHANGELOG, architecture/security documents; and ADRs 0010–0016. Accepted ADRs were not reopened because no contradictory evidence was found.

## Approved scope and task closure

Task 144 selected Desktop end-to-end repository authoring, staged review, explicit human reviewed-snapshot authorization, and bounded `repo.commit`; its conclusion was **NO NEW AUTHORITY REQUIRED**. The delivered implementation stayed within that boundary.

| Task | Purpose | Commit SHA | CI evidence | Implementation / research result | Authority impact | Known limitation | Closure status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 144 | Scope roadmap | `aa2f2d32d7c83033cbd7ff4d24abd64c96f3330c` | `33302269295` PASS | Selected this workflow; docs only | None | No implementation | COMPLETE |
| 145 | Integration research | `27e9247bd3653240c679f95d775a2884c47ec11b` | Recorded gates PASS | Direct public `rah-tools` composition is safe; no `rah-cli` dependency | None | Live proof deferred | COMPLETE |
| 146 | Shared composition foundation | — | Not applicable | Direct composition was feasible | None | None | **SKIPPED BY DESIGN** |
| 147 | Stage/Unstage and staged review | `669a4b36751121e137a539398e21d603a4c7ca98` | `33304029128` PASS | Human actions, opaque selectors, complete review, binary refusal | None | 147A required | COMPLETE |
| 147A | Selector hardening follow-up | `cb836446593ec8f3f79178196bce90fd18c4993a` | Recorded gates PASS | Repository/observation binding and canonical digest | None | None | COMPLETE follow-up |
| 148 | Reviewed authorization UX | `fd16d8fc42867b1777514be5a56da1ca2b9748d6` | Recorded gates PASS | Opaque Rust-only compare-and-arm control | None | No direct commit yet | COMPLETE |
| 149 | Desktop `repo.commit` integration | `9beb057377fbe529c91a260208eb83504b6d27c7` | Recorded gates PASS | Paired Tool/control enters Desktop registry; sanitized result and refresh | None | No bridge redesign | COMPLETE |
| 150 | Deterministic hardening | `499f726c0be6b977a9ca716e12e49cb62bb65a9c` | Recorded gates PASS | Cross-layer pairing and Execute-not-authorization proof | None | Live proof remained | COMPLETE |
| 151 | Windows live validation | `1021b1fb1f2662f6c59002a14119ab3cde37cba3` | `33463666613` PASS | Live workflow and certified-bundle repair | None | Two observability gaps | COMPLETE |

The Task 144 sequence is delivered: 144 COMPLETE; 145 COMPLETE; 146 **SKIPPED BY DESIGN**; 147 COMPLETE; 147A COMPLETE follow-up; 148 COMPLETE; 149 COMPLETE; 150 COMPLETE; 151 COMPLETE; and 152 audited here.

## Product workflow audit

| Transition | Classification | Audit evidence |
| --- | --- | --- |
| Choose Repository -> Connect | IMPLEMENTED; DETERMINISTICALLY VERIFIED | One canonical repository and repository/model generation-bound connection. |
| Connect -> model inspect/read | IMPLEMENTED; DETERMINISTICALLY VERIFIED | Private registry exposes bounded read/observer tools for the selected repository. |
| Inspect/read -> bounded edit/create | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | Bounded authoring path live-proven; exact Tool label is not durably known. |
| Edit/create -> Desktop presentation | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | Host observations refresh presentation. |
| Presentation -> human Stage/Unstage | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED (Stage) | Rust-owned, consumed single-file selectors. Unstage is not separately live-proven. |
| Stage/Unstage -> host-observed review | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED (Stage path) | Complete textual review is required; binary is not authorizable. |
| Review -> human Authorize Commit | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | One lease-held compare-and-arm of opaque Rust review. |
| Authorize -> message-only `repo.commit` | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | Existing bridge dispatch; model supplies only message. |
| `repo.commit` -> ADR 0016 validation -> one Git attempt | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | Snapshot revalidation and one-shot fixed native command. |
| Attempt -> verified result -> refresh | IMPLEMENTED; DETERMINISTICALLY VERIFIED; WINDOWS LIVE VERIFIED | Sanitized OID matches independent Git HEAD; post-commit refresh is clean. |

Unix native Desktop validation is **NOT LIVE-PROVEN / DEFERRED**. Cross-platform deterministic tests are not an assertion of Unix live support.

## Authority and composition audit

| Boundary | Audit result |
| --- | --- |
| Read: `fs.read` / repository observers | Preserved as host-bounded observation. |
| Worktree mutation: `repo.patch`, `repo.create-file`, `repo.edit-files` | Preserved as separate existing bounded tools. |
| Index mutation: human Stage / Unstage, ADR 0010 | Preserved as host action, not model Tool authority. |
| Commit/history mutation: `repo.commit`, ADR 0016 | Preserved as reviewed, snapshot-bound, one-shot authority. |

Desktop directly composes existing approved `rah-tools`, with no `rah-cli` dependency. A single `RepositoryCommitTool::compose` result supplies the exact `RepositoryCommitTool` registered in the Desktop `ToolRegistry` and its Rust-only `RepositoryCommitControl`. `repo.commit` remains in the existing Generic Tool Bridge with a closed message-only schema. Repository, model, and identity generations are host-owned; one connection binds one selected repository; reconnect and repository changes cannot retain authorization. Task 146 remains legitimately skipped.

Model request is not authorization. Execute is only the outer dispatch gate. Frontend presentation is not authorization, and the Task 147 digest is not authority. `RepositoryCommitReview` remains opaque and Rust-only. Authorization is host-owned, in-memory, one-shot, and non-durable; persisted identity is configuration only. No auto-stage, direct host Commit button, generic Git authority, or generic shell/process authority exists.

The displayed review and opaque review binding come from the same host observation. Compare-and-arm holds the repository lease. Refresh, Stage/Unstage, disconnect, identity change, repository/model generation change, restart, and resume revoke or cannot restore pending authorization. Stale selectors fail closed and binary staged content cannot become authorizable.

## `repo.commit` and deterministic evidence

ADR 0016 remains unchanged and authoritative. The model input is exactly `{ "message": string }`; the host fixes repository, Git executable, identity, branch/HEAD expectations, index snapshot, and tree binding. One fresh authorization allows at most one native Git attempt. No auto-stage, model-selected branch/arguments/identity/token/ref, generic ref mutation, network Git, retry, or replay after uncertain effects is available.

Task 150 proves exact Desktop Tool/control pairing and that Execute dispatch without authorization cannot commit. With host review and authorization, it proves verified OID, exact parent, one commit, reviewed index content, unstaged and untracked non-inclusion, and replay refusal. Existing lower-layer/Desktop tests cover closed message schema and bounds, strict result parser and taxonomy, stale index/HEAD/branch refusal, one-shot consumption, repository/model/identity invalidation, disconnect/reconnect and restart/resume non-restoration, binary refusal, no auto-stage, post-attempt refresh, and sanitized frontend DTOs. No required deterministic invariant is missing.

## Windows live evidence and certified bundle repair

Task 151 used certified `C:\Users\spider.tp\AppData\Local\codex-baselines\0.149.0\codex.exe`, reported `codex-cli 0.149.0`, with manifest v2. Required runtime artifacts were `codex.exe` SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00` and `codex-code-mode-host.exe` SHA-256 `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`; the bridge marker was `RAH_ECHO_BRIDGE_OK`.

At `D:\rah-task151-clean`, live commit `90683f5eaab129a75e815879e69586ff75de5e86` used message `RAH Task 151 live commit`. Independent Git verification proved displayed/model-reported OID equalled actual HEAD, parent equalled old HEAD, count changed 1 to 2, committed `tracked.txt` was exactly `RAH_TASK151_EDIT_OK` LF-terminated, current branch equalled new HEAD, staged diff and worktree were clean, and no second commit/replay occurred.

The old verifier could certify `codex.exe` with no required code-mode host. The repair is runtime prerequisite hardening, not new authority: closed manifest v2; exact sibling `codex.exe` and `codex-code-mode-host.exe`; separate lowercase SHA-256 values; Windows PE identity; no reparse points; canonical containment; invalid existing certified directories fail closed; PATH fallback only when no certified baseline directory exists. No extra sandbox helpers are claimed required.

The exact live repository-authoring Tool label was not durably persisted, so this audit does not invent one. Exact live `repo.commit` `ToolRequested` / `ToolStarted` / `ToolFinished` counts were not recoverable, so it does not claim `1 / 1 / 1`. These are non-blocking observability gaps: deterministic lifecycle coverage exists and independent Git state uniquely proves one commit effect with no automatic second commit.

## Scope limits and final conclusion

No generic `fs.write`, `process.exec`, `shell.exec`, arbitrary Git command, branch create/switch, delete/rename, network Git, credential Git, profile hot reload, network MCP, PluginManager lifecycle authority, multi-repository execution, OS sandbox guarantee, network isolation guarantee, or rollback guarantee was introduced. Process supervision is not sandboxing. Trusted Profile remains an independent trusted-host path; Desktop did not become one and no profile schema changed. The Generic Tool Bridge was not redesigned.

Windows is live-certified. Task 120 remains deferred: `RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`; transport confinement remains not claimed. Intentional limits include Windows-only live certification, Codex 0.149.0 pinning, the demonstrated helper pair, binary authorization refusal, no direct host commit button, no branch/ref management, no delete/rename, no generic file write, no network Git/MCP, no plugin manager, no dynamic profile reload, no multi-repository authority, no OS sandbox or rollback guarantee, and mitigated/revalidated rather than mathematically race-free TOCTOU.

**READY FOR v0.12 RELEASE PREPARATION**

RAH v0.11.0 remains released. ADR 0010, ADR 0011, ADR 0012, ADR 0013, ADR 0014, ADR 0015, and ADR 0016 remain authoritative. v0.12 introduces no new authority. This task creates no version bump, tag, release, or release gate. The next task is **TASK 153 — v0.12 RELEASE PREPARATION**.
