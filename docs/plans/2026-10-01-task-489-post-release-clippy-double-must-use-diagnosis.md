# Task 489 - Post-release Clippy double_must_use diagnosis

## Starting state and scope

Clean dedicated worktree: `F:\coding\otherPrj\rah-task-489`, branch
`task-489-clippy-diagnosis`. Starting HEAD and origin/master both exactly
`1a35ce36ff6f8e751e3c1ce0d7edcbb0f03131a7`; `git status --short` returned
no entries before diagnosis and again after local Clippy. Independent GitHub
and internal-mirror master queries agreed with that SHA. The dirty primary
checkout was inspected and preserved without edits.

This task changes only this report. No production source, Cargo metadata,
dependencies, ADRs, Tool contracts, authority, or release artifacts change.
RAH remains 0.33.0, HostExplicit 11, Codex baseline 0.157.1; no v0.34
capability is selected.

## Exact source and identity

Affected item: public inherent consuming method on `AgentHandle` in
`crates/rah-runtime/src/lib.rs`, lines 59-63:

```rust
    /// Consumes the handle and returns its event stream.
    #[must_use]
    pub fn into_events(self) -> AgentEventStream {
        self.events
    }
```

The attribute has no message. Return alias:

```rust
pub type AgentEventStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;
```

The alias itself has no attribute. Its underlying `futures::Stream` trait
(reexported from futures-core 0.3.34) is already marked
`#[must_use = "streams do nothing unless polled"]` at upstream
`futures-core-0.3.34/src/stream.rs:36`. This was inspected in the local Cargo
registry. The failing diagnostic explicitly recognizes the pinned boxed
Stream trait object. The plain method attribute adds no distinct reason;
removing it is the narrow correction suggested by the new diagnostic.

`git log -L 55,65:crates/rah-runtime/src/lib.rs` traces the attribute and method
to `4ccff6af4906fef980032aabc108f123426cc01b` (2026-08-20,
`feat: add agent runtime abstraction`), predating v0.33.

Release source `ad25355c81ed0a39499cd75e5b240a7594f3e7a5` and starting master
contain the exact same affected file: both Git blob IDs are
`9669305d007d18ac7581fae817d7f4afb50fd10f`. This proves byte/content identity
of the committed source, not merely similarity around the lint line.
`git diff` across those commits for this file, Cargo.lock and `.github`
returned no differences. Cargo.lock blob in both is
`77c5481c87faa408d5b5fe4c8a66789de61d7532`. The complete intervening diff
contains only the eight Task 488 documentation paths.

## Passing and failing CI comparison

Full logs and run metadata were retrieved with `gh run view` for all three
runs. Run links are primary evidence:

