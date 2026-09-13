# Task 319 — RAH v0.26 Multi-Repository Switching Integration and Security Audit

Date: 2026-09-13
Audit mode: independent, deterministic, audit only
Audited checkout: `e71054b849f941cb71e585041e48b69c39139f87`
Direct parent: `4ea4d0462cafa327be5f530f0485fd6a8e396e7e`

## Authoritative checkpoint and boundaries

The audited master was exactly the supplied Task 318 final head. The worktree
was clean before validation and no production code, test, frontend, permission,
Cargo, lockfile, ADR, version, changelog, tag, release, or live-certification
state was changed by this audit. The only intended change is this document.

The immutable v0.25.0 source remains
`a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`; annotated tag object
`ea3c31aaf5190b632d7ef86387f7aff6004ae664`; GitHub Release `387406579`.
No v0.26 release or live two-repository certification is claimed.

## Architecture chain

Tasks 316–318 implement the accepted ADR 0027 chain:

1. Task 316 stores a process-local descriptive catalog of inert members. Each
   member retains only host-private admission data (`root`, admission identity,
   display data, and admission generation); executable repository state stays
   in the single active `DesktopAppState.repository` slot.
2. Task 317 supplies the activation/currentness transaction and shared
   `lifecycle_coordination` gate. It revalidates the selected identity, rejects
   active model/HostExplicit/connection transitions, publishes one active
   repository generation, and keeps connection publication generation-bound.
3. Task 318 adds the narrow selector IPC and frontend presentation. The switch
   route ends in `activate_admitted_member`; it does not add a second dispatch
   path, repository-qualified Tool input, persistence, or removal.

The central invariant is structurally present: membership is descriptive, and
the active repository is the only repository source for executable authority.
The activation invalidation ordering described under Findings prevents this
audit from accepting the complete integration as conformant.

## Selector security

`RepositoryMemberId::selector` emits `m<workspace_epoch>-<ordinal>` and
`parse_selector` enforces the `m` prefix, 42-byte bound, canonical nonzero
decimal components, overflow rejection, and no trailing data. The fields are
private. `activate_repository_member` accepts only a string selector, parses it,
looks up the current host-owned catalog, copies the member's private admission
identity and admission generation into `ActivationTransaction`, revalidates
the identity, and requires the final publication gate.

A guessed valid-looking selector therefore does not itself grant a root,
ToolRegistry, permission, mutation authority, Commit control, or provider
authority. Malformed, unknown, stale, removed, and replaced-`.git` cases are
bounded errors. Selecting the active member takes the explicit already-active
no-op path and does not use the selector as authority.

The process-local epoch is allocated by the process static
`NEXT_WORKSPACE_EPOCH`; separate `DesktopAppState` instances receive distinct
epochs under ordinary operation. It is not cryptographic secrecy and selectors
are not persisted. The atomic allocation wraps after `u64::MAX` instances;
zero is rejected by the parser, so this is a theoretical availability edge,
not an authority grant. There is no dedicated cross-instance epoch test.

## IPC, Tauri permissions, and frontend authority

The only new public Desktop IPC commands are:

- `repository_membership`
- `activate_repository_member`

`build.rs` registers each exactly once. The generated permissions allow exactly
their corresponding command and have no wildcard, filesystem scope, repository
path scope, or generic invoke scope. `capabilities/default.json` opts into only
those exact permission identifiers.

`choose_repository` remains distinct: it uses the host folder dialog, captures
and validates a new admission, then activates it; duplicate and nested/alias
admissions reject before changing active state. `activate_repository_member`
does not accept a path, repository object, member object, or model-supplied
authority field. No automatic discovery or path-based switch shortcut exists.

The UI renders only backend `memberId`, safe basename `displayName`, `active`,
and `availability`; it sends the selected opaque value. It disables switching
while running, Connecting, Connected, or Disconnecting, but backend checks are
independent. It does not optimistically publish authority. On success it
refreshes the backend-derived presentation and, only for `activated`, the
transcript, repository snapshot, Effective Authority, and status. On failure it
re-reads bounded membership/status and does not infer activation. DOM or manual
`invoke` forgery can therefore submit only a selector and remains subject to
the backend chain.

## Membership presentation and lifecycle

`repository_membership` only locks and serializes `WorkspaceMembershipState`.
It does not resolve Git, revalidate identities, construct `DesktopRepository`,
compose Tools, activate providers, change either generation, alter workflow,
conversation, or Commit state. The serialized schema is exactly:

```text
{ members, activeMemberId, membershipGeneration }
member: { memberId, displayName, active, availability }
```

