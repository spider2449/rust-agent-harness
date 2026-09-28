# Task 458 — Interactive RAH normal-use workflow session

Date: 2026-09-28. Type: one production Desktop observation session.

**Disposition: B — CONCRETE MATERIAL WORKFLOW FRICTION OBSERVED.** No v0.33
capability is selected here.

## Starting checkpoint and environment

The isolated `rah-task-457` worktree was clean. After `git fetch origin master`,
both `HEAD` and `origin/master` were exactly
`d50d2edca3d3c4524d6b7cb8abcbf66099263686`. The supplied natural push CI
run `36379648272` was PASS; this session did not independently recertify it.
The original dirty RAH checkout was not edited.

The session used the production `rah-desktop.exe` release executable and its
normal Windows WebView UI. The UI displayed RAH `0.32.0`, connected RAH and
Codex runtime, certified baseline `codex-cli 0.157.1`, selected repository,
active repository tools, and later OpenAI / `gpt-6-luna` in the Model form.
The user entered prompts, operated repository and runtime controls, and
provided short observations and screenshots. No test-only hook, synthetic Tool
event, protocol injection, or internal harness control was used as a workflow
substitute. This report does not include credentials or provider configuration.

Two disposable Git repositories outside RAH were used. A was an existing clean
worker-coordinator fixture with `README.md`, `docs/`, `src/`, and `tests/`, at
baseline `fea996895b36e73402cc4275af6ca85757a71d50`. It has the tracked
`WORKER_LIMIT` symbol in `src/config.rs` and its use in `src/engine.rs`. B was a
new clean inventory-ledger fixture with the same broad directory structure and
an `available` stock calculation in `src/ledger.rs`. Both had normal Git HEADs.
They were ordinary exploration material, not fixtures designed to force a
candidate capability failure.

## Workflow observations

| Workflow | Goal and starting state | User-facing actions and Tool paths | Result and manual intervention | Workaround, complexity, friction, severity |
| --- | --- | --- | --- | --- |
| 1. Understand an unfamiliar repository | Find where A's worker limit is defined and enforced; A initially clean. | User connected and asked in Desktop chat. User reported visible repository list, search, and file-read tools. The answer cited `src/config.rs`, `src/engine.rs`, `src/main.rs`, and the test. | Correct answer; no manual intervention reported. Exact Tool event names and arguments were not retained, so `repo.file-info` use is unconfirmed. | No search workaround was needed. Literal tracked search plus repository exploration sufficed. Low complexity; **NONE**. |
| 2. Inspect repository state | Understand a staged addition in `docs/usage.md` and unstaged addition in `README.md`. These fixture changes were made outside RAH solely to create a realistic starting state. | User asked Desktop chat to explain staged and unstaged changes. The UI response excerpt called `docs/usage.md` unstaged; the user confirmed that wording. Git independently showed `M  docs/usage.md` and ` M README.md`, with no unstaged `docs/usage.md` change. | The state explanation was wrong for `docs/usage.md`. The user could continue, but had to rely on the visible Git/RAH review state to resolve the discrepancy. Exact `repo.status`, `repo.diff`, `repo.diff-staged`, and `repo.file-info` events were not captured. | Additional review was required to distinguish index from worktree. **MINOR** for this isolated answer; consequential if followed without checking. |
| 3. Small edit and review | Change only A's printed label in tracked `src/main.rs`, then inspect the diff. | User submitted the request in Desktop Chat. The RAH chat screenshot showed the changed line and an inline diff. Git confirmed `flux coordinator capacity` became `available worker slots` with no other source change. | Edit and review completed. No manual file editing was used for this change. Specific edit/review Tool event names and authority prompts were not retained. | The edit and diff were understandable in the chat. **NONE** observed for the bounded edit. |
| 4. Reviewed commit | Stage and review A's three fixture changes, then commit them. | User used normal Desktop chat and host review. The UI showed Staged Review authorized for one future commit request; Git showed all three files staged. User then requested the commit in Chat. | Commit `ab4d1d9050a29bf33d042681072c17db05a7dee5` had the requested message; fixture A was clean afterward. No manual Git commit was used. Exact stage/commit Tool names and every confirmation click were not retained. | The host-owned authorization did not prevent completion. **NONE** for the commit path. |
| 5. Repository switching | Admit B, observe B, and return to A. | User tried Repository selection after the commit. Repository showed `Repository selection is unavailable while chat is running`, while Chat simultaneously showed `Chat ready` and the completed commit. Retrying at ready reproduced the block. User then used Disconnect, selected/admitted B, switched, and reconnected; later returned to A. | User reported correct switching and correct repository-specific answers. A screenshot later showed A active. The exact B and return chat transcripts were not retained. No simultaneous multi-repository execution was attempted. | Disconnect/Connect cleared the inconsistent selection block; the user had to discover and perform that extra lifecycle sequence. **MATERIAL** friction in switching/recovery, though the goal completed. |
| 6. Provider/model lifecycle and runtime state | Recompose after a normal model preference change and continue using A. | User disconnected, set the Model form to OpenAI / `gpt-6-luna`, reconnected, and asked a simple A question. | User reported a correct answer. The screenshot showed the model form and A active. Earlier UI screenshots displayed connected state, certified Codex version, repository selection, and Chat ready. A screenshot of the exact post-change connection status was not retained. | Disconnect/Connect worked as a recovery boundary in this session. **NONE** for the completed model change; model activation is user-reported rather than separately inspected. |

