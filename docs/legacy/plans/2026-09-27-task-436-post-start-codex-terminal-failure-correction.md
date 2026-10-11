# Task 436 — Localize and Correct the Post-Start Codex Terminal Failure

Date: 2026-09-27

Type: root-cause localization; no production correction warranted

Outcome: **B — POST-START FAILURE IS EXTERNAL / CONFIGURATION-OWNED**

## 1. Starting checkpoint

- Required HEAD and fetched `origin/master`: `3b99621f0ad2103bd76e5c8538e3273f3be1bc9e` — PASS.
- `origin/master...HEAD`: `0 0` — PASS.
- `git merge-base --is-ancestor origin/master HEAD`: PASS.
- Starting `git status --porcelain=v1`: empty — clean worktree and index.

## 2. Task 435 exact-head CI and known facts

Task 435 was based on `67190ba9a888881937999a20b009d674248b8170`, subject
`docs: audit production Desktop chat failure`. Its exact-head CI disposition
was PASS (as supplied in the Task 436 checkpoint). Its verdict was **A —
PRODUCTION CHAT FAILURE REPRODUCED**.

Task 435 established successful Connect and `connection_published`, accepted
production `send_chat`, successful generation/currentness checks, a returned
runtime handle, `thread_start`, partial assistant text `RAH`, no Tool activity,
and a still-running certified Codex `0.149.0` process after failure. PATH
Codex `0.157.1` was not used. The sanitized record did not distinguish a
failed Codex turn, a connection/receiver fault, or stream exhaustion. Effective
model/provider was not recorded there.

## 3. Unchanged-source build and baseline verification

At Task 436's starting SHA, `cargo build --release -p rah-desktop` passed.
The unmodified `target/release/rah-desktop.exe` SHA-256 was
`FDACBC4D280B5A759F6292FD3B554ACED1B3649FB629D7E4DACA41AA15B31D18`, matching
Task 435's recorded executable. `scripts/codex-baseline.ps1 verify 0.149.0`
reported `verified baseline 0.149.0`.

## 4. Pre-fix production reproduction

The first Task 436 production form submission failed after `thread_start`.
Its evidence showed a repository selected, 15 repository Tools advertised,
and no Tool request or Tool lifecycle event. The user reported the visible
terminal as **Chat failed**. This run reproduced the broad post-start failure
but had no private diagnostic instrumentation.

The required exact prompt used for the instrumented neutral runs was:

```text
Reply exactly:

RAH436_CHAT_OK

Do not use tools.
```

Three subsequent `New Conversation` runs in the diagnostic Desktop had no
repository selected. All three failed in the same post-start manner. No model
delta was recorded in those neutral runs. Tool activity was absent.

## 5. Certified executable identity

The certified baseline verified as `codex-cli 0.149.0`. Windows process
inspection during live runs showed the Desktop child at
`%LOCALAPPDATA%\codex-baselines\0.149.0\codex.exe`, running
`app-server --stdio`. PATH Codex `0.157.1` was not selected. The child remained
alive after terminal failure and accepted later fresh thread/turn starts.
This establishes process survival and later protocol use, not model-call
success.

## 6. Private diagnostic method

Existing tracing was present but no Desktop tracing subscriber/log sink exposed
the private terminal detail. A temporary investigative build was made in the
ignored `target/task436-diagnostic` directory. A separately ACL-restricted
local diagnostic file recorded only terminal status, Codex error category,
Desktop event path, effective model/provider, receiver/fault category, and
model-delta lengths. A second ACL-restricted local file briefly held the
`turn/error/message` solely for in-memory classification; it was cleared after
each analysis. No raw message was printed, committed, or retained in the final
artifact. All temporary source instrumentation was reverted.

The actual form was exercised through Windows UI Automation using the
production `Connect Codex`, `New Conversation`, `chat-prompt`, and `chat-send`
controls. No command handler or direct `send_chat` invocation substituted for
the form. Fresh sanitized evidence and separate private diagnostic paths were
used.

## 7. Exact terminal cause and safe error classification

The pinned Codex 0.149.0 app-server emitted:

```text
turn/completed
turn.status = failed
turn.error.codexErrorInfo = other
```

The terminal message was 163 characters. It was not retained. Local safe
classification found an HTTP 400 request rejection and authentication/account
terms. The message did not match a specific invalid-credential/account-denied
relation, model-unavailable/deprecated signal, usage/rate-limit signal, or
service-overload signal. Therefore the precise authentication subtype remains
unknown; the supported category is **Codex upstream HTTP 400 with an
authentication/account-related request rejection**. No raw Codex text is
published here.

The same `codexErrorInfo=other` and failed status recurred in all three neutral
fresh conversations.

## 8. Protocol chronology

Observed chronology for a neutral diagnostic run:

1. `connection_started` and `connection_published` succeeded.
2. The `thread/start` response succeeded; its effective model/provider were
   `gpt-6-luna` / `openai`.
