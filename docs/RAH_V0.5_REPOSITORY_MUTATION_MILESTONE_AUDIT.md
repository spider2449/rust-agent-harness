# RAH v0.5 Repository-Mutation Milestone Audit

Date: 2026-08-22
Task: 053
Decision: **Complete with non-blocking limitations; accept ADR 0012.**

## Decision and implemented capability

ADR 0012 is ready to move from Proposed to Accepted. Its narrow authority was
implemented in `rah-tools`, hardened with deterministic adversarial fixtures,
composed only by the trusted host profile path, exercised through the generic
Codex bridge, and live verified on Windows with the pinned native
`codex-cli 0.149.0`.

The exact v0.5 capability is:

> RAH can conditionally replace exactly one literal text occurrence in one
> bounded existing HEAD-tracked, unstaged UTF-8 worktree file through a trusted
> host-composed `repo.patch` authority.

This does not grant generic file writes, model-selected host resources or
executables, index mutation, Git history/ref mutation, network Git, replay,
rollback, or OS sandboxing. No audit finding requires a code fix before release
preparation.

## Authority-boundary result

The reviewed implementation preserves all required inequalities:

```text
model request != authorization
PermissionLevel::Execute != worktree mutation authority
trusted profile != generic write authority
repo.patch != generic filesystem mutation
worktree mutation != index mutation != Git history/ref mutation
```

`RepositoryWorktreeMutationPolicy` is crate-private in `rah-tools`. The public
`RepositoryWorktreePatchTool::new` is a host API that requires host-supplied
absolute Git and repository paths; the model schema never carries either.
`TrustedStaticProfile` stores only symbolic executable/repository IDs and its
static pass does not construct the tool. The actual `rah-cli` effective composer
resolves those IDs and invokes the constructor, which alone creates the private
policy with fixed limits. The registry and bridge can dispatch an already
registered tool but cannot manufacture policy authority. Provider metadata does
not participate in this binding.

ADR 0010 remains index-only: its `RepositoryMutationPolicy`, `host.git.stage`,
and `host.git.unstage` do not write worktree bytes. ADR 0011 remains
composition-only: a profile configures the closed binding but cannot deserialize
the policy, alter limits, select raw paths, or bypass registry/permission/policy
checks. ADR 0012 is the sole decision that authorizes v0.5 worktree content
mutation.

## ADR 0012 evidence matrix

Classification applies to the implementation evidence, not merely ADR intent.
“Live” means the Task 052 Windows fixture exercised the successful end-to-end
path; it does not turn every adversarial case into a live claim.

