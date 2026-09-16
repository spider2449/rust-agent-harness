# RAH v0.28.0 Release Gate

Status: **READY FOR PUBLICATION - NOT YET RELEASED**

This gate records the accepted release-preparation candidate. Publication is a
separate immutable step and is not performed by Task 344.

## Release identity

- Theme: **Explicit Inactive Repository Member Removal**.
- Authority delta: **NONE**; HostExplicit remains exactly 11.
- Current immutable published release: RAH v0.27.0.
- Release-preparation source: the Task 344 commit created after this document
  is prepared; its SHA is reported in Task 344 closure and is not invented
  here.

## Certified evidence and source boundaries

- Certified behavior/live source:
  `a55347fc1df413fea6ab1647a7d80d88c2c99fe4`.
- Certified source exact-head CI: `35074858762` - PASS.
- Certification documentation head:
  `5c43faa7ea11125d0f25dbc160b540ab62e442d6`.
- Certification documentation exact-head CI: `35075439416` - PASS.
- Marker: `RAH_V028_INACTIVE_MEMBER_REMOVAL_LIVE_OK`.
- Classification: **PASS WITH EXPLICIT NONCLAIMS**.

The certified behavior source is a test-only, opt-in certification-harness
descendant of the Task 342 production/frontend behavior. Task 344 changes only
version metadata, the internal Cargo.lock package records, and documentation;
the Task 343 behavioral certification is therefore carried forward without a
new live effect.

## Audited contract

Tasks 339-343 close the v0.28 milestone in this progression:

```text
339 scope: explicit inactive process-local member removal
340 lifecycle contract and acceptance matrix
341 backend lifecycle foundation and generation correction
342 inactive-only Desktop UX and exact permission
343 independent audit and Windows host-driven certification
```

The release claim is that Desktop can explicitly remove an inactive admitted
repository member from the current process-local workspace. It is not
filesystem deletion, remembered-candidate deletion, active-repository removal,
or a new authority category.

The certified lifecycle claims are:

- For `members = [A, B]`, `active = A`, removing inactive B yields
  `members = [A]`, `active = A`, preserving active A state.
- Active, unknown, stale, malformed, and double-removal selectors fail with
  zero mutation; active removal is rejected.
- Membership generation advances only after successful membership mutation.
- Target-bound started/uncertain effects fail closed; unrelated active-A state
  does not globally block removal of B.
- Re-admission of the same repository creates a fresh process-local member ID
  and remains inert until separate explicit activation.
- Removal does not write the remembered catalog, delete files, or mutate
  worktree, `.git`, index, HEAD, refs, history, or network Git.
- The exact command-specific frontend permission and explicit confirmation
  distinguish process-local removal from files and remembered entries.

## Preserved isolation and accounting

Task 343 certified active-A preservation across inactive-B removal for the
active member, `DesktopRepository`, repository generation, active
composition/registry definitions, Commit, Stage/Unstage, HostExplicit prepared
state, conversation, and provider/connection state. This is limited to the
recorded host-driven evidence and does not generalize beyond it.

Recorded counts:

```text
model requests             = 0
model Tool requests        = 0
MCP activations            = 0
Process Plugin activations = 0
network Git operations     = 0
automatic commits           = 0
automatic activations      = 0
```

Explicit host activation and switching in the certification are not automatic
activation.

## Nonclaims

The release does not claim GUI automation or GUI certification (not executed),
model-selected dynamic Tool dispatch, OS sandboxing, network isolation,
rollback/replay/compensation/cancellation of uncertain effects, race-free
TOCTOU protection, cross-platform live parity, cross-process persistent
membership, or linked-worktree removal semantics. Process supervision is not
OS sandboxing.

## Validation and publication hard stop

Task 344 must pass the full deterministic Rust, metadata, Desktop release-build,
frontend static, formatting, and diff checks listed in the Task 344 plan. The
release build must identify as `rah-desktop v0.28.0`. After the preparation
commit, `HEAD == origin/master`, the worktree is clean, and exact-head CI must
pass.

No `v0.28.0` tag, tag push, GitHub Release, publication date, or published
status may be created by Task 344. Task 345 is the next task and may tag only
the accepted exact Task 344 SHA, then require tag CI and publish the release.
