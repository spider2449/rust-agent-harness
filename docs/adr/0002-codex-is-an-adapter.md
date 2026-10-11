# ADR 0002 — Codex is an adapter, not the architecture

Status: Superseded by [ADR 0036](0036-plugin-kernel-everything-is-a-plugin.md) on master (2026-10-11); authoritative only for the `legacy/authority-first` line. Original status: Accepted

## Context

Codex contains useful Rust agent-runtime functionality, but RAH must remain independently defined.

## Decision

Only `rah-runtime-codex` may directly depend on Codex crates.

No other RAH crate may import Codex types.

If Codex requires a different internal representation, the adapter translates it.

## Consequences

RAH may selectively reuse, vendor, upgrade, replace, or remove Codex without redefining RAH's public protocol.
