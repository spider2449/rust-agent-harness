# ADR 0033 ? Neutral runtime composition and host Tool lifetime

Status: Accepted

Date: 2026-10-03

## Context

ADRs 0001, 0002 and 0005 separate RAH runtime contracts from provider process
and protocol details. ADR 0006 mediates model Tool requests through RAH policy.
Production adoption needs an explicit lifetime boundary between runtime handles
and the host composition that authorizes Tool requests.

## Decision

The trusted host configures an adapter at its composition root, then owns neutral
runtime, conversation and turn handles. Adapter-specific artifact admission,
transport, provider identifiers and cancellation translation remain adapter-owned.
The host selects repository context; a runtime cannot select or switch repositories.
RAH conversation identity, turn SessionId and private provider identity are distinct.

The host supplies a conversation-scoped, revocable Tool request port. It owns the
registry, permissions, active-turn admission and effect accounting. A model or
adapter request is never authorization. Every executable request uses existing
ToolRegistry and authorized dispatch policy. Distinct Tool requests may coexist
inside one host-admitted turn; this does not admit another model turn.

Teardown policy remains host-owned. Before withdrawing executable composition
and awaiting adapter teardown, the host revokes Tool admission. Retained runtime
or conversation handles cannot restore that authority. Already admitted effects
remain accounted for by the host. Cancellation, teardown or response loss never
imply rollback or authorize replay. History and descriptive repository state are
separate from executable runtime composition.

Runtime failures retain a sanitized diagnostic and process-local typed source
under ADR 0032. Only the diagnostic crosses serialization boundaries.

## Consequences

This refines the ownership boundaries of ADRs 0001, 0005 and 0006 without enabling
provider-owned capabilities or expanding host authority. Runtime contracts remain
experimental; production use does not declare API stability. Provider registration,
provider selection UI, alternate adapters and inference engines are outside this
decision. Admission remains adapter-specific: ADR 0030's exact-version policy
is superseded in design by [ADR 0034](0034-artifact-bound-codex-compatibility-admission.md).
Replacement implementation is pending Task 510C2; conversation/authority
ownership and current production admission remain unchanged.
