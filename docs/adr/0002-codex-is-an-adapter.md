# ADR 0002 — Codex is an adapter, not the architecture

Status: Accepted

## Context

Codex contains useful Rust agent-runtime functionality, but RAH must remain independently defined.

## Decision

Only `rah-runtime-codex` may directly depend on Codex crates.

No other RAH crate may import Codex types.

If Codex requires a different internal representation, the adapter translates it.

## Consequences

RAH may selectively reuse, vendor, upgrade, replace, or remove Codex without redefining RAH's public protocol.
