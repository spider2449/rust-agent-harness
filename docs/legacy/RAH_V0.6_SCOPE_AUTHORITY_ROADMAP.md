# RAH v0.6 scope and authority roadmap

Status: Research complete; no implementation is authorized by this document
Date: 2026-08-23
Scope: product and authority decision after the released RAH v0.5.1 baseline.

## 1. Decision

**Recommend v0.6 deliver a bounded, repository-aware read-only workflow inspection toolkit.** Its first coherent capability set is:

- repo.status: normalized, bounded repository status;
- repo.diff: normalized, bounded unstaged worktree-versus-index diff;
- repo.diff-staged: normalized, bounded staged-index-versus-HEAD diff; and
- repo.file-info: bounded facts for one logical path, including tracked/index state and the raw byte length/SHA-256 needed to make a fresh repo.patch request.

This is one product capability, not a collection of unrelated diagnostics. It lets an agent observe the repository state required to plan, verify, and hand off a bounded change through the already released repo.patch and index tools. It closes the most immediate reliability gap without creating a new mutation, history, network, persistence, or lifecycle authority.

The v0.6 authority model should reuse existing host-owned repository identity checks, HostExecutionPolicy, PermissionLevel::Execute, trusted-profile composition, ToolRegistry, and Generic Tool Bridge. Each Git invocation must be host-fixed; model input must not select executable, argv, cwd, repository, revision, pathspec, config, output mode, or environment. repo.file-info may accept one narrow logical relative path only after a repository policy validates it. The profile remains composition only.

**No new ADR is required for this scope**, provided Task 058 confirms the tools are strictly read-only and retain the existing fixed-command execution boundary. A private shared observation helper is permitted as an implementation detail, but must not become a new generic authority class. Task 058 must stop and propose an ADR if required behavior would broaden Execute, accept model-selected Git arguments, grant a write, change active-session authority semantics, or expose a new public architecture boundary.

The next task should be **Task 058 — repository workflow inspection contract research**. It is a narrow research/contract task, not an implementation task and not an ADR by default.

## 2. Baseline and constraints

This roadmap begins at the fully required-CI verified **RAH v0.5.1** baseline, not at the historical v0.5.0 release:

| Item | Established state |
| --- | --- |
| Current release | v0.5.1, published |
| Release-preparation commit | 0ea648d84d6f48720c33e8b1bb07e1c24101c870 |
| v0.5.1 recovery | 6aae5b1fb710cb9d84fc1bcc51bddb9d1be9e22e; portability-only cfg import fix |
| v0.5.0 immutable tag target | b1f0fb4a903a59e0b5c23ca107d7508ebcbd8786 |
| Current post-release cleanup | 7c66d51b0c6635ede396da17942c9ef4530596dd |
| Exact Codex baseline | codex-cli 0.149.0 |
| Live validation evidence | Windows; no Unix live Codex claim |

The following remain accepted architecture, rather than starting points for a redesign:

- RAH owns AgentRuntime, Tool, ToolRegistry, protocol, and permission boundaries; Codex is an optional adapter.
- MCP and Process Plugin providers enter through ordinary RAH Tool values.
- The Generic Tool Bridge is provider-neutral. Provider metadata and model requests never grant authority.
- Trusted profiles are host-owned, immutable authority composition. They do not invent authorities or allow partial admission.
- Missing explicit external permission fails closed.
- Repository worktree content, Git index, Git history/ref, and network Git are distinct state/authority planes.

The released repo.patch authority is exactly one literal replacement in one existing HEAD-tracked, unstaged, normal-stage-0, bounded strict-UTF-8 file. It has raw SHA-256 and byte-length preconditions, preserves BOM/newlines, makes one replacement attempt, never retries or replays uncertain effects, offers no rollback, and is constructed by a private host-owned RepositoryWorktreeMutationPolicy. It is not generic file-write authority.

## 3. Current workflow and product gap

The released components permit this mediated path:

~~~text
Codex/model request
  -> Generic Tool Bridge
  -> ToolRegistry
  -> permission and host policy
  -> RAH Tool
  -> bounded ToolOutput / host audit
~~~

