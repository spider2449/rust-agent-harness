# Task 510B-R5C — Provider-active cancellation readiness

Starting HEAD: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
Preserve all existing unstaged/untracked WIP and empty staging.

1. Audit Tasks 500/501/502, ADR 0033, neutral event/lease/control contracts,
   Codex event origin, exact generated 0.157.1/0.160.0 readiness schemas,
   and drop cleanup. Stop on S2 semantic defect or unavailable readiness.
2. For S1, change only certification synchronization: captured provider
   `turn/started` with exact acknowledged thread/turn IDs and inProgress state;
   reject already observed terminal state. Use event notification, no sleeps.
3. Run deterministic regressions before exactly one fresh bundle cancellation.
   Retain ordered timestamped wire/neutral/shutdown evidence and identities.
   Stop on corrected live failure; no retry or Tool recertification.
4. Only after cancellation PASS resume remaining R5 gates and full closure.
   Only final A permits certification commit/push/exact-head CI. No Task 510C.

Private evidence: `F:/Temp/rah-task510br5c-evidence`.

Audit: AgentEvent docs say "A model backend request started." Task 500 section
10 explicitly records ambiguous logical-step granularity, rather than provider
RPC readiness. MinimalTestRuntime yields the event before backend.complete.
Task 501 adds cancellation outcomes but no provider-readiness guarantee. Task
502 and ADR 0033 assign admission/effect accounting to the host, cancellation
translation to the adapter. Therefore S1: logical request start, not a promise
that a remote provider already accepts interruption. Codex event_stream emits
it locally on stream polling after successful turn/start response and ID parsing;
it does not originate from write completion or provider turn/started.

HostTurnLease admits host Tool submissions and owns effect accounting. It does
not attest remote provider activity. The start acknowledgement establishes
provider routing IDs; the separate turn/started notification reports activity.
Both exact locally generated schemas expose TurnStartedNotification with
threadId and turn.id/status; use the same synchronization without version branches.

Drop audit: runtime.rs TurnGuard sends best-effort interrupt when no provider
terminal was consumed. Failed explicit cancellation does not set that terminal
flag, so ID 5 is expected fallback cleanup, not proof of a second explicit retry.
Preserve both production guards. Successful certification consumes terminal/EOF
and closes conversation before shutdown to distinguish explicit and drop cleanup.

## Result

**B — MODELREQUESTSTARTED SEMANTICS WERE MISUSED; HARNESS FIXED BUT LATER R5 GATE FAILED**

S1 synchronization correction is certification-feature-only. ProtocolCapture
retains timestamped wire messages and wakes a Notify waiter; readiness requires
matching start acknowledgement and provider turn/started with inProgress status.
Already captured matching turn/completed vetoes cancellation. This is a
snapshot check, not a claim to eliminate a later provider completion race.
No production event mapping, interruption method/params, or cleanup changed.

Deterministic regression command:
`cargo test -p rah-runtime-codex --features certification-harness observation_tests -- --nocapture`
PASS: 6/0, including local-start insufficient, provider-start unlock without
sleep, normal completion veto, foreign thread/turn exclusion, required start
acknowledgement, and existing two-direction wire capture. IDs are taken from
actual acknowledged route; live interrupt matched those exact IDs. These tests
exercise readiness eligibility, not a full fake-provider cancellation dispatch.

Exactly one corrected fresh cancellation turn: snapshot_cancellation, ignored
exact test with RAH_R5_BUNDLE=1, 1/0 PASS, 39.55s. No Tool advertised/requested/
executed; original lighthouse workload. Timestamp units below are Unix us:

| Event | Timestamp / result |
| --- | --- |
| turn/start request, ID 3 | 1791209744176603 |
| start acknowledgement, inProgress, startedAt null | 1791209744189226 |
| local ModelRequestStarted observed | 1791209744189409 |
| provider turn/started, inProgress, startedAt 1791209744 | 1791209744315584 |
| explicit turn/interrupt, ID 4 | 1791209744315779 |
| interrupt accepted, `{"id":4,"result":{}}` | 1791209744355077 |
| provider turn/completed, status interrupted, durationMs 158 | 1791209744355218 |
| neutral Cancelled observed | 1791209744355414 |
| lease released / fresh host lease accepted | afterward, log order retained |
| explicit shutdown | afterward, Ok(Ok(())), alive=false; no independent shutdown timestamp |

