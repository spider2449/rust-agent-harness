# Task 322 — RAH v0.26 Multi-Repository Remaining Product Scope Decision

Date: 2026-09-13

Decision type: research and product-scope decision only

## 1. Decision

**Recommendation A — STOP FEATURE EXPANSION FOR v0.26.**
The current explicit multi-repository admission and switching slice is
coherent, useful, and sufficiently distinct to define v0.26. Membership loss
on Desktop restart and the absence of a removal control are real product
costs, but they are management conveniences, not authority or safety gaps.
They do not justify reopening the repository, connection, currentness,
provider, or lifecycle surfaces before milestone audit and certification.

The correct v0.26 story is:

> RAH Desktop can explicitly admit multiple repositories during one process
> and safely switch one active repository at a time, with active-only
> authority composition.

Persistence and removal remain valuable v0.27 candidates. They should be
researched as separate follow-ups if user evidence later shows that session
scoping materially limits adoption.

## 2. Authoritative checkpoint

The decision starts from the supplied, verified checkpoint:

| Item | Value |
| --- | --- |
| Branch | `master` |
| Current master | `52c6d7386a62be51ec2735c883f1096aa24dc44c` |
| Direct parent | `ed52503f7c0441e01f4bade97ac058abca9ec511` |
| Task 321-I exact-head CI | `34758895718` — PASS |
| Task 321-I outcome | Outcome A — REAL CONNECT / INDEX EVIDENCE CLOSED |
| Task 321-H disposition | Verdict B only for deterministic evidence fidelity; closed by Task 321-I |
| Accepted architecture | ADR 0027 — Workspace/Repository Identity and Authority-Composition Boundary |
| Published release | RAH v0.25.0 |
| v0.25 immutable source | `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4` |
| v0.25 annotated tag object | `ea3c31aaf5190b632d7ef86387f7aff6004ae664` |
| v0.25 GitHub Release | `387406579` |
| Workspace metadata | 13 packages, version `0.25.0`, Rust edition 2024 |
| HostExplicit eligible set | Exactly 11 names |

Task 321-I corrected the evidence-fidelity gap by exercising the real
`begin_repository_index_effect` reservation path and the real
`publish_connected_provider_state` publication path. No further Task 321
re-audit is required by this decision unless new evidence is discovered.
Task 321-I did not add persistence, removal, a public contract, a provider
protocol, or a new authority.

This document does not claim v0.26 certification and does not perform live
certification.

## 3. Current v0.26 product slice

Tasks 308–321-I establish the following bounded product:

- explicit human repository admission;
- process-local workspace membership;
- duplicate, alias, and nested-membership rejection;
- opaque process-local member selectors;
- one active repository at a time;
- explicit human A-to-B and B-to-A switching;
- fresh active-only repository composition;
- no union `ToolRegistry`;
- repository-bound HostExplicit currentness;
- repository-bound reviewed Commit authorization;
- repository-bound Stage/Unstage lifecycle;
- fresh conversation behavior on switching;
- correct Connect, Disconnect, model, identity, and provider lifecycle
  ordering after the Task 321-I evidence correction;
- nested `.git` boundary protection;
- inert inactive membership; and
- restart with no restored executable repository authority.

The implementation shape is consistent with ADR 0027:

```text
explicit host/human admission
  -> process-local inert member record
  -> explicit member selector
  -> one fresh active repository composition
  -> existing repository-bound Tools, policies, tickets, index, and Commit
     authorities
```

`WorkspaceMembershipState` is initialized empty for a new Desktop state. An
admitted member retains private host-side root and admission identity data,
but the public membership presentation contains only bounded opaque member
ID, safe display name, active state, availability, and membership generation.
An inactive member contributes no `DesktopRepository`, registry, provider,
runtime, HostExplicit preparation, Commit control, or Stage/Unstage state.

The active switch is a host lifecycle operation, not a Tool operation. A
successful switch withdraws the old executable composition, invalidates old
repository-bound state, publishes one fresh composition for the new member,
and starts a fresh conversation. A model, provider, MCP server, Process
Plugin, conversation, repository content, or Tool input cannot select or add
a member.