3. The `turn/start` response succeeded. The Desktop's `thread_start` evidence
   is emitted after `runtime.start` returns a handle, which occurs after both
   correlated start responses.
4. No `item/agentMessage/delta` notification was recorded in the neutral run.
5. A matched `turn/completed` notification arrived with `status=failed` and
   `codexErrorInfo=other`.
6. The runtime emitted `AgentEvent::Failed(code=Internal)`; Desktop consumed
   that event and rendered the existing sanitized `Chat failed` state.

No connection `Fault`, broadcast receiver error, `RecvError::Lagged`, or
stream-exhaustion-without-terminal event occurred in the instrumented runs.

## 9. Root-cause ownership

This is an upstream Codex/model-service or inherited account/configuration
failure, not a RAH terminal interpretation defect. RAH successfully starts the
thread and turn, receives Codex's failed terminal notification, and translates
it to `AgentEvent::Failed(Internal)`. The Desktop then maps that failure to
its closed frontend error code and displays **Chat failed**. That mapping does
not pretend the turn succeeded or expose Codex's private text.

The evidence does not show that RAH supplied a model override: the Desktop used
inherited Codex configuration, and the effective configuration reported by
the pinned server was `openai` / `gpt-6-luna`. No model-unavailable signal was
found in the private terminal message.

## 10. Deterministic reproduction

The upstream terminal behavior reproduced in three fresh neutral production
chat conversations on the same connected Codex 0.149.0 app-server child. Each
produced `turn/completed(status=failed, codexErrorInfo=other)` and the same
sanitized Desktop terminal state. This is live deterministic evidence of the
failure category, not a deterministic test fixture.

No RAH-owned defect was found, so no fake adapter correction or misleading
success-path regression test was added.

## 11. Implementation correction

No production implementation correction was warranted. The correct adapter
behavior for a Codex failed turn is to surface failure. RAH did not suppress,
retry, replay, or convert the failed turn to success.

## 12. Diagnostic taxonomy changes

No permanent diagnostic taxonomy or user-facing error contract change was
retained. Temporary diagnostics proved the emitted `AgentEvent::Failed`
branch and were removed after classification. Existing frontend output remains
privacy-safe and closed.

## 13. Tests and validation

The unchanged release build passed before reproduction. The temporary
investigative Desktop release build compiled successfully, and `cargo fmt
--check` passed while it was present. No implementation tests were added or
run because no RAH behavior was changed. Temporary source instrumentation was
reverted and source files returned to the starting blob content.

Documentation-only close validation is required before commit:

- `git diff --check` — PASS.
- `cargo metadata --no-deps --format-version 1` — PASS: 13 packages,
  13 workspace members, version `0.32.0`, edition `2024`.

## 14. Three neutral live runs

All three diagnostic fresh conversations used the production Desktop form,
certified Codex 0.149.0, inherited configuration, no selected repository, and
the exact no-tool prompt above. All three **FAILED** with the same terminal
category. These are failure reproductions, not post-correction pass
certification. Since no RAH code was corrected, the three-pass release gate is
not applicable and no green run is claimed.

## 15. Repository-context live result

The first uninstrumented Task 436 production run had a repository selected and
failed after `thread_start` without Tool activity. It is a contextual failure
observation, not a repository-context certification. No separate repository
context no-Tool pass was reached or claimed.

## 16. First model-selected Tool result

Not reached. The neutral chats failed before any Tool request. Tool
advertisement in the repository-selected run is not evidence of model-selected
Tool dispatch. No Tool dispatch behavior is claimed.

## 17. Security and privacy implications

No credentials, tokens, raw Codex messages, raw stderr, prompts, generated
responses, account identifiers, or private configuration values were included
in the committed artifact. The effective model/provider identity is recorded
because it was directly returned by the pinned app-server and was needed to
assess the inherited configuration hypothesis. Local raw error text was
cleared after classification. No authority, permission, provider trust, or
repository boundary changed.

## 18. Nonclaims

This result does not prove the exact account credential subtype, model
availability beyond the absence of a matching error signal, behavior on other
Codex versions or platforms, general service health, successful production
chat, repository-context chat success, or model-selected Tool dispatch. Task
435's partial text `RAH` remains recorded there; the Task 436 neutral diagnostic
runs emitted no captured model delta.

## 19. Final outcome

**B — POST-START FAILURE IS EXTERNAL / CONFIGURATION-OWNED.** The pinned
Codex app-server itself reported the turn failed with an HTTP 400
authentication/account-related request rejection. RAH's protocol and Desktop
terminal handling were correct for that failed turn. No RAH code fix is
justified by this evidence.

## 20. Exact next task recommendation

**Task 437 — Verify inherited Codex authentication/account and provider
configuration, then recertify neutral production chat.** Keep Codex pinned to
0.149.0. Do not change RAH terminal handling based on this incident. Once the
external configuration is verified or corrected by the host owner, repeat the
neutral production form gate and only then resume repository-context or Tool
certification.

Throughout Task 436, v0.33 product capability remains **NONE SELECTED** and
HostExplicit remains exactly 11.
