# Task 513 — native provider Connect restoration

Starting checkpoint: d955f735b74fb38f92e920cce2a0a472676c4178. Clean local HEAD,
origin/master and live GitHub master verified equal before creating the task worktree.

## Original Connect defect

The Connect button uses model-source eligibility before invoking connect_codex.
Task 511 captures RAH_OPENAI_MODEL at startup and native model source requires it.
The frontend disables model/provider configuration unless the runtime is Codex.
Consequently the existing picker cannot configure native OpenAI, and without the
environment model Connect is disabled before IPC. Missing credentials are collapsed
into runtime_connection_failed. The native factory itself already exists. llama.cpp
preferences belong to the legacy Codex route; there is no native llama.cpp factory.

## Implementation and gates

Restore native configuration and typed errors, retain fixed official OpenAI origin,
reuse private Responses transport with a separate loopback-only llama.cpp factory
and provider-local optional key. Retain neutral runtime, conversation, stream/control
and revocable host Tool port. HostExplicit remains 11; no version/release/Codex work.

Run OpenAI adapter, llama.cpp mock, Desktop composition tests before live local chat
and credential-conditional OpenAI chat. Then cancel, reconnect and Tool controls;
neutral/Desktop/workspace/fmt/check/clippy/diff gates. Publish only after required
acceptance passes. No server/model download is authorized.

Initial environment observation: no OPENAI_API_KEY, RAH_OPENAI_MODEL or
RAH_RUNTIME_PROVIDER in the process environment; no llama-server process found.
Live acceptance remains separately required and cannot be inferred from mock tests.

## Implementation evidence

Native provider/model configuration now has its own host-owned disconnected
boundary and UI controls. Legacy model preferences remain separate. Normal Desktop
build includes provider-openai and provider-llamacpp; official OpenAI origin stays
fixed. LlamaCppFactory shares only private Responses/runtime machinery, restricts
the endpoint to loopback, bounds readiness to ten seconds and never reads
OPENAI_API_KEY. Optional server authentication uses RAH_LLAMA_CPP_API_KEY.

OpenAI adapter: 14 existing tests PASS. llama.cpp adapter: six additional tests
PASS, including health/loading/unreachable/configuration, model resolution,
streaming/malformed events, cancellation/recovery, reconnect and host Tool result
correlation. One new cancellation assertion initially expected AgentEvent::Cancelled;
the existing adapter contract instead emits a terminal typed Failed/Cancelled source.
The corrected assertion tests that contract and a succeeding next turn; no runtime
cancellation semantics changed. Total native adapter suite: 20/0.

Desktop production OpenAI fixture: Connect/text/Tool/Disconnect/Reconnect/Cancel
PASS. Desktop production llama.cpp mock: actual Connect dispatch, sole-model
resolution, first streamed text, Disconnect/Reconnect and connected provider-switch
rejection PASS. All eight frontend suites PASS (including new native configuration).
Neutral runtime suite: 18/0.

An already-running user llama-server appeared during implementation:
F:\llama_cpp\llama-b10621-bin-win-cuda-13.3-x64\llama-server.exe,
endpoint http://127.0.0.1:8080. Health: 200 {"status":"ok"}. Exactly one model:
F:\llama_cpp_models\ornith-1.5-35b-Q4_K_M.gguf. No server started or model downloaded
by this task. Responses worked directly; no Chat Completions fallback.

Real Desktop composition acceptance: one test PASS, five streamed deltas,
RAH_LLAMA_OK received, completion and Disconnect PASS, no failure. Provider-codex
was not compiled. Before/after codex and codex-code-mode-host identity delta: zero.
Evidence: target/task513/live-llama-measured.log and live-codex-*-measured.txt.
An initial command omitted the tests:: prefix under --exact and selected zero
tests; it is not acceptance. Its original log is preserved as live-llama.log.

OpenAI credential checked only for presence at process/user/machine scopes:
all absent. OpenAI live: NOT_RUN_NO_CREDENTIAL. No credential value printed.
OpenAI native deterministic composition is separately PASS.

## Live Tool correction and final native acceptance

The first local live host-Tool attempt failed before any ToolStarted/ToolFinished.
Captured provider-only wire diagnosis proved llama-server omits output_index and
the distinct function_call_arguments.done event. A llama.cpp-only normalizer was
added; a subsequent failure exposed that reasoning output occupies an index before
the function. Offline replay identified response.completed as the failing event.
The final normalizer counts every added output item, resolves argument events only
by previously observed item ID, and synthesizes argument-done from the final item.
The existing parser still checks accumulated arguments, call/name/ID equality,
output order, duplicate completion and final output; official OpenAI normalization
is unchanged. Negative correlation tests and a reasoning-before-function mock
round-trip protect these checks. These are explained failures, not transient retries.
Failed evidence remains in live-llama-tool.log and live-llama-normalized.log;
provider-only capture remains in llama-tool-wire.sse. No host effect preceded either
failure. The diagnostic source was replaced with a portable deterministic regression.

