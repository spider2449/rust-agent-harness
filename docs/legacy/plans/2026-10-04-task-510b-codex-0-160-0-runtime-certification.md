# Task 510B — Codex 0.160.0 runtime certification

Reference-only follow-up: [Task 510B1 harness prerequisite audit](2026-10-04-task-510b1-codex-certification-only-desktop-harness.md).
This reference does not change this report's classification or resume certification.

Starting HEAD and local origin/master: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Initial worktree clean. Starting exact-head CI `37189461776` is the supplied
Task 509 historical checkpoint; no new CI verification was performed here.

## Plan and stop condition

1. Read ADR 0030 and relevant architecture/security material.
2. Measure baseline and exact candidate, then inspect whether an existing
   bounded certification-only Desktop path can run the real artifact.
3. Only if that prerequisite exists, perform schema, deterministic, direct,
   Tool, cancellation, diagnostic and Desktop certification gates serially.
4. Preserve production admission and preferred baseline throughout.

**B — DESKTOP CERTIFICATION REQUIRES WEAKENING PRODUCTION ADMISSION**

Stopped at the task section 21 prerequisite. No existing real-artifact
certification-only admission mechanism was found. Do not substitute fake
transport tests or historical direct success for Desktop certification.

## Exact artifact audit

| Field | Certified baseline | Candidate |
| --- | --- | --- |
| Reported version, measured with --version | codex-cli 0.157.1 | codex-cli 0.160.0 |
| Bytes | 322515248 | 326872368 |
| SHA-256 | `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574` | `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d` |
| LastWriteTimeUtc, descriptive only | 2026-09-26T08:20:41.5289688Z | 2026-10-02T11:18:58.4901097Z |
| PE machine, measured | 0x8664, Windows x86_64 | 0x8664, Windows x86_64 |

Baseline path:
`C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe`.

Candidate path:
`C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.

Both executable hashes were measured twice, before and after --version checks,
and matched the identities above. No live/direct certification probes ran;
these measurements are not a pre/post live-probe immutability certification.

Candidate provenance: existing global npm package; local @openai/codex
package.json reports 0.160.0. No acquisition, installation or external supply
chain verification occurred. Adjacent bin inventory contains codex.exe and
codex-code-mode-host.exe (74697520 bytes). Actual companion requirements were
not exercised. Baseline historical certification remains unchanged.

## Desktop prerequisite and admission evidence

Source inspected at the starting HEAD:

- `crates/rah-runtime-codex/src/lib.rs`: current admission contains only exact
  codex-cli 0.157.1; preferred version is unchanged.
- `src/process.rs`, ProcessTransport::start: resolve executable, verify_version,
  verify_schema, then spawn app-server. check_version returns VersionMismatch
  for a successful --version output outside that exact set.
- `src/experimental.rs`, CodexFactory::create: real executable creation uses
  ProcessTransport::start. Its cfg(test) fixture seam supplies a fake transport,
  not a candidate process with narrowly authorized admission.
- `crates/rah-desktop/src/codex_composition.rs`: test resolver/factory seams do
  not supply a real candidate admission policy. Replacing the runtime factory
  with a fake would not certify the real neutral Desktop/runtime composition.
- Public legacy CodexRuntime connect paths also call ProcessTransport::start.

Thus source inspection establishes rejection before app-server startup and
therefore before inference or Tool execution. This is not a newly executed
production rejection probe. No production admission changes or bypass occurred.
Unknown 0.160.1/0.161.x and other unlisted versions remain rejected by the same
exact-set policy.

## Certification evidence ledger

| Required gate | Task 510B outcome |
| --- | --- |
| Protocol/schema audit: initialize, catalog, config, thread, turn, streaming, Tool, completion, cancellation, shutdown, errors | Not run after prerequisite stop; no compatibility conclusion |
| Deterministic adapter/model/catalog/neutral/preflight/admission/Tool/lifecycle suites | Not run; no current counts |
| Runtime-global catalog count and gpt-6.1-sol advertisement | Not measured here |
| Fresh direct gpt-6.1-sol turn | Not run |
| Authorized repo.status Tool round trip | Not run |
| Candidate cancellation/shutdown/process cleanup | Not run; no app-server started by this task |
| Task 498 diagnostic/typed-cause/sanitization regression | Not run |
| Real candidate Desktop Connect/catalog/preflight/turn/Disconnect | Blocked by missing certification-only path |
| Desktop repo authority/currentness/stale-handle/lifecycle checks | Not run |
| Workspace/canonical Desktop/frontend/static validation | Not run; documentation-only stop record |
| Tauri 47/47/47/47/47 | Starting Task 509 evidence only; no command-surface changes |
| 14 packages/members, all 0.33.0 | Starting checkpoint requirement; no manifests/dependencies changed; not re-executed |
| HostExplicit exactly 11, static/executable | Historical Task 509 evidence only; not re-executed |

Task 510A previously recorded 11 candidate advertised models, gpt-6.1-sol
present, provider-unbound model/list, a successful direct turn, and limited
schema checks. Those are historical research facts, not Task 510B certification.
Task 509's runtime-advertised/provider-unverified picker policy is unchanged;
no model-string special case, provider validation or hot-switch is introduced.
Disconnect remains the recomposition boundary.

## Authority, ADR and conclusion

Only this documentation file changed. ToolRegistry, repository authority and
switching, leases, permissions, Trusted Profiles, mutation uncertainty,
remembered workspaces, frontend, public APIs, dependencies, versions and
admission tables are untouched. No model/provider/version/thread metadata
authority was added. This is preservation by unchanged source, not new live
authority certification.

ADR result: **ADR-B — existing ADR sufficient**. ADR 0030 requires live Desktop
certification in addition to direct/schema/deterministic evidence. No new ADR.

The exact candidate matches the requested hash, but remains uncertified and
unadmitted. Recommend a separate bounded certification-harness task: an
explicit certification-only path bound to exact artifact identity, unavailable
to normal production admission and retaining all host authority checks. Then
repeat Task 510B's full evidence gates. Do not change the preferred baseline.

Task 510C — authorize and apply Codex 0.160.0 preferred/admission baseline update
is appropriate only after classification A and separate explicit authorization.
It is not the next executable step after this B disposition.

No commit, push, tag, release or new exact-head CI: the task authorizes publication
for a completed certification record, and certification stopped at B. This stop
record remains an uncommitted new file for review; final worktree is consequently
not clean. No production source or artifact was modified.

Reference-only follow-up: the construction seam implementation and deterministic
failure stop are recorded in [Task 510B2](2026-10-04-task-510b2-codex-cross-crate-certification-construction-seam.md).
Task 510B remains stopped; its historical classification and evidence are unchanged.

## Resumed certification — Task 510B after published 510B2

Starting HEAD and origin/master: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Remote master was freshly read with `git ls-remote origin refs/heads/master`
and matched. Initial worktree clean. Supplied Task 510B2 publication:
`test: add Codex runtime certification seam`, 32 files, classification
**A — CROSS-CRATE CERTIFICATION CONSTRUCTION SEAM VALIDATED**, supplied
exact-head CI `37208294157 — PASS`. That CI was not independently queried here.
This resumed section preserves, rather than replaces, the historical B stop.

Resume plan: verify snapshot/version; run deterministic adapter and neutral
regressions; run fresh catalog/direct turn; then serial Tool, cancellation,
diagnostic and Desktop phases with identity checks; only after live PASS run
full closure and consider publication. Production admission/baseline stay fixed.

### Exact artifact and baseline controls

Execution artifact only:
`F:\Temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex.exe`.
SHA-256 `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`;
Windows file ID `0x0000000000000000002e0000000a80a1`; length `326872368`.
PowerShell complete-file SHA, fsutil ID and file length matched before version,
after version, and after the failed Tool setup. Version: `codex-cli 0.160.0`,
exit 0. No snapshot recreation, live npm execution or PATH executable fallback.

The new explicitly ignored, feature-gated integration test also used
`measure_artifact`: complete read `326872368`, before/after lengths both
`326872368`, ID `12947848929378465` (the expected hexadecimal ID), expected
SHA, volume serial `2114581433`. The complete direct-turn measurement compared
equal before/after shutdown, including file timestamps and attributes.
The failed Tool phase exited before its in-test post-measurement; independent
PowerShell post-stop measurement matched SHA/ID/length.

Baseline was freshly hashed at
`C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe`:
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
Its production admission and preferred status were not changed.

### Executed evidence

Commands used `CARGO_TARGET_DIR=F:/temp/rah-task504-target` and matching
`RAH_TEST_TARGET_DIR`. Source was unchanged during each completed command.

| Gate | Fresh result |
| --- | --- |
| `cargo test -p rah-runtime-codex --all-features --lib` | 107 passed / 0 failed / 1 ignored; 108 discovered; exit 0; 59.26s test time |
| `cargo test -p rah-runtime --all-features --lib` | 8 passed / 0 failed / 0 ignored; exit 0 |
| `cargo test -p rah-runtime-codex --features certification-harness --test task510b_snapshot snapshot_catalog_and_direct_turn -- --ignored --exact --nocapture` | 1 passed / 0 failed; exit 0; 44.96s |
| `cargo test -p rah-runtime-codex --features certification-harness --test task510b_snapshot snapshot_tool_cancellation_diagnostic -- --ignored --exact --nocapture` | 0 passed / 1 failed / 1 filtered out; exit 1; 29.96s; stopped |

Adapter regressions include parsing, translation, catalog, host Tool mediation,
cancellation, shutdown and typed/sanitized errors. Neutral regressions include
factory/instance/conversation/stream, host port revocation, cancellation,
shutdown, retained handles and typed errors. These runs preceded the new live
test file; they are not full final-source workspace validation.

The direct phase used the sealed verified candidate factory, real Codex adapter,
neutral runtime and an empty host-owned ToolRegistry/HostToolScope. Snapshot
app-server spawn evidence: PID `12108`, exact supplied snapshot path. Fresh
runtime catalog: **10 models**, complete, `gpt-6.1-sol` present:
`gpt-6.1-sol`, `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-reserve`,
`gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`, `codex-auto-review`.
This is runtime-advertised/provider-unverified. Neutral discovery does not
expose hidden/deprecated flags; none were freshly audited.

Fresh direct conversation selected `gpt-6.1-sol` with OpenAi adapter provider.
Prompt: `Reply with exactly RAH510B_OK. Do not use any tools.`
Session `ff79a6ae-e54f-4101-a5ec-212770aa06ad`, request
`d5897769-74c6-482a-9c20-216ce3c2059a`. Event sequence:
Started → ModelRequestStarted → five ModelDelta events (`RA`, `H`, `510`, `B`,
`_OK`) → Completed, exact final `RAH510B_OK`. Tool executions 0. Clean explicit
shutdown returned Ok, runtime alive=false, complete post-measurement equal.
This is compatibility evidence only for the tested model/provider context.

Schema generation/contract validation and initialize/handshake succeeded on
the exact artifact as part of real factory creation. Model/list, restricted
configuration/thread/start, turn/start, streaming and completion succeeded.
No parser change. Full field-by-field comparison to 0.157.1, Tool request,
cancellation and live errors were not completed; no full protocol certification.

### Stop — D — TOOL / CANCELLATION / DIAGNOSTIC CERTIFICATION FAILED

The second live probe launched exact snapshot PID `18776`, then failed during
host Tool construction at `tests/task510b_snapshot.rs:71:76`:

```text
called `Result::unwrap()` on an `Err` value: Execution { message: "Git repository policy rejected capability: repository Git layout identity validation failed" }
```

The harness used `std::env::current_dir()` under Cargo's package test context,
so its proposed repository root was the package directory rather than the
repository root. `RepositoryStatusTool::new` rejected that layout before
registration, HostToolScope creation, conversation creation or turn submission.
Repository layout capture requires the approved root's `.git` entry. This is
a harness setup defect, not evidence of a candidate protocol/provider defect.
Tool requests/executions/results **0/0/0**; no Tool certification turn started.
No retry, source correction or later live phase after the failure.

The runtime was dropped during panic; explicit shutdown/post-check code was
not reached in this phase. Independent final process inventory found no Codex
process executing the snapshot path (including PIDs 12108 and 18776).
This proves no lingering snapshot process at final observation, not a successful
explicit shutdown result for the failed phase.

| Remaining requirement | Disposition at stop |
| --- | --- |
| Authorized Tool round trip | Failed at harness host construction; not certified |
| Cancellation and executable lease withdrawal | Not run live |
| Live sanitized diagnostic / typed cause | Not run; host ToolError above is not that probe |
| Real Desktop turn / picker / stale handles / repo lifecycle | Not run |
| Fresh no-feature production 0.160.0 rejection | Not run; deterministic production separation test passed, not substituted for this control |
| Full fmt/check/test/clippy closure | Not run after live stop; cargo fmt was executed before probes |
| Canonical Windows Desktop / complete frontend-static | Not run |
| Tauri 47/47/47/47/47 | Not remeasured; no IPC source changed |
| Metadata 14 / 0.33.0 / edition 2024 | Not remeasured; no manifests changed |
| HostExplicit exactly 11 static/executable | Not remeasured; no authority implementation changed |
| Cargo.lock | Unchanged; git diff empty |
| git diff --check | PASS at closure |

Authority/security conclusion: production implementation is unchanged.
No edits to ToolRegistry authorization, active-repo authority/switching, leases,
permissions, Trusted Profiles, uncertainty, remembered workspaces,
provider/model authority or runtime admission. The rejected construction
demonstrates fail-closed root validation for this invalid setup. It does not
replace the unrun authority certification gates. No dependency/public API/ADR
change. **ADR-B — existing ADR sufficient**, under ADR 0030 and existing
host-lifetime/typed-error boundaries.

Final resumed classification: **D — TOOL / CANCELLATION / DIAGNOSTIC CERTIFICATION FAILED**.
Exact snapshot remains uncertified and unadmitted. Task 510C is not authorized
by this result and must not begin. Recommended prerequisite: separately
authorize a narrow harness correction to bind an explicit approved repository
root and construct host prerequisites before spawning, preserving this failed
source/evidence; then resume outstanding certification gates.

Changes: this report and new ignored certification integration test. No commit,
push, new CI, tag, release or version bump because classification A was not
achieved. HEAD and origin/master remain the starting checkpoint; worktree
intentionally dirty with the report and test preserved.

## Task 510B-R1 — repository-root correction and Tool resumption

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Initial worktree contained exactly the resumed report and untracked
`crates/rah-runtime-codex/tests/task510b_snapshot.rs`. Both historical stops
above remain preserved. Prior direct/catalog and deterministic evidence was
not repeated or promoted to final-source closure.

### Narrow harness correction and host proof

The erroneous package path was
`F:\coding\otherPrj\rust-agent-harness\crates\rah-runtime-codex`.
The old source actually used Cargo's package-context `current_dir()`, rather
than an explicit repository binding. Audit found the existing convention in
`crates/rah-runtime-codex/tests/architecture.rs`: two ancestors above
`CARGO_MANIFEST_DIR`. R1 reuses that convention, canonicalizes the resulting
root, checks workspace manifest metadata and the adapter manifest, and accepts
either a file or directory `.git` entry. It does not discover another root
through Git output, depend on process cwd, or hardcode the repository path.
The existing architecture helper is private to its separate integration-test
binary; the certification test mirrors its layout convention with added proof.

Selected authority root:
`F:\coding\otherPrj\rust-agent-harness` (canonical Windows extended path).
`RepositoryStatusTool::new(git, root)` succeeded through unchanged host
Git-layout validation before candidate factory creation or app-server startup.
The same host registry construction asserted exactly one definition:
`repo.status`. No certification-only Tool or union registry was added; one
host-selected repository supplies the only executable registry, with Execute
permission through `HostToolScope` and its normal authorization path.
No inactive repository registry is constructed. Linked-worktree `.git` file
compatibility is preserved by the selection check and existing host validation;
no fresh linked-worktree live test was run.

Focused deterministic command:
`cargo test -p rah-runtime-codex --features certification-harness --test task510b_snapshot repository_host_setup -- --exact --nocapture`:
**1 passed / 0 failed / 4 filtered out**, exit 0, 0.08s.
This executes under Cargo's package test context and proves independence from
repository-root process cwd. The live phases were split into separately
invoked ignored tests to preserve serial stop boundaries.

### Fresh Tool turn — failed after valid host setup

Snapshot precheck matched the required full SHA, file ID and length. Fresh
snapshot-backed OpenAi adapter conversation selected `gpt-6.1-sol`.
Command:
`cargo test -p rah-runtime-codex --features certification-harness --test task510b_snapshot snapshot_tool -- --ignored --exact --nocapture`.
Result: **0 passed / 1 failed / 4 filtered out**, exit 101, 41.41s.
Snapshot app-server PID: `18312`.
Session: `146b2f81-9053-4575-be8b-514264b49396`.
Request: `ebd32663-f38f-431d-a1d4-5e111a38bb9a`.
Model request: `b9282dab-4441-4316-83f3-acfdf006caa0`.

Observed flow: Started → ModelRequestStarted → ModelDelta events → Completed.
Final text: `I’ll call \`repo.status\` once with \`{}\`.RAH510B_TOOL_OK`.
There were **zero ToolRequested, zero ToolStarted, zero ToolFinished** events:
requests/executions/results **0/0/0**, versus required **1/1/1**.
The count assertion failed at `task510b_snapshot.rs:226`. A textual claim and
success marker do not establish Tool use. No request entered the neutral host
Tool port; no dispatch authorization decision, execution or result was
observed. Therefore repository result sanity was not reached. This is fresh
failure after successful host setup, distinct from the historical invalid-root
failure. It does not by itself diagnose whether advertisement, adapter request
translation, upstream behavior or model choice caused the missing call.