| Workflow need | Current capability | Limitation |
| --- | --- | --- |
| Read several bounded text files | fs.read | Caller must already know paths; it does not describe Git/index state or produce a patch precondition. |
| Inspect basic repository state | host.git.status | Fixed raw porcelain output only; no bounded diff or stable file-state/digest observer. |
| Run approved host operations | Capability-specific tools, such as host.cargo.version | No generic process authority is exposed; a host may add only separately approved fixed capabilities. |
| Use extensions | Local stdio MCP and Process Plugin RAH tools | Metadata cannot create permissions; installed/provider lifecycle is intentionally limited. |
| Stage or unstage | Host-selected host.git.stage / host.git.unstage | Index mutation is target-specific and separate from content and commit authority. |
| Make one source edit | repo.patch | One eligible file and one exact replacement per call; no transaction, creation, rename, or diff result. |
| Supply repo.patch through Codex | Profile -> ToolRegistry -> Generic Tool Bridge -> real Codex | Verified live only on Windows with the exact pinned Codex baseline. |

An agent can inspect multiple source files and may make several sequential repo.patch calls, but it cannot obtain a reliable repository-native answer to what changed, what is staged, which file is eligible, whether a previous patch produced only the intended diff, or which fresh digest/length to use after a refusal. It also cannot create a new file, atomically coordinate a multi-file change, or create a commit.

~~~text
read relevant files
  -> plan a bounded edit
  -> repo.patch existing eligible file(s)
  -> [missing: bounded repository-native verification]
  -> host-approved test capability, if separately configured
  -> host.git.stage selected target(s)
  -> [missing: history authority]
~~~

v0.6 should fix the verification gap before widening destructive authority. It makes the existing patch capability more dependable, supports human review, and gives later mutation research a deterministic observation contract rather than raw-shell conventions.

## 4. Evaluation method

Every matrix score is 1 through 5, where **5 is more favorable for a focused v0.6 release**. Thus authority containment 5 means existing authority can be used unchanged; security, filesystem/Git, rollback, and dependency scores of 5 mean lower risk/complexity. The weighted score is a normalized 0–100 suitability score, not an implementation estimate.

| Criterion | Weight | Meaning of 5 |
| --- | ---: | --- |
| User/product value | 15 | Solves an important current user problem. |
| End-to-end coding workflow unlock | 10 | Makes a safe coding loop materially more complete. |
| Authority containment | 10 | Uses an accepted authority without silently stretching it. |
| Security simplicity | 7 | Has a small, reviewable abuse surface. |
| Filesystem/Git safety | 6 | Has low destructive or ambient Git risk. |
| Deterministic testability | 8 | Owned fixtures can prove behavior without a live model/network. |
| Live-test feasibility | 5 | Can be tested with bounded local fixtures. |
| Windows complexity | 4 | Is practical on the release baseline. |
| Cross-platform complexity | 4 | Has a small portability delta. |
| Trusted-profile fit | 4 | Composes cleanly as host authority. |
| Generic Tool Bridge fit | 4 | Uses ordinary provider-neutral tools. |
| Architectural fit | 6 | Preserves RAH-owned boundaries. |
| New public surface | 3 | Needs little new durable API/schema surface. |
| Rollback/uncertainty simplicity | 4 | Does not introduce destructive uncertain effects. |
| Runtime/dependency simplicity | 3 | Needs no network service or heavy runtime. |
| Implementation size | 5 | Is small enough for one coherent milestone. |
| Release risk | 2 | Has a bounded validation/release blast radius. |

Abbreviations below are PV product value, E2E workflow unlock, Auth authority containment, Sec security simplicity, FS filesystem/Git safety, Det deterministic testability, Live live feasibility, Win, Xplat, Profile, Bridge, Fit, Surface, Uncertain, Runtime, Size, and Release. Scores assess the first useful increment, not an unbounded future version.

