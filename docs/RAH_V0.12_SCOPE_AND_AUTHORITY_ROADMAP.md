# RAH v0.12 Scope and Authority Roadmap

**Task 144 — research / roadmap only**

## Decision

**RECOMMENDATION: Desktop end-to-end repository authoring, review, and bounded commit workflow.**

**NEW AUTHORITY REQUIRED: NO.** v0.12 should productize the already accepted
host-owned authorities in ADRs 0010, 0011, 0012, 0013, 0014, and 0016. In
particular, it should make the existing `repo.commit` authority usable through
the Windows Desktop host without making the frontend, transcript, or model an
authority source.

**WHY NOW:** v0.11 completed the narrow final authority needed for the existing
repository authoring path, but Desktop does not compose or expose that path.
The largest immediate user gain is therefore closing a documented product gap,
not widening the authority surface again.

**WHAT USER CAN DO:** select one host-owned repository, use Codex through RAH
to inspect and perform existing bounded edits/creates, stage through a
host-controlled surface, inspect the current staged diff, explicitly confirm
that exact state, allow one message-only `repo.commit` call, and see the
independently verified result in the same Desktop conversation.

**WHAT REMAINS DEFERRED:** branch/ref/checkout authority, delete/rename,
network MCP, PluginManager lifecycle, dynamic profile reload, multi-repository
execution, transport confinement, and remote llama.cpp successful-generation
proof.

## Authoritative baseline

- Required and observed `HEAD` and `origin/master`:
  `41d9e30e4c119fd0e09069db9569c5198aafd8bb` (`docs: mark RAH v0.11.0 released`).
- Initial worktree: only `?? .vscode/`; it is outside this task and untouched.
- Immutable release: `v0.11.0`, tag object
  `3fd37807f382c2c0c61328e72d7542984db05983`, peeled commit
  `44a2ee3c6580b862fd0a71b9e773984de757dc15`.
- Product baseline: 12 packages, version `0.11.0`, Rust edition 2024; certified
  Codex baseline `codex-cli 0.149.0`; Windows is the only live-certified
  platform.
- Task 143 exact-head CI `33301603658` passed. RAH v0.11.0 remains released.

This assessment read the release/product documents, ADRs 0010–0016, the Task
139 Windows record, Task 140 audit, Task 143 cleanup record, the Desktop Rust
and frontend implementation, `rah-cli` composition, and the commit policy.

## Actual v0.11 product capability

### Core and trusted profile

The RAH implementation has the following bounded capabilities:

| Capability | Current implementation / authority |
| --- | --- |
| `fs.read` | Host-selected workspace read capability. |
| Repository observers | `repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`; fixed native Git and canonical repository. |
| `repo.patch` | Separate exact replacement authority under ADR 0012. |
| `repo.create-file` | Exclusive bounded absent-file creation under ADR 0013. |
| `repo.edit-files` | Bounded multi-file exact replacement under ADR 0014. |
| Stage / unstage | Existing `host.git.stage` and `host.git.unstage`, each narrow index-only ADR 0010 capability. |
| `repo.commit` | ADR 0016 message-only Tool, Execute outer gate, trusted composition, and one fresh host-reviewed in-memory authorization. |
| MCP / Process Plugin | Existing local external Tool providers through ToolRegistry; no installation/update authority. |

The effective trusted-profile composer is `rah_cli::profile_composition::compose`
in `crates/rah-cli/src/profile_composition.rs`. It constructs the commit Tool
and retains its `RepositoryCommitControl`; its `authorize_current_reviewed_snapshot()`
captures the current admitted staged snapshot. That control is deliberately not
a Tool, serializable value, or model-visible capability
(`crates/rah-tools/src/repository_commit.rs`).

### Desktop

Desktop already provides model selection, a bounded llama.cpp endpoint,
certified Codex discovery, native Git discovery, canonical repository choice,
repository-scoped SQLite conversation persistence/resume, and a UI repository
snapshot containing status, worktree diff, and staged diff. Repository
selection increments a host-owned generation and clears the in-memory
conversation context (`crates/rah-desktop/src/main.rs`,
`replace_selected_repository`).

At connection, however, `desktop_tool_registry` builds a private fixed registry:
`echo`; and, only for a selected repository, `fs.read`, the four observers,
`repo.patch`, `repo.create-file`, and `repo.edit-files`. The asserted list in
the Desktop test omits `host.git.stage`, `host.git.unstage`, and `repo.commit`.
Desktop does not depend on `rah-cli`, does not call its composer, has no
equivalent composition retaining `RepositoryCommitControl`, and has no Tauri
command for snapshot authorization, staging, unstaging, or commit result.