Final adapter suite: 22 passed, zero failed: 14 existing OpenAI/shared and eight
llama.cpp tests. Corrected live native acceptance: two serial tests PASS, basic chat
and host echo Tool continuation. Each returned RAH_LLAMA_OK in five text deltas;
ToolStarted/ToolFinished exactly 1/1 for the Tool turn. Both completed and disconnected.
Provider-codex not compiled. Before/after new Codex identities zero for both named
executables. Evidence: live-llama-final.log and live-final-codex-before/after.txt.
Tool support is proven for this current model/template, not all llama.cpp models.

The initial canonical Desktop gate passed 333/0/12 before the Tool correction.
Final Desktop/workspace/quality validation follows the corrected source separately.

## Final error presentation and security review

Final review found that post-start native failures still collapsed to a generic
chat code. Desktop now retains closed credential-rejected, model-unavailable and
network-failed codes for native turns; malformed llama.cpp streams have their own
closed code. Cancellation and legacy fallback semantics remain unchanged. A
deterministic mapping test covers these projections without response bodies,
headers or keys. This correction is included in the final gate reruns.

Latest live Desktop artifact: live-closure.log, two tests PASS. Basic chat and
host-authorized echo continuation each return the marker in five streamed deltas,
complete and disconnect. Tool lifecycle exactly 1/1. New codex.exe and
codex-code-mode-host.exe identities: zero (closure-codex-before/after.txt).

Native credential configuration: set OPENAI_API_KEY in the Desktop backend's
environment before launching/restarting; select OpenAI and Apply its model in the
native controls. Frontend sees presence only. llama.cpp defaults to
http://127.0.0.1:8080, requires no key and resolves the sole server model when the
field is empty. Optional RAH_LLAMA_CPP_API_KEY remains provider-local. Official
OpenAI origin is fixed, loopback-only native llama.cpp endpoints are validated,
and neither adapter reads the other's credentials. Native configuration is
session-scoped; startup environment configuration remains supported.

No new neutral abstraction, crate, dependency, ADR or version. Cargo.lock is
unchanged. Default production dependency tree contains rah-runtime-openai and
does not contain rah-runtime-codex. The two new IPC permissions configure provider
state only; they add no Tool authority. Provider switching is refused while
connected. HostExplicit remains exactly eleven existing Tools; existing host
authorization, repository-generation ownership and revocable leases are retained.

Changed areas: rah-desktop native configuration, frontend controls, model-source
and production factory composition, Tauri command/permission registration and
tests; rah-runtime-openai private shared transport, separate LlamaCppFactory and
adapter tests; bounded README/architecture/security guidance and this report.

## Closure validation

- Native adapter: 22/0/0 (14 OpenAI/shared, eight llama.cpp).
- Neutral runtime: 18/0/0.
- Final cargo test --workspace: 1072/0/18 (workspace-closure.log).
- Canonical Windows Desktop: 334/0/13, helper build and Desktop test exit zero,
  no watchdog timeout. Evidence: canonical-desktop-closure/
  20261008-233653-929-7044bea904fd486ea9a7b7a52c27c92c.
- Frontend: all eight suites PASS; Tauri inventory 49 runtime/manifest/generated/
  default permissions, 49 frontend commands PASS.
- OpenAI production fixture: native Connect, streamed first turn, host repo.status
  continuation, cancel and Disconnect/Reconnect PASS; no new Codex identities.
- llama.cpp deterministic: health/loading/configuration/model/malformed/correlation,
  cancellation plus succeeding turn, actual Desktop Connect dispatch and
  Disconnect/Reconnect PASS. Current-model live chat and Tool continuation PASS.
- cargo fmt --check, cargo check --workspace,
  cargo clippy --workspace --all-targets --all-features -- -D warnings,
  git diff --check: PASS. Source gates follow the final error-mapping correction.
- OpenAI live credential acceptance: NOT_RUN_NO_CREDENTIAL.

Publication requires the final workspace rerun, normal commit/push, master equality
and naturally triggered exact-head CI. The final response records immutable commit
and CI identities; no release preparation, tag or version bump belongs to this task.
