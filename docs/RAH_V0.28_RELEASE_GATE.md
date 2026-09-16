# RAH v0.28.0 Release Gate

Status: **RELEASED — HISTORICAL RECORD**

This gate records the accepted v0.28.0 release and its immutable publication
facts. Task 346 is a later documentation-only cleanup and is not the immutable
v0.28.0 release source.

## Release identity

- Theme: **Explicit Inactive Repository Member Removal**.
- Authority delta: **NONE**; HostExplicit remains exactly 11.
- Immutable v0.28.0 release source: `616af196b03c6473b93984d429e7dd0825edc0b7`.
- Annotated tag: `v0.28.0`.
- Tag annotation: `RAH v0.28.0`.
- Tag object: `2927c297e47cb04ea629c761b74afd43d2f685f1`.
- Peeled target: `616af196b03c6473b93984d429e7dd0825edc0b7`.
- Release-preparation CI: `35095044576` - PASS.
- Tag CI: `35095965370` - PASS.
- GitHub Release ID: `389903124`.
- Release name: `RAH v0.28.0`.
- Published: `2026-09-16T12:30:07Z` (`draft=false`, `prerelease=false`).
- Assets: 0.

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

The source boundaries remain distinct: Task 343 certified behavior/live source
is `a55347fc1df413fea6ab1647a7d80d88c2c99fe4`, Task 343 documentation head is
`5c43faa7ea11125d0f25dbc160b540ab62e442d6`, and the immutable v0.28.0 release
source is `616af196b03c6473b93984d429e7dd0825edc0b7`. Task 346 is only a later
documentation descendant and does not replace the release source.

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

## Validation and historical publication record

Task 344 passed the full deterministic Rust, metadata, Desktop release-build,
frontend static, formatting, and diff checks listed in the Task 344 plan. The
release build identified as `rah-desktop v0.28.0`; its preparation commit was
the immutable v0.28.0 release source above. Task 345 subsequently created the
annotated tag, passed tag CI, and published the GitHub Release recorded above.

Task 344 itself stopped before publication exactly as required: it did not
create the tag or GitHub Release. Task 346 does not mutate the tag or GitHub
Release and is not the v0.28.0 release source.

## Authority and package baseline

- ADR 0027 and ADR 0028 remain unchanged.
- Stage/Unstage remain separate host index authority; Commit remains separate
  reviewed repository authority.
- The certified Codex baseline is `codex-cli 0.149.0`.
- The workspace contains 13 packages, all at version `0.28.0`, on Rust edition
  `2024`.
