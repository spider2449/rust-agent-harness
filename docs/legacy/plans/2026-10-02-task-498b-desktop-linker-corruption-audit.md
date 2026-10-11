# Task 498B — Isolated Desktop linker corruption audit and validation recovery

## Checkpoint, scope and source freeze

Starting HEAD: `5954189bf721527461efa673b7838d51cf5a20f3`, exactly the
authoritative checkpoint. Supplied Task 497 exact-head CI `37015819945` PASS
remains historical baseline evidence. Worktree `F:/temp/rah-task498`, branch
`task-498-neutral-errors`, initially had 11 modified tracked and 6 untracked
intended files, all unstaged. No relevant validation process was running.
Initial git diff --check passed. No reset, clean, stash, rebase, toolchain
change, product edit, or live provider/model validation was performed.
Task 495 remains untouched; Task 499 has not started.

Before building, git status and diff --check were captured; git diff was saved
as `F:/temp/rah-task498b-source-snapshot.patch`, SHA-256
`F8CB5BE43A2E38B967370A06C9CC01FCBFA987759BE72B83DAC387AA0DAA43D7`.
Hashes of all 17 modified/untracked files were captured separately, including
untracked source and reports which git diff does not contain. All 17 hashes
were verified unchanged after isolated linking and again after all validation.
No source or documentation edits occurred during validation. Only this report
and the Task 498A disposition annotation were written after all gates exited.
Evidence is outside the repository at `F:/temp/rah-task498b-evidence`.

## Original failure and preserved artifact

Original full linker log remains at
`F:/temp/rah-task498a-desktop-focused/20261002-222416-489-6b2d92c154f140788aba957bebed4716/desktop.stderr.log`.
A byte-preserving copy is `F:/temp/rah-task498b-evidence/original-desktop.stderr.log`.
Original Cargo exit was 101; linker exit 1223; no Desktop tests executed.

```text
libtauri-1f50c6f83fc386ac.rlib(tauri-1f50c6f83fc386ac.tauri.3773942bfd718f42-cgu.12.rcgu.o) : fatal error LNK1223: invalid or corrupt file: file contains invalid .pdata contributions
```

Original effective target was explicitly supplied to the gate as
`F:/temp/rah-task498-target`. Current shell CARGO_TARGET_DIR was unset.
The gate sets both CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR from its parameter.
Offending archive: `F:/temp/rah-task498-target/debug/deps/libtauri-1f50c6f83fc386ac.rlib`.
Size: 56,081,168 bytes. Creation and last-write UTC: 2026-10-02 14:25:53
(full metadata in original-artifact.json). SHA-256:
`2BF310AEB0B97BB4EA42B6574F994642183885AD9EA734F6E165FFD7DC32DA0A`.
The object is an archive member, not a separately preserved .obj file.
The old target and offending artifact were never modified, deleted or cleaned.
The original log retains the reported linker invocation; Cargo omitted some
arguments. No more complete historical invocation is claimed.

## Toolchain identity

- rustc 1.98.1, commit `48a229ceaefd4985c50990b14116b6d856af0985`, 2026-09-01;
  host x86_64-pc-windows-msvc; LLVM 22.1.8.
- cargo 1.98.1 (`797e8a9bc`, 2026-08-05).
- rustdoc 1.98.1 (`48a229cea`, 2026-09-01).
- where.exe link found no PATH entry; bare link invocation was unavailable.
  Cargo auto-discovered the linker in Visual Studio 18 BuildTools:
  `C:/Program Files (x86)/Microsoft Visual Studio/18/BuildTools/VC/Tools/MSVC/14.51.36231/bin/HostX64/x64/link.exe`.
- Explicit read-only invocation of that binary reports Microsoft Incremental
  Linker 14.51.36256.0. No active VC/VS/SDK/INCLUDE/LIB environment variables
  matched the recorded inventory; no development-shell environment was added.

Full command outputs and shell lookup errors are preserved in toolchain.txt.
No linker flags, compiler versions, dependencies, manifests or lockfile changed.

## Isolated build and infrastructure disposition

`F:/temp/rah-task498-target-isolated` was verified absent before the first build.
Nothing was copied from the failed cache. With CARGO_TARGET_DIR set to that path:

```powershell
cargo test -p rah-desktop --bin rah-desktop --no-run
```

