# Repository mutation fixture plan

## Scope

Implement ADR 0010's deterministic, host-owned repository-mutation fixture in
`rah-tools`. No Git mutation, network capability, public RAH contract change,
or live Codex registration is included.

## Steps

1. Record ADR 0010 and the security-model limits.
2. Add a private repository mutation policy that captures repository/target
   identity, snapshots the bounded fixture root, serializes mutation per root,
   and verifies the post-state.
3. Drive the existing supervised `HostExecutionPolicy` through a fixed native
   fixture executable and exact host-generated argv.
4. Add deterministic fixture modes and integration coverage for success,
   adversarial effects, timeouts, and lease behavior.
5. Run the required workspace validation and diff checks.

## Acceptance criteria

The only model-visible input is `{}`. The only normal mutation changes the
host-mapped `fixture-marker` file from `before\n` to `after\n`; all other
fixture modes are deterministic test support and never model-visible.