Stopped immediately; no retry or later certification phase. The assertion
panic prevented the explicit runtime shutdown/post-measurement block from
being reached. Independent final process inventory found **no process running
the snapshot path**. This establishes exit at observation, not successful
explicit shutdown. Failed source and logs were preserved without subsequent
harness edits in `F:\Temp\rah-task510br1-evidence`:
`task510b_snapshot-before.rs`, `report-before.md`, `host-setup.log`, `tool.log`,
and `task510b_snapshot-failed.rs`.

### Identity, closure and disposition

Independent post-stop measurement:
SHA-256 `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`;
file ID `0x0000000000000000002e0000000a80a1`; length `326872368`.
All match the precheck. Only the verified snapshot was executed.
Fresh baseline hash:
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`;
0.157.1 certification/admission/preference unchanged.

| Outstanding gate | R1 outcome |
| --- | --- |
| Cancellation | Not run after Tool failure |
| Live diagnostic/error envelope | Not run |
| Real Desktop turn | Not run |
| Fresh production/no-feature rejection control | Not run |
| Full protocol audit | Incomplete; Tool request/result and later paths unproven |
| Workspace check/test/Clippy | Not run after live stop |
| Canonical Desktop / frontend-static | Not run; no fresh counts |
| Tauri 47/47/47/47/47 | Not remeasured |
| Metadata 14 packages / 0.33.0 / edition 2024 | Not remeasured; manifests unchanged |
| HostExplicit 11 static/executable | No change; executable closure not rerun |
| Cargo.lock / dependency drift | Unchanged; no dependency edits |
| cargo fmt --check / git diff --check | PASS |

Diff guard: changes confined to this report and the Task 510B integration
harness. Production repository Tools, Git-layout validation, ToolRegistry,
authority policy and admission are unchanged. No public API or crate dependency
edge changed. Runtime version/model/provider grants no authority. HostExplicit
remains conceptually exactly 11, without a new executable closure claim.
**ADR-B — ADR 0030 remains sufficient**; no new trust requirement established.

Final classification:
**B — TOOL ROUND TRIP FAILS AFTER VALID HOST SETUP**.
No commit, push, new CI, tag, release or version bump. HEAD remains the starting
checkpoint; the worktree intentionally remains dirty with the two certification
files. Supplied `37208294157 — PASS` is the historical 510B2 exact-head CI, not
new R1 certification CI. Task 510C must not begin. Recommended next task:
bounded investigation of missing Tool requests using the preserved failed
source/event evidence, without broadening authority or production admission.

## Task 510B-R2 � raw Tool-request path diagnosis

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
The original report and untracked harness WIP were preserved before edits in
`F:\Temp\rah-task510br2-evidence\report-before.md` and `harness-before.rs`.
All historical B/D/B stops above remain unchanged.

### Observation seam and bounded method

Existing `FakeTransport`/`FakePeer` captures wire values for deterministic tests;
`AppServerConnection::subscribe` is after parsing and omits raw server requests.
Existing bridge live evidence is not a complete pre-translation wire observer.
R2 adds an explicit, in-memory `ProtocolCapture` and transport wrapper only under
`certification-harness`, attached to `VerifiedCertificationCandidate`. Successful
outgoing sends and all incoming JSON values are copied before connection parsing.
No production logging, frontend diagnostic, parser, admission, permission,
ToolRegistry, repository-root, model-picker or preferred-baseline change.
No dependency edge or ADR change. HostExplicit remains unchanged.

The same integration harness, manifest-ancestor repository root, sole
`repo.status` registry, Execute-authorized `HostToolScope`, counters and exact R1
prompt were used once for each runtime, serially. No retries. Prompt:

```text
Call repo.status exactly once with {}. Use only the available RAH Tool. After the result, reply RAH510B_TOOL_OK. Do not call any other tool.
```

The candidate advertised `gpt-6.1-sol`, which was selected. The exact 0.157.1
catalog did not advertise it; the control used advertised `gpt-6-luna`, the
existing known ordinary-Tool control model. This is **not a same-model A/B**.
The control executable was
`C:\Users\morefunfun\AppData\Local\codex-baselines\0.157.1\codex.exe`,
SHA-256 `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
Its measured length was `322515248`, file ID `10414574138354066`; pre/post
measurements were equal. Baseline admission/bundle state was not altered.

### Outgoing advertisement comparison

Tools are advertised in the exact **thread/start** preceding **turn/start**.
Absence of a duplicate Tool declaration in turn/start is not H1.

| Field | 0.160.0 | 0.157.1 |
| --- | --- | --- |
| Advertised Tool count | 1 | 1 |
| Runtime name | `rah_tool_0` | `rah_tool_0` |
| Public name in description | `repo.status` | `repo.status` |
| Description | `RAH public tool `repo.status`. Reports normalized read-only Git status for one host-authorized repository.` | Identical |
| Type / loading | `function` / `deferLoading:false` | Identical |
| inputSchema | `{"additionalProperties":false,"properties":{},"type":"object"}` | Identical |
| Tool choice / required mode | Absent | Absent |
| thread/start model | `gpt-6.1-sol` | `gpt-6-luna` |
| modelProvider | `openai` | `openai` |
| cwd | canonical `F:\coding\otherPrj\rust-agent-harness` | Identical |
| approvalPolicy / sandbox | `never` / `read-only` | Identical |

`thread/start` config is identical: shell_tool/unified_exec/memories false,
web_search/view_image false, apps default disabled, empty mcp_servers,
serviceName rah-runtime-codex. `turn/start` has the same user-prefixed text input,
approvalPolicy never and sandboxPolicy `{"type":"readOnly"}`; only generated
threadId differs. There is no turn/start model override. Tool declarations are
byte-equivalent as JSON values; no Tool-advertisement field differs.

Current RAH source has no toolChoice/tool_mode forcing mechanism. The retained
0.157.1 and 0.160.0 `v2/TurnStartParams.json` schemas in Task 509D/510A evidence
also have no top-level Tool-choice/required field. No new forcing capability was
introduced or used.

### Raw incoming and adapter translation audit

Full private JSON captures:
`F:\Temp\rah-task510br2-evidence\0.160.0-original-wire.json` and
`0.157.1-original-wire.json`; matching `.log` files preserve neutral events.
Wire response envelopes are retained as well as notifications and requests.

0.160.0 had **zero** server `item/tool/call` requests, zero dynamicToolCall items,
and no other incoming event carrying function/Tool-call semantics. Item types
were userMessage, agentMessage and reasoning only. It completed normally.
Assistant text claimed a call; this was not counted as Tool evidence.

0.157.1 had one dynamicToolCall item and this exact server request:

```json
{"id":0,"method":"item/tool/call","params":{"arguments":{},"callId":"exec-d5999f5c-7943-4a71-96c4-a1b4ef367cfd","namespace":null,"threadId":"01a1075d-2778-7633-a19b-a7b2073942c6","tool":"rah_tool_0","turnId":"01a1075d-2812-7a42-829e-dfef8cbd2b01"}}
```

This matches the existing router schema: method item/tool/call; threadId,
turnId, tool, nonempty callId, arguments, namespace absent/null. Alias lookup
mapped rah_tool_0 to repo.status. Neutral ToolRequested carried `{}` and the
expected public name. No unknown Tool event was found, so H3 is unsupported.

| Raw method/item family observed | Existing mapping |
| --- | --- |
| JSON-RPC result envelopes | Correlated request responses; not neutral events |
| item/agentMessage/delta (0.160.0 only) | ModelDelta text |
| turn/completed, status completed (0.160.0 only) | Completed |
| item/tool/call (0.157.1 only) | Router alias validation then host request_live |
| item/started/completed: userMessage, agentMessage, reasoning | Ignored additive lifecycle notifications |
| item/started: dynamicToolCall (0.157.1 only) | Allowed bridge item; request is handled separately |
| thread/started, thread/status/changed, turn/started | Ignored additive notifications |
| thread/tokenUsage/updated, account/updated, account/rateLimits/updated | Ignored additive notifications |
| remoteControl/status/changed, mcpServer/startupStatus/updated | Ignored status notifications, not Tool calls |
| configWarning, warning | Ignored private provider warnings; not neutral RuntimeDiagnostic |

Both runtimes warned that tools.view_image is ignored. **Only 0.160.0** also
warned that Code Mode was unavailable because its snapshot sibling
`codex-code-mode-host.exe` was missing, and Code Mode would fail closed. This
is concrete runtime prerequisite evidence, not proof that this caused the
missing Tool request. Do not characterize the difference as a model-only defect
or general version regression. No companion was copied, configured or run.

### Host counters and control failure

| Runtime / model | Advertised | Raw calls | Neutral requests | Initial authorization attempts | Execution starts | ToolFinished results | Raw result replies to Codex | Final completion |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.160.0 / gpt-6.1-sol | 1 | 0 | 0 | 0 | 0 | 0 | 0 | completed |
| 0.157.1 / gpt-6-luna | 1 | 1 | 1 | 1 accepted | 1 | 0 | 1 failure reply | not observed; host Tool failure |

Authorization attempts count the initial host dispatch check immediately after
ToolRequested; authorized_tool_dispatch has an additional unchanged dispatch
revalidation. ToolStarted proves the initial check accepted Execute-authorized
repo.status. Execution count is lifecycle start, not proof a Git child completed.
No unexpected authorization denial was observed.

The control subsequently emitted neutral Failed with code Tool and sanitized
`runtime Turn failed (Operation)`. The adapter returned
`{"contentItems":[{"text":"host Tool request failed","type":"inputText"}],"success":false}`
to Codex. No successful ToolOutput or repository result sanity proof exists.
The precise underlying host execution cause was not captured by this harness;
do not infer timeout, Git failure or a bridge regression from the sanitized error.
This is a separate host-execution prerequisite failure after a real request,
not evidence that the candidate's missing request was lost in host dispatch.
The failed source is frozen in `harness-failed.rs`; no correction or retry followed.
The control's count summary line was bypassed by its failure; counts above are
reconstructed exactly from retained neutral lifecycle events and raw reply.

H4 is not supported because the original prompt produced a real control request.
No stronger-prompt turn was run. H5's known-valid forcing/control failure condition
was not established; the control reached the request bridge and authorization.
This control is not a successful complete Tool round trip or certification.

### Identity, process cleanup and focused validation

0.160.0 pre/post measurements were equal:
SHA-256 `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`,
file ID `0x0000000000000000002e0000000a80a1`, length `326872368`.
App-server PID `17940`. Explicit shutdown `Ok(Ok(()))`, alive false.
Final CIM inventory found no process at the snapshot path, and no process at the
exact baseline path after its explicit successful shutdown. Both executable
identities were unchanged. No unrelated process was terminated.

Commands used `CARGO_TARGET_DIR=F:\Temp\rah-task510br2-target`:

- Focused observer unit test `observer_preserves_both_wire_directions`:
  1 passed / 0 failed / 108 filtered, exit 0.
- Ignored integration test `tool_path_diagnosis` with RAH_R2_VERSION=0.160.0:
  1 passed / 0 failed / 5 filtered, 48.86s, exit 0. Diagnosis capture success
  does not mean Tool round-trip success.
- Same integration test with RAH_R2_VERSION=0.157.1:
  0 passed / 1 failed / 5 filtered, 124.83s, exit 101; host Tool failure.
  Explicit shutdown and identity checks completed before the final assertion.
- cargo fmt --check and git diff --check: PASS.

No full workspace/Desktop/frontend closure, cancellation certification,
error-envelope certification, admission, commit, push or new CI was performed.
The code changes are confined to the certification observer wrapper and harness;
this report records the resulting failure without patching its cause.

### Root-cause disposition and next task

**B � H2: 0.160.0 RECEIVED TOOL ADVERTISEMENT BUT EMITTED NO TOOL REQUEST**.

The missing interaction is between valid outgoing Codex Tool advertisement and
incoming raw Tool request. Equivalent Tool declarations produced a real request
under 0.157.1/gpt-6-luna; 0.160.0/gpt-6.1-sol produced none. This supports scoped
runtime/model/provider Tool-use incompatibility for this snapshot certification
path, not an adapter translation failure. The different models and candidate's
missing-code-mode-host warning limit causal attribution. Candidate remains
unadmitted. Task 510B stays stopped.

Recommended next task: **Task 510B-R3 � bounded prerequisite diagnosis of the
0.160.0 missing code-mode companion warning and the separate 0.157.1 host
repo.status execution failure**, starting from retained raw evidence, with no
production authority/admission change or live retry until prerequisites and
scope are explicitly defined. This recommendation is not authorization.
No automatic cancellation, diagnostics, Desktop or Task 510C continuation.
HEAD remains the starting checkpoint; WIP/evidence remain unpublished.

## Task 510B-R3 — prerequisite diagnosis (2026-10-05)

Starting/final HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Historical stops and starting dirty WIP preserved. Evidence:
`F:\Temp\rah-task510br3-evidence`, including `harness-before.rs`,
`report-before.md`, `harness-failed.rs`, `host-only.log`, exact warning,
package manifests, narrow upstream source, and secondary Git result.

### Exact companion warning and phase

R2 wire entry 13, incoming warning emittedAtMs 1791124826910:

```text
Code Mode is unavailable because failed to spawn code-mode host F:\Temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex-code-mode-host.exe: host executable was not found. Code mode will fail closed; enable `features.code_mode_host` and install `codex-code-mode-host`.
```

After outgoing `turn/start` entry 11, before inference; a Tool-enabled turn
warning, not startup/thread creation. Earlier tools.view_image warnings are
separate. Source availability checks is_file(); wording "failed to spawn"
does not prove an attempted child launch. Exact envelope: `exact-warning.json`.

### Installed distribution vs snapshot

Platform package P:
`C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64`.
Executable directory B: `P\vendor\x86_64-pc-windows-msvc\bin`.
Paths below are relative to B:

| Component | Live npm distribution | Current snapshot directory |
| --- | --- | --- |
| codex.exe | 326872368 bytes; matching candidate SHA | Present, same SHA/size |
| codex-code-mode-host.exe | 74697520 bytes | Absent |
| ..\codex-package.json | 215 bytes, layout/version metadata | Absent |
| ..\codex-path\rg.exe | 4218880 bytes, search support | Absent |
| ..\codex-resources\codex-command-runner.exe | 8207152 bytes, process support | Absent |
| ..\codex-resources\codex-windows-sandbox-setup.exe | 17685808 bytes, Windows sandbox support | Absent |

