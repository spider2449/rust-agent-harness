# ADR 0001 — Runtime abstraction is RAH-owned

Status: Superseded by [ADR 0036](0036-plugin-kernel-everything-is-a-plugin.md) on master (2026-10-11); authoritative only for the `legacy/authority-first` line. Original status: Accepted

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
