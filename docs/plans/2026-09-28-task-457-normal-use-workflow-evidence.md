# Task 457 — RAH normal-use workflow evidence

Date: 2026-09-28. Type: observation and evidence collection only.

Disposition: **C — EVIDENCE INSUFFICIENT; CONTINUE NORMAL USE**.

## Starting checkpoint and method

The clean isolated Task 457 worktree started at
`32cc24518e9db2874897b3dce8ff5cc5a7d80f70` (`docs: reassess RAH
v0.33 capability scope`). `origin/master` identified the same SHA. The supplied
natural push CI run `36375869801` was PASS; this report does not independently
certify that run. The original checkout at `deba6a10` has unrelated work in
progress and was left untouched.

The intended evidence unit is a user goal, starting state, actions, expected
and actual result, friction, workaround and cost, completion, and authority
boundary. An actual RAH user action is required to fill those fields. Source
inspection, deterministic tests, Git commands outside RAH, and certification
probes cannot establish the user's result or workaround cost.

The earlier Task 434 normal-use probe launched Desktop but could not deliver
input to its WebView. It recorded no prompt, Tool call, edit, review, or commit.
Task 456 likewise found no completed normal-use workflow evidence after that
probe. This Task 457 session had repository and source access, but no reliable
interactive Desktop chat input or transcript capture. Consequently, no new
end-to-end normal-user workflow was executed. This is a collection limit, not
an observed product failure.

## Workflow scenarios and actual observations

| Workflow | User goal and intended starting state | Actions taken in RAH | Expected result | Actual result, friction, workaround, completion | Authority boundary |
| --- | --- | --- | --- | --- | --- |
| A: discovery and reading | In an admitted ordinary repository, find an implementation and known string, inspect a relevant file and repository state. | None in this session. | `repo.list`, `repo.search`, `fs.read`, and `repo.file-info` support bounded discovery and reading. | No user result or friction observed; no workaround cost measured; task not started. | Selected active repository; read and repository observation. |
| B: edit and review | In a disposable repository, inspect, edit, review diff, stage, review staged diff, and commit. | None in this session. | Reviewed host workflow completes a bounded change. | No user result or friction observed; no workaround cost measured; task not started. | Content authoring, index mutation, reviewed commit. |
| C: repository switching | Admit A and B, observe A, activate B and observe it, then return to A. | None in this session. | Exactly zero or one active member; observations follow host selection. | No user result or switching pain observed; task not started. | Host selected repository membership and active member. |
| D: profile/provider lifecycle | Connect, use chat and a Tool, Disconnect, adjust relevant configuration, and Connect again. | None in this session. | New connection uses the current host composition. | No failed recovery, restart need, or live recomposition cost observed; task not started. | Host owned profile/provider composition and connection generation. |
| E: runtime presentation | Encounter supported and unsupported runtime states and recover from a connection error. | No interactive mismatch or recovery action in this session. | User can identify support status, error, and recovery action. | Source review finds version/source, connection state, sanitized error, and reconnect cues; user comprehension and recovery remain unobserved. | Exact Codex runtime admission; presentation grants no authority. |

Task 434's disposable repository remains a fixture from that earlier probe; its
existence is not a Task 457 RAH workflow. We did not use manual Git operations
as a substitute for RAH actions or count tests as normal-user runs.

## Candidate evidence table

The severity column says `UNOBSERVED` where no normal-user friction can be
classified as no, minor, material, or blocked. `NONE` means this collection
added no evidence for the candidate, not that all users are friction free.

| Workflow | User goal | Observed friction | Severity | Existing workaround | Authority implication | Potential capability implicated | Evidence strength |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | Find known code or text | No RAH search run | UNOBSERVED | `repo.list`, literal `repo.search`, known-path `fs.read` and `repo.file-info` are documented routes; cost unmeasured | Tracked selected-repository observation | Regex or fuzzy search | NONE |
| A | Find a file outside tracked inventory | No such user goal occurred | UNOBSERVED | Known-path read or host inspection, if appropriate; cost unmeasured | Would broaden local file disclosure | Untracked or ignored discovery | NONE |
| B | Create a directory during reviewed editing | No edit run | UNOBSERVED | Ordinary `repo.create-directory` or manual folder creation; adequacy unmeasured | HostExplicit #12 would be separate reviewed authority | HostExplicit directory creation | NONE |
| B/C | Work in another Git worktree | No worktree task occurred | UNOBSERVED | External Git setup and explicit RAH admission; cost unmeasured | Git/worktree mutation and selected-root authority | Worktree lifecycle | NONE |
| C | Move between admitted repositories | No switch run | UNOBSERVED | Activate one admitted member; cost unmeasured | ADR 0027 single active member | Multi-active repositories or union ToolRegistry | NONE |
| D | Apply changed profile or recover provider | No connection run | UNOBSERVED | Disconnect/Connect; cost unmeasured | Host composition and lifecycle ownership | Live recomposition, restart, or status | NONE |
| D | Use a remote MCP server | No such user workflow occurred | UNOBSERVED | Supported local adapters may or may not serve a specific goal; no cost measured | New network and credential authority | Network MCP | NONE |
| E | Understand unsupported runtime and recover | No interactive error encountered | UNOBSERVED | Existing status and recovery cues are present in source; user adequacy unmeasured | Exact 0.157.1 admission | Runtime presentation | NONE |

No concrete normal-user event involved broader WebView authority, additional
Tauri permissions, or another HostExplicit route. No row meets the threshold
of a specific observed goal, consequential limitation, materially inadequate
workaround, and bounded remedy. The Task 456 closed-candidate decisions are
preserved; no capability is selected here.

## Evidence boundary and disposition

**Normal-user evidence collected:** none. There is no basis to label the
unrun scenarios `NO MATERIAL FRICTION`, `MINOR FRICTION`, `MATERIAL WORKFLOW
FRICTION`, or `BLOCKED WORKFLOW`.

**Developer/certification observations excluded:** Task 434's inaccessible
WebView input, source inspection, repository/test fixture setup, process or
file-identity diagnostics, CI and Clippy outcomes, and exact-head mechanics.
These do not establish product demand or a RAH user-facing failure.

**C — EVIDENCE INSUFFICIENT; CONTINUE NORMAL USE.** A future collection needs
an interactive production Desktop session or a concrete user report with the
actual prompt, Tool actions/results, outcome, and workaround cost. This report
does not authorize Task 458, a v0.33 capability, or a release. RAH remains
`0.32.0`; HostExplicit remains exactly 11; current certified Codex remains
`0.157.1`; historical v0.32 release certification remains `0.149.0`.

This task changes documentation only. No Rust code, Cargo metadata,
permissions, ADR, Tool schema, provider lifecycle, repository authority,
runtime policy, or Desktop UI changed.
