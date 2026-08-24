# RAH v0.7 Milestone Audit

Date: 2026-08-24

Task: 077 — audit / release-gate analysis only

Starting baseline: Task 076 `26fc3f12c5392527c2ef28004b43112a43976053`
Task 076 CI: `32697486808` (completed / success)

## Verdict

## NOT RELEASE READY

The sole release blocker is a user-facing documentation contradiction. The
current README says `repo.patch` can replace “exactly one literal text
occurrence” and repeats that one-replacement limit in its security boundary
list. The v0.7 implementation, tests, ADR 0012, and v0.7 contract instead
support a bounded `replacements[]` form of one through sixteen exact
replacements in one existing tracked file, while retaining the legacy form.
The code and tests are green, but code, tests, and docs do not agree; this is a
release blocker under the Task 077 gate.

No product, authority, profile, dependency, version, baseline, CHANGELOG, tag,
or release-preparation change was made by this audit.

## Milestone summary

v0.7 extends the already-authorized `repo.patch` capability with bounded
multiple exact literal replacements in **one** existing HEAD-tracked,
unstaged, strict-UTF-8 worktree file. It does not add a target class or generic
file/process/Git authority. All matches are resolved against the original
snapshot, a complete deterministic postimage is prepared first, and one native
target replacement is the only mutation commit point.

| Task | Commit | Purpose | Validation evidence | Release relevance |
| --- | --- | --- | --- | --- |
| 070 | `f29d891` | Define v0.7 scope and authority roadmap. | Scope/authority record. | Limits v0.7 to a narrow existing-file extension. |
| 071 | `830d7c7` | Research multi-replacement contract and ADR amendment. | Closed schema, limits, failure model. | Establishes compatible `repo.patch` design. |
| 072 | `024d460` | Implement bounded multi-replacement `repo.patch`. | Deterministic tool tests, Windows/Unix coverage. | Product feature and ADR 0012 amendment. |
| 073 | `fa42c07` | Validate through trusted profile and Generic Tool Bridge. | Real profile/composition/registry/Git fixture tests. | Proves routed authority and no-replay behavior. |
| 074 | `e7f84a4` | Windows live Codex multi-patch gate. | Three successful certified native runs. | Live proof of the intended end-to-end path. |
| 075 | `5991071` | Codex platform alignment audit. | Documentation audit plus CI. | No pre-release architecture change required. |
| 076 | `26fc3f1` | Reusable certified-baseline management. | Script tests, explicit archived smoke, CI `32697486808`. | Decouples certified release evidence from daily PATH drift. |

## Feature and bounds audit

The implemented schema retains the legacy single-replacement form and adds a
closed, mutually exclusive `replacements[]` form. It accepts only the same
existing tracked target file and performs all matching over one captured
original snapshot. Each nonempty expected text must occur exactly once;
duplicate, overlap, repeated-source, generated/sequential-match, stale hash,
and stale length cases refuse. Adjacent ranges are permitted. Ranges are sorted
by original byte offset and used to construct one postimage in one pass.

| Bound | Required and implemented value |
| --- | ---: |
| Replacements | 16 |
| Serialized request | 64 KiB |
| Aggregate old/new replacement text | 64 KiB |
| Per old/new text item | 64 KiB |
| Input file | 1 MiB |
| Final output | 1 MiB |

Task 072 unit coverage and Task 073 bridge coverage exercise the limit and
rejection matrix. ADR 0012 and the v0.7 contract document the same limits.
README is the only identified documentation disagreement because it still
states the obsolete one-replacement capability description.

## Authority and commit-point review

`repo.patch` remains the canonical capability and retains
`PermissionLevel::Execute` as an outer runtime gate. The private,
host-constructed `RepositoryWorktreeMutationPolicy` remains the actual
authority. There is no `fs.write`, `shell.exec`, `process.exec`,
model-selected executable/argv/cwd/environment, worktree-wide mutation, Git
commit/history/ref authority, or network Git authority.