The exact HostExplicit set remains:

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

No option in this decision recommends expanding that set.

## 4. Current omissions and their product meaning

The current UX has two omissions:

1. membership disappears when Desktop restarts; and
2. there is no user-facing removal operation.

The first makes the current feature session-oriented rather than
workspace-oriented. A user who routinely alternates between the same A/B
repositories must repeat admission after every restart. That is meaningful
friction, especially for a Desktop product.

The second prevents curation within a running process. An accidental member
remains visible and can only be avoided by switching elsewhere or restarting.
This is a real usability defect, but it is not repository deletion: removal
would alter only RAH host membership state and would not touch Git or
filesystem contents.

Neither omission prevents the milestone core value. During one process, the
user can admit A and B, see both, explicitly switch between them, and work
with exactly one repository-bound authority composition at a time. Restart
loss is therefore a cost of convenience and continuity, not evidence that
the current active-only boundary is incomplete.

## 5. Option A — Stop feature expansion

### Exact experience

The user launches Desktop, explicitly adds repository A, explicitly adds
repository B, switches A-to-B and B-to-A as needed, works against exactly
one active repository, closes Desktop, and explicitly admits the repositories
again after restart.

This is acceptable for v0.26 because the release is introducing a safe
multi-repository session boundary, not promising durable workspace
management. The process-local behavior is visible and predictable. An empty
workspace after restart also makes the security rule obvious: no old member,
path, registry, ticket, generation, runtime, provider, Commit authorization,
or index state is executable merely because an earlier process saw it.

### Advantages

- smallest authority and lifecycle surface;
- no new persistent trusted input or at-rest repository-location exposure;
- no stale path, root, `.git`, Git executable, alias, or removable-media
  reconciliation on startup;
- no schema migration or crash-consistency contract for membership;
- no active-member withdrawal transaction or removal race policy;
- existing Task 308–321-I evidence remains the scope under audit;
- Windows live certification can focus on two disposable repositories and the
  actual active-only composition boundary; and
- the release can explain one strong new capability without management-scope
  qualification.

### Disadvantages and explicit weight

The restart cost is **medium product pain**. It is frequent for users who
close and reopen Desktop, and it makes repeated A/B work less convenient. It
is not dismissed; it is simply outweighed by the fact that the core value is
available within a process and that durable membership would require a new
privacy and reconciliation surface.

The lack of removal is **low-to-medium product pain**. It matters after an
accidental admission or when a repository becomes irrelevant, but it has a
simple operational workaround in v0.26: leave the member inactive or restart
the process. It is not a safety requirement and does not justify active
authority-withdrawal work before certification.

### Boundary answer

Option A introduces no new executable authority, no trusted persistent
source, no cross-restart authority, no change to explicit admission, no new
live repository-generation domain, and no new lifetime owner. It changes no
Trusted Profile semantics, ToolRegistry composition, HostExplicit eligibility,
provider behavior, Commit authorization, Stage/Unstage behavior, or
model-visible Tool schema. ADR 0027 remains sufficient.

## 6. Option B — Inert descriptive membership persistence only

### What safe persistence would mean

Safe persistence cannot serialize or restore an admitted executable member.
The persisted record could contain only bounded descriptive information, for
example:

- closed schema version;
- opaque descriptive workspace ID;
- safe display label and ordering;
- bounded descriptive member ID;
- optional user label;
- optional user-approved repository location reference; and
- optional previously selected descriptive member.

It must not persist or reconstruct:

- `DesktopRepository`;
- `ToolRegistry` or Effective Authority;
- repository mutation authority;
- Commit capability or authorization;
- Stage/Unstage authority or prepared action;
- HostExplicit tickets, preparations, or retained preparers;
- runtime or provider connection state;
- provider credentials or provider authority;
- active repository executable authority;
- repository, admission, composition, or connection generation as live
  authority;
- filesystem identity proof, `.git` identity, or Git executable identity; or
- uncertain-effect continuation, replay, rollback, or recovery authority.

