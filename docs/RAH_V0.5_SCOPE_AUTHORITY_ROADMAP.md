# RAH v0.5 scope and authority roadmap

Status: Research complete; no implementation is authorized by this document
Date: 2026-08-22
Scope: product/security roadmap after the released v0.4.0 authority-composition milestone.

## 1. Released baseline

RAH v0.4.0 is the stable baseline for this roadmap:

| Item | Released state |
| --- | --- |
| Release commit | `ebd6358` |
| Post-release documentation | `af54ffb` |
| Tag | `v0.4.0` at `ebd6358` |
| CI and GitHub Release | Green and published |
| Codex compatibility baseline | `codex-cli 0.149.0` |
| Verified release platform | Windows |

The following are established. This roadmap does not reopen or redesign them:

- RAH-owned `AgentRuntime`, `Tool`, `ToolRegistry`, protocol, permissions, and optional Codex adapter boundary.
- Generic Tool Bridge, hardened local stdio MCP adapter, and Process Plugin adapter through the common RAH-owned `Tool` boundary.
- Trusted static capability profile and ADR 0011's atomic, redacted, immutable authority composition.
- `fs.read`; `host.cargo.version`; `host.git.status`; and host-constructed index-only `host.git.stage` / `host.git.unstage`.
- `HostExecutionPolicy`, private `RepositoryMutationPolicy`, explicit runtime permissions, provider lifecycle ownership, and no-replay semantics for uncertain external effects.

ADR 0010 deliberately limits `RepositoryMutationPolicy` to an index-mutation prototype. It does not authorize worktree byte mutation. ADR 0011 composes already approved authority; a profile entry cannot invent a new authority class.

## 2. Product gap after v0.4

The current desktop/local workflow is useful for inspection and bounded host operations:

```text
inspect repository / read files
 -> call safe host capabilities or local providers
 -> inspect Git status
 -> stage or unstage one host-selected existing target
```

It cannot complete the central coding-agent action deliberately and safely:

```text
understand source
 -> modify source content
 -> inspect the resulting diff
 -> let the trusted host decide whether to stage or commit
```

`host.git.stage` does not close this gap: it only changes an existing index entry for a target selected during host construction. Persistent session checkpoints would improve usability, but they cannot create or alter source content. Commit creation would produce a durable checkpoint, but is not a substitute for a bounded editing authority and brings substantially broader history/ref identity and hook authority.

## 3. Authority inventory

| Authority class | Current examples | Existing controls | Boundary statement |
| --- | --- | --- | --- |
| None / pure tool behavior | `echo`; external tools explicitly assigned `PermissionLevel::None` | Tool schema, `ToolRegistry`, explicit external permission assignment | No host filesystem/process authority follows merely from registration. |
| Read | `fs.read` | `PermissionLevel::Read`, `WorkspacePolicy`, UTF-8 and byte limits | Path policy is not OS isolation. |
| Execute | `host.cargo.version`, `host.git.status` | `PermissionLevel::Execute` plus capability-specific `HostExecutionPolicy` | The host fixes executable, argv, cwd, environment, timeout, and output bounds. |
| Repository index mutation | `host.git.stage`, `host.git.unstage` | Execute plus private `RepositoryMutationPolicy` | Host-owned symbolic target; lease; pre/post state proof; no worktree bytes, refs, or history mutation. |
| Profile authority composition | trusted static profile | ADR 0011; strict source/resource validation; fresh registry; immutable effective profile | Selects and configures existing authority only. It is not a grant of a new authority. |

The deferred classes remain separate, even where they happen to use Git or a child process:

- worktree content mutation, including destructive restore;
- Git commit, refs/history/reflogs/object creation;
- network Git and credential-bearing Git;
- generic model-selected executable, argv, cwd, or environment;
- network MCP / Streamable HTTP;
- profile discovery, reload, or dynamic authority changes;
- plugin installation, download, and `PluginManager` lifecycle;
- OS sandboxing, network isolation, and rollback guarantees.