Consequently, `TrustedStaticProfile` is not currently the Desktop construction
path. The status UI reports repository-tool activity, and the frontend renders
status/diffs plus chat activity, but it has no per-operation authorization or
post-commit result surface.

## Current end-to-end user workflow

An ordinary Desktop user cannot complete the requested workflow without leaving
RAH Desktop today:

| Arrow | Classification | Evidence and reason |
| --- | --- | --- |
| Select repository | AVAILABLE | `choose_repository` canonicalizes and constructs `DesktopRepository`. |
| Ask model to inspect | AVAILABLE | selected registry exposes `fs.read` and observer Tools through the Codex bridge. |
| Edit/create source | AVAILABLE | selected registry exposes `repo.patch`, `repo.create-file`, `repo.edit-files`. |
| Inspect diff | AVAILABLE | `repository_snapshot` renders worktree and staged diffs; model also has diff Tools. |
| Stage | NOT WIRED | Desktop registry and Tauri commands omit `host.git.stage`. |
| Inspect staged diff | AVAILABLE | `repo.diff-staged` is both an observer Tool and rendered snapshot. |
| Explicit review exact staged snapshot | NOT WIRED | rendered diff is presentation only; no Rust-side reviewed-snapshot command/control. |
| Host authorizes one commit | NOT WIRED | Desktop has no `RepositoryCommitControl`. |
| Codex calls `repo.commit` | NOT WIRED | `repo.commit` is absent from Desktop ToolRegistry. |
| Independent verification | NOT WIRED in Desktop | the core Tool has it, but Desktop cannot invoke it. |
| Result in same conversation | PARTIAL | normal Tool lifecycle/chat presentation exists, but no Desktop commit Tool/result can occur. |

Unstage is also **NOT WIRED**. Repository selection, reconnect, external branch
or HEAD/index change, and resume currently do not create commit authority;
that is safe, but no reviewed-commit UX exists to invalidate.

## Desktop wiring audit

Desktop connects by calling
`CodexRuntime::connect_tool_bridge_with_model_config_and_workspace` with the
fresh private registry and permission set. A selected repository gives the
bridge `None`, `Read`, and `Execute`; no repository gives only `None`. The
repository root is the Codex working directory, otherwise Desktop uses an
app-owned neutral directory. This is a sound host-context boundary but it is
not trusted-profile composition.

Adding a dependency from `rah-desktop` to `rah-cli` would reverse the intended
product direction: a desktop application should not depend on the command-line
application package merely to obtain a reusable host-composition helper.
There is no neutral shared composer today. Directly reproducing the existing
composer in Desktop would duplicate the authority-sensitive wiring. Therefore
a narrowly scoped shared host-composition extraction is justified **only after
a dedicated integration research task proves it can preserve the exact existing
composition contract and dependency-bottom protocol boundary**. It is not
justified as aesthetic refactoring, and v0.12 must omit it if Desktop can safely
reuse the current construction without duplication or undesirable dependency.

## Current product gaps

v0.11 proved a host can perform the final bounded commit through trusted-profile
composition and the Generic Tool Bridge, including Windows live evidence. But
the ordinary Desktop product stops after bounded edits and inspection. It cannot
stage or unstage, cannot use the profile-composed commit control, and cannot
turn a human-reviewed staged diff into one verified commit. The existing
repository snapshot is useful but lacks an explicit decision/action boundary.

This makes the prior v0.11 runner-up—Desktop workflow/task UX—the strongest
v0.12 choice now that repo.commit exists. It is stronger than a generic UX-only
scope because it completes an actual user workflow with no new authority.

## Candidate assessment

Scores are 1–5. Value, workflow completion, frequency, deterministic/live
testability, architecture fit, and discoverability: 5 is better. New authority,
security risk, cross-platform complexity, implementation size, recovery
complexity, operational burden, lock-in, and release risk: 5 is worse.

## Decision matrix

