# Task 490 — Current-stable Clippy compatibility maintenance

## Checkpoint and scope

Starting HEAD and origin/master: `6b271cbd44d89dfa8ca6d56b65d5bb1715cbb7ac`.
Both configured master destinations independently advertised this exact SHA.
Task 489 worktree was clean. Dedicated clean worktree created at this SHA:
`F:\coding\otherPrj\rah-task-490`, branch `task-490-clippy-maintenance`.
The primary checkout contains frozen unrelated WIP at an older SHA and is preserved.

Task 489 classified B — TOOLCHAIN / CLIPPY DRIFT PROVEN. Byte-identical
release/current source passed Rust 1.98.1 and failed rolling stable Rust 1.99.0;
exact-head CI 36865347618 reproduced `clippy::double_must_use`.

## Source inspection and correction

Confirmed the exact plain `#[must_use]` on `AgentHandle::into_events`; no
message-bearing method contract exists. Removed only that attribute from
`crates/rah-runtime/src/lib.rs`. The signature remains
`pub fn into_events(self) -> AgentEventStream`; its body remains `self.events`.
`AgentEventStream` remains `Pin<Box<dyn Stream<Item = AgentEvent> + Send>>`.
Inspected the pinned futures-core 0.3.34 trait in the local registry: `Stream`
has `#[must_use = "streams do nothing unless polled"]`. Return-type must-use
semantics remain intact. Public signature, docs text and behavior are unchanged;
only redundant method annotation metadata is absent. Existing runtime unit,
minimal-runtime, conformance and Codex provider tests consume this method.
No new test asserting Clippy internals is appropriate.

No runtime/stream/provider semantic, dependency, package version, ADR,
Tool authority, HostExplicit or CI toolchain policy change. No v0.34 capability
selected. v0.33.0 tag object `2a187c6e634b330cea4d45f457497f165c6f84f2`
peels to `ad25355c81ed0a39499cd75e5b240a7594f3e7a5`; Release 400948501
was inspected before work (updated_at 2026-10-01T12:31:40Z) and is immutable.

## Validation and disposition

Toolchain installed in task-local RUSTUP_HOME outside Git, with isolated target
output and RAH_TEST_TARGET_DIR pointing to that target. Host configuration and
existing toolchain are unchanged. Versions actually measured:

- rustc 1.99.0 (b940084d7 2026-09-28)
- cargo 1.99.0 (5f94df478 2026-08-27)
- clippy 0.1.99 (b940084d7e 2026-09-28)

Focused commands, each executed once and passed (exit 0), in required order:

1. `cargo fmt --check`
2. `cargo check -p rah-runtime`
3. `cargo clippy -p rah-runtime --all-targets --all-features -- -D warnings`
4. `git diff --check`

Full matrix, each executed once and passed (exit 0), in required order:

1. `cargo fmt --check`
2. `cargo check --workspace`
3. `cargo test --workspace`: **1,010 passed, 0 failed, 24 ignored**, summed
   from every test-result line, including integration and doc-test targets.
   Desktop: 324 passed, 20 ignored. Runtime unit, minimal runtime and conformance
   tests passed; Codex provider event-stream tests passed.
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
5. `git diff --check`

Repository CI additionally requires the Desktop Tauri permission inventory.
`node crates/rah-desktop/tauri_permission_test.js` passed: 47 runtime, manifest,
generated and default allows; 47 frontend commands. No additional frontend or
live Desktop gate is mandated for this attribute-only maintenance by current
policy. The ordinary Windows Desktop tests ran as part of the full workspace.

The entire product-source diff is one removed attribute. Method body, return
alias, signature and documentation text are byte-unchanged; generated API
content is therefore unchanged except the redundant method annotation metadata.
No standalone rustdoc generation is claimed. Existing stream regression
coverage passed. No new tests, suppression, version pin or policy change.

**A — CURRENT-STABLE CLIPPY COMPATIBILITY RESTORED.** Focused and workspace
Clippy, format/check/tests and diff checks passed on current stable. No
semantic/API behavior change. Commit and normal publication are now authorized.

## Publication record

Starting master SHA remains `6b271cbd44d89dfa8ca6d56b65d5bb1715cbb7ac`.
Only the runtime attribute removal and this report are staged for
`fix: restore current stable Clippy compatibility`. Both destination pushes,
final commit SHA and natural exact-head CI (including effective Rust version,
Clippy and overall result), final clean state and release invariants will be
recorded in the completion response and task-local publication evidence after
execution. This committed report cannot contain its own SHA or future CI result;
no amendment or additional publication commit is planned.
Raw validation/publication evidence is retained outside Git under
`F:\coding\otherPrj\rah-task-490-evidence` (PowerShell command logs are UTF-16LE).
No v0.34 capability is selected.
