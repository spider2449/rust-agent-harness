# Task 510B-R5B cancellation protocol diagnosis

Starting HEAD: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
Preserve all starting unstaged/untracked WIP and historical R5 failure.

Plan: capture complete outgoing cancellation before transport write and incoming
response privately, compare exact bundle-generated 0.160.0 schema with local
0.157.1 schema, audit identifiers and lifecycle, and correct only an evidenced
narrow mismatch. Run deterministic regression before one fresh live retest.
Stop on live failure. Resume remaining R5 gates only after cancellation PASS.
No admission, ToolRegistry, baseline, authority, or Task 510C changes.

Private evidence: `F:\Temp\rah-task510br5b-evidence`.

## Result: B — CANCELLATION LIFECYCLE/HARNESS ERROR

Protocol classification: **C1 — SAME CANCELLATION CONTRACT**. Both exact local
executables generated experimental schemas into schema157/schema160. Both
ClientRequest schemas require id/method/params for turn/interrupt; both params
schemas require string threadId and turnId; both response schemas are objects.
No jsonrpc member is required by this Codex stdio contract. Cancellation is a
request, not a notification. No token, conversation ID or model request ID belongs
in these params. Adapter shape originated in experimental runtime adaptation
commit 3ab73e3 and matches the existing restricted runtime cancellation shape.

Exact outgoing request (jsonrpc absent, no cancellation token):

```json
{"id":4,"method":"turn/interrupt","params":{"threadId":"01a10c63-37e7-7890-949a-a7f4f588210e","turnId":"01a10c63-388b-7993-801a-16022308f625"}}
```

Exact incoming response (data and jsonrpc absent):

```json
{"error":{"code":-32600,"message":"no active turn to interrupt"},"id":4}
```

Identifiers exactly match thread/start and turn/start results. Actual JSON has
correct casing, nesting, string fields and request ID; no null/omitted params.
Thread created; turn/start acknowledged status=inProgress, startedAt=null,
completedAt=null, items empty. No provider turn/started, agent delta, Tool request
or turn/completed was received before cancellation. Neutral Started and locally
synthesized ModelRequestStarted were consumed. Neutral host lease was admitted;
Control::cancel changes accepting true->false and calls lease.stop_requests()
before sending. Thus local admission/start acknowledgement was mistaken for
provider cancellability. No evidence of completed-turn cancellation: the provider
explicitly reported no active turn, without any preceding completion. Static
schema proves the wire shape, not provider activation timing guarantees.

One capture reproduction, no Tool, unchanged lighthouse prompt/model. Exit 101,
0 passed / 1 failed, 38.81s. Typed local cause retained and printed privately.
The existing stream drop guard queued a second interrupt (id 5) after failure;
no corresponding reply was received before shutdown. This is pre-existing
cleanup behavior, not an operator retry; no claim of exactly-one wire interrupt.
No further live attempt, behavior correction or remaining R5 gate was performed.
Follow-up harness correction should synchronize on actual provider streaming
(neutral ModelDelta), audit completion before send, and cover timing
deterministically before any fresh live retest. No arbitrary sleep is indicated.

Only capture infrastructure changed: outgoing messages recorded before the
transport write, existing cancellation harness retains full wire after shutdown
and prints process-local cause on failure. Capture observer deterministic test
1/0 PASS. No cancellation regression correction tests claimed. No protocol
discriminator/version branching needed: exact contracts identical.

Shutdown Ok(Ok(())), runtime alive=false, main post-measurement equals pre.
Independent final hashes, lengths and file IDs for both bundle members match
the supplied checkpoint. No residual bundle-owned processes. Bundle unchanged.

Protocol matrix: startup/initialize/thread-start/turn-start response observed
here; prior R5 Tool PASS preserved (advertisement/call/result/continuation/text/
completion). Cancellation FAIL. Diagnostic, Desktop, fresh production rejection,
fresh baseline certification verification, final complete matrix and full
workspace/Desktop/frontend/Tauri/metadata/HostExplicit closure NOT RUN after STOP.
Baseline executable used solely for local schema generation, not live inference
or renewed certification; existing baseline support/admission unchanged.

All prior WIP preserved. New R5B changes confined to certification_support.rs,
task510b_snapshot.rs and this plan/report. No dependencies, ADR, public API,
authority, admission, ToolRegistry, provider state or baseline changes.
HEAD unchanged; staging empty; no commit/push/new CI. Starting exact-head CI
37317133407 remains historical checkpoint evidence. Task 510C denied.
