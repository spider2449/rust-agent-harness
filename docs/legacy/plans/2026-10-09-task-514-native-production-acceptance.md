# Task 514 — native production acceptance and release readiness

## Scope and checkpoint

Verify production Desktop controls, native llama.cpp chat, OpenAI error recovery,
switching, Tool authorization, cancellation, composition and release documentation.
No source correction, version change, commit, tag or publication is authorized here.
Stop the release gate on a production defect and classify the smallest correction.

Starting worktree clean. HEAD, origin/master and live GitHub master equal
`4f6315224c2d813dad6503a890e565de52723188`.
GitHub runs 37804317343 and 37804313253 independently checked: success at that SHA.
Task 513 exact-head quality baseline is retained; no legacy compatibility rerun.

## Sequence

1. Inspect production composition, authoritative boundaries and acceptance evidence.
2. Build the normal release Desktop at the checkpoint and launch it visibly.
3. Inspect Windows UI automation accessibility; exercise actual controls if reliable.
4. Record exact local manual steps for any unmeasurable behavior, with no manual PASS.
5. Audit security and documentation; report outstanding gates and version recommendation.

## Evidence

Results and manual instructions will be recorded below. Backend-only prior results
do not certify Task 514 UI acceptance.

### Measured production launch

`cargo build -p rah-desktop --release` exited zero (optimized, 3m45s).
It emitted three dead-code warning groups for uncompiled legacy branches; this
was not a warnings-denied build. Task 513 exact-head CI remains the quality baseline.
The normal dependency tree includes rah-runtime-openai, excludes rah-runtime-codex;
default features are provider-openai and provider-llamacpp. No fixture feature.
Binary SHA-256:
`123D395E0D4E56A9DC01C8D8ADA80765C29D786E3E69E9811C4094B26F600D52`.

Visible launch: PID 3576, responding RAH window, started 2026-10-09 03:16:42 +08:00.
Screenshot `target/task514/desktop.png` shows Desktop UI ready, application 0.33.0,
Windows shell ready, no Trusted Profile selected and zero external providers.
The observed direct child was msedgewebview2.exe PID 1992.
Bounded before/after identity observations for codex.exe and codex-code-mode-host.exe
are in `target/task514/codex-before.json` and `codex-after.json`. These observations
cover launch only, not unexecuted chat or every possible transient process.
Snapshots are byte-identical: two pre-existing identities, codex PID 8860 and
codex-code-mode-host PID 5472, unchanged; identity delta zero. An initial summary
incorrectly wrapped the decoded array as one nested object (reported 1/1/new 1);
direct enumeration and snapshot hashes corrected that diagnostic, without relaunch.
The launched window was closed normally after observation.

Windows UI Automation inspection (`target/task514/ui-tree.json`) exposed sixteen
descendants, all panes; no usable ComboBox, Edit, Button, Value, Invoke or Selection
patterns for actual provider/chat controls. Screenshot proves rendering, not chat.
Coordinate/key guessing cannot reliably establish control state, streamed deltas,
conversation identity or Tool lifecycle. No backend IPC substitute was used.

Existing user llama-server PID 16436 was healthy at http://127.0.0.1:8080 and
advertised one model: `F:\llama_cpp_models\ornith-1.5-35b-Q4_K_M.gguf`.
Only health/model discovery was requested here; no new server or model download.
OPENAI_API_KEY presence checked without values at process/user/machine scopes:
all absent. **OPENAI_LIVE_NOT_VERIFIED**. No OpenAI API request made.

### Acceptance status

| Required gate | Task 514 result |
| --- | --- |
| Production build and visible launch | PASS |
| Actual llama.cpp UI Connect / first streamed answer / second turn | NOT VERIFIED |
| llama.cpp UI Disconnect / Reconnect / another answer | NOT VERIFIED |
| OpenAI UI model configuration / missing-key recovery | NOT VERIFIED |
| OpenAI real API acceptance | OPENAI_LIVE_NOT_VERIFIED |
| UI provider switching / connected-switch rejection | NOT VERIFIED |
| Production model-selected authorized Tool round-trip | NOT VERIFIED |
| UI cancellation / Disconnect / Reconnect / successful next turn | NOT VERIFIED |
| Default native composition without Codex | PASS (build/dependency and launch scope) |
| HostExplicit 11 / security boundaries | Source review PASS; live UI authority gate outstanding |

Task 513 backend live llama.cpp chat and echo Tool lifecycle and deterministic
OpenAI/switch/cancel/revocation evidence remain prior evidence, not Task 514 UI PASS.
No real production behavior defect was observed; B/C cannot be inferred from
inaccessible automation. No source correction was attempted.

## Exact Windows-local user validation

Keep credentials out of recordings, terminal output and diagnostic artifacts.
Use the existing llama-server; do not start or download another server/model.
Launch this artifact from PowerShell:

```powershell
Set-Location 'F:\coding\otherPrj\rust-agent-harness'
Start-Process -FilePath '.\target\release\rah-desktop.exe'
```

1. Scroll to Runtime. Select **llama.cpp** under Provider. Set endpoint to
   `http://127.0.0.1:8080`, leave Model empty, click **Apply Provider Configuration**.
   Click **Connect Runtime**. Record resolved model and visible Connected state.