| Candidate | Value | Flow | Freq. | New auth | Risk | Xplat | Test | Live | Fit | Size | Recovery | Discover | Ops | Lock-in | Release | Short assessment |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A Desktop author/review/commit | 5 | 5 | 5 | 1 | 2 | 3 | 4 | 4 | 5 | 3 | 2 | 5 | 2 | 1 | 3 | **Primary: closes the existing path.** |
| B Desktop workflow UX only | 4 | 3 | 5 | 1 | 1 | 2 | 5 | 4 | 5 | 2 | 1 | 5 | 1 | 1 | 2 | Runner-up: valuable, but leaves commit external. |
| C Branch create/switch | 3 | 3 | 3 | 5 | 5 | 5 | 3 | 2 | 3 | 4 | 5 | 4 | 4 | 4 | 5 | New ref/worktree authority. |
| D Delete/rename | 3 | 3 | 3 | 5 | 5 | 5 | 3 | 2 | 3 | 4 | 5 | 3 | 3 | 3 | 5 | Destructive path semantics. |
| E1 Task 120 remote proof | 2 | 1 | 1 | 1 | 2 | 3 | 2 | 1 | 4 | 2 | 2 | 1 | 3 | 1 | 2 | Validation debt, not a product authority. |
| E2 Transport confinement | 3 | 2 | 2 | 5 | 5 | 5 | 2 | 2 | 2 | 5 | 5 | 2 | 5 | 4 | 5 | Separate security programme. |
| F Network MCP / HTTP | 4 | 3 | 3 | 5 | 5 | 5 | 2 | 2 | 2 | 5 | 5 | 3 | 5 | 5 | 5 | Major remote-effect authority. |
| G PluginManager lifecycle | 3 | 2 | 2 | 5 | 5 | 4 | 3 | 2 | 2 | 5 | 4 | 3 | 5 | 5 | 5 | Install/update is not current plugin execution. |
| H Dynamic profile reload | 2 | 1 | 2 | 5 | 5 | 4 | 2 | 2 | 2 | 5 | 5 | 2 | 5 | 4 | 5 | Stale/in-flight authority complexity. |
| I Multi-repository execution | 3 | 3 | 2 | 5 | 5 | 5 | 2 | 2 | 2 | 5 | 5 | 3 | 4 | 5 | 5 | Context and replay confusion. |
| J Codex baseline maintenance | 3 | 2 | 3 | 1 | 2 | 3 | 4 | 3 | 4 | 2 | 2 | 3 | 3 | 2 | 3 | Prerequisite reliability work, not scope leader. |

## Authority matrix

| Candidate | Existing authority sufficient? / owner | Inputs and fixed controls | Effects, replay, persistence | Profile/Desktop/ADR |
| --- | --- | --- | --- | --- |
| A | Yes; Rust Desktop host owns existing repository/tool/commit controls. | Model: existing closed tool inputs, commit message only. Host: repo, Git, identity, snapshot, generation. | Existing bounded worktree/index/history effects; commit one-attempt/no replay; no authority persisted. | Compose existing profile; Desktop presents redacted state; no ADR. |
| B | Yes; Rust Desktop owns presentation only. | UI display/action state; no model authority. | No external effect; durable display state must not become authority. | No profile change; no ADR. |
| C | No; new host ref/worktree authority. | Names and checkout policy must be closed; host fixes repository/Git. | Ref/worktree mutation; uncertain effects cannot replay; namespace changes. | New profile capability, UI, ADR required. |
| D | No; new destructive file authority. | Validated paths and snapshots; host fixes root/collision rules. | Filesystem/index-visible partial effects; no rollback/replay. | New capability/ADR and careful Desktop UX. |
| E1 | Yes; existing endpoint authority. | Existing host-selected endpoint. | Request proof only; no new effect contract. | No profile/ADR necessarily. |
| E2 | No; transport-security authority/contract. | Host fixes effective destination, identity, redirects/proxy policy. | Network effects uncertain. | ADR and likely profile/host changes. |
| F | No; host owns admitted remote endpoint, auth, transport. | Model only admitted tool schemas. | Remote effects; no generic retry/rollback. | New profile and ADR. |
| G | No; host owns catalog/executable/provenance. | No model installer input. | Process/install/update persistence and restart effects. | New ADR/profile/UI lifecycle. |
| H | No; host owns registry generations. | At most explicit host action. | In-flight/stale authorization issues; never restore dynamic authority. | New ADR and persistence rules. |
| I | No; host owns multiple canonical roots. | Model must not freely select authority. | Multiple mutation/context planes; no replay. | New profile and ADR. |
| J | Yes for certification procedure; host owns runtime bundle verification. | Host-fixed version/bundle files. | No model authority. | Improve discovery/error reporting separately; no baseline change here. |

