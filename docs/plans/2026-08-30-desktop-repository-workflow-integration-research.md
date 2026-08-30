# Task 145 — Desktop Repository Workflow Integration Research

## Status

Research only. No Rust, frontend, Cargo, profile, bridge, dependency, version, or ADR change is proposed by this document.

## Starting checkpoint

At research start, `HEAD` and `origin/master` both were
`aa2f2d32d7c83033cbd7ff4d24abd64c96f3330c`; `git status --short` contained
only `?? .vscode/`, which remains untouched. The product baseline is 12
packages, version 0.11.0, edition 2024, certified Codex 0.149.0. RAH v0.11.0
remains released. Task 144 exact-head CI `33302269295` passed.

## Adopted Task 144 scope

The adopted primary scope remains Desktop end-to-end repository
authoring/review/commit. It uses existing ADRs 0010, 0011, 0012, 0013, 0014,
and 0016; it does not select a new authority or ADR.

## Current Desktop construction

`choose_repository` discovers host-selected native Git, canonicalizes the
selected folder through `DesktopRepository::new`, replaces the repository,
increments `repository_generation`, selects the repository persistence
namespace, and clears in-memory conversation context. `DesktopRepository`
retains canonical display/root paths, native Git, and status, worktree-diff,
and staged-diff observers. `repository_snapshot` executes those observers and
renders normalized output.

On Connect, Desktop snapshots repository/model generations, builds a private
registry, resolves the certified Codex executable, and connects the Codex
Generic Tool Bridge with the selected canonical root (or neutral workspace).
With a selected repository, its permission set is exactly `None`, `Read`, and
`Execute`; without one it is `None`. Disconnect shuts down the runtime.
Repository/model generation mismatches require reconnect before chat/resume.
Resume restores bounded transcript pairs only; it does not restore runtime or
tool state. The current Tauri capability is a closed command list and has no
stage, unstage, authorization, or commit command.

Safe future workflow state belongs only in `DesktopAppState`, keyed to selected
repository and runtime/model generations: repository observation/review display
state and, later, an in-memory `RepositoryCommitControl`. It must be cleared
with those generations, never placed in JS or SQLite.

## Existing public rah-tools seams

`rah-tools` publicly exports `GitStageTool`, `GitUnstageTool`,
`RepositoryCommitTool`, and `RepositoryCommitControl`. The current Desktop
registry returns only `Arc<ToolRegistry>` and registers exactly:

* `echo` always;
* with a repository: `fs.read`, `repo.file-info`, `repo.status`, `repo.diff`,
  `repo.diff-staged`, `repo.patch`, `repo.create-file`, and `repo.edit-files`.

It does not register `host.git.stage`, `host.git.unstage`, or `repo.commit`.

`RepositoryCommitTool::compose(git, repository, name, email)` is public and
returns `(RepositoryCommitTool, RepositoryCommitControl)` with no authorization
armed. The supplied Git/repository/identity are host configuration; the Tool
accepts only a bounded `message`. The paired control is not a Tool,
serializable value, or model-visible capability.

## Composition alternatives

| Route | Result |
| --- | --- |
| A. Desktop directly constructs public rah-tools capabilities | Recommended. It reuses the existing policy implementation and retains the host-only control in Desktop Rust state. |
| B. Extract a neutral CLI composer | Not required. Consider only if a future public seam proves insufficient. |
| C. `rah-desktop -> rah-cli` | Rejected: application-to-application dependency reverses the intended direction and imports profile/provider lifecycle concerns. |
| D. Copy `rah_cli::profile_composition` | Rejected: duplicates authority-sensitive composition and unnecessary TrustedStaticProfile/provider logic. |

The CLI composer interprets `TrustedStaticProfile`, resolves symbolic resources,
constructs provider adapters/effective inventory, owns their lifecycle, and
retains a commit control. Those are profile-specific host composition concerns,
not the commit policy itself. Desktop already owns its selected canonical
repository, exact native Git, runtime generation, model selection, and private
registry. It does not need `TrustedStaticProfile` for this fixed workflow.

