# RAH v0.9 Scope and Authority Roadmap

Date: 2026-08-25

## Decision

**RECOMMENDED v0.9 PRODUCT CENTER: A bounded multi-file repository edit over
several existing clean HEAD-tracked UTF-8 files.**

This is the next smallest useful repository-authoring authority. It completes
the common coherent-change workflow (for example source plus test, or a caller
plus its declaration) while retaining the proven literal-precondition model.
It must be an operation with independently committed target replacements, **not
a transaction**: no portable all-or-nothing cross-file filesystem guarantee has
been established.

The initial v0.9 design should allow a small host-fixed number of distinct
existing files (recommended maximum: four). Each item has its own logical path,
complete-file SHA-256 and byte-length preconditions, and one through sixteen
exact literal replacements evaluated only against that file's original snapshot.
All targets, preimages, postimages, and repository invariants must validate
before the first target mutation. A shared repository lease covers the entire
operation; the host fixes a deterministic lexical commit order. The operation
reports `partial_effect` after a verified prefix of target replacements and
`uncertain` whenever an attempted or observed effect cannot be fully classified.
It never retries, replays, rolls back, stages, changes HEAD, or changes refs.

Do not initially combine `repo.create-file` with this operation. Creation has a
different commit point and can retain a partial file; mixing it would enlarge
the outcome, recovery, and audit model before the existing-file operation is
proven.

## Authoritative v0.8 baseline

- `v0.8.0` is released at `0b12d5448dcea89b158e4941e7b741b7539c8894`.
- The post-release cleanup/current `master` starting point is
  `1bbeea43ff3bf93f09c25dc4c3f6d5521437d407`.
- The workspace has 11 packages, all version `0.8.0`, Rust edition 2024.
- The certified live executable remains exactly `codex-cli 0.149.0` on
  Windows. Ubuntu/Linux evidence is deterministic CI/native-test evidence only
  unless a later gate explicitly establishes a different claim.

`repo.patch` already performs one final replacement for one existing, clean,
HEAD-tracked regular UTF-8 file. It supports the legacy single exact
replacement or one to sixteen exact replacements, resolves all matches against
one original snapshot, rejects duplicate/overlap ambiguity, shares the
repository mutation lease, verifies pre/post target and Git state, and never
replays an uncertain effect. `repo.create-file` separately creates one absent
UTF-8 regular file with exclusive native creation, no parent creation or
overwrite, and conservative partial-write/no-replay semantics. `host.git.stage`
and `host.git.unstage` already mutate one host-selected index target using an
empty model schema; they are not model-selected staging authority.

`rah-session` already owns a provider-neutral `SessionStore` and
`MemorySessionStore`; no durable workflow/session persistence exists.

## Current product gap and authority ladder

The current product can inspect repository state, make several separate
single-file edits, create one new file, and perform one host-predetermined
index operation. It cannot submit one bounded, reviewable change spanning
several already-clean source files with a single preflight and explicit partial
effect model. Sequential `repo.patch` calls leave the model responsible for
ordering and recovery after every intermediate effect.

```text
bounded read and repository observation
  -> one existing-file conditional edit / one absent-file creation
  -> bounded multi-file existing-file edit                         [v0.9]
  -> model-selected index mutation
  -> commit object / history and ref mutation
  -> network Git
```

Every step is separately host-authorized. A model request, an external tool
declaration, Codex approval, or provider metadata is never authorization.

## A-L decision matrix

Scale: value, reuse, deterministic testability, and live-gate feasibility: 5
is strongest. Risk and complexity: 5 is highest/worst. ADR impact is `none`,
`amend`, `new`, or `successor`.

| Candidate | Value | Authority risk | Reuse | Implementation | Failure/recovery | Deterministic | Windows | Cross-platform | Live gate | ADR | Suitability |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- |
| A. Multi-file bounded edit | 5 | 4 | 4 | 4 | 5 | 3 | 5 | 5 | 3 | successor/new | **Best v0.9** |
| B. Richer patch/edit primitive | 3 | 4 | 3 | 4 | 4 | 3 | 3 | 4 | 3 | amend/new | Defer |
| C. Git staging authority | 3 | 4 | 3 | 4 | 4 | 3 | 3 | 3 | 3 | successor/new | Defer |
| D. Git commit/history/ref authority | 3 | 5 | 2 | 5 | 5 | 2 | 3 | 4 | 2 | new | Defer |
| E. Session/workflow persistence | 4 | 4 | 3 | 5 | 5 | 3 | 3 | 3 | 2 | new | Separate track |
| F. Network MCP / Streamable HTTP | 3 | 5 | 2 | 5 | 5 | 2 | 3 | 4 | 2 | new | Defer |
| G. PluginManager / lifecycle | 2 | 5 | 2 | 5 | 5 | 2 | 3 | 4 | 2 | new | Defer |
| H. Trusted Profile reload | 2 | 5 | 3 | 5 | 5 | 2 | 3 | 3 | 2 | new | Defer |
| I. Repository directory creation | 4 | 4 | 3 | 4 | 4 | 3 | 5 | 5 | 3 | successor/new | Fallback only |
| J. Repository delete/rename | 3 | 5 | 2 | 5 | 5 | 2 | 5 | 5 | 2 | new | Defer |
| K. Codex schema/baseline automation | 4 | 1 | 5 | 2 | 1 | 5 | 2 | 2 | 4 | none | Supporting tooling |
| L. Live-gate/release-tooling hardening | 4 | 1 | 5 | 2 | 1 | 5 | 3 | 3 | 5 | none | Supporting tooling |

