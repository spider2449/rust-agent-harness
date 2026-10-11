# RAH v0.8 Scope and Authority Roadmap

Date: 2026-08-25

## Decision

**RECOMMENDED: A. Bounded file creation.**

v0.8 should authorize one conditional creation of one regular UTF-8 file beneath
an existing host-trusted repository root. It must not become generic `fs.write`,
directory creation, overwrite, staging, commit, or history authority. The
eventual public name/schema are intentionally left to Task 084.

This closes the practical workflow break:

```text
inspect repository -> reason about change -> edit existing files ->
need a new source/config/test file -> stop
```

`repo.patch` already makes bounded exact replacements in one clean HEAD-tracked
file. Creation completes the smallest missing unit of normal repository
authoring while reusing the repository binding, lease, identity checks,
ToolRegistry, profile composition, observer evidence, and Generic Tool Bridge.

## v0.7 baseline and product gap

Released tag `v0.7.0` is at `9521fa4e5f5c184eabd0061eb71854422752b8f1`.
This roadmap starts at post-release cleanup
`1c7c35bab91839798b1f1b4833ed5c26e0d8f4e0`: 11 packages, all `0.7.0`, Rust
edition 2024. `AgentRuntime` remains the abstraction; the Codex app-server is
the primary Codex boundary; RAH does not implement inference; ToolRegistry is
the extension boundary; MCP and Process Plugin are Tool providers.

v0.7 can inspect bounded workspace files/repositories, patch one existing
tracked UTF-8 file, execute closed host capabilities, consume local stdio
providers, compose trusted profiles, and reproduce certified Codex live paths.
It cannot make the new source, test, configuration, fixture, or documentation
file required by many ordinary coding tasks. This is now the main product
bottleneck. No generic shell/process/file-write, Git commit/history/ref/network
authority, persistence, network MCP, plugin lifecycle, or hot reload follows.

## Decision matrix

Scale: value, reuse, deterministic testability, and live feasibility: 5 is
strongest. Risk and complexity: 5 is highest/worst. Scope: 5 is largest.

| Candidate | Value | Risk | Reuse | Deterministic | Live | Cross-platform complexity | ADR | Schema complexity | Failure/rollback | Scope | Classification |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | --- |
| A. Bounded file creation | 5 | 3 | 5 | 4 | 4 | 3 | new | 3 | 3 | 3 | **v0.8 product** |
| B. Multi-file bounded edit operation | 4 | 4 | 3 | 3 | 3 | 5 | new | 4 | 5 | 5 | v0.9 research |
| C. Richer patch primitive | 3 | 3 | 3 | 4 | 3 | 2 | amend/new | 4 | 3 | 3 | defer |
| D. Git commit/history authority | 3 | 5 | 2 | 3 | 2 | 3 | new | 4 | 5 | 5 | defer |
| E. Session/workflow persistence | 4 | 4 | 3 | 3 | 3 | 3 | new | 4 | 4 | 5 | independent track |
| F. Network MCP / Streamable HTTP | 3 | 5 | 2 | 2 | 2 | 4 | new | 4 | 4 | 5 | defer |
| G. PluginManager/lifecycle | 2 | 5 | 2 | 2 | 2 | 4 | new | 4 | 5 | 5 | defer |
| H. Dynamic profile reload | 2 | 5 | 2 | 2 | 2 | 3 | new | 3 | 5 | 5 | defer |
| I. Codex schema compatibility automation | 4 | 1 | 4 | 5 | 4 | 2 | none | 1 | 1 | 2 | release-tooling side task |
| J. Release/live-gate hardening only | 3 | 1 | 5 | 5 | 5 | 2 | none | 0 | 1 | 2 | continuous side task |

File creation wins over multi-file editing because it solves the missing-file
gap without partial multi-target effects. It wins over persistence because a
resume system cannot complete the basic authoring loop. It wins over schema
automation because compatibility tooling is essential supporting work, not new
user workflow authority.

## Authority ladder

```text
Read bounded workspace content
  -> Observe repository state
  -> Mutate one existing clean HEAD-tracked file
  -> Create one new file at a host-approved repository target       [v0.8]
  -> Bounded logical edits across several existing files
  -> Mutate Git index
  -> Create commit object
  -> Update branch/tag refs or rewrite history
  -> Network Git
```