## 4. Candidate evaluation

Ratings are relative to a focused v0.5 release. For risk, complexity, new authority, network dependency, and scope size, **Low** is preferable. The ratings assess the proposed first useful increment, not every imaginable later extension.

| Candidate | Product value | Architectural fit | Security risk | Complexity | Deterministic testability | Windows reliability | New authority | Network / credentials | v0.5 scope size | Decision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A. `host.fs.write` / `host.fs.patch` / `host.git.restore-worktree` as generic worktree mutation | High for coding, but restore itself is narrow recovery value | Medium | High | High | Medium | Medium-Low | High | Low | High | Do not use generic filesystem write or restore as the first product capability. |
| B. `host.git.commit` | Medium-High after edits exist | Medium | Very High | Very High | Medium | Medium-Low | Very High | Medium | Very High | Defer. History authority is independent. |
| C. Repository-aware editing primitive | **High** | **High** | High but bounded | High | **High** for a narrow text subset | Medium | **High** | Low | **Medium** | **Recommend.** |
| D. Persistent session/workflow checkpoints | Medium | High | Low-Medium | Medium | High | High | Low | Low | Medium | Secondary discovery/hardening candidate, not the primary gap closer. |
| E. Network MCP / Streamable HTTP | Medium for integrations; low for local coding completion | Medium | Very High | Very High | Low-Medium | Medium | Very High | High | Very High | Defer. |
| F. Plugin lifecycle / `PluginManager` | Medium | Medium | Very High | Very High | Medium | Medium | Very High | High | Very High | Defer. |
| G. Profile reload / dynamic authority | Low-Medium | Low-Medium | High | High | Medium | Medium | High | Low | High | Defer. |

Candidate C is the smallest direction that lets a local RAH coding workflow produce a deliberate source change. Unlike generic `fs.write`, it can bind the operation to one validated repository, reject Git-internal and unsupported files, require an exact content precondition, report a structured result, and prove that index/history were not intentionally changed. It is still a strong authority and must not be presented as harmless merely because it does not run Git.

### Why the alternatives are not the v0.5 theme

**Restore-worktree is not the best first mutation capability.** It discards known user bytes from a Git source and needs durable recovery and destructive consent, while it does not help an agent author a fix. A bounded constructive edit has a wider content surface, but directly addresses the product gap and can use the same conservative precondition, backup, audit, and no-replay discipline. Restore remains a separate destructive-recovery feature.

**Commit does not belong in this milestone.** Worktree content mutation, index mutation, and history mutation are three different state planes:

```text
worktree mutation != index mutation != history/ref mutation
```

A commit must separately decide message provenance, author/committer identity, templates, editor suppression, hooks, signing, Git configuration, partial or empty commits, amend, ref races, audit, and uncertain effects. It is not a follow-on switch on the editing policy.

**Persistence is valuable but secondary.** `rah-session` currently offers a provider-neutral serializable session model and only `MemorySessionStore`. Durable session persistence would need its own trusted-storage, recovery, and runtime-resume analysis. It does not need stronger repository/process authority, so a narrowly scoped persistence research task is a reasonable secondary v0.5 hardening/product task only after the edit boundary has an accepted design. It must not become a reason to delay the primary coding capability.

**Network MCP and plugin lifecycle compound multiple authorities.** Local process provider trust is not network endpoint trust. Streamable HTTP would need endpoint selection, TLS/server identity, redirects/proxies, credentials, reconnection, streaming limits, and failure/replay rules. A `PluginManager` would additionally need installation, executable provenance, filesystem mutation, and possibly download/network authority. Neither fits a focused v0.5.

**Hot reload changes authority while sessions exist.** It would require generation ownership, fresh all-or-nothing composition, provider drain or termination, and an invariant that existing sessions retain their original registry. The current immutable profile is the correct baseline.

## 5. Strong recommendation