## Candidate analysis

| Candidate | Exact authority and value | Boundary, API, profile, and bridge impact | Failure and platform conclusion |
| --- | --- | --- | --- |
| A | One host-bound request may replace complete postimages for a small fixed set of existing clean tracked UTF-8 paths. It makes coherent edits useful without generic write. | Needs a successor/new ADR beside ADR 0012; private policy and likely a new closed Tool schema, but no `PermissionLevel` change. Additive profile binding may reuse symbolic repository/Git resources at profile version 1 if compatibility is proved. Generic Tool Bridge remains generic. | Before first commit proven no effect; after each commit a verified prefix is `partial_effect`; any contradictory/lost/cancelled/crashed observation is `uncertain`. Windows per-file native replacement and locks dominate; Unix same-directory replacement is per-file only. One Windows certified live fixture is feasible after deterministic fault coverage. |
| B | Range, hunk, unified-diff, regex, or syntax-aware requests would gain more ambiguous content-selection authority than literal exact replacement. | Requires an ADR amendment or new ADR and materially larger parser/schema surface; profile and bridge could transport it but cannot authorize it. No permission change is justified. | Offset drift, newline/encoding normalization, fuzzy matching, parser versions, and multi-path patch forms make deterministic refusal harder. It does not improve the proven Windows commit primitive; defer. |
| C | Existing `host.git.stage`/`unstage` only acts on one host-selected target with `{}`. Generalized staging would let a model select repository paths/index content. | ADR 0010 is index-only and insufficient for a broadened target-selection class; use a successor/new ADR. New profile grammar and schema would be required; bridge mechanics remain generic; retain Execute as outer gate. | Git index lock, sparse/conflict/intent-to-add paths, lost process result, and hook/config isolation require conservative uncertain outcomes. It is not needed for authoring a working-tree change. |
| D | Commit, amend, tree/object, branch/tag/ref, history rewrite, and network publication are separate durable visibility authorities. | New ADR and likely several closed capabilities, profile entries, and fixed host metadata/identity policy. The bridge adds no authority. | Hooks, editors, signing helpers, templates, credentials, reflogs, object writes, and ref-update ordering make rollback unsafe. Defer broad history/ref authority. |
| E | Durable session/workflow state would support resume, audit, and pending-work continuity. | `SessionStore` is reusable, but durable storage, migration, retention, encryption, stale runtime/thread/profile invalidation, and no-replay state need a new ADR and public-store error/API design. | Crash consistency and resuming pending effects are high-risk. Windows/Unix filesystem behavior is manageable but no certified Codex live proof can establish durable authority safety alone; separate track. |
| F | Streamable HTTP MCP permits remote tool-provider connectivity, hence endpoint/auth/network authority. | Requires new ADR, HTTP/TLS/DNS/redirect/proxy/Origin/auth policy, dependencies, profile representation, and adapter lifecycle. Generic Tool Bridge should only see registered Tools. | Remote effects, reconnect/redelivery, cancellation, and credential handling are inherently uncertain. Cross-platform transport is not the issue; the authority expansion is. Defer. |
| G | A manager would discover, install, update, start, restart, and remove providers, beyond today's fixed Process Plugin adapter. | New ADR, lifecycle/provenance/installation APIs and profile rules; current Tool convergence is reusable but not authority to manage providers. | Executable replacement, crash/restart, provenance, partial update, and rollback need a durable authority model. Defer. |
| H | Reload changes the effective authority set while sessions and provider processes may be active. | ADR 0011 explicitly defers automatic hot reload; a new ADR, registry handoff/lifetime API, profile identity generation, and session invalidation policy are necessary. | In-flight calls, stale aliases, provider teardown/startup and audit ordering make uncertain authority state likely. Do not add. |
| I | `mkdir` would enable nested new module/test layouts but authorizes persistent namespace allocation beyond `repo.create-file`. | ADR 0013 excludes it; require a successor/new ADR, private policy and closed schema/profile binding. Bridge remains generic and Execute stays outer-only. | A created directory can remain after failures and its removal is destructive recovery. Windows reparse/reserved-name/ACL behavior and Unix directory-FD checks are substantial. It is the fallback only if A is rejected. |
| J | Delete/rename changes path identity and can remove or hide user work; rename crosses existing/new targets. | New ADR, distinct policy, profile binding, and possibly schema. Neither ADR 0012 nor 0013 authorizes it. | Rollback can overwrite third-party changes; cross-volume/name-case Windows behavior and Unix hard-link/rename semantics are difficult. Defer. |
| K | Candidate-versus-certified schema and baseline comparisons improve adapter compatibility detection, not model authority. | No ADR, public API, profile, bridge, or permission impact if kept as host release tooling. It may reuse local schema generation and fixed fixture comparisons. | Normalize only known nonsemantic fields, fingerprint artifacts, and require human review for unknown differences. Never auto-promote a candidate binary; deterministic and Windows-friendly. |
| L | Stronger host attestation, fixture cleanup, version/hash/config checks, and exact-commit CI evidence reduce release risk. | No product authority or public boundary change. Reuse existing release gates and Generic Tool Bridge fixtures. | Assert call counts, final continuation, cleanup, and no replay. Windows remains the certified live platform; Ubuntu/Linux remains deterministic unless separately certified. Fold into release work, not the v0.9 center. |