Inactive members contain no `DesktopRepository`, ToolRegistry,
`DesktopToolComposition`, mutation authority, Commit control, Stage/Unstage
workflow, HostExplicit preparation, provider activation, or runtime. Their
retained `root` and `RepositoryAdmissionIdentity` are host-private descriptive
admission records, not executable state.

The active no-op path checks only that the requested member is the current
active catalog member and returns `already_active`. It increments no repository
generation, starts no conversation, clears no workflow or prepared ticket,
revokes no Commit authorization, rebuilds no composition, and activates no
provider. It does not fresh-revalidate the filesystem, but it grants no new
authority or currentness and leaves existing operation currentness checks in
control. This no-op behavior is conformant to the stated contract.

Successful activation publishes a fresh active repository and increments
`repository_generation` once. Admission changes only `membership_generation`.
Switching existing members does not change membership generation. Failed and
losing activation publication paths do not increment repository generation.

## Active-only authority, ToolRegistry, and Effective Authority

`DesktopAppState` retains one `repository`, one repository generation, one
workflow, one Commit capability slot, one HostInvocationCoordinator, one
provider activation, and one connected `DesktopToolComposition`. The inert
membership map contains no executable objects. There is no registry cache keyed
by member ID and no workspace-wide union registry.

`desktop_tool_registry(repository, commit_tool)` registers repository-bound
Tools only when passed the one active `DesktopRepository`. The repository Tool
names are ordinary names such as `repo.status`; no `repoA.*`, `repoB.*`, member
selector, or repository selector is in a Tool schema. A later Connect captures
the currently active repository and creates a fresh registry/composition. The
previous active repository cannot contribute to that composition.

Effective Authority is derived from the one selected repository and one
published connection composition. It exposes bounded classifications and
generations, not private admission identity. With no current connection it
does not advertise executable repository Tools. The source and type audit found
no A+B union and no inactive executable availability. Existing source coverage
proves the composition is active-only, but there is no dedicated real
A→B→A Effective Authority serialization stress test.

## Connection composition and Task 317 race invariants

Switching is rejected for Connecting, Connected, and Disconnecting, and the
same checks run in both activation capture and final publication. Connect
captures repository generation/root and the full repository/model/profile/
connection tuple. Publication rejects stale tuples and reaps rejected runtime
and provider state. After disconnect, the next Connect captures the new active
repository and rebuilds its composition. A connection prepared for A cannot
publish after B wins.

The deterministic Task 317 race cases are green for connection transition,
model turn, HostPrepared confirmation, HostRunning, concurrent B/C activation,
target removal, target `.git` replacement, previous-member change, and
repository-generation change. No runtime is migrated and no uncertain Host
effect is replayed.

### Finding F-319-1 — activation invalidation is not atomic with publication

In `activate_admitted_member`, after the second target identity revalidation,
the command calls `revoke_repository_commit_context(state).await` before the
final `publish_activation_if_current` gate. The final gate later rechecks
membership binding, active member, repository generation, lifecycle state, and
target identity. The invalidation is therefore not part of the same successful
linearization point.

A deterministic stale-target sequence is:

```text
A active with A workflow / reviewed Commit state
activate B
B passes the second revalidation
the target .git or another currentness input changes
revoke_repository_commit_context clears A workflow / Commit control
final revalidation rejects B; A remains active; repository generation is unchanged
```

The failed switch has nevertheless changed active A's executable review state.
This violates the required failed-switch transaction boundary and can revoke
the still-current active repository's reviewed Commit authorization without a
successful switch.

The same ordering permits a concurrent losing B/C activation to call the
pre-publication invalidation after the other activation has published. A
losing call then fails `RepositoryBusy` at publication but can clear workflow
state installed for the winning active repository in the intervening window.
The existing concurrent test synchronizes both calls at the barrier after this
invalidation and therefore does not cover loser-after-winner invalidation.

Smallest correction scope: make Commit/workflow invalidation conditional on
the final current activation transaction and serialize it with the same
lifecycle/membership publication protocol, while retaining the existing
nonblocking async/lifecycle ownership rules. Add a deterministic stale-after-
second-revalidation and loser-after-winner test. Do not solve this by broadening
authority, retaining per-member executable state, or replaying effects.

Because this is a lifecycle/stale-state integrity defect in the central switch
transaction, it is material even though the observed consequence is state
revocation/denial rather than cross-repository execution.

## HostExplicit audit

The production HostExplicit set remains exactly 11:

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

`repo.create-directory`, `repo.commit`, MCP, Process Plugin, fixture/diagnostic,
unknown/provider-defined Tools, and repository switching itself remain outside
the set. No Tool schema or PermissionLevel changed.

