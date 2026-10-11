# ADR 0003 — Tools are the initial extension boundary

Status: Superseded by [ADR 0036](0036-plugin-kernel-everything-is-a-plugin.md) on master (2026-10-11); authoritative only for the `legacy/authority-first` line. Original status: Accepted

## Context

RAH needs built-in tools, MCP tools, and process plugins without creating incompatible extension models.

## Decision

All executable external capabilities converge into the RAH-owned `Tool` abstraction and `ToolRegistry`.

The initial plugin model is tool-oriented.

Plugins do not directly own AgentContext, Session, AgentRuntime, ModelBackend, policy, or sandbox internals.

## Consequences

Built-in tools, MCP tools, and process-plugin tools are interchangeable from the AgentRuntime perspective.

A full general-purpose plugin platform is deferred.
