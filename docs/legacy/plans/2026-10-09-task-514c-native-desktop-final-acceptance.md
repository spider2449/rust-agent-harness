# Task 514C — native Desktop final acceptance

## Scope and plan

Acceptance only: retain Task 514B production and quality evidence; verify actual
Desktop model-selected Tool execution and cancellation/recovery with human input.
No production code, version, Tool, dependency, ADR or authority changes. No release,
tag, Codex compatibility, VCTIP investigation or new UI automation framework.

1. Verify checkpoint, artifact, historical report and exact-head CI.
2. Review production Tool/permission/lifetime and endpoint/credential boundaries.
3. Prepare a disposable read fixture and manual production UI evidence launch.
4. Inspect reported UI results and recorded lifecycle evidence before closing gates.
5. Record release-preparation requirements without starting Task 515.

## Verified checkpoint and retained evidence

Starting HEAD = origin/master = live GitHub master:
`5afc73abcda5b8b8fa6f35ee219b7eddfbad8d4b`.
Verified with local rev-parse and live git ls-remote. GitHub run
[37859204714](https://github.com/spider2449/rust-agent-harness/actions/runs/37859204714)
was queried live: completed/success at this exact SHA.

Retain Task 514B classification A and actual Windows Connect, first response,
second turn, Disconnect/Reconnect and missing-key recovery back to llama.cpp PASS.
Historical evidence remains in target/task514b/final-ui and the tracked 514B report.
The existing untracked Task 514 report remains untouched and unstaged; SHA-256:
`DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43`.

Production target/release/rah-desktop.exe hash verified against final 514B report:
`49B5FA68D4E13D71ECDEC6CBA42C6C3B046D28BF2B29393DE323C973CF6BD577`.
Local llama.cpp /health returned status ok. No inference request sent by the agent.
No existing rah-desktop.exe process was observed during preparation.

Reuse final 514B gates: native adapters 23, neutral runtime 18, Desktop 335,
workspace 1074 passed/18 ignored, nine frontend suites; fmt, workspace check,
strict clippy and diff-check PASS. Reviewed final-gate-exits.txt and exact-head CI.
No unrelated suites rerun; no production source changes justify rerunning them.

## Authority/security review

Reviewed README, architecture guardrails, native architecture/security sections,
accepted ADR 0033, production composition and HostToolScope request_live path.
The host binds the neutral conversation to a revocable scope. Active accepting
session validation precedes authorization; authorization precedes Started and
authorized execution. Requested/Started/Finished internally share the host call ID;
Desktop activity correlates that ID internally, while existing JSONL evidence
projects runtime/session/repository generations rather than raw call IDs.
Consequently generation counts alone are not full Tool-result proof: require one
named UI Activity sequence and the unseen file token in the final model response.

Host-selected repository admission supplies the existing None/Read/Execute policy.
This test uses only fs.read (Read); it grants no additional mutation authority.
Effective Authority is informational, not an authorization bypass. HostExplicit
remains eleven per unchanged inventory and retained passing exact-allowlist tests.
Trusted Profiles, worktree selection, stale-handle rejection, conversation ownership,
credential redaction and permission boundaries are unchanged; actual new UI lease
and recovery observations remain pending.

OpenAI fixed origin and disabled redirects/proxy/retries, separate backend keys,
llama.cpp loopback-only endpoint, and default native features excluding the optional
Codex dependency remain unchanged. No native-path Codex launch is introduced;
no fresh process-lifetime proof is claimed before the manual run.
OPENAI_LIVE_NOT_VERIFIED remains explicit. No API credential or ChatGPT session
credential is used. Public release needs separate live acceptance or a documented
exception approved by the human release decision.

## Manual acceptance procedure

The user agreed to run the manual steps; no result has yet been reported.
Ignored preparation artifacts: target/task514c/launch-manual.ps1 and a new disposable
Git repository target/task514c/fixture-repository. No fixture commit or index mutation.
acceptance-marker.txt is untracked UTF-8 with a random token, omitted from the prompt.
Initial file SHA-256:
`9FA5E9A72557194C1312234FEC33C6EF20226881021E97DBE8D833527554C69F`.
The fixture is retained for inspection, not deleted after testing.

Close any existing Desktop normally, then run:

```powershell
Set-Location 'F:\coding\otherPrj\rust-agent-harness'
& '.\target\task514c\launch-manual.ps1'
```

The launch checks the accepted binary hash, refuses to overwrite prior launch
evidence, and sets only child-process evidence output and absent OpenAI key.
It records process creation identity without enabling remote debugging or automation.

1. While disconnected, Choose Repository:
   F:\coding\otherPrj\rust-agent-harness\target\task514c\fixture-repository.
2. Select llama.cpp, endpoint http://127.0.0.1:8080, empty Model; Apply Provider
   Configuration and Connect Runtime. Refresh Authority. Require active fixture,
   fs.read advertised with Read permission. Do not change profiles or authorize mutation.
3. Send: `Invoke fs.read exactly once with {"path":"acceptance-marker.txt"}.
   Do not invoke any other Tool or mutate anything. After receiving the Tool result,
   reply with the exact file contents. Do not guess the contents.`
4. Record one fs.read Requested/Started/Finished sequence and final assistant token,
   timestamps and sanitized errors. Do not use a manual HostExplicit invocation.
   No request is NO_TOOL_REQUEST: model/template behavior is distinct from dispatch
   failure; do not broaden permissions or retry to manufacture PASS.
5. Send: `Write 500 numbered paragraphs explaining a different programming concept
   in each paragraph.` Click Cancel Turn during visible streaming. If already
   complete, record NOT_TESTED. Require stopped streaming and actionable controls.
6. Before disconnecting send `Reply with exactly: RAH514C_CANCEL_RECOVERY_OK`;
   require success without previous IPC/stream state overwriting the new turn.
7. Disconnect and Refresh Authority: no advertised executable Tools. Reconnect;
   send `Reply with exactly: RAH514C_RECONNECT_OK`, require success; disconnect.
8. Report Tool PASS/FAIL/NO_TOOL_REQUEST, Cancel PASS/FAIL/NOT_TESTED, both recovery
   markers, Activity counts and sanitized diagnostics. Inspect manual-host-events.jsonl
   for one intended Tool sequence, terminal count per session, fresh reconnect
   generation and no late Tool activity. Compare final token to fixture bytes and
   preserve the fixture hash/status. Raw call IDs are not present in this logger;
   do not claim captured wire-level IDs from projected lifecycle records.

## Current acceptance results

### Confirmed production authority presentation blocker

User screenshot F:\rah-01.png shows Current fixture-checkout binding, 17 Effective
Tools, zero unavailable, and both supported Host action forms and Not supported
rows. It does not show Tool names, source, dispatch permission or advertised state.
A timestamped authority-observation PNG copy is retained under target/task514c.
The screenshot corrects the earlier all-Tools interpretation: some forms are eligible.

Source confirms renderEffectiveTool creates title/details but appends only hostBox
before returning (frontend/status.js). Thus the missing Tool identities/permission
details are a concrete rendering defect, not evidence that all Tool authorization
failed. Source also hard-codes connection runtime_kind to codex and maps every
Native runtime source to native_openai (effective_authority.rs). These explain
the misleading screenshot labels; they do not prove Codex CLI execution or actual
OpenAI connectivity. Reviewed commit Authorization revoked remains distinct from
read-only Tool permission.

This is **F — OTHER PRODUCTION VALIDATION BLOCKER**, an authority presentation
defect preventing the instructed visible per-Tool authority review. Actual Tool and
Cancel gates remain unverified. No authority/security bypass or runtime execution
defect is established. Stop acceptance at this demonstrated blocker; no production
code or already-passing lifecycle code changed. Small correction scope is Tool
identity/details rendering and truthful native runtime labels, followed by focused
validation, a new production artifact and actual UI acceptance. Task 515 remains
unauthorized. Historical failed dispatch evidence remains separate and unresolved.

### Reported authority labels and observed dispatch failure

User clarification: the reported interaction was only Refresh Authority; all
Effective Tools reportedly show Not supported. No Tool prompt was sent for this
reported interaction. The older log's dispatch failure is not attributed to this
Refresh action. Source maps fs.read to a supported HostExplicit kind, so an all-Tools
Not supported display cannot be dismissed as expected for the supported eleven.
Need the fs.read row and snapshot/currentness presentation to distinguish an
observed presentation mismatch from model-route permission failure. No new Tool
execution or authorization denial is proven by Refresh Authority alone.

The user reported Authorization revoked and Host action unavailable: Not supported.
Source inspection maps Authorization revoked specifically to reviewed commit
authorization presentation, not a blanket revocation of fs.read Read permission.
Host action eligibility is a separate HostExplicit descriptor: Not supported applies
when the Tool is outside its eleven-name mapping. The exact displayed Tool name
has not yet been supplied, so no conclusion about its model-route eligibility follows.

The current manual session log records successful repository admission and
connection generation 1, followed by session 2 ToolRequested, ToolStarted and
desktop_failure with failure_stage tool_dispatch_failure, with no ToolFinished.
Later session 3 completed. A second repository identity and connection generation 2
were subsequently published; no model turn appears for generation 2 in the inspected
snapshot. A separate timestamped observed-tool-failure JSONL copy preserves this
evidence without overwriting the active log. The logger omits Tool name/input and
raw result details; it cannot establish that the failed request was the intended
fs.read fixture call. Requested user prompt, Activity name and sanitized error.
Do not retry the failed call or broaden permissions to manufacture acceptance.
The Tool acceptance gate is stopped pending exact failure identification; no
runtime correction is attempted from these labels alone. Final B versus a setup/
model-input failure remains unresolved; acceptance is not complete.

### Disposable fixture admission correction

The user reported Selected folder is not a valid repository root for the initial
git-init fixture. Inspection confirmed it has no committed HEAD and no .git/index.
It was an inadequate production admission fixture; no Tool request occurred.
The exact rejecting admission subcomponent has not been observed in backend logs,
so the missing HEAD/index observations alone are not claimed as a complete causal trace.

Preserved that fixture and token. Created a separate full local clone at
target/task514c/fixture-checkout, HEAD 5afc73abcda5b8b8fa6f35ee219b7eddfbad8d4b,
with a normal checkout/index and only acceptance-marker.txt untracked. The copied
marker retains its initial SHA-256. No commit was created and the primary index,
history and user changes were untouched. The first clone failed before creating
the destination because Git rejected source ownership. A command-scoped
safe.directory exception for the exact source .git path allowed the local clone;
no global Git or host configuration changed. This is setup correction, not a
retry of failed model/Tool acceptance. Use fixture-checkout instead of the original
fixture-repository in all manual steps. Actual Desktop admission and remaining
acceptance gates still require the user's observations.

### Manual launcher collision follow-up

The user reported the launcher's preservation guard rejecting a second launch.
Inspection found an existing manual-launch.json recording PID 15304, created
2026-10-09T07:40:49.1420031+08:00 with the accepted executable hash. Its existing
host log contains two connection publications, both without a selected repository;
it contains no Tool or turn acceptance evidence. No current rah-desktop.exe process
was observed during follow-up. This guard rejection occurred before a new launch
and is not a native runtime defect or acceptance PASS.

Preserved both original files. Corrected only the ignored manual launcher to create
a unique timestamp/UUID session directory under target/task514c for each launch,
and print evidenceDirectory. The executable hash and existing-process guards remain.
Future manual-host-events.jsonl and manual-launch.json live in that session directory;
the disposable fixture path remains unchanged. No UI framework or production source
change. Actual acceptance remains pending; relaunching after this preparation guard
does not retry a failed production acceptance workflow.

| Gate | Result |
| --- | --- |
| Actual Desktop Tool request/execution/continuation | PENDING MANUAL UI ACCEPTANCE |
| Exactly one execution and correlated result | PENDING |
| Actual active-stream Cancel and next turn | PENDING |
| Disconnect/Reconnect after Cancel | PENDING |
| Previously accepted five workflows | Retained Task 514B PASS |
| OpenAI live API | OPENAI_LIVE_NOT_VERIFIED |
| Unchanged authority/security source baseline | Retained/reviewed; new UI observations pending |
| Exact-head quality | Retained 514B PASS; live exact-head CI success confirmed |

## Required next release-preparation work

Review confirms root workspace version 0.33.0 and Desktop bundle metadata 0.9.0.
CHANGELOG and prior preparation records use workspace minor release versions
(v0.32.0 then v0.33.0), workspace-inherited package versions and corresponding
internal lock records. Recommend provisionally v0.34.0 for native production
composition; bundle divergence must be explicitly resolved during Task 515.
No versions are changed here and no installer acceptance is claimed.

Task 515, if later authorized, must address CHANGELOG native provider/lifecycle
history; README native connection and recovery guidance; ARCHITECTURE native
composition; SECURITY endpoint/key/host-authority rules; a new release acceptance
record with exact artifact, CI, Windows UI evidence, model/template limitations
and explicit OpenAI live decision; cancellation/missing-key/server recovery guidance;
workspace/package/lock version conventions and Desktop bundle version reconciliation.
Existing native instructions/security sections already cover much of this behavior;
review/update only where needed, preserving immutable release history.

## Open disposition

**F — OTHER PRODUCTION VALIDATION BLOCKER.** Confirmed authority presentation defect;
actual Tool/cancellation manual UI acceptance also remains outstanding.
Task 515 release preparation is not authorized while the actual Tool and Cancel
gates remain open. Final HEAD remains the starting checkpoint; no commit/push,
release, tag, version bump or production change. Only this new report is proposed
repository documentation; historical Task 514/514B records remain unchanged.
No closure commit is appropriate before successful actual acceptance.