Successful publication clears `HostPrepared` under the final lifecycle gate;
the old ticket is then unavailable. HostRunning and ModelTurn block switching,
and the existing no-replay/uncertain-effect semantics remain unchanged. The
focused tests prove A ticket invalidation on successful switch and blocking at
prepared/running races. A complete production-style reverse-direction ticket
test and a loser-after-winner ticket/workflow race test are missing.

## Commit audit

There is one `DesktopCommitCapability` slot, bound to repository/model/identity
generations, and `revoke_repository_commit_context` takes and clears its paired
control. A successful switch therefore withdraws the old authorization; a
fresh connection/review is required on the new active member; switching back
does not restore the old capability. Disconnect also revokes it. Existing
Commit tests prove one-shot control, disconnect invalidation, and generation
binding.

Finding F-319-1 means this otherwise-correct invalidation can happen on a
failed switch and can race a winning activation's workflow. The actual
A→B→A command sequence with a real reviewed authorization is not independently
covered, so the correction remains required on source proof alone.

## Stage / Unstage audit

`RepositoryWorkflowState` is a single active slot. Each action binds action
kind, repository generation, observation generation, canonical target, and
target observation. Publication resets actions, observation review, staged
review, review selector, and Commit review. Execution consumes the action before
effect and rechecks generation/observation/target currentness; an A action
cannot resolve to B. Existing tests prove old action IDs do not target a new
repository and that stage/unstage effects remain repository-local.

The Task 318 invalidation test proves the stored A fields are cleared on a
successful switch. There is no complete real A workflow → B switch → old
action attempt → B workflow → A switch stress test. F-319-1 can also clear a
winning repository's newly observed workflow after publication.

## Conversation and provider/model isolation

Successful publication selects the repository-namespaced persistence key and
calls `conversation.start_new()`. The frontend clears its transcript only after
an `activated` result; the backend reset is authoritative. No automatic resume
occurs. Resume is a separate explicit command and requires current connected
generations. Switching back to A starts a fresh in-memory conversation and does
not silently load A history. Conversation text is never used as authority.

Trusted Profile selection is global configuration only. MCP and Process Plugin
Tools are external provider Tools, are not HostExplicit eligible, and are
composed once per explicit connection. Inactive membership does not activate,
clone, or retarget providers. Provider metadata cannot choose a member.

Model-visible Tool requests and schemas contain no member ID, workspace member,
repository selector, or switching command. Only host Desktop IPC can activate a
member. Existing bridge and runtime tests preserve the model/provider boundary.

## Privacy and error audit

Membership serialization exposes only the four per-member presentation values
and the three top-level conceptual fields. It does not serialize the canonical
root, absolute path, filesystem identity, `.git` identity, Git executable,
admission generation, `RepositoryAdmissionIdentity`, ToolRegistry, permission
state, provider credentials, or HostExplicit ticket IDs. The basename is safe
descriptive presentation. Effective Authority and HostActivityEvent are
classified/sanitized and do not expose the private admission fields. Internal
currentness uses an opaque repository fingerprint for diagnostic correlation;
it is not a selector, authority token, or public membership field.

Malformed selector, unknown member, stale member, and busy lifecycle errors are
closed enum classifications. They do not include raw path, Git stderr, OS
error, filesystem ID, or identity evidence. Frontend output uses text nodes and
does not turn backend data into HTML.

The existing membership test checks serialized type/name/path absence, and the
frontend/Effective Authority tests pass. A dedicated sentinel-value scan of
every serialized membership, generic activity, model-visible, persisted, and
Effective Authority field is not present; this is a test gap, not the source
of F-319-1.

## Nested boundary, duplicate, alias, and unsupported forms

Task 315's shared `RepositoryNestedBoundaryPolicy` remains in the actual
repository-bound Tools. The focused five-test nested-boundary run passed,
including real repository A/nested repository B rejection and pre-effect
boundary checks. Membership admission independently rejects parent/child
co-membership, duplicate roots, case/drive aliases, and unsupported `.git`
file/linked-worktree forms. Admission rejection is supplementary; it does not
replace per-operation nested-boundary enforcement.

Bare repository support and linked-worktree/gitfile admission remain
unsupported. No cross-repository rename, move, copy, patch, edit, Stage,
Unstage, Commit, or transaction API exists.

## Repeated switching and generation matrix

Source behavior and the existing A/B tests establish the following successful
single-switch contract:

| Transition | Active repository | Repository generation | Membership generation | Workflow/ticket/conversation |
|---|---|---:|---:|---|
| startup | none | 0 | 0 | empty; no authority |
| admit A | none | 0 | 1 | unchanged; A inert |
| activate A | A only | 1 | 1 | fresh conversation and active composition boundary |
| admit B | A only | 1 | 2 | A remains active; B inert |
| activate B | B only | 2 | 2 | A state invalidated; B fresh |
| activate A | A only | 3 | 2 | B state invalidated; A fresh |