**Recommendation: v0.5 should focus on a repository-aware, guarded text-editing authority for existing repository worktree files.** The initial product capability should be conceptually named `repo.patch` during research, not generic `fs.write` and not `host.git.restore-worktree`.

The capability's purpose is:

```text
trusted host selects one repository and a bounded edit policy
 -> model requests a conditional edit to an existing text file under that root
 -> RAH verifies preconditions and applies one bounded replacement
 -> RAH reports the observed effect without staging, committing, or replaying
```

The name `repo.patch` is a product placeholder, not approval for a public API or tool schema. The ADR/research step must settle terminology. Of the policy names considered, **`RepositoryWorktreeMutationPolicy` is the recommended working name** because it names the exact state plane and distinguishes it from the existing index-only `RepositoryMutationPolicy`. If the final first scope is limited to file bytes and intentionally excludes Git-specific preconditions, `RepositoryContentMutationPolicy` is a viable alternative. Do not call it `WorkspaceMutationPolicy`: that name is too broad and risks conflating a repository-aware authority with the existing read-oriented `WorkspacePolicy`.

This new private, host-owned policy is required. It should be separate from, not a broadening of, `RepositoryMutationPolicy`. It may retain `PermissionLevel::Execute` as the existing outer runtime gate only if the ADR shows that no new public permission is necessary; that coarse permission must not be described as sufficient authorization for editing bytes. The policy is the durable capability-specific authority boundary.

### Recommended initial capability envelope

The first implementation should be intentionally smaller than arbitrary file authoring:

- one host-selected, non-bare repository root and one edit per call;
- one model-supplied **relative** path with normal components only; no absolute, drive/UNC/verbatim/device, ADS, `.` or `..` forms;
- an existing, tracked, stage-0 regular file beneath the canonical repository root; no create, delete, rename, move, chmod/ACL/attribute change, hard link, symlink, reparse point, submodule, gitlink, nested repository, sparse entry, or `.git` content;
- bounded UTF-8 text only, with an explicit BOM and newline policy; reject binary, invalid UTF-8, unsupported encodings, and mixed/ambiguous line-ending cases in the first release;
- a bounded expected preimage digest and length supplied by the model from a prior read, matched immediately before the write; mismatch is a safe refusal;
- a host-defined per-file and per-call byte limit, plus a maximum number of edits; no unbounded payload;
- atomic same-volume temporary-file replacement where Windows semantics permit, with explicit known-failure versus uncertain-result handling for sharing violations, antivirus/indexer races, and replacement failure;
- capture and retain a verified host-private preimage before the first write; no automatic rollback and no model-visible backup path or bytes;
- pre/post repository, target-parent, target identity, target content, index, `HEAD`, and ref observations sufficient to report the intended worktree-only result; no automatic retry or replay after an attempted write; and
- model-visible output restricted to status, symbolic repository/relative target, changed/no-op/uncertain indicators, and bounded redacted reason codes. Audit detail is host-owned.

The target's worktree may already be dirty, but the exact preimage condition must make RAH refuse stale replacement. The initial policy should preserve the index exactly and reject any observed index/`HEAD`/ref change as a policy violation or uncertain result rather than silently attributing it to the edit. Whether a staged target is supported must be an explicit ADR decision; the default recommendation is to support it only when the index is provably unchanged and the host-visible audit makes the three-way state clear.

This is not a promise of cross-process exclusion, perfect TOCTOU prevention, OS sandboxing, network isolation, or rollback. A per-repository RAH lease only serializes RAH's own calls. External editors, Git, antivirus, indexers, and the filesystem can still race the operation.

## 6. Patch versus write primitive

The v0.5 product capability should be a bounded repository edit, but the first payload form determines much of its safety and usability.

