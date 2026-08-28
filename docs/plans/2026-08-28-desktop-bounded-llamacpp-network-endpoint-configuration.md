# Task 120 — Desktop bounded llama.cpp network endpoint configuration

## Starting point and boundary

Task 120 starts from Task 119's accepted ADR 0015 commit
`1955ea795e2d08d3088633a63f22fc9dc88bc795`. It implements only the
Desktop-private, trusted-human selection of one initial llama.cpp endpoint for
an explicit connection. It adds neither a network Tool nor provider lifecycle,
credentials, persistence, discovery, automatic connection, or transport-policy
controls.

## Implementation

`rah-desktop` owns a private `ProviderEndpoint`: closed `http|https`, parsed
`IpAddr` or normalized ASCII DNS hostname, and nonzero `u16` port. It rejects
whitespace, URL syntax, brackets, embedded ports, invalid DNS, Unicode, and
malformed dotted-decimal input. Rust alone synthesizes the fixed
`<scheme>://<host>:<port>/v1` base URL, including IPv6 brackets, and constructs
`CodexLlamaCppProvider::new(base_url, None)`.

IPC accepts only structured llama.cpp endpoint fields. Non-llama providers
reject endpoint input, and llama.cpp requires it. Presentation returns only
normalized endpoint facts and the non-loopback HTTP disclosure state. No
credential, path, proxy, redirect, process, or persistence fact is exposed.

Equivalent normalized endpoints do not change model generation. A changed
endpoint increments the existing model generation; changes during a running
chat are rejected. An idle connected change is presented as reconnect required
and the active runtime retains its already-captured connection configuration.
Applying configuration only validates and updates desired state; it does not
create a runtime or contact a provider.

## Task 120 extension: explicit readiness and bounded cancel recovery

ADR 0015 permits exactly one additional Desktop-private operation: a human
initiated `GET <normalized-endpoint>/health` for the current llama.cpp desired
selection. The `rah-desktop`-only reqwest 0.13.4 client is configured with
redirects, proxies, and retry disabled; it uses fixed 2 second connect, 5
second total, and 4 KiB streamed-body bounds. Results are diagnostic-only and
generation-scoped (`not_tested`, `checking`, `ready`, `loading`,
`unreachable`, `tls_failure`, or `check_failed`), with stale results discarded.

Desktop also bounds the known unbounded adapter `cancel` wait: 2 seconds for
graceful cancellation and 2 seconds for Desktop-owned Codex runtime shutdown.
Terminal ownership is generation, session, and runtime-identity scoped. Hard
recovery stops waiting and tears down only RAH's Codex app-server transport;
it makes no provider rollback, llama.cpp process, replay, fallback, or
automatic reconnect claim.

## Validation and live acceptance

Focused deterministic tests cover structural endpoint validation,
normalization, fixed URL synthesis, loopback classification, provider closure,
and generation behavior. Frontend static validation uses `node --check`.

### Validated local evidence

- Structured llama.cpp endpoint configuration: PASS.
- Local readiness probe against a healthy endpoint: PASS.
- Unreachable-endpoint readiness classification: PASS.
- A valid-but-unreachable endpoint remains valid configuration: PASS.
- Cancel Turn operator recovery: PASS.
- `cancel_chat` Tauri capability repair: PASS.
- Stale-result, generation, and cancellation deterministic coverage: PASS.
- Non-loopback HTTP disclosure presentation: PASS.

### Genuine second-machine evidence and current status

RAH reached a genuine remote llama.cpp server and displayed the applicable
non-loopback HTTP disclosure: PASS. The remote provider then returned an HTTP
500 from its llama.cpp Jinja chat template because the system message was not
at the beginning; it also emitted Responses compatibility warnings. This
proves non-loopback routing and disclosure behavior, but not a successful
Codex-compatible remote generation.

`RAH_TASK120_NETWORK_OK = NOT VALIDATED / DEFERRED`.

Task 120 is **IMPLEMENTATION COMPLETE WITH DEFERRED EXTERNAL-HARDWARE
VALIDATION**. It is not full second-machine live validation complete.

### Deferred validation: genuine second-machine llama.cpp

Reason for deferral: no suitable second-machine Codex-compatible llama.cpp
deployment/hardware is currently available. This is an environment/hardware
limitation, not an implementation PASS.

Required future proof:

1. Use an already-running llama.cpp service on a genuinely separate machine.
2. Use a Codex-compatible server, model, and chat-template combination.
3. In Desktop, select **Test Endpoint** and verify `Ready`.
4. Verify the non-loopback HTTP disclosure when HTTP is selected.
5. Explicitly connect Codex.
6. Send `Reply exactly: RAH_TASK120_NETWORK_OK`.
7. Verify the exact marker is returned.
8. Commit no private endpoint identity.

No private network identity, model path, token, or filesystem path belongs in
this evidence.

## Residual transport boundary

This is bounded initial endpoint selection only. RAH does not claim to prevent
Codex-owned redirects, proxies, DNS changes, or other effective-destination
routing behavior.