The correct restart sequence is:

```text
persist descriptive candidates
  -> startup displays remembered/inactive candidates
  -> explicit human or host-controlled fresh admission
  -> fresh canonical-root, filesystem-identity, .git, Git, and policy checks
  -> new process-local RepositoryMemberId and admission generation
  -> ordinary inert membership
  -> explicit activation and fresh active composition
```

A remembered record must therefore not simply reappear as an admitted
member. It is a remembered candidate, not authority and not currentness.
Automatic startup re-admission would materially change the existing rule that
admission is an explicit host/human authority-composition action. It would
also make an old path or location hint an implicit authority trigger. That
behavior is not acceptable as an incidental persistence feature. A safe B
implementation would need an explicit human admission step or a separately
defined host-controlled admission transaction with equally clear consent and
currentness semantics.

### Stale and ambiguous cases

Every remembered candidate would require a bounded, privacy-safe outcome for:

- repository deletion;
- repository movement;
- root replacement;
- `.git` replacement or changed `.git` form;
- changed Git executable;
- symlink or Windows reparse-point path;
- changed nested relationship;
- alias or case-equivalent duplicate;
- a repository that has become nested in another member;
- inaccessible path or unavailable removable media; and
- malformed, truncated, duplicated, or schema-incompatible data.

The safe result is remembered-but-unavailable or fresh-admission-required.
There is no silent recovery based on path spelling, basename, hash, or
physical continuity. If fresh validation proves a candidate is a repository,
it still receives a new process-local handle, new admission generation, and
fresh authority construction.

### Current storage does not automatically solve this

Desktop currently has two separate persistence planes:

- `desktop-preferences.json` is a closed JSON format, currently version 3,
  bounded to 4096 bytes, containing model selection, optional commit identity,
  and an optional remembered Trusted Profile path. It rejects unknown fields,
  bounds and validates values, and uses a temporary file plus synchronized
  replacement for writes. It is configuration storage, not repository
  membership storage.
- `conversation-transcript.sqlite3` is an authoritative schema-version-1
  SQLite store for bounded transcript namespaces, epochs, and pairs. It uses
  strict schema validation, full synchronization settings, transactional
  mutation, staging/migration, quarantine on corruption, and no stale V3
  fallback after SQLite becomes authoritative. Its `repo-sha256:` namespace
  is a conversation namespace, not repository identity or authorization.

Neither format should be reused by assumption. Absolute repository paths are
private host information even when they are descriptive. The existing
normal-user-account at-rest boundary is not encryption, and logs, errors,
activity, model-visible state, provider metadata, and Effective Authority
must not echo the path or private identity. A membership store would need its
own closed schema, size limits, unknown-field behavior, corruption policy,
atomic save ordering, migration/version policy, and Windows path
serialization rules. It must also decide whether location persistence is
opt-in, how unavailable entries are displayed, and when a removal write is
durable.

### Boundary answer

With the safe semantics above, B adds no executable authority and no
authority across restart, but it does add a new persistent descriptive input
and a new storage lifetime. It preserves explicit admission, but requires
fresh process-local generations after restart and likely an additive
authority/restart-semantics ADR or amendment before implementation. It does
not require a new Tool schema, PermissionLevel, provider protocol, Trusted
Profile rule, HostExplicit name, Commit rule, or Stage/Unstage rule. Its
trust-boundary and certification burden is nevertheless substantial for a
feature whose primary benefit is convenience.

## 7. Option C — Explicit member removal only

### Inactive-member removal

A narrow inactive removal route can be defined as:

```text
explicit human selects existing opaque member selector
  -> host resolves the current process-local member
  -> host confirms membership identity/currentness under membership coordination
  -> membership record is removed
  -> membership generation increments
  -> removed member is no longer selectable or admissible as an existing member
```

This is descriptive catalog mutation only. It does not delete, alter, stage,
unstage, rename, or otherwise mutate repository contents. The route must
accept only the opaque selector, never a path or repository object, and must
not rely on frontend state as authority.

