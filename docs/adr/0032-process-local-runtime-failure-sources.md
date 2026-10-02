# ADR 0032 — Process-local runtime failure sources

Status: Accepted

Date: 2026-10-02

## Decision

Following Task 497's selected design, `rah-runtime::RuntimeFailure` owns a
sanitized `rah-protocol::RuntimeDiagnostic` and an optional immutable
`Arc<dyn Error + Send + Sync + 'static>`. Adapter conversions retain the original
typed error. Standard `Error::source()` exposes the inner error reference;
provider recovery through source traversal and downcast remains adapter-local.
Generic runtime code never imports provider error types.

`AgentError::Failure` and atomic process-local `RuntimeEvent` retain the envelope.
Clones share the cause without requiring provider errors to implement Clone.
Source-bearing errors have no equality or serde contract. Neutral diagnostic
values retain equality and serde. AgentRuntime signatures and object safety are
unchanged; AgentError's whole-error Eq/PartialEq derives are removed deliberately.

AgentHandle retains a local stream and adds `into_runtime_events()`;
`into_events()` is the explicit compatible protocol projection. Projection
formats only closed diagnostic templates, never the source's Debug or Display.
No serialized source registry or independently correlated source channel exists.
Legacy ordinary AgentEvent producers remain supported. Terminal Tool and permission
codes retain their meaning. rah-session storage is not a runtime source carrier.

## Consequences

Sources remain recoverable while local owners exist, including across connection
fanout and stream consumption. Serialization discards source ownership and cannot
recover provider errors. Concrete adapter connection/shutdown APIs still return
their concrete typed errors; explicit shutdown propagates transport failure.
Drop cleanup remains best effort. No dependency edge, authority, retry, rollback,
runtime version admission, or provider-selection policy changes.