2. Scroll to Chat. Send `Reply with exactly: RAH_LLAMA_OK`. Record visible text
   accumulating before completion and final exact marker; an instant final answer
   alone does not prove visible streaming. Send a second message in the same chat:
   `What exact marker did I ask you to reply with in my previous message?`
   Require `RAH_LLAMA_OK` and preservation of both messages. Record any failure.
3. While connected, verify provider configuration cannot be applied. Disconnect
   using the runtime connection control. Reconnect and send
   `Reply with exactly: RAH_LLAMA_RECONNECT_OK`. Require streamed successful answer.
4. Disconnect. Select **OpenAI**, type a valid account-supported model identifier
   into the native Model field, and Apply Provider Configuration. Do not use the
   legacy Codex Model Provider controls. Click Connect Runtime with the key absent.
   Require the sanitized message explaining OPENAI_API_KEY configuration/restart,
   no permanent disabled Connect control and no Codex fallback. Repeat configuration
   or Connect once as a deliberate recovery check; record both states.
5. Return through Disconnect if available, select llama.cpp, restore endpoint/empty
   model, Apply, Connect and send `Reply with exactly: RAH_SWITCH_OK`. Require fresh
   connected state and successful chat. Use New Conversation to test fresh chat
   separately; record any history shown and whether replay was explicitly requested.
6. For a harmless Tool round-trip, disconnect, select this repository using the
   Desktop repository picker, retain existing host permission policy, and reconnect
   llama.cpp. Inspect Effective Authority for advertised/authorized `repo.status`.
   Send `Use the repo.status Tool exactly once to inspect the selected repository.
   Do not mutate anything. After the Tool result reply with exactly: RAH_TOOL_OK`.
   Require correlated Tool requested/started/finished activity, actual read-only
   result, and model continuation. Prose promising a Tool call is not PASS. A manual
   HostExplicit invocation alone does not prove the Model-to-HostToolPort route.
7. Send `Write a long numbered explanation of the numbers 1 through 300.` Once
   text is visibly streaming, click **Cancel Turn** (the Send button changes label).
   Require termination and enabled controls. Disconnect, reconnect, send
   `Reply with exactly: RAH_CANCEL_RECOVERY_OK`, require success and disconnect.
8. If an authorized OpenAI key later becomes available, configure it securely only
   in the backend launch environment and restart Desktop. Through native controls:
   OpenAI/model/Apply, Connect, `Reply with exactly: RAH_OPENAI_OK`, verify streaming,
   Disconnect, Reconnect, `Reply with exactly: RAH_OPENAI_RECONNECT_OK`, Disconnect.
   Otherwise retain OPENAI_LIVE_NOT_VERIFIED and obtain an explicit release decision.

For each workflow retain timestamps, visible states, sanitized errors and activity
correlation. Record permission/profile/repository state before and after switching;
no widened permission or retained executable authority is acceptable. Source review
and screenshots alone cannot prove absence of stale leases; retain lifecycle evidence
where available. Any failed required behavior stops the release gate for classification.

## Authority and documentation audit

Reviewed README, ARCHITECTURE, ARCHITECTURE_GUARDRAILS, SECURITY, CHANGELOG,
ADRs 0032/0033, Task 513, and v0.33 release conventions. Source checks confirm
official OpenAI fixed origin with no redirects/proxy/retries, separate key resolution,
loopback-only llama.cpp validation, backend-only presence presentation, disconnected
configuration under lifecycle coordination, and revocation before runtime teardown.
HostToolScope validates active leases before authorization; no public API, dependency,
ADR, Trusted Profile, repository/worktree or permission change in this task.
HostInvocationKind and its exact allowlist test still contain the existing eleven
names; no HostExplicit expansion. This audit does not certify unexecuted UI lifecycle.

README already explains native selection, endpoint/model discovery, both credential
variables, session-scoped configuration, restart after credential changes, optional
legacy Codex and no automatic fallback. Architecture/security already describe native
composition and host authorization. Required release documentation work remains:

- Add a new release changelog covering native production providers and Task 513
  corrections; current top entry is the immutable v0.33.0 released record.
- Create a new release gate with exact artifact/CI/UI evidence, platform scope,
  model/template Tool limitation and explicit OpenAI live result or approved decision.
- Add a human acceptance/troubleshooting walkthrough for Apply/Connect, missing keys,
  server loading, cancellation and recovery; do not turn prior backend tests into UI PASS.
- Reconcile Tauri bundle metadata `tauri.conf.json` version 0.9.0 with workspace 0.33.0
  under the next preparation task; establish whether the existing divergence is intended.
  No installer was built/tested here and no packaging defect is claimed from metadata alone.
- Preserve unsupported OpenAI discovery/provider-native continuation, llama.cpp
  Responses/model-template requirements, and historical legacy Codex nonclaims.

Recommend **v0.34.0**, consistent with the project's minor release convention for
new production capability/composition. No version changed; no release prepared.

## Disposition

**F — VALIDATION INFRASTRUCTURE FAILURE**: actual Desktop controls cannot be measured
reliably with available automation. OpenAI live evidence is also outstanding (D),
and release documentation/packaging review remains open. **Release preparation is
not authorized.** Next work is Windows-local production acceptance using the steps
above, then explicit OpenAI release decision/evidence and bounded release preparation.
Only this plan/report changes tracked workspace content; build/evidence remain ignored
under target. No commit, push, version bump, tag, publication or Codex investigation.
