# Task 028 plan: trusted static capability profile implementation

## Scope

Implement the smallest v0.4 trusted-host composition boundary: an explicit
static JSON profile is strictly parsed and version-checked, resolves symbolic
host resources, constructs only existing built-in capabilities, and returns a
fresh `ToolRegistry` with a redacted effective inventory.

The supported initial capabilities are `fs.read`, `host.cargo.version`, and
`host.git.status`. MCP, Process Plugin, Git stage, and Git unstage composition
remain out of scope because this task must not invent missing adapter or
mutation-policy configuration seams.

## Design

1. Add a `rah-tools` profile module with a closed, version-1 JSON schema.
   Reject duplicate JSON keys, unknown fields, unknown capabilities, invalid
   symbolic identifiers, duplicate capability declarations, mismatched fixed
   permissions, and invalid enabled capability bindings.
2. Require a caller-selected absolute profile path. Treat it as a trusted host
   input; do not search, include, interpolate, or reload profiles.
3. Keep raw paths limited to top-level symbolic resource declarations. Resolve
   them only while constructing existing `FsReadTool`, `CargoVersionTool`, and
   `GitStatusTool` constructors, which retain their native and repository
   identity checks and execution policies.
4. Build the registry locally and return it only after every enabled
   capability validates and registers. Convert constructor failures into
   bounded redacted profile errors.
5. Return a redacted immutable inventory containing profile/capability IDs,
   symbolic resource IDs, fixed RAH permissions, and effective state only.

## Tests

Add deterministic tests for a valid profile, unknown fields/version/capability,
duplicate keys and capability names, permission mismatches, unresolved symbolic
resources, all-or-nothing registration, and absence of paths/secrets in errors
and inventory.

## Validation

```powershell
cargo fmt --check
cargo test -p rah-tools
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
git status --short
git diff
```

## Acceptance

The loader never registers partial authority, does not introduce a provider or
Codex dependency, retains existing `ToolRegistry` permission enforcement and
capability-specific policy, and provides no raw host topology to the model.