| Candidate | PV | E2E | Auth | Sec | FS | Det | Live | Win | Xplat | Profile | Bridge | Fit | Surface | Uncertain | Runtime | Size | Release | Score | Result |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Read-only repository workflow inspection | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 5 | 4 | 5 | 5 | 5 | 4 | 5 | 5 | 4 | 4 | **92** | **Recommend** |
| Multiple exact replacements in one existing file | 4 | 3 | 3 | 3 | 3 | 5 | 4 | 4 | 4 | 4 | 5 | 4 | 4 | 2 | 5 | 4 | 3 | 75 | Later ADR 0012 extension research |
| Bounded multi-file patch transaction | 5 | 4 | 2 | 2 | 2 | 3 | 3 | 2 | 2 | 3 | 5 | 3 | 2 | 1 | 4 | 2 | 2 | 57 | Defer |
| Structured unified-diff/hunk primitive | 4 | 4 | 2 | 1 | 2 | 2 | 3 | 3 | 3 | 3 | 5 | 3 | 2 | 1 | 5 | 2 | 2 | 51 | Defer |
| One bounded new-file creation | 4 | 3 | 2 | 2 | 2 | 4 | 4 | 3 | 3 | 3 | 5 | 3 | 3 | 2 | 5 | 3 | 3 | 62 | Separate authority research |
| File deletion/rename | 3 | 2 | 1 | 1 | 1 | 3 | 3 | 2 | 2 | 2 | 5 | 2 | 2 | 1 | 5 | 2 | 2 | 40 | Defer |
| repo.commit-staged | 4 | 3 | 1 | 1 | 1 | 3 | 2 | 2 | 2 | 2 | 5 | 2 | 2 | 1 | 4 | 2 | 1 | 37 | Separate history ADR; defer |
| Durable session/workflow persistence | 3 | 3 | 3 | 3 | 5 | 4 | 4 | 5 | 5 | 3 | 4 | 4 | 2 | 3 | 3 | 3 | 3 | 69 | Secondary research |
| MCP Streamable HTTP/network MCP | 3 | 2 | 1 | 1 | 4 | 2 | 2 | 3 | 3 | 1 | 4 | 2 | 1 | 2 | 1 | 1 | 1 | 29 | Defer |
| PluginManager/install/lifecycle | 3 | 2 | 1 | 1 | 3 | 2 | 2 | 3 | 3 | 1 | 4 | 2 | 1 | 2 | 1 | 1 | 1 | 26 | Defer |
| Dynamic profile reload | 2 | 2 | 1 | 2 | 4 | 3 | 3 | 3 | 3 | 1 | 4 | 2 | 1 | 2 | 4 | 2 | 2 | 33 | Defer |
| Unix live validation/hardening only | 2 | 2 | 5 | 5 | 5 | 4 | 4 | 3 | 4 | 5 | 5 | 5 | 5 | 5 | 5 | 4 | 4 | 75 | Fold into milestone |
| Fixed host-owned test runner | 4 | 4 | 3 | 2 | 2 | 4 | 4 | 4 | 4 | 4 | 5 | 4 | 3 | 2 | 4 | 3 | 3 | 65 | Separate Execute research |

The recommendation wins because it is both high user value and the only candidate with no destructive-effect uncertainty. Read-only Git is not free: an external Git executable still needs the existing host-fixed executable, canonical repository, sanitized environment, bounded output, timeout, and lifecycle policy. The risk is already represented by ADR 0009 and the released host.git.status pattern; it does not justify generic process execution.

## 5. Authority classification

| Candidate | Classification | Boundary decision |
| --- | --- | --- |
| Repository workflow inspection | **Uses existing authority unchanged** | Fixed read-only Git observation under HostExecutionPolicy and existing repository identity checks; repo.file-info uses a narrow logical path observer. No new authority class. |
| Multiple exact replacements, one existing file | **Narrow extension within ADR 0012 only after research** | It remains worktree content mutation, but changes one-call semantics, audit, and uncertainty. It is not automatically authorized by the policy name. |
| Multi-file transaction / diff hunks | **New authority class or ADR amendment of equivalent force** | Atomicity and coordinated effects across paths differ materially from ADR 0012's one-target commitment. |
| repo.create | **New authority class / ADR** | Persistent creation changes the target population and produces untracked Git state; ADR 0012 expressly excludes it. |
| Delete / rename / move | **New authority class / ADR** | Name removal/relocation, metadata, recovery, and Git path state are not a literal replacement. |
| Commit/history/ref | **New authority class / ADR** | History mutation is explicitly separate from index and worktree content mutation. |
| Session persistence | **New persistence/storage design decision if implemented** | It may not persist live authority or secrets without revalidation; it reuses no repository mutation authority. |
| Network MCP | **New network/provider authority class / ADR** | Local stdio assumptions cannot authorize endpoints, TLS, credentials, redirects, or replay. |
| Plugin lifecycle of trusted local executables | **Potential narrow lifecycle extension; install/download needs new supply-chain authority** | Managing configured local processes differs from acquiring executable code. |
| Dynamic profile reload | **New authority-lifecycle ADR** | Session snapshots, revocation, provider draining, and audit generations change the immutable-profile decision. |
| Unix validation | **No authority change** | It is validation scope only. |
| Fixed test runner | **Existing Execute outer gate, but new capability-specific policy research** | It must never become model-selected process.exec; repository code has ambient authority. |

