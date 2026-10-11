# RAH v0.26.0 Release Gate — released historical record

## Status

**RELEASED — HISTORICAL RECORD.** RAH v0.26.0 is the current immutable
published release. Task 327 records the post-release documentation cleanup;
this later descendant is not the release source.

## Immutable publication identity

- Release source: `8b22b18739a54b752ce11e791fe380e522c10049`.
- Annotated tag: `v0.26.0`.
- Tag object: `f60ee2465c1819b59b5671f20387b74734ace019`.
- Peeled source: `8b22b18739a54b752ce11e791fe380e522c10049`.
- Tag message: `RAH v0.26.0`.
- Release-preparation CI: `34965819523` — PASS.
- Tag CI: `34966821012` — PASS.
- GitHub Release ID: `389111343`.
- GitHub Release name: `RAH v0.26.0`.
- Published: `2026-09-15T12:07:53Z`.
- `draft=false`; `prerelease=false`; assets: 0.

RAH v0.25.0 remains the prior immutable published release:

- annotated tag: `v0.25.0`;
- tag object: `ea3c31aaf5190b632d7ef86387f7aff6004ae664`;
- peeled source: `a8b4d7b92f545a37d2ef2c8eae224f91c9d939c4`;
- GitHub Release ID: `387406579`, `draft=false`, `prerelease=false`.

Task 325 prepared the immutable v0.26.0 source at the release source above.
Task 326 published the annotated tag and GitHub Release without a repository
commit. Task 327 is the later documentation-only cleanup descendant.

## Release theme and authority boundary

The theme is **Explicit Multi-Repository Membership and Active-Only Repository
Switching**. Desktop can explicitly admit multiple repositories into one
process-local membership set, while exactly one human/host-selected repository
has executable authority. Only the active repository receives its
`DesktopRepository`, ToolRegistry/composition, workflow actions, Stage/Unstage
authority, Commit currentness, HostExplicit repository-bound currentness, and
conversation repository context. Inactive members remain inert.

The v0.26 composition is:

```text
trusted/human admission -> process-local membership -> inert members
  -> one active member -> fresh DesktopRepository
  -> fresh active-only ToolRegistry/composition -> repository-bound authority
```

Membership is descriptive/organizational, not filesystem or Git authority.
There is no union registry, workspace-wide authority, cross-repository
operation, model/provider repository selection, or parallel active repository.
Switching A→B→A creates fresh active-only authority and invalidates stale
repository-bound state. Membership is not persisted and member removal is
absent by design; restart restores zero membership authority.

## Milestone evidence layers

1. **Deterministic/security milestone audit:** Task 323 READY — **v0.26
   MULTI-REPOSITORY MILESTONE READY FOR WINDOWS LIVE CERTIFICATION**.
2. **Real Windows host-driven authority/effect certification:** Task 324-C-R4
   PASS — **WINDOWS HOST-DRIVEN MULTI-REPOSITORY AUTHORITY/EFFECT
   CERTIFICATION PASSED**.
3. **Model-selected GPT-5.6 Terra dynamic Tool dispatch:** NOT ESTABLISHED.

The historical task sequence is: Task 322 stop-expansion decision; Task 323
READY milestone audit; Task 324 original model-driven live attempt BLOCKED;
Task 324-A external/generic model-selection block; Task 324-B host-driven
certification disposition; Task 324-C attempts and corrections; Task 324-C-R4
final host-driven PASS; Task 325 release preparation; Task 326 immutable
publication; and Task 327 documentation-only post-release cleanup.

Historical intermediate evidence remains distinct: the original Task 324 was
BLOCKED; Task 324-A found an external/generic model-selection block; Task
324-B selected Recommendation B; and Task 324-D corrected the observer and
app-server ownership evidence. These records explain why the final release
claim is host-driven and why process evidence changed. They are not rewritten
as a model-driven PASS.

## Task 324-C-R4 evidence

- live certification source:
  `e59319166dc0776458c9c0bee61f4c73d9d6399c`;
- final certification documentation head:
  `21bc151efab969dcd725842459c049f2d6dd74c8`;