## Recommended composition route

Desktop should continue fixed explicit host composition and construct existing
public tools directly. At connection time, future Desktop can compose:

```rust
let (commit_tool, commit_control) = RepositoryCommitTool::compose(
    exact_native_git, exact_selected_repository, explicit_host_name, explicit_host_email,
)?;
```

then register `commit_tool` and retain `commit_control` solely in Rust state for
that runtime/repository generation. This neither reimplements ADR 0016 policy
nor creates parallel authority semantics. Execute remains the existing outer
bridge permission; no Commit permission level is needed.

`DIRECT_RAH_TOOLS_COMPOSITION_FEASIBLE = YES`

`TASK_146_SHARED_COMPOSITION_REQUIRED = NO`

Therefore Task 147 is unblocked. There is no Task 146 implementation work.

## Stage/unstage integration model

`GitStageTool::new` and `GitUnstageTool::new` take native Git, an absolute
repository root, a host-selected symbolic target, and an absolute target path.
Each binds one canonical tracked regular file. Model input is exactly `{}`.
The policies share the per-repository RAH lease, revalidate repository/metadata
and target identity, capture pre/post state, verify the narrow index result,
and return `ok`, `failed_known`, `uncertain`, or policy-violation semantics.

Desktop should use explicit human host actions, not model-visible tools:

`DESKTOP_STAGE_MODEL = HUMAN_HOST_ACTION`

`DESKTOP_UNSTAGE_MODEL = HUMAN_HOST_ACTION`

The source chain is: Rust obtains bounded changed-file observations; frontend
renders a display entry; a user selects Stage/Unstage; frontend submits only an
opaque observed-item reference (or bounded relative path plus observation
generation); Rust validates it against the current host observation and its
canonical repository-relative identity; Rust constructs the single-target tool,
executes it once, then refreshes the snapshot. A frontend path string is never
authority. v0.12 should offer one changed tracked file per click, not “Stage
all”: the accepted authority is one target and does not establish a generic
multi-target index operation.

## Commit identity source

Desktop currently has no explicit commit name/email configuration; its current
preferences persist only validated model selection. The recommended v0.12
source is a persisted, validated Desktop preference containing explicit name
and email, entered/changed by the human and read only by Rust at composition.
It is an ordinary host configuration preference, not an authorization source.
It must never fall back to repository/global Git config, OS username, model
text, or provider metadata.

`COMMIT_IDENTITY_SOURCE = persisted validated Desktop preference: explicit human name and email`

Persisting identity must not restore a repository, connection, tool permission,
pending authorization, or automatic Connect/commit.

## Staged review contract

Before Authorize, Desktop must present canonical repository display identity,
branch, abbreviated display HEAD while retaining full Rust identity, staged
file paths/change kinds/modes, added/deleted counts, textual patch where
available, explicit binary metadata, and a fresh/stale review state. Internal
authorization hashes are not displayed.

`repo.diff-staged` fixes index-versus-HEAD and accepts only `{}`. Its shared
foundation obtains raw, numstat, and patch streams under the RAH lease, checks
HEAD before/after, correlates all streams exactly, and rejects capture,
processing, output, file-count, path, and patch limits rather than returning a
successful truncated patch. Thus:

`STAGED_REVIEW_COMPLETE_ON_SUCCESS = YES`

`REVIEW_OUTPUT_OVERFLOW = FAIL_CLOSED`

Binary records are structurally normalized (`binary`, no line counts, and no
textual patch), while ADR 0016 can admit ordinary tracked binary files. For
v0.12 the safer product restriction is to refuse Desktop authorization if the
reviewed staged scope contains unrenderable binary content. This is a Desktop
layer narrowing, not an ADR 0016 change.