## 6. Recommended v0.6 product scope

### Exact capability

The v0.6 milestone should expose a small **repository workflow inspection toolkit** for one host-selected non-bare repository. The output is for model and human workflow, while raw host diagnostics remain bounded and redacted. The first contract contains no mutation operation and no model-selected command parameters.

| Proposed observer | Fixed host operation | Model input | Bounded result purpose |
| --- | --- | --- | --- |
| repo.status | Host-pinned, sanitized status observation | {} | Machine-readable changed/staged/untracked state; no raw absolute paths. |
| repo.diff | Host-pinned unstaged comparison; external diff/text conversion disabled | {} | Review the worktree effect of repo.patch before staging. |
| repo.diff-staged | Host-pinned staged comparison | {} | Review exactly what an existing index authority staged. |
| repo.file-info | Host-owned repository/path state observation and bounded raw-byte digest | One logical relative path | State needed to decide eligibility and form a new repo.patch precondition; no mutation and no traversal. |

Task 058 must choose final names only after reviewing stable RAH naming and schema conventions. It should not silently replace released host.git.status; compatibility may require it to remain supported while the new observer is introduced. repo.patch-plan is deferred: producing a model action payload would blur an observer into an edit planner and is unnecessary for the first reliability increment.

### Why this is the highest value now

repo.patch already proves a deliberate edit path, but it asks models to infer repository state from file reads, raw porcelain, and external knowledge. The inspection toolkit makes the precondition and verification loop first-class without allowing RAH to author broader content. It also benefits MCP and Process Plugin tools because they consume the same RAH-native repository observations through ToolRegistry, rather than asking a provider to become an authority source.

~~~text
inspection does not create files
inspection does not edit more files atomically
inspection does not run arbitrary tests
inspection does not stage automatically
inspection does not commit
~~~

The profile integration is additive only after the observers prove deterministic contracts. A future profile entry must bind symbolic host-selected Git/repository resources and fixed limits; it must not expose raw argv, revision, pathspec, environment, timeout, output limits, or a model-selected repository. Existing effective composition remains atomic and the profile schema is not changed by this research task.

### Expected validation

- **Deterministic:** owned temporary Git repositories covering clean, unstaged, staged, untracked, rename-like, conflict/unmerged, sparse/skip-worktree, NUL, binary, large-output, malformed path, repository identity, hostile Git config, output truncation, timeout, cancellation, and no-write assertions. Diff command construction must explicitly avoid external diff/textconv execution.
- **Bridge:** captured app-server interactions prove generic tool-definition translation, canonical aliasing, permission denial before tool execution, bounded outputs, malformed/replayed call handling, and no Codex-owned capability enablement.
- **Windows live:** with exactly codex-cli 0.149.0, run a fresh local repository fixture through trusted effective composition and the real bridge; observe status/diff/file-info and prove observers do not mutate index, HEAD, refs, or worktree.
- **Unix:** direct native-tool live validation is required before claiming cross-platform support. A Unix live Codex bridge is a separate claim: run it only after the exact executable/schema baseline is verified on that host. The v0.5.1 Windows-only claim is not retroactively broadened.

## 7. Repository patch evolution: deliberately deferred

The product need for more expressive edits is real, but it is not the most valuable v0.6 increment. All variants below retain the current state-plane separation and no-replay rule.

| Form | Preconditions and ordering | Atomicity / partial effect | Windows and index implications | Decision |
| --- | --- | --- | --- | --- |
| 1. Multiple conditional replacements in one existing file | One raw preimage digest/length; every old text exact and uniquely found against the same captured text; reject overlapping edits; deterministic request-array order; construct the full postimage before one replacement. | One temp postimage and one target replacement give one-file all-or-nothing-attempt semantics; after the commit point classify success/known failure/uncertain as ADR 0012 requires. | Same-parent temp and native replacement rules remain. Revalidate target, repository, index, HEAD, and refs before/after. | Best future editing extension, but research ADR 0012 semantics first. |
| 2. Bounded multi-file repository patch | Per-file identity/preimage plus a transaction-wide ordered manifest digest. Sort/canonicalize paths; every file revalidates before the first commit. | Normal filesystem rename has no portable all-files atomic commit. Sequential replacement produces partial success; rollback would need new destructive authority. | Windows locks can fail after earlier replacements. Index preservation must be checked across all targets; external changes make the result uncertain. | Defer. |
| 3. Structured unified diff/hunk primitive | Prohibit fuzzy context, implicit paths, mode changes, rename/delete headers, binary patches, conversion, and unspecified offset/Unicode rules. Per-file digest alone does not settle hunk matching. | It can construct postimages before attempts, but inherits multi-file partial effects; parser ambiguity enlarges audit/output. | Disable external conversions and establish exact CRLF/BOM/attributes rules. Git syntax is not a security boundary. | Defer; do not use git apply as an authority shortcut. |

