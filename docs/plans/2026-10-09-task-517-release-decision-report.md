# Task 517 - v0.34.0 release decision report

Date: 2026-10-09. **C - OPENAI RELEASE GATE REMAINS BLOCKED**.

## Checkpoint and prerequisites

Local HEAD, origin/master, live GitHub master (GitHub branches API) and origin's
advertised master matched `041daae9cc448d0eaaef71dc15dfdce7a4aac6ef`.
[Starting exact-head CI 37888938589](https://github.com/spider2449/rust-agent-harness/actions/runs/37888938589)
was completed/success.

The inherited backend execution environment has no nonempty OPENAI_API_KEY.
The check emitted presence status only. No key was printed, persisted or logged.
RAH_OPENAI_MODEL is unset in this environment. No selected model's API validity
or account availability has been established. Persisted UI configuration and
credentials in other processes were not inspected; they cannot prove availability
to this execution environment. ChatGPT subscription access supplies no API proof.
Production backend selection reads OPENAI_API_KEY from its process environment.

Explicit potentially billable request authorization is not established by the
conditional Task 517 instruction. No real API request, model discovery request,
paid retry, Desktop live launch or test-only HTTP substitution was performed.
A bounded real acceptance cannot run with the present prerequisites.

## Acceptance contract and retained deterministic proof

The v0.34 release gate requires actual official Responses API acceptance or an
explicit documented human waiver. README documents production fixed-origin
OpenAI, streamed responses, host-owned text replay and host-authorized Tools;
provider-native continuation and OpenAI model discovery are unsupported.
The adapter's official endpoint is `https://api.openai.com/v1/responses`.

Any later live run must use the production native adapter and a disposable
repository with read-only Tools, bounded prompts and no automatic paid retries.
It must separately prove Connect, streamed response and valid terminal,
model-selected Tool dispatch plus host-result continuation, multi-turn continuity,
disconnect/reconnect recovery, sanitized diagnostics and unchanged host authority.
Stop and preserve evidence on the first required failure. Transport success alone
cannot certify Tool continuation. This task demonstrates no live product defect.

Task 516's committed report records native tests 23, neutral tests 18, production
OpenAI host Tool fixture 1, workspace 1,078/0/18 and Desktop 339/0/13, all PASS.
These are existing deterministic results, not live OpenAI certification or fresh
Task 517 executions. Task 515C records accepted actual Windows missing-key and
reconnect/recovery behavior. Successful llama.cpp and UI work is not reopened.

## Proposed waiver - awaiting explicit human decision

Proposal: permit later source-only v0.34.0 publication with native OpenAI included
but explicitly uncertified against the real official Responses API. Retain
deterministic OpenAI acceptance and missing-key UI recovery evidence, while
disclosing that actual account/model access, streamed completion, Tool continuation,
conversation continuity, reconnect and real-provider error sanitization remain
unverified. API users need separately configured backend credentials, an available
model and potentially billable API access. Real provider incompatibility, quota,
authentication, streaming or Tool behavior may still prevent successful operation.
The accepted llama.cpp Windows scope and redesigned Desktop remain as recorded.
Unsupported model discovery/provider-native continuation stay unsupported.

Human waiver status: **NOT APPROVED**. This proposal grants no waiver and no
publication authorization. CI, Task 516, missing credentials and historical release
practice cannot substitute for explicit approval. A waiver is not live PASS.

## Installer and publication boundary

**NSIS_INSTALLER_NOT_VERIFIED** remains separate. Task 516 validated executable
version/resources and bundle configuration; it did not build or execute an installer.
Its report records unavailable Tauri packaging CLI. No packaging dependencies are
installed and no packaging scope is added here.

The live v0.33.0 GitHub Release has zero uploaded assets; the established release
gate records tag/source publication. The proposed v0.34 disposition follows that
source-only convention, with no executable or installer upload. Task 517 authorizes
none of those publication actions. An executable upload or validated installer
requires separately authorized artifact scope and evidence; source-only publication
does not claim installer certification. No separate mandatory installer blocker
is established for the proposed source-only scope.

## Changes, validation and closure

Only the v0.34 release gate, Task 517 plan and this report change. No production
code, version, dependency, ADR, public API, ToolRegistry or authority changes.
The two historical untracked Task 514 reports retain their original hashes:
`DD8ED29F56C022D30E1E4504A9F8D62DBE11D265D76F7C9FD145836670E8BC43` and
`A6C834178A8428F3EBD3F3F99A4B254B639DFA1A9A6BEEA3E3B326AD8CAE277F`.
Existing reports and validation evidence are preserved; no cleanup occurred.

Documentation review and diff integrity precede normal documentation commit/push.
Final SHA, master equality and exact-head CI outcome are post-commit closure
evidence reported in the completion response; this report does not predict PASS.
No local implementation tests are rerun for this documentation-only change.
No v0.34.0 tag, GitHub Release, release artifact or version bump is created.

Next required decision: explicitly approve or reject the proposed waiver, or
securely provide backend API access, choose an available model and explicitly
authorize bounded potentially billable production acceptance. Until live PASS or
explicit waiver approval, outcome C remains. Publication requires later separate
human authorization even after that blocker is resolved.