| Run | Head | Result | Effective rustc |
| --- | --- | --- | --- |
| [36861427522](https://github.com/spider2449/rust-agent-harness/actions/runs/36861427522) | ad25355c81ed0a39499cd75e5b240a7594f3e7a5 | PASS | 1.98.1 (48a229cea 2026-09-01) |
| [36862035589](https://github.com/spider2449/rust-agent-harness/actions/runs/36862035589) | ad25355c81ed0a39499cd75e5b240a7594f3e7a5 | PASS, tag push | 1.98.1 (48a229cea 2026-09-01) |
| [36864219183](https://github.com/spider2449/rust-agent-harness/actions/runs/36864219183) | 1a35ce36ff6f8e751e3c1ce0d7edcbb0f03131a7 | FAIL | 1.99.0 (b940084d7 2026-09-28) |

All three used runner 2.337.0, Ubuntu 24.04 image 20260927.320.1,
runner-image provisioner 20260901.588, host x86_64-unknown-linux-gnu,
`actions/checkout@v7` resolved to
`3d3c42e5aac5ba805825da76410c181273ba90b1`, and
`dtolnay/rust-toolchain@stable` resolved to
`6bed0761d98439e5a578e2877258200ad565ba87`.

The failing install step records, at 2026-10-01T12:48:18Z, the latest stable
update to 1.99.0, removal of the previous clippy/cargo/rustc components, and
installation of five components. At 12:48:25.5485490Z it records exactly:

```text
stable-x86_64-unknown-linux-gnu updated - rustc 1.99.0 (b940084d7 2026-09-28) (from rustc 1.98.1 (48a229cea 2026-09-01))
```

Passing installations recorded 1.98.1 unchanged. LLVM changed from 22.1.8
to 23.1.1. CI does not separately print `cargo -V` or `cargo clippy -V`;
their exact version strings cannot be quoted from these logs. The bundled
Clippy component replacement is directly logged, and the failing lint links
to the rust-1.99.0 Clippy documentation. Do not invent separate CI version
strings from the rustc version.

All runs execute exactly:

```powershell
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

No explicit `--target`; all targets means Cargo target kinds on the native
host. Features, preceding fmt/check/test commands, shell
`/usr/bin/bash --noprofile --norc -e -o pipefail`, and visible Clippy environment
(`CARGO_HOME=/home/runner/.cargo`, `CARGO_INCREMENTAL=0`,
`CARGO_TERM_COLOR=always`) agree. No workflow RUSTFLAGS or feature override.
Workflow contains no Cargo build-cache restore/save step; setup reports cache
mode write in all runs. All logs compile the same futures/futures-core 0.3.34;
the lockfile is identical. No dependency-resolution or cache explanation is
established. All workflow/action/runner inputs examined agree; effective
stable toolchain changed. Unlogged runner environment is not fully certified.

Task 488 fmt, workspace check and tests passed. Clippy exited 101; Desktop
permission validation was skipped. Passing runs completed Clippy and Desktop
permission validation successfully.

## Exact failing diagnostic

Task 488 diagnostic below preserves the diagnostic text with ANSI color and
CI timestamp prefixes removed:

```text
error: this function has a `#[must_use]` attribute with no message, but returns a type already considered as `#[must_use]`
  --> crates/rah-runtime/src/lib.rs:61:5
   |
61 |     pub fn into_events(self) -> AgentEventStream {
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
note: the return type is pinned boxed `futures::Stream` trait object
  --> crates/rah-runtime/src/lib.rs:61:33
   |
61 |     pub fn into_events(self) -> AgentEventStream {
   |                                 ^^^^^^^^^^^^^^^^^
help: remove `must_use`
  --> crates/rah-runtime/src/lib.rs:60:5
   |
60 |     #[must_use]
   |     ^^^^^^^^^^^
   = note: alternatively, you may add an explicit reason to the `must_use` attribute
   = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.99.0/index.html#double_must_use
   = note: `-D clippy::double-must-use` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(clippy::double_must_use)]`

error: could not compile `rah-runtime` (lib) due to 1 previous error
warning: build failed, waiting for other jobs to finish...
```

## One bounded local reproduction

Executed the exact canonical command once in the initially clean diagnostic
worktree, with the normal local environment and fresh worktree target output.
No controlled variant or retry was required: the CI install logs already prove
drift. Result: exit 0, no double_must_use diagnostic:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 08s
```

Measured local versions:

```text
rustc 1.98.1 (48a229cea 2026-09-01)
commit-hash: 48a229ceaefd4985c50990b14116b6d856af0985
host: x86_64-pc-windows-msvc
LLVM version: 22.1.8
cargo 1.98.1 (797e8a9bc 2026-08-05)
clippy 0.1.98 (48a229ceae 2026-09-01)
```

Local CARGO_HOME is `F:\rust\.cargo`. Windows local host differs from Linux
CI; the local pass is on the older passing toolchain, not a certification of
1.99.0. No host configuration or installed toolchain was changed.
Full raw CI logs and local log are retained outside Git under
`F:\coding\otherPrj\rah-task-489-evidence` (local PowerShell redirection log
is UTF-16LE). This report contains the decisive evidence independently.

## Classification and disposition

**B - TOOLCHAIN / CLIPPY DRIFT PROVEN.** Unchanged source passed both release-era
1.98.1 gates, then the same CI command failed immediately after stable installed
1.99.0 and replaced Clippy. The local canonical 1.98.1 run also passes. This
is not a demonstrated Task 488 source regression, invocation/configuration
drift, or transient failure. No precise upstream implementation-change commit
is claimed; effective toolchain drift is directly established.

Repository policy evidence: `rust-toolchain.toml` explicitly selects
`channel = "stable"`, minimal profile, clippy and rustfmt; CI independently
selects `dtolnay/rust-toolchain@stable`. Neither pins 1.98.1. Existing policy
therefore selects rolling stable rather than an immutable compiler version.
Recommend one bounded next maintenance task: adopt the current stable lint by
removing only the redundant plain `#[must_use]` on `AgentHandle::into_events`,
then validate against the effective current stable canonical gate and exact-head
CI. Do not suppress the lint or pin the older compiler merely to recover green.
If maintainers want a pinned compiler, that is a separately authorized policy
decision. No correction is performed in Task 489.

v0.33.0 remains published. Remote annotated tag object remains
`2a187c6e634b330cea4d45f457497f165c6f84f2`, peeled to
`ad25355c81ed0a39499cd75e5b240a7594f3e7a5`; GitHub Release 400948501 remains
historical. No retag, republish, or release-note change is authorized.

## Documentation validation and publication

`git diff --check` is required before this report's sole-path commit.
No behavior tests are added; the single canonical local Clippy execution is
the diagnostic validation. Commit subject: `docs: diagnose post-release Clippy failure`.
Normal pushes, final SHA, exact-head CI and final clean-state proof will be
recorded in the completion response after execution, as in Task 488. This file
cannot contain its own commit SHA or future CI outcome. A repeat of the same
diagnosed lint in that natural push run is expected evidence; do not retry it
or begin the correction automatically.
