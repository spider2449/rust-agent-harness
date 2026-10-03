# Task 508 — OpenAI production composition and Codex-free proof

Starting HEAD: `80a937806ef7a44ca2ccf624f5c616b75e3b04be` (clean master).
Supplied checkpoint CI: `37120001093` PASS. Version stays 0.33.0; no
v0.34 capability, tag, release or version bump.

## Implementation and configuration

Desktop adds optional `provider-openai = ["dep:rah-runtime-openai"]`.
Default remains `provider-codex`. Selection is private and captured when
DesktopAppState is created. Single-provider builds select that provider;
both-provider builds require backend `RAH_RUNTIME_PROVIDER=codex|openai`.
Missing/unknown both-provider selection and no providers fail closed with
`runtime_adapter_unavailable`; there is no fallback.

OpenAI configuration is backend-only process environment: `OPENAI_API_KEY`
and explicit nonempty `RAH_OPENAI_MODEL` (maximum 256 bytes). Configure before
startup. No credential persistence/UI or frontend Tool credential channel.
The production origin stays fixed at https://api.openai.com/v1/responses.
OpenAI validates configuration, has unsupported discovery, and forwards the
explicit model; no catalog or external model-availability claim. Codex keeps
its existing admission and catalog preflight.

Shared production connection assembly moves from codex_composition.rs into
production_composition.rs. Factory/preflight/bind, currentness, registry/policy,
publication, generic turn/cancel/Disconnect/shutdown and repository lifetime
remain common. Native source metadata does not claim a Codex version/executable.
No second Tool dispatch path, new Tool, permission or repository selector.
ADR-B: accepted ADRs 0032/0033 suffice; no new ADR or neutral API change.
New dependency edge: optional Desktop -> existing OpenAI adapter, required by
this task. No new dependency package or secret-storage library.

`openai-fixture` is a development proof feature, enabling OpenAI and the
adapter's `fixture-support`. The seam accepts only a loopback SocketAddr and
uses a fixed fake credential. Ordinary `provider-openai` excludes this seam.
Fixture tests inject only the configured factory/model at the same production
connection assembly; registry, authorization, publication and lifecycle are real.

```powershell
cargo build -p rah-desktop --bin rah-desktop
cargo build -p rah-desktop --bin rah-desktop --no-default-features --features provider-codex
cargo build -p rah-desktop --bin rah-desktop --no-default-features --features provider-openai
cargo build -p rah-desktop --bin rah-desktop --no-default-features
cargo check -p rah-desktop --features provider-openai
cargo test -p rah-desktop --no-default-features --features openai-fixture task508 -- --test-threads=1
```

## Development evidence

All-feature development check passed. Initial test compile exposed one legacy
Codex-only exhaustive source match; Native was explicitly rejected there.
First executable development fixture: 2 passed / 1 failed. Selection/config
passed and ordinary text/completion passed; Tool script omitted required
function_call_arguments.done before output_item.done. Adapter correctly returned
sanitized Protocol failure before dispatch. Preserved log:
F:/temp/task508-dev-tests2.log. Corrected fixture only, no parser/authority change.
Next all-feature development fixture: 3 passed / 0 failed (6.72 seconds),
F:/temp/task508-dev-tests3.log. Covers production Connect, streamed ordinary turn,
one repo.status execution/continuation with matching call_id, Desktop send_chat,
Disconnect, retained runtime/port rejection, blocked connected repository switch,
permitted disconnected switch and zero Codex activation counters.

## Final validation and frozen failure checkpoint

GitHub master and checkpoint CI were independently verified: starting SHA
`80a937806ef7a44ca2ccf624f5c616b75e3b04be`, CI `37120001093`, completed/success.
That historical CI does not validate this uncommitted patch.

Development compile corrections also fixed the factory helper's selection
argument and the no-provider-only exhaustive Option match after static selection
was captured in app state. Failed logs remain at focused-openai.log and
focused-none.log under F:/temp/task508-evidence/. Final proof adds an inactive
repository marker, history comparison across Disconnect and stale-port rejection
after switching repositories. Final OpenAI-only fixture: 3/0/0, 8.16 seconds.

| Feature mode | Actual result |
| --- | --- |
| default | Codex; selection 2/0/0; workspace Desktop 335/0/20 |
| Codex-only | Codex; explicit no-default/provider-codex selection 2/0/0 |
| OpenAI-only | OpenAI; production build PASS; production fixture 3/0/0 |
| both | Explicit backend selection required; absent/unknown rejected; compile/selection 2/0/0 |
| none | Fail closed; focused Connect 1/0/0; production build and empty-PATH smoke PASS |

Cargo trees used explicit x86_64-pc-windows-msvc and normal/build/dev edges.
openai-tree.txt includes rah-runtime-openai and zero rah-runtime-codex entries;
default-tree.txt includes Codex and zero OpenAI entries; none-tree.txt includes
neither. This is Desktop dependency evidence, independent of workspace membership.

OpenAI-only production build (the exact command above) passed. Its executable
was copied to the evidence root and run with PATH empty and
--runtime-composition-smoke: exit 0,
`{"adapter":"openai","codex_compiled":false,"http_requests":0,"passed":true}`.
This proves headless production state/adapter initialization, not GUI automation
or external inference. Codex discovery/admission/configuration modules are feature
excluded; fixture activation counters also observed zero Codex resolver and
runtime-construction activation. No CLI is required or invoked on this path.

