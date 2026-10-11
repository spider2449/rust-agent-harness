# Task 184 — Repository Directory Creation Composition

## Scope

Verify the Task 183 `repo.create-directory` Tool through the normal host
composition, ToolRegistry, and Generic Codex Tool Bridge path. Keep authority
host-owned, preserve the public ToolName and closed request schema, and avoid
Desktop, Trusted Profile schema, live Codex, and Windows live-certification
work.

## Starting state

- Starting `HEAD`: `f12683df7ecfe3501bfe64668a81757fd74de156`
- Starting `origin/master`: `f12683df7ecfe3501bfe64668a81757fd74de156`
- Working tree: clean
- RAH v0.14.0 and its release state remain immutable.
- ADR: `docs/adr/0019-bounded-repository-directory-creation-authority.md`
- Task 183 baseline: `docs/plans/2026-09-03-core-repository-directory-creation-implementation.md`

## Composition decision

The inspected Trusted Profile schema does not express `repo.create-directory`.
Following ADR 0019 and Task 183 sequencing, this task does not add profile
schema or Trusted Profile loading changes. The lower-level neutral CLI
composition contract accepts an optional, already host-constructed
`RepositoryDirectoryCreationAuthority` and registers the Tool only when that
authority is supplied. The default composition path supplies no authority and
therefore does not publish the capability.

## Inspected path

The verified path is:

```text
host RepositoryDirectoryCreationAuthority
  -> rah-cli effective composition
  -> ToolRegistry
  -> generic rah-runtime-codex dynamic Tool bridge
  -> private Codex-compatible alias
  -> normal public ToolRegistry dispatch
  -> structured ToolOutput JSON
```

The bridge's existing generic alias mapping, permission check, request
forwarding, result translation, evidence, and no-replay behavior are reused.
No `repo.create-directory` special case is added.

## Planned changes

- Add the narrow CLI composition hook for an explicit directory authority.
- Add deterministic bridge coverage for authority presence/absence, exact
  public schema, private alias mapping, exact request routing, one dispatch,
  structured verified result, and public-name preservation.
- Record that duplicate registration, permission enforcement, cancellation,
  disconnect, and replay guarantees are inherited from generic contracts.

## Boundaries and validation

No Desktop integration, profile authority manufacturing, release/version or
dependency changes, live Codex execution, Windows live certification, or core
Task 183 semantic changes are in scope. Validate with focused changed-crate
tests and the required workspace, formatting, lint, diff, and metadata gates.

## Completion notes

Changed files/crates:

- `crates/rah-cli/src/profile_composition.rs`: added the explicit optional
  host-authority composition hook and registration.
- `crates/rah-runtime-codex/src/bridge_tests.rs`: added the deterministic
  end-to-end composition/bridge fixture and assertions.
- This plan document.

With no directory authority, normal composition succeeds but the registry does
not contain `repo.create-directory`. With an explicitly constructed
`RepositoryDirectoryCreationAuthority`, the registry contains the public
`repo.create-directory` Tool with `PermissionLevel::Execute`. Its exact schema
is the single string `path` field with no additional properties. The generic
Codex bridge maps it to the deterministic test alias `rah_tool_0`; that alias
is private and non-contractual. The public ToolName remains unchanged.

One private-alias request carrying exactly
`{"path":"existing/new-directory"}` produced one registry/tool invocation,
created one directory leaf, and preserved the structured result
`directory_created_verified`, `uncertain: false`, and
`git_metadata_changed: false`. The bridge emitted one requested, started, and
finished lifecycle event. No Codex special case, alias leakage, retry, replay,
or authority escalation was added.

Duplicate ToolName rejection, permission enforcement, cancellation,
disconnect, and possible-effect no-replay behavior remain generic existing
contracts; no contrived directory-specific production hooks were added.

Trusted Profile code and schema did not change and cannot manufacture this
authority. Desktop code did not change. No live certification was performed.
Versions remain `0.14.0`, editions remain 2024, package count is 12, and no
dependency changes were made.

Validation passed:

- `cargo fmt --check`
- `cargo test -p rah-runtime-codex host_composed_repo_create_directory_uses_generic_bridge_once --lib`
- `cargo test -p rah-cli --lib`
- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `git diff --check`
- `cargo metadata --no-deps --format-version 1`

Recommended next task: Task 185 — Desktop host-owned
`repo.create-directory` integration and lifecycle hardening.