## Recommended A contract boundaries

The next research task should test—not accept or implement—the following
starting contract:

- One canonical non-bare host-selected repository and one shared RAH mutation
  lease. The model supplies only a bounded array of distinct logical relative
  paths and exact replacement requests.
- Recommended limits: two through four targets, one through sixteen exact
  replacements per target, existing `repo.patch` byte/text bounds, and a
  host-fixed aggregate byte/request bound. A later task must justify exact
  constants.
- Each target is independently a regular, strict-UTF-8, clean HEAD-tracked
  stage-0 file under the existing no-link/no-reparse/no-sparse/no-submodule
  rules. It receives independent SHA-256 and raw byte-length preconditions.
- Capture every original snapshot; resolve every match only in its own original
  snapshot; reject duplicate paths, duplicate replacements, overlap, missing or
  non-unique matches, and no-op ambiguity. Build and validate every complete
  postimage before mutation begins.
- Revalidate repository/root/`.git`/parent/target identities, every preimage,
  and index/HEAD/ref observations immediately before the first commit. Commit
  independently prepared postimages in deterministic host-owned lexical path
  order, never model order.
- A successful item needs exact postimage plus unchanged index/HEAD/refs and
  target identity verification. Earlier success does not make later failure a
  transaction failure that can be undone.
- Return a bounded redacted result. It must identify the safe logical target
  status/counts, never preimage text, absolute paths, temporary names, or
  recovery instructions that imply authority.

### Failure, cancellation, and recovery model

There is no cross-file atomicity, rollback, automatic cleanup guarantee, or
automatic retry/replay after a possible effect.

1. A refusal before the first native replacement is known non-mutation only
   when every target still equals its captured preimage.
2. After any verified replacement, a later known failure produces
   `partial_effect` and records the verified committed prefix. It must not
   replace prior files back to their preimages.
3. A replacement error is known for that target only if post-observation proves
   its preimage intact. If any committed prefix or target cannot be fully
   observed, report `uncertain` (and retain any safely known prefix only as
   audit/result context).
4. Cancellation, disconnect, timeout, lost OS result, or process crash at or
   after an item commit point is never rollback. A new call may not infer that
   it is a safe retry; an operator must inspect state and authorize a new,
   fresh-preconditioned request.
5. Host-private bounded audit/preimage evidence may aid diagnosis but grants no
   recovery authority. Any future recovery must have its own ADR and
   compare-before-recovery design.

## Architecture, dependency, and authority effects

The recommendation should reuse the `rah-tools` repository identity checks,
Git-state verifier, native per-file final replacement, shared lease, observers,
trusted-profile construction pattern, and RAH-owned `Tool`/`ToolRegistry`
path. It must not put Codex types outside `rah-runtime-codex`, add an edge below
`rah-protocol`, add a runtime dependency to tool providers, or add a general
filesystem/process abstraction.

The policy remains crate-private and host-constructed. A future public tool
definition is a RAH-owned closed input/output schema, not a public policy API.
`PermissionLevel::Execute` remains an outer dispatch gate; it is insufficient
on its own and no new level should be introduced for this narrow capability.