No-provider production build passed. Its copied executable with PATH empty
returned exit 0, adapter_available=false, runtime_adapter_unavailable,
runtime_status=not connected and passed=true. No panic or Codex activation.

Final OpenAI-only fixture uses real shared production connection assembly:
Connect publishes runtime/conversation and Native source; ordinary deltas arrive
and completion occurs once; one repo.status executes through the production
HostToolScope/authorized dispatch, returning error-free clean active-root status.
Matching fixture-call function_call_output reaches the local continuation and
final text completes. A separate send_chat invocation uses generic Desktop chat
coordination and returns to Idle. Four local HTTP requests are captured, all
with fixture-model and the fixed fake credential. No paid/provider request.

Connected repository switching is rejected. Disconnect revokes scope, withdraws
composition and shuts down the adapter; retained runtime and port fail closed.
Repository generation/member and conversation history survive Disconnect.
Disconnected activation of the other admitted repository succeeds; old port
remains unusable afterward. Inactive repository marker is absent from Tool output.

| Gate | Actual result |
| --- | --- |
| Task 499 preflight | 4 passed / 0 failed |
| Task 503 experimental Codex tests | 7/0/0; integration filter matched zero, full workspace later ran integration suites |
| Task 504 Desktop lifecycle | 4 passed / 0 failed |
| cargo fmt --check | PASS, exit 0 |
| cargo check --workspace | PASS, exit 0 |
| cargo test --workspace | PASS, 1,062 passed / 0 failed / 24 ignored |
| Desktop within workspace | 335/0/20, 281.82 seconds |
| OpenAI core | 14/0/0 |
| Codex core/integration | 102/0/1 plus 6/0/0 and 11/0/0 |
| cargo clippy --workspace --all-targets --all-features -- -D warnings | FAIL, exit 101; collapsible_if at production_composition.rs:49 |
| Canonical Windows Desktop PrepareOnly | PASS; helper preparation only |
| Canonical Windows Desktop full gate | NOT RUN after Clippy failure |
| Frontend/static | Seven JS syntax checks and all six test files PASS, including browser layout |
| Tauri inventory | PASS: 47 runtime/manifest/generated/default allows/frontend commands |
| Cargo metadata | 14 members; Desktop 0.33.0/edition 2024; Codex default; optional OpenAI edge |
| HostExplicit | Static exactly 11; executable workspace host_allowlist_is_exact PASS |
| git diff --check | PASS after stop |

Stable-source gates used CARGO_TARGET_DIR=RAH_TEST_TARGET_DIR=
F:/temp/rah-task504-target. Evidence root: F:/temp/task508-evidence/.
Hashes: final-source-hashes.txt. Failure: workspace-clippy.log.
Canonical preparation record:
desktop-prepare/20261003-200839-029-4eb6cc62c62b420e9827dbd76956e537/.
Standalone default executable build was not run; default workspace check/test
and focused selection/regressions passed. Canonical/full-gate PASS is not claimed.

First frozen full-gate failure: nested Codex configuration-validation if,
introduced while preserving pre-activation validation ordering. Clippy requests
the equivalent combined if/let condition. Source remains unchanged at failure;
no suppression, correction or gate retry follows. Only report/evidence recording.

Authority/security delta NONE: registry ownership, policy, repository selection,
permission mutation, HostExplicit, effect accounting and neutral lifetime remain
host-owned. No authority or credential regression observed. Cancellation remains
local HTTP/SSE cancellation through generic turn control; no remote-work or
Tool-effect rollback claim. ADR-B; no new ADR or dependency package.

**LIVE_PROOF_READY: NO** — structural/focused proofs pass, full gates incomplete.
No real key, public OpenAI API request, paid/live inference or availability proof.

**E — DETERMINISTIC VALIDATION FAILED**

No commit or push authorized by this result. HEAD remains starting SHA. Eleven
tracked files modified; two untracked task files (this report and shared module);
none staged. Worktree intentionally dirty and frozen, retaining failed evidence.
No tag/release/version bump or new exact-head CI.

Suggested next task: narrow follow-up for the equivalent Clippy condition fix,
then validate corrected source, finish canonical Desktop/remaining build checks,
and require classification A before commit, normal GitHub master push and natural
exact-head CI closure. No live inference.

## Task 508A follow-up reference

The frozen failure and evidence above are retained. See
[Task 508A](2026-10-03-task-508a-clippy-correction-and-final-openai-composition-validation.md)
for the source-equivalent Clippy correction, resumed gates and final local
classification. This note does not rewrite the original Task 508 checkpoint.

## Task 508L requirements

Only after Task 508 classification A and separate explicit human authorization:
use the validated OpenAI-only production build, backend-only real key and explicit
model at fixed origin; prove actual external Connect/ordinary streamed completion,
one harmless authorized read-only Tool round trip, cancellation and Disconnect/
revocation/repository currentness. Preserve sanitized diagnostics/effect accounting;
never claim model discovery, entitlement from fixtures, rollback or automatic replay.
No secret-bearing artifacts. Task 508 itself grants no live inference authorization.