Other voice resources are outside the warning path and were not hashed.
No claim that all inventory members are required for the RAH lane.
The snapshot still contains only codex.exe; no files added.

Companion exact SHA-256:
`1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6`.
Local package membership, not filename, establishes provenance: platform
package.json (511 bytes) identifies @openai/codex 0.160.0-win32-x64,
Windows/x64 and files:[vendor]. Launcher package.json (1082 bytes) identifies
0.160.0 and exact optional alias
`@openai/codex-win32-x64: npm:@openai/codex@0.160.0-win32-x64`.
Retained codex-package.json records version 0.160.0, target
x86_64-pc-windows-msvc, layoutVersion 1, entrypoint bin/codex.exe,
resourcesDir codex-resources, pathDir codex-path.
No signed package or independent published checksum attestation is claimed.
PE FileVersion/ProductVersion absent. Source Cargo.toml uses version.workspace;
main embeds CARGO_PKG_VERSION for telemetry. Clap main has no version option;
no unsupported --version or operational command was executed. Coupling is
package-level; independent binary-reported version/checksum coupling unproven.

### Companion resolution and necessity

Official OpenAI documentation search/open did not establish version-specific
behavior. Read-only primary upstream source fetched at rust-v0.160.0, resolved
commit/tree `a956835d020762cb2b570053af06f643a11c0ecc`, retained privately.
No downloaded source installed/built. Source references:

- install-context/src/lib.rs:176-206 prefers package/standalone
  codex-resources companion, otherwise package bin/standalone release directory,
  then current_exe parent plus exact Windows filename. Snapshot takes sibling
  path. No companion PATH search or companion-path environment override here;
  install-manager flags classify installation method. App-server supports
  explicit remote --code-mode-host URL; none supplied in R2.
- code-mode/src/remote_session.rs:56-74 default provider uses InstallContext;
  missing is_file() causes unavailable before operational spawn.
- core/src/session/turn_context.rs:1300-1338 emits warning for requested
  CodeMode/CodeModeOnly with unavailable service.
- core/src/tools/code_mode/mod.rs:104-121 chooses exact "fail closed" suffix
  for effective CodeMode/CodeModeOnly; Direct says "Falling back to direct tools".
  Thus R2 warning itself proves effective mode was not Direct.
- core/src/tools/mod.rs:75-99 lets model metadata choose Tool mode;
  unavailable CodeMode can fall back, but CodeModeOnly does not.
- core/src/tools/spec_plan.rs:174,569,794-805,820-930,1426 puts dynamic Tools in
  registry, hides nested code-mode-capable Tools from direct model exposure in
  CodeModeOnly, exposes them through code-mode execution. handlers/dynamic.rs:
  48-88 marks non-deferred function Direct exposure (also code-mode-capable),
  not a direct-only exception.

Current local models_cache client_version 0.160.0, fetched_at
2026-10-05T11:18:14.576802300Z, marks gpt-6.1-sol code_mode_only; selected
metadata retained. This later cache is corroboration, not recovered R2 metadata.
R2 model/list did not expose tool_mode. Source correspondence is not binary
reproducibility proof; exact warning/path/mode strongly establish prerequisite.

**C1 — REQUIRED COMPANION ABSENT FROM EXE-ONLY SNAPSHOT.** Deployment-incomplete
for the active Tool/code-mode path. Does not prove a complete bundle would emit
a request or that 0.160.0 is Tool-incompatible. Future certification boundary
needs exact verified runtime bundle; no bundle implemented in R3.

### Exact historical host evidence and direct dispatch

R2 retained RuntimeFailure operation Turn, kind Operation, rpc_code None;
Failed code Tool, message `runtime Turn failed (Operation)`; runtime reply:

```json
{"contentItems":[{"text":"host Tool request failed","type":"inputText"}],"success":false}
```

R2 did not retain process-local source chain, child output/status or exact stage.
Those cannot be recovered from sanitized evidence. R3 does not misattribute
new typed evidence to the historical invocation. No Codex retry ran.

Added diagnostic `r3_host_only_status` in existing harness. Same manifest-root
host_registry, normal Git-layout validation, ToolRegistry, Execute-only
HostToolScope, admitted lease and request_live(repo.status,{}) as R2, without
Codex. Normal authorization/dispatch revalidation retained. Lease active until
result; then revoke/drain. No separate active-repo identifier/generation field
exists in this harness; registry owns one bound repository.

Host-only result FAIL: 0 passed / 1 failed / 6 filtered, 163.64s. Exact new chain:

```text
RuntimeFailure: operation=Turn, kind=Operation, rpc_code=None
AuthorizedDispatchError::Tool(ToolError::Execution {
 message: "Git repository policy rejected capability: repository observation exceeded its total timeout"
})
display: authorized tool dispatch execution failed: tool execution failed: Git repository policy rejected capability: repository observation exceeded its total timeout
source: ToolError::Execution, same message
display: tool execution failed: Git repository policy rejected capability: repository observation exceeded its total timeout
```

One ToolRequested, one ToolStarted, zero ToolFinished, one Failed code Tool.
Accepted authorization; downstream execution failed. Frozen executed source
harness-failed.rs preserved before adding ignore annotation so real-repository
failing diagnostic does not enter normal tests. No failure-cause correction.

### Git/environment and exact failure layer

Resolved Git: `C:\Program Files\Git\cmd\git.exe` (first where.exe hit).
Process cwd: `F:\coding\otherPrj\rust-agent-harness\crates\rah-runtime-codex`.
Authority/child root: `\\?\F:\coding\otherPrj\rust-agent-harness`.
Arguments {}; Execute-only snapshot; active lease. Full parent PATH retained
in host-only.log, includes Git cmd; unchanged. Children use absolute Git and
env_clear, so inherited PATH does not resolve their program.
Child environment: GIT_CONFIG_NOSYSTEM=1, GIT_CONFIG_GLOBAL=NUL,
GIT_CONFIG_COUNT=3, keys 0/1 core.fsmonitor/core.untrackedCache false,
key 2 safe.directory exact canonical root, GIT_OPTIONAL_LOCKS=0,
GIT_TERMINAL_PROMPT=0. No child PATH inserted; no global PATH change.

Source: repository_status.rs enters observer run_with_budget; first normal
layout revalidation uses eight fixed Git probes, then whole-tree nested boundary
validation, then remaining STATUS_TIMEOUT=10s check. Boundary traversal includes
ordinary ignored directories, including target; no Git-ignore pruning.
The exact error is observer child_timeout's zero-budget branch. Status has no
aggregate probe budget, so layout-probe timeout cannot emit this total-budget
message. Pre-status validation exhausted status budget; final status command
was NOT launched. Earlier layout Git probes did launch successfully; their
individual output/exit values were not logged. No final status child exit,
stdout or stderr exists for the failing dispatch. Per-stage timings/subtree
were not measured; target attribution remains inference. Proven root cause:
exhausted pre-status observation budget, independent of Codex.

Secondary localization only, bypassing Tool dispatch/boundary scan: absolute
Git with equivalent cleared child environment and exact root, command
`--no-pager status --porcelain=v2 -z --untracked-files=normal --ignored=no --no-renames --ignore-submodules=all`
returned exit 0, empty stderr, four valid dirty entries in 0.0514823s.
Retained secondary-git.json. Python attempt did not execute (unavailable on
PATH); actual control used native PowerShell ProcessStartInfo.

Unchanged fixture test mixed_states_untracked_normal_and_read_only_invariant_are_normalized
PASS 1/0, 3 filtered, 1.59s. Lower-level Tool fixture, not exact HostToolScope
path. Dirty status valid; failure assertion followed real execution error.
No global Tool regression or dirty-result interpretation error established.

**H2 — HOST TOOL EXECUTION FAILS IDENTICALLY WITHOUT CODEX**, in observable
accepted/start/Failed pattern; equality of unrecorded R2 typed cause remains
unprovable. Independent host execution prerequisite fails on actual repo.
Do not relax nested-boundary policy or permissions to repair it.

### Validation, identity and disposition

CARGO_TARGET_DIR=F:\Temp\rah-task510br2-target. Host-only focused test FAIL
(real prerequisite failure, not certification PASS); unchanged fixture PASS.
cargo fmt --check and git diff --check PASS. No full closure suite.

Snapshot original path remains
F:\Temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex.exe;
fresh SHA fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d,
length 326872368, file ID 0x0000000000000000002e0000000a80a1.
Live source exe matches that SHA. Baseline original path remains
C:\Users\morefunfun\AppData\Local\codex-baselines\0.157.1\codex.exe;
fresh SHA 8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574,
length 322515248. gpt-6-luna control is not a same-model comparison.
Neither runtime executed during R3; no companion execution.

**D — TWO DISTINCT DEFECTS PROVEN.** Required companion absent from snapshot,
plus independent host pre-status budget failure. No general 0.160.0 Tool
incompatibility conclusion. Separate bounded corrections required.

Exact recommended next task: **Task 510B-R4 — design an exact verified 0.160.0
runtime-bundle snapshot in a new artifact directory, and localize/correct host
repo.status pre-status validation budget failure while preserving nested-boundary
policy and all authority; prove deterministic host dispatch before separately
authorizing a bundle Tool-only certification turn.** Define bundle members,
provenance, hashes, copy algorithm, layout, pre/post identity and execution proof.
This recommendation does not automatically authorize or start corrections.
Task 510B remains stopped; Task 510C remains blocked. No cancellation,
diagnostic envelope, Desktop, production rejection or workspace closure resumed.

R3 edits only diagnostic harness and this report; R2 source WIP preserved.
No production admission, dependency, ADR, authority, ToolRegistry policy,
HostExplicit, Cargo.lock or host configuration change. Four starting dirty/
untracked paths remain. No commit, push, CI, tag, release or version bump.
Report append preserves pre-existing bytes (file already contained non-UTF8 bytes).

## Task 510B-R4 - independent prerequisite lanes (2026-10-05)

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Preserved starting WIP: certification_support.rs, experimental.rs, this report,
and untracked task510b_snapshot.rs. Private evidence:
`F:\Temp\rah-task510br4-evidence`. Starting harness/report copies retained.
All historical stops above remain unchanged. No Task 510B commit/push/new CI.

### Lane A: membership audit and provenance

Re-inventoried the installed platform package and copied current platform,
launcher and runtime manifests into evidence. Local provenance:
`@openai/codex-win32-x64` alias of `@openai/codex@0.160.0-win32-x64`, launcher
0.160.0, runtime manifest 0.160.0, x86_64-pc-windows-msvc, layoutVersion 1.
No signed-package or independent published-checksum attestation is claimed.
Complete candidate inventory: installed-inventory.json.

| Class | Candidate | Decision |
| --- | --- | --- |
| M1 | bin/codex.exe | Include: exact app-server entrypoint |
| M1 | bin/codex-code-mode-host.exe | Include: proven required code-mode availability sibling |
| M2 | launcher/package.json, platform/package.json, codex-package.json | Record outside bundle: provenance/layout only |
| M3 | codex-path/rg.exe | Exclude: search unused |
| M3 | codex-resources/codex-command-runner.exe | Exclude: shell/unified execution disabled |
| M3 | codex-resources/codex-windows-sandbox-setup.exe | Exclude: not required by this availability path |
| M3 | codex-resources/voice files/licenses | Exclude: voice unused |

M3 is scoped to this control, not all possible Codex operations. No additional
required runtime artifact was discovered; no entire npm package tree copied.

Fresh create-new bundle root:
`F:\Temp\rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c`.
Exactly two read-only sibling binaries. Structured certification-only
bundle-descriptor.json remains outside the root.

| Member | Source and bundle SHA256 | Length | Source file ID | Bundle file ID |
| --- | --- | ---: | --- | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x0000000000000000001d00000000b96d | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x00000000000000000045000000005924 | 0x000000000000000000180000000c1862 |

Canonical source paths are corresponding files under
`C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin`.
Descriptor retains canonical paths, per-file hashes/lengths/IDs, versions and
volume serials. Source volume 2929214197; bundle volume 2114581433.
Both source/copy file IDs differ; copies are distinct filesystem objects.

`r4_create_bundle` reused create_snapshot_at per member: one source handle,
identity capture, streamed hash/copy, complete read and stable-handle checks,
flush/sync/close, independent reopened destination hashes/identities. Exact
expected hashes checked; no hash(path)-then-File::copy trust boundary.
Test PASS 1/0, 53.08s; bundle-create.log. No aggregate replaces member hashes.
Independent PowerShell Get-FileHash SHA256 matched both exact hashes above;
powershell-hashes.json. Bundle codex.exe --version returned
`codex-cli 0.160.0`, exit 0; version.log. No companion version command.

### Lane A: deterministic prerequisite, resolution and fallback audit

`r4_bundle_prerequisite` verifies both bundle hashes, then starts its exact
codex.exe app-server. Isolated CODEX_HOME; empty child PATH; npm/bun/pnpm/VitePlus
managed-install hints and OPENAI_API_KEY removed. No remote code-mode-host URL.
Credential-free r4-local provider points at a closed loopback port. No RAH Tool
is supplied; shell/unified execution, memories, web search, apps and MCP are
disabled in requested thread configuration. Explicit code_mode_only and
code_mode_host test availability; no production settings or model policy change.

Audited retained rust-v0.160.0 source, install-context/src/lib.rs:176-206:
package resources first, then executable/package sibling. This bundle has no
package layout or resources directory; current_exe parent is its root, so the
exact availability path is that root plus codex-code-mode-host.exe. No companion
PATH search. remote_session.rs:56-74 performs is_file on the resolved host path;
turn_context.rs:1312-1338 emits the missing-host warning before inference when
code mode is requested but unavailable. Source/path audit plus binary preflight
is availability-resolution evidence, not an OS companion-spawn trace or
binary/source reproducibility attestation.

First harness reached turn/started, then loopback connection failure, without
the missing-host warning, but incorrectly waited for turn/completed. Provider
reconnection led to the 45s harness timeout: FAIL 0/1, 56.82s. Preserved separately:
harness-prerequisite-failed.rs, bundle-prerequisite-first-failed.log,
offline-protocol-first.json, offline-stderr-first.log. Its isolated logs confirm
failed POSTs to 127.0.0.1:55383/v1/responses; no successful model inference.

Bounded harness correction stops at the first connection failure after
turn/started, before reconnection. Changed-source control PASS 1/0, 17.25s;
bundle-prerequisite.log, offline-protocol.json, offline-stderr.log.
Prior host-executable-not-found warning absent; no item/tool/call. Other
configuration/unstable-feature warnings are retained. No live inference or
Tool round-trip occurred; no Tool certification claimed.

CIM prerequisite-processes.json captured main PID 4516 at the exact new bundle
path, app-server argv. No companion child of that PID spawned: remote_session
creates its connection/session lazily upon code-mode execution. Inventory also
contained existing npm companion PID 8800, parent 7976. Separate
unrelated-companion-parent.json identifies 7976 as an unrelated existing npm
Codex process, not the bundle parent. It was neither used nor terminated.
No npm companion or PATH fallback belongs to this control. Actual companion
execution remains untested until a later authorized Tool turn.

**A1 - VERIFIED RUNTIME BUNDLE VALIDATED**, at the requested pre-inference
availability/resolution boundary. Preserve new root/descriptor. Original
exe-only snapshot remains unchanged: **historical incomplete artifact**.

### Lane B: single pre-correction real host control

Opt-in RAH_R4_OBSERVATION_TIMING under existing live-test-support records
diagnostic phase durations/counters; no policy or budget choice changed.
Reused R3 ordinary HostToolScope, registry, Execute authorization and
request_live(repo.status,{}) on exact manifest-derived RAH repository root.
No Codex used. The single reproduction unexpectedly PASSED 1/0, 6.28s;
host-before.log. It did NOT reproduce R3 exhaustion. No retry loop.

