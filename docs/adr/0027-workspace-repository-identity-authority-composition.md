# ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary

Status: Accepted

Date: 2026-09-12

## Context

RAH v0.25.0 has one selected `DesktopRepository`, one repository generation,
one repository workflow state, one reviewed Commit capability, one
repository-bound first-party `ToolRegistry`, one repository binding in
Effective Authority, and one active connected composition. HostExplicit uses
the global `HostInvocationCoordinator` and process-local currentness.

Task 308 selected multi-repository workspace/repository selection as the v0.26
product direction. Task 309 independently researched the boundary and reached
**Outcome A — MULTI-REPOSITORY BOUNDARY VIABLE**, selecting Option A: admitted
descriptive membership, one active repository, and a fresh active-only
composition.

The current implementation review found one important prerequisite: nested
`.git` boundary rejection is not uniform across all existing repository-bound
read and mutation paths. The current reviewed rename path has a nested-boundary
check, while `fs.read` and other paths do not yet provide uniform evidence.
This ADR therefore makes uniform nested-boundary enforcement a later
conformance gate. It does not claim that v0.25 already implements it and does
not broaden `WorkspacePolicy`.

## Decision

RAH v0.26 may represent multiple explicitly admitted repositories in one
descriptive workspace, subject to this boundary:

```text
host-owned descriptive workspace membership
  -> fresh process-local repository identity per admitted root
  -> zero or one explicit active repository
  -> one fresh current repository-bound composition
  -> existing Tool, policy, ticket, index, and reviewed Commit authorities
```

Workspace membership is descriptive organization, not authority. Repository
identity scopes repository-bound authority. Exactly one admitted repository may
supply current executable repository composition at a time in the v0.26
foundation. Changing that repository withdraws old prepared/current executable
state and requires fresh composition. No Tool or model input selects
repository authority.

This ADR wraps existing authority boundaries. It adds no new mutation
capability, no HostExplicit eligibility, and no provider or network authority.

## Workspace identity and membership

A workspace is a host-owned organizational container. It is not a filesystem
root, a Git root, a permission, an authority object, or a ToolRegistry.

The process may hold an opaque process-local coordination identity and
activation epoch. It may also persist inert descriptive state, such as:

- schema version;
- an opaque descriptive workspace ID;
- a bounded, control-free workspace label;
- bounded descriptive member IDs and user labels;
- an optional privacy-approved location hint; and
- an optional last-active descriptive member ID.

Membership is not permission. A remembered member is remembered, not admitted
or current. Persistence never authorizes a repository or restores executable
state. Automatic filesystem scanning, nested Git discovery, repository
contents, MCP, Process Plugin, provider metadata, model output, and frontend
authority metadata cannot add membership.

An empty workspace and a zero-active-repository setup/transition state are
valid. The host/human explicitly admits and selects members; the frontend may
request those actions but cannot decide them.

## Repository identity and admission

Admission gives a repository a fresh process-local, host-owned opaque handle.
The authority identity is private evidence, not a display path alone. Where
required by the capability, the binding may include:

- canonical existing repository root;
- root filesystem identity;
- supported `.git` form and `.git` filesystem identity;
- Git executable identity;
- supported ordinary-worktree/non-bare classification;
- fresh host admission epoch and per-repository generation/currentness; and
- capability-specific policy, preparer, and target identities.

Absolute/native paths and filesystem identities remain private authority
evidence. A basename, display label, descriptive member ID, persistence
namespace, or presentation `RepositoryIdentity` is not authority identity.

Admission is an explicit host/human operation. A native picker or typed
backend-owned request may propose a path; only the host validates and admits
it. Admission never results from model output, Tool input, MCP, Process
Plugin, repository content, automatic filesystem scanning, nested Git
discovery, provider metadata, or frontend-supplied authority metadata.

The host must freshly validate the canonical root, supported `.git` form,
filesystem identities, Git binding, repository state, and the authority
constructors needed for the member. Missing, malformed, unsupported, or
ambiguous evidence fails closed.

