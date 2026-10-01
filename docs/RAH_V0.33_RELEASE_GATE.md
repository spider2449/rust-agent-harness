# RAH v0.33.0 Release-Preparation Gate

Status: **RELEASE PREPARATION VALIDATED — unpublished**. No v0.33.0 tag or
GitHub Release is authorized by this record. Task 487 owns publication.

## Source and version

- Preparation base: `44083e48d0911cf43103737fe08d7602b96e3535`.
- At Task 486 start, GitHub and internal master both advertised that SHA. The
  internal master advanced normally from `49f1bdf7f4354e37c057673d27e43ae8e54b9288`.
- The workspace has 13 packages and 13 members, all prepared at 0.33.0,
  edition 2024. No external dependency or feature change is intended.
- The exact preparation commit, configured-destination pushes, and exact-head
  CI are closure checks after the preparation commit.

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

Task 486 must pass `cargo fmt --check`, `cargo check --workspace`,
`cargo test --workspace`, warnings-denied all-target/all-feature workspace
Clippy, Cargo metadata verification, frontend/static permission and inventory
checks, Desktop release check/build, the canonical Windows Desktop gate, and
`git diff --check`. Record actual results in the Task 486 report before the
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

After the preparation commit, both configured masters must advertise its
exact SHA and GitHub push CI for that SHA must pass. Until then this is a
preparation gate, not a completed release record.