| ADR requirement | Evidence | Classification |
| --- | --- | --- |
| Private, host-owned authority; model request is not consent | Private policy, host constructor, registry dispatch, profile/bridge denial tests | Implemented and deterministically verified |
| One `repo.patch` call and closed five-field schema | `RepositoryWorktreePatchTool`, parser rejects unknown fields/NUL/oversize | Implemented and live verified (Windows) |
| Excluded generic write/process, broad patch forms, create/delete/move, binary, retry, replay, rollback | No corresponding schema/tool path; direct bounded replacement only; refusal/uncertain fixtures | Implemented and deterministically verified; excluded scope is deferred by ADR |
| Canonical non-bare repository confinement | Root/.git identity and Git top-level/non-bare revalidation; traversal/alias fixtures | Implemented and deterministically verified |
| Existing regular HEAD-tracked target | HEAD tree parser accepts one regular blob only; untracked/directory/submodule cases refuse | Implemented and deterministically verified |
| One normal stage-0 entry equal to HEAD | `ls-files -s` stage-0 parse, equality and normal-tag checks; staged and unmerged cases are fixtures | Gap/blocker: implemented, but direct sparse/skip-worktree fixture is absent |
| Symbolic host-resource composition | Closed profile binding and real effective composer; raw authority fields reject | Implemented and live verified (Windows) |
| `Execute` is only an outer gate | Tool definition requires Execute; bridge None/Read/Write denials make zero mutation | Implemented and deterministically verified |
| Private `RepositoryWorktreeMutationPolicy` | Private module type; public profile exposes only symbolic binding | Implemented and deterministically verified |
| Full SHA-256 and byte-length preconditions | Raw-byte validation before decode and revalidation before commit; stale hash/length fixtures | Implemented and live verified (Windows) |
| Exactly one nonempty literal match; no fuzzy/first-match behavior | `match_indices`, missing/duplicate/no-op refusal tests | Implemented and deterministically verified |
| Strict UTF-8, NUL rejection, BOM and newline rules | Malformed UTF-8 and BOM/CRLF fixtures; NUL rejection and request-BOM rejection are source-inspected guards | Gap/blocker: implemented, but direct NUL/request-BOM fixtures are absent |
| Request/raw/postimage bounds | Fixed 64 KiB request/text and 1 MiB file/postimage limits; oversized fixtures | Implemented and deterministically verified |
| Same-parent exclusive temporary complete postimage | UUID sibling, `create_new`, flush, identity/bytes revalidation | Implemented and deterministically verified |
| Target/temporary identity revalidation | Before-commit and post-replacement identity checks; A-H race and tamper fixtures | Implemented and deterministically verified |
| Link, reparse, junction, and hard-link rejection | Ancestry/component checks; Windows reparse attributes; supported symlink/hard-link fixtures | Implemented and deterministically verified on Windows; Unix execution not claimed |
| One native replacement attempt | One `MoveFileExW` call site with no retry; attempt counters and lock/race fixtures | Implemented and live verified (Windows) |
| Known failure proof versus uncertain result | Intact preimage+temporary+cleanup yields known failure; ambiguous target/temp/post-state yields uncertain | Implemented and deterministically verified |
| No retry, replay, or rollback | Policy has one commit call; bridge duplicate/cancel/disconnect/uncertain tests execute once | Implemented and deterministically verified |
| Cancellation semantics | Before-entry cancellation executes zero tool calls; post-mutation cancellation is terminal cancelled without replay/rollback | Implemented and deterministically verified |
| Index, HEAD, and ref separation | Pre/post Git observations; deterministic and live fixture preserve index/HEAD/refs and unrelated file | Implemented and live verified (Windows) |
| Restricted Codex-owned capabilities | Bridge tests and live handshake disable shell, filesystem, MCP, process, network-tool, web, image, app, approvals | Implemented and live verified (Windows) |
| Output redaction and lifecycle cleanup | Bridge/output assertions; live app-server reap, no provider child, no temp sibling, fixture removal | Implemented and live verified (Windows) |

The two Gap/blocker classifications above are deterministic-coverage gaps, not
observed correctness or authority-boundary defects: the guards are present,
narrow, and exercised by adjacent parser/Git-state fixtures. They are
non-blocking under the stated v0.5 Windows release criteria, but a later
hardening task should add direct sparse/skip-worktree, raw-NUL, and request-BOM
fixtures. Unix replacement and identity branches are platform-gated source paths
only in this Windows audit; they are not live-verified claims.

## Deterministic evidence

Tasks 048 and 049 provide owned temporary-repository tests for success;
malformed UTF-8; BOM/CRLF preservation; stale hash/length; missing/duplicate
literal text; oversized postimage; untracked, staged, and non-stage-0 targets;
path traversal, aliases, `.git`, namespace, and ADS forms; directory and link
targets; hard links where reported; target and temporary identity substitution;
and index/unrelated-file preservation.

The hardening fixtures inject changes across the pre-commit phases (initial
path, Git/index, preimage, unique-text, temporary write, final target identity,
and immediately-before replacement) and verify refusal before an attempt. They
also exercise immediately-after replacement identity substitution, temporary
tampering/disappearance, known failure versus uncertain classification, one
attempt, temporary cleanup, and Windows delete-sharing locks. A lock whose
post-state is provably intact is known failure; missing or contradictory
evidence is uncertain. Direct sparse/skip-worktree, raw-NUL, and request-BOM
fixtures are the three limited coverage gaps noted in the evidence matrix; the
implementation guards were source-inspected and are non-blocking for this
Windows release baseline.

The policy lease serializes RAH calls for the captured repository identity. It
does not claim cross-process mutual exclusion. Revalidation narrows the race
window and makes incomplete or contradictory observations fail closed as
uncertain; it does not eliminate TOCTOU.

## Profile and bridge evidence

Task 050 verifies that static profile validation is non-spawning and
non-mutating. Effective composition performs construction and registration but
never calls `repo.patch`. Symbolic resources remain authoritative; raw roots,
policy parameters, shell/argv/environment, and resource-limit overrides are
not profile fields. Inventories are redacted, duplicate registration fails
closed, and a mixed-provider late failure reaps previously staged providers.
`repo.patch` has no persistent child/provider lifecycle of its own.

