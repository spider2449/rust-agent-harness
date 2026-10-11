# Task 162 — Desktop `repo.delete-file` Integration

## Scope

Integrate the existing host-constructed `RepositoryFileDeletionAuthority` into
the selected-repository Desktop ToolRegistry and deterministic Desktop tests.
Preserve ADR 0017, the Task 161 schema and Generic Codex Tool Bridge, the
human Stage/Unstage boundary, and the reviewed commit workflow.

## Implementation boundaries

- Construct deletion authority only at the trusted selected-repository host
  boundary and retain it inside the Rust-owned repository context.
- Register `repo.delete-file` only from that opaque authority; do not add a
  frontend command, frontend authority state, or privileged filesystem path.
- Bind the existing registry/runtime to the selected repository and current
  repository/model generations. Invalidate reviewed commit state on observed
  repository mutation and rely on the existing refresh path.
- Add deterministic tests for authorized dispatch, missing authority, stale
  preimage, repository switching/generation binding, unchanged index/no
  auto-stage, review revocation, no replay, and frontend trust boundaries.

## Validation and completion gates

Run focused Desktop/composition/runtime tests, then `cargo fmt --check`,
`cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`git diff --check`, and `cargo metadata --no-deps --format-version 1`.
Commit and push one coherent Task 162 change and require exact-head CI PASS.
Windows live Codex validation remains deferred.
