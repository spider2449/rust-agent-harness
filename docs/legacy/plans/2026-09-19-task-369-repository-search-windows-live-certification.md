# Task 369 - `repo.search` Windows Live Certification

Date: 2026-09-19

## Verdict

**PASS WITH EXPLICIT CODEX NONCLAIM**

All non-Codex Windows live gates passed on the corrected working tree. The
required `codex-cli 0.149.0` baseline was unavailable and no newer Codex was
substituted.

## Source identity and scope

- Starting committed production checkpoint: `152a5fff96f621083a3ec43892ecdeffc2f6ff62`
  (`feat: add bounded repository search`).
- The checkpoint was clean before certification.
- Certification exposed one real production-path blocker: the Desktop
  active-repository registry did not register the already implemented
  `RepositorySearchTool`. The profile-composition path and effective-authority
  classification already handled it.
- The working-tree certification candidate therefore contains one narrow
  production correction: Desktop registry construction now registers
  `repo.search` for the selected repository. This is not a new authority,
  permission, schema, dependency, persistence, network, or mutation path.
- Certification-only coverage was added to the Desktop Windows live test and
  Generic Codex Bridge tests. No commit, tag, release, or version bump was
  made during certification.

## Environment

| Item | Recorded value |
| --- | --- |
| Windows | Windows 11 IoT Enterprise LTSC, version `10.0.26100`, build `26100`, x64 |
| rustc | `1.98.1 (48a229cea 2026-09-01)`, host `x86_64-pc-windows-msvc`, LLVM `22.1.8` |
| Cargo | `1.98.1 (797e8a9bc 2026-08-05)` |
| Native Git | `2.55.0.windows.5` |
| Workspace | 13 packages, all `0.30.0`, Rust edition 2024 |
| Codex commands | `C:\Users\morefunfun\AppData\Roaming\npm\codex`, `.cmd`, and PowerShell shim |
| Codex detected | `codex-cli 0.155.1` |
| Certified baseline | `codex-cli 0.149.0 unavailable`; ambient `0.155.1` was not substituted |

The metadata command was run directly on this checkout and reported 13
packages, one workspace version (`0.30.0`), and one edition (`2024`).

## Native fixture and production path

The live harness created a disposable native-Git repository containing main,
linked A, and linked B worktrees. All three shared one Git common directory
while retaining distinct private worktree Git directories, indexes, and
memberships. Main, A, and B received distinct tracked sentinel files.

The exercised current production path was:

```text
native Git fixture
 -> production admission and active-member selection
 -> Desktop active-repository DesktopRepository
 -> desktop_tool_registry
 -> effective_authority::compose
 -> ToolRegistry
 -> repo.search RepositorySearchTool
```

The Generic Tool Bridge test additionally exercised:

```text
ToolRegistry -> private dynamic alias -> Generic Codex Tool Bridge
 -> repo.search -> normalized ToolOutput -> continuation events
```

The live model-selected Codex path was not run because the exact approved
baseline was unavailable.

## Live evidence

- Path mode returned tracked A paths in deterministic lexical order with
  literal, case-sensitive matching. `path_prefix: "src"` returned only the
  two `src/repository_search_*` paths and excluded the tracked `src2` sibling.
- Text mode returned only repository-relative paths and 1-based line numbers,
  with no snippets and `consistency = "best_effort"`. CRLF input returned the
  expected line number.
- Current-worktree proof returned `CURRENT_WORKTREE_SENTINEL` and did not
  return the committed/index-only `OLD_INDEX_SENTINEL`.
- Tracked modified, staged-new, and tracked-ignore-matching files were found.
  Untracked and ignored files were absent, proving tracked inventory rather
  than filesystem or ignore enumeration.
- NUL/binary and files larger than 1 MiB were absent from text matches and
  reported through the corresponding omission counters.
- With A active, A-only content was found and main/B-only content was absent.
  After switching to B, a fresh B composition found B-only content and did
  not find A-only content. No union registry or model repository selector was
  used.
- The three worktrees shared common Git state but their private Git state was
  distinct. Search outputs and effective authority serialization contained no
  private Git directory, common Git directory, registration, backlink,
  commondir, absolute worktree root, or filesystem identity.
- A descendant nested repository caused the complete search to fail closed.
- A tracked junction-backed escape candidate caused the complete search to
  fail closed without disclosing its external sentinel.
