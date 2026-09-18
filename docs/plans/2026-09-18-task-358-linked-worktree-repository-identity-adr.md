# Task 358 — Linked Worktree Repository Identity ADR

Date: 2026-09-18

Status: **ACCEPTED ARCHITECTURE DECISION / CONFORMANCE FREEZE**

## Checkpoint and inputs

- Required parent and starting `HEAD`: `00af79d821342a430ae8c37d351c45481fff25da`.
- Required starting state: `HEAD == origin/master`; clean worktree.
- Task 357 exact-head CI: run `35301502551` — PASS.
- Authoritative research: `docs/plans/2026-09-18-task-357-linked-worktree-authority-contract-research.md`.
- Released baseline: RAH v0.29.0.
- Workspace baseline: 13 packages, all `0.29.0`, edition 2024.
- HostExplicit production set: exactly 11.
- Certified Codex lifecycle baseline: `0.149.0`.

The starting SHA, origin branch, and clean worktree were verified before
editing. Task 357's real-Git research established the distinction between a
selected worktree's private identity and the common directory's shared-state
role. Its copied/fabricated gitfile, submodule, and separate-git-dir evidence
supports a closed linked-worktree form rather than admitting any Git-recognized
gitfile.

## Decision and change from Task 356

Task 357 Decision B is accepted: **NARROW IDENTITY EXTENSION**. Task 356
provisionally classified linked worktree membership as a **NEW AUTHORITY
CATEGORY** while its identity/currentness questions were unresolved. Task 357
showed that the target remains one explicitly selected root plus its validated
private Git identity, under ADR 0027's existing member and one-active
composition. The common Git directory is relationship/shared-state evidence,
not an executable target. Existing capability policies can bind the exact
per-worktree and shared Git facts they need without granting a new authority.

Task 356 remains historical roadmap evidence and is not rewritten. The Task
356 provisional classification is superseded by the accepted Task 357
research conclusion and this ADR. No new PermissionLevel, mutation
capability, Tool authority, HostExplicit category, routing authority, or
generic Git/filesystem authority is created.

## Accepted ADR and scope

Task 358 creates `docs/adr/0029-linked-worktree-repository-identity-extension.md`,
**ADR 0029 — Linked Worktree Repository Identity Extension**, with status
Accepted and date 2026-09-18.

ADR 0029 is additive. ADR 0027 continues to define repository identity,
membership, and one-active authority composition. ADR 0029 extends only the
closed supported repository identity forms, superseding the prior linked
worktree support limitation for future conformance. It leaves ADR 0027's
authority model intact and does not make the current production implementation
support linked worktrees. ADR 0027's v0.26 foundation nonclaim remains
historically accurate. ADR 0028 and all persistence rules remain unchanged.

This is an architecture decision and conformance freeze only. It authorizes
no Rust, frontend, test, permission, persistence schema, Cargo, dependency,
README, release-history, or ADR 0027/0028 change.

## Identity contract

The supported main form remains the existing canonical non-reparse root with
a real `.git` directory, root filesystem identity, fixed Git proof of a
non-bare ordinary worktree and private gitdir equal to common gitdir equal to
root `.git`, current host-owned Git executable identity, and existing nested,
alias, and path checks.

The sole new form is a registered standard linked worktree. Admission requires
a canonical non-reparse root; one regular non-reparse `.git` file with
bounded exact single-record `gitdir:` content; agreement with Git's reported
private gitdir; a private non-reparse directory immediately under
`<common>/worktrees/`; standard `../..` commondir resolution; an exact private
`gitdir` backlink to selected root `.git`; fixed Git semantic probes; exactly
one matching `git worktree list --porcelain -z` registration; non-bare,
non-prunable, non-stale status; no submodule or unsupported nested relation;
and current filesystem identity/reparse/alias checks on every relevant path
and object. Admission revalidates before publishing an inert member. Any
disagreement fails closed.

