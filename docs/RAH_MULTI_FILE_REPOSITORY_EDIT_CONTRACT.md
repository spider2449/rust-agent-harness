# RAH Bounded Multi-File Repository Edit Contract

Status: Implemented through Task 094C; Trusted Profile and live certification deferred

## 1. Scope

`repo.edit-files` is a logical bounded edit across existing clean HEAD-tracked UTF-8 files in one host-authorized repository. It is not a transaction and makes no cross-file atomicity, crash-durability, rollback, recovery, retry, or replay claim. Task 094C exposes it only as a direct host-constructed `rah-tools` Tool backed by its private authority engine.

## 2. Terminology

- Logical path: slash-separated repository-relative request string.
- Canonical target: safely resolved regular-file identity under host canonical root.
- Preimage/postimage: complete bounded target bytes before/after all replacements.
- Preflight: every validation/preparation step before first native replacement.
- Commit point: native replacement invocation for one target; never a Git commit.
- Committed prefix: ordered targets with verified postimages.

## 3. Existing authority baseline

ADR 0012 authorizes `repo.patch`, one clean existing tracked file, through private `RepositoryWorktreeMutationPolicy`. It resolves up to sixteen exact replacements against one snapshot, creates a same-parent temporary, attempts replacement once, verifies Git/target state, preserves Unix mode, and makes lost post-observation uncertain. ADR 0013 authorizes `repo.create-file`, one exclusive new file, with materially different partial-write semantics. `repo.patch`, `repo.create-file`, `host.git.stage`, and `host.git.unstage` share one per-repository mutation lease. `PermissionLevel::Execute` is an outer gate only; model data does not select authority.

## 4. Authority boundary

The proposed public name is `repo.edit-files`; private policy is `RepositoryMultiFileMutationPolicy`. Host construction binds one canonical non-bare repository, Git/repository identities, shared lease, fixed limits, and temporary names. One request may edit **one through four** files. Four is deliberately small: one-file patch already has 1 MiB/16-replacement bounds, while four bounds images, temporary artifacts, attempts, inventory, and partial-state reasoning without claiming atomicity.

Targets must be existing regular HEAD-tracked files with one normal stage-0 index entry equal to the HEAD regular blob, clean worktree state, strict NUL-free UTF-8, no symlink/reparse/hard-link, and no sparse, submodule, gitlink, intent-to-add, staged, unmerged, ignored, or untracked ambiguity. No creation, deletion, rename, directory creation, mode/ACL/attribute mutation, binary edit, staging, Git history/ref/network, shell/process, generic `fs.write`, rollback, or replay is granted.

## 5. Request schema

```json
{"targets":[{"path":"src/example.rs","expected_file_sha256":"lowercase-64-hex-digest","expected_file_byte_length":123,"replacements":[{"expected_old_text":"old","replacement_text":"new"}]}]}
```

`targets` is the only top-level field. Each target has exactly the four displayed fields; each replacement has exactly the two displayed fields. New multi-file API supports only `replacements`, not `repo.patch` legacy single-pair fields; legacy compatibility has no consumer for a new surface. Paths have nonempty normal `/` components only: no `.`, `..`, backslash, colon/ADS, NUL, absolute, drive-relative, UNC, verbatim/device, or case-insensitive `.git`. Absolute root never enters arguments, output, or inventory.

## 6. Fixed bounds

| Limit | Value |
| --- | ---: |
| Serialized request | 256 KiB |
| Files per call | 1 through 4 |
| Logical path | 1,024 UTF-8 bytes |
| Replacements per target | 1 through 16 |
| Old/replacement text | 64 KiB UTF-8 bytes each |
| Original/postimage per target | 1 MiB |
| Aggregate original/postimage bytes each | 4 MiB |
| Aggregate replacements | 64 |

Parser checks serialized size before allocation, uses checked arithmetic, and enforces aggregate postimage bound before temporary creation. All bounds are host-owned.

## 7. Repository/target validation

Under the lease, validate root, `.git` directory form, Git executable identity, root identity, and repository state. Walk each target rejecting symlink/reparse traversal; canonical resolution must remain beneath and equal the walked path. Reject special files, Windows unsupported attributes, hard links, replaced parent/target identity, linked worktree, sparse/index flags, submodules, and nested-repository ambiguity. Reject duplicate logical paths first, then duplicate canonical paths and file identities to stop case, hard-link, normalization, and other aliases.

## 8. All-target preflight

Before first persistent mutation, the policy must:

1. Acquire the existing shared repository mutation lease.
2. Validate host-bound repository and policy identity.
3. Parse and bound the complete request.
4. Safely resolve every target and reject aliases.
5. Verify every target tracked and clean.
6. Capture every preimage, file/parent identity, raw index, HEAD, refs, and Git observations.
7. Verify every requested SHA-256 and byte length.
8. Resolve all replacements against original snapshots.
9. Reject missing, duplicate/ambiguous, and overlapping matches.
10. Compute all postimages and enforce per-file/aggregate bounds.
11. Prepare/verify every host-owned same-parent temporary postimage.
12. Immediately revalidate repository/index/HEAD/refs, paths, parent/target/temp identities, Git observations, and preimages.

