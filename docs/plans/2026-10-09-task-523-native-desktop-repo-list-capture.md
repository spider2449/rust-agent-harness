# Task 523 — Native Desktop repo.list failure capture

Local diagnostic plan, 2026-10-09. Starting HEAD/origin/master/GitHub master:
`80fc345b1ff63e7f7c787abf6c15b785bee5e6e0`; exact-head CI run
37901305764 completed/success. No correction is authorized.

1. Preserve three historical untracked reports, ignored evidence and immutable
   v0.34.0; retain byte-identical source backups and restoration manifest.
2. Prepare temporary opt-in capture using RAH_TASK523_CAPTURE together with
   RAH_LIVE_EVIDENCE_PATH. This build accepts only reviewed Task 523 records,
   capped at 4096, with PID/time, session, host call and round correlation.
3. Capture replay count/bytes, actual advertised Tool name and input category,
   actual parser outcome, host authorization, execution terminal, observer child
   status/counts, closed typed error details, serialization/correlation, native
   HTTP/protocol/round terminal, backend events and Desktop IPC emit outcomes.
   Never capture raw arguments, paths, prompts, results, history or credentials.
4. Validate the diagnostic build; stop at the first required deterministic failure.
   Build actual Windows production Desktop with isolated target provenance.
   Prove executable path/SHA256/length/PID/start time and capture startup record.
5. Human alone admits and activates a repository, connects native llama.cpp,
   opens New Conversation and sends exactly one specified listing request.
   Human reports visible Activity and Chat; prompt wording is not actual arguments.
6. Interpret A–F from actual evidence. Do not repeat Task 522 success fixtures,
   retry inference, normalize paths, change permissions or replay/round limits.
7. Preserve instrumentation and evidence privately; restore all source bytes and
   verify manifest hashes, historical reports and empty unintended tracked diff.
   Publish a sanitized report only for useful new evidence, review exact paths,
   diff check, normal commit/push and exact-head CI. No version/tag/release/ADR.

Private working material: ignored `target/task523/private/`.
Human confirmed availability to perform one probe when build is ready.
Production preparation stopped: diagnostic release build returned exit 101,
E0433 in temporary `experimental_host.rs` instrumentation because `serde_json`
is not a production dependency of rah-runtime. This is an instrumentation defect,
not evidence of the original Desktop failure. No dependency was added or retry
performed. `cargo fmt --check` returned 0. The already scheduled capture-only
privacy/category tests returned 0 (two passed); they do not override the failed
production build gate. No Task 522 fixtures or inference requests were repeated.

`ACTUAL_DESKTOP_PROBE_NOT_PERFORMED`

Decision: **F — ACTUAL DESKTOP CAPTURE NOT COMPLETED OR INCONCLUSIVE**.
Human confirmed availability, but no diagnostic executable was produced/launched.
Actual arguments, measured replay, authorization, Tool result, continuation,
Chat terminal and Activity presentation remain unobserved. Task 524 correction
is not justified by this evidence. Preserve this local plan and private failure
artifacts; do not publish a redundant diagnostic report or documentation commit.

Closure: all seven temporarily modified tracked files restored to their original
SHA256; temporary helper moved into the private evidence directory. Tracked diff
and `git diff --check` are empty/clean. Historical report hashes remain identical.
Final HEAD/origin/master/live GitHub master still match the starting SHA; exact-head
CI run 37901305764 remains completed/success. No commit, push, version, tag, release,
ADR, dependency or authority change. HostExplicit remains 11. Only the local Task
523 plan was added to the three historical untracked reports.
