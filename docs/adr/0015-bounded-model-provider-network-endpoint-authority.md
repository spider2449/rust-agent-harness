# ADR 0015 — Bounded model-provider network endpoint authority

Status: Accepted

## Context

RAH Desktop currently configures the Responses-compatible `llama_cpp` provider
with the fixed local base URL `http://127.0.0.1:8080/v1`. Task 118 established
that allowing a human to select a non-loopback service determines where current
user prompts, bounded replayed conversation context, runtime-generated model
input, and other data deliberately supplied through the current RAH request
path may be sent. It is therefore a new host authority boundary, not an
ordinary form-field extension.

The existing certified composition is `codex-cli 0.149.0` started by the
Desktop host and configured through the adapter-local llama.cpp provider seam.
RAH does not own that Codex HTTP stack. Current pinned evidence does not prove
that RAH can force it never to follow redirects, bypass ambient proxies, or
otherwise connect directly only to the configured host. This ADR must not turn
an unproven implementation property into an authority guarantee.

Model requests and provider output remain untrusted. An endpoint must never be
selected or widened by a prompt, model output, `ToolCall`, tool result,
repository content, MCP metadata, Process Plugin metadata, or provider
metadata.

## Decision

RAH authorizes a trusted human through the Desktop Rust host to select one
initial model-provider endpoint for one explicit `llama_cpp` connection. The
authority path is:

```text
trusted human/Desktop host
 -> validated structured ProviderEndpoint
 -> immutable connection configuration snapshot
 -> Codex model-provider configuration
 -> provider transport
```

`llama_cpp` remains a separately fixed provider kind. The endpoint is a closed
object structurally equivalent to:

```text
ProviderEndpoint {
    scheme: http | https,
    host: IPv4 literal | IPv6 literal | DNS hostname,
    port: 1..=65535,
}
```

RAH synthesizes the sole base path as `<scheme>://<host>:<port>/v1`. There is
no configurable path, query, fragment, userinfo, credential, proxy, redirect,
or other URI component. IPv6 is stored as a parsed structural address and is
bracketed only while serializing the final base URL; the structured host field
does not require brackets.

### Validation, normalization, and equality

Rust validates the submitted fields authoritatively. It accepts only the
closed scheme enum and ports 1 through 65535. It rejects an empty host,
whitespace, embedded scheme, userinfo, slash/path syntax, query, fragment,
brackets, an embedded `host:port` form, invalid IP literal, and malformed DNS
hostname.

IPv4 and IPv6 are parsed and stored in canonical address form. Otherwise the
host is an ASCII DNS hostname: at most 253 characters excluding an optional
terminal root dot, labels are 1 through 63 characters, use ASCII letters,
digits, or hyphens, and do not begin or end with a hyphen. A terminal root dot
is removed and ASCII letters are lowercased. A dotted decimal-looking host
that fails IPv4 parsing is rejected rather than reinterpreted as a DNS name.
Internationalized-domain-name conversion is not part of the initial boundary;
callers must provide the already-ASCII DNS representation.

Effective endpoint equality is structural: normalized scheme enum, canonical
IP address or normalized DNS hostname, and numeric port, with the fixed `/v1`
path implicit. This avoids generation churn from equivalent scheme, IP, DNS
case, or terminal-dot spellings without asserting DNS identity equivalence.

### Transport policy and address classification

HTTP and HTTPS are both accepted for literal loopback endpoints. For a
non-loopback endpoint, HTTPS is the normal and recommended transport. HTTP is
permitted only as an explicit host-selected insecure transport choice, so that
already-running LAN llama.cpp deployments without TLS remain usable. A later
Desktop implementation must clearly disclose that non-loopback HTTP has no
transport encryption or authenticated server identity; selecting HTTP is the
host's explicit insecure choice. This ADR sets that semantic requirement and
does not prescribe a particular acknowledgement widget.

IPv4 literals, IPv6 literals, and DNS hostnames are all permitted as
human-selected host identities. Literal IPv4 loopback is `127.0.0.0/8`; literal
IPv6 loopback is `::1`. These literal cases are the closed loopback
classification. `localhost` remains a hostname selection, not literal
loopback, because name resolution is ambiguous.

RFC1918 IPv4, IPv6 link-local, IPv6 ULA, and similar classifications may be
presentation or deterministic-policy metadata. They are not authorization,
peer trust, encryption, stable identity, redirect confinement, or DNS
confinement. No `PermissionLevel::Lan` is introduced and no host discovery is
authorized. DNS resolution is not server identity; direct IP selection is not
proof of server identity.

### Initial-endpoint, redirect, and proxy boundary

For v0.10 this is bounded **initial endpoint selection**, not an enforced
effective-destination policy. A server redirect can lead to another destination
if the Codex-owned stack follows it, and current RAH evidence does not prove a
practical pinned `0.149.0` seam that can prohibit this. Observed or upstream
transport behavior must remain distinct from an RAH-enforced guarantee.

Likewise, RAH does not add, select, clear, or expose proxy configuration in
this task. `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`, system proxy
behavior, and Codex-specific proxy configuration may affect the route where
the Codex composition honors them; RAH cannot presently prevent or fully
observe that routing. The authority bounds the host-selected initial provider
endpoint, not every intermediate or effective network peer.

### Data, credentials, persistence, and lifecycle

Selecting this endpoint is security-relevant because it determines where model
input is intended to be sent. It does not authorize the remote provider to run
host tools: provider output remains untrusted model/runtime output and every
ToolCall continues through the RAH-owned `ToolRegistry`, permission/policy,
sandbox/executor, and `Tool` path.

