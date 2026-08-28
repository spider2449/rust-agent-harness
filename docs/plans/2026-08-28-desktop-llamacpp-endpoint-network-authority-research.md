# Task 118 — Desktop llama.cpp endpoint and network authority research

## Starting point and conclusion

Research began from clean `master` at
`064cadad9cd9d4f99ec18f3080111f8a01bd3ec5` (Task 117 complete; exact-head
GitHub Actions CI #107 / `33127311475` successful). This task changes no Rust,
frontend, dependency, or ADR.

Desktop maps `DesktopModelProvider::LlamaCpp` to
`CodexLlamaCppProvider::default_local()`, fixed at
`http://127.0.0.1:8080/v1`. The adapter also has the broader host-side
constructor `CodexLlamaCppProvider::new(base_url, credential_environment_variable)`.
Its simple HTTP(S) validation is an adapter input check, not a Desktop network
authority decision and must not become JavaScript's argument surface.

**Recommendation:** RAH should ultimately support a human/host-selected
llama.cpp service on the local machine, a LAN, or an explicitly configured
remote host. It should use a **structured, bounded endpoint configuration**,
never an arbitrary URL. Non-loopback selection allows model input to leave the
Desktop machine, so it is a new bounded model-provider outbound-network
authority. Write ADR 0015 before implementing it; the next task is the ADR,
not endpoint code.

## Current facts and provider contract

- Desktop model selection is host-owned. A different effective selection
  increments `model_generation`; a same selection does not; mutation during
  chat is rejected; a connected runtime then reports reconnect required. These
  mechanics are sufficient; do not create an endpoint generation.
- Desktop conversation persistence deliberately excludes endpoint and
  credential values. An endpoint preference is not a conversation record.
- Certified llama.cpp compatibility is for the Responses subset through `/v1`.
  Current llama.cpp server documentation exposes `POST /v1/responses` and uses
  base URLs ending in `/v1`. The bounded contract should synthesize exactly
  `/v1`, with no path/query/fragment/userinfo field. This does not claim every
  llama.cpp build/model/template is compatible.
- llama.cpp can listen beyond loopback; upstream examples use `--host 0.0.0.0`.
  Network deployment is therefore a real product use.
  <https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md>

## Separate authorities

| Boundary | Meaning | Decision |
| --- | --- | --- |
| Model authority | What untrusted model output can request or cause | None. An endpoint never comes from output, ToolCall input, prompt, repository, provider/MCP/process-plugin metadata, or tool results. |
| Host configuration | Validated desired provider configuration selected by a human through Desktop | The only selector; configuration is not a capability or reachability proof. |
| Active network authority | Where a connected provider may send model-provider traffic | Loopback is the narrow case; non-loopback requires ADR 0015. |
| Durable configuration | An inactive remembered preference | Defer. It is never active, trusted, reachable, identity-verified, or auto-restored authority. |
| Credentials | Authentication material/selection mechanism | Defer. No raw token IPC, JavaScript exposure, conversation persistence, or model-selected environment variable. |
| Provider/process lifecycle | Starting/selecting/supervising `llama-server` or GGUF | Out of scope. RAH connects to an already-running service only. |

Remote configuration changes the data boundary: the provider may receive user
prompts, replayed context, runtime-generated model input, and deliberately
included request data. It is not cosmetic. It does not grant ToolRegistry,
repository mutation, process execution, trusted-profile, MCP, Git, browser, or
generic network-tool authority. Tool calls retain the ordinary parsed ToolCall
-> ToolRegistry -> permission/policy -> sandbox/executor -> Tool path.

## Recommended representation

Reject a port-only control because it cannot express LAN/remote deployments.
Reject a free URL field because it makes path, query, userinfo, and opaque URL
parsing an unchecked part of Desktop authority. ADR 0015 should define an
object equivalent to:

```text
provider = llama_cpp                 (fixed)
scheme   = http | https              (closed enum)
host     = IP literal | DNS hostname (validated host-selected value)
port     = 1..65535                  (bounded TCP port)
path     = /v1                       (fixed; synthesized by RAH)
```

No path override, query, fragment, userinfo, embedded credential, discovery,
or proxy field exists. IPv6 needs an unambiguous structured literal and
bracketed serialization only when creating the URL. DNS is operationally useful
but adds resolution and rebinding considerations; direct IP does not establish
trust. “LAN” is not a stable security class: RFC1918/IPv6-private checks can be
useful signals but do not establish identity, prevent malicious peers/rebinding,
cover enterprise topology, or constrain redirects/proxies.

## Decision matrix

High is favorable for product value/testability and unfavorable for
risk/complexity/ambiguity/impact.

| Candidate | Product value | Security risk | Complexity | Deterministic testing | Windows live testing | Portability | Credential dependency | Redirect/proxy ambiguity | Persistence impact | ADR | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Loopback only | Medium | Low | Low | High | High | High | None | Low, transport still adapter-owned | Low | No | Preserve as narrow case; insufficient alone. |
| Loopback + LAN | High | Medium/High | Medium | Medium | Medium | High | None initially | Medium/High | Medium | Yes | Valuable, but address class is not a guarantee. |
| Arbitrary host with bounded scheme/port/fixed path | High | Medium/High | Medium | High validation; medium transport | Medium | High | None initially | High pending proof/control | Medium | Yes | **Recommended ADR target.** |
| Full URL | High | High | Low initially/high to secure | Low | Medium | High | Optional | High | High | Yes | Reject; adapter breadth is not Desktop policy. |
| Remote HTTPS with credentials | High for managed use | High | High | Medium | Medium | High | Required | High | High | Yes | Defer credential design. |

The recommended row means arbitrary *human-selected host within a closed
object*, not arbitrary URL or generic network authority.

## Transport analysis

Loopback HTTP remains practical and permitted by its narrow local boundary.
Non-loopback HTTP sends prompts/replay context in plaintext and has no server
identity. HTTPS normally supplies confidentiality and certificate-bound server
authentication, while adding DNS, certificate, and reverse-proxy deployment
work. HTTPS should be the normal non-loopback recommendation and required for
an Internet-routed service. The ADR must explicitly decide whether a deliberate
non-loopback LAN HTTP exception is allowed for common llama.cpp deployments;
if allowed, it needs an explicit plaintext-disclosure acknowledgement and must
not call private addressing secure.

RAH starts Codex and supplies provider configuration; it does not own Codex's
HTTP client or expose redirect/proxy controls. Current RAH evidence does not
prove redirect policy for the certified Codex 0.149.0 transport. Upstream Codex
reports describe reqwest redirect following and proxy/environment behavior, but
they are not a version-pinned guarantee for this integration. Therefore no
current claim may say the configured host is the only destination. A redirect
from A to B, or ambient `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`,
system proxy, or equivalent Codex setting can alter effective traffic. Process
supervision is not network isolation.

ADR 0015 must either obtain evidence plus an enforceable Codex transport policy
(redirect/proxy treatment) before claiming endpoint confinement, or explicitly
adopt bounded *initial endpoint selection* while documenting that direct-
destination confinement is unproven. Task 120 must test the actual certified
Codex composition seam, not a hand-built HTTP client. Task 118 changes no HTTP
behavior.

## Lifecycle, persistence, and failures

Endpoint fields are part of effective model configuration: effective endpoint
change increments the existing model generation; equality does not. A connected
idle change requires explicit reconnect, never auto-reconnect. A chat-running
change is rejected with existing busy semantics. The runtime uses its captured
configuration snapshot.

Do not add persistence in the first endpoint implementation. A later task may
store only validated inactive desired values in a separate versioned Desktop
host-preferences schema; startup remains disconnected and must not reactivate
network authority. Never use transcript persistence.

| Class | Meaning |
| --- | --- |
| Configuration invalid | Fails syntax/adopted bounded policy before connection. |
| DNS resolution failure | Valid hostname cannot resolve. |
| Connection refused/timeout | Valid endpoint did not accept/complete transport. |
| TLS failure | Certificate, hostname, protocol, or handshake verification fails. |
| Authentication failure | Provider responds but requires/rejects authentication. |
| Provider protocol/model failure | Transport works but Responses compatibility/model/server response fails. |
| Codex runtime failure | Runtime/process/adapter failure outside a provider response. |

A valid offline endpoint is not invalid configuration. Sanitized presentation
must not disclose secrets or unnecessarily persist/log private host facts.

## Later validation plan

Deterministic tests: enum/host/IP/DNS/IPv6/port validation; fixed `/v1`
serialization; rejection of userinfo/query/fragment/path override; host-only
selection; equal-versus-changed generation; busy rejection; reconnect required;
no auto-reconnect; no credential IPC/persistence; and distinct errors above.

After ADR and implementation, prove both classes against already-running
servers: (1) Desktop -> `127.0.0.1:<non-default-port>` -> Send/Reply PASS;
(2) Desktop -> explicitly configured second-machine endpoint -> Send/Reply
PASS. RAH must not launch/select `llama-server.exe`, choose GGUF, or install
anything. Record only endpoint class/scheme/safe port fact, success marker,
certified Codex version, and failure class—never private IP, machine name,
token, filesystem path, or request content.

## ADR decision and next task

Non-loopback support is a new bounded host-selected outbound model-provider
network authority. Loopback-only remains the no-new-authority case. The exact
next task is **Task 119 — ADR 0015: Bounded Model Provider Network Endpoint
Authority**. It must decide selector/representation/schemes/IP-DNS-port/fixed
path, HTTP/HTTPS, redirects/proxies, disclosure, credential deferral, inactive
persistence, reconnect, failures, validation, and explicit non-authorities. It
must not authorize arbitrary network tools, MCP transport, Git network,
browser/fetch, model-selected URLs, credential IPC, or process launch. Only
then may Task 120 implement the bounded Desktop endpoint configuration.