This reproduces the canonical script's default debug test build/link phase,
without executing tests before helper preparation. Inspection established that
the failed gate did not use release mode, additional features, explicit target,
or custom linker flags. Fresh build/link PASS, exit 0, 3m 05s. Exact log:
`F:/temp/rah-task498b-evidence/isolated-link.log`.
Executable: `F:/temp/rah-task498-target-isolated/debug/deps/rah_desktop-ca1764551ecc1481.exe`.
Fresh archive of the same name has SHA-256
`BB8AFFA837A0A029CA0B8070923C605F7698F00C9993B576C13A573C3A38DD1C`;
its metadata is in isolated-artifact.json. Different archive hashes alone do
not explain the internal cause of the invalid original .pdata contributions.

**I1 — STALE/CORRUPT BUILD ARTIFACT CONFIRMED** under the requested fresh-build
criterion. The unchanged source links successfully. I2/I3/I4 were not observed;
no source-triggered defect, storage cause, or particular compiler bug is claimed.

Canonical gate then ran once using this same isolated target:

```powershell
powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:\temp\rah-task498-target-isolated -OutputDirectory F:\temp\rah-task498b-desktop
```

PASS, helper exit 0, Desktop exit 0, outer exit 0; 325 passed, 0 failed,
20 ignored. Gate elapsed 363.690 seconds, tests 318.74 seconds, no watchdog.
Evidence: `F:/temp/rah-task498b-desktop/20261002-223652-070-66f9847daafd46eb9b0a2242e17b888e`.
The old target is retained as contaminated build-cache evidence.

## Fresh deterministic validation recovery

All commands ran serially with CARGO_TARGET_DIR and, for workspace tests,
RAH_TEST_TARGET_DIR set to the isolated path. Earlier pre-correction workspace
check results were not reused. The already passing Task 498A focused suites
were not separately rerun; their coverage also executes in the required full
workspace run.

| Gate | Actual result |
| --- | --- |
| cargo fmt --check | PASS, exit 0 |
| cargo check --workspace | PASS, exit 0 |
| cargo test --workspace | PASS, exit 0; 1,020 passed, 0 failed, 24 ignored, including empty doctest suites |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS, exit 0 |
| git diff --check | PASS |
| node --check crates/rah-desktop/frontend/status.js | PASS |
| status_authority_test.js | PASS |
| remembered_workspace_test.js | PASS |
| remembered_workspace_layout_test.js | PASS |
| repository_membership_test.js | PASS |
| node crates/rah-desktop/tauri_permission_test.js | PASS: 47 runtime, 47 manifest, 47 generated, 47 default allows, 47 frontend commands |
| cargo metadata --no-deps --format-version 1 | PASS, exit 0; 13 packages/members, isolated target |
| HostExplicit static enum inventory | PASS: exactly 11 expected variants |
| HostExplicit executable allowlist and composition inventory | PASS in both canonical Desktop and workspace tests |

Workspace counts sum actual Cargo test-result lines; Desktop workspace count
is 325/0/20 and rah-tools is 353/0/0. Detailed workspace logs, metadata JSON,
frontend-tauri.log and hostexplicit-static.txt are in the audit evidence folder.
Executable checks are host_invocation::tests::host_allowlist_is_exact and
effective_authority::tests::snapshot_composition_keeps_the_eleven_host_explicit_kinds.
No live tests were enabled. HostExplicit remains FsRead, RepoFileInfo,
RepoStatus, RepoDiff, RepoDiffStaged, RepoCreateBranch, RepoPatch, RepoEditFiles,
RepoCreateFile, RepoDeleteFile, RepoRenameFile; no authority expansion.

## Authority, security and publication boundary

Task 498 implementation remained byte-for-byte unchanged throughout recovery.
No authority, ToolRegistry, Trusted Profile, remembered-workspace authority,
permission, lease, mutation-uncertainty, provider-admission or dependency change
was made by Task 498B. ADR 0032 is the preserved Task 498A implementation
decision; existing authority ADRs remain unchanged. No tag, release or version
bump. No model/list, gpt-6.1-sol, Codex 0.160.0 or Task 495 preflight validation.

Both remote master readbacks equaled the starting checkpoint before publication.
Classification A authorizes one intended-files-only Task 498 implementation
commit, recommended message `refactor: preserve typed runtime failure sources`,
then normal master pushes to GitHub and internal mirror and natural exact-head
CI verification. The final response records the resulting SHA and remote/CI
readbacks without adding a self-referential commit hash to this document.
Build artifacts and external logs are not staged. Task 499 is not started.

## Final Task 498B classification

**A — TRANSIENT BUILD ARTIFACT CORRUPTION CLEARED; TASK 498 VALIDATION COMPLETED**

All required local gates passed on the unchanged implementation. Historical
Task 498/498A failures remain preserved; this report records their authorized
validation recovery, not a retroactive rewrite of those results.
