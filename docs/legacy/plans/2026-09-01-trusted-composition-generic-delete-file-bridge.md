# Task 161: Trusted composition and Generic Codex Tool Bridge for `repo.delete-file`

## Scope

Integrate the Task 160 host-constructed deletion authority into the existing
Trusted Profile composition and provider-neutral Generic Codex Tool Bridge.
Keep deletion authority separate from profile configuration, Execute
permission, other repository policies, model/provider metadata, and Desktop or
live Codex workflows.

## Gates

- Preserve the exact Task 160 starting head and clean worktree.
- Read ADR 0011, ADR 0017, and the Task 158-160 evidence before editing.
- Keep the public tool name and narrow schema unchanged.
- Prove authorized composition, closed failure cases, stale preimage and
  repository-generation checks, unchanged index, and no replay through the
  deterministic bridge fixture.
- Run focused crate tests and all requested workspace gates.
- Commit one coherent change, push it, and require CI success for the exact
  pushed head before Desktop integration.

## Implementation boundaries

The host supplies an opaque, already-constructed deletion authority to the
existing effective composer. A profile may describe only a closed symbolic
selection; it cannot construct, widen, or revive the authority. The composer
registers the resulting RAH `Tool` in the existing `ToolRegistry`. The Codex
adapter remains generic and uses its existing private alias routing.

## Exclusions

No Desktop UI/workflow, live Codex validation, generic filesystem or Git
authority, rename/move, recursive deletion, auto-stage, release/version, or ADR
changes.