Removal would affect duplicate admission behavior: after removal, a later
explicit admission of the same physical repository is a fresh admission with
a new process-local member selector and generation. No old ticket,
conversation authority, Commit authorization, workflow action, provider
state, or runtime state returns. If persistence is absent, there is no saved
descriptive record to delete; the process-local record simply disappears.

### Active-member policy

The smallest safe product policy is to reject removal while the member is
active and require the user to switch to another admitted member first. The
active member can then be removed as an inactive member. If it is the sole
member, the user must first enter an explicit zero-active or empty-workspace
state through a separately designed operation.

That policy keeps removal out of the active authority-withdrawal transaction.
It is less convenient than a direct “remove active repository” button, but it
avoids silently withdrawing the repository that currently owns the runtime,
registry, conversation, workflow, and model context.

If direct active removal were allowed, it would be authority withdrawal in
addition to descriptive catalog mutation. The operation would have to
atomically invalidate or withdraw:

- `DesktopRepository` and active repository composition;
- repository and active-selection currentness;
- Commit capability and authorization;
- Stage/Unstage selectors, observation, and staged review;
- HostExplicit prepared state and pending tickets;
- conversation context and repository namespace selection;
- connection/runtime and provider composition;
- model-turn binding where applicable; and
- Effective Authority presentation.

It would also need a clear disconnect-first rule and race behavior for
`ModelTurn`, `HostRunning`, index-effect reservation, `Connecting`,
`Connected`, `Disconnecting`, and activation. Removal cannot roll back a
started effect, and it cannot replay an uncertain effect. A running effect
would have to block removal or finish under its existing ownership before
withdrawal. This is a significant lifecycle feature, not a catalog button.

### Boundary answer

Inactive-only C adds descriptive membership mutation, a membership-generation
transition, a new selector/action surface, and a new lifecycle test family,
but no repository filesystem/Git authority. It does not need persistence,
provider changes, Tool schema changes, HostExplicit expansion, Commit changes,
or Stage/Unstage changes if active removal is rejected. ADR 0027 already
defines the relevant inactive-removal boundary. A direct active-removal
variant would add authority withdrawal and require a focused lifecycle design
task before implementation.

## 8. Option D — Persistence plus removal

D is not merely B and C added together. It combines two state machines:

```text
durable descriptive candidate state
  <-> fresh process-local admitted membership
  <-> active-only executable composition
  <-> explicit removal and durable catalog update
```

The combined design must define, test, and certify:

- persistence write ordering when a member is removed;
- crash consistency between in-memory removal and durable deletion;
- startup reconciliation of missing, moved, replaced, nested, aliased, or
  unavailable repositories;
- active selection persistence without active authority restoration;
- duplicate handling when two old entries now identify one repository;
- a removed item becoming eligible for fresh re-admission;
- privacy of absolute paths and labels at rest and in diagnostics;
- schema migration and corruption behavior; and
- whether an interrupted save leaves a remembered candidate, a removed
  candidate, or a bounded warning without accidentally restoring authority.

The combined feature would make the product feel more workspace-like and
curated, but it would delay the core multi-repository certification for a
management feature bundle. It also multiplies the deterministic matrix and
the Windows restart/removal scenarios. There is no evidence at this
checkpoint that this larger surface is required for the v0.26 milestone.

### Boundary answer

D introduces a new descriptive persistent source, new membership mutation, and
potentially active authority withdrawal. It requires a dedicated design
sequence, likely additive ADR treatment for the concrete restart and storage
contract, and the greatest lifetime, privacy, currentness, and certification
burden. It does not require new model-visible repository Tool schemas or a
larger HostExplicit set, but it is the least deferrable option operationally
once shipped because persistence and removal semantics become coupled.

## 9. Authority-boundary comparison