Exact thread: `01a10c6b-a67d-72b3-8522-b3c6d3102714`.
Exact turn: `01a10c6b-a731-7a81-a977-005903dee393`.
Outgoing params: threadId and turnId with these exact values. One explicit wire
interrupt, no ID 5/drop-time interrupt captured. Neutral repeat cancel returned
AlreadyTerminal; stream consumed through EOF and conversation closed. Provider
readiness preceded cancellation by 195us; no sleep or elapsed-time heuristic.

Resumed genuine live diagnostic: snapshot_diagnostic ignored exact test,
1/0 PASS, 44.50s. Invalid model rah-task510b-invalid-model caused Failed;
process-local source downcast to CodexAdapterError JsonRpc or TurnFailed PASS.
Public diagnostic `{"operation":"turn","kind":"provider_rejection"}`;
model/path/hash excluded. Shutdown Ok(Ok(())), alive=false, identity unchanged.
No additional standalone diagnostic wire archive was exported by this harness.

Later failed gate: Desktop certification harness compilation, exit 101. Added
task510br5c_bundle_desktop_turn to exercise real backend Connect/catalog/chat/
Disconnect with the validated bundle. It did not launch because of two harness
type mistakes introduced here:
- certification_tests.rs:83 E0277: ConnectionResult does not implement Debug.
- certification_tests.rs:145 E0308: disconnect_codex returns ConnectionResult,
  while the test matches unit.

First stable-source failure preserved; no correction/rebuild/retry after STOP.
Desktop test is retained as uncompiled WIP, not certification evidence. No live
Desktop inference occurred. No production prerequisite defect inferred.

## Protocol matrix and remaining gates

| Gate | Evidence |
| --- | --- |
| startup / initialize | corrected live runtime constructed PASS |
| model/list | prior Tool/catalog evidence retained; no fresh standalone catalog gate |
| thread/start / turn/start | corrected cancellation responses PASS |
| provider turn-active signal | fresh exact turn/started PASS |
| streaming / normal completion | prior Tool gate PASS preserved |
| Tool advertisement / call / result continuation | prior proven 1/1/1 gate PASS preserved, not rerun |
| cancellation | fresh accepted interrupt / interrupted terminal / neutral Cancelled PASS |
| diagnostics | fresh typed local cause / sanitized public envelope PASS |
| shutdown | cancellation and diagnostic explicit shutdown PASS |
| real Desktop | FAIL at harness compilation, no launch |
| fresh ordinary production rejection | NOT RUN after STOP |
| fresh 0.157.1 certified baseline verification | NOT RUN after STOP |
| full workspace/Desktop/frontend closure | NOT RUN after STOP |
| Tauri 47/47/47/47/47, 14 packages, 0.33.0, edition 2024, HostExplicit 11 | not freshly certified |

Final independent bundle measurement confirms:
- codex.exe SHA256 fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d,
  length 326872368, file ID 0x000000000000000000110000000c1861.
- companion SHA256 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6,
  length 74697520, file ID 0x000000000000000000180000000c1862.
- final bundle-owned process census empty (0), both names covered by bundle path.

Retained all starting WIP. New task changes: certification_support.rs readiness/
notification/timestamp capture plus regressions; task510b_snapshot.rs readiness
wait and private evidence; certification_tests.rs failed Desktop test; this report.
Cargo.lock had a pre-existing rah-sandbox edge at task start; this task did not
change it. No dependencies, ADRs, authority/security, baseline/admission, versions,
ToolRegistry, cancellation wire, or production lifecycle changed.

HEAD remains the starting SHA; staging empty; no commit/push/new CI. Starting
R4L CI 37317133407 is historical PASS, not a new certification result.
Task 510C denied. Next bounded task: correct the two Desktop certification-test
type errors, preserve this failed evidence, then resume remaining R5 gates without
repeating the successful Tool or cancellation turns unless full closure requires.

Final scoped review: cargo fmt --check PASS; git diff --check PASS. Status/stat
inspected; HEAD unchanged and staged path count 0. Failed Desktop source copied
to private evidence alongside desktop.log/desktop.exit and final tracked patch.
No full deterministic closure claimed.
