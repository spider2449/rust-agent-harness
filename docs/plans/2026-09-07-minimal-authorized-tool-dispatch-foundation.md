# Task 237 — Minimal Authorized Tool Dispatch Foundation

## Status

IMPLEMENTED — AWAITING EXACT-HEAD CI

## Starting checkpoint

- Repository: `spider2449/rust-agent-harness`.
- Required baseline: `HEAD == origin/master == 00537acab427303ea714da09bba6bfde8ed4c598`.
- Task 236 exact-head CI: `34073532081` PASS.
- Worktree was clean. The workspace has 13 packages, all `0.19.0`, edition 2024.

## ADR 0021 contract

ADR 0021 selects D2: a neutral `rah-tools` authorized-dispatch primitive. It is distinct from runtime/model dispatch and from capability authorization. It admits only an exact current registered definition permitted by the host-owned current permission policy, then delegates to ordinary `ToolRegistry` execution.

## Existing ToolRegistry boundary

`ToolRegistry::execute` remains the neutral lower-level lookup and `Tool::execute` boundary. It does not receive a permission policy and has not changed. D2 wraps it for callers that need definition/current-permission admission.

## D2 API

`authorized_tool_dispatch(&ToolRegistry, &ToolDefinition, &[PermissionLevel], ToolCall, ToolContext)` returns `Result<ToolOutput, AuthorizedDispatchError>`. Its inputs are only registry, expected current definition, host-owned allowed permissions, call, and context.

## Check ordering

The primitive checks call/expected public name equality, current registry lookup, exact current/expected definition equality, and current permission membership in that order. Only then it calls `ToolRegistry::execute` exactly once.

## Definition identity

`ToolDefinition` equality is used directly and completely: name, description, JSON schema, and permission. It performs no schema canonicalization or partial comparison. A current permission change is therefore a `DefinitionMismatch` even if the new permission would otherwise be allowed.

## Permission semantics

Permission is taken from the current registered definition and admitted only by exact membership in the caller-supplied list. There is no permission hierarchy, default accepted set, or inferred authority.

## Structured rejection

`AuthorizedDispatchError::Rejected(AuthorizedDispatchRejection)` distinguishes `NameMismatch`, `UnknownTool`, `DefinitionMismatch`, and `PermissionDenied`. Each is pre-dispatch and does not execute the Tool.

## Underlying Tool result/error preservation

After admission, `ToolRegistry::execute` errors return as `AuthorizedDispatchError::Tool(ToolError)`. Successful `ToolOutput`, including `is_error: true`, is returned unchanged. D2 does not interpret Tool output and does not retry.

## No authority amplification

D2 constructs no capability, repository, Git, profile, provider, executable, or host-execution authority. Tool-specific policy and preconditions remain authoritative.

## No lifecycle / eligibility ownership

D2 emits no lifecycle events and owns no provenance. It has no Desktop eligibility policy, Tauri types, frontend data, Codex aliases, turns, provider metadata, or host tickets.

## Deterministic tests

Local fixture Tools use only configurable definitions/results and atomic execution counts. Tests cover exact output preservation; explicit membership for None, Read, Write, and Execute; order/duplicate-insensitive membership; all admission rejections with zero execution; stale description/schema/name/permission changes; underlying errors; `is_error` output; and exactly-once/no-retry behavior.

## Codex bridge deferral

`rah-runtime-codex/src/bridge.rs` is unchanged. Its current dynamic bridge comparison does not include permission in its definition equality condition. Migrating it to D2 would deliberately harden stale-permission behavior and is deferred to Task 238.

## Desktop deferral

`rah-desktop` is unchanged. D2 provides no host invocation, IPC, ticket, HostExplicit activity, eligibility, model-turn exclusion, or live certification.

## Dependency / Cargo audit

No dependencies, Cargo manifests, lockfile, crates, or protocol surface were added or changed.

## Validation

Passed sequentially:

- `cargo fmt --check`
- `cargo check --workspace`
- `cargo test -p rah-tools authorized_dispatch -- --nocapture` (13 focused tests)
- `cargo test -p rah-tools` (190 unit tests)
- `cargo test -p rah-runtime-codex` (82 passed, 1 ignored)
- `cargo test --workspace` (passed; host-only live tests remained ignored)
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1` (13 packages, all `0.19.0`, edition 2024)

The first workspace attempt encountered Windows linker error LNK1104 while an earlier Cargo invocation still held the Desktop test executable. After all active Cargo processes exited, the serial rerun completed successfully.

The first exact-head CI run caught `clippy::result_large_err` with the CI Rust/Clippy version. `DefinitionMismatch` retains both complete definitions but boxes them so the public structured error remains compact. Required validation is rerun after this corrective change.

## Files changed

- `crates/rah-tools/src/authorized_dispatch.rs`
- `crates/rah-tools/src/lib.rs`
- `docs/plans/2026-09-07-minimal-authorized-tool-dispatch-foundation.md`

## Commit

Pending `feat: add authorized tool dispatch foundation` after scope audit.

## Exact-head CI

Pending push of the Task 237 commit and completed successful push CI for that exact SHA.

## Next task

Task 238 — Authorized Dispatch Equivalence and Codex Bridge Hardening; not started.