| Primitive | Determinism and conflict detection | Windows / encoding behavior | Binary and replay risk | Model ergonomics | v0.5 assessment |
| --- | --- | --- | --- | --- | --- |
| Full-file write | Strong only with mandatory whole-file preimage digest; replacement is unambiguous | Can preserve a supported BOM/newline convention; atomic replacement is practical but can fail on locks | Reject binary; write after spawn/replace is uncertain and never replayed | Simple but large payloads and accidental wholesale rewrites | **Recommended semantic baseline** for the first single-file scope. |
| Structured patch / edit list | Can be strong if every edit has exact, unique preconditions and all offsets are validated against one digest | Requires a carefully specified byte/UTF-8 offset and newline model | Reject binary; multi-edit partial application must be designed | Better for small edits, but more schema and conflict rules | Defer until the full-file conditional write semantics are proven, or define it as an all-or-nothing transformation over the captured text. |
| Unified diff patch | Hunk fuzz, path headers, mode lines, and parser choices complicate exactness | CRLF and encoding behavior varies; parsing must be fully specified | Can include deletes/renames/mode changes unless heavily restricted | Familiar to models and users | Defer. Do not accept ambiguous fuzz or implicit paths. |
| Git-apply-backed patch | Git gives a familiar parser, not a security boundary | Attributes/configuration and index/worktree flags add ambient semantics | May touch index or accept broad patch features; process result is not sufficient proof | Good apparent ergonomics | Reject for the first capability. `git apply` is not automatically safer. |
| Line/range edit | Conflicts only if line identity/context is precisely pinned | Line endings, Unicode normalization, and byte-versus-character indexing must be fixed | Text-only; partial multi-range failure needs a transaction design | Compact once schema is learned | Defer; it should not be the first public editing language. |

The recommended first shape is therefore a **conditional, single-file, whole-text replacement** under the product label `repo.patch`: path, exact preimage digest/length, and bounded replacement text. It is a repository-aware patch capability because the repository policy validates the operation and observes its effect; it is not a generic filesystem write. The later ADR may choose a more ergonomic structured edit form only if it can retain a single captured preimage and all-or-nothing transformation before any write occurs.

## 7. Updated authority boundary

```text
trusted host
    |
    +-- trusted static capability profile (composition only; immutable)
    |       |
    |       +-- existing capability constructors and policies
    |       +-- future RepositoryWorktreeMutationPolicy constructor
    |
    v
ToolRegistry
    |
    +-- None / pure Tool behavior
    +-- Read authority: WorkspacePolicy + PermissionLevel::Read
    +-- Execute authority: HostExecutionPolicy + PermissionLevel::Execute
    +-- Index mutation: RepositoryMutationPolicy + Execute
    +-- v0.5 worktree-content mutation: new policy + outer permission gate
    |
    v
Tool
    |
    v
bounded ToolOutput / host-owned audit
```

The trusted profile remains above this tree as a host-controlled composition mechanism. It may compose the new capability only after the capability and its policy have passed their own hardening and validation. It cannot substitute for the new policy, bypass `ToolRegistry`, or turn a model request into consent.

Generic `shell.exec`, `process.exec`, model-selected executable/argv/cwd/env, and generic arbitrary `fs.write` remain explicitly prohibited as a solution to coding workflows. Bounded semantic capabilities are the preferred design.

## 8. ADR and crate placement

### ADR recommendation

**A new ADR is required before implementation.** ADR 0010 is intentionally index-only; widening it to worktree byte mutation would silently alter its accepted security contract. ADR 0011 explicitly says profile composition does not create an authority class. The new ADR should receive the next available number and define, at minimum:

1. the worktree-content authority as distinct from Execute, index mutation, restore, and history/ref mutation;
2. host selection of repository and the allowed path domain;
3. path, canonical identity, symlink/reparse, hard-link, and case/alias rules;
4. the text encoding, BOM, newline, size, create/overwrite/delete, and replacement semantics;
5. exact precondition, lease, audit, preimage backup, retention, and compare-before-manual-recovery rules;
6. atomic replacement, cancellation, timeout, crash, partial, uncertain, and no-replay semantics;
7. index/HEAD/ref and ignored/untracked handling; and
8. Windows-specific guarantees and non-guarantees.