Times are seconds relative to actual run_with_budget started Instant;
remaining is the unchanged 10s status budget.

| Actual phase | Start | Duration | Cumulative | Remaining |
| --- | ---: | ---: | ---: | ---: |
| revalidate | 0.0000006 | 0.0012839 | 0.0012845 | 9.9987155 |
| validate_git_with_budget | 0.0012845 | 0.3632942 | 0.3645787 | 9.6354213 |
| validate_observation | 0.3645787 | 5.7944037 | 6.1589824 | 3.8410176 |
| child_timeout | 6.1589824 | 0.0000603 | 6.1590427 | 3.8409573 |
| execute_process | 6.1590427 | 0.0542469 | 6.2132896 | 3.7867104 |

Each actual semantic Git probe was measured separately:

| Fixed argv | Duration ms | Aggregate passed |
| --- | ---: | --- |
| rev-parse --show-toplevel | 41.4235 | None |
| rev-parse --path-format=absolute --absolute-git-dir | 40.8955 | None |
| rev-parse --path-format=absolute --git-common-dir | 41.9607 | None |
| rev-parse --is-bare-repository | 39.4574 | None |
| rev-parse --show-superproject-working-tree | 54.6500 | None |
| rev-parse --path-format=absolute --git-path index | 41.4795 | None |
| rev-parse --path-format=absolute --git-path HEAD | 38.8423 | None |
| worktree list --porcelain -z | 57.7004 | None |

Probe durations include policy setup/execution. Per-probe parent-budget start
and cumulative values were not captured. Status passes no aggregate to probes;
their existing five-second ceiling remains. This source composition gap is
recorded, but this successful trace does not prove it caused R3 exhaustion.
Existing composed staged-diff/file-info helpers and tests were inspected.

Boundary traversal: 6274 directories, 109894 entries; internal total
5.7942691s, existing reject_nested_marker subtotal 393.1883ms. No ignored
tree, layout/marker check, linked-worktree validation or authority check skipped.
No subtree is proven responsible for R3's historical 163.64s bottleneck.
Authorization accepted, ToolRequested/ToolStarted/ToolFinished present; final
ordinary Git status launched and returned status ok, is_error=false with seven
valid dirty entries. Environment isolation and host-owned Git/root unchanged.

**Exact budget defect: NOT PROVEN.** No exhausted-budget trace in R4; R3
lacked phase timing. Genuine slow required work versus historical implementation
or environment effects cannot be distinguished. Do not infer B1-B7 causal
subclassification from the fast trace or eliminate marker work/increase timeout.
No production budget correction, post-correction timing or post-correction
real host control exists. Current success does not erase historical H2.

Focused existing tests PASS: repository_observer tests 6/0, layout timeout tests
4/0, boundary tests 13/0, repository_status integration 4/0, exact single-handle
snapshot/rejection fixture 1/0. These check existing composed budget helpers,
fail-before-spawn, linked observation, boundary rejection, dirty status and
environment isolation. They do not prove all six correction criteria or correct
the historical defect. No regression assertions for an unproven fix were added.

**B4 - HOST FAILURE REMAINS INDETERMINATE.** Environmental causation is not
proven either, so B2 would overstate evidence. Timeouts/fail-closed policy retained.

### Combined outcome and next task

**B - RUNTIME BUNDLE VALIDATED, HOST BLOCKER REMAINS**: Lane A A1, Lane B B4.
**Task 510B-R5 NOT AUTHORIZED**; both prerequisites have not passed.
Exact next task if separately authorized: bounded follow-up to capture the
unreproduced historical host timeout with complete phase/budget evidence.
No automatic repetition or timeout increase. R5 can be proposed only after B1.
Task 510C blocked. 0.160.0 remains unadmitted/uncertified; preferred 0.157.1.

R4 files: certification harness, diagnostic-only repository_observer.rs,
repository_git_layout.rs, repository_boundary.rs, live-test-support feature
description, and this report. Prior adapter WIP preserved. No dependency edge,
ToolRegistry/authorization, permissions, repository authority, HostExplicit,
model policy or admission change. **ADR-B**; no new trust boundary or new ADR.
No commit/push/new CI or full workspace/Desktop/frontend closure.

Final checks: cargo fmt --check PASS (exit 0); git diff --check PASS (exit 0).
Final independent bundle hashes and fsutil file IDs match descriptor; both
binaries remain read-only. Historical snapshot SHA/size/file ID unchanged;
its directory still contains only codex.exe. Historical report prefix verified
byte-for-byte against report-before.md. HEAD remains the starting SHA; WIP
preserved, no commit/push/new CI. Evidence: final-checks.json,
final-bundle-identities.json and historical-snapshot-final.json.

## Task 510B-R4H - host observation timeout evidence (2026-10-05)

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Existing R4 runtime bundle frozen; no Codex or Desktop turn, no Task 510C.
Evidence directory: `F:\Temp\rah-task510br4h-evidence`.
Starting WIP patch and report copy preserved. R4H adds only opt-in diagnostic
instrumentation, native fixture/test controls, and this report; no timeout,
retry, environment, probe, permission or authority policy correction.

Predeclared campaign: exactly six serial ordinary HostToolScope observations,
labels 1 through 6, against the current dirty RAH root. Each label runs once;
stop and retain evidence on failure or >=10s dispatch. A designated final
control is conditional on deterministic evidence supporting lane closure.
No passing run replaces failed evidence. All source is fixed while tests run.

### Historical timer and current timing scope

R3 `host-only.log` and frozen `harness-failed.rs` were read directly. `163.64s`
is libtest's **whole single-test run** elapsed report, excluding Cargo's preceding
3m02s build. It is not an explicit Instant around request_live. The retained
code includes Tokio test/runtime setup, cwd/root/PATH printing, where.exe Git
resolution, RepositoryStatusTool construction/captured layout identity, registry
and HostToolScope setup, turn admission, request_live, result/source printing,
three event reads (each with a 1s ceiling), revoke/drain, and the final failing
assertion. The log contains all three events, so three one-second event waits
cannot explain 153 extra seconds. Exact historical subphase start/end instants
were never recorded; they cannot be reconstructed. R2's missing source chain
is not reconstructed or attributed to R3.

R4's 6.2132896s starts at the Instant passed to observer.run, after status input
validation, repository lease acquisition and preliminary revalidation. It ends
after execute_process, before final repository revalidation, status parsing,
serialization, host result/event conversion, reply delivery and dispatch return.
Thus historical 163.64s and R4 6.21s are not equivalent timing scopes.

R4H starts a monotonic timer at ordinary request_live entry and brackets snapshot
lookup, reservation/currentness, ToolRequested, registry authorization, host task
scheduling, authorized_tool_dispatch's second ordinary authorization, Tool
execution, result conversion, reply delivery and return (including failure).
The status trace brackets lease wait/acquisition and observer start/end; prior
R4 phases retain revalidate/Git-validation/boundary/child-timeout/final-process
measurements. Sandbox diagnostics cover child execution entry, spawn/ownership,
wait completion, timeout detection, termination request/completion, reap, both
pipe joins, output conversion and child return. Absolute wall_ns markers allow
cross-layer reconciliation; monotonic elapsed values are the duration evidence.
No timing field enters Tool output or model-facing diagnostics.

The campaign-source dispatch timer started after printing dispatch_start, which
omitted that print's latency from dispatch_total. Sample 1's recorded entry/return
markers span 2.5488531s, versus the outer Instant's 2.5524664s: 3.6133ms remainder
for wrapper/terminal timing output. Its old dispatch_total=2.3662138s is explicitly
not substituted for external wall time. The final diagnostic source starts the
Instant before printing and records every child return, including error paths.
This instrumentation adjustment does not change timeout/process policy.

### Exact timeout/process audit

The repository observation deadline is represented as `(std::time::Instant,
Duration)` and/or `started` plus the command ceiling. Status supplies a 10s
ceiling with no aggregate probe budget. child_timeout uses saturating elapsed
subtraction, fails closed at zero, and supplies the remaining allowance to the
final fixed status policy; it does not give status a fresh 10s.

Important unresolved composition limits:

- Status acquires its asynchronous shared repository lease before starting this
  deadline. There is no queue-wait timeout in that layer.
- Status's eight semantic layout probes receive aggregate=None, so each has its
  own existing 5s child limit. They do not consume an enforced common deadline
  at their entry. This differs from staged-diff/file-info aggregate composition.
- Identity checks, executable/cwd policy checks and whole-tree nested-boundary
  validation use synchronous filesystem calls. Boundary validation has a
  100000-directory count limit, but no elapsed-deadline check or interruptible
  filesystem call. The elapsed time is charged only when child_timeout runs
  afterward. A Tokio timeout cannot preempt blocking work on its worker.
- execute_host_process builds the fixed cleared-environment command, spawns it
  synchronously and attaches process ownership before creating its Tokio timer.
  Spawn/ownership setup is not inside that timer.
- Success parsing/revalidation/serialization and outer host event/reply work are
  also outside the observer child timer.

Repository observation uses **execute_host_process**, not ProcessSandbox's
simpler process.output timeout path. Normal waiting selects child.wait against
Tokio sleep and bounded-output overflow. On timeout it calls Windows
TerminateJobObject, then child.start_kill (a termination request), then child.wait
under the existing **2s TERMINATION_GRACE**. TerminateJobObject is a synchronous
API call without an additional wrapper deadline; the implementation does not
wait indefinitely in wait/wait_with_output. It uses no wait_with_output.
Stdout/stderr drain concurrently in Tokio tasks, with bounded retained bytes.
Each subsequent join_reader has its own 2s grace and aborts that reader on expiry.
The conservative async wait envelope is child allowance plus 2s reap plus up to
2s per reader, excluding synchronous OS calls/spawn/scheduling. A first-reader
error returns early rather than waiting on the second; a dropped second handle
can leave a reader task unjoined, although it cannot hold dispatch return.
This audit does not claim all possible fault paths have confirmed task cleanup.

Windows attaches the direct child to a kill-on-close Job Object and terminates
the Job on timeout; inherited descendants can own the pipes. Attachment happens
after spawn and is best effort, not an OS sandbox. Therefore a native descendant
with inherited stdout/stderr is relevant and was tested. No generic process
redesign, timeout increase, retry, probe removal, environment inheritance or
authority correction was made.

### Native forced-timeout and pipe/tree controls

Reused `rah_execute_fixture` pid-delay/delay operations, existing process-liveness
helpers and existing descendant fixture. Added only `spawn-pipe-child`, sharing
the existing descendant operation with inherited output pipes. No shell timing
fixture. Observer control uses a test-owned initialized temporary Git repository
with an index, swaps only its private test status policy for the native fixture,
and backdates the test start by nine seconds. Its real layout checks consume
part of the one remaining second before the 30s slow child starts. This does not
alter production status construction or expose a configurable process Tool.

The first fixture attempt failed layout identity validation because its unborn
empty repository lacked an index. The failed source/log are preserved. Initial
host-control compilation also failed because execute_process is crate-private;
source/log retained, and the test was changed to the public HostExecutionTool
wrapper without widening that API. Both are diagnostic fixture defects, not
historical timeout reproductions.

Initial exact-observer control: PASS 1/0, allocated 655.7461ms after validation;
actual remaining-time control wall 1.0611071s, timed_out=true,
termination_attempted=true, child PID 9964. Initial ordinary authorized host
control: PASS 1/0, returned the expected typed Tool failure, 1.0157418s external,
1.0156093s instrumented, PID 14996. Job termination/reap markers are retained.
The observer's nominal total reached 10.0609429s because its process timer starts
after spawn and cleanup is additional; this small accounted overrun is not a
163-second cleanup hang or proof of a hard ten-second dispatch bound.

After diagnostic error-retention/timer-placement improvements, necessary
final-source deterministic controls were run separately; no real-repo sample
was repeated or replaced:

| Final-source control | Result | Actual wall | Termination API/request | Request complete to reap | Timeout to return |
| --- | --- | ---: | ---: | ---: | ---: |
| Exact private status observer, reduced remaining budget | PASS 1/0; timeout/termination true | 1.012073s | 0.2100ms | 1.2174ms | 1.5992ms (child return) |
| Ordinary HostToolScope test-owned slow observation-shaped Tool | PASS 1/0; dispatch failure expected | 1.016993s | 0.2066ms | 1.1495ms | 1.7000ms (dispatch return marker) |
| Windows descendant owning inherited stdout/stderr | PASS, in execute-policy 13/0 | 1.1012977s | 0.2211ms | 1.4646ms | See execute-final.log |
| Permanently pending output reader | PASS, in sandbox 12/0 | approximately 2.01s | not applicable | not applicable | reader aborted after existing 2s grace |

The host-shaped control uses ordinary registry lookup and Execute authorization;
it is explicitly a test capability, not a successful production repo.status
result. The separate private observer control tests the exact observer runner.
Existing status rejection maps timed_out output to failure. The descendant test
asserts its PID is no longer running at return, no delayed marker was written,
and both output joins completed. No helper/output task is claimed to enforce
an OS sandbox. The test wall assertions follow the audited existing grace:
remaining allowance <=1s plus up to three 2s cleanup graces, not a new policy
or unexplained 163s tolerance. These are test bounds under ordinary OS
scheduling, not universal bounds on synchronous filesystem/kernel stalls.

Focused budget tests preserve staged/file-info one-total-budget composition,
elapsed remainder, child ceilings and zero-budget final-child rejection. New
status test checks nine elapsed seconds leave one second and ten leave zero.
They **do not** establish that status's pre-final Git probes or synchronous
boundary traversal stop at the status deadline. No production correction is
justified as a proven cause/cure of the historical event by these controls.

### Fixed-count real-repository campaign - stopped on sample 1

All six labels were declared before execution. Each intended run was ordinary
HostToolScope -> registry lookup -> Execute authorization -> repo.status, with
absolute native Git and the production fixed/cleared environment. The native
host-only test binary was invoked directly (no Codex dependency/runtime turn),
with output captured by hidden Start-Process; relevant child inventory was
sampled by parent test PID. This launcher differs from Cargo's harness and its
synchronous diagnostic I/O visibly adds timing overhead. It is not evidence
that the historical Cargo launch had the same overhead or cause.

| Sample | Result | External dispatch | Revalidate | Git validation | Observation validation | Child timeout computation | Final status | Remaining before final status | Dirty entries |
| --- | --- | ---: | ---: | --- | --- | --- | --- | --- | --- |
| 1 | FAIL, first layout probe path after spawn | 2.5524664s | 1.5476ms | incomplete; approximately 1.7220485s elapsed including diagnostic overhead | not entered | not entered | not launched | unavailable | unavailable |
| 2 | NOT RUN - campaign stopped | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| 3 | NOT RUN - campaign stopped | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| 4 | NOT RUN - campaign stopped | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| 5 | NOT RUN - campaign stopped | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| 6 | NOT RUN - campaign stopped | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |

Sample 1 test PID 14868, Git PID 13580. Git resolution was exactly
`C:\Program Files\Git\cmd\git.exe`. Constructor setup=4.7427ms, external
single-test launch=3.3330329s, libtest report=2.75s. Entry to authorization
completion=0.1843106s; entry to observer start=0.8207793s; observation interval
=1.7235993s; child-start to spawn marker=0.3423502s; spawn marker to observation
failure=0.1229475s; host result conversion=1.4383ms. Event entry-to-return
=2.5488531s reconciles to external dispatch within 3.6133ms.

No timeout/kill/reap/wait-completion marker was reached in this failed trace.
Those durations are unavailable, not recorded as zero. The error lies after
first spawn and before normal wait completion; supervisor attachment or another
early process path is possible. Exact source chain was not printed by the
campaign-source wrapper (RuntimeFailure Debug intentionally hides it), so the
specific OS cause is **not recoverable from this trace** and is not invented.
Diagnostic logging between spawn and supervisor attachment could itself permit
a fast-exit race; causation is unproven. Final instrumentation moves that log
after attachment and retains attachment errors and the local source chain for
a future explicitly authorized investigation. This is a diagnostic ordering
adjustment, not a proven production-process correction. Campaign-source copies
and original logs remain frozen; sample 1 was not rerun with the adjustment.