ADR 0012 is coherent with the implementation: capture and verify original
target identity/hash/length; construct and flush a host-named temporary image;
immediately revalidate target and repository state; make one native final
target replacement; then exactly post-verify. Its public outcome taxonomy
remains `precondition_failed`, `replacement_failed_known`, `ok`, and
`uncertain`. It makes no rollback claim, and uncertain effects are never
automatically replayed.

## Repository invariants and bridge evidence

Task 073 uses a real `TrustedStaticProfile`, actual effective composition, a
fresh `ToolRegistry`, the Generic Tool Bridge, a real `repo.patch`, and a real
temporary Git repository. One three-replacement call, legacy compatibility,
call-identity dedupe/exact-once handling, generated-match dependency refusal,
overlap/duplicate/repeated-source refusal, bounds refusal, Execute admission,
canonical-name handling, cancellation, redaction, and restricted-Codex tests
remain covered. It made no production bridge or profile change.

The deterministic and live fixtures prove the target-only change and retain
HEAD, refs, raw index, staged diff/no auto-stage, and an unrelated tracked
sentinel. Task 074 additionally validates the postimage with `repo.file-info`,
worktree status, semantic diff, and empty staged diff.

## Live Codex evidence and platform scope

Task 074 recorded three fresh successful Windows runs using the exact certified
native `codex-cli 0.149.0`. Each used a trusted profile, fresh registry, Generic
Tool Bridge, native app-server, one `repo.patch` request with three
replacements, and exactly one request/start/finish/native-mutation count. Each
reached `Completed`, reaped the app-server, and emitted
`RAH_MULTI_PATCH_LIVE_OK`.

The historical aliases were `repo.diff = rah_tool_0`,
`repo.diff-staged = rah_tool_1`, `repo.file-info = rah_tool_2`,
`repo.patch = rah_tool_3`, and `repo.status = rah_tool_4`; they are evidence,
not API guarantees. Codex shell, unrestricted write, Codex-owned MCP,
arbitrary process, network/web, image, apps, and approval bypass were disabled,
so the successful mutation is attributable only to RAH-owned `repo.patch`.

Windows live Codex validation and Windows x64 baseline management are proven.
Ubuntu deterministic CI is green. This audit makes no Unix/macOS live-Codex,
Windows ARM64, or cross-platform binary-portability claim.

## Codex alignment and certified baseline audit

Task 075 requires no v0.7 code change: preserve
`AgentRuntime -> CodexRuntime -> native app-server`, ADR 0004’s no-inference
rule, RAH `ToolRegistry` authority, and MCP/Process Plugin as Tool providers.
Approval cannot create RAH authority; the desired layering remains RAH
authority AND Codex runtime policy AND required approval. A RAH Session
references, but is not identical to, a Codex thread; resume must recompute
authority from current trusted host state.

Task 076 supplies `scripts/codex-baseline.ps1` and
`scripts/test-codex-baseline.ps1`. It supports `save`, `verify`, `path`,
`list`, and `verify-all`, defaults to `%LOCALAPPDATA%\codex-baselines`, and
accepts `CODEX_BASELINE_HOME` or explicit `StorePath`. Its closed manifest and
code enforce same-hash idempotence; same-version/different-hash rejection;
corrupt-manifest, missing-binary, SHA, reported-version, invalid-version, and
missing-version refusal. Archives are user-local and not stored in Git.

This audit re-verified the explicit archived binary as `codex-cli 0.149.0` and
SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
Global `codex --version` was `codex-cli 0.149.1`. With the verified archive
passed through the existing host-selected executable path, `live_smoke`
completed with `RAH_CODEX_SMOKE_OK`. This confirms certified release evidence
is independent of the daily global executable. The precedence remains explicit
host-selected path, then legacy PATH/npm discovery only when the host supplies
or discovers `codex`; no model, tool, or profile selects the executable.

## Compatibility, dependencies, and APIs