`BINARY_STAGED_COMMIT_POLICY = REFUSE_IN_V0_12`

## Review-to-authorization race analysis

A displayed review identity must be host-owned and include repository and
runtime/model generations, canonical repository identity, branch, full HEAD,
ordered normalized staged-file identities/modes/change kinds, and a stable
digest of the complete normalized staged review. The frontend receives only
display data and a stale/fresh outcome, never an authorization token, index
hash, tree OID, UUID, or serialized authorization.

The current `authorize_current_reviewed_snapshot()` captures the current ADR
0016 compound snapshot while holding the shared RAH lease, but it accepts no
expected review descriptor. Desktop cannot safely do “re-observe and compare,
then call current authorize” as two operations: another RAH actor may acquire
the lease between them, so authorization B could differ from displayed A.
The shared lease serializes RAH observers/mutations/commit policy; it cannot
exclude external Git processes.

Task 148 therefore needs the smallest host-only rah-tools addition: a
nonserializable expected-review descriptor created from a host review
observation, plus a compare-and-authorize control method. It must acquire the
same repository lease once, re-observe/compare the expected displayed review,
then capture and store the existing ADR 0016 authorization before releasing
the lease. It returns only a redacted match/stale result. It is not Tool input,
not a generic hash constructor, not model-visible, and grants no authority.
ADR 0016’s existing final pre-spawn revalidation still protects later changes.

`REVIEW_AUTHORIZATION_BINDING = host-only expected complete review descriptor; under one RAH lease compare current review then capture ADR-0016 authorization`

## Rust ownership/lifetime and invalidation

Future Desktop Rust state should own a `RepositoryCommitControl` only alongside
the matching private registry/runtime generation and redacted pending status.
Frontend may show “Commit authorized for reviewed staged snapshot,” never the
authority material. Any pending authorization disappears on repository change
or removal, disconnect, reconnect, model/provider/configuration generation
change, Codex executable/runtime change, Git identity change, branch/HEAD/index
change, stage/unstage, reviewed-file staging-affecting edit/create, Resume,
application/Desktop restart, and persistence/migration load. Before detected
staleness is refreshed, UI shows stale/review-required; a consumed or absent
authorization yields existing `precondition_failed`.

SQLite may retain chat and display history, but cannot persist/reconstruct a
pending authorization, commit token, reviewed authorization, or tool authority.

`AUTHORIZATION_PERSISTED = NO`

`AUTHORIZATION_MODEL_VISIBLE = NO`

## Model Tool lifecycle and result presentation

The intended sequence is: stage; complete staged review; explicit human
Authorize; Rust arms one snapshot; UI shows redacted pending state; conversation
asks for a commit; model calls existing `repo.commit` with `{"message":"..."}`;
Generic Tool Bridge dispatches under Execute; the Tool consumes authorization;
and the independently verified result appears in that conversation. The human
authorizes snapshot only; the model controls only the bounded message, never
repository, branch, HEAD/tree, identity, Git, hooks/date, or authorization.

A direct host Commit button is rejected because it bypasses the selected
Generic Tool Bridge lifecycle (`ToolRequested`, `ToolStarted`, `ToolFinished`)
and message-only Tool semantics.

`DIRECT_HOST_COMMIT_BUTTON = NO`

Current Desktop activity output records only tool name and generic
success/failed status; it discards `ToolOutput` content. Task 149 therefore
needs the smallest Desktop-only, sanitized `repo.commit` result presentation
from `ToolFinished`: redacted status and `commit_oid` only for
`committed_verified`, in the same activity/conversation surface. No ToolOutput
schema or bridge change is necessary. For `invalid_input`, `precondition_failed`,
`known_no_effect`, `committed_verified`, and `uncertain`, pending authorization
is already consumed before policy execution; UI clears it. In particular,
`uncertain` requires manual inspection/refresh and never offers retry/replay.