Any failure before first native replacement causes zero repository-content mutations by this call. Failed proven-safe temporary cleanup is required; unproven cleanup becomes `uncertain`, not recovery authority.

## 9. Replacement resolution

Each nonempty old fragment occurs exactly once in the original decoded target snapshot. Resolve every match before generated output; reject duplicate definitions, ambiguous matches, and overlapping original ranges; permit adjacency. Sort by original byte offset and build one deterministic postimage in one pass. Same old/new is rejected as verified no-op. No regex, glob, fuzzy/unified-diff, line/range, or encoding-normalization semantics exist.

## 10. Postimage preparation

Each complete postimage is written to one unique exclusive host-named regular temporary in validated same-filesystem target parent, flushed, and checked for identity/type/bytes/bounds. Preserve Unix target mode on temporary; Windows rejects unsupported attributes. Preparation is not commit. Remove a temporary only after identity/content proof.

## 11. Commit ordering

Host sorts targets by ascending lexicographic UTF-8 byte sequence of canonical logical repository-relative path. Request order never controls commit order. Immediately before each replacement revalidate repository, raw index, HEAD, refs, target/parent identity, clean state, original bytes, and temporary postimage/identity. Changed observation stops before that target commit.

## 12. Per-target commit point

First persistent side effect is first native replace-once invocation. Each target gets one attempt and no retry. Pre-call errors are known-no-effect for that target. After success verify bytes, target/parent identity, Unix mode where applicable, index, HEAD, and refs before continuing. Per-file OS primitive behavior never makes this operation cross-file atomic.

## 13. Result taxonomy

Redacted JSON output contains only `status` and, for commit-engine outcomes, ordered `effects` inventory. Each effect contains only the logical `path` and state. It never contains a reason, count, absolute path, temporary name, image text, native error detail, or policy internal.

| Status | Meaning |
| --- | --- |
| `ok` | Every target committed and all final observations verified. |
| `invalid_target` | Schema/bounds/path/duplicate-logical/shape rejection before mutation. |
| `precondition_failed` | Repository, identity, clean state, snapshot, replacement, preparation, or pre-commit validation failed before first commit. |
| `failed_known_no_effect` | Native failure with original proven and no target committed. |
| `partial_effect` | Verified committed prefix and all other targets proven postimage, original, or not attempted. |
| `uncertain` | Any attempted/possibly affected target or required observation cannot be fully classified. |

Only `ok` is non-error. A later known failure after a committed target is `partial_effect` only with complete proof; otherwise it is `uncertain`.

## 14. Partial-effect semantics

`partial_effect` needs trustworthy repository observations and per-target values `committed_verified` (intended postimage proved), `unchanged_verified` (original proved after known failure), or `not_attempted` (commit point unreached and original proved). `uncertain` is only permitted in overall `uncertain`. Thus ordered A/B committed verified, C pre-commit failure/original verified, and D original/not attempted verified returns `partial_effect` with `[committed_verified, committed_verified, unchanged_verified, not_attempted]`. Any untrustworthy target/index/HEAD/ref/repository observation returns `uncertain`.

## 15. Uncertain-effect semantics

`uncertain` covers ambiguous native failure; success with unprovable post-state; OS/filter/share errors with unproven state; changed repository/target/parent/index/HEAD/ref observation during commit; unaccounted temporary; and interruption, cancellation, disconnect, crash, or lost response after a commit point without complete proof. Never retry, replay, continue prefix, roll back, or silently clean up. Caller must observe/reconcile before next mutation.

## 16. Cancellation/disconnect semantics

Before first effect, cancellation is known zero effect only after cleanup and zero repository-content mutation are proven. After commit point it never means rollback/no side effect. Complete bounded safe observation when possible; otherwise `partial_effect` only with full inventory proof or `uncertain`. Existing bridge dedupe remains at-most-once and must not resend cancelled/disconnected calls.

## 17. Concurrency/repository lease

Use `git_stage::repository_lease` keyed by canonical root from mutation-sensitive preflight through outcome construction. `repo.patch`, `repo.create-file`, stage, and unstage retain that exact lease. No independent multi-file lock. Lease serializes RAH only; observable external races fail conservatively.

## 18. Index/HEAD/ref preservation

Capture raw `.git/index` bytes, HEAD, and bounded refs during preflight; compare before each commit, after each success, and in final result. Mismatch stops future commits or is `uncertain` after a commit. Tool invokes no Git mutation and preserves index, HEAD, refs.

## 19. Trusted Profile integration (deferred)

`profile_version = 1` remains unchanged. Task 095 will add a closed symbolic `repo.edit-files` capability with existing `executable`/`repository` resources and `Execute`; Task 094C makes no profile schema, static inventory, effective-composer, CLI, or publication change.

## 20. Generic Tool Bridge impact (deferred composition evidence)

No production bridge semantic change. Alias mapping, Execute enforcement, ToolRegistry dispatch, dedupe, cancellation/disconnect, response translation, bounds, and no replay remain capability-agnostic adapter behavior. No bridge branch was added; full Trusted Profile-to-bridge composition evidence is Task 095.

