# Task 509A — Runtime adapter / Codex model-provider state separation

Starting HEAD: `44666d4de3e514339918fab4143f440d35892c8e`.
The existing untracked Task 509 report is preserved, with a reference-only note.
No Task 509 implementation commit or push preceded this task.

## Ownership audit and bounded correction

The old Desktop presentation conflated host-selected runtime identity with the
persisted upstream provider inside Codex. `DesktopModelSelection.provider`
and `.model`, the preference JSON's `model.provider` and `model.model`, and
the existing `set_model_configuration` arguments retain their historical Codex
meaning. In particular `openai` means the Codex OpenAI upstream, never native
`rah-runtime-openai`. No schema/key rename, migration, version bump, runtime
preference persistence, credential channel, or OpenAI discovery is introduced.

The audit found no persisted model shared by both runtime adapters. The Codex
preference only looked global in the frontend. Native OpenAI already obtains
its model from `RAH_OPENAI_MODEL`; the correction captures that non-secret model
at startup and supplies the same snapshot to presentation and Connect.
Missing/invalid native model configuration never falls back to a saved Codex model.

`DesktopAppState.runtime_adapter`, set by `runtime_selection::selected_adapter`
from compiled features and host configuration, is the authoritative identity.
The bounded `RuntimeSelectionState` exposes `codex`, `openai`, or `none`,
availability, model source, applicable Codex upstream provider, and current
scoped desired/configured model. Desired state is explicitly not catalog validation.
Connection state remains available through existing `app_status` lifecycle fields.

Startup stays disconnected. The frontend reads backend identity and activates the
corresponding model source. Native OpenAI and none hide the active Codex provider
value and disable Codex preference controls; the preferences remain stored.
Returning to a Codex composition restores that preference and Connect revalidates
against the selected runtime's catalog under unchanged Task 499 behavior.
Runtime selection remains startup configuration, with no hot-switch API.
Frontend configuration refreshes when backend identity changes. Changing a Codex
upstream draft clears its model and old catalog display; it does not rewrite storage.
The three Task 509 stale `gpt-6.1-sol` cases remain regression fixtures.

Active runtime composition continues to own its immutable connection/model snapshot.
Existing generation and Disconnect/recomposition boundaries remain intact.
No model catalog membership, remote entitlement, inference, or picker completion
is claimed by this state observation.

## Validation and publication

Evidence directory: `F:/temp/task509a-evidence/`.
Focused compatibility, model scoping, adapter identity, return-to-Codex and
no-provider checks precede the required full deterministic gates.
Executed Rust focused checks (all exit 0):

| Gate | Passed / failed / ignored |
| --- | --- |
| Both adapters, `task509a` | 4 / 0 / 0 |
| Serialized model-configuration presentation | 1 / 0 / 0 |
| No-provider `runtime_selection::tests` | 2 / 0 / 0 |
| Native OpenAI-only `task509a` | 3 / 0 / 0 |

Logs: `focused-both.log`, `presentation.log`, `focused-none.log`,
`focused-openai.log` under the evidence directory. Reduced-feature builds emit
unused/dead-code warnings; their focused commands exited 0.
Earlier frontend scoping/three-stale-case checks, existing model-preflight tests,
and `node --check status.js` passed before the last frontend test extension.
`cargo fmt` was executed, but this is not a claim of `cargo fmt --check` PASS.

The final extended frontend test stopped with exit 1:

```text
ReferenceError: refreshModelConfiguration is not defined
    at Object.loadStatus (evalmachine.<anonymous>:4:95)
    at async F:\coding\otherPrj\rust-agent-harness\crates\rah-desktop\frontend\runtime_model_state_test.js:51:5
```

The test extracts `loadStatus` into a Node VM without including its
`refreshModelConfiguration` dependency. The failure is in the new regression
harness; production acceptance is not established. No correction or rerun was
performed after this failure, as instructed by the deterministic-failure stop rule.

**E — DETERMINISTIC VALIDATION FAILED**

Full fmt/check/workspace-test/Clippy, canonical Desktop, complete frontend/static,
Tauri inventory, metadata, and static/executable HostExplicit gates were not run.
Workspace/Desktop counts and current executable HostExplicit certification are
unavailable. HostExplicit implementation and eligibility source are untouched;
this task does not claim a newly executed 11-name certification.
No authority/lifecycle acceptance claim is made from focused state tests alone.

Live `git ls-remote origin refs/heads/master` matched the starting HEAD during
this turn. No commit, push, exact-head CI, tag, release, version bump, Codex
runtime refresh or live OpenAI request occurred. The worktree intentionally
retains the implementation, new tests, Task 509A plan, and preserved Task 509
report with its reference-only note. It is not clean. Classification A and
Task 509B commencement are not authorized by this result.

## Authority and ADR

No new dependency or public/core API edge. No changes to ToolRegistry dispatch,
repository authority/switching, leases, permissions, Trusted Profiles, mutation
uncertainty, remembered workspaces, or HostExplicit eligibility.
HostExplicit must remain exactly 11 and is verified statically and executably.
ADR-B: clarification of existing ownership under ADRs 0002, 0015, 0030 and 0033;
no new ADR or new preference-ownership rule is needed.

## Exact Task 509B scope

Reference-only recovery follow-up: [Task 509A1](2026-10-04-task-509a1-frontend-state-test-harness-recovery.md)
records the harness dependency correction and resumed validation. This note does
not revise the stopped Task 509A evidence or its historical classification.

Complete the provider-aware Model picker using these explicit scopes: bounded
Codex catalog discovery/freshness with owned teardown and stale-result rejection;
catalog loading/empty/error UX; validated selection and Connect eligibility;
native OpenAI configured-model presentation and, if selected by that task,
non-live supported presets. Decide custom-model policy explicitly, retain fresh
Connect-time validation and Disconnect boundaries, and test absent-model and
runtime/source switches. No Codex runtime refresh, live OpenAI work, Task 510,
layout redesign, persistent native OpenAI preference, or authority expansion.

## Task 509B reference-only follow-up

[Task 509B](2026-10-04-task-509b-provider-aware-model-picker-catalog-freshness-and-connect-gating.md)
records the remaining preselection backend catalog/source contract gap and its B
disposition. This reference does not revise Task 509A's implementation, validation
evidence, historical disposition or subsequent checkpoint classification.
