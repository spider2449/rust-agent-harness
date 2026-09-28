# ADR 0030 — Moving Codex runtime certification

Status: Accepted

Date: 2026-09-27

## Context

ADR 0005 requires a version-validated Codex app-server process boundary. A
permanent single version pin conflates historical release evidence with the
compatibility policy of current source. Task 438 established a direct model
control on exact Codex 0.157.1 after the 0.149.0 direct control failed under
the same account and selected model.

## Decision

1. Codex remains behind the private, version-validated app-server process
   boundary. ADR 0005 otherwise remains in force.
2. Historical Release Certification records the exact Codex version that
   certified a released RAH version. It is immutable. RAH v0.32.0 records
   0.149.0 and later policy does not rewrite that evidence.
3. Current Runtime Certification is an explicit set of exact CLI versions
   certified for current source. It may change only through a certification
   task. It is never a semver range. Unknown versions fail closed.
4. Preferred Current Baseline is a deterministic exact member of that set.
   Its selection never follows store contents, latest release, highest semver,
   PATH order, model input, or provider input. For Task 438 it is 0.157.1.
5. Removing a version from current support does not erase its historical
   certification. The 0.149.0 artifact may remain stored as legacy certified
   evidence while current source rejects it.
6. Adding a current version requires an app-server schema audit, deterministic
   regression validation, and live direct and Desktop certification. The
   baseline manifest and hashes are checked separately from runtime admission.
7. The trusted host owns executable selection. Adapter validation applies to
   the preferred baseline, explicit host override, and PATH fallback alike.
   Version admission grants no Tool, model, provider, repository, or other
   authority. Existing RAH policy and ToolRegistry remain authoritative.

## Consequences

The adapter owns one exact current admission authority and validates the
installed schema at startup. Historical release documents retain their exact
versions. Baseline storage can contain multiple exact artifacts without
expanding runtime compatibility.