| Boundary question | A — stop | B — persistence | C — removal | D — both |
| --- | --- | --- | --- | --- |
| New executable authority? | No. | No, if remembered data is inert and re-admission is fresh. | No for inactive-only removal; active removal would withdraw authority. | No new Tool authority, but active removal may withdraw authority. |
| New trusted persistent source? | No. | Yes: descriptive, private, fail-closed storage. | No. | Yes, plus removal durability. |
| Authority across restart? | No. | No; new handle, generation, identity proof, and composition are required. | No restart behavior changes. | No if designed safely, but more restart states must be proven. |
| Explicit admission semantics? | Unchanged. | Must remain explicit; automatic startup admission is rejected. | Unchanged. | Must remain explicit and interact with removal/re-admission. |
| New live generation/currentness fields? | No. | No new authority fields; every restart gets fresh process-local values. | Membership generation changes on remove; active removal would use existing active/connection domains. | Both membership persistence metadata and removal transitions. |
| New lifetime owner? | No. | Yes, a bounded storage/reconciliation owner. | No new external lifetime; a membership transaction is needed. | Yes, storage plus removal transaction ownership. |
| Trusted Profile semantics? | No change. | No change. | No change. | No change. |
| ToolRegistry composition? | Existing active-only registry. | Existing active-only registry after fresh admission. | Existing active-only registry; inactive removal removes reachability. | Same, with more reconciliation paths. |
| HostExplicit eligibility? | Exactly 11; no expansion. | Exactly 11; no expansion. | Exactly 11; no expansion. | Exactly 11; no expansion. |
| Providers/runtime? | Existing lifecycle only. | No restore or per-member provider state. | No inactive provider state; active removal would require disconnect policy. | No restore, but combined stale/lifecycle cases grow. |
| Commit authorization? | Existing repository-bound behavior. | Never persisted or restored. | Inactive removal does not affect active Commit; active removal must revoke it. | Same, plus restart/removal ordering. |
| Stage/Unstage? | Existing active-only behavior. | Never persisted or restored. | Inactive removal does not affect active workflow; active removal must invalidate it. | Same, plus durable reconciliation. |
| Model-visible Tool schemas? | No change. | No repository selector or persistence metadata. | No repository selector; removal is host IPC only. | No schema change, but more host presentation. |
| ADR 0027 impact | Sufficient as accepted. | Conceptually compatible; concrete persistence/restart design should get an additive ADR or amendment before implementation. | Sufficient for the narrow boundary; active-removal details need focused design. | Additive ADR/design treatment is prudent before implementation. |

Option A answers “no” to every new authority and lifetime question. B and D
keep executable authority inert, but they still create a new trusted input and
privacy surface. C can remain narrow only if active removal is rejected while
active. That distinction is why a convenience comparison must not collapse
all four options into feature count.

## 10. Persistence trust-boundary analysis

### Descriptive does not mean security-irrelevant

Repository locations identify private host projects. A persisted absolute path
can disclose project names, account layout, drive structure, removable-media
locations, or customer names even if it cannot authorize a Tool. It must be
treated as sensitive host configuration. The path must not enter generic
activity, model context, provider metadata, error strings, telemetry, or
Effective Authority snapshots merely because it was loaded from preferences.

### Safe restart contract

The only safe contract is remembered candidate state followed by fresh host
admission. Fresh admission must re-establish, as applicable:

- canonical existing root;
- root filesystem identity;
- supported `.git` form and `.git` identity;
- ordinary-worktree and repository-state constraints;
- Git executable identity;
- nested boundary and alias/case rules;
- capability-specific authority construction; and
- a fresh process-local member handle and admission generation.

The persisted descriptive member ID, path, basename, label, path hash, or
previous active marker is never sufficient proof. A changed root, `.git`, Git
executable, alias, mount, nested relationship, or Windows reparse path is a
new or unavailable candidate and must fail closed until fresh admission.

### Storage requirements before any future implementation

A future persistence research task would need to settle, before code:

- separate closed schema and versioning rather than implicit reuse of model or
  transcript storage;
