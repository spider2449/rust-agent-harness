# Task 385 — `rah-desktop` Main Test Child Mechanical Extraction

## Scope and starting checkpoint

This records the mechanical relocation of the existing Windows/test-only child
module from `crates/rah-desktop/src/main.rs` to
`crates/rah-desktop/src/main_tests.rs`. No semantic refactor was intended.

Starting HEAD was `a1d240da2568e61ff117d8728307891a6e04b826`
(`docs: plan rah-desktop main structural decomposition`). Before editing,
`git status --short` was empty, `git rev-parse HEAD` matched that SHA, and the
two preceding commits were `9e2e4c2` and `ca96d1e`.

Task 384's starting measurements were 35,532 physical lines and 34,350
nonblank lines in `main.rs`; its approximate composition was 9,805 production
lines and 25,727 test-only lines, including roughly 12.5k lines of live and
certification harness code.

## Module identity and extraction

The original declaration began at line 9,929 and was exactly:

```rust
#[cfg(all(test, target_os = "windows"))]
mod tests {
```

There were no comments or other attributes immediately attached to this
declaration. The child module's contents continued to the final closing brace
at end of file. The complete body was 25,601 physical source lines before
formatting.

The resulting declaration in `main.rs` is:

```rust
#[cfg(all(test, target_os = "windows"))]
#[path = "main_tests.rs"]
mod tests;
```

`tests` remains a child of the same parent module, preserving its identity,
privacy relationship, cfg gate, test paths, and Windows/test-only compilation
conditions. The extracted file contains the child module's contents directly;
it does not add a nested `tests` module. No production visibility was widened.

The original source substring inside the braces was copied to the new file,
then Rustfmt formatted the standalone child-module file. To check equivalence,
the pre-edit module and the formatted external body were each reconstructed
under the same original `#[cfg]` and `mod tests` wrapper and formatted with
Rustfmt. Their 25,604-line module outputs differed in only three small hunks,
each changing indentation around multiline `return Err` expressions. No code,
string contents, comments, or attributes differed in those hunks. The
pre/post `fn` and `async fn` item-name inventories were both 365 names with
zero differences. This, along with the test-list comparison below, verifies
that the move preserved module items while allowing Rustfmt-required layout.

## Test and harness inventory

Before extraction, the serial Desktop test command passed with 315 passed,
zero failed, and 18 ignored (333 total; 738.51 seconds). The captured
`cargo test -p rah-desktop -- --list` inventory contained 333 test names.

After extraction, the same list command contained 333 names with zero
differences from the baseline. The serial Desktop test command passed with
315 passed, zero failed, and 18 ignored (333 total; 741.91 seconds). The
Task 369 repository-search and Task 379 Windows `repo.list` certification
tests remain discoverable and ignored behind their existing environment
gates. Task 385 did not rerun live certification.

## Measurements

Physical lines after extraction and Rustfmt:

| File | Lines |
| --- | ---: |
| `main.rs` | 9,931 |
| `main_tests.rs` | 25,270 |
| Combined | 35,201 |

`main.rs` decreased by 72.05% from its 35,532-line starting size. The new
external file is 331 lines shorter than the verbatim pre-format body because
Rustfmt reformatted the child at standalone-file scope; the reconstructed
module comparison above confirms those layout changes do not change its code
or literal contents.

## Validation

All requested checks passed on Windows:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo check -p rah-desktop` | Passed |
| `cargo test -p rah-desktop -- --test-threads=1` | 315 passed, 18 ignored |
| `cargo check --workspace` | Passed |
| `cargo clippy -p rah-desktop --all-targets --all-features -- -D warnings` | Passed |
| `git diff --check` | Passed; no trailing whitespace in either untracked file |

## Scope and authority impact

Production runtime semantics: unchanged. The only change to `main.rs` is the
test-only external module declaration. Tauri command registration and
`invoke_handler` were not changed. The Task 384 command/permission generation
mismatch was not touched; no permission or capability files were changed.
There are no production visibility, authority, architecture, dependency,
Cargo configuration, version, or release changes.

## Files, Git state, and follow-up

Expected and actual changed files:

```text
crates/rah-desktop/src/main.rs
crates/rah-desktop/src/main_tests.rs
docs/plans/2026-09-23-task-385-main-test-child-mechanical-extraction.md
```

No commit, push, or tag was created. The intended final HEAD remains
`a1d240da2568e61ff117d8728307891a6e04b826`. The extraction is intentionally
left uncommitted for the independent Task 386 audit. Task 386 remains
required before checkpointing.