Inventory records test-owned Git counts 0/1, and zero remaining owned children
at return. Relevant fixture/Git PIDs were absent in the final read-only check.
No unrelated Git/Edge/Codex/user process was killed. Start-Process's returned
Process.ExitCode was null after exit in this capture; the failed libtest report
and panic establish failure independently. It is not reported as exit 0.

### Interpretation, classification and final control decision

- **T1:** no deterministic 163-second timeout/kill/reap/pipe defect reproduced
  or corrected. Source audit identifies unenforced timing regions and incomplete
  status probe deadline composition; it does not prove the historical duration
  originated in any one of them.
- **T2:** outer setup/lease/conversion are outside the observer timer. Their
  existence is proven; attribution of historical excess time to them is not.
- **T3:** proven timing-scope mismatch: libtest whole run versus R4 observer
  interval. This explains why the numbers cannot be equated, not where every
  historical second went.
- **T4:** not proven. Passing native cleanup controls do not establish an
  external/environmental cause for the historical stall.
- **T5:** the historical 163.64s event remains causally unexplained. Retain the
  exact failed evidence; do not erase it as a transient that supposedly cannot
  recur. Primary historical-cause conclusion is T5, with T3 established.

**H6 - STILL INDETERMINATE.** Cleanup controls are bounded under the audited
runner contract, but the full status deadline is not proven interruptible, the
six-sample campaign stopped on a new preserved failure, and the specific new
failure cause is unavailable. H1/H2 criteria are not met. There was no
historical-like overrun to classify H3, no measured outer-path overrun to
classify H4, and no proven external cause for H5.

The designated final production host-only status control was **NOT RUN**:
deterministic evidence plus the failed campaign does not support a trustworthy
lane. The successful final-source forced-failure control is not substituted for
that required successful dirty-status dispatch. **Task 510B-R5 NOT AUTHORIZED.**
Do not begin R5 automatically. Task 510C remains blocked. A future task should
first retain the exact early process/attachment source, account for launcher
and diagnostic-I/O perturbation, and resolve the synchronous/status-probe timing
contract without weakening policy or replaying this campaign as retries.

### Validation, identity, authority and Git state

Final-source focused evidence:

- repository_observer: **7 passed / 0 failed / 1 ignored** (forced control run
  explicitly and separately: **1/0**).
- repository_git_layout timeout tests: **4/0**.
- repository_boundary: **13/0**.
- repository_status integration: **4/0**.
- execute_policy integration: **13/0**, including descendant/pipe timeout.
- rah-sandbox library: **12/0/1 ignored**, including blocked-reader control.
- rah-runtime library: **8/0**; forced host-dispatch integration **1/0**.
- cargo check for rah-sandbox/rah-tools/rah-runtime with diagnostic feature:
  PASS; scoped Clippy all-targets with -D warnings: PASS. The first Clippy
  failure in diagnostic helper placement/nested if is preserved and corrected.
  No full workspace/Desktop/frontend/Task 510B closure or live inference ran.

Final bundle verification (read-only; no recreation/extension):

| File | SHA-256 | Length | File ID |
| --- | --- | ---: | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 |

Both files remain read-only under the existing R4 bundle root. Original
certification_support.rs/experimental.rs Codex WIP diff was compared exactly to
the starting patch and is unchanged. R4H touched runtime/sandbox/tools opt-in
feature wiring, host/status/process diagnostics, observer tests, the native
fixture/execute-policy test and a new ignored runtime host-only integration
harness. Earlier observer/layout/boundary diagnostic WIP and historical report
are retained. No new crate dependency edge, public bypass/API, ToolRegistry,
authorization, active-repository authority, permissions, admission, model policy
or HostExplicit route change. Static host_invocation enum still has exactly
**11** supported variants; its existing count assertion remains unchanged.
No Desktop test execution is claimed. **ADR-B**, no new ADR.