No ADR status changes occur in this research task.

### Crate placement

The recommended placement is **private capability-specific policy and tool code in `rah-tools`**. That crate already owns the private index-mutation policy and the relevant tool-level repository state semantics. `rah-sandbox` should remain the owner of reusable process/workspace primitives, but it should not become a repository-edit authority or learn Git state. `rah-core` should not receive a repository-specific policy, and a new crate is not justified for the first bounded capability.

Expected dependency direction remains unchanged:

```text
rah-tools -> rah-sandbox / rah-protocol
rah-protocol -> no RAH crates
```

There is no Codex dependency edge and no provider-specific type at this boundary.

## 9. Required implementation sequence

Trusted-profile schema or composition must be last, not first:

```text
Task 047 authority research and ADR draft
 -> accept ADR before code
 -> private repo.patch policy/tool implementation in rah-tools
 -> deterministic adversarial hardening and conformance tests
 -> opt-in live validation in an isolated temporary Git repository
 -> trusted-profile constructor/composition and redacted inventory support
 -> profile-composition regression and release gate
```

The implementation plan must retain the released profile version and Process Plugin protocol `1` unless the independently accepted design proves a format change necessary. It must retain `codex-cli 0.149.0`; upgrading Codex is a separate compatibility task, not v0.5 scope.

## 10. Future deterministic verification

The normal suite must use owned temporary directories/repositories and require no network, credentials, real model, GPU, or live Codex process. It should prove at least:

- repository-root and target workspace confinement, relative-only path rules, traversal refusal, and `.git` refusal;
- normal supported text replacement, explicit no-op, overwrite rules, and configured file/operation size bounds;
- precondition mismatch, target identity/parent change, and external concurrent content modification refusal;
- symlink, Windows junction/reparse point, hard-link, case/short-name alias, UNC/verbatim/device/ADS, nested-repository, sparse, submodule, gitlink, and unsupported file-type rejection where representable on the host;
- UTF-8, BOM, CRLF/LF, mixed-line-ending, binary, and unsupported encoding behavior;
- temporary-file cleanup, replacement failure, locked target, sharing violation, and post-write verification behavior;
- cancellation, timeout, disconnect, crash, lost result, partial failure, and the rule that an attempted mutation is never automatically replayed;
- verified private preimage capture, redacted audit/result output, bounded retention metadata, no automatic rollback, and compare-before-recovery refusal;
- index, `HEAD`, refs, reflogs, and unrelated supported worktree files remain unchanged unless a separately authorized future capability says otherwise;
- ToolRegistry permission denial and profile-composition failure leave no registered partial authority; and
- deterministic proof that no production call reaches generic shell/process execution or model-selected launch parameters.

Tests must distinguish path checks from OS isolation and should platform-gate only an assertion that the host genuinely cannot represent, never the default-deny behavior.

## 11. Future opt-in live validation

Live validation is not part of this roadmap task. A future release gate needs a deliberately isolated temporary repository and a trusted local setup proving:

```text
Codex 0.149.0
 -> RAH Generic Tool Bridge
 -> bounded repo.patch Tool
 -> isolated temporary Git repository
 -> exactly one expected existing file changes once
 -> index, HEAD, refs, and unrelated files remain unchanged
 -> Completed
```

The live run must capture redacted lifecycle evidence, exactly one tool request and one attempted edit, pre/post content hashes recorded host-side, no replay, and cleanup/reaping of the Codex process. A second opt-in run should exercise a locked target or stale precondition and prove refusal without an edit. It does not prove OS sandboxing, network isolation, complete TOCTOU prevention, or that all external filesystem races are impossible.

## 12. Windows-first requirements

Windows remains the primary release baseline. The ADR and implementation must not infer Unix semantics. Required evidence includes NTFS case-insensitivity and aliases, drive-rooted path rules, rejection or precise support of UNC/verbatim/device/ADS paths, junctions and all reparse points, parent/target identity, sharing modes and locked-file behavior, antivirus/indexer races, same-volume atomic rename/replace behavior, read-only/ACL preservation or explicit refusal, and CRLF/LF/BOM behavior. Git executable use, if any later feature needs it, must remain direct native executable invocation; v0.5 editing itself should not need native Git to apply content.