ADR 0012 must not be silently broadened. Task 093 should decide whether the
different multi-target commit/failure model needs a successor ADR (recommended)
or a carefully scoped amendment. ADR 0010 remains index-only, ADR 0013 remains
single absent-file creation, and ADR 0011 remains composition-only.

Trusted Profile impact is additive only after deterministic implementation:
one closed capability can bind already-approved symbolic repository/Git
resources and fixed limits. It cannot select paths, relax preconditions,
authorize rollback, reload itself, or let a provider declaration grant access.
Keep `profile_version: 1` unless an actual incompatible schema change is
proved. The Generic Tool Bridge should require no production redesign: it
advertises a private alias, enforces the existing required permission, dispatches
the RAH Tool call, preserves lifecycle/dedupe/cancellation behavior, and must
not replay possible effects.

## Test and validation strategy

Deterministic tests must cover closed schema and aggregate bounds; duplicate
paths; all precondition/path/UTF-8/BOM/NUL failures; snapshot-relative match
resolution; overlap; initial all-target validation; deterministic order;
unchanged index/HEAD/refs; unrelated sentinel preservation; every prefix
failure; external changes between preparation and each commit; temporary/native
replacement faults; lost results; cancellation/disconnect; redaction; and no
replay. A real profile composition test must build a fresh `ToolRegistry` and
prove the outer Execute denial occurs before native mutation.

Windows tests need native handle identity at every target and parent, rejected
UNC/verbatim/device/ADS/reserved/reparse/junction aliases, case-equivalence,
hard-link policy, sharing violations, filter/antivirus-like replacement
failure seams, and exact prefix classification. Unix/Linux tests need
descriptor/path traversal protections, symbolic-link rejection, same-directory
replacement, mode preservation, invalid UTF-8 path refusal, and external writer
races. Neither platform's per-file rename/replacement proves durability,
cross-process exclusion, or cross-file atomicity.

The later certified Windows live gate should use the unchanged exact
`codex-cli 0.149.0` binary/hash and isolated configuration, compose a fresh
trusted profile/registry, disable Codex-owned write/shell/process/network/MCP/
plugin paths, and host-attest exact bounded call count, per-target postimages,
unchanged index/HEAD/refs, terminal lifecycle/final continuation, child and
fixture cleanup, and no replay. Ubuntu/Linux CI remains deterministic
cross-platform/native evidence only.

## Explicitly deferred authorities

- Generic `fs.write`, full-file overwrite/append/truncate, binary edits,
  filesystem restore, chmod/ACL/attribute changes, and generic shell/process.
- Mixing file creation with A; directory creation; deletion; rename/move;
  untracked/staged target edits; links/reparse points; and automatic rollback,
  recovery, retry, or replay.
- Model-selected Git staging, commit/amend, objects, refs, tags, branches,
  history rewrite, hooks/signing, credentials, and network Git.
- Durable session/workflow persistence, network MCP/Streamable HTTP,
  PluginManager/installation/lifecycle, and Trusted Profile reload.
- Any change to `PermissionLevel`, Trusted Profile `profile_version`, or the
  certified Codex baseline.

## Fallback recommendation

If Task 093 establishes that safe per-prefix classification or deterministic
Windows replacement evidence makes A too broad, make **I. bounded repository
directory creation** the fallback product center. It is a narrower persistent
namespace authority than delete/rename or model-selected staging and directly
unblocks nested source/test layouts. It still needs a separate ADR and must be
one empty, previously absent, model-selected repository-relative directory at
an existing parent, with native exclusive creation, no child creation, no
rollback/replay, and no create-file bundling. Do not substitute a richer patch
language or generic write as the fallback.

## Proposed Task 093+ sequence

1. **Task 093 — Multi-File Bounded Repository Edit Contract and ADR Research.**
   Research only: define exact schema/limits, outcome taxonomy, commit ordering,
   ADR relationship, audit model, deterministic and certified-live gates. Stop
   before production implementation.
2. Task 094 — Multi-File Bounded Repository Edit Deterministic Implementation.
   Implement only the accepted Task 093 contract with Windows/Unix fault tests.
3. Task 095 — Trusted Profile and Generic Tool Bridge Composition Research and
   Deterministic Validation. Confirm additive closed composition before any live
   example.
4. Task 096 — Certified Windows Live Multi-File Edit Gate. Require the existing
   certified baseline and host attestation; do not promote a candidate binary.
5. Task 097 — v0.9 Milestone Audit and Release Preparation. Include supporting
   Codex compatibility/release-tooling hardening as separately reviewed work.

Candidate/certified Codex automation remains supporting release tooling: it can
generate, fingerprint, and diff candidate artifacts, but only an explicit
human/host certification decision can promote a binary or baseline.