The same publication logic gives one increment per successful switch,
zero for already-active no-op, failed switch, losing activation, and unrelated
admission. No switch operation changes membership generation. A repeated
`A→B→A→B→A` deterministic stress test with every listed field is absent;
the current source is reviewed but the requested evidence is incomplete.

## Persistence and removal absence

`WorkspaceMembershipState` is initialized empty in every new
`DesktopAppState`. No persistence schema or field contains members, selectors,
active member, membership generation, or private admission identity. Existing
conversation/preferences persistence is separate and does not restore
repository authority, ToolRegistry, tickets, Commit authorization, providers,
runtime connection, or uncertain effects. Restart therefore yields zero
members and zero active repository.

No production removal IPC or removal UI exists. The internal/test `remove`
helper is non-user-facing and refuses removal of the active member. No removal
authority was added.

## Public contract comparison

Comparison of Task 317 final `b8065fbaee3ad5f6b210940b85391b1372f07472`
with Task 318 final `e71054b849f941cb71e585041e48b69c39139f87` found only the
allowed new Desktop IPC/presentation surface: `repository_membership` and
`activate_repository_member`, with their generated permissions and frontend
rendering. There is no Tool schema change, PermissionLevel change, provider
protocol change, Trusted Profile schema change, repository mutation-status
change, generic filesystem/Git authority, or dependency edge.

## Deterministic validation

Executed on the exact audited head:

- `cargo fmt --check` — PASS.
- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS: Desktop 229 passed, 10 ignored; all
  workspace suites passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — PASS.
- `git diff --check` and `git diff --cached --check` — PASS at closure after
  final document staging.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages,
  version `0.25.0`, edition `2024`.
- `cargo build -p rah-desktop --release` — PASS.
- `node --check crates/rah-desktop/frontend/status.js` — PASS.
- `node crates/rah-desktop/frontend/repository_membership_test.js` — PASS.
- `node crates/rah-desktop/frontend/status_authority_test.js` — PASS.
- `node crates/rah-desktop/tauri_permission_test.js` — PASS.
- `cargo test -p rah-desktop activation -- --nocapture` — PASS: 11 passed,
  228 filtered.
- `cargo test -p rah-desktop task_318 -- --nocapture` — PASS: 1 passed,
  238 filtered.
- `cargo test -p rah-tools nested_repository_boundary -- --nocapture` — PASS:
  5 nested-boundary unit tests passed; other targets had zero matching tests.

No tests were changed. Warnings observed during repository fixtures were only
Git LF/CRLF working-copy warnings; no validation failure resulted.

## Closure state and next task

Exact-head CI run `34731507831` is PASS for branch `master` and head
`e71054b849f941cb71e585041e48b69c39139f87`. Before this audit document was
created, `HEAD == origin/master`, the worktree was clean, and `Cargo.lock` had
no diff. The audit document itself is the sole pending file. No dependency or
lockfile drift exists.

Selector authority verdict: selector is non-authoritative and backend-bound.
Presentation privacy verdict: bounded for the implemented membership schema;
sentinel-value coverage gap recorded.
Already-active verdict: safe no-op; zero generation/authority/lifecycle churn.
A↔B repeated-switch verdict: source shape is active-only, but requested full
stress evidence is missing.
ToolRegistry/Effective Authority verdict: active-only; no union/cache/member
selector route found.
Connection composition verdict: lifecycle-blocked and tuple-current; no
migration path found.
Task 317 race verdict: existing deterministic cases pass, but F-319-1 requires
activation invalidation correction.
HostExplicit prepared/running verdict: successful switch invalidates prepared
state; running state blocks; no replay.
Commit verdict: successful switch/disconnect withdraws the capability, but
failed-switch invalidation ordering is unsafe.
Stage/Unstage verdict: active-generation binding prevents A→B execution;
workflow can be cleared by F-319-1.
Conversation verdict: fresh backend context and explicit-only resume.
Provider/model verdict: no repository selection or inactive provider route.
Task 315 nested-boundary verdict: PASS and independently enforced.
Persistence/removal: absent as required.
HostExplicit count: exactly 11.
Recommended next task: `Task 319-C — Atomic activation invalidation correction`,
including the two missing activation race tests. Keep persistence, removal,
milestone live certification, and release audit blocked until that correction is
audited. Task 320 must wait for this correction and a fresh Task 319 audit.

Verdict B — CORRECTION REQUIRED