## Deep analysis of top candidates

### 1. Desktop end-to-end repository authoring, review, and commit

**Outcome and missing pieces.** The user obtains a single coherent Desktop
workflow. Missing pieces are stage/unstage exposure, trusted/static composition
or a proven equivalent, retention of the host-only commit control, a staged
review surface bound to a host observation, authorization invocation, commit
Tool registration, and redacted result presentation.

**Authority and security.** No authority delta is needed. ADR 0010 remains
index-only; ADRs 0012–0014 remain distinct worktree authorities; ADR 0016
remains the only history/ref authority. The frontend is presentation/control
only. A frontend action may request Rust to capture a reviewed snapshot, but
Rust must validate selected repository/context, re-observe state, retain the
opaque control, and return only bounded/redacted status. JavaScript cannot
construct, hold, serialize, restore, or replay authorization.

Human review means: (1) host shows selected repository identity and current
branch/HEAD; (2) it obtains staged diff from host observers; (3) it shows the
exact staged scope; (4) the user explicitly confirms; (5) Rust immediately
captures the current ADR 0016 compound snapshot; (6) it arms exactly one
in-memory authorization; (7) the model gets no token/hash/root; (8) one
`repo.commit` consumes it; and (9) the verified taxonomy result is shown.

The time-A/B/C race is safe only if confirmation is followed by capture, not
treated as authority over a stale rendered diff. Recommended semantics: retain
a host-generated displayed snapshot identity; on confirmation Rust re-observes
and compares it, then captures current authorization only if it still matches.
If it differs, fail closed and require a fresh review. ADR 0016 final
revalidation separately protects the period between authorization and spawn.

An explicit **Authorize** action should remain separate from the model's future
Tool call. A model request (“please commit”) is a request, never approval. This
allows a user to review/arm before asking the model to make its message-only
call, but the UI must clearly show that authorization is pending and one-use.
A direct host “Commit” button would be a different product semantic that could
bypass the model Tool lifecycle; it should not be introduced in this scope.
The host may show pending status without exposing its internal binding.

Changing repository, runtime reconnect, repository movement, external branch
or HEAD/index change, model configuration generation, conversation resume, or
application restart must invalidate pending review/authorization and require a
fresh host action. SQLite may retain messages and display history only. It must
not persist commit authorization, executable authority, repository mutation
capability, or dynamic profile state. A transcript can never replay-arm or
consume authorization.

Likely affected areas are `rah-desktop`, a shared composition layer only if
research requires it, and focused deterministic tests—not protocol types or
authority contracts. Windows test plan: selected fixture repository; displayed
staged diff; changed-index refusal; one explicit authorization; one
`committed_verified` result in chat; reconnection/restart invalidation. Linux
and macOS need deterministic tests for portable state behavior; Windows remains
the only live claim until independent live runs exist. Path canonicalization,
symlink/reparse handling, Git executable discovery, Tauri UI automation, and
process supervision differ by platform and must not be overclaimed.

Non-goals: branches/checkout/refs, direct host commit, generic Git/shell,
delete/rename, auto-stage, authorization persistence, profile reload, network
Git, and changes to the Generic Tool Bridge or trusted-profile schema.

### 2. Desktop task/workflow UX without new repository authority

**Outcome.** Better repository context, staged-change state, operation progress,
history, and resumable display-only task state. It is low risk and highly
discoverable, but it does not let the user finish the current local authoring
workflow inside RAH Desktop.

It needs no authority: Rust owns all underlying observations and frontend state
is disposable. A deterministic test plan can cover context changes, redacted
status, resume isolation, and stale presentation. Windows live validation can
exercise UI state but cannot prove any new authority. It should follow A,
because A necessarily supplies the most valuable review/progress surfaces and
then permits UX polish without broadening capability. No ADR, profile schema,
or Tool bridge change is justified.

### 3. Codex certified runtime/baseline reliability

**Outcome.** A user can diagnose an incomplete certified runtime rather than
discovering it only during dynamic-tool validation. Task 139 established that
Windows certified dynamic-tool operation needed `codex.exe` plus same-version
`codex-code-mode-host.exe`; a bare executable is not sufficient for that path.