Credentials are out of scope for v0.10 endpoint implementation. Task 120 must
use `credential_environment_variable = None`. It must not add raw API-token
IPC, JavaScript-visible or persisted tokens, URL userinfo, model-selected
environment variables, arbitrary headers, or secret-store integration. A later
authentication design requires separate credential-authority research.

Task 120 adds no persistence. A future separate Desktop settings schema may
retain only inactive, validated desired preferences. Startup remains
disconnected: a saved endpoint must not auto-connect, contact a provider,
recreate a runtime or `ToolRegistry`, restore repository authority, or imply
server identity or reachability. Conversation transcript persistence remains
unrelated.

Endpoint authority is captured only on explicit connect. Same effective
endpoint means no model-generation change. A changed effective endpoint
increments the existing model generation; a change while `ChatState::Running`
is rejected. An idle connected change updates desired configuration while the
active runtime retains its captured old snapshot and the UI reports reconnect
required. There is no automatic disconnect or reconnect. Explicit reconnect
revalidates current fields, captures a fresh immutable snapshot, and creates a
runtime from it.

This authority connects only to an already-running service. It does not find,
launch, stop, restart, probe, install, download, update, or supervise
`llama-server`, select a GGUF, or set GPU/context/server flags.

### Failure and replay semantics

The endpoint contract preserves distinct concepts: configuration invalid (bad
structured field or impossible serialization), DNS failure, connection failure
(refused, timeout, unreachable), TLS failure, authentication failure, provider
protocol/model failure, and Codex runtime failure. A valid offline endpoint is
not configuration-invalid. Task 120 may present only distinctions the actual
Codex seam reliably observes; otherwise it must return a sanitized broader
runtime/provider failure without mislabeling it as syntax failure.

Timeout, cancellation, disconnect, proxy failure, redirect failure, and other
provider-transport uncertainty grant no retry, rollback, replay, or fallback
authority. RAH must not automatically try another endpoint.

## Authority distinction

This is model-provider transport configuration only. It is not generic HTTP,
socket, browser/fetch, MCP network transport, Git fetch/pull/push, Process
Plugin network, `ToolRegistry` network capability, process-launch, provider or
model installation, credential, arbitrary redirect, or model-selected-
destination authority. It does not change `AgentRuntime`, the optional Codex
adapter boundary, tools, permissions, trusted profiles, repository authority,
conversation persistence, or Desktop model-generation mechanism.

Task 120's IPC is correspondingly closed: JavaScript submits only structured
desired endpoint fields; Rust validates them and alone synthesizes `/v1` for
the Codex provider. Frontend presentation may display normalized host-selected
configuration, but receives no credentials, proxy values, filesystem paths,
server-process details, GGUF path, environment, or hidden transport state.

## Consequences

Task 120 is authorized to implement only the Desktop-private boundary above.
Its deterministic acceptance must cover valid IPv4/IPv6/DNS and ports 1 and
65535; port/host rejection; fixed `/v1`; canonical serialization including
IPv6 brackets; transport policy; host-only/no-credential/no-process/no-network-
tool authority; generation/reconnect state; and adapter delivery of the exact
synthesized endpoint to `CodexLlamaCppProvider` with credentials always `None`.
The existing `127.0.0.1:8080/v1` behavior must remain compatible.

Live acceptance must use already-running servers: Desktop to a non-default
loopback port and Desktop to an explicitly selected second-machine
non-loopback endpoint, each with a deterministic Send/Reply PASS marker. If
the latter is HTTP, evidence must call it explicit insecure transport. Committed
evidence may record only safe endpoint class, scheme, optional non-sensitive
port, certified Codex version, and marker result—never tokens, machine names,
private addresses, filesystem paths, or request content. HTTPS evidence is
included when practical infrastructure exists.

This ADR extends but does not replace ADR 0001 (`AgentRuntime` remains
RAH-owned), ADR 0002 (Codex remains optional), ADR 0003 (tools remain the
extension boundary; this is not a Tool), ADR 0004 (no inference engine), ADRs
0005/0006 (Codex app-server/runtime/tool bridge constraints), ADRs 0007/0008
(no MCP or Process Plugin network widening), ADR 0009 (no generic
Execute/process authority), ADRs 0010/0012/0013/0014 (no repository-authority
widening), and ADR 0011 (Trusted Profile cannot silently invent or configure
this authority). For v0.10, endpoint selection is Desktop-private host state.

## Alternatives rejected

### Free URL text

Rejected because it exposes path, query, userinfo, and opaque URL parsing as
authority without need.

### Model-selected endpoint or generic network Tool

Rejected because model data is not host authorization and model-provider
transport is not a generic network capability.

### Loopback only forever or private-IP-equals-trusted

Rejected because external llama.cpp services are useful, while address class
does not establish a trusted peer, confidentiality, or identity.

### Automatic discovery, fallback, or provider lifecycle

Rejected because discovered/fallback services would redirect data without
current host selection, and process lifecycle is distinct host authority.

### Credentials in this implementation

Deferred because authentication is a separate sensitive authority surface.

## Security non-guarantees

A bounded initial endpoint is not a network sandbox. ADR 0015 does not
guarantee network isolation, direct routing, absence of proxies or redirects,
DNS integrity or pinning, peer trust from an IP range, TLS security or provider
identity on HTTP, provider/model correctness or availability, request rollback
or safe replay, credential secrecy beyond credentials being out of scope, or
process sandboxing.
