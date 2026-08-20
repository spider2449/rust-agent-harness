# ADR 0001 — Runtime abstraction is RAH-owned

Status: Accepted

## Context

RAH needs an agent runtime while preserving the ability to use Codex now and another runtime later.

## Decision

RAH owns the `AgentRuntime` abstraction.

Codex is integrated through `rah-runtime-codex`.

RAH public APIs must not expose Codex runtime types.

## Consequences

- Codex can accelerate early implementation.
- upstream Codex changes remain adapter-local.
- a future NativeRuntime can implement the same RAH contract.
- adapter translation code is an intentional cost.