Git recognition alone never admits a gitfile. Copied and fabricated gitfiles,
submodules (including old `.git` directory forms), `--separate-git-dir`, bare
repositories, arbitrary gitfiles, malformed or oversized metadata, nested
repositories, and unsupported reparse/path forms remain rejected. A coherent
locked worktree may be admitted; RAH never unlocks or prunes it.

## Stable identity and mutable currentness

Stable evidence retains the canonical root and root filesystem identity,
selected Git executable canonical path and filesystem identity, and worktree
classification. Main worktrees additionally retain `.git` directory identity.
Linked worktrees additionally retain `.git` file identity and bounded exact
content/digest evidence, private and common canonical paths and filesystem
identities, `worktrees` parent identity, registration directory identity,
backlink and commondir identity/content relations, and the exact validated
registration for the selected root. All raw identity evidence is
host-private.

HEAD OIDs, branch ref OIDs, index contents or index object identity, ordinary
shared ref values, and object-store contents are mutable capability-specific
currentness, not immutable admission identity. Normal Git activity does not
force needless fresh admission while the selected root/private/common and
registration identity remains valid. External move, removal, prune, repair,
gitfile/private/common replacement, backlink change, commondir change, or
registration change makes retained identity stale as appropriate; RAH does
not repair or silently migrate it.

## Membership and shared Git state

Membership keeps ADR 0027's exact zero-or-one active rule. Main, linked A, and
linked B may be admitted as distinct inert members. Same root/root object or
same validated private worktree target is a duplicate. Canonical parent/child
roots are nested and cannot co-exist. Different roots and private Git targets
are distinct even when the common Git directory is identical. Git's sibling
registry is validation evidence only and is not consent or automatic
membership.

Common Git state may include objects, ordinary shared refs, and shared config
as Git defines them. HEAD, index, worktree-specific pseudorefs, and namespaces
such as `refs/bisect`, `refs/worktree`, and `refs/rewritten` retain
per-worktree semantics; not all refs are shared. Future resolution uses Git
semantic paths where practical.

There is no global common-dir generation or process/machine-wide lock. Each
operation captures exact facts it depends on. Stage/Unstage bind the selected
worktree and its current index/required HEAD/target. Reviewed Commit binds the
selected identity, exact index, HEAD, attached branch, expected selected
branch OID, composition, and existing policy. Unrelated B branch Commit does
not automatically stale A; changing A's expected branch OID does. Observation
captures only its required facts. External Git may race; exact stale checks
and existing uncertain-effect semantics apply.

## Existing authority, lifecycle, persistence, and privacy

ADR 0012 and subsequent bounded mutation decisions still constrain file
operations to the selected active root. ADR 0016 remains the sole reviewed
Commit authority. Stage/Unstage remain separate authorities targeting only
the selected active worktree's Git-resolved current index. Branch creation
remains separate and must use selected identity and exact current/ref
preconditions. No operation gains cross-worktree scope, and no effect class
is broadened.

Switch withdraws A's executable composition, validates B, builds a fresh B
composition, then publishes B. No sibling registry is retained. Close
withdraws executable authority without changing Git registration or
filesystem/index/ref state. Inactive removal changes only process-local
membership and never invokes Git worktree remove or prune.

ADR 0028 remains authoritative: restart restores zero admitted members and
zero active authority. Only existing descriptive remembered-candidate data
may persist. Linked identity, registration, filesystem evidence, member IDs,
active state, registries, preparations, Commit authorization, Stage/Unstage
selectors, and tickets are not persisted. Identity details remain private to
the host and are not automatically exposed through model, Tool, provider,
Activity, Effective Authority, persistence, or unbounded errors.

HostExplicit stays exactly 11; no permission, Tool field, worktree selector,
sibling dispatch, or provider dispatch is added. Fixed read-only Git probes
use host-owned executable/argv, bounded output and time, explicit cwd and
environment, and owned lifecycle. No shell or mutating worktree command is
used for admission. Windows alias, reparse, device/verbatim, and ADS
ambiguity checks remain fail closed. There is no race-free TOCTOU or
cross-process exclusion claim. Timeout, cancellation, disconnect, failure,
or lost response does not imply rollback or authorize replay/compensation.