- explicit opt-in or clearly documented path persistence posture;
- bounded UTF-8/control-free labels and serialized Windows paths;
- supported drive, UNC, verbatim, long-path, case, and alias rules;
- unknown-field, duplicate-field, truncation, and corrupt-file behavior;
- atomic save, temporary-file cleanup, synchronization, and crash ordering;
- authoritative-versus-stale-file selection and migration policy;
- warning/error redaction; and
- tests that search actual serialized values for sentinel paths, `.git`
  values, filesystem identities, tickets, and credentials.

These requirements are a meaningful new trust boundary even though the output
is “only descriptive.” They are not release-blocking defects in Option A.

## 11. Removal lifecycle analysis

Removal has two classifications:

| Target | Classification | Required effect |
| --- | --- | --- |
| Inactive member | Descriptive catalog mutation only | Remove the host membership record, increment membership generation, and revoke future selection. No repository I/O. |
| Active member | Descriptive mutation plus authority withdrawal | Withdraw active composition and all repository-bound state, or reject until the user explicitly switches first. |

The recommended narrow C policy is to reject active removal while active.
That means the first implementation, if later selected, can be an
inactive-member operation only. It should use the same opaque selector
discipline as activation, resolve under membership coordination, and reject
busy or stale state without exposing private identities.

If product evidence later requires direct active removal, it must be a separate
research/design task. It must specify disconnect ordering, zero-active
presentation, runtime/provider shutdown ownership, invalidation of
HostExplicit and Commit state, Stage/Unstage action consumption, conversation
namespace behavior, and all races with model turns, Connect, Disconnect,
activation, and index-effect reservations. It must not be implemented as a
shortcut that calls the existing repository replacement path with `None`.

## 12. User-value analysis

| Frequent action | Current slice | B | C | D |
| --- | --- | --- | --- | --- |
| Open Desktop for a one-off task | Works after explicit admission. | Slightly faster on a remembered candidate, but still needs fresh admission. | No change. | Faster plus curation. |
| Work repeatedly across A and B in one process | Strong: explicit A/B switching is the milestone value. | Same value. | Same value. | Same value. |
| Restart and resume the same A/B set | Re-admission required; medium friction. | Better convenience if safe re-admission is understandable and quick. | No change. | Better convenience. |
| Accidentally admit the wrong repository | Leave it inactive or restart; low-to-medium friction. | Remembered wrong entry could persist unless removal exists. | Strong improvement through inactive removal. | Strong improvement. |
| Clean stale/unavailable entries | Restart clears process-local entries. | Requires stale-entry presentation and reconciliation. | Removal is direct for inactive entries. | Requires both reconciliation and removal. |

The high-value action is working across A/B without model-selected authority,
and A already delivers it. Restart loss is the strongest argument for B; lack
of removal is the strongest argument for C. Neither argument demonstrates
that the core v0.26 capability is unusable. The evidence supports giving
restart convenience **medium** weight and curation convenience **low-to-medium**
weight, while giving authority-boundary clarity and certification confidence
**high** weight for this release decision.

## 13. Release-theme analysis

Option A has the clearest release theme: explicit multi-repository admission,
active-only composition, and safe switching. Its limitation is easy to state:
membership is process-local and restart requires fresh admission.

Option B adds a potentially attractive phrase—“RAH remembers repository
membership”—but that phrase needs a prominent qualification: remembered does
not mean admitted, current, or executable. The qualification is correct but
adds conceptual and certification weight without changing the core authority
story.

Option C adds a useful curation phrase—“users can remove inactive members”—but
it is peripheral to the milestone theme. Direct active removal would turn the
release story into lifecycle management rather than active-only switching.

Option D makes the release theme broad but less crisp: remembered candidates,
fresh admission, stale reconciliation, and member curation. It materially
strengthens convenience, not the security capability that v0.26 is intended
to prove.

The strongest and easiest-to-explain v0.26 release story is therefore Option A.
B and C are clean follow-up themes for v0.27, where their own research,
privacy, and lifecycle evidence can be evaluated without blocking the current
milestone.

## 14. Certification-cost comparison