Task 051 uses the actual static loader and `rah-cli` effective composer, not a
second test composer. It preserves canonical `repo.patch` through the private,
deterministic `rah_tool_0` alias, requires Execute, and proves denial before
tool execution. Duplicate delivery returns the stored response after one
execution; invalid requests, cancellation, disconnect, and an uncertain result
are not retried or replayed. Public outputs remain redacted. The bridge stays
restricted to RAH dynamic tools; it cannot enable Codex-owned capabilities.

## Live and Windows evidence

Task 052 used exactly native `codex-cli 0.149.0`, passed the adapter's
app-server compatibility checks, loaded a trusted static profile, used the real
effective composer, and supplied a fresh registry containing only canonical
`repo.patch` at Execute. The bridge alias was `rah_tool_0`.

The Windows live fixture observed exactly one ToolRequested, ToolStarted, and
ToolFinished event; one actual tool invocation; one native replacement attempt;
the expected target-only mutation; unchanged index/HEAD/refs and unrelated
file; terminal Completed; and exact final marker `RAH_REPO_PATCH_LIVE_OK`.
It also verified redacted output, disabled Codex-owned shell/filesystem/MCP/
process/network-tool capabilities, no plugin/MCP child, app-server reaping, no
temporary sibling, and removal of the temporary repository. The evidence was
passed three times with fresh fixtures. This is **Windows live verification
only**.

Windows-specific implementation and deterministic evidence cover native
executable identity, canonical paths, unsupported namespace and ADS rejection,
reparse/junction/symlink rejection, volume-plus-file identity, hard-link count
rejection, same-volume `MoveFileExW` replacement with write-through, lock-based
known failure, ambiguity-to-uncertain classification, CRLF preservation, and
fixture cleanup. External filters, antivirus/indexers, cross-process races, and
all TOCTOU are explicitly residual limitations rather than safety claims.

## Platform status, limitations, and blockers

The current release baseline is Windows. Windows cfg paths were compiled and
tested by this audit's validation commands, and the native live test is Windows
only. Unix uses cfg-gated identity and same-filesystem rename code, but it was
not compiled or executed on a Unix host in this task and no Unix live claim is
made. The project release criteria do not require Unix live validation, so this
is a non-blocking platform limitation.

Intentional scope restrictions:

- one existing HEAD-tracked, unstaged, regular strict-UTF-8 file; one literal
  replacement; no binary changes, create/delete/move, multi-file transaction,
  staged-file edit, restore-worktree, index/history/ref mutation, or network Git;
- no generic `fs.write`, generic shell/process authority, MCP/network addition,
  PluginManager, profile hot reload, automatic retry, replay, or rollback.

Technical debt / residual limits:

- no OS sandbox or network isolation; external filters and processes may race;
- no complete TOCTOU exclusion or transactional rollback; uncertain effects
  require caller awareness and a fresh explicit request;
- Codex compatibility remains pinned to exactly `0.149.0`; Unix live coverage
  remains outstanding if a future release policy requires it.

Release blockers: **none for v0.5 release preparation.** The noted sparse/
skip-worktree, raw-NUL, and request-BOM fixture gaps are test-depth follow-up,
not a deterministic correctness, live validation, platform-validation, or
authority-boundary release blocker under the stated Windows baseline.

## Public API, dependency, and architecture impact

Since Task 047, the justified public surface is the host-constructed
`RepositoryWorktreePatchTool`, its stable `repo.patch` name constant, and the
closed host-only `RepositoryWorktreePatchProfile`/profile accessors needed by
the existing effective composer. The optional live-test replacement counter is
feature-gated and unavailable from the default production build. None exposes
`RepositoryWorktreeMutationPolicy`, mutable policy limits, raw model-selected
authority, Codex types, or provider metadata authority.

No dependency edge changed in Task 053. The existing `rah-tools ->
rah-protocol, rah-sandbox` edge remains appropriate for the private tool policy;
`rah-protocol` remains dependency-bottom. ADR 0012 is accepted. ADRs 0010 and
0011 remain unchanged in meaning. No architecture deviation was found.

## Recommendation

Proceed to **Task 054 — v0.5 release preparation and release gate**. It should
perform version, changelog, release-gate, and full-workspace validation work;
it must not introduce new mutation authority or features.