Same spelling does not imply the same repository. A moved repository, replaced
root, replaced `.git`, changed Git executable, or changed identity becomes
stale and requires fresh admission. Restart creates a new process-local
authority context. Physical continuity observed after a move is descriptive
evidence only and never revives tickets, preparations, Commit authorization,
or a runtime connection.

Case-equivalent, drive-letter, canonical, and supported alias-equivalent paths
refer to one membership candidate after host identity validation; they never
create a second authority member. Duplicate or alias-equivalent admission is
rejected as `already_member` without focus or activation side effects.

The v0.26 foundation rejects nested repository co-membership. If two canonical
roots have a parent/child repository relationship, admitting the second to the
same workspace is rejected as a bounded nested-member conflict. There is no
automatic discovery. Independently, every repository-bound path capability
must fail closed when its target ancestry crosses a nested `.git` repository
boundary, even when the nested repository is not a workspace member. This
includes reads, observation, worktree mutation, creation, deletion, rename,
and index-target validation. The uniform implementation of this invariant is
a prerequisite for later implementation tasks.

The foundation does not claim linked-worktree support. Unsupported linked
worktrees, `.git` indirection, path forms, reparse aliases, device/ADS
ambiguity, or filesystem identity that cannot be safely established are
rejected according to the applicable capability policy. These checks do not
claim race-free TOCTOU.

## Active repository and composition

At most one repository is active for ordinary execution. The active repository
is the repository for which a fresh, current, executable composition exists;
it is not a generic permission, workspace root, model-selected target, or Tool
argument. Zero active repositories is valid during setup, transition, failed
admission, or withdrawal.

Changing the active repository withdraws the old executable composition and
builds a fresh composition for the new repository. The v0.26 foundation does
not retain executable registries for inactive members and does not maintain
parallel per-repository HostExplicit queues.

Option A is accepted:

- one active repository;
- one fresh repository-bound first-party composition;
- one current executable ToolRegistry path; and
- existing global provider composition only within the explicitly connected
  host-owned runtime lifecycle.

There is no workspace-wide union ToolRegistry, repository-qualified model Tool
name, repository ID in an ordinary Tool input, generic invoke-on-member route,
or model-selected repository dispatch. Existing ToolRegistry lookup,
permission checks, D2, capability-specific policy, repository lease, and
provider duplicate-name fail-closed behavior remain authoritative.

Effective Authority may expose only the active repository's executable
authority. Inactive members may be shown descriptively, but descriptive state
must not imply current executable authority. No union Effective Authority
object makes all member capabilities appear simultaneously executable.

Trusted Profile remains global host-owned authority composition. MCP and
Process Plugin remain Tool providers. Provider metadata cannot admit, select,
or access a repository through membership, and RAH does not create one
provider Tool instance per workspace member merely for convenience.

## Generations and currentness

The exact Rust field layout is intentionally not fixed here, but currentness
must remain separable for:

- descriptive workspace membership generation/epoch;
- active-selection generation/epoch;
- per-repository generation;
- model generation;
- Trusted Profile generation;
- connection generation; and
- composition/registry identity.

Every operation proves its own repository identity and currentness. An
unrelated change to Repository B must not grant, migrate, or accidentally
refresh authority for Repository A. If repository identity, root identity,
`.git` identity, active selection, registry/composition identity, required
generations, permission, D2, or capability-specific preparation cannot be
proven current, execution fails closed. Missing membership is not equivalent
to `PermissionLevel::None`; inactive is not executable.

## HostExplicit and running work

HostExplicit authorization remains process-local, opaque, capability-specific,
single-use, repository-bound, currentness-bound, and nonpersistent. Switching
the active repository invalidates prepared HostExplicit state. There are no
workspace tickets, retained inactive-member tickets, or per-repository
parallel prepared queues. The existing global coordinator remains the
foundation.

An active-repository switch or member removal is blocked while a HostExplicit
effect is running where current lifecycle controls permit. A running effect is
never migrated. Once an effect has Started, selection/removal is not rollback;
timeout, cancellation, disconnect, crash, or a lost response does not prove no
effect and never permits replay. Existing uncertainty and ownership semantics
remain unchanged.