| Option | Deterministic expansion | Windows live-certification expansion | Release risk |
| --- | --- | --- | --- |
| A | Existing Tasks 308–321-I matrix plus final milestone audit. | Two disposable repositories: admit A/B, activate A, switch B, switch A, verify active-only reads/mutations, stale ticket rejection, provider/model non-selection, cleanup. | Lowest; evidence stays aligned with the implemented milestone. |
| B | Add schema, malformed/corrupt/truncated data, atomic-save/crash cases, startup remembered/inactive presentation, fresh re-admission, moved/replaced root and `.git`, alias/nested changes, unavailable media, privacy sentinel scans, and no authority restoration. | Restart between admissions; prove no registry, runtime, provider, ticket, Commit, Stage/Unstage, or active authority restoration; then fresh admission and fresh A/B composition. | High; a convenience feature adds a second lifecycle boundary and privacy-sensitive paths. |
| C | Add selector removal, unknown/stale/duplicate removal, membership-generation effects, inactive active-presentation updates, re-admission after removal, stale ticket behavior, and busy rules. Direct active removal additionally requires full invalidation/race coverage. | Remove inactive member and verify no repository mutation; if active removal is supported, certify disconnect/zero-active/cleanup and all lifecycle races. | Medium; manageable inactive-only scope, high if active removal is included. |
| D | Union of B and C plus write-order/crash consistency, remove-versus-startup reconciliation, duplicate alias convergence, removed-item re-admission, and combined stale/currentness cases. | Restart, stale reconciliation, removal, re-admission, A/B switching, and provider/runtime cleanup in one matrix. | Highest; delays core certification and multiplies failure interpretation. |

Option A is the only option whose live certification can remain a focused
proof of the milestone itself. B adds restart certification; C adds
catalog/lifecycle certification; D adds their cross-product. None of those
additional live gates should be run under Task 322.

## 15. Milestone-risk analysis: Tasks 308–321-I

The recent history is evidence of architectural sensitivity, not evidence
that ADR 0027 is unsound.

- Task 319 found that activation invalidation could occur before a later
  publication check, which could destroy state belonging to the repository
  that remained active.
- Task 320 corrected the activation commit point and loser-after-winner
  preservation.
- Task 321-C and Task 321-D exposed the need to coordinate Commit
  authorization, model/identity writers, and activation under the same
  lifecycle boundary.
- Task 321-D also found Connect identity-generation and Stage/Unstage
  lifecycle gaps.
- Task 321-E through Task 321-H closed the Connect/index reservation ordering
  and evidence issues, but Task 321-H still found an evidence-fidelity gap
  caused by testing a shortcut rather than the real publication paths.
- Task 321-I closed that gap through the real
  `begin_repository_index_effect` and
  `publish_connected_provider_state` paths.

The pattern is consistent: the active-only architecture contains the right
boundaries, but each new lifecycle writer or restart state needs precise
linearization and evidence. Persistence would add startup and durable-state
writers. Active removal would add authority-withdrawal writers. Continuing to
expand before the milestone audit would increase the chance of another subtle
currentness or lifecycle finding and would make a later failure harder to
attribute.

The prudent inference is not “never add these features.” It is “certify the
bounded slice whose lifecycle evidence is now closed, then research the next
management boundary separately.”

## 16. Decision matrix

Ratings are qualitative; they are not a hidden score.

