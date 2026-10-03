# Task 507 — Native OpenAI Responses adapter core

Starting HEAD: `0a24b7750f229b9e0f1adee9602aba29ed7b0541`.
Starting worktree clean; version 0.33.0. No Desktop integration, live/paid
inference, default-provider change, tag, release or version bump.

## Official contract and bounded implementation

Reviewed on 2026-10-03: official [streaming guide](https://developers.openai.com/api/docs/guides/streaming-responses)
and [function-calling guide](https://developers.openai.com/api/docs/guides/function-calling).
HTTP POST Responses uses stream=true, SSE semantic lifecycle events, function
output items and function_call_output correlated by call_id. No Assistants API
or SDK. Only model, input, function tools, stream and store=false are sent.
System instructions use neutral System messages. Hosted tools are unsupported.
The current [reasoning guide](https://developers.openai.com/api/docs/guides/reasoning)
states that store=false returns encrypted reasoning by default; the legacy
include field is not required. Completed reasoning items remain private and
are replayed unchanged within the Tool loop. No extra endpoint/options are added.

New isolated `rah-runtime-openai` depends inward on rah-runtime/rah-protocol;
rah-tools is dev-only for real host-port conformance. Reuse async-stream,
async-trait, futures, serde_json, thiserror, Tokio and tokio-util. Reuse Desktop's
reqwest 0.13.4 default-features=false rustls/stream configuration. Manual JSON
encoding avoids an additional feature. No new upstream dependency versions or
lock packages: only the new workspace package. Rustls currently uses aws-lc
(existing native C build dependency) and platform certificate verification;
this task does not switch TLS backend or install a system dependency. Windows
and natural Linux CI are required validation, not assumptions of compatibility.

Fixed production endpoint https://api.openai.com/v1/responses; no public endpoint
setter. A cfg(test)-only local endpoint seam exercises the real reqwest client,
HTTP status path, bytes_stream and parser. Redirects, retries and proxies are
disabled. Connect/read/request deadlines are 30/60/300 seconds respectively.
No unrestricted production endpoint or provider SDK semantics enter neutral APIs.

Credential: backend-only factory with private key, redacted Debug, no serde/key
accessor. It never enters input/tools, host port or diagnostics. Transport errors
discard raw reqwest data; errors do not retain response headers/body/SSE text.

Capabilities: streaming, Tool calls, local turn cancellation and text replay
true; discovery and native continuation false. Explicit model is required.
No remote rollback/cancel-server guarantee or Codex-style admission/preflight.

Conversation **O1**: each TextReplay consumes the complete host-owned neutral
snapshot; no previous_response_id or OpenAI Conversation object. Within a turn,
retain bounded completed output items (including private reasoning items) and
matching function outputs for follow-up requests. Later top-level turns use the
new host snapshot, not automatic replay of uncertain prior effects. A standalone
neutral Tool-role message lacks call identity and is rejected, not reinterpreted.
RAH ConversationId, per-turn host SessionId and provider IDs remain separate.

SSE byte parser supports arbitrary chunk boundaries, split UTF-8, CR/LF/CRLF,
comments, multi-line data and multiple frames. Frames terminate on blank lines.
Limits: 256 KiB/frame; 8 MiB streamed bytes/response; 4 MiB text/history; 128
function calls/response; 32,768 emitted events/turn. Unexpected/malformed/error/
failed/incomplete events fail closed. Known inert text/content/reasoning lifecycle
events are recognized; hosted/custom tool events are rejected. Completion is
accepted only after complete function items match the completed response output
and accumulated text matches completed message text. Exactly one neutral success
or failure terminal is published. No partially assembled function dispatch.

Arguments accumulate by output_index and are checked against item_id, arguments
done, item done and completed output. Duplicate provider call/item identity is
rejected within each response. One serialized live host request at a time binds
the host-emitted neutral ToolCallId privately to its original call_id; the result
is serialized as a string function_call_output for that call_id. Host live events
are forwarded from the owned lease; no synthetic execution facts are invented.
Multiple calls are supported serially with distinct neutral identities. Registry,
repository, policy, permissions and HostExplicit configuration are inaccessible.

Tool names, descriptions and parameter JSON are preserved. Conservatively
recognized strict-compatible schemas emit strict=true. Other object schemas emit
explicit strict=false without normalization, preserving optional fields and
additional-property semantics. Non-object schemas, empty/oversized/duplicate
names fail deterministically before HTTP. Host validators remain authoritative.

Maximum **eight Tool continuation rounds** (up to nine HTTP requests including
the final response). Ninth response requesting a Tool fails with typed
ContinuationLimit before dispatching that round. No automatic network retry.

Owned worker tasks multiplex provider output, live host events and cancellation.
Turn stream Drop aborts its worker; runtime owns task joins/aborts, never a child
process. Local cancellation drops active HTTP/host response futures; admitted
effects remain owned/accounted for by the host. Shutdown cancels and joins tasks
and rejects new operations; close withdraws the conversation. Retained handles
cannot renew a revoked host scope. No replay/rollback claim.

OpenAiAdapterError categories: configuration, transport, HTTP status, API,
SSE, event JSON, protocol, Tool schema, continuation limit, cancellation and
shutdown. HTTP status retains only u16 (401/403, 429, 5xx and other status are
distinguishable), not entitlement/retirement guesses. ADR 0032 RuntimeFailure
retains the typed source, with sanitized closed RuntimeDiagnostic categories.
Secret-sentinel projections cover diagnostic serde, UI Display and Debug/source
snapshots; no unrestricted remote data is formatted.

## Deterministic proof and validation

Local scripted TCP HTTP server verifies request path, fake Authorization and
JSON; returns chunked SSE in three-byte chunks. Scripts cover text, UTF-8,
incremental arguments, one/two host Tool calls and continuation, status errors,
API error, malformed framing/JSON, interrupted and held-open streams, cancel,
shutdown and loop bound. Real HostToolScope/counter tests cover authority and
revocation, host-assigned call IDs and results. Parser tests cover every split
point, multiple frames, line endings, multi-line data and limits.

Implementation checks: the first compile caught a `?` expression inside a JSON
macro in the try-stream body; it was moved to a separate local expression. A
development Clippy pass caught test mutex guards with explicit Drop but lexical
scope still spanning await; those assertions were scoped before await. These
were implementation checks before freezing source, not failed conformance runs.
Final focused development Clippy passed with warnings denied.

Stable-source validation, all native exits **0**:

| Gate | Result |
| --- | --- |
| cargo test -p rah-runtime -p rah-runtime-openai | Neutral 18/0/0; OpenAI 14/0/0; doctests 0/0/0 |
| cargo fmt --check | PASS |
| cargo check --workspace | PASS |
| cargo test --workspace | 1,061 passed / 0 failed / 24 ignored, including Desktop 334/0/20 and OpenAI 14/0/0 |
| Codex suites within workspace | Unit 102/0/1; integration 6/0/0 and 11/0/0; total 119/0/1 |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | PASS |
| Canonical windows-desktop-test-gate.ps1 | PASS; helpers exit 0; Desktop 334/0/20; watchdog false |
| Disabled Windows Desktop check | PASS; existing no-provider dead-code/unused warnings retained, no unrelated cleanup |
| Disabled normal/build/dev dependency graph | Zero rah-runtime-codex and zero rah-runtime-openai edges |
| Tauri permission inventory | 47 runtime / manifest / generated / default allows / frontend commands; unchanged |
| Cargo metadata | 14 members, new adapter 0.33.0/edition 2024, default provider-codex, no Desktop OpenAI dependency |
| git diff --check | PASS before publication |

Focused evidence: `F:/temp/task507-evidence/focused.log`. Full serial gates used
`CARGO_TARGET_DIR=RAH_TEST_TARGET_DIR=F:/temp/rah-task504-target`, with canonical
Desktop PrepareOnly first. Workspace check/test/Clippy and fmt logs are under
`F:/temp/task507-evidence/`. Canonical Desktop status/logs:
`F:/temp/task507-evidence/desktop-gate/20261003-192327-663-77673d3046e94589a450ff28fa020783/`;
elapsed 297.481 seconds, no watchdog timeout. Disabled check used
`F:/temp/rah-task506-disabled-target` with explicit Windows target and
--no-default-features; its log and normal/build/dev tree are retained under the
same evidence root. Source was unchanged throughout these serial gates.

All required core proofs passed: ordered text/completion once, real HTTP/SSE,
function assembly, one/two Tool calls, exact call_id/result pairing, distinct
host IDs, typed/downcastable failures, redacted projections, no retries, bounded
continuation, cancellation, worker Drop, close/shutdown and retained scope
revocation including after admission but before dispatch. No paid/live inference.

## Isolation, ADR and handoff

Desktop source/dependency/features/UI, Codex adapter, Tauri inventories and
HostExplicit remain unchanged. Default provider remains Codex, provider-codex
remains default. The disabled Desktop graph must still exclude Codex/OpenAI.
ADR-B: ADRs 0032/0033 and existing runtime/authority decisions suffice; no new ADR
or neutral contract change. v0.34 capability remains NONE SELECTED.

**A — NATIVE OPENAI ADAPTER CORE CONFORMS TO NEUTRAL RUNTIME**

HostExplicit is exactly 11, proven by unchanged host_kind inventory and the
passing executable Desktop allowlist tests. Authority delta NONE; no new ADR,
permission or Tool contract. All required deterministic/full regression gates
passed. Starting GitHub master was rechecked and still matched the checkpoint.
One coherent commit of only Cargo workspace/lock, adapter code/tests and this
report is authorized, followed by normal GitHub master push and natural
exact-head CI. Commit and final CI identity are reported in the completion
return; this file cannot embed its own future commit SHA. No mirror, tag/release.

Task 508 scope: production OpenAI feature/factory
selection through the existing neutral Desktop path, explicit backend model/key
configuration, Codex-free Windows build/operational proof, preserved default
Codex and HostExplicit 11. Paid live production proof requires separate explicit
authorization; Task 507 does not start Task 508 or implement credential UX.