`cargo metadata --no-deps --format-version 1` reports 11 packages, all version
`0.6.0`, all edition `2024`. The v0.7 commit range introduces no Rust dependency
edge. No Rust public API expansion was identified: parsing/normalization is
private and the RAH-owned Tool interface remains unchanged. The product schema
change is additive at the existing `repo.patch` capability: bounded
`replacements[]` is accepted alongside the legacy form.

`profile_version` remains `1`; trusted-profile capability name `repo.patch`,
binding semantics, permission mapping, canonical routing, private aliasing,
dispatch, dedupe, and no-replay behavior remain unchanged. Existing profiles
need no migration. ADR 0010 remains index-only and ADR 0012 is the correct
amended worktree-content authority; ADR 0013 is unnecessary.

## Release evidence matrix

| Gate | Evidence | Platform | Status |
| --- | --- | --- | --- |
| Multi-replacement unit tests | Task 072 | Windows + Ubuntu CI | PASS |
| Bridge deterministic | Task 073 | Windows + Ubuntu CI | PASS |
| Live Codex multi-patch | Task 074 | Windows | PASS |
| Architecture alignment | Task 075 | Documentation + CI | PASS |
| Certified baseline tooling | Task 076 | Windows + Ubuntu CI | PASS |
| Full workspace gates | Task 077 current head | Windows | PASS |
| Documentation consistency | Task 077 README review | Repository | **BLOCKED** |

Fresh current-head checks passed: `cargo fmt --check`, `cargo check
--workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `git diff --check`, and cargo metadata. Fresh
certified baseline verification and the optional explicit-baseline smoke passed.

## Known limitations that are not blockers

The bounded authority intentionally excludes new-file creation, deletion,
rename, multi-file transaction, arbitrary unified-diff ingestion, Git
commit/ref/history and network Git authority, network MCP, PluginManager,
dynamic profile reload, generic shell/process authority, rollback, and automatic
replay. Cancellation/timeout is not rollback; external provider supervision is
not OS sandboxing. Session/workflow persistence and protocol/schema compatibility
automation remain future work. Certified baseline tooling is Windows x64 first.
These are deliberate limitations, not v0.7 defects.

## Required next task

Perform a narrow documentation-correction task: update README’s v0.7 product
summary and security boundary wording from “exactly one” to the ADR 0012
bounded one-to-sixteen original-snapshot `replacements[]` contract, preserving
all excluded-authority and no-rollback statements. Then rerun the Task 077
audit gates. Do not begin release preparation, version changes, tagging, or
baseline promotion until that re-audit returns `RELEASE READY`.

## Task 079 Re-Audit

Date: 2026-08-24

Task: 079 — RAH v0.7 milestone re-audit / release-gate analysis only

Starting baseline: Task 078 `47a5b9bbf709e4d38d63a3406a80faf25c6490d6`
Task 078 CI: `32698501475` (completed / success)

### Blocker closure

Task 077 blocker: README contract mismatch. Its current-state product summary
and security-boundary list incorrectly limited `repo.patch` to exactly one
replacement, while ADR 0012 and the v0.7 implementation accepted the legacy
single form and a bounded `replacements[]` form with one through sixteen
items.

Resolution: Task 078 corrected both README locations. The current README now
states that the legacy single form remains supported; `replacements[]` permits
one through sixteen exact replacements in one existing HEAD-tracked,
unstaged, strict-UTF-8 file; all matches use the original snapshot;
non-overlapping edits are deterministic; SHA-256 and byte-length preconditions
are required; and no automatic staging occurs. It continues to exclude broad
filesystem write, shell/process, and Git history/ref authority.

**Was the Task 077 sole blocker resolved by Task 078?**

YES

The focused stale-wording search found only the corrected current README and
historical records. The v0.5/v0.6 roadmap and planning matches accurately
describe their earlier one-replacement scope, and the Task 077 passages
truthfully preserve its historical finding; none is a current-state claim.

### Contract and authority revalidation

README and ADR 0012 agree on the legacy form plus bounded multiple
replacements, one existing tracked file, original-snapshot matching, no broad
filesystem authority, uncertain mutation semantics, and no automatic replay.
The current implementation and tests retain the same contract: 1–16 items,
legacy compatibility, duplicate/overlap/repeated-source refusal, adjacent
ranges allowed, deterministic postimage construction, and SHA-256/length
preconditions.

The bounds remain aligned across documentation, code, and tests: 16
replacements; 64 KiB serialized request; 64 KiB aggregate replacement text;
64 KiB per replacement item; 1 MiB input file; and 1 MiB final output.

`repo.patch` remains `PermissionLevel::Execute`-gated and is still bound only
by a trusted profile to host-owned `RepositoryWorktreeMutationPolicy` and
`ToolRegistry` authority. No generic `fs.write`, generic shell/process,
model-selected executable/path, Git commit/history/ref, or network-Git
authority was introduced. `profile_version` remains `1`, with capability
`repo.patch` and no profile migration. The Generic Tool Bridge behavior and
Task 073 deterministic evidence therefore remain valid.

No relevant production behavior changed after Task 074, so its three fresh
certified native app-server runs, one three-item `replacements[]` call, exact
single execution counts, observer post-state checks, unchanged index/HEAD/refs,
restricted Codex-owned authority, `Completed` terminal state, and
`RAH_MULTI_PATCH_LIVE_OK` marker remain valid evidence. Task 075's platform
conclusions also remain valid: the native app-server is the primary boundary,
the SDK is optional future adapter convenience, RAH does not implement
inference, `ToolRegistry` is host-owned, MCP/Plugin are Tool providers, Codex
approval creates no RAH authority, Sessions reference Codex threads, and
authority is recomputed on resume.

### Fresh current-head checks

The certified Windows x64 baseline verified as `codex-cli 0.149.0` with
SHA-256 `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
The independent global daily executable reported `codex-cli 0.149.1`; it is
not a release requirement. With `RAH_CODEX_EXECUTABLE` explicitly set to the
archived certified executable, `live_smoke` launched that archive and passed
with `RAH_CODEX_SMOKE_OK` and terminal `Completed`.