Unix tests are valuable but are not evidence of Windows behavior. The existing Windows-verified v0.4 release does not imply a cross-platform worktree-mutation guarantee.

## 13. Technical-debt audit

### Release-critical for the recommended capability

| Debt / evidence gap | Why it blocks v0.5 editing | Required disposition |
| --- | --- | --- |
| ADR 0010 is index-only | It cannot govern byte replacement or recovery semantics | New ADR before code. |
| `WorkspacePolicy` is read-oriented path validation | It permits unresolved-write path resolution and does not provide worktree mutation identity, text, backup, or result semantics | Reuse only low-level lessons; do not relabel it as edit authorization. |
| Existing mutation snapshot fixture is bounded and test-oriented | It is not a scalable proof or durable recovery design for arbitrary source files | Design targeted pre/post evidence and explicit supported limits. |
| Windows identity/reparse/lock/atomic-replace evidence is absent for content mutation | A successful write is not a supported Windows security/behavior contract | Deterministic Windows fixture matrix plus opt-in live evidence. |
| Profile composition currently admits only existing hardened capability constructors | Adding schema first would make an unproven authority compose-able | Harden capability before profile support. |

### Non-blocking cleanup

| Debt | v0.5 treatment |
| --- | --- |
| CLI-owned effective composer placement | Keep private and stable during the capability work; reassess only if an accepted implementation plan needs a narrow seam. |
| Test fixture/lifecycle complexity and Windows process-lock flakes | Preserve the v0.4 provider lifecycle tests; fix only concrete flakes that block the new deterministic matrix. |
| Profile source TOCTOU residual race and Unix live-validation gaps | Keep documented limitations. They are not a reason to broaden the v0.5 scope or claim stronger isolation. |
| Persistent `SessionStore` | Research separately after the authority ADR; do not couple durable sessions to repository mutation implementation. |

## 14. Explicit v0.5 deferrals

The recommended v0.5 scope does **not** authorize:

- `host.git.restore-worktree`, reset, clean, checkout/switch/stash, deletion, rename/move, arbitrary filesystem mutation, or any automatic recovery;
- Git commit, amend, refs, history, reflogs, object-database, merge/rebase, remote/network Git, credentials, hooks, signing, templates, editors, or identity management;
- generic `shell.exec`, generic `process.exec`, or model-selected executable, argv, cwd, environment, or timeout;
- Network MCP, MCP Streamable HTTP, endpoint trust, TLS/credential policy, or network isolation;
- profile auto-discovery, hot reload, dynamic active-session authority changes, `PluginManager`, plugin installation/download, or generic plugin lifecycle;
- OS sandboxing claims, network-isolation claims, cross-process locking claims, rollback guarantees, or automatic replay; and
- Codex upgrade, profile-version upgrade, Process Plugin protocol upgrade, protocol DTO changes, or provider-specific public API changes unless an independently approved later task proves them necessary.

## 15. Recommended next task

**Task 047: Repository worktree content-mutation authority research and ADR proposal.** It should validate the recommended policy name and first text-edit envelope against the current `rah-tools` and `rah-sandbox` seams, draft the new ADR without changing its status, and produce a bounded implementation/test plan. It must not implement a tool, modify a profile schema, add a permission, or extend `RepositoryMutationPolicy`.

## Evidence boundary

This roadmap is based on the released v0.4 security/release documents, ADRs 0010 and 0011, the v0.3 index/worktree mutation research, and current local source inspection of `rah-tools`, `rah-sandbox`, and `rah-session`. It makes no claim that `repo.patch`, `RepositoryWorktreeMutationPolicy`, persistent session storage, network MCP, or any new ADR has been implemented or accepted.