No production timeout/process policy correction. No commit/push/new CI, tag,
release, admission change or full Task 510B closure; preserve unpublished WIP.
HEAD remains `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.

Final R4H checks: `cargo fmt --check` PASS (exit 0), `git diff --check` PASS
(exit 0), final-source scoped all-targets Clippy PASS (exit 0). Historical report
prefix preserved byte-for-byte; static HostExplicit variant count independently
confirmed 11. Final two-file hash/length/file-ID assertions PASS. Git status
retains all starting WIP plus R4H diagnostics/tests; no publication performed.


## Task 510B-R4I - exact subprocess provenance and designated host control (2026-10-05)

Starting/final HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
All starting unpublished R4/R4H WIP preserved; starting binary patch and host
harness saved in `F:\Temp\rah-task510br4i-evidence`. No Codex ran.
R4H's first real-repo failure had lost its concrete subprocess source after
spawn. Its cause remains unexplained; this execution is a newly designated
control, not a retry or replacement of that historical sample. Historical
**T3 timing scope mismatch** and **T5 unexplained** remain unchanged.

### Process-local evidence schema and deterministic proof

Opt-in `live-test-support` captures concrete type names, Debug structured
fields, nested Error::source snapshots and io::Error kind/raw OS code before
existing string conversions. Public/frontend diagnostics remain unchanged.
The diagnostic capture is explicitly serial and process-wide so records from
HostToolScope's owned execution task and pipe-reader tasks can reach the same
capture. Source phase is task-local. Records are printed after dispatch;
existing R4H synchronous timing markers are still enabled for reconciliation.
This capture is diagnostic-only, not a production event API or concurrent
telemetry facility. It retains typed-error snapshots, not new production error
variants or a changed public source-chain contract.

Per subprocess: HostProcessSpec includes canonical executable, exact argv, cwd,
explicit environment and child allowance; source phase and actual aggregate
budget option accompany records. Spawn PID, wall and monotonic timestamps,
wait/pipe/termination/reap markers, total wall duration, HostProcessOutput
(exit code, timeout, overflow, termination attempt, bounded stdout/stderr bytes)
and concrete errors are retained. Output lengths below derive from retained
byte arrays. For successful lifecycle, termination is unnecessary and kill/reap
is not attempted. OS errors are retained before erasure, with named Windows
job API failures recorded separately. No OS error occurred in this sample.

The runtime integration adds a test-only rah-runtime -> rah-sandbox dependency
(and corresponding Cargo.lock entry) to access the diagnostic observer. No
production dependency edge is added. Diagnostic source changes are confined
to sandbox process capture, host_execute's error-conversion capture, layout
probe/source phase scope, observer final-command scope, and host test harness.

`provenance_retains_failed_dispatch`: **PASS 1/0** through HostToolScope,
registry lookup and Execute authorization, using a deliberately missing child
executable and exact argv `--r4i-exact-command`. Assertions prove source phase,
command identity, spawn failure with no PID, concrete io::Error NotFound/code,
SandboxError, ToolError and AuthorizedDispatchError retention. Final trace
records OS code 2. Initial assertion-only failure expected std::io::error::Error;
this toolchain reports core::io::error::Error. Original failed log retained;
corrected type-name assertion passed. No real-repo execution occurred in that
fault control.

### Designated real-repository control - FAIL; stop

Exactly one ignored `host_status_sample` run, label `R4I-designated`, test PID
10216, ordinary HostToolScope -> registry -> authorization -> repo.status against
`F:\coding\otherPrj\rust-agent-harness`. Hidden native test launch;
constructor setup **4.9917ms**, external dispatch **13.8229073s**, libtest
lifecycle **16.20s**. Authorization accepted; all eight required layout Git
probes succeeded; final status never launched; no dirty result returned.

Every row is phase `repository_git_layout::probe`, child allowance **5s**,
parent aggregate **None** (the actual source supplies no aggregate deadline
to these status layout probes; None is not invented as a numerical allowance).
Observation total policy remains 10s, unchanged. Resolved Git is
`C:\Program Files\Git\cmd\git.exe`; canonical executable spelling is
`\\?\C:\Program Files\Git\cmd\git.exe`; cwd is
`\\?\F:\coding\otherPrj\rust-agent-harness`.

Start/completion timestamps below are Unix wall nanoseconds from retained
SystemTime records. Duration is the inner process-runner wall measurement;
completion is the output-retention timestamp, so diagnostic overhead can make
its start/end span larger. All exit codes are 0; all timeout/overflow/termination
flags are false/None/false; stderr is empty; no kill was required.

| Exact argv | PID | Start wall ns | Completion wall ns | Runner duration | Exit | stdout/stderr bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| [rev-parse, --show-toplevel] | 724 | 1791203492208853300 | 1791203492633382800 | 422.5133ms | 0 | 38/0 |
| [rev-parse, --path-format=absolute, --absolute-git-dir] | 16228 | 1791203492639283100 | 1791203492717184000 | 75.9906ms | 0 | 43/0 |
| [rev-parse, --path-format=absolute, --git-common-dir] | 13172 | 1791203492724701600 | 1791203492778314500 | 51.2422ms | 0 | 43/0 |
| [rev-parse, --is-bare-repository] | 10532 | 1791203492786024300 | 1791203492839485300 | 51.006ms | 0 | 6/0 |
| [rev-parse, --show-superproject-working-tree] | 11508 | 1791203492845289500 | 1791203492912128600 | 64.8562ms | 0 | 0/0 |
| [rev-parse, --path-format=absolute, --git-path, index] | 15656 | 1791203492918834500 | 1791203492969825600 | 48.7833ms | 0 | 49/0 |
| [rev-parse, --path-format=absolute, --git-path, HEAD] | 16036 | 1791203492978111300 | 1791203493028753400 | 48.6609ms | 0 | 48/0 |
| [worktree, list, --porcelain, -z] | 2852 | 1791203493036202100 | 1791203493110906300 | 72.9165ms | 0 | 4824/0 |

Environment is **cleared**, with exactly these supplied variables:

- GIT_CONFIG_COUNT=3
- GIT_CONFIG_GLOBAL=NUL
- GIT_CONFIG_KEY_0=core.fsmonitor; GIT_CONFIG_VALUE_0=false
- GIT_CONFIG_KEY_1=core.untrackedCache; GIT_CONFIG_VALUE_1=false
- GIT_CONFIG_KEY_2=safe.directory;
  GIT_CONFIG_VALUE_2=\\?\F:\coding\otherPrj\rust-agent-harness
- GIT_CONFIG_NOSYSTEM=1
- GIT_OPTIONAL_LOCKS=0
- GIT_TERMINAL_PROMPT=0

No general user shell environment inherited by Git. Complete bounded byte
contents, per-event timestamps, process fields and typed snapshots are in
`F:\Temp\rah-task510br4i-evidence\designated.stderr`; raw stdout and owned-child
inventory are adjacent. Successful output identifies the correct repository
root/Git metadata; worktree listing is retained for comparison, not mutation.

### Exact causal chain and failure classification

**P8 - OTHER EXACTLY IDENTIFIED FAILURE**: pre-launch observation-budget
exhaustion in `repository_observer::child_timeout`, called by
`RepositoryObserver::run_with_budget` after `validate_observation` and before
constructing/launching the final status process.

Retained process-local chain:

`RuntimeFailure { diagnostic: RuntimeDiagnostic { operation: Turn,
kind: Operation, rpc_code: None }, .. }`
-> `rah_tools::authorized_dispatch::AuthorizedDispatchError::Tool`
-> `rah_tools::ToolError::Execution { message:
"Git repository policy rejected capability: repository observation exceeded its total timeout" }`.

ToolError has no nested Error::source in this branch; there is no failing
SandboxError or io::Error because **no child failed**. Eight concrete
HostProcessOutput records prove successful Git lifecycle. The exact source
predicate is `remaining.is_zero()` in child_timeout; for this status call
aggregate=None and remaining=10s.saturating_sub(now-started). This is the unique
reachable producer of this error after the recorded validate_observation marker
and before execute_process. It is a correct exhausted-budget refusal, not a
Git nonzero exit, child timeout, pipe failure, authority rejection, or conversion
of a completed final status result. No P1/P2/P3/P4/P5/P6/P7/P9 claim is made.

Phase measurements: revalidate **1.4873ms**; validate_git_with_budget
**2.2660532s**; validate_observation **7.4523564s**. The last marker had cumulative
**9.7199007s**, remaining **280.0993ms** before its synchronous diagnostic write;
child_timeout then refused the exhausted remainder. Existing diagnostic I/O
perturbs this run and must not be attributed to historical root causes. The
sample proves this run's refusal branch, not the earlier R4H attachment failure.

Direct exact-subcommand control: **NOT APPLICABLE / NOT RUN**. No Git subcommand
failed. No correction, timeout change, policy change, automatic replay or retry.
Six additional serial samples: **all NOT RUN**, because designated control
failed. Do not substitute focused fixture tests for real-repository PASS.

### Full dispatch timing and cleanup

| Consecutive marker interval | Seconds |
| --- | ---: |
| dispatch_start -> authorization_complete | 0.2160790 |
| authorization_complete -> lease_wait_start | 0.2086007 |
| lease_wait_start -> lease_acquired | 0.2078837 |
| lease_acquired -> observation_start | 0.2185393 |
| observation_start -> observation_end | 11.0396617 |
| observation_end -> tool_execution_complete | 0.8246150 |
| tool_execution_complete -> result_conversion_complete | 0.3004917 |
| result_conversion_complete -> dispatch_return | 0.1850301 |
| Accounted marker span | **13.2009012** |
| External dispatch | **13.8229073** |
| Unexplained timing gap | **0.6220061** |

The 622.0061ms gap exceeds ordinary low-overhead instrumentation; full timing
reconciliation is **not claimed**. Marker writes/setup around measurements may
contribute, but the exact partition of this gap was not retained. This does
not erase the exact pre-launch failure evidence. No timing-policy correction.

Scope revoked and drained before the failure assertion. Parent-PID inventory
and final read-only CIM check found no remaining test PID, recorded Git PIDs,
or child of test PID 10216. No unrelated process killed. All eight Git children
completed normally and their pipes joined. The failed sample source was copied
before subsequent lint-only module/re-export placement correction; no designated
sample rerun on final source. R4H timeout fault-control evidence remains valid;
no contradictory timeout/kill/reap evidence appeared.

### Focused validation, identities and closure

- Provenance dispatch test: **1 passed / 0 failed**.
- repository_observer: **8 passed / 0 failed / 1 ignored**.
- repository_git_layout: **20 passed / 0 failed**.
- repository_boundary: **19 passed / 0 failed**.
- repository_status integration: **4 passed / 0 failed**.
- sandbox library: **12 passed / 0 failed / 1 ignored**.
- execute_policy integration: **13 passed / 0 failed**.
- Scoped check rah-sandbox/rah-tools/rah-runtime with diagnostic feature: PASS.
- Scoped all-targets Clippy with diagnostic feature and -D warnings: PASS after
  correcting diagnostic re-export/module placement. Initial lint failure saved.
- cargo fmt --check and git diff --check: PASS.

No full Task 510B closure, Desktop, runtime certification, live model or new CI.
Existing HostExplicit enum remains exactly **11** variants and count assertion
unchanged. ToolRegistry, authorization, active repository authority, permissions,
production Codex admission, preferred baseline, timeout values, deadline origin,
layout rules and host behavior unchanged. **ADR-B**, no new ADR.

Read-only final bundle check: both exact identities unchanged:

| File | SHA-256 | Length | File ID |
| --- | --- | ---: | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 |

**Final B - EXACT HOST FAILURE CLASSIFIED.** Current designated run is P8;
H2 is not established. R4H's earlier exact subprocess cause and historical T5
remain unexplained. A separately scoped next task should address the proven
pre-launch budget-exhaustion path and diagnostic timing perturbation without
weakening timeout or repository policy. No production correction made here.
**Task 510B-R5 NOT AUTHORIZED / remains blocked.** Do not begin R5 automatically.
Task 510C remains blocked. No commit, push or new CI. Preserve unpublished WIP.


## Task 510B-R4J - observation composition audit (2026-10-05)

Starting HEAD: 52850938a5d88a21ec14c5aa1d8f1cd9f1382820.
All starting unpublished WIP retained; binary patch and untracked host-test copy
preserved in F:\Temp\rah-task510br4j-evidence. No Codex or publication.

### Source of truth, invariant mapping and exact probes

Read README, architecture/guardrails/security, accepted ADRs 0027/0029,
Task 058 observer research, Task 060 status design, Task 312 nested-boundary
conformance, Task 359 linked-layout foundation, source/tests and R4I.
The RAH lease excludes other RAH operations, not external layout replacement.
Admission or authorization observations cannot replace execution-time freshness.

Common table fields:
- Source: RepositoryGitLayout::validate_git_with_budget (binding below),
  runner repository_git_layout::probe.
- Consumer: final comparison in RepositoryGitLayout::validate_git_with_budget,
  via RepositoryIdentity::validate_git_with_budget and
  RepositoryObserver::run_with_budget.
- Executable: \\?\C:\Program Files\Git\cmd\git.exe.
- R / cwd: \\?\F:\coding\otherPrj\rust-agent-harness.
- E / inputs: cleared environment with GIT_CONFIG_COUNT=3,
  GIT_CONFIG_GLOBAL=NUL, GIT_CONFIG_NOSYSTEM=1, GIT_OPTIONAL_LOCKS=0,
  GIT_TERMINAL_PROMPT=0; keys/values core.fsmonitor=false,
  core.untrackedCache=false, safe.directory=R. ToolInput {}.
- Seven rev-parse stdout limits 16 KiB; worktree list 1 MiB; stderr 16 KiB.
- All child allowances 5s, aggregate None, per Task 359 semantic-probe contract.
  Elapsed time still consumes the final status observation remainder.
- All exit 0, no timeout/overflow/termination; bounded raw outputs retained in
  designated.stderr. Main-worktree outputs below do not generalize to linked.

| # | Source binding | Exact Git argv, excluding executable | Cwd / inputs | Output in designated trace | Invariant / consumer predicate | Probe envelope duration | Deadline / allowance |
| --- | --- | --- | --- | --- | --- | ---: | --- |
| 1 | top | ["rev-parse", "--show-toplevel"] | R / E, selected root | F:/coding/otherPrj/rust-agent-harness plus LF | ADR 0027 authority root; canonical_probe_path(top) == self.root | 599.0232ms | aggregate None / 5s |
| 2 | private | ["rev-parse", "--path-format=absolute", "--absolute-git-dir"] | R / E, captured layout | F:/coding/otherPrj/rust-agent-harness/.git plus LF | ADR 0029 selected private identity; canonical_probe_path(private) == self.git_dir() | 58.4326ms | aggregate None / 5s |
| 3 | common | ["rev-parse", "--path-format=absolute", "--git-common-dir"] | R / E, common relationship | F:/coding/otherPrj/rust-agent-harness/.git plus LF | ADR 0029 common relationship, not sibling authority; canonical_probe_path(common) == self.common_git_dir() | 67.4613ms | aggregate None / 5s |
| 4 | bare | ["rev-parse", "--is-bare-repository"] | R / E, supported layout | false plus LF | ADR 0027/0029 non-bare executable target; parse_line(bare) == "false" | 59.6128ms | aggregate None / 5s |
| 5 | superproject | ["rev-parse", "--show-superproject-working-tree"] | R / E, supported layout | empty | ADR 0029 excludes submodules; superproject.is_empty(); old-form directory submodule fixture proves .git directory alone insufficient | 71.6778ms | aggregate None / 5s |
| 6 | index | ["rev-parse", "--path-format=absolute", "--git-path", "index"] | R / E, private target | F:/coding/otherPrj/rust-agent-harness/.git/index plus LF | ADR 0029 selected index resolution; canonical_probe_path(index) == self.index_path() | 53.7124ms | aggregate None / 5s |
| 7 | head | ["rev-parse", "--path-format=absolute", "--git-path", "HEAD"] | R / E, private target | F:/coding/otherPrj/rust-agent-harness/.git/HEAD plus LF | ADR 0029 selected HEAD resolution; canonical_probe_path(head) == self.head_path() | 64.6877ms | aggregate None / 5s |
| 8 | worktrees | ["worktree", "list", "--porcelain", "-z"] | R / E, retained root | bounded NUL-delimited native registrations, exact bytes retained | ADR 0029 unique supported registration; selected_worktree_registered(worktrees, self.root) | 73.7928ms | aggregate None / 5s |

Registration parser requires exactly one matching non-bare/non-prunable root,
valid HEAD shape, branch/detached coherence and recognized, unambiguous records.
Coherent locked registrations remain accepted. A registration HEAD OID does
not substitute for Git's selected HEAD-path resolution.

### D1/D2/D3 overlaps, reusable snapshot and probe order

| Overlap | Classification | Security/semantic disposition |
| --- | --- | --- |
| Admission/authorization identity vs dispatch revalidate | D1 | Earlier identity is not fresh execution currentness; external replacement possible. |
| Status pre-run and run_with_budget revalidate, RepositoryIdentity wrapper and layout pre/post revalidations | D1 retained | Different envelope/semantic boundaries. No proof a filesystem observation is dispensable or that expensive duplicated work exists under R4. |
| Captured paths/objects vs Git semantic answers | D3 | Filesystem relationship evidence does not authoritatively provide native Git interpretation. |
| private vs common | D3 | Coincide on main; differ on linked. Common relationship does not merge authority. |
| private directory vs index/HEAD paths | D3 | Directory identity does not return semantic selected HEAD/index resolution. |
| top vs worktree registration | D3 | Current root vs unique coherent native registration. |
| bare vs registration flags | D3 | Current Git classification vs supported registration coherence. |
| revalidate/validate_git_with_budget vs validate_observation | D3 | Root/private/common identities vs descendant-boundary safety. |
| validate_observation vs final status preparation | D3 | Fresh tree-safety validation then fixed status argv/allowance, no repeated Git layout command. |
| Repeated identical Git command in status dispatch | None | All eight commands run once. No D2 candidate. |

All eight facts are retained until the final comparison; none is discarded and
recomputed in this observation. Structured Main/Linked evidence is already
retained. Earlier admission/dispatch data is outside the necessary freshness
window. No cross-dispatch cache or reusable earlier snapshot qualifies.

RepositoryNestedBoundaryPolicy::validate_observation walks descendants once,
rejecting nested markers, ambiguous host spelling and reparse traversal before
Git reads the tree (ADR 0027 / Task 312). Root layout validation cannot supply
this proof. Per-directory marker checks and enumeration have different
host-spelling/currentness purposes; safely removable expensive duplication is
not proved. Their resemblance is insufficient for R4.

Order audit: eight distinct facts are collected in fixed order, checked together,
then filesystem evidence is revalidated. No requirement that, for example, bare
must follow common was found, but all facts and freshness boundaries must
complete before status. No useful security-equivalent reorder is established;
no reorder made. Repeating validation across separate observer commands is
freshness protection, not a status-dispatch duplicate.

### Exactly one pre-correction designated trace

R4J-pre-correction-designated, test PID 10124: PASS 1/0 through HostToolScope ->
registry -> authorization -> observation validation -> final status -> valid
dirty ToolOutput. 19 entries; setup 4.5766ms; dispatch 9.4955729s;
whole operation 10.3627598s; libtest 10.52s. This is diagnostic-only.
No retry or second designated diagnostic run.

Before/after measurements use the actual status observation Instant.
Remaining = 10s.saturating_sub(elapsed); child allowance is actually 5s,
aggregate None, not the displayed status remainder. Duration includes setup,
process execution and buffered output retention, excludes subsequent R4 print.

| # | Elapsed before (s) | Remaining before (s) | Child allowance (s) | Duration (ms) | Elapsed after (s) | Remaining after (s) | PID | Inner runner (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1.3381996 | 8.6618004 | 5 | 599.0232 | 1.9372201 | 8.0627799 | 14868 | 596.2236 |
| 2 | 1.9442005 | 8.0557995 | 5 | 58.4326 | 2.0026317 | 7.9973683 | 8232 | 54.9632 |
| 3 | 2.0106560 | 7.9893440 | 5 | 67.4613 | 2.0781150 | 7.9218850 | 11336 | 64.1690 |
| 4 | 2.0860470 | 7.9139530 | 5 | 59.6128 | 2.1456577 | 7.8543423 | 14132 | 56.6148 |
| 5 | 2.1519103 | 7.8480897 | 5 | 71.6778 | 2.2235859 | 7.7764141 | 15812 | 68.1231 |
| 6 | 2.2286352 | 7.7713648 | 5 | 53.7124 | 2.2823471 | 7.7176529 | 5280 | 50.8834 |
| 7 | 2.2906437 | 7.7093563 | 5 | 64.6877 | 2.3553304 | 7.6446696 | 15252 | 61.8651 |
| 8 | 2.3640351 | 7.6359649 | 5 | 73.7928 | 2.4378264 | 7.5621736 | 11356 | 70.7942 |

Phases: run revalidate 1.5994ms; validate_git_with_budget 2.4478983s;
validate_observation 6.1179531s. Boundary walk 6.1044487s:
6580 directories, 111287 entries, marker time 418.7619ms.
Final child_timeout allowance 1.4261118s; subsequent provenance phase remaining
1.4191645s before execute_process. Status PID 11832, inner runner 66.8342ms,
execute_process envelope 76.7448ms, exit 0, no timeout/overflow/termination.
Final argv: ["--no-pager", "status", "--porcelain=v2", "-z",
"--untracked-files=normal", "--ignored=no", "--no-renames",
"--ignore-submodules=all"], same R/E, dirty result valid.

Accounted dispatch marker span 9.4909234s; external dispatch 9.4955729s;
unexplained gap 4.6495ms (timing-reconciliation.json). Exact zero-gap
reconciliation is not claimed. Scope revoked/drained and final CIM inventory
contains no test/direct/Git PID or child of recorded PIDs.
R4I P8 / 13.8229073s / 622.0061ms gap are preserved independently.
Historical 163.64s remains T3 timing-scope mismatch / T5 cause unexplained.

Direct slow-probe control: exactly one ["rev-parse", "--show-toplevel"],
same executable/R/E, no inherited environment, no shell, PID 14376,
exit 0, correct root stdout, empty stderr, 52.8922ms (direct-control.json).
First probe dominates Git-probe time, not whole validation. Retained host trace
shows slow first spawn; one later fast direct execution cannot attribute a
repeatable cause to Git/environment or prove duplicated orchestration.
This command is not host validation and does not justify R4 correction.

### Budget contract and starvation audit

C2: layout/boundary validation plus final status in the same observation
envelope. Not C1 and not a C3 complete-dispatch deadline.
Read jointly: Task 058 bounded Tool timeout (status 10s), Task 060 status design
(10s process), Task 359 distinct five-second semantic probes, unchanged
checkpoint run comment "within a common timeout" and child_timeout arithmetic,
R4H status-remainder/expiry test, and R4I accepted exhausted-observation record.
Early design documents alone do not enumerate later-added validation stages;
they do not establish an independent fresh final-status timeout. Current
tests/task evidence clarify the operational scope; no stronger hard dispatch
wall bound is invented.

Status clock begins at observer.run after lease acquisition / outer revalidate.
Actual elapsed validation reduces final allowance: 10s minus elapsed.
Authorization, lease wait, outer post-observation revalidate and conversion are
outside this clock. Status probes intentionally have aggregate None / individual
5s ceilings per Task 359. This does not reset the final clock. It is not a hard
aggregate interrupt for synchronous traversal/probes at ten seconds.
Consequently required work can finish after ten seconds and fail closed before
status. Supplying a new aggregate to probes would tighten a separate child
contract, not correct proven arithmetic, and would not reserve status time.

No guaranteed final-status opportunity or reserved quota is specified.
Required validation plus positive remainder are necessary for launch.
R4I proves legitimate expiry; this fast pre-correction trace proves positive
remainder in one run, not reliability. No full/stale final allowance, reset,
independent timeout, reserved slice, or timeout increase is justified.

### Correction gate, security equivalence and validation

R1 none: no accidental recomputation / qualifying same-window reusable snapshot.
R2 none: no existing invocation returns another required semantic answer.
R3 none: actual elapsed is used for final allowance; five-second layout ceiling
is a distinct explicit contract, not demonstrated stale arithmetic.
R4 none: no expensive safely removable duplicate was demonstrated.
No production correction or security-equivalence claim for a patch.

Only live-test-support diagnostic additions in repository_git_layout.rs and
repository_observer.rs: task-local status clock and buffered per-probe records.
They never supply execution allowances, change deadlines, cache facts, or grant
authority. R4I process-local phase/argv/PID/outcome/typed-source capture retained.
After the trace the diagnostic scope was restricted to Status to avoid labeling
other observer commands with 10s; designated source copies/hashes retained,
no host rerun. Non-feature execution unchanged.

Focused serial validation on final source:
- repository_observer: 8 passed / 0 failed / 1 ignored.
- repository_git_layout (including budgets, main/linked and rejected layouts):
  20 passed / 0 failed.
- repository_boundary: 19 passed / 0 failed.
- repository_status integration: 4 passed / 0 failed.
- execute_policy integration: 13 passed / 0 failed.
- rah-sandbox library: 12 passed / 0 failed / 1 ignored.
- host provenance_retains_failed_dispatch: 1 passed / 0 failed.
- Scoped cargo check rah-tools / rah-runtime with live-test-support: PASS.
- Scoped all-targets Clippy same crates/feature, -D warnings: PASS.
Initial provenance command placed --exact before Cargo's -- separator and was
rejected by Cargo without running a test; saved provenance.log. Corrected
command passed in provenance-final.log; no source failure/retry or host replay.

Post-correction designated host control: NOT RUN, no production correction.
Six-sample campaign: NOT RUN (0/6), conditional correction gate not reached.
Pre-correction diagnostic PASS is not a campaign/admission/certification claim.

### Bundle, authority and final disposition

Read-only SHA-256, length and fsutil file-ID verification (bundle.json):
| File | SHA-256 | Length | File ID |
| --- | --- | ---: | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 |

Frozen bundle unchanged; Lane A remains A1. No bundle executable launched.
HostExplicit exact 11-name allowlist/count assertion source unchanged; no
Desktop execution claimed. Active root, linked-worktree authority, HostToolScope,
ToolRegistry, authorization, permissions and dependency edges unchanged.
ADR-B; no new ADR.

Final **B - OBSERVATION WORKLOAD IS REQUIRED AND CURRENT POLICY BUDGET IS
INSUFFICIENT**: required workload can legitimately exhaust the current policy
budget (R4I), despite this designated diagnostic PASS. No authorized composition
defect proved; stop without forcing optimization. Separate policy/performance
decision required. **Task 510B-R5 NOT AUTHORIZED / remains blocked.**
Task 510C remains blocked pending full Task 510B certification and exact-head CI.
No commit/push/new CI/tag/release/admission change; unpublished WIP retained.


Final cargo fmt --check and git diff --check: PASS (exit 0).
Starting/final patch comparison confirms only repository_git_layout.rs,
repository_observer.rs and this plan changed during R4J; other starting tracked
WIP blocks are byte-identical. Existing untracked host-test source was preserved.

## Task 510B-R4K - repository observation budget policy decision (2026-10-05)

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Research/documentation only; all historical stops and diagnostic WIP preserved.
Lane A and its exact runtime bundle remain frozen. No executable launch,
Codex certification, production change or additional diagnostic sample.
Starting dirty/untracked hashes: F:\Temp\rah-task510br4k-evidence\starting-hashes.json.

### 1. Current C2 contract

Execution-time layout/boundary validation and final status share one 10s elapsed
observation allowance. The Instant starts after lease acquisition and outer
identity revalidation. Final status gets only the actual positive remainder;
there is no reserved slice or independent clock. Authorization, lease waiting,
outer post-observation revalidation and conversion are outside C2.
Preserve authorize -> fresh execution-time observation -> final repository status.
Earlier admission/authorization evidence cannot substitute for fresh observation.
C2 is not a hard interrupt or complete dispatch deadline: layout probes have
aggregate None / individual 5s ceilings, and synchronous traversal is not
interrupted at ten seconds. Exhaustion refuses status before launch.

### 2. Ten-second provenance

Earliest supported numeric policy: Task 058 bounded-result table in
`docs/RAH_V0.6_REPOSITORY_OBSERVER_RESEARCH.md`, commit 43c33f2 (2026-08-23),
assigns repo.status 10s. It requires host-owned limits and fail-closed results,
but gives no numeric threat calculation, measured workload or UX target.
Task 060, `docs/plans/2026-08-23-repository-status.md`, records a 10s process
limit. Commit 1afcb80ee133459d1676325350c66badab27ed42 introduces STATUS_TIMEOUT
and elapsed subtraction; git log -S STATUS_TIMEOUT confirms its introduction.
Classification: documented implementation policy default; numeric rationale
unknown. Neither security-derived, usability-derived nor historically arbitrary
is proved. Later Task 312 added boundary traversal; Task 359 / 02c3eed added
eight 5s layout probes. Tasks 417/419/422 concern staged-diff composition, not
a security derivation for status's ten seconds. Current tests and R4H/R4I/R4J
clarify elapsed refusal semantics; no inspected ADR/security document mandates
exactly ten seconds. Historical released records must remain intact.

### 3. Timeout hierarchy

| Limit | Current contract | Role |
| --- | --- | --- |
| Status total | 10s C2 elapsed allowance | Host resource/latency policy, not authority or freshness TTL |
| Layout children | Each 5s; status aggregate None | Bounded child execution; elapsed still consumes C2 |
| Final status child | 10s minus actual elapsed; positive required | Inside C2; no independently reserved budget |
| Termination/reap | 2s TERMINATION_GRACE for child.wait after termination attempt | Cleanup; no rollback or successful-work allowance |
| Pipe readers | 2s each join, stdout then stderr | Cleanup/drain; up to 4s sequentially, subject to earlier errors |
| Lease wait | Async per-root mutex, no timeout here; outside C2 | RAH serialization, not an external-process lock |
| Synchronous validation | Elapsed charged, no hard C2 interruption | Existing limit on wall-time claims |
| Other observers | File-info 5s child / 20s total; diff 15s; search 15s | Separate unchanged policy |

Sources: repository_observer.rs, repository_git_layout.rs, repository_status.rs,
supervised_process.rs. The process timer starts after spawn, supervisor
attachment and reader setup. Preparation, startup, reap and reader draining
prevent equating child allowance with exact return time. Neither C2 nor child
limits prove a full dispatch bound. Timeout/overflow remain closed failure,
termination attempt, no partial success and no automatic replay.

### 4. Security rationale

Bounded host-controlled execution is security-relevant; exactly ten seconds is
not an established security boundary. ADR 0009 delegates timeout/supervision
to the host; ADRs 0027/0029 require selected identity/currentness and fail-closed
validation. They do not make ten seconds a freshness TTL. Enlarging a numeric
allowance grants no cached authorization, new root, stale-evidence substitution
or transaction guarantee. External TOCTOU remains best effort.

### 5. Required workload

All eight R4J probes remain required: selected root, private gitdir, common
relationship, non-bare classification, superproject exclusion, selected index,
selected HEAD and unique coherent native worktree registration. No safe D2;
D1 freshness and D3 distinct evidence retained. Descendant-boundary validation
is independently required before whole-tree Git reads.
R4I: all layout children completed normally; required work plus diagnostic
I/O exhausted C2 before final status (P8). Observation marker span 11.0396617s;
dispatch 13.8229073s; unexplained dispatch gap 622.0061ms retained.
R4J: dispatch 9.4955729s; 19 valid dirty entries; observation marker span
8.8228097s from timing-reconciliation.json; final allowance 1.4261118s;
status runner 66.8342ms / process envelope 76.7448ms. Diagnostic I/O perturbs
both samples: planning evidence, not a normal-latency distribution or certification.

### 6. Performance decomposition

R4J designated.stderr and timing-reconciliation.json were inspected.

| Measured region | Time | Attribution |
| --- | ---: | --- |
| Eight probe envelopes summed | 1.0484006s | Preparation/process/capture, not pure Git CPU |
| Eight inner runners summed | 1.0236364s | Startup/supervision/wait/readers included |
| Layout-validation phase | 2.4478983s | 1.3994977s outside probe envelopes |
| Boundary walk | 6.1044487s | Separate synchronous walk; no Git child here |
| Nested marker checks within walk | 0.4187619s | Subset, not additive |
| Walk remainder | 5.6856868s | Enumeration/metadata/spelling/reparse checks/queueing combined |
| Whole boundary-validation phase | 6.1179531s | Walk plus 13.5044ms surrounding overhead |
| Run identity revalidation | 1.5994ms | Separate measured phase |
| Status process envelope | 76.7448ms | Inner runner 66.8342ms |
| Dispatch reconciliation gap | 4.6495ms | Unassigned; no zero-gap claim |

Walk: 6,580 directories / 111,287 entries. Source uses read_dir, per-entry
symlink_metadata, per-directory ambiguous-component/nested-marker checks and
queueing. Root .git is skipped, symlink traversal excluded; 100,000-directory
cap retained. The eight raw child runtimes do not explain the six-second walk.
R4I whole boundary phase 7.4523564s versus R4J 6.1179531s: spread 1.3344033s.
Layout remainder includes executable/layout identity capture and revalidation,
canonicalization, policy/environment construction, parsing and diagnostic writes.
Existing instrumentation cannot partition its 1.3994977s further, or partition
the walk remainder into individual filesystem/policy percentages. Lease work
is outside these phases: R4J lease marker span 175.0667ms includes writes and
is not pure lock contention. First probe 599.0232ms versus later direct control
52.8922ms shows startup-path variability without proving its OS/filesystem cause.
No attribution to antivirus, removable duplication or optimization is established.
The six-second scan merits separate later performance research under unchanged
checks; increasing C2 does not declare implementation performance acceptable.

### 7. P1 - retain ten seconds

Rejected as selected policy. Bounded latency/resource use is justified, but the
exact ten-second threshold is not. Required work exhausts it, and R4J established
no security-equivalent performance correction. P1 would retain all checks and
require a separate performance task with R5 blocked; no credible proven path
currently supports choosing it over P2.

### 8. P2 - enlarge existing C2

Selected: **14 seconds**, preserving current C2 composition and actual elapsed
subtraction, including the configured status ceiling that supplies this total.
Leave layout 5s children, cleanup graces and other observer policies unchanged.
Evidence rule for this decision:

    H = max(R4I, R4J observation marker spans) = 11.0396617s
    L = min(same spans) = 8.8228097s
    V = H - L = 2.2168520s
    S = measured R4J final-status process envelope = 0.0767448s
    B = ceil_to_whole_second(H + V + S) = ceil(13.3332585s) = 14s

H retains diagnostic overhead and the failed refusal interval as a conservative
planning proxy. S adds the final child absent in R4I. V repeats the entire
observed spread once as finite margin for local filesystem/startup/orchestration
variance. Rounding adds 0.6667415s; headroom above H plus S is 2.8835935s.
This is an explicit engineering margin, not a percentile, statistical bound,
unperturbed benchmark or reliability proof. Neither historical 163.64s libtest
nor full dispatch 13.8229073s sizes C2. No six-sample success prerequisite.
Final status has opportunity, not a reservation; exhausted validation still
refuses launch. Pathological children retain timeout/fail-closed controls.
Pathological filesystem stalls and lease waiting remain existing limitations,
not normal variance this formula promises to absorb. If implementation
validation fails, preserve failure and stop; no automatic enlargement or retries.

### 9. P3 - redefine/split

Rejected: no concrete security rationale established for splitting. Validation
freshness remains best effort because external actors can mutate between phases;
RAH lease is not a transaction. Status still requires the same root, identities
and authorization. Serial independent budgets increase stall exposure and need
an explicit total bound and stale-window analysis. These are not solved by
guaranteeing status launch. No split/clock reset/stale evidence is authorized;
an actual semantic split would require ADR-A review and explicit security analysis.

### 10. P4 - stop certification

Rejected as selected direction. Keeping policy and stopping is safe, but the
provenance and required workload justify bounded P2 without authority change.
Candidate desirability is not evidence. P4 would retain all checks and leave
candidate certification incomplete.

### 11. UX and denial-of-service exposure

Fast case: a larger deadline adds no deliberate delay; small-tree/warm local
work can finish earlier. No ordinary host latency distribution measured; the
52.8922ms single direct probe is not a full status UX estimate. Slow valid case:
R4J 9.4955729s dispatch for this large diagnostic tree, not expected normal UX.
Hard allowance case: P2 admits work within 14s C2, four seconds more than current
policy. Cleanup can add 2s reap and two 2s reader joins where applicable.
Preparation, synchronous validation and outside lease waits mean neither 14s
nor 20s is a proven hard dispatch bound. Longer lease ownership increases RAH
serialization/DoS exposure. Fixed numeric allowance, unchanged child/output/tree
caps, failure rules and no replay constrain it within the existing model. This
tradeoff is accepted for required read-only work, not generalized to mutations.

### 12. ADR assessment

**ADR-B** for selected P2: checked ADR 0009 process policy, ADR 0027 currentness,
ADR 0029 linked identity, security docs and architecture guardrails. Numeric
host-owned resource policy changes within the same C2 semantics; authorization,
freshness order, public API, dependency direction and security model remain.
None of these ADRs fixes status at ten seconds. Historical released ten-second
records must not be rewritten. Next task documents the new current policy and
its distinction from historical records. ADR-B does not authorize a split,
hard-dispatch guarantee or model-controlled budget.

### 13. Selected policy and authority conclusion

**B - ENLARGE THE EXISTING C2 TOTAL OBSERVATION ENVELOPE.** Target 14s from
section 8; not implemented here. P1/P2/P4 retain every check. P3 was rejected
and cannot currently claim equivalent semantic/security composition; any future
P3 must also preserve all checks. Specifically unchanged: fail-closed validation,
linked-worktree checks, submodule exclusion, active authority root, private/common
Git identity, index/HEAD identity, worktree registration and Tool authorization.
No ToolRegistry bypass, permission grant, HostExplicit change (exactly 11),
admission, provider, dependency or runtime bundle change. TOCTOU not eliminated.

### 14. Exact next task and closure

Next: **Task 510B-R4L - implement and deterministically validate the 14-second
repo.status C2 policy**. Only status policy value and corresponding current
policy documentation/tests. Preserve deadline origin, elapsed remainder,
zero-remainder refusal, all eight 5s probes, boundary checks, cleanup and authority.
Test 14s initial allowance, remainder after validation and refusal at/after
expiry; focused observer/layout/boundary/status/authorization validation plus
fmt/check/clippy/diff checks as appropriate. No optimization, split contract,
Codex certification or automatic replay. Stop at a stable-source failure for
explicit disposition. Report policy validation before separate R5 resumption.
Later separate performance research may decompose synchronous costs without
weakening freshness. R4K does not begin R4L automatically.
Only this plan changed. No added instrumentation/build/tests/diagnostic workload.
No commit/push/new CI/tag/release/admission change; all starting WIP retained.
**R5 explicitly NOT AUTHORIZED pending R4L implementation validation.**
Task 510C remains blocked pending Task 510B final A, publication, exact-head CI PASS.

R4K closure checks: git diff --check PASS; status/stat inspected; SHA-256
comparison confirms every starting tracked/untracked WIP file other than this
plan is byte-identical. HEAD unchanged. No production validation claimed.

## Task 510B-R4L - implement 14-second C2 policy (2026-10-05)

Starting HEAD: `52850938a5d88a21ec14c5aa1d8f1cd9f1382820`.
Evidence: `F:\Temp\rah-task510br4l-evidence`; starting binary patch, WIP hashes
and original observer/host test retained. All historical stops preserved.

Policy source: `crates/rah-tools/src/repository_observer.rs:33`, STATUS_TIMEOUT,
10s -> 14s. Existing authoritative constant supplies constructor, diagnostic
status scope and final status allowance; three diagnostic-only labels now use
that same constant. No new timeout abstraction or arithmetic change.
Fresh validation + final status retain one elapsed envelope; final child gets
actual positive 14s minus elapsed, with no reset, reservation or aggregate
change to layout children. Lease waiting remains outside; synchronous validation
is non-preemptible. C2 is not a hard end-to-end dispatch deadline.
Layout PROBE_TIMEOUT stays 5s. TERMINATION_GRACE stays 2s, used independently
for child reap and each pipe-reader join. Other observer policies unchanged.
Deterministic controlled times assert 14s exactly, elapsed 0/6/13s and 14s-1ns,
and refusal at 14/15s; forced-child fixture backdates from the policy constant
to retain its one-second control. Host sample no longer imposes an obsolete
10s full-dispatch target; it still requires successful valid dirty status and
normal requested/started/finished lifecycle.

Focused serial tests: observer 8/0/1 ignored; layout 20/0; boundary 19/0;
status 4/0; process-policy 13/0; sandbox 12/0/1 ignored; provenance 1/0.
All eight semantic probes and boundary walk retained byte-identically;
linked-worktree, dirty-fixture and fail-closed layout tests passed.
Scoped check rah-tools/rah-runtime/rah-sandbox with live-test-support PASS.
Scoped Clippy same crates, all-targets/all-features, -D warnings PASS.
cargo fmt --check and git diff --check PASS. Existing external R4 target reused.
Pre-host diff guard: only observer and host test changed from starting hashes;
no probes, optimization, ToolRegistry or authority edits.
ADR-B: numeric host policy within existing semantics, no new ADR/dependencies.
Task 058 / 43c33f2 earliest supported 10s policy, Task 060 implementation;
numeric historical rationale unknown, no ADR mandates exactly 10s.
14s is the approved empirical bounded allowance, not a reliability guarantee.
R4K boundary 6.1044s / markers 0.4188s / remainder ~5.6857s remains future
performance research only; no boundary optimization in R4L.
Host validation and final disposition recorded below after execution.

Host lane: exactly one designated control, six additional serial samples, then
one final control. All PASS, no retries; each authorizes through the ordinary
HostToolScope -> registry -> Execute permission -> fresh repository validation
-> final status route, returns 19 dirty entries, eight successful layout probes
plus final status (nine captured PIDs), no child timeout/termination. Each
captured PID absent from CIM checks. Scope drained and normal lifecycle asserted.
Initial residual regex omitted Option(Some(pid)); corrected read-only extraction
verified all nine captured PIDs for every sample; no host dispatch was replayed.

| Control | Dispatch | Boundary validation | Cumulative before status | Remaining | Status allowance | Status envelope |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| designated | 6.6779835s | 6.2351921s | 6.6163534s | 7.3836466s | 7.3835086s | 57.4449ms |
| sample-1 | 6.5432627s | 6.1057641s | 6.4829305s | 7.5170695s | 7.5169112s | 56.2586ms |
| sample-2 | 6.5515418s | 6.1069046s | 6.4895551s | 7.5104449s | 7.5103461s | 57.6735ms |
| sample-3 | 6.587103s | 6.1225141s | 6.5269541s | 7.4730459s | 7.4728799s | 56.0691ms |
| sample-4 | 6.5713217s | 6.1454645s | 6.509586s | 7.490414s | 7.4903121s | 56.7758ms |
| sample-5 | 6.5200452s | 6.0894607s | 6.4589704s | 7.5410296s | 7.5408957s | 57.0293ms |
| sample-6 | 6.7224218s | 6.2457255s | 6.6597969s | 7.3402031s | 7.3400987s | 58.3248ms |
| final | 6.4015965s | 5.9714091s | 6.340758s | 7.659242s | 7.6591325s | 56.6881ms |

Designated walk: 6.2350519s, marker checks 434.6208ms, 6,580 directories /
111,287 entries. Probe order/durations and cumulative elapsed/remaining:

| Probe | Duration | Cumulative | Remaining C2 |
| --- | ---: | ---: | ---: |
| ["rev-parse", "--show-toplevel"] | 45.6936ms | 49.9673ms | 13.9500327s |
| ["rev-parse", "--path-format=absolute", "--absolute-git-dir"] | 42.988ms | 93.038ms | 13.906962s |
| ["rev-parse", "--path-format=absolute", "--git-common-dir"] | 41.6931ms | 134.8253ms | 13.8651747s |
| ["rev-parse", "--is-bare-repository"] | 42.0137ms | 176.9612ms | 13.8230388s |
| ["rev-parse", "--show-superproject-working-tree"] | 53.8851ms | 230.9286ms | 13.7690714s |
| ["rev-parse", "--path-format=absolute", "--git-path", "index"] | 42.2815ms | 273.3091ms | 13.7266909s |
| ["rev-parse", "--path-format=absolute", "--git-path", "HEAD"] | 41.9157ms | 315.3086ms | 13.6846914s |
| ["worktree", "list", "--porcelain", "-z"] | 62.118ms | 377.509ms | 13.622491s |

Raw .stdout/.stderr/.exit, host-results.json and per-control residual-verified.json
retain complete evidence. Status envelope includes policy/process overhead;
allowance captured at execute-process entry differs slightly from immediately
preceding boundary remainder because intervening work consumes the same clock.
Optional R4H wall-marker env was not enabled; full dispatch and phase timings
and process-local R4I/R4J argv/PID/outcome provenance remain captured.
No claim of an independently measured authorization interval or hard deadline.

Frozen bundle read-only SHA-256/length/fsutil file-ID verification PASS:
- codex.exe: fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d;
  326872368 bytes; 0x000000000000000000110000000c1861.
- codex-code-mode-host.exe: 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6;
  74697520 bytes; 0x000000000000000000180000000c1862.
No bundle executable launched. HostExplicit exact eleven-name host_kind mapping
and host_allowlist_is_exact assertion inspected, unchanged; no Desktop test run.
ToolRegistry, HostToolScope, authorization, active repository authority, linked
gitfile/registration, submodule exclusion, private/common Git identity, index/HEAD,
production Codex admission and preferred baseline unchanged. Every other starting
WIP file byte-identical; no new dependencies, public APIs or authority semantics.

Final classification: **A — 14-SECOND C2 POLICY IMPLEMENTED AND HOST LANE VALIDATED**.
Implementation eligible for isolated publication. Source policy/test can be
separated via audited index content from the diagnostic hunks without changing
working files. Only policy constant plus self-contained deterministic budget test
and this R4L report are eligible; historical unpublished certification stays WIP.
Publication/CI disposition and exact R5 decision follow below. R5 not begun.

Isolated publication candidate: HEAD source plus only STATUS_TIMEOUT 10 -> 14
and the self-contained deterministic remainder test; no diagnostics/features or
new dependency edges. Observer 8/0, scoped rah-tools check and all-targets /
all-features Clippy -D warnings, fmt/diff checks PASS. Candidate preserves
child_timeout and all repository validation code. Working files were not
replaced or reverted to prepare it. Audited two-file staged scope is source
policy/test plus R4L-only report appended to the checkpoint report; unpublished
R1-R4K historical additions and instrumentation remain in the original WIP.
No Codex certification, admission change, tag or release. Normal GitHub master
push and exact-head CI required for publication and R5 authorization. Until
exact-head CI PASS, R5 remains blocked; R5 will not start automatically.

Publication closure: isolated commit `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`,
`fix: widen repository observation budget`, two files / 136 additions / 1 deletion.
Normal origin master push PASS; local HEAD = origin/master = this exact SHA.
No mirror push, force push, tag or release. All working-file hashes unchanged
by commit. GitHub CI 37317133407 completed SUCCESS for this exact head:
https://github.com/spider2449/rust-agent-harness/actions/runs/37317133407
Formatting, workspace check/test/Clippy and Desktop Tauri permissions all PASS.
CI JSON retained in ci-current.json. Publication complete; diagnostic WIP and
historical stops remain unstaged/untracked in the original certification tree.
Final report publication paragraph intentionally remains WIP, not another commit.

**Task 510B-R5 AUTHORIZED**, classification A plus exact-head R4L CI PASS.
Use the unchanged validated two-file runtime bundle. First rerun the real
repo.status Tool round trip before later Tool/cancellation/diagnostics/Desktop
gates. R5 NOT STARTED by this task. Task 510C is not authorized by R4L.
Final authority/security conclusion unchanged; ADR-B; HostExplicit exactly 11.

## Task 510B-R5 - pre-run identity gate STOP (2026-10-05)

Starting HEAD and local origin/master: `3a11a315f1f5fc8a2bd6c3ed324f794c82da570d`.
R4L publication: `fix: widen repository observation budget`; exact-head CI
`37317133407` PASS as recorded in the published checkpoint/task instruction.
Staging empty. All 19 starting diagnostic WIP paths retained; only this report
is changed by R5. No executable launched, rebuild, admission or policy change.

Pre-run bundle: `F:\Temp\rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c`.
Independent PowerShell Get-FileHash and fsutil file queryfileid measurements:

| Member | Measured SHA-256 | Length | File ID | R5 result |
| --- | --- | ---: | --- | --- |
| codex.exe | fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d | 326872368 | 0x000000000000000000110000000c1861 | FAIL: differs from explicit required SHA |
| codex-code-mode-host.exe | 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6 | 74697520 | 0x000000000000000000180000000c1862 | PASS |

R5 explicitly requires main SHA
`fdda5fa3cf3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
That supplied string is 61 hexadecimal characters, whereas the measured SHA-256
is 64 characters. Measured main SHA matches the retained R4L measurement above.
This discrepancy does not establish artifact mutation; it establishes failure
of the literal R5 identity prerequisite. Do not silently substitute historical
identity for the explicit task requirement. Both lengths and file IDs match.
Read-only CIM inspection found no bundle-owned processes before the stop.

