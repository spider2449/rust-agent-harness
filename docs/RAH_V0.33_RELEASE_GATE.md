# RAH v0.33.0 Completed Release Record

Status: **v0.33.0 = RELEASED**.

## Immutable publication

- Release source and peeled tag target: `ad25355c81ed0a39499cd75e5b240a7594f3e7a5`.
- Annotated tag: `v0.33.0`; annotation: `RAH v0.33.0`.
- Tag object: `2a187c6e634b330cea4d45f457497f165c6f84f2`.
- Release-source master push CI: `36861427522` - PASS.
- Tag CI: `36862035589` - PASS, exact release source, event `push`.
- GitHub Release ID: `400948501`; published `2026-10-01T12:31:40Z`;
  `draft=false`; `prerelease=false`.
- [Published release](https://github.com/spider2449/rust-agent-harness/releases/tag/v0.33.0).

Task 486 established READY_FOR_RELEASE. Task 487 published the annotated tag
and GitHub Release without a repository commit or source-code change. Both
GitHub and internal master remained at the release source during publication.
Task 488 is later documentation cleanup, not the release source. The release
tag remains immutable on the SHA above.

See the [Task 487 record](plans/2026-10-01-task-487-v0.33-publication.md) and
[Task 488 report](plans/2026-10-01-task-488-v0.33-post-release-state-cleanup.md).

## Source and version

- Preparation base: `44083e48d0911cf43103737fe08d7602b96e3535`.
- At Task 486 start, GitHub and internal master both advertised that SHA. The
  internal master advanced normally from `49f1bdf7f4354e37c057673d27e43ae8e54b9288`.
- The workspace has 13 packages and 13 members, all released at 0.33.0,
  edition 2024. No external dependency or feature change is intended.
- Preparation commit `de966dfa48aed1a7ad464b357d3c2928005945a4` reached
  both masters; exact-head push CI `36860934153` passed. Documentation
  closeout `ad25355c81ed0a39499cd75e5b240a7594f3e7a5` became the
  immutable release source; exact-head push CI `36861427522` passed.

## Capability and authority

ADR 0031 owns `repo.edit-untracked-file`: bounded correction of one existing,
regular, non-ignored UTF-8 file in the active admitted repository. The target
must be absent from HEAD and all index stages, including intent-to-add. Exact
whole-file SHA-256 and byte-length preconditions and bounded exact literal
replacements apply. The Tool revalidates repository state immediately before
one prepared replacement attempt and reports uncertainty conservatively. It
leaves the target untracked; it does not stage, commit, edit tracked or ignored
files, grant arbitrary filesystem writes, or select another repository.

This is ordinary repository Tool authority, outside HostExplicit. HostExplicit
remains exactly 11. ADRs 0012–0014 and their existing tracked-file and
creation scopes are unchanged. The host owns repository selection and Tool
authority; model and provider metadata do not grant permission.

## Evidence and required validation

Task 484's production composed `authorized_tool_dispatch` test changed
`worker_count=44` to `worker_count=4` while the file stayed untracked, with no
staging, unexpected temporary file, or unrelated change. Its final Windows
Desktop gate passed 324 tests with 20 intentionally ignored. That live Tool
workflow remains applicable if Task 486 changes only release metadata/docs.

Task 486 required `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace`, warnings-denied all-target/all-feature workspace
Clippy, Cargo metadata verification, frontend/static permission and inventory
checks, Desktop release check/build, the canonical Windows Desktop gate, and
`git diff --check`. Actual results were recorded in the Task 486 report before the
release-preparation commit.

Task 486's clean serial workspace run passed 1,010 tests, with 24 intentionally
ignored. Its clean serial canonical Windows Desktop gate passed 324 tests,
with 20 intentionally ignored. Format, check, Clippy, release-profile Desktop
check, metadata, five frontend/static tests, and diff integrity passed. The
first workspace and Windows gate attempts had fixture/setup or Desktop test
failures; their exact results are retained in the Task 486 report. The later
passes do not establish a cause for those earlier failures.

The certified Codex runtime baseline remains exact `codex-cli 0.157.1`. Task
485 found no reason to repeat full provider/runtime recertification for this
ordinary Tool addition. Task 484's host-driven production dispatch evidence
does not claim a model-selected live Tool turn.

Both configured masters and exact-head push CI satisfied preparation
closure. Task 487 completed tag publication, successful tag CI, and GitHub
Release publication. Task 488 independently reverified the immutable
publication identities before documentation cleanup.