Minimum visible states are: no staged changes; staged/review required; review
stale/refresh required; reviewed/not authorized; authorized/awaiting one Tool
call; commit succeeded; known-no-effect/review again; and uncertain/manual
inspection/no replay.

## Dependency graph

Current relevant direction is:

```text
rah-desktop -> rah-tools -> rah-protocol, rah-sandbox
rah-cli     -> rah-tools, rah-tools-mcp, rah-tools-plugin
rah-tools-mcp -> rah-tools
rah-tools-plugin -> rah-tools
```

Desktop does not depend on CLI. The recommended route preserves this graph and
does not add dependencies.

## Security invariants

The integration uses only ADR 0010 index mutation, ADR 0012 patch, ADR 0013
create, ADR 0014 edit, ADR 0016 commit, and ADR 0011 trusted host composition.
No model/frontend/transcript authority is added; no profile schema or Generic
Tool Bridge behavior changes; no raw authorization material crosses the
Rust/JS boundary. External Git remains able to race RAH, so both pre-arming
comparison and ADR 0016 final verification fail closed rather than claiming
exclusive process control.

`NEW_AUTHORITY_REQUIRED = NO`

`NEW_ADR_REQUIRED = NO`

## Deterministic test plan

Future tests must cover: stage/unstage one observed tracked file; rejection of
wrong/stale observation and repository-generation change; review overflow
failure; binary refusal; changed review/HEAD/index before authorization;
stage/unstage review invalidation; reconnect invalidation; Resume/restart not
restoring authorization; one authorization consumed once; uncertain never
replayed; redacted commit result; and proof that frontend data cannot recreate
authority. State-machine tests are portable; Windows Tauri/live certification
is separate.

## Windows live plan

Task 151 should use a disposable local repository, Desktop release/debug host,
native Git, certified Codex 0.149.0 with its same-version code-mode host, and
a selected repository. Have the model edit/create a known file; use the human
Stage action; display review; explicitly authorize; make one `repo.commit`
call; observe `committed_verified` in the same conversation; verify the actual
Git commit and absence of a second commit; then narrowly test reconnect/restart
invalidation and clean up. This task makes no live claim.

Cross-platform deterministic tests are portable, but live Desktop support is
not claimed beyond Windows. Risks needing separate live evidence include
Windows reparse points, Unix symlinks, path case rules, native Git identity,
process supervision, and Tauri automation.

## Proposed task boundaries

Task 146 is skipped: no shared composition extraction is necessary.

Task 147 — Desktop Repository Index Workflow + Staged Review Foundation:
Rust-owned per-file Stage/Unstage, current-observation target validation,
snapshot refresh, staged-review descriptor/display state, overflow/binary
fail-closed UX, and no commit control arming or `repo.commit` registration.

Task 148 — Rust-side Reviewed Snapshot Authorization UX: expected displayed
review matching, minimal host-only compare-and-authorize control support,
one-shot control arming, redacted pending state, and invalidation. No Tool
registration change if it remains Task 149.

Task 149 — Desktop `repo.commit` Integration: register the existing composed
Tool; retain control; use existing Generic Tool Bridge and Execute gate;
present sanitized verified results in conversation; no new authority.

## Explicit non-goals

No branch/ref/checkout, delete/rename, multi-repository, network MCP,
PluginManager, profile reload, generic Git/shell, direct host commit, automatic
staging, authorization persistence, TrustedStaticProfile adoption, transport
confinement claim, Codex baseline change, version/tag/release work, or Task 120
network proof is included. `RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`;
transport confinement remains `NOT CLAIMED`.

## Recommendation

Proceed directly to Task 147 after this docs-only research. Direct public
`rah-tools` construction is safe and avoids both a CLI dependency and duplicated
authority-sensitive composition. Preserve ADR 0016 as authoritative and add
only the narrow host-only review precondition needed in Task 148 before any
Desktop authorization control can arm a commit.