Final classification: **F — DETERMINISTIC / AUTHORITY / BUNDLE CLOSURE FAILED**.
Specific cause: pre-live bundle identity requirement mismatch. Tool gate not
attempted; no T1-T6 runtime failure classification and no TOOL_CERTIFICATION PASS.
Tool advertisement, companion resolution/spawn, raw Tool events, neutral requests,
authorization, execution, result replies and turn completion: NOT RUN.
Cancellation, diagnostics, Desktop, fresh production rejection and baseline
verification: NOT RUN. Protocol matrix remains incomplete; no new compatibility
claim. Full workspace/Clippy/Desktop/frontend closure, Tauri inventory, metadata
and executable HostExplicit checks: NOT RUN because live prerequisites failed.
No authority/security changes; C2 14s and timeout hierarchy untouched, production
admission/preferred baseline unchanged. No new ADR; existing ADR-B assessment
unchanged. No WIP removed, no commit/push/new CI, no tag/release/version change.
Task 510C remains blocked and is not authorized. Resume requires explicit corrected
main identity requirement; no automatic retry or bundle rebuild.

## Task 510B-R5A / resumed R5 (2026-10-05)

Plan: preserve diagnostic WIP and historical STOP; independently re-establish
bundle/source identities; retain digest-shape guard and test malformed inputs;
run focused metadata/check/Clippy/fmt/diff gates; repeat pre-run process gate;
resume the original R2 Tool prompt on the frozen bundle with gpt-6.1-sol.
Stop at the first Tool failure; only a passing Tool gate permits later R5 gates.
No architecture, admission, authority, prompt, baseline or timeout changes.

