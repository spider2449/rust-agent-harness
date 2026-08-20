# ADR 0004 — RAH does not implement model inference

Status: Accepted

## Context

Implementing inference would couple RAH to model execution, hardware runtimes, and model formats.

## Decision

RAH orchestrates external or pluggable model backends.

RAH does not implement transformer inference, weight loading, GPU kernels, KV cache, or tokenizer internals.

## Consequences

Model execution can be provided by remote APIs or local inference servers while RAH remains focused on agent orchestration.