Option 1 is the likely post-v0.6 mutation research direction because it keeps one target and one replacement commit point. Even it should not be implemented on an assumption that ADR 0012 already authorizes arbitrary multi-edit grammar. Task 058 is observation research first; later work can ask whether evidence supports a narrow ADR 0012 extension.

## 8. Repository file creation

A narrow repo.create would have tangible value for new tests and source modules, but is not a safe corollary of repo.patch:

~~~text
one repository-relative normal path
  -> path must not exist
  -> no .git / no link or reparse traversal
  -> bounded strict UTF-8 bytes
  -> exclusive creation
  -> post-observed untracked Git state
~~~

Even this shape needs a new authority decision. It persists a model-selected pathname rather than replacing an existing repository-owned target, its cleanup after an uncertain exclusive-create outcome can itself be destructive removal, and it creates an untracked file. ADR 0012 requires HEAD-tracked targets specifically to avoid this state population.

The interaction is material: after repo.create, an agent could not use released repo.patch to correct its new file because repo.patch rejects untracked targets. Solving that needs a second rule for recently created paths, durable provenance, or more general untracked edit authority; none should be smuggled into creation. This makes repo.create less valuable than inspection in v0.6 despite its apparent coding-agent ergonomics.

## 9. Commit/history authority

The narrowest plausible future concept is repo.commit-staged, with all of these restrictions:

- commit exactly the current staged index; no automatic staging;
- no amend, reset, merge, rebase, checkout, branch/ref selection, or network;
- host-owned fixed identity policy, explicit no-signing default, controlled hooks/editor/template/configuration behavior, and bounded commit message;
- clean/stable HEAD and ref preconditions, explicit empty-commit policy, and pre/post repository observations; and
- one invocation with post-observation and no automatic replay after an uncertain effect.

This is still Git history/object/ref mutation. It cannot reuse ADR 0010's index authority or ADR 0012's worktree authority. Identity, commit-message provenance, hooks, signing, templates, editor suppression, author/committer timestamps, ref races, and user configuration all demand a **new history authority ADR**. It is valuable only after a host has safely inspected, edited, tested, and deliberately staged a coherent change, so it is deferred.

## 10. Other candidates

### Session and workflow persistence

rah-session currently has provider-neutral serializable state and a process-local MemorySessionStore. Durable persistence could provide task resumption, audit continuity, and checkpoints, but it must decide where authority snapshots live, how profile changes/revocation are revalidated after crash, how secrets and sensitive tool output are excluded or protected, how schema versions migrate, and how a resumed Codex/provider lifecycle is reconciled. It is valuable, but premature while the product has no stable repository-inspection contract and no explicit persistence trust boundary. Keep it as secondary research rather than combine storage with v0.6 tools.

### Network MCP / Streamable HTTP

MCP Streamable HTTP is not an alternate transport toggle for the local stdio adapter. It needs host-owned endpoint selection, TLS/server identity, authentication and credential handling, redirects/proxies, DNS/rebinding considerations, request/response size limits, discovery invalidation, timeout/cancellation, reconnect and replay semantics, and profile representation. It is a network/provider authority problem requiring a new ADR; defer it.

### PluginManager and installation

Managing explicit already-trusted local plugin executables (enable/disable, health, compatibility, controlled restart) is distinct from downloading or installing plugins. The latter adds network, filesystem write, executable provenance, signature/update, rollback, and supply-chain authority. Neither is needed to make the current coding loop reliable. Keep lifecycle research separate and defer install/update flows.

### Dynamic profile reload