- marker: `RAH_V026_HOST_DRIVEN_MULTI_REPOSITORY_LIVE_OK`;
- Windows: Microsoft Windows 10 Professional, build 19045, 64-bit;
- Rust/Cargo: `1.96.0`;
- Git: `2.54.0.windows.1`;
- Codex: `0.149.0`;
- Codex SHA-256:
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

Essential evidence:

- admissions 2; duplicate admission successes 0; active sequence A→B→A;
- A marker reads 2; B marker reads 1; cross-repository read rejected;
- switch while Connected successes 0;
- retained A/B runtime and app-server ownership proof PASS; A/B
  Disconnect/reap PASS;
- old A Stage effects 0 across switches; B Stage effects 1; B Unstage effects 1;
- no-union registry PASS; fresh-host restored repository authority none;
- current-run temporary-root cleanup PASS; new leaked roots 0; remaining
  attributable app-server 0;
- model requests 0; model Tool requests 0; model-selected switches 0;
  automatic commits 0; network Git operations 0;
- A/B Git integrity PASS.

The five pre-existing Task 324 temporary roots are historical failed-run
artifacts, not v0.26 release artifacts or successful evidence. R4 proved
pre-existing count 5, post-run count 5, new leaked roots 0, and absence of the
current R4 root after explicit cleanup. R4 did not delete the historical roots.

## HostExplicit, Stage/Unstage, Commit, and providers

The exact HostExplicit set remains 11:

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
v0.26 does not change HostExplicit eligibility. Stage/Unstage are separate host
index authority and repository/currentness-bound actions; an old A action does
not revive after A→B or A→B→A. Commit remains separate repository-bound
reviewed authority. Trusted Profile remains global host-owned composition; MCP
and Process Plugin do not select repositories. R4 ran with MCP 0 and Process
Plugin 0, so no provider-specific live certification is claimed.

## Required nonclaim

`MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED GPT-5.6-TERRA LIVE GATE.`
Task 324-A showed real Codex/model turns completed but selected no RAH dynamic
Tool. Task 324-B applied the established policy that model selection is not
host authorization. The Windows evidence certifies the host-owned
authority/effect plane and real Codex process lifecycle, not current GPT-5.6
Terra model-selected Tool behavior. This is not a RAH authority failure and
not model Tool PASS evidence.

## Validation and release conditions

The preparation changes only workspace version metadata and documentation.
Rust production/test source, frontend JS/HTML, Tauri permissions, ADRs,
trusted-profile/provider schemas, model defaults, Codex baseline, Tool schemas,
Tool names, PermissionLevel, provider protocols, IPC, HostExplicit eligibility,
membership semantics, persistence, and removal are unchanged.

Task 325 closure records PASS for the full deterministic validation, Desktop
release build, frontend/Tauri checks, focused regressions, metadata, and
dependency-drift audit. Task 327's cleanup is documentation-only and leaves
those facts unchanged. The workspace remains 13 packages, all `0.26.0`, all
edition 2024. Cargo.lock contains only the 13 internal RAH package-version
updates; no external crate version, checksum, source, or dependency drift.

The release retains these limitations: Windows 10 evidence does not establish
Windows 11/Linux/macOS live parity; TOCTOU hardening is not race-free proof;
process supervision is not OS sandboxing; there is no network isolation or
rollback guarantee; uncertain external effects are not replayed; persistence,
removal, parallel active repositories, cross-repository operations,
model/provider-selected routing, HostExplicit `repo.commit`, and network Git
are not release claims.

## Publication checklist — completed

- [x] Task 325 prepared the immutable source and passed release-preparation CI.
- [x] Task 326 created the annotated `v0.26.0` tag at the exact source.
- [x] Tag object and peeled target were verified.
- [x] Tag CI `34966821012` passed.
- [x] GitHub Release `389111343` was published non-draft and non-prerelease.
- [x] Task 327 converted preparation wording to this released historical record.

The Task 327 cleanup commit is a later docs-only descendant. It does not
change the immutable release source, tag object, peeled target, or GitHub
Release metadata.
