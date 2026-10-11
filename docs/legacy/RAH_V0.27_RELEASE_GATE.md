# RAH v0.27.0 Release Gate

## Status

**RELEASED — HISTORICAL RECORD**

This gate is the historical Task 336 release-preparation record and Task 337
publication record. RAH v0.27.0 is released; RAH v0.26.0 is the prior immutable
published release. Task 338 is a later documentation-only descendant and is not
the immutable v0.27.0 release source.

## Immutable publication record

- Release source: `1297787df07c725e41deb758cc7ecab5593b28d8`.
- Annotated tag: `v0.27.0`.
- Tag object: `587bb55747ac1c9cdf1c02bc56e7dff244ede93b`.
- Peeled target: `1297787df07c725e41deb758cc7ecab5593b28d8`.
- Release-preparation CI: `35061634078` — PASS.
- Tag CI: `35063921592` — PASS.
- GitHub Release ID: `389683736`.
- Release name: `RAH v0.27.0`.
- Published: `2026-09-16T06:31:29Z` (`draft=false`, `prerelease=false`).
- Assets: 0.

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

The Task 335 certification source above is not the release source. Task 337
subsequently published the immutable release source recorded above. Task 338
is a later documentation-only descendant and is not the release source.

## Validation record

The final local deterministic and release validation is recorded in the Task
336 plan. The release build must identify as `rah-desktop v0.27.0`.

## Publication stop boundary

Task 336 stopped after the release-preparation commit was pushed with
`HEAD == origin/master`, a clean worktree, and passing exact-head CI, as
required. Task 337 subsequently created and pushed `v0.27.0`, waited for
passing tag CI, and created the GitHub Release recorded above. Neither Task 337
nor Task 338 changes the immutable v0.26.0 release identity.
