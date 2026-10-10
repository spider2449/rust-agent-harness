# Task 524A - File reading recovery and Windows acceptance

Checkpoint: 80fc345b1ff63e7f7c787abf6c15b785bee5e6e0. Preserve all Task 524 WIP and historical/private evidence.

1. Retain three-file fingerprint and exact formatting failure. Apply Desktop fmt and check all; review formatting-only delta.
2. Run pending direct/host fixture, fs.read bounds tests, frontend recovery, native/Desktop regressions and workspace gates including strict Clippy and command inventory. Stop at first genuine failure.
3. Build current WIP release from Git root; record executable hash and process provenance.
4. Launch exact binary visibly, preserve running application, check local provider availability; hand off as READY_FOR_HUMAN_ACCEPTANCE.
5. Await human explicit-read and ambiguous-input acceptance. Publish only after acceptance, final review and exact-head CI. No Task 525, authority/dependency/ADR/version/tag changes.

Private evidence: target/task524a/private/.

## Phase 1 and focused validation

Original patch SHA-256: 9852013E164491E90C993254AD42E52A9527DE8577FF0A8C3E89D26D9D330E6C.
Formatted patch SHA-256: 41ACF924B5CCAB8B2CEE4E0C826F191A39F95231083C2AD379B129668EE3017B.
Both patches and individual pre-format hashes retained privately. Only the three recorded Rust formatting locations changed; frontend hashes unchanged and fixture assertions retained.

Retained-process runner self-tests returned exact exit 0 and exit 7.
`cargo fmt -p rah-desktop` and `cargo fmt --all -- --check`: exit 0.
Fixture: 1 passed. An initial unqualified `--exact` filter selected zero tests; this is preserved and is not counted as fixture validation. Correct fully-qualified fixture then passed.
fs.read bounds: 9 passed, 0 failed. Chat recovery regression and all nine other frontend regressions: exit 0, including Edge layout at four widths.
Tauri inventory: 49 runtime / manifest / generated / default allows / frontend commands.
Native runtime suites: exit 0. Desktop suite: 340 passed, 0 failed, 13 ignored, exit 0. Existing eleventh HostExplicit Tool test passed; host_kind source unchanged with exactly 11 entries.
Workspace check: exit 0. Remaining workspace tests and strict Clippy pending.
No dependency, ADR, authority, version or tag changes. No release build, actual inference, acceptance or publication yet.

## E - READY_FOR_HUMAN_ACCEPTANCE

All required deterministic commands returned 0: fmt all, workspace check/test, strict Clippy all-targets/all-features and diff check. Full workspace suite results retained privately. Release build returned 0; actual Git root F:/coding/otherPrj/rust-agent-harness. No old rah-desktop.exe process; output exclusive-open check succeeded before build. Source hashes remained unchanged across build.

Executable: F:/coding/otherPrj/rust-agent-harness/target/release/rah-desktop.exe
SHA-256: 6B874F9CD14E6AE1E2B63FA2E9E8B73094765A3148561FD6F938F1DD76144EEA
Length: 21177344 bytes
PID: 16032
Start: 2026-10-09T17:14:23.1304840+08:00
Window: RAH, visible, not minimized, running. Exact executable hash rechecked after launch. Provider default explicitly llama_cpp for this child only; existing UI configuration still requires human review/connect.
Loopback http://127.0.0.1:8080 health ok, one model listed. HTTP availability only; no inference performed.
Existing production JSONL evidence enabled at target/task524a/private/human-acceptance.jsonl. No new instrumentation.

Human must admit/activate an authorized disposable repository, verify an existing UTF-8 fixture filename, Connect llama.cpp and use a fresh Conversation for one explicit fs.read request. Preserve actual Tool name/arguments, host authorization, lifecycle, typed result/error, provider continuation, terminal, displayed result and human confirmation. Prompt text is not Tool execution proof. After successful explicit read, use a fresh Conversation for ambiguous prompt negative UX acceptance. No automatic repeated inference.

Application intentionally left open. Human acceptance and publication pending. No commit/push, dependency/ADR/authority/version/tag/release changes. v0.34.0 unchanged. Task 524/523 and historical private evidence preserved. HEAD remains starting checkpoint. Suggested next step is human acceptance only; Task 525 excluded.

## Human explicit-read observation

Human supplied the displayed answer containing the two fixture lines. Read-only verification of the active repository fixture F:/coding/otherPrj/MaleCNS-Sim/fake-sample.txt matches those lines; SHA-256 830F4E06A51BBA1E20FB7BD481608BD3867B987E80B52E62A318AD1C4D7B3425. No file mutation was performed by this verification.
Production PID 16032 remains running with the verified executable. PrintWindow captured the RAH client, active repository, original prompt, displayed result and three fs.read Activity cards. Initial screen-copy image was occluded by another window and is not accepted as full UI proof. UIAutomation enumeration returned panes without useful content; this is an observation limitation, not a product failure.
Preserved explicit-read-snapshot.jsonl: session generation 1 has thread_start, tool_requested, tool_started, tool_finished, desktop_completed and desktop_chat_terminal completed. Session generation 2 is a human follow-up asking whether RAH created the file; it completed without a recorded Tool lifecycle.
Evidence supports an actual fs.read lifecycle, matching displayed content and completed Chat. Existing JSONL omits actual Tool arguments, explicit authorization decision and full typed Tool result; those fields are not claimed captured. Model continuation response is observed in displayed Chat after the Tool lifecycle. Do not infer missing arguments from prompt wording, or automatically repeat inference to fill gaps.
Await human confirmation of matching content and fresh-Conversation ambiguous-input negative UX test. No publication yet; classification A requirements remain incomplete. App left open. No new implementation change.