At model request start, the host captures one immutable active repository
identity/currentness and the relevant model, profile, connection,
conversation, runtime, and composition state. Switching active repository is
blocked while a model turn is active. A later Tool request remains constrained
to the captured active repository's registry; it cannot migrate to another
repository. Model output, provider output, and Tool input cannot select or
admit a repository.

## Commit and Stage/Unstage

Reviewed Commit authorization remains bound to one repository, exact staged
snapshot, attached branch/parent, identity, and currentness. There is at most
one executable reviewed Commit authorization in the foundation, for the active
repository. Switching away, removal, restart, or relevant identity/currentness
change withdraws it. Returning requires fresh observation, review, and
authorization. There is no workspace-wide Commit authorization, and
`repo.commit` is not HostExplicit-eligible.

Stage and Unstage remain separate repository-bound index authorities. An action
binds the member/repository identity, repository and active-selection
generations, observation generation, canonical target, target identity, and
action kind. Switching invalidates old A actions and staged review state; an A
action can never execute against B. Returning to A requires fresh observation
and reselection. Index actions are never unioned across members.

## Conversation and persistence

The initial v0.26 conversation model is Task 309's selected model: separate
conversation context per admitted repository member, with an optional
workspace-level descriptive overview. A repository transcript is descriptive
model context only. It grants no authority, and B does not receive A's history
by default. Each model turn captures one repository identity/currentness before
starting.

Persistence is inert descriptive state. It may remember bounded workspace and
member metadata, labels, opt-in location hints, descriptive transcript data,
and last-active intent. It must not persist or reconstruct:

- authority identities or filesystem identities as executable proof;
- ToolRegistry or Effective Authority objects;
- permission or D2 decisions;
- HostExplicit tickets, preparations, or retained preparers;
- Commit authorization or Stage/Unstage capabilities;
- runtime connections, provider credentials, or private provider lifecycle;
  or
- uncertain-effect continuation, replay, rollback, or recovery authority.

Restart requires fresh host admission and fresh composition before any
repository becomes executable/current.

Removing an inactive member removes future reachability and its descriptive
membership without unnecessarily invalidating unrelated active authority.
Removing the active member withdraws the active composition, clears its
repository-bound prepared/Commit/index state, and enters zero-active state.
Removal during a Started effect does not roll back, prove no effect, or permit
replay; where required by lifecycle controls, removal is rejected as busy.

## Privacy

Explicit repository-management UI may show a user-approved path, label, or
basename. That presentation choice does not make the path authority-bearing.
Generic activity, model-visible state, provider metadata, serialized authority
snapshots, and persisted workspace state must not automatically expose
absolute native paths, filesystem identities, `.git` paths or identities, Git
executable identity, authority handles, private membership proof, workspace
storage paths, or credentials.

Future privacy tests must inspect serialized values, not merely property names:
actual sentinel paths, `.git` values, filesystem IDs, ticket strings, and
credentials must be absent from every field of generic activity, model-visible
state, persistence, and Effective Authority snapshots.

## Cross-repository boundary

The v0.26 foundation rejects cross-repository rename, move, copy, patch/edit
bundles, Stage, Unstage, Commit, transactions, rollback/compensation,
automatic sibling edits, dependency synchronization, repository selectors in
Tool inputs, shared HostExplicit tickets, and shared Commit authorization.
Every effectful operation remains scoped to exactly one repository. Parallel
multi-repository mutation is a separate future research topic.

## HostExplicit eligibility

Acceptance changes no HostExplicit eligibility. The exact production set
remains these 11 names:

```text
fs.read
repo.file-info
repo.status
repo.diff
repo.diff-staged
repo.create-branch
repo.patch
repo.edit-files
repo.create-file
repo.delete-file
repo.rename-file
```

`repo.create-directory`, `repo.commit`, MCP Tools, Process Plugin Tools,
fixture/diagnostic Tools, unknown Tools, and provider-defined Tools remain
ineligible. No category, prefix, permission, provider, or effect-class
eligibility rule is introduced.

## Rejected alternatives

### Current single-repository replacement model

This is safe and is the v0.25 baseline, but it does not deliver the v0.26
multi-repository workspace UX.

