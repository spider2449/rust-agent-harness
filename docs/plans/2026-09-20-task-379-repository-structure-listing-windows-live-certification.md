# Task 379 - `repo.list` Windows Live Certification

Date: 2026-09-20

## Verdict

**FAIL — LIVE CERTIFICATION BLOCKER**

The exact committed production checkpoint was not live-certified. The real
Windows Desktop production path returned a sanitized boundary-observation
failure during repeated nested navigation on a disposable native Git linked
worktree fixture. This was not converted into a nonclaim because it occurred
on a required normal repository-listing path.

## Source identity and scope

Required starting checkpoint:

```text
ca96d1e5252bd4f320014981c562b6af27df71cf
feat: add bounded repository structure listing
```

Parent:

```text
093d156d0f4cb3bc57ba0d13dcc0bc4b2d9ae1b1
```

The starting worktree was clean and `HEAD` matched the required checkpoint.
`origin/master` was `2f9bd83957a7cefbd83e2ca377fd2125b8376616` and was not
updated. No push, tag, release, or version bump was performed.

The only source change made during this task is certification-only Desktop
test harness coverage for the real active-repository registry path. No
production correction was made because the failure's exact cause was not
established safely within Task 379 scope.

## Environment

| Item | Recorded value |
| --- | --- |
| Windows | Microsoft Windows 11 IoT Enterprise LTSC, version `10.0.26100`, build `26100`, x64 |
| rustc | `rustc 1.98.1 (48a229cea 2026-09-01)`, host `x86_64-pc-windows-msvc`, LLVM `22.1.8` |
| Cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Native Git | `git version 2.55.0.windows.5` |
| Workspace | 13 packages, all `0.31.0`, edition 2024 |
| Codex discovery | `C:\Users\morefunfun\AppData\Roaming\npm\codex`, `.cmd`, and PowerShell shim; detected `codex-cli 0.155.1` |
| Adopted Codex baseline | `codex-cli 0.149.0` unavailable; `0.155.1` was not substituted |

## Live production path

The harness created a disposable native Git repository outside the RAH source
tree with main, linked A, and linked B worktrees sharing common Git storage
but retaining distinct private worktree state. It used:

```text
host admission and active selection
 -> Desktop active-repository composition
 -> desktop_tool_registry
 -> effective authority composition
 -> repo.list
```

The Desktop production registry contained `repo.list` with the closed input
schema and `PermissionLevel::Execute`. Effective authority classified it as
`ReadOnly` / `RepositoryObservation` / repository-bound / repository-host.
`HostExplicit` remained exactly 11; `repo.list`, `repo.search`, and
`repo.create-directory` remained ineligible.

## Live evidence before the blocker

- Root `{}` returned a successful, tracked-only, repository-relative direct
  projection. It included `Cargo.toml`, `a-only.txt`, `crates`, and `docs`,
  and did not include untracked-only, ignored-only, physical-empty,
  physical-untracked, deleted-only, or nested grandchildren.
- The `crates` request returned exactly `crates/alpha` and `crates/beta` as
  synthesized direct-child directories, and `crates/alpha` returned exactly
  `crates/alpha/src`.
- Main/A/B identity and common/private Git topology were checked, and the
  effective-authority serialization was redaction-checked.
- The root output reported `consistency = "best_effort"`, deterministic
  repository-relative UTF-8-byte ordering, and omission accounting.

## Blocking live evidence

The required next navigation request, using the same real production registry,
failed before the remaining Task 379 gates could complete:

```text
request: {"path":"crates/alpha/src"}
result: Execution { message: "Git repository policy rejected capability: repository boundary observation failed" }
```

The fixture contained ordinary tracked `crates/alpha/src/lib.rs`, a modified
current-worktree copy, and no nested repository or reparse entry under that
path. Repeating the run reproduced the failure at the source-directory
listing stage; the root, `crates`, and `crates/alpha` calls had already passed.
This blocks certification of the complete live matrix. The exact production
root cause remains open.

## Gates not certified because of the blocker

The following are intentionally not claimed from this failed run: staged-new
file live projection, live tracked-ignore and deleted-path completion,
sparse-checkout behavior, saturation, nested-repository rejection, junction
escape, Windows symlink behavior, Gitlink/submodule behavior, active B
switching completion, no-intentional-mutation closure, privacy closure over the
complete output set, and a successful full live `repo.search` smoke sequence.

The deterministic Task 377 conflict-projection test remains the applicable
evidence for file/directory conflict hardening; the conflict was not naturally
reproduced in this failed Windows fixture. Windows symlink creation and the
exact Codex `0.149.0` model-selected gate remain explicit nonclaims. The
Task 377 timeout record remains unchanged:

```text
rah-tools::repository_diff_staged::
unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty
```

It remains classified as a non-reproduced transient timeout after the three
current-tree isolated passes, clean `093d156` baseline non-reproduction, and
final full serial workspace pass.

## Generic bridge and deterministic coverage

The existing `repo.list` Generic Tool Bridge test was retained and exercised
the dynamic private alias, normalized output, and requested/started/finished
event ordering. Deterministic `rah-tools` coverage continues to cover root and
nested projection, request validation, saturation, conflict fail-closed
behavior, and nested-repository rejection. Those deterministic results do not
replace the failed Windows production-path gate.

## Validation

The following checks were run after the harness change:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo test -p rah-tools -- --test-threads=1
cargo test -p rah-runtime-codex repository_list_dispatches_through_the_generic_bridge -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
```

All passed. The serial workspace run completed with exit code 0; the
deterministic `rah-tools` suite reported 332 passed and the focused generic
bridge test passed. Metadata remained 13 packages, all `0.31.0`, edition 2024.
Ignored live tests were not run by the ordinary workspace command. No release
or publication validation was authorized or started after the live blocker.

## Changed files

- `crates/rah-desktop/src/main.rs` - certification-only ignored Windows live
  harness and production registry/effective-authority assertions.
- `docs/plans/2026-09-20-task-379-repository-structure-listing-windows-live-certification.md`
  - this failed certification record.

## Disposition

Do not prepare, tag, publish, or push `ca96d1e` as a live-certified candidate.
The recommended next task is to diagnose the exact Windows boundary/current
worktree failure on the smallest reproducer, independently of release
preparation, then rerun the affected live gates under an explicitly authorized
follow-up task.