## 21. Permission/security model

Outer `PermissionLevel::Execute` stays unchanged. Private host-bound policy is real authority; request provides only bounded conditions and literals. Path checks/process supervision are not OS sandboxing or cross-process exclusion.

## 22. Windows semantics

Retain canonical volume/file-index identity and reject all reparse/junction traversal. Use existing one-attempt `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`. Sharing violations, antivirus, cloud sync, and filter drivers can fail before/during/after call; unprovable state is uncertain. Same-parent temporary cleanup requires proof. `WRITE_THROUGH` is not crash durability.

## 23. Unix/Linux semantics

Use descriptor-relative/no-follow directory validation, same-directory same-filesystem temps, Unix mode preservation, and per-file rename only after parent/target revalidation. Rename success then failed observation is uncertain. Rename may be atomic for one name replacement, never several. No durability claim without future file/directory fsync proof.

## 24. Temp-file/native replacement behavior

One host temporary and at most one native replacement attempt per target. Pre-first-commit preparation failure is zero content effect only if all cleanup proves safe; otherwise uncertain. After replacement never recreate, rewrite, delete, or reuse target for recovery. Native primitives are private-policy details.

## 25. No rollback/no replay rationale

Rollback is extra mutation authority, can overwrite concurrent work, fail/interruption, and convert known partial state to uncertain. No hidden journal/recovery subsystem. Future explicit journal, operator recovery, or rollback needs separate ADR research. Replay is excluded because preconditions are time-sensitive; bridge dedupe is at-most-once boundary.

## 26. Interaction with `repo.patch`

`repo.patch` remains supported/certified as smaller one-target authority with backwards-compatible legacy form and simpler failure model. It is not replaced.

## 27. Interaction with `repo.create-file`

Targets already exist. `repo.create-file` remains separate because exclusive creation has partial-write residue and different commit/cleanup semantics. Mixed create/edit is not authorized.

## 28. Explicit non-goals/deferred authority

No OS sandboxing, arbitrary write, file/directory creation, deletion, rename, chmod, binary patching, shell/arbitrary process execution, staging, commit, refs, history rewriting, network Git, rollback, replay, recovery journal, or cross-file transaction. Generic diff/hunk/range/regex transforms, mixed creation, durable recovery, and external locking are deferred.

## 29. Deterministic test plan

Cover one/four success; zero/five rejection; duplicate logical/canonical aliases; invalid UTF-8; non-tracked/dirty/staged/unmerged/ignored/untracked; symlink/reparse/junction; SHA/length mismatch; missing/ambiguous/duplicate/overlap/adjacent replacement; replacement/request/aggregate bounds; all-target preflight zero mutation; first/middle/final preparation failure; repository/target race; native failure before/after one/after multiple commits; verified partial effect; uncertain/post-observation; cancellation/disconnect before/after one commit; deduped repeat/no replay/no rollback; index/HEAD/ref preservation; shared lease against patch/create/stage/unstage; Windows path/reparse/share; Unix descriptor/symlink/rename/mode.

## 30. Fault-injection matrix

| Phase | Required result |
| --- | --- |
| Parse/resolve/Git/snapshot/replacement | `invalid_target` or `precondition_failed`; zero target mutations |
| First/middle/final temporary prepare | zero mutation if cleanup proven; else `uncertain` |
| First replacement failure, original proven | `failed_known_no_effect` |
| Later pre-replacement failure, prefix/remainder proven | `partial_effect` |
| Ambiguous replacement/failed post-observation | `uncertain` |
| Cancel/disconnect before commit, zero effect proven | known zero-effect non-success |
| Cancel/disconnect after commit | `partial_effect` only with proof; else `uncertain` |

Each seam asserts one native attempt per attempted target, no next target after terminal failure, no automatic retry, no rollback.

## 31. Certified Codex live-gate plan

Future Windows-only gate verifies exact native `codex-cli 0.149.0`, clean fixture with at least three existing HEAD-tracked UTF-8 files, `TrustedStaticProfile::load -> rah_cli::profile_composition::compose`, fresh registry, and unchanged Generic Tool Bridge. Codex invokes `repo.edit-files` exactly once for three distinct exact edits. Host observer/independent checks prove postimages, expected unstaged status, raw index/HEAD/refs/sentinel unchanged, one request/start/finish, `Completed`, no alternate tool/replay, and cleanup. Deterministic native fault fixture—not forced live OS failure—proves partial effects. Ubuntu/Linux stays deterministic evidence only.

## 32. Implementation decomposition for Task 094A+

1. **094A:** pure request/preflight/postimage foundation/tests; no native/profile/bridge/live path.
2. **094B:** private policy, shared lease, native ordered commit/result inventory, platform/fault/race tests.
3. **094C:** implementation audit and ADR 0014 acceptance only if evidence matches.
4. **095:** additive profile-v1 and unchanged bridge deterministic integration.
5. **096:** Codex baseline/schema automation hardening.
6. **097:** certified Windows multi-file live gate.
7. **098--101:** v0.9 audit, release prep, release, cleanup.