- Windows symlink creation was unavailable on this host. The run emitted
  `RAH_V031_WINDOWS_SYMLINK_LIVE_NONCLAIM=1`; Windows symlink live behavior is
  not certified.
- Path saturation returned exactly 128 deterministic results with
  `complete = false` and `truncation_reason = "result_limit"`.
- HEAD, selected branch/ref, porcelain-v2 status, and selected index bytes for
  A, main, and B were equal before and after the normal full search suite.
  Search caused no intentional repository mutation. Boundary fixture setup
  and teardown were intentionally outside that comparison.

## Composition, authority, and privacy

- The current Desktop path is host-owned direct repository composition; it does
  not use the older Trusted Profile registry as its active repository source.
  The existing Trusted Profile observer contract remains the exact closed
  configuration:

  ```json
  {"name":"repo.search","enabled":true,"permission":"execute","executable":"git","repository":"workspace"}
  ```

- Effective composition registered `repo.search` with `PermissionLevel::Execute`
  and classified it as `ReadOnly` / `RepositoryObservation` / repository-bound
  / first-party repository host.
- `host_kind("repo.search") == None`; the production HostExplicit set remains
  exactly 11. No `RepoSearch` HostExplicit kind was added.
- ToolOutput, authority serialization, and tested activity surfaces remained
  redacted. Only bounded repository-relative paths and line numbers were
  exposed; private topology, executable, cwd, environment, raw stderr,
  generations, and authority handles were not exposed.
- The certification-only native fixture helper captures setup Git output
  instead of inheriting it, keeping private fixture topology out of the live
  harness console.

## Codex and bridge result

The exact baseline was unavailable:

```text
codex-cli 0.149.0 was unavailable on the certification host.
A newer ambient Codex version was not substituted.
Real Codex inference/model-selected repo.search was not certified.
```

Without model inference, the Generic Tool Bridge test still advertised the
real `repo.search` definition under its private alias, dispatched one real
bridge request into the production ToolRegistry/tool, returned the expected
JSON result, and observed requested/started/finished lifecycle before
continuation. No fake model event was treated as Codex evidence.

## Changed files

- `crates/rah-desktop/src/main.rs` - narrow Desktop registry registration and
  Windows certification test/harness support.
- `crates/rah-runtime-codex/src/bridge_tests.rs` - certification-only real
  bridge dispatch test for `repo.search`.
- `docs/plans/2026-09-19-task-369-repository-search-windows-live-certification.md`
  - this certification record.

## Validation

The focused live checks passed:

```text
cargo check -p rah-desktop
cargo check -p rah-desktop --tests
cargo test -p rah-runtime-codex repository_search_dispatches_through_the_generic_bridge -- --test-threads=1
$env:RAH_RUN_V031_REPOSITORY_SEARCH_LIVE='1'; cargo test -p rah-desktop task369_windows_repository_search_live_certification -- --ignored --nocapture --test-threads=1
```

The required final workspace validation passed after the certification changes
were complete:

```text
cargo fmt --check
cargo check --workspace
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo metadata --no-deps --format-version 1
```

`cargo test --workspace -- --test-threads=1` exited 0. The final workspace
suite included 314 passing Desktop tests, 84 passing Codex-runtime tests, and
326 passing `rah-tools` tests; all other workspace crates and integration/doc
tests also passed, with only the repository's existing ignored tests skipped.
No dependency drift was observed. Workspace metadata remains 13 packages,
version `0.30.0`, edition 2024.

## Explicit nonclaims

This Windows evidence does not claim Linux or macOS live certification,
cross-platform parity, OS sandboxing, network isolation, race-free TOCTOU,
machine-wide external Git locking, rollback, compensation, replay after
uncertain effects, untracked or ignored-file search, sibling-worktree search,
Git worktree lifecycle authority, multiple-active-repository authority,
persistent executable repository authority, generic shell/process/Git/
filesystem authority, GUI automation, or model-selected Codex routing.

Windows symlink live behavior and real `codex-cli 0.149.0` inference remain
uncertified. Task 370 must decide whether the explicit Codex nonclaim is
sufficient for v0.31 release readiness.

## Checkpoint disposition

The live evidence is complete for the corrected working-tree candidate. The
working tree is intentionally left uncommitted and reviewable. A later Task
369 checkpoint commit must include the narrow Desktop registration correction,
certification-only coverage, and this document. Task 370, version `0.31.0`,
tagging, publication, and release preparation were not started.
