# RAH v0.27.0 Release Gate

## Status

**READY FOR PUBLICATION — NOT YET RELEASED**

This gate is the Task 336 release-preparation record. It does not create or
authorize a tag or GitHub Release. The current published release remains the
immutable RAH v0.26.0 release.

## Release theme

**Durable Remembered Workspaces with Explicit Fresh Re-Admission**

RAH Desktop may durably remember descriptive repository candidates across
restart while restoring zero executable repository authority. A remembered
candidate becomes executable only through:

```text
remembered candidate
→ explicit human fresh admission
→ current repository validation
→ fresh process-local RepositoryMemberId
→ separate explicit activation
→ fresh active-only authority composition
```

## Final milestone audit

Tasks 328–335 form a complete v0.27 milestone with no unresolved mandatory
blocker:

| Task | Delivered boundary |
| --- | --- |
| 328 | Selected durable remembered workspace scope and fixed the authority boundary. |
| 329 | Researched persistence, privacy, storage, restart, and fresh-admission constraints. |
| 330 | Accepted ADR 0028 and the closed implementation contract. |
| 331 / 331-A | Added the bounded persistence foundation, staged reread validation, and Windows reparse-ancestor hardening. |
| 332 | Integrated passive, inert startup catalog loading. |
| 333 | Added catalog actions and explicit fresh re-admission without activation. |
| 334 | Added Desktop UX and explicit privacy-controlled location presentation. |
| 335 | Completed the security audit and Windows host-driven live certification. |

No design choice was reopened. ADR 0027 remains authoritative for
process-local membership, fresh repository identity/currentness, one active
repository, and active-only composition. ADR 0028 is additive and owns only
descriptive remembered persistence, privacy, restart inertness, fresh
re-admission, and catalog mutation.

## Authority invariants

- Automatic re-admission and automatic activation are forbidden.
- Persisted paths are location hints, not repository identity.
- Restart restores zero repository executable authority.
- Catalog deletion is not process-local member removal.
- Fresh re-admission uses current repository facts and a fresh member identity.
- At most one repository is active.
- There is no union ToolRegistry and no parallel active-repository authority.
- Stage/Unstage remain separate host index authorities.
- Commit remains a separate reviewed repository authority.

The exact production HostExplicit set remains 11:

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
fixtures/diagnostics, and unknown/provider-defined Tools remain ineligible.

## Persistence and privacy claims

The release records the dedicated `RememberedWorkspaceStore` and
`remembered-workspace.json` closed JSON schema v1. The catalog is bounded to
64 candidates, 256 UTF-8 bytes per label, 64 ASCII bytes per opaque ID, 4096
UTF-8 bytes per location hint, and 256 KiB for the final serialized catalog.
The store uses same-directory atomic replacement, staged-file reread and exact
validation, Windows ancestor/final-file reparse hardening, persist-before-
publish ordering, and bounded coordination.

Startup performs passive catalog load only. It does not probe candidate
filesystems or Git. Generic catalog presentation is path-free; full location
reveal is explicit and human-controlled. Fresh re-admission reuses the ADR 0027
pipeline and remains inert until separate explicit activation.

No encryption or race-free TOCTOU protection is claimed.

## Task 335 evidence

The certified production source is recorded separately from this future release
source:

- Certified production source:
  `b90e63dd138732d616582d935b1fa45b842fcfe8`.
- Certification verdict: `PASS WITH EXPLICIT NONCLAIMS`.
- Required marker: `RAH_V027_REMEMBERED_WORKSPACE_LIVE_OK`.
- Certified production exact-head CI: `35054734280` — PASS.
- Certification documentation head:
  `c2b3c070dbcc44ec877dc1f298657690d938fe98`.
- Task 335 documentation exact-head CI: `35055170395` — PASS.

The host-driven evidence covered P1/P2/P3 restart isolation, fresh
`RepositoryMemberId`, admission/activation separation, delete isolation,
current-facts rejection, corrupt/future catalog behavior, privacy, zero
startup candidate-path access, A/B Git integrity, zero model requests, zero
model Tool requests, zero MCP activations, zero Process Plugin activations,
zero automatic commits, zero network Git, and bounded cleanup. This was not
GUI certification.

## Required nonclaims

```text
MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.
```

Also retained: GUI automation was not certified; a real junction/reparse live
fixture was not executed; true simultaneous two-process live mutation was not
executed; no network isolation, race-free TOCTOU, or automatic recovery/replay
guarantee after uncertain native effects is claimed.

## Version and source gate

All 13 workspace packages transition from `0.26.0` to `0.27.0` and remain on
Rust edition 2024. Cargo.lock may change only the 13 internal RAH package
version records; dependency versions, checksums, sources, features, and the
dependency graph must remain unchanged.

The certification source above is not the release source. The final Task 336
release-preparation commit will become the immutable candidate source after
exact-head validation. Its SHA is intentionally not fabricated inside its own
commit; Task 336 closure reports it, and Task 337 may record it for
publication.

## Validation record

The final local deterministic and release validation is recorded in the Task
336 plan. The release build must identify as `rah-desktop v0.27.0`.

## Publication stop boundary

Task 336 stops after the release-preparation commit is pushed with
`HEAD == origin/master`, a clean worktree, and passing exact-head CI. It does
not create or push `v0.27.0`, create a GitHub Release, publish release notes,
mark the changelog released, or change the immutable v0.26.0 release identity.