Current-head deterministic gates passed: `cargo fmt --check`, `cargo check
--workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, and `git diff --check`. `cargo metadata
--no-deps --format-version 1` reports 11 packages, all version `0.6.0`, all
edition `2024`. Tasks 077–078 introduced no dependency edges, Cargo.toml or
Cargo.lock changes, unintended public Rust API changes, ADR change, or version
change.

The audit makes only supported platform claims: Windows live Codex validation,
Windows x64 certified-baseline management, and Ubuntu deterministic CI. It
makes no Unix/macOS live-Codex, Windows ARM64, or universal binary-portability
claim. Existing limitations remain intentional and documented: existing tracked
files only; no creation/deletion/rename, multi-file transaction, arbitrary
unified patches, Git history/ref or network Git, network MCP, PluginManager,
profile hot reload, generic shell/process, rollback, or automatic replay;
cancellation/timeout is not rollback; external process supervision is not OS
sandboxing; workflow/session persistence and schema-diff automation remain
future work; certified baseline tooling is Windows x64 focused.

### Release evidence matrix

| Gate | Evidence | Result |
| --- | --- | --- |
| Multi-replacement implementation | Task 072 | PASS |
| Deterministic bridge validation | Task 073 | PASS |
| Windows live Codex gate | Task 074 | PASS |
| Codex platform alignment | Task 075 | PASS |
| Certified baseline tooling | Task 076 | PASS |
| Milestone audit | Task 077 | blocker identified |
| README correction | Task 078 | PASS |
| Re-audit current gates | Task 079 | PASS |

**Did the re-audit discover any new release blocker?**

NO

### RELEASE READY

Task 079 confirms that the Task 077 documentation blocker is closed, no new
release blocker was found, and the v0.7 evidence and current gates remain
valid. This audit does not bump versions, mark a release, create a tag, or
perform release preparation.

Suggested next task: **Task 080 — RAH v0.7 Release Preparation**.
