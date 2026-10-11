# Task 386 — `main_tests.rs` Mechanical Extraction Independent Audit

## Verdict

**PASS — MECHANICAL EXTRACTION INDEPENDENTLY VERIFIED**

The Task 385 extraction preserves the test child module's item bodies,
namespace, cfg, privacy relationship, and runnable/ignored test inventories.
Task 386 made no Rust changes and created no commit, push, or tag.

## Checkpoint and scope

- Starting and final audit HEAD: `a1d240da2568e61ff117d8728307891a6e04b826`
  (`docs: plan rah-desktop main structural decomposition`).
- Initial dirty paths were exactly:
  - `crates/rah-desktop/src/main.rs`
  - `crates/rah-desktop/src/main_tests.rs`
  - `docs/plans/2026-09-23-task-385-main-test-child-mechanical-extraction.md`
- Initial `git diff --check` passed. No unrelated paths were present.
- Task 386's only file change is this audit document. No Rust file was
  modified during the audit.

## Independent body comparison

The original source was independently read with `git show` from the starting
HEAD. The body was taken from the unique original declaration
`#[cfg(all(test, target_os = "windows"))] mod tests { ... }`, excluding only
that outer wrapper. The current `main_tests.rs` body was then wrapped under the
same cfg and `tests` module. Both reconstructed modules were formatted with
Rustfmt using edition 2024 and compared.

The reconstructed outputs were each 25,604 lines. The only differences were
three whitespace-only indentation/reflow hunks around multiline `return Err`
expressions for the V028 inactive-member removal live gate and the V029 active
repository close live gate. No identifiers, literals, attributes,
expressions, assertions, timeout values, paths, comments, or item order
changed. The independent body comparison therefore found **zero semantic
differences**.

## Item inventory

The independently reconstructed old body and current extracted body have
identical item counts, names, and source order:

| Item kind | Before | After |
| --- | ---: | ---: |
| Function items | 365 (121 async, 244 sync) | 365 (121 async, 244 sync) |
| Structs | 21 | 21 |
| Enums | 2 | 2 |
| Impl blocks | 17 | 17 |
| Const/static items | 3 | 3 |
| Type aliases | 4 | 4 |
| Nested modules | 0 | 0 |
| `macro_rules!` definitions | 2 | 2 |
| Lexically inventoried macro tokens | 2,765 | 2,765 |

No item was missing, added, duplicated, renamed, or reordered. There are no
`include!` or `cfg_if!` invocations and no nested modules in the moved body.
Both `macro_rules!` definitions retain their original relative positions, so
textual macro scope and invocation ordering are preserved.

## Test and fixture inventories

The explicit source-level test attributes match exactly: 231 before and 231
after, with the same function names and order. The runtime inventory is the
complete runner view, including runners not represented by a direct source
attribute. A clean detached worktree at the starting HEAD and the current
checkout both returned 333 names from `cargo test -p rah-desktop -- --list`;
all fully qualified names matched exactly and remain under `tests::<name>`.

The ignored-only runtime inventory also matched exactly: 18 before and 18
after. The same 18 names were returned by
`cargo test -p rah-desktop -- --list --ignored`. The extracted file itself
contains the same 17 `#[ignore]` attributes as the old body; the additional
package-level ignored runner is outside this moved module.

Task 369's `task369_windows_repository_search_live_certification` and Task
379's `task379_windows_repository_list_live_certification` remain present,
discoverable under their unchanged `tests::` names, and ignored behind their
same environment gates. No Task 382-named harness is represented in the moved
file. Live certification was not run. The helper and fixture inventory is
preserved by the complete body comparison and matching function/type/macro
inventories; helpers remain inside the private test child module.

## Module identity, cfg, and privacy

The original inline child was at the crate root with the name `tests` and cfg
`all(test, target_os = "windows")`. The replacement is at the same location
and has the same child name and cfg, with only
`#[path = "main_tests.rs"]` added. The extracted file is the child module's
contents directly and does not introduce another module layer. Rust therefore
retains the same parent, child namespace, private parent access, and test
paths. There is no unconditional `mod main_tests;` declaration.