Host-side reload is an authority-lifecycle feature, not configuration parsing. It needs profile generations, atomic replacement, session registry snapshot semantics, in-flight call ownership, provider drain/termination, revocation behavior, and auditable authority-transition records. ADR 0011 specifies immutable effective profiles; reload therefore needs a new ADR and remains deferred.

### Cross-platform hardening

Unix direct/live validation has high engineering value: it can exercise Git observation and the existing native repo.patch replacement branch beyond Windows. It does not itself add a user-visible product capability. Fold it into the recommended milestone's release evidence rather than label it v0.6's sole feature.

### Fixed test runner

A future host.cargo.test-like capability could improve the coding loop only when a trusted host pins a canonical executable, exact arguments, cwd, environment, limits, cancellation, and output policy. It still executes repository-controlled build/test code with ambient host authority. It must be researched as a capability-specific Execute policy, never generalized into shell.exec, process.exec, arbitrary argv, or model-chosen cwd/environment. It is not bundled with repository observation.

## 11. Technical-debt classification

| Follow-up from v0.5 | Classification | Rationale |
| --- | --- | --- |
| Explicit sparse/skip-worktree fixtures | **Block the repo.file-info eligibility contract; fold into v0.6** | The observer must accurately expose or refuse special index states before a model relies on it to form repo.patch preconditions. |
| Raw-NUL fixture | **Independent maintenance; complete before a future mutation extension** | The current guard is source-inspected. Read-only diff/file-info must define binary/NUL output conservatively, but this does not block status/diff implementation if they never represent NUL as text. |
| Request-BOM fixture | **Independent maintenance; complete before a future mutation extension** | It hardens repo.patch input parsing, not the proposed read-only observers. |
| External filesystem filter / TOCTOU limitations | **Fold relevant command hardening into v0.6; residual limitation remains** | Disable external diff/textconv and validate identity/output bounds. Do not claim observation or path checks eliminate external races. |
| No Unix live repo.patch validation | **Fold into v0.6 milestone** | Direct Unix live tooling and repo.patch evidence close the main platform-evidence gap; do not make an unverified Codex bridge claim. |

## 12. Explicit deferrals and non-goals

The following remain deferred regardless of the inspection recommendation:

- generic shell.exec, generic process.exec, arbitrary model-selected executable/argv/cwd/environment, and arbitrary fs.write;
- unrestricted worktree mutation, model-selected temporary/backup paths, automatic recovery/rollback, and automatic replay of uncertain effects;
- untracked/new-file edits, deletion, rename/move, binary edits, multi-file transaction, unified diff/hunk fuzz, and implicit Git conversion/filter semantics;
- arbitrary Git refs/history manipulation, Git network operations, credentials, signing, hooks, or identity selected by a model;
- network MCP, provider download/installation, plugin marketplace, and credential-bearing provider configuration;
- profile auto-discovery/hot reload and active-session authority mutation; and
- claims of OS sandboxing, network isolation, cross-process exclusivity, or perfect TOCTOU prevention without an OS-enforced boundary.

## 13. Suggested sequencing

~~~text
Task 058: repository workflow inspection contract research
  -> decide exact fixed Git invocations, result schemas, redaction, binary and conversion behavior, and confirm existing-authority reuse
  -> create an ADR only if that research finds a material boundary change
  -> narrow private implementation in rah-tools
  -> deterministic hostile-Git/repository fixture matrix
  -> trusted-profile closed composition and redacted inventory integration
  -> deterministic Generic Tool Bridge verification
  -> Windows live bridge validation at codex-cli 0.149.0
  -> Unix direct live validation; Unix Codex live validation only if separately verified on the target host
  -> milestone audit and release gate
~~~

Profile/bridge work follows deterministic tool hardening. It must not be used to make an unfinished observer look authorized, and no v0.6 task should begin multi-edit, creation, commit, network MCP, PluginManager, persistence, or dynamic reload implementation.

## 14. ADR, dependency, and API impact

This Task 057 changes documentation only.

- **ADR impact now:** none. ADRs 0010, 0011, and 0012 remain accepted and unchanged.
- **Expected ADR impact for v0.6:** none if Task 058 proves strict reuse of the existing fixed-command observer pattern. Stop and create the necessary ADR before code if that premise fails.
- **Expected dependency direction:** unchanged. A future implementation belongs in rah-tools using existing rah-tools -> rah-sandbox / rah-protocol direction; no Codex crate dependency, provider SDK type, network client, or new crate is justified by this scope.