The user also encountered **Remembered Workspaces** friction while trying to
select a location: an entry editor extended beyond its content area, clipping
the left side of its `Choose Location` control, which the user reported did not
work. The section separately displayed `Desktop frontend unavailable` in red
while Repository, Staged Review, and Chat remained functional. The user had
entered the edit-task text as a remembered-workspace label before returning to
the actual Chat input. The remembered entry was a descriptive shortcut; it was
not needed to complete the A/B active-repository flow. These observations show
an awkward and partly unavailable optional path, but do not establish the
cause of the red error or authorize a UI fix here.

## Friction and evidence boundaries

| Observation | User impact | Severity | Evidence limit |
| --- | --- | --- | --- |
| Correct discovery/edit/review/commit in A | Ordinary goals completed | NONE | Exact Tool event inventory was not captured. |
| `docs/usage.md` called unstaged despite staged-only Git state | Extra verification needed for index/worktree understanding | MINOR | One answer, not a repeated failure rate. |
| Repository selection blocked as `chat is running` while Chat displayed ready | A/B switch required Disconnect/Connect recovery | MATERIAL | The internal cause is unknown; no implementation diagnosis is claimed. |
| Remembered-workspace location editor clipped and section reported `Desktop frontend unavailable` | Optional location shortcut could not be completed through that UI attempt | MATERIAL for that optional path | No cause or broader prevalence established. |

The search task did not naturally require regex, fuzzy matching, ignored-file
discovery, or simultaneous repository execution. No directory-creation task or
remote MCP workflow arose. CI time, developer logs, process identity, and
fixture setup are excluded from product-friction classification. Screenshots
were supplied during the session but are not copied into this repository.

## Disposition and next step

**B — CONCRETE MATERIAL WORKFLOW FRICTION OBSERVED.** The representative
session completed its main exploration, edit, reviewed commit, switching, and
reconnect goals, but repository selection was blocked by a user-visible
ready/running state conflict until Disconnect/Connect. The remembered-workspace
location UI also had an observed optional-path failure. Neither observation
selects a product capability or proves a root cause.

Recommend **Task 459 — v0.33 Evidence-Based Capability Reassessment** to decide
whether the observed goals and workaround cost justify bounded product work.
Task 458 makes no product, authority, schema, ADR, Cargo, or runtime-policy
change. RAH remains `0.32.0`; HostExplicit remains exactly 11; current
certified Codex remains `0.157.1`; v0.33 product capability remains **NONE
SELECTED**. No tag or release is part of this task.