`main_tests.rs` remains excluded from normal production builds by the existing
`test` cfg. `cargo check -p rah-desktop` and `cargo check --workspace` both
passed. The only installed compilation target in this environment is
`x86_64-pc-windows-msvc`; no non-Windows target compile was available. The
non-Windows conclusion is limited to the unchanged cfg expression: because
`target_os = "windows"` is false there, the test child is not selected. No
cross-platform live certification is claimed.

## Production prefix and authority-sensitive behavior

The production prefix of `main.rs` before the test declaration is byte-for-
byte identical to the baseline through line 9,928. The only `main.rs` diff is
the replacement of the inline test body with the external-module declaration.
This establishes zero production visibility changes and no changes to
functions, imports, commands, state, registration, constants, cfgs, or
production comments.

The unchanged production prefix also confirms no changes to repository
membership, active switching, ToolRegistry composition, Trusted Profile,
Effective Authority, HostExplicit, review/currentness, commit authorization,
runtime/session lifecycle, or provider/model behavior.

The Tauri command attributes and `invoke_handler` registration list are
identical before and after: 47 commands, same names, order, attributes, and
signatures. The Task 384 permission/capability finding remains unchanged: 44
commands are covered by the build/autogenerated/default capability inventory;
the three registered HostExplicit prepare handlers still absent are
`host_prepare_repo_edit_files`, `host_prepare_repo_create_file`, and
`host_prepare_repo_delete_file`. This audit did not change or repair that
separate finding.

## Source formatting and line counts

Independently measured physical line counts:

| File/body | Before | After |
| --- | ---: | ---: |
| `main.rs` | 35,532 | 9,931 |
| Moved test body / `main_tests.rs` | 25,601 | 25,270 |
| Combined files after extraction | — | 35,201 |

The 331-line reduction is from formatting the moved body as a standalone
child-module file: removing its common inline indentation and Rustfmt's
standalone-scope reflow and blank-line normalization let some formerly
multiline expressions occupy fewer physical lines. The same source, when
wrapped back in its original module and formatted, expands to 25,604 lines and
diffs from the formatted baseline only in the three whitespace-only hunks
listed above. Item, macro-token, and test inventories all match. Physical
line-count equality was not used as the semantic proof.

Git reports the tracked `main.rs` hunk as 2 insertions and 25,603 deletions;
the new `main_tests.rs` is untracked at this audit checkpoint and therefore
does not appear in ordinary `git diff --stat` output. Independent normalized
body comparison establishes this is a mechanical relocation, not a rewrite or
refactor.

## Validation

All requested commands passed:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo check -p rah-desktop` | Passed |
| `cargo check --workspace` | Passed |
| `cargo test -p rah-desktop -- --list` | 333 names; exact match to baseline |
| `cargo test -p rah-desktop -- --list --ignored` | 18 names; exact match to baseline |
| `cargo test -p rah-desktop -- --test-threads=1` | 315 passed, 18 ignored, 0 failed |
| `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --workspace -- --test-threads=1` | Passed; all workspace targets completed |
| `git diff --check` | Passed |

The historical `repository_diff_staged` timeout did not recur: that test suite
completed with 7 passed in 24.07 seconds during the workspace run.

`cargo metadata --no-deps --format-version 1` reported 13 packages, all at
version `0.31.0` and edition `2024`. `Cargo.toml` and `Cargo.lock` are not
changed; dependencies and features are unchanged.

## Final state and next task

HEAD remains `a1d240da2568e61ff117d8728307891a6e04b826`. No commit, push, or tag
occurred. Task 386 changes only this audit document; the three Task 385 dirty
paths remain as audit input. No production or test Rust code was changed.

Next: **Task 387 — `main_tests.rs` Mechanical Extraction Checkpoint**. After
Task 387 checkpoints the exact audited extraction, production-domain
decomposition research should proceed as a separately numbered task.
