# Task 514D — Effective Authority presentation

Starting checkpoint: local HEAD, origin/master and live GitHub master verified
as `5afc73abcda5b8b8fa6f35ee219b7eddfbad8d4b`. Preserve Task 514/514C reports
and all prior validation evidence.

1. Attach production Tool title/details; scope reviewed-commit revocation.
2. Supply host-selected adapter identity to snapshot presentation only; keep
   native artifact source generic and suppress current identity on stale snapshots.
3. Exercise actual production rendering and Rust snapshot composition regressions.
4. Run focused checks, required quality gates and normal release Desktop build.
5. Reuse owned production WebView2 acceptance for authority, repo.status and Cancel.
6. Publish the presentation fix only after quality gates pass; verify exact-head CI.
   Record actual UI results separately from publication. No release preparation.

No runtime factory, Tool definitions, permission, HostExplicit, transport, profile,
repository authority, dependency, ADR or version changes are planned.

## Correction and focused evidence

`renderEffectiveTool` previously constructed a strong title and definition list,
but returned a list item containing only the Host action box. It now attaches
all three production nodes. Eligibility still exclusively controls forms.
Reviewed commit revocation explicitly says “Reviewed commit authorization revoked”.

The snapshot previously hardcoded connected kind `codex` and mapped every native
artifact source to `native_openai`. The presentation input now receives the existing
host-owned `RuntimeAdapterIdentity`, gathered while the captured connection is
locked. Current snapshots map OpenAI/llama.cpp/Codex to OpenAI/llama.cpp/Codex CLI;
native source is independently `native`. Noncurrent snapshots have no runtime kind,
and production rendering labels their runtime with snapshot currentness status.
The runtime factory contract and authorization state are unchanged.

Changed implementation: frontend/status.js, src/effective_authority.rs and src/main.rs
under crates/rah-desktop. Regression coverage: frontend/status_authority_test.js and
the existing effective_authority Rust tests. Tests execute the production rendering
functions and inspect their attached DOM tree; no duplicate renderer was added.

Focused production frontend regression PASS; Rust snapshot tests 15 passed.
Native adapter tests 23 passed; neutral runtime tests 18 passed. Initial canonical
Windows Desktop gate passed 337 tests, 13 ignored, exit zero, no watchdog timeout.
Nine frontend suites and the 49-command Tauri inventory passed. Final-source
workspace tests passed 1,076 tests, zero failed, 18 ignored, including the same
337-pass/13-ignored Desktop inventory. `cargo fmt --check`, `cargo check --workspace`
and `cargo clippy --workspace --all-targets --all-features -- -D warnings` all
exited zero. Normal production release build and diff-check subsequently exited
zero; actual UI acceptance stopped at the contract conflict below. Command exit
records: target/task514d/gate-exits.txt;
separate stdout/stderr retained for each Cargo gate.

Prior reports remain untracked and unstaged. Task 514 SHA-256 remains
`DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43`;
Task 514C SHA-256 is
`A6C834178A8428F3EBD3F3F99A4B254B639DFA1A9A6BEEA3E3B326AD8CAE277F`.
New logs and disposable checkout are isolated under target/task514d.

## Actual Windows authority observation and contract conflict

Normal `cargo build -p rah-desktop --release` and `git diff --check` exited zero.
Executable SHA-256:
`69B6947A5991F5BE1A8FC923082D3ECCD3A1357E48E1ED04A9869C9ED155F2CB`.
An owned actual production WebView2 launch connected to llama.cpp with the admitted
disposable checkout. Existing acceptance methods were reused; the child OpenAI
key was absent. Listener ancestry and native RAH window identity are preserved.

Actual snapshot and screenshots show llama.cpp / native, 17 named effective Tools,
zero unavailable capabilities, attached source/effect/authority/permission/binding/
advertisement details, and distinct eligible Host action controls. All screenshots
were visually inspected. `fs.read` displays Read classification, Advertised and
“Host action — not Model”; ineligible echo has no controls. `repo.status` displays
Read-only effect, Repository observation authority, **Execute classification**,
Advertised, and an eligible Host action. Screenshots and snapshot are under
target/task514d/ui: effective-authority.png, repo-status-authority.png,
fs-read-authority.png, authority-capture.json and ui-evidence.json.

The required `repo.status` Read assertion failed before any model turn, Tool or
Cancel attempt. Source at the starting master and current unchanged source both
define `RepositoryStatusTool` with `PermissionLevel::Execute`
(crates/rah-tools/src/repository_status.rs:59). Effective Authority copies the
actual definition permission; PermissionLevel::Execute denotes subprocess
execution, while the independently displayed effect remains Read-only.
Thus the task's required Read display conflicts with its instruction to preserve
Tool definitions and permissions. No false Read label, permission change,
assertion weakening or retry was performed. A clarification is pending on which
acceptance contract governs; Tool/Cancel acceptance and publication remain pending.
Host logs show repository admission, composition and connection only: no model
turn or Tool execution. Prior chat messages visible in the transcript are historical
514B data and are not acceptance evidence for this run.

## Initial stopped disposition and human clarification