This is maintenance/product reliability, not a new Tool authority or a baseline
upgrade. A small prerequisite can improve bundle completeness validation and
redacted error reporting, provided it does not alter the `0.149.0` baseline,
install software, or claim other versions. Deterministic fixtures can exercise
missing/mismatched companion detection; Windows live validation can verify the
complete bundle. Linux/macOS applicability is unestablished because the
certification claim is Windows-specific. It is a runner-up supporting task only
if it blocks the Desktop workflow implementation; otherwise retain it as an
operational certification procedure.

## Recommended v0.12 scope

Implement the Desktop workflow in small stages, retaining all current accepted
authority semantics. The recommended product boundary is intentionally not
“approval from the model”: it is an explicit human action at a Rust-owned
reviewed-snapshot boundary followed by a separately model-initiated, one-shot
Tool call.

## Authority / ADR decision

**NO NEW ADR NEEDED FOR PRIMARY v0.12 SCOPE** because it is composition, host
integration, and productization of already accepted authority. Desktop can
reuse ADRs 0010/0012/0013/0014/0016 exactly. Any discovery that requires a
semantic change to their ownership, snapshot binding, model input, replay,
rollback, or persistence rules is a stop condition requiring separate research
and an ADR decision, not an implementation shortcut.

## Security model

`repo.commit` still does **not** imply branch creation, branch switch,
checkout, arbitrary `update-ref`, amend, merge/rebase, tag, remote Git,
credential, retry, or rollback authority. Execute remains an outer dispatch
gate, not generic repository authority. The selected repository and native Git
remain host-fixed; AGENTS.md and model/provider metadata grant nothing.

## Desktop / persistence interaction

The selected canonical repository is bound per runtime generation. Repository
replacement already forces a new conversation context; v0.12 must make the
same event invalidate review state. Persisted records may identify display
context and messages but never retain the authorization object or an equivalent
secret. Pending state after restart is absent, not “pending.”

## Cross-platform assessment

Windows has native Git discovery, Tauri Desktop, reparse-aware repository
validation, and the only live-certified Codex path. Ubuntu/Linux has
deterministic CI evidence but no live claim; Git executable identity, symlinks,
process groups, and UI automation require separate proof. macOS has no live
claim and needs its own path/case/symlink, Git, Tauri, and UI validation.
Portable deterministic tests should cover repository-generation invalidation
and commit-control stale snapshot refusal, while platform-specific tests cover
native discovery and filesystem semantics.

## Known limitations / deferrals

- Task 120 remote llama.cpp proof: `RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`.
- Transport confinement: **NOT CLAIMED**; E1 proof and E2 confinement remain separate.
- Network MCP/Streamable HTTP, PluginManager install/update, profile reload,
  dynamic authority, multi-repository execution, delete/rename, and branch/ref
  authority remain deferred.
- `codex-cli 0.149.0` remains the certified baseline; no upgrade is proposed.

## Proposed Task 145+ sequence

1. **Task 145 — Desktop repository workflow integration research.** Confirm the
   smallest safe composition route, Desktop ownership/lifetime, exact review
   presentation contract, and no dependency-direction regression. No authority change.
2. **Task 146 — Shared host composition foundation (conditional).** Do this only
   if Task 145 proves that direct safe reuse is impossible without duplication
   or Desktop-to-CLI dependency. Preserve existing composition semantics.
3. **Task 147 — Desktop repository index workflow and staged review surface.**
   Wire only existing stage/unstage and host observations; no commit authorization yet.
4. **Task 148 — Rust-side reviewed snapshot authorization UX.** Add redacted
   presentation and a Rust-owned one-shot control using ADR 0016 exactly.
5. **Task 149 — Desktop `repo.commit` integration.** Register the existing Tool
   through the existing bridge; show the verified result in the conversation.
6. **Task 150 — Deterministic Desktop workflow hardening.** Cover stale review,
   generation/reconnect/restart invalidation, external index changes, one-use,
   and no transcript replay.
7. **Task 151 — Windows Desktop live end-to-end validation.** Use a disposable
   local repository and complete certified runtime bundle; make no Linux/macOS
   live claim without their own evidence.
8. **Task 152 — v0.12 milestone audit.** Reconcile accepted ADRs, test/live
   evidence, release claims, dependency graph, and deferrals.

## Non-implementation statement

Task 144 adds this roadmap only. It implements no Rust or frontend code, no
Tool, Desktop command, branch/ref or network authority, trusted-profile or
Generic Tool Bridge change, dependency, version bump, ADR, tag, or release.