Starting HEAD: 3a11a315f1f5fc8a2bd6c3ed324f794c82da570d; staging empty.
Evidence directory: F:\Temp\rah-task510br5a-evidence.
The previous R5 STOP remains historically correct and is preserved above.
Its supplied 61-character expected SHA was a metadata transcription/truncation
defect, not evidence of changed bundle bytes. Correct canonical expected main
SHA-256 (regenerated from artifacts, not reconstructed from text):
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

Occurrence classification: the sole repository occurrence at historical R5 STOP
line 2057 is S3, preserved verbatim with this correction note. No S1 executable
expectation was malformed: existing HASH and bundle-descriptor.json already hold
the full value. No additional S2 or S4 occurrences found by whole-repository rg.
Historical R4 bundle-descriptor.json and R4L bundle.json both independently retain
the full digest, corroborating the regenerated measurement; no historical files
were rewritten. External historical evidence remains unchanged.

PowerShell Get-FileHash and harness measure_artifact agree for frozen main and
installed source; package.json confirms 0.160.0-win32-x64; both lengths 326872368.
Harness also measures source and frozen companion, agreeing with retained full
SHA 1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6,
length 74697520. Both digest lengths 64 and hexadecimal; case round-trip equal.
Frozen file IDs: main 0x000000000000000000110000000c1861;
companion 0x000000000000000000180000000c1862. PowerShell/fsutil raw identities
are in independent-identities.json; independent Rust measurements in
bundle-harness.log. Source file IDs are not required to equal snapshot IDs.

Existing expected-SHA guard already requires exactly 64 lowercase hexadecimal
characters before runtime probing. Added deterministic rejection coverage for
61/63/65 characters and 64 non-hex characters plus a valid mixed hex digest.
Added ignored read-only exact bundle/source identity test and narrowly selected
R5 frozen-bundle configuration for the existing R2 live harness; historical
snapshot default preserved. Original prompt and timeout unchanged; R5 prevents
stronger prompt/model fallback and stops on any second neutral Tool request.
Wire evidence goes to a new R5 file, preserving R2 historical bytes.

Focused validation: certification_support tests 6 passed / 0 failed, including
digest-shape, complete stable measurement fixture and snapshot fixture;
r5_bundle_identity 1 passed / 0 failed (23.76s), no runtime launch;
cargo check -p rah-runtime-codex --all-targets --features certification-harness
PASS; scoped all-targets/all-features Clippy -D warnings PASS;
cargo fmt --check PASS; git diff --check PASS. Existing Desktop manifest
multi-target warning persists. CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR use
F:\Temp\rah-task510br2-target. Full workspace closure not yet run.

Repair classification: **B — SHA REQUIREMENT REPAIRED; BUNDLE IDENTITY RESTORED**.
Pre-run identity hashes/lengths/file IDs PASS. Repeat no-process check immediately
before live launch below. R5 live outcome will be appended after shutdown.

### Resumed R5 Tool gate PASS; cancellation STOP

First live Tool gate used the exact validated frozen two-file bundle,
gpt-6.1-sol, only existing repo.status, normal HostToolScope Execute permission,
and authority root F:\coding\otherPrj\rust-agent-harness. Prompt verbatim:
"Call repo.status exactly once with {}. Use only the available RAH Tool. After
the result, reply RAH510B_TOOL_OK. Do not call any other tool."
No stronger prompt, policy adjustment or baseline fallback.

Tool round trip PASS (r5-tool.log, r5-tool-wire.json, tool-summary.json):
- Tool advertisement: thread/start dynamicTools exactly one rah_tool_0 mapped
  to repo.status with empty-object input schema.
- Raw item/tool/call: 1, tool rah_tool_0, arguments {}, request id 0.
- Neutral ToolRequested: 1; authorization attempts 1, accepted 1 (normal
  ToolStarted demonstrates successful existing authorization).
- Execution starts 1; successful ToolFinished 1, is_error=false, repository
  normalized status ok and preserved WIP observed.
- Outgoing runtime result reply 1, id 0, success=true, actual status content.
- Completed turn 1, assistant output includes RAH510B_TOOL_OK. No extra Tool.
- Shutdown Ok(Ok(())); alive=false; before/after main measurements equal.
Integration diagnostic test exit 0 / 1 passed, 56.64s. This test itself is an
observation harness; its green exit alone is not whole R5 certification.

Companion process evidence: bundle main PID 13968 (parent test PID 15944);
companion PID 15716 parent 13968, executable inside the frozen bundle.
Observed separately pre-existing installed-package companion PID 7924 parent
16168; not descended from the certification main and not used or terminated.
No npm/PATH fallback in the observed certification process tree. Wire capture
contains zero missing-host warnings. Successful real raw Tool round trip and
bundle-local companion execution corroborate functioning code-mode prerequisite.
The harness retains stderr privately inside ProcessTransport but does not export
normal-exit stderr; no claim of a separately retained complete stderr audit.
Process samples retained in live-processes.json/live-process-summary.json.

Next authorized gate: snapshot_cancellation with RAH_R5_BUNDLE=1. Narrow harness
selection uses the same frozen main identity, original lighthouse prompt,
original 180s gate and 20s shutdown limits. No runtime code changed. Fresh active
Started and ModelRequestStarted observed, then neutral turn.control.cancel()
returned RuntimeFailure diagnostic operation=Cancellation, kind=ProviderRejection,
rpc_code=-32600. Expected Stopped/Cancelled lifecycle was NOT established.
First stable-source failure preserved in cancellation.log, exit 101, 0 passed /
1 failed, 39.12s. No retry, no subsequent live gate. Cancellation's process-local
cause was not separately dumped by the existing harness; the retained log proves
the sanitized failure envelope, not successful Task 498 diagnostic certification.
Static inspection locates the request at experimental.rs Control::cancel:
turn/interrupt with threadId and turnId, provider rejection mapped to Cancellation.
This is failure-layer evidence only; the exact provider rejection reason is not
established and no protocol repair is authorized or performed by this task.

Cancellation shutdown Ok(Ok(())), alive=false, main POST identity unchanged.
Host scope revoked and drained by the harness. Expected cancelled state and
subsequent fresh-lease acceptance were not reached, so no stale-lease certification
claim. Final CIM inventory: no residual bundle process. Cancellation does not
imply rollback. Final independent bundle identities in final-identities.json
retain both exact full hashes, lengths and file IDs listed above.

Final combined classification:
**D — CANCELLATION / DIAGNOSTIC CERTIFICATION FAILED**.
Repair-step classification B remains true; Tool gate passed, so no T1-T6 failure.
Diagnostic invalid-model gate, real Desktop turn, fresh production rejection,
fresh 0.157.1 baseline verification and full protocol matrix: NOT RUN after STOP.
Workspace check/test/full Clippy, canonical Desktop suite, frontend/static closure,
Tauri 47/47/47/47/47 inventory, metadata 14 packages/all 0.33.0/edition 2024 and
HostExplicit executable closure: NOT RUN; final invariants are not newly certified.
Pre-live scoped metadata/check/Clippy/fmt/diff PASS remain scoped evidence only.
Final fmt/diff/status review follows; no repair to cancellation attempted.

Only three starting WIP files changed by this task: certification_support.rs
(digest rejection tests), task510b_snapshot.rs (read-only identities and bounded
frozen-bundle selection for existing live tests), and this historical report.
All other starting WIP paths are byte-identical by starting-hashes.json comparison,
including Cargo.lock and all authority/process/runtime diagnostic WIP. No WIP
removed, no bundle bytes modified or recreated, no dependency/ADR/authority/security
change, no admission/baseline/timeout change. Existing HostExplicit source untouched;
no executable exactly-11 revalidation claimed. Staging remains empty.
HEAD and local origin/master remain 3a11a315f1f5fc8a2bd6c3ed324f794c82da570d.
No commit, push or new CI; checkpoint CI 37317133407 remains the starting evidence.
No tag/release. Task 510C NOT AUTHORIZED because R5 A plus exact-head CI was not
achieved; no Task 510C work started. Suggested next task: separately authorize
bounded diagnosis of fresh-turn cancellation rejection, retaining this first
failure and capturing raw cancellation response plus process-local typed cause.

Final review: cargo fmt --check PASS; git diff --check PASS; git status and
complete diff stat retained; staged path count 0. Final-checks.json records
exit codes 0/0. The cancellation failure remains unresolved and preserved.