**F — ACTUAL UI ACCEPTANCE STILL UNVERIFIED.** Required repo.status Read assertion
cannot be met while preserving its existing Execute definition. This is an
acceptance-contract conflict, not evidence of a new ToolRegistry failure or a
request to redesign authorization. The actual UI proves the implemented detail/
identity corrections, but no complete Task 514D acceptance PASS is claimed.
No Tool prompt, Cancel prompt, permission widening, commit, push, tag, release or
version bump was performed. Local HEAD remains the starting master; successful
CI 37859204714 covers that starting SHA, not this uncommitted fix.

After screenshot preservation, actual Disconnect + Refresh showed a disconnected
snapshot with runtimeKind omitted, advertisement false and zero effective Tools.
The temporary remembered candidate was removed through its production command.
The owned Desktop PID 6400 was creation-time/path/hash verified and normally closed;
cleanup.json records requested normal close and successful exit. The first cleanup
observation expected null where the existing serde schema omits None; preserved
the original cleanup script, corrected only that observation to require omission,
and completed remaining cleanup without repeating a model or Tool attempt.

The human subsequently instructed: “Preserve Execute; verify Read using fs.read
and accept repo.status’s actual permission.” This resolves the acceptance-contract
conflict without changing any Tool permission or definition. The original failed
Read assertion and first launch evidence remain preserved; a fresh owned UI session
under target/task514d/ui-accepted-contract uses the clarified requirement. The
frontend regression now uses fs.read for Read and independently verifies repo.status
Read-only effect plus Execute dispatch permission. Its focused rerun passed.
Production source and the tested production executable are unchanged. Tool/Cancel
acceptance and publication remain pending at this checkpoint. Task 515 is not
authorized or started.

## Clarified actual acceptance and final classification

The fresh owned production session used the same normal executable and disposable
checkout; child OpenAI key absent. Actual authority screenshot review passed:
llama.cpp / native, all 17 Tool names/details attached, repo.status Execute,
fs.read Read, advertised state and Host action distinction. Ineligible Tools had
no forms. Source and exact eleven-kind regression preserve HostExplicit = 11.
No Host action was invoked as model-execution proof.

**Actual repo.status round-trip PASS.** One model-selected call in session 1,
Activity Requested / Running / Completed, and one host tool_requested /
tool_started / tool_finished sequence preceded desktop_completed. The assistant
returned the exact random untracked filename omitted from the prompt. The retained
fixture still has only that untracked file. This proves functional correlated
result delivery and final continuation; the existing logger does not expose raw
wire call IDs, so no captured-wire-ID claim is made.

**Cancel stopped the stream; recovery PASS; cancellation presentation FAIL.**
At 00:52:48.064Z, the actual UI had streamed more than 100 characters, chatRunning
true, disabled prompt and the Cancel Turn button. CDP mouse input pressed Cancel.
At 00:52:48.073Z the UI was idle with enabled prompt and Send, but displayed
`Chat failed runtime operation failed at chat turn.` Host session 2 terminal was
desktop_failure / model_runtime_failure, rather than cancellation. Subsequent
session 3 returned RAH514D_CANCEL_RECOVERY_OK. Disconnect/reconnect created runtime
generation 2; session 4 returned RAH514D_RECONNECT_OK. Both recovery turns completed,
with no stale lease preventing admission. Final Disconnect/Refresh exposed no
runtimeKind and no effective advertised Tools.

Source explains the observed classification path: native cancellation's biased
CancellationToken branch produces a typed OpenAiAdapterError::Cancelled inside
RuntimeEvent::failed (rah-runtime-openai/src/lib.rs); that typed error projects
Operation (error.rs). Desktop run_chat handles the Failed terminal before
cancel_chat can win its terminal claim, presenting generic ChatRuntimeFailed.
The native cancellation test already expects a typed Cancelled failure and
CancelOutcome::Stopped. The defect is Desktop cancellation presentation/terminal
translation; no HTTP transport or cancellation/recovery redesign was attempted.
No claim that cancellation rolled back any remote effect is made.

The acceptance script printed cancel PASS because it checked stopping and recovery.
Evidence review rejected that flag as complete acceptance: adjudication.json
records the terminal presentation failure, while the original ui-evidence.json
and host logs remain unchanged. No Cancel retry was performed.

**D — CANCELLATION / RECOVERY DEFECT FOUND**, specifically cancellation presented
as generic runtime failure. Presentation correction and model-selected Tool
acceptance passed; full Task 514C product acceptance remains incomplete.
The quality-approved presentation patch is authorized for publication under
Task 514D's partial-acceptance publication rule. Publication/CI results follow at
closure; no tag, release, version or additional production changes are included.
Task 515 remains unauthorized. Suggested next task: separately scoped Desktop
native cancellation terminal-presentation correction and deterministic/Windows
acceptance, preserving existing runtime and authority contracts.

Accepted-session evidence: target/task514d/ui-accepted-contract/ui-evidence.json,
ui-host-events.jsonl, ui-launch.json, ui-listener-ownership.json, adjudication.json,
effective-authority.png, repo-status-authority.png, tool-result.png,
cancel-recovery.png and reconnect-recovery.png. PID 15980 was path/hash/creation-time
verified, then normally closed; cleanup.json proves exit. Temporary candidates
were removed through production commands, and the original remembered catalog is
restored. No current rah-desktop.exe process remained at cleanup inspection.