## Future conformance and release gates

ADR 0029 requires future deterministic tests for main and multiple linked
identities, aliases and duplicates, common/private relation, rejected
gitfiles and repository forms, identity replacement/move/prune staleness,
independent HEAD/index state, selected-only Stage/Unstage/file mutation/Commit,
unrelated sibling ref isolation, selected branch OID staleness, switch, Close,
inactive removal, restart zero authority, and fresh remembered-candidate
admission. Concurrency/currentness tests must not use sleeps.

Production support remains gated on host-driven Windows evidence with a
`main`/`linked-a`/`linked-b` fixture and exact marker
`RAH_V030_LINKED_WORKTREE_LIVE_OK`. The gate must prove complete admission,
distinct member IDs sharing common state, one active repository, switching,
Close/removal/restart, selected-root and selected-index effects, selected
reviewed Commit and stale branch OID rejection, all specified unsupported
form/reparse rejections, zero worktree lifecycle mutations and network Git,
and no provider/model repository selection. Host-driven evidence is
acceptable; it does not imply GUI automation, model dispatch, or
cross-platform live certification.

## Explicit exclusions and next task

This decision does not authorize arbitrary gitfiles, submodules,
separate-git-dir, bare repositories, automatic sibling discovery, worktree
add/remove/prune/repair/move/lock/unlock, checkout/switch, multiple active
repositories, a union registry, cross-worktree Tools/Stage/Commit, network
Git, persisted executable membership, generic shell/process/filesystem
authority, HostExplicit `repo.create-directory`, network MCP, PluginManager,
or profile hot reload.

Next: **Task 359 — Linked Worktree Identity and Repository Policy
Foundation**. Sequence the work as reusable identity resolver; ordinary-main
compatibility; linked capture/revalidation; membership duplicate/relation;
repository construction/path resolution; observation/index/Commit
capability-currentness conformance; deterministic fixtures. Existing picker
and admission frontend should need minimal or no structural change unless
backend acceptance demonstrates otherwise; any frontend work stays explicit
and narrow. Do not fold Desktop-wide flows, independent audit, live
certification, or release tasks into one uncontrolled patch.

## Task 358 scope and validation

Expected changes are exactly:

- `docs/adr/0029-linked-worktree-repository-identity-extension.md`
- `docs/plans/2026-09-18-task-358-linked-worktree-repository-identity-adr.md`

Required checks:

- `git diff --check`
- `cargo metadata --no-deps --format-version 1` — 13 packages, all
  `0.29.0`, edition `2024`.
- Confirm `Cargo.lock`, dependencies, Rust, frontend, tests, permissions,
  README, ADR 0027/0028, and release history are unchanged.

No implementation or test is included in Task 358. Completion requires the
accepted ADR and plan, one commit, push to `origin master`, `HEAD ==
origin/master`, a clean worktree, and PASS exact-head CI for the pushed SHA.

## Closure record

| Required fact | Task 358 result |
|---|---|
| Parent checkpoint | `00af79d821342a430ae8c37d351c45481fff25da` |
| Final SHA | Set after commit |
| Changed files | The ADR 0029 and Task 358 plan listed above |
| ADR | 0029 — Linked Worktree Repository Identity Extension |
| Authority classification | NARROW EXTENSION |
| ADR 0027 | Remains authoritative; no edit; only linked support limit is superseded prospectively |
| Supported identity forms | Existing ordinary main and one closed standard linked-worktree form |
| HostExplicit | 11, unchanged |
| Codex baseline | `0.149.0` |
| Workspace | 13 packages; all `0.29.0`; edition 2024 |
| Exact-head CI | Set after push; must PASS |
| `HEAD == origin/master` | Must be true at closure |
| Worktree clean | Must be true at closure |