Index mutation is separate from worktree content. Creating a commit object,
moving a ref, rewriting history, tagging, and pushing are separate authority
classes. Content mutation never implies Git history authority.

## A. Bounded file creation: research contract

### Host ownership and target rules

- The host selects one canonical, non-bare repository resource and fixed limits.
  The model provides only a logical relative `/` path, never a native path,
  temporary path, parent policy, or file mode.
- Accept only nonempty normal components. Reject absolute, drive-relative or
  qualified, UNC, verbatim, device, dot/dot-dot, backslash, colon/ADS, NUL,
  empty, and case-insensitive `.git` components.
- The target must be absent at commit-point validation. Existing tracked,
  untracked, ignored, staged, or special targets all refuse; none can be
  overwritten. This makes the tracked/untracked distinction observable but
  never an overwrite authorization.
- The parent must already exist, remain inside the repository, and be a regular
  directory. No parent creation, nested repositories, sparse/linked worktrees,
  or special files in the initial capability.

### Native behavior, race handling, and recovery

- Under the per-repository RAH lease, revalidate the root, `.git`, every parent,
  and target absence. Reject symlinks, junctions, all Windows reparse points,
  mount-like redirections, and special types. Native identities, not strings,
  are the Windows basis.
- Require bounded strict UTF-8 with no NUL and fixed maximum bytes. Extensions
  are not a safety boundary. Do not normalize newlines or create ADS.
- Use an exclusive native create-new call at the final target, not a temporary
  rename: rename cannot preserve no-overwrite semantics when an actor creates
  the target between validation and commit. Unix creation uses a host-fixed
  restrictive mode; any final approved mode is host-fixed. The model cannot set
  executable bits, ACLs, ownership, attributes, or streams. Windows requires a
  tested Unicode create-new primitive and refusal of unsupported conditions.
- If a file appears concurrently, create-new must fail. It is a known refusal
  only after post-observation proves RAH did not create the target; otherwise it
  is uncertain. Every reported success verifies exact bytes, regular-file and
  parent/root identity, and unchanged raw index, HEAD, and refs.
- The create-new syscall is the commit point. After timeout, crash, lost result,
  conflicting observation, or incomplete cleanup observation, report uncertain
  effect and never auto-replay. Deletion is not rollback: it is separate
  destructive authority and may delete a subsequently replaced third-party file.
  Host-private audit records bounded identity/digest evidence only.

### ADR, permission, profile, and bridge

Use a **new successor ADR**. ADR 0012 expressly defers untracked/new files and
has a different replacement commit point, no-overwrite rule, mode behavior, and
recovery model. Keep it unchanged.

`PermissionLevel::Execute` remains the outer gate; a private host-owned
repository-creation policy is the real authority. No new permission level is
justified. A future profile capability can reuse symbolic repository resources
(and a symbolic Git executable only if required for observation) as an additive
closed binding. Retain `profile_version: 1` if compatibility proof permits; do
not bump it merely for a new recognized closed capability. The Generic Tool
Bridge needs no production change: normal ToolDefinition, alias, permission,
dedupe, cancellation, and lifecycle behavior suffice.

## Why alternatives do not win

### B. Multi-file edit operation

Several edits in one request are a logical operation, not a portable atomic
filesystem transaction. It needs repository lease, all-preimage validation,
temp postimages, commit ordering, post-observation, and defined partial effects.
Windows replacement can fail under sharing/filter interference; Unix rename is
only per directory entry. If one later replacement fails, rollback can fail,
overwrite external changes, or be interrupted. Crash recovery needs a
host-owned journal and new recovery authority. Call the future feature a
`bounded multi-file edit operation`, never a transaction without a real portable
all-or-nothing proof.

### C. Richer patch; D. Git history

Exact replacement is deterministic: a whole-file digest/length and unique
matches in one original snapshot. Line/range edits add newline, encoding, and
offset drift; unified diff adds paths, hunks, fuzz, parser ambiguity, and
possibly multi-file partial effects; syntax-aware edits add language/parser
version authority. Defer until creation shows exact matching is the bottleneck.

Git classes stay separate: create commit object, mutate index, update branch
ref, create branch, amend/rewrite, tag, and push/network Git. Commit identity,
hooks, signing, templates, parents, and visibility make even safe commit
creation a later, distinct authority.