## Human negative UX observation - awaiting error detail

Human sent 查看文件內容 in New Conversation. PrintWindow shows the New conversation context separator before this prompt, the model asking for an identifiable path, then proposing a repository listing. Activity shows repo.list for this turn. Human-provided terminal text says Turn failed. The response above is incomplete. Chat failed runtime operation failed at chat turn.
negative-ux-snapshot.jsonl preserves session generation 3: thread_start -> tool_requested -> tool_started -> desktop_failure(failure_stage=tool_dispatch_failure, typed_native_cancelled=false) -> desktop_chat_terminal(kind=failed). There is no tool_finished or completed terminal for this turn.
Negative UX observations pass: no fabricated file content in displayed response, partial text retained, explicit failed/incomplete message, no false Completed terminal. Live post-failure next-prompt recovery not yet observed. Process 16032 remains open.
Source inspection confirms tool_dispatch_failure groups Tool, PermissionDenied and Sandbox AgentErrorCode values. It is a layer/category observation, not proof of a specific host defect or root cause. Actual repo.list arguments, complete typed error and explicit authorization record were not captured by existing production logging. Do not classify B or C from this generic stage; do not assign a model input defect merely from the ambiguous prompt or proposed listing. Request human Activity error details and explicit fixture-content confirmation. No automatic repeat inference or product change.
Publication remains pending complete human acceptance and evidence review. No commit/push, version/tag/dependency/ADR/authority change. Preserve all first-attempt records. No Task 525.

## Human supplied screenshot rah-05.png

Preserved a byte-identical private copy and SHA-256 manifest. Screenshot confirms:
- fs.read Activity Requested / Running / Completed.
- New Conversation separator before the ambiguous prompt.
- Model asks for a filename; no invented file content in this response.
- repo.list Activity Requested / Running only.
- Chat failed diagnostic and explicit Turn failed / response incomplete message remain visible; composer and Send are enabled.

The Activity remains at Running after Chat Failed without a Tool terminal card. This is observed presentation/evidence incompleteness; Running is not proof of a still-live Tool. Screenshot contains no actual arguments or typed Tool error, so it does not establish a file Tool/host defect (B), native provider continuation defect (C), or invalid model argument (D). Do not infer a cause from the generic tool_dispatch_failure category. The explicit-read success and negative Chat failure presentation are supported, but full classification A evidence and live post-failure next-prompt recovery remain incomplete. No new inference, source correction or publication performed. RAH remains open, PID 16032.

## Live post-failure recovery verified

Human sent one recovery-only prompt in the failed Conversation and reported displayed RAH response RECOVERY_OK. recovery-snapshot.jsonl captures session generation 4: thread_start -> desktop_completed -> desktop_chat_terminal completed; no Tool request is recorded in that turn. This proves a next prompt can complete after the observed failed Tool turn; it does not retry or repair repo.list.
All three source hashes still exactly match the deterministic validation and production build fingerprints. Reviewed full three-file diff: frontend presentation/recovery and regression assertions plus Desktop direct/host read fixture only. No new source or contract changes. Final diff check remains required below; no additional test repetition warranted without source changes.
Human observations now support explicit file reading, matching displayed fixture content, fresh ambiguous prompt clarification without fabricated content, visible partial-response failure, and next-prompt recovery. Deterministic/workspace/Clippy/build gates all passed. Remaining publication gate is original Phase 5's evidence-field requirement: actual Tool arguments, explicit authorization record and complete typed Tool result/error are absent from existing live recording; inferred values must not be substituted. Root cause of repo.list failure remains unclassified. Classification A/published is not claimed, and neither B nor C is established.
Validated candidate consists of the three implementation/test files plus relevant Task 524/524A reports. Private screenshots/JSONL remain local. Historical reports and evidence are preserved. Commit/push/CI have not been performed pending resolution of this explicit task evidence requirement. RAH is intentionally left running.

## Authorized continuation - Activity closure correction

Human requested continued investigation after noting repo.list remained Running. Source proves Desktop clears pending ToolCallActivity entries at turn terminal without emitting Activity closure; host dispatch errors emit RuntimeEvent Failed instead of ToolFinished. This establishes a Desktop presentation defect, independently of the unknown repo.list error cause.
Narrow correction: preserve private execution IDs, add independent presentation-only correlation, emit outcome-unknown closure for calls without observed results, and update their Running card. Preserve actual execution/error semantics, effect accounting and refresh rules. Cover actual host error lifecycle, multiple same-name calls, unstarted denial, successful finish and repeated closure deterministically. No new public runtime contract, dependency, ADR, authority, retry or Task 525 work. Original acceptance evidence remains tied to old executable SHA.

## F - Activity correction validation blocked

First required gate for the Activity correction stopped at Rust test compilation, exit 101. New fixture incorrectly creates Arc<Arc<ToolRegistry>> and indexes String event payloads as JSON values. These are fixture implementation mistakes, not a demonstrated product/runtime regression or attribution of the original repo.list failure. Full stderr, retained process metadata, and complete patch preserved privately under activity-first-failure* and activity-focused*.
Correction fmt, all fmt check and frontend Chat/Activity regression passed. New Rust tests, full Desktop/workspace regressions, strict Clippy, corrected release build and new human Activity acceptance NOT PASSED / NOT RUN after this failure. Do not use earlier green gates or old executable to certify current four-file patch.
Stop without retry or fixture changes after the failure. User WIP, original acceptance artifacts and historical files remain preserved. Old RAH PID 16032 remains open; it still has the original Running presentation behavior. No commit/push/CI, dependency, ADR, authority, version, tag or Task 525 changes.