### Option B — retained per-repository executable compositions

Retaining registries, preparers, provider/runtime associations, and lifecycle
state for inactive members creates a larger currentness, revocation, ticket,
privacy, shutdown, and provider-isolation surface than the foundation needs.

### Option C — workspace-wide union registry

A union makes model-visible Tool names or repository-qualified inputs act as
authority routing. It increases confused-deputy and collision risk and weakens
the active repository as a structural boundary.

### Workspace-root filesystem authority

A workspace root would create forbidden generic filesystem authority and make
membership a permission-like capability.

### Nested auto-discovery

Repository contents and filesystem layout cannot grant authority. Auto-
discovery would also make parent safety depend on mutable membership state.

### Persisted authority restoration

Restoring registries, tickets, preparations, Commit state, or effects would
create durable resumable authority contrary to the HostExplicit/currentness
model.

## Relationship to existing ADRs

ADR 0027 is additive and supersedes none of the existing authority decisions.
It explicitly preserves:

- ADR 0003 — built-in, MCP, and Process Plugin capabilities converge through
  RAH-owned `Tool` and `ToolRegistry`;
- ADR 0011 — Trusted Profile is host-owned, bounded, validated composition;
- ADR 0016 — exact repository-bound, reviewed, nonpersistent Commit authority;
- ADR 0018 — bounded single-repository rename/move authority;
- ADR 0021 — generic HostExplicit coordinator, currentness, ticket, D2, and
  provenance boundary; and
- ADRs 0022–0026 — capability-specific reviewed HostExplicit routes.

Those ADRs remain authoritative for their existing contracts. ADR 0027 wraps
repository identity, admission, selection, and composition around them; it
does not amend their mutation semantics, eligibility, provider composition,
Trusted Profile behavior, or Commit/Stage/Unstage behavior.

## Future conformance and sequencing

Task 309's future deterministic matrix of 28 scenario groups remains required;
this ADR does not reduce it. It must continue to cover admission and
isolation, duplicates and aliases, switch invalidation, ticket and model-turn
binding, Commit and Stage/Unstage binding, inactive/active removal, restart,
malformed persistence, moved/replaced roots and `.git`, nested boundaries,
privacy sentinels, provider isolation, the unchanged 11-name set, platform
identity cases, and no replay after uncertain effects. The matrix must use
fresh disposable repositories, assert A and B state independently, count
Tool/native attempts, inspect actual serialized values, and fail closed on
ambiguous observation.

Task 309's future two-repository Windows certification plan remains separate
and is not performed by this ADR. It must establish fresh host admission,
active-only composition, A/B isolation, ticket rejection under B, provider
and model non-selection, restart non-restoration, and no replay after
uncertainty. It must not claim race-free TOCTOU, rollback, OS sandboxing,
network isolation, Linux/macOS live parity, or model-selected HostExplicit
certification.

The largest implementation prerequisite is a focused repository nested-boundary
conformance audit/research task across existing repository-bound path
capabilities. Task 311 should begin there. If that audit requires corrections,
those boundaries must be fixed and tested before inert workspace membership
and explicit admission. If it proves the existing authorities sufficiently
enforce the accepted invariant, the next bounded step is inert membership and
explicit host admission, followed by active-only composition/currentness,
Desktop presentation, deterministic isolation/privacy/restart gates, and only
then separately authorized live certification and milestone/release gates.

## Nonclaims

This ADR does not claim or authorize:

- workspace-wide filesystem or Git authority;
- cross-repository operations or transactions;
- automatic discovery or nested-repository support;
- linked-worktree support;
- model/provider/frontend-selected admission or repository authority;
- persistent executable authority, tickets, preparations, or Commit state;
- resume/replay, rollback, compensation, or race-free TOCTOU;
- OS sandboxing, network isolation, or Linux/macOS live certification;
- parallel repository mutation;
- HostExplicit `repo.commit` or `repo.create-directory`;
- network MCP, `PluginManager`, or dynamic Trusted Profile reload; or
- implementation, frontend/Tauri changes, Tool schema changes, or live
  certification as part of this decision.

No implementation is performed by acceptance of this ADR.