### E. Session persistence

Task 075 remains decisive: persist only RAH session ID, runtime-adapter identity,
Codex thread ID, workflow metadata/checkpoint, bounded history/summaries,
repository identity, trusted-profile identity/reference, and pending/terminal
state. Never serialize a live ToolRegistry, process handle, provider process,
dedupe map, private alias, or resolved authority object. On resume recompute
authority from current trusted state, recompose a fresh registry, regenerate
aliases, and invalidate stale thread/workspace/profile/provider state. This
needs durable-state migration, privacy, and recovery design, so it is an
independent v0.9-or-later track.

### F/G/H. Network MCP, PluginManager, dynamic profile

Streamable HTTP adds endpoint allowlisting, TLS/authentication/token storage,
redirect/proxy/DNS-host-change policy, request/response bounds, cancellation,
redelivery, remote effects, and provider identity. The MCP 2025-06-18 transport
specification requires servers to validate Origin against DNS rebinding; HTTP
authorization is OAuth-based when supported. RAH would not provide network
isolation. This is too broad for v0.8.

Plugin lifecycle further requires installation authority, provenance/signature
or hash, pin/update policy, startup/restart/crash health, replacement/rollback,
discovery, and profile binding; auto-discovery cannot be authority. Reload adds
authority gain/loss while sessions exist, in-flight calls, stale registries,
atomic recomposition, provider-start rollback, and model-visible tool changes.
Both remain deferred.

### I/J. Codex compatibility and live gates

These are release-tooling side tasks. Generate certified and candidate
app-server schemas, normalize only stable ordering/irrelevant metadata, store
content fingerprints, diff methods/required fields/events/tool constraints, and
auto-classify only clearly additive or clearly breaking change. Unknowns require
human review, deterministic adapter fixtures, isolated live smoke, and explicit
promotion; no candidate is automatically certified.

Tasks 080A-080C changed priorities for validation, not product scope: binary
version/hash alone was insufficient, user configuration drift changed behavior,
isolated `CODEX_HOME` was necessary, model/reasoning/features required pinning,
model final prose was not evidence, and host structural assertions were stronger.

## ADR and debt matrix

| Candidate | ADR impact | Debt/dependency | Classification |
| --- | --- | --- | --- |
| A | new successor to ADR 0012 | Reuses mutation foundation; adds native create semantics | v0.8 product |
| B | new | Requires partial-effect/recovery design | likely v0.9 research |
| C | amend/new | Adds matching and schema complexity | defer |
| D | new | Requires index/identity/hook/ref policy | defer |
| E | new | Requires durable-state/invalidation foundation | independent track |
| F | new | Requires network/auth policy foundation | defer |
| G | new | Requires provenance/install/update authority | defer |
| H | new | Requires atomic lifecycle/recomposition | defer |
| I | none | Reduces compatibility debt | release tooling side task |
| J | none | Reduces reproducibility debt | continuous side task |

## Future verification strategy

Deterministic work must cover closed input/schema, path and encoding limits,
tracked/untracked/ignored/staged/nested repository refusals, index/HEAD/ref
invariants, race/fault seams, and redaction. Windows-specific coverage must
exercise ADS/device/UNC/verbatim forms, reparse/junction/symlink chains, native
identity, sharing failures, and create-new faults. Unix must cover links,
regular-file/mode behavior, and create-new races. This is not a macOS/ARM64
claim.

Profile composition must build a fresh registry; bridge tests must prove outer
Execute denial before filesystem entry, canonical lifecycle/alias isolation,
dedupe/no-replay, cancellation, and uncertain outcomes. Future Windows live
validation selects the certified binary by SHA in isolated `CODEX_HOME`, pins
model/reasoning/features and a redacted config fingerprint, uses fresh profile
composition/registry, and host-attests exactly one request/native creation,
exact postimage, unchanged index/HEAD/refs, restricted Codex authority,
terminal completion, and cleanup. Ubuntu CI remains deterministic evidence
only; model prose is never proof.

## Proposed next task

**Task 084 — Repository File Creation Contract and ADR Research.** Define the
successor ADR, exact authority boundary, native create strategy, profile
compatibility decision, error taxonomy, deterministic tests, and certified live
gate. It must stop before production implementation. A reviewed Codex schema
compatibility tool may proceed separately as release tooling.
