# Task 435 — Production Desktop Chat Failure Reproduction and Root-Cause Audit

Date: 2026-09-27

Type: production failure reproduction / root-cause audit

Outcome: **A — PRODUCTION CHAT FAILURE REPRODUCED**

## 1. Starting checkpoint and Task 434 evidence

The required start gate passed before investigation:

| Check | Result |
| --- | --- |
| `git status --short` | Empty; clean worktree and index |
| `HEAD` | `67190ba9a888881937999a20b009d674248b8170` |
| fetched `origin/master` | `67190ba9a888881937999a20b009d674248b8170` |
| `origin/master...HEAD` | `0 0` |
| `git merge-base --is-ancestor origin/master HEAD` | PASS |

Task 434's exact-head CI run `36296582548` was reported PASS. Task 434's
disposition was **C — NORMAL-USE PROBE INCONCLUSIVE**. Its artifact records
that no production chat prompt was submitted because its automation could not
focus the WebView. The separate human report that production Desktop chat
fails was treated as new evidence; Task 434 was not amended or rewritten.

## 2. Exact production build and host

The unmodified source at the required checkpoint built successfully with
`cargo build --release -p rah-desktop`.

| Item | Evidence |
| --- | --- |
| Source SHA | `67190ba9a888881937999a20b009d674248b8170` |
| Executable | `target/release/rah-desktop.exe` |
| Executable SHA-256 | `FDACBC4D280B5A759F6292FD3B554ACED1B3649FB629D7E4DACA41AA15B31D18` |
| Windows | Windows 10 IoT Enterprise LTSC 2024, build `26100`, x64 |
| Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `x86_64-pc-windows-msvc` |
| Cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Tauri crates | `tauri 2.11.5`; `tauri-build 2.6.3`; `tauri-runtime-wry 2.11.4`; `tauri-plugin-dialog 2.7.2`; `wry 0.55.1` |
| Codex on PATH | `codex-cli 0.157.1` |
| Baseline verification | `scripts/codex-baseline.ps1 verify 0.149.0` — verified |

The release executable was launched from the repository working directory. A
new temporary JSONL evidence path was configured for that process; the path is
omitted here because it is local evidence storage. Only sanitized records are
included below. No credentials or private configuration were copied into the
evidence file or this artifact.

## 3. Actual Codex executable selection

`RAH_CODEX_EXECUTABLE` was unset for the Desktop launch. The Codex child
process spawned by Desktop was observed at:

```text
C:\Users\morefunfun\AppData\Local\codex-baselines\0.149.0\codex.exe
```

That executable reported `codex-cli 0.149.0`; its SHA-256 was
`14B7E6B2356E82D1D9275579EAA588757B4E0A501B65DCC19FCCDF77BD83DC00`.
This establishes that the run used `certified_baseline`, not PATH. PATH's
`0.157.1` version is not causal evidence for this failure.

## 4. Connection and production state

Before Connect, the Desktop showed: Codex disconnected; no profile remembered
or loaded; no repository selected; repository Tools inactive; model
configuration inactive. The provider selector showed **Use Codex
configuration**, and the model field was blank (inherited host configuration;
the configured model/provider identity is not recorded here).

Connect was activated through the production Desktop control and succeeded.
The UI showed both RAH Runtime and Codex as connected and the chat hint changed
to **Chat ready**. The evidence records `connection_started` then
`connection_published` for connection generation 1, with no selected profile
or repository. The certified baseline child remained running after the chat
failure. No authentication error was shown during Connect; this does not rule
out a later model/account/service failure.

## 5. Reproduction 1 — minimal chat without repository Tool use

The real production textarea and Send button were used. The exact prompt was:

```text
Reply exactly:

RAH435_CHAT_OK

Do not use tools.
```

The prompt appeared in the production transcript and `send_chat` accepted it.
The sanitized evidence contains `thread_start`, which is emitted after the
runtime returns a handle for the started thread and turn. Therefore this was
not a synchronous `send_chat` rejection, stale-generation rejection, or
thread/turn-start failure.

Observed result:

| Surface | Result |
| --- | --- |
| Prompt transcript | Present with the exact submitted text |
| Running state | Turn started and later returned to the ready state |
| Assistant transcript | Partial text `RAH` |
| Visible terminal error | `Chat failed` |
| Chat terminal | Failed; no successful completion observed |
| Activity / Tool events | Activity panel empty; no Tool requested, started, or finished |
| Live evidence stage | `terminal_disconnect_failure` |
| Cancel/Send | The running turn ended; chat returned to ready. No cancellation was issued. |

Sanitized live JSONL records, in observed order:

```jsonl
{"event":"desktop_registry_composed","selected_repository":false,"relevant_tool_names":[],"branch_creation_authority_present":false,"deletion_authority_present":false,"directory_creation_authority_present":false,"rename_authority_present":false,"registry_contains_repo_create_branch":false,"registry_contains_repo_delete_file":false}
{"event":"connection_started","connection_generation":1,"model_generation":0,"profile_generation":0,"repository_generation":0,"repository_fingerprint":null,"selected_profile":false,"selected_repository":false,"bridge_enabled":true,"identity_generation":0}
{"event":"connection_published","connection_generation":1,"model_generation":0,"profile_generation":0,"repository_generation":0,"repository_fingerprint":null,"profile_active":false}
{"event":"tool_advertised","public_tool":"echo","private_alias":"echo","dynamic_definition_emitted":true}
{"event":"thread_start","connection_generation":1,"model_generation":0,"repository_generation":0,"repository_fingerprint":null,"runtime_generation":1,"session_generation":1}
{"event":"desktop_failure","failure_stage":"terminal_disconnect_failure","connection_generation":1,"model_generation":0,"repository_generation":0,"repository_fingerprint":null,"runtime_generation":1,"session_generation":1}
```

`tool_advertised` records a definition sent to Codex; it is not evidence of a
model Tool request. No repository was selected and no repository Tool was
available for this turn.

## 6. Failure-stage localization and bounded unknown

The production path passed form submission, synchronous `send_chat` checks,
connection-currentness checks, runtime invocation, and thread/turn start. The
failure occurred after model output began, while the runtime turn was expected
to produce its terminal event. The frontend received the closed
`chat_runtime_failed` contract, rendered as **Chat failed**.

The current `terminal_disconnect_failure` label does not distinguish the
underlying terminal cause. In `main.rs`, it is used both when an adapter
`AgentEvent::Failed` carries `AgentErrorCode::Internal` and when the Desktop
event stream ends without a terminal event. The Codex adapter maps a failed
`turn/completed` notification and app-server connection faults to an internal
failure. The sanitized JSONL intentionally omits that private message, and no
Desktop log file was present in the inspected app-data root. The live record
therefore does **not** establish which of these occurred:

- Codex reported a failed turn after emitting partial text;
- the app-server transport or protocol faulted after partial text; or
- the event stream ended without a terminal notification.

The exact upstream/private error is a bounded unknown. This run does not
support an authentication diagnosis, a model-selection diagnosis, a Codex
version-skew diagnosis, or a RAH authorization diagnosis. The certified
`0.149.0` executable was selected; PATH `0.157.1` was not used.

## 7. Reproductions not reached

Reproduction 1 failed, so the prescribed later cases were not run:

- **Repository-bound no-Tool prompt:** not run. The Task 434 fixture was not
  selected or modified.
- **Model-selected repository observation Tool:** not run. No Tool dispatch
  was observed in the minimal case.

No direct Rust chat call or direct `send_chat` invoke was used as a substitute
for the production form.

## 8. Existing coverage and why it did not catch this

Reviewed coverage includes Desktop chat-state and terminal failure/cancellation
tests such as `terminal_failure_and_cancellation_restore_desktop_chat_controls`,
prompt validation and closed chat-event serialization tests, and Codex runtime
tests that use a fake app-server peer to supply `turn/completed` outcomes. An
ignored Windows live ownership test starts the real Codex app-server and proves
Connect/process ownership, but it does not submit a model turn. The reviewed
coverage therefore checks Connect, `send_chat` gates/state, adapter startup,
and mapped terminal events in separate deterministic or Connect-only paths;
it does not exercise a real model response and terminal event through the
production WebView form and real Codex process together.

The reviewed live certification record does not claim model-selected dynamic
Tool dispatch, and Task 434 did not submit any production prompt. There was no
exact real production Desktop chat completion certificate to catch this
failure before Task 435.

## 9. Root-cause verdict and classification

**A — PRODUCTION CHAT FAILURE REPRODUCED.** A real no-Tool production chat
request failed after thread/turn start and after partial assistant output. The
failure is localized to the post-start Codex runtime terminal path. No Tool,
repository, authorization, or connection-publication failure was observed.

This proves a production chat failure on the tested machine and build. It does
not yet prove whether the causal defect is in RAH adapter/terminal handling or
in the Codex model/account/service response. Connect succeeded and the
certified executable remained alive; no external failure message was captured.
The failure must not be described as an authorization defect or attributed to
PATH version skew.

Smallest next task recommendation:

```text
Task 436 — Localize and correct the post-start Codex terminal failure
```

Task 436 should use private tracing or an equivalent protected diagnostic to
distinguish failed `turn/completed`, app-server transport/protocol fault, and
silent stream closure on this path, then make only the correction supported
by that evidence. This is maintenance/reliability work for existing chat, not
a v0.33 product capability. Keep v0.33 product capability at **NONE SELECTED**.

## 10. Scope and validation

This audit changed only this plan artifact. No production Rust or frontend
source, permissions, dependencies, ADRs, versions, or release state changed.
HostExplicit remains exactly 11; the existing source test enumerates its 11
current kinds. No new authority was introduced.

Documentation-only close validation:

| Command | Result |
| --- | --- |
| `git diff --check` | PASS |
| `cargo metadata --no-deps --format-version 1` | PASS — 13 packages, 13 workspace members, version `0.32.0`, edition `2024` |

Commit, push, and exact-head CI results are recorded in the task completion
report.