| Dimension | A — stop | B — persistence | C — removal | D — both |
| --- | --- | --- | --- | --- |
| User value | High for the core A/B session workflow; medium restart friction remains. | Medium-high convenience, but remembered candidates still need admission. | Medium convenience for curation; core workflow unchanged. | High convenience, but mostly additive management value. |
| Release-theme fit | High: exactly the multi-repository switching milestone. | Medium: adds a qualified restart story. | Medium-low: curation is adjacent. | Medium: broad but less focused. |
| New authority complexity | Low; no new authority. | Medium; no executable restore, but new persistent input/re-admission boundary. | Low for inactive-only; high for active removal. | High; combines both and may include authority withdrawal. |
| Persistence trust-boundary complexity | None. | High: private paths, schema, corruption, atomicity, reconciliation. | None. | High, plus removal write ordering. |
| Lifecycle/currentness complexity | Existing closed switching/connect/index lifecycle. | High startup and fresh re-admission matrix. | Medium inactive-only; high with active removal. | Highest. |
| Privacy impact | No new persisted repository locations. | Medium-high: absolute locations become durable host data. | Low: process-local selector/catalog only. | Medium-high. |
| Deterministic test burden | Existing milestone matrix and audit. | High expansion. | Medium expansion; high for active removal. | Highest and combinatorial. |
| Windows live-certification burden | Focused two-repository certification. | Adds restart and stale-path certification. | Adds removal and possible zero-active certification. | Adds both and their interactions. |
| Release delay/risk | Lowest. | High. | Medium; high if active removal is included. | Highest. |
| Deferrability | Can cleanly defer both omissions. | Clean v0.27 persistence theme. | Clean v0.27 curation theme. | Difficult to separate after coupled semantics ship. |
| Architecture fit | Directly fits accepted ADR 0027. | Fits only with strict inert semantics and additional design evidence. | Fits if inactive-only or explicit busy-first active policy. | Fits conceptually but broadens the accepted lifecycle. |
| Overall recommendation | **Select.** | Defer. | Defer. | Reject for v0.26. |

## 17. ADR impact

ADR 0027 remains sufficient for Recommendation A. No ADR change is required
or authorized by Task 322.

ADR 0027 already establishes that workspace membership is descriptive,
restart restores no executable authority, active composition is singular, and
member removal must preserve currentness and no-replay rules. If B or D is
selected in a later task, the concrete persistence schema, privacy consent,
startup semantics, corruption policy, and crash ordering should be accepted
through an additive ADR or explicit amendment before implementation. If a
later C task remains inactive-only and rejects active removal while active,
the existing ADR boundary is sufficient; direct active removal requires
focused lifecycle research before coding.

No ADR was modified for Task 322.

## 18. Explicit deferrals

The following remain deferred regardless of this decision:

- generic workspace filesystem authority;
- parallel active repositories;
- cross-repository operations or transactions;
- model-selected repository routing;
- provider-selected repository routing;
- workspace-wide `ToolRegistry`;
- repo-qualified model Tool names;
- network Git;
- HostExplicit `repo.commit`;
- network MCP;
- `PluginManager`;
- profile hot reload;
- generic shell/process authority; and
- rollback or replay guarantees.

Persistence and member removal are also explicitly deferred from v0.26 by
this recommendation. The exact HostExplicit set remains 11; this decision
does not recommend expanding it.

## 19. Exact next task sequence

Proceed in this order, with each gate required before the next task:

1. **Task 323 — v0.26 Multi-Repository Milestone Audit.** Audit the entire
   Tasks 308–321-I architecture and deterministic evidence, including
   admission, nested boundaries, active-only composition, A/B switching,
   currentness, Commit, Stage/Unstage, HostExplicit, Connect/index ordering,
   provider/model isolation, privacy, restart non-restoration, and the exact
   11-name set. Do not reopen settled implementation without new evidence.
2. **Task 324 — Windows Two-Repository Live Certification**, only if Task 323
   is READY. Use two fresh disposable repositories and Windows-native tooling
   to prove the actual Desktop/host behavior. Do not broaden the gate to
   persistence, removal, model-selected repository routing, or unsupported
   authority claims.
3. **Task 325 — v0.26 Release Preparation**, only if Task 324 passes. Release
   preparation is separate from publication and must preserve immutable
   release chronology.
4. Release publication remains a later, separately authorized phase after
   preparation and exact-source CI.

Task 322 does not implement, certify, prepare, publish, or bump v0.26.

## 20. Scope and public-contract closure

This is exactly one research/decision document. It contains no Rust
production-code change, frontend production change, persistence implementation,
removal implementation, permission change, Cargo change, dependency change,
lockfile change, ADR change, Tool schema change, PermissionLevel change,
provider protocol change, Trusted Profile schema change, or Tauri command.

The final decision is therefore:

**Recommendation A — STOP FEATURE EXPANSION FOR v0.26.**
