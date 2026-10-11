# Task 139: Windows live repository commit Codex validation

Starting checkpoint: `c1a77bdc1a2a20afc677c2292fe4ed5a69e7100f` (Task 138 exact-head CI `33290249642` PASS).

## Invocation

On the certified Windows host, select the installed certified executables and run:

```powershell
$env:RAH_CODEX_EXECUTABLE = (Get-Command codex -CommandType Application).Source
$env:RAH_REPOSITORY_COMMIT_GIT_EXECUTABLE = (Get-Command git.exe -CommandType Application).Source
cargo run -p rah-runtime-codex --example live_repository_commit_bridge
```

The example's connection uses the runtime's exact `SUPPORTED_CODEX_VERSION`
gate (`0.149.0`); it requires a canonical native `git.exe`. It creates a
separate ordinary, attached, no-remote fixture repository, baseline commit,
and exactly one staged `tracked.txt` change. An untracked file is deliberately
left present. Fixture setup Git is separate from `repo.commit` authority.

## Certified path and assertions

The task-owned Trusted Profile v1 has exactly symbolic native Git and fixture
repository resources and one Execute `repo.commit` capability, with fixture-only
host identity `RAH Live Commit <rah-live-commit@example.invalid>`. It is loaded
by `TrustedStaticProfile::load` and composed through
`rah_cli::profile_composition::compose`. The effective registry is asserted to
contain exactly one public `repo.commit` Tool with required message-only schema
and `additionalProperties: false`; its private bridge alias is `rah_tool_0`.

Before arming, the example records canonical fixture state, attached branch,
old HEAD, staged tree, staged diff, and refs, and verifies the exact reviewed
change. It then calls only host-side
`repository_commit_control().authorize_current_reviewed_snapshot()`. No
authorization, repository, Git, branch, HEAD, index, tree, identity, hook,
signing, ref, argv, or credential fields are model-visible. Codex receives only
the message request `{"message":"RAH live reviewed commit"}`.

The live turn requires the sequence `Started -> ModelRequestStarted ->
ToolRequested -> ToolStarted -> ToolFinished -> ... -> Completed`, exactly one
of each tool lifecycle event, a post-tool model delta, no approval, no retry or
replay, and exact final text `RAH_REPOSITORY_COMMIT_LIVE_OK`. Tool output must
be `committed_verified` with a commit OID equal to actual fixture HEAD.

Postconditions independently verify one parent equal to old HEAD, reviewed tree,
message, trusted author and committer, no signature, attached branch unchanged,
only its ref advanced, staged diff empty, preserved untracked file, and normal
reflog effect. Local ambient identity is intentionally wrong, proving fixed host
identity wins. The app-server is shut down/reaped, composition is shut down, and
the fixture is removed after evidence collection.

On success the markers are `RAH_REPOSITORY_COMMIT_LIVE_OK` and
`LIVE_REPOSITORY_COMMIT_BRIDGE_PASS`. This supports only the Windows local,
native-Git, one-commit claim. Linux/macOS, network Git, Desktop, linked
worktrees, submodules, signing, and branch operations remain unclaimed.

## Results

One live attempt was deliberately not started. On 2026-08-30 the available
native executable was
`C:\Users\morefunfun11\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe`,
which reported `codex-cli 0.150.1`; the certified Task 139 baseline is exactly
`0.149.0`. The native Git executable was
`C:\Program Files\Git\cmd\git.exe` (`git version 2.55.0.windows.4`).

Classification: before connection / before `ToolStarted`; no fixture commit,
authorization consumption, model turn, or RAH checkout mutation occurred.
`RAH_REPOSITORY_COMMIT_LIVE_OK = NOT VALIDATED / DEFERRED` until the exact
certified Codex executable is available. This does not support a Windows-live
claim and Task 140 remains blocked. The focused deterministic checks passed:
the example check, `rah-runtime-codex` repo-commit tests, `rah-cli` profile
tests, and `rah-tools` repository-commit tests.

ADR 0016 remains authoritative.

### 2026-08-30 Task 139A certified retry

The official `rust-v0.149.0` Windows x86_64 executable was provisioned
side-by-side at
`C:\Users\morefunfun11\AppData\Local\RAH\certified-codex\0.149.0\codex.exe`.
Its SHA-256 was verified as
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`, and it
reported `codex-cli 0.149.0`. `RAH_CODEX_EXECUTABLE` selected that exact
process-local path. The normal installed application remained present and
reported `codex-cli 0.150.1`; it was not used. Native Git was
`C:\Program Files\Git\cmd\git.exe` (`git version 2.55.0.windows.4`).

The existing `live_repository_commit_bridge` example was then run. Profile
validation, composition, reviewed-snapshot authorization, Codex connection,
and app-server shutdown/reap all completed. The live model turn instead ended
with `Started -> ModelRequestStarted -> ModelDelta... -> Completed`, with no
`ToolRequested`, `ToolStarted`, or `ToolFinished`; the gate therefore failed
its required exact-one `repo.commit` lifecycle assertion. The fixture cleanup
ran via the example's drop path; no success marker, commit OID, or Windows-live
commit claim was produced.

Classification: certified live attempt reached the model turn, but the model
did not invoke the only permitted RAH tool. `RAH_REPOSITORY_COMMIT_LIVE_OK =
NOT VALIDATED / DEFERRED`; Task 140 remains blocked. This failure is not a
Codex-version selection failure and does not authorize changing the certified
baseline or weakening the Task 139 gate.

### 2026-08-30 Task 139B live-tool-selection diagnosis

Attempt 1 used the certified executable
`C:\Users\morefunfun11\AppData\Local\RAH\certified-codex\0.149.0\codex.exe`
(`codex-cli 0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`) and
`C:\Program Files\Git\cmd\git.exe` (`git version 2.55.0.windows.4`). The
runtime used inherited Codex model/provider selection; it supplied no explicit
workspace context/CWD to `thread/start`. Its complete RAH event shape was
`Started -> ModelRequestStarted -> ModelDelta... -> Completed`: ToolRequested
= 0, ToolStarted = 0, ToolFinished = 0, Git commits = 0, and no authorization
was consumed by Tool execution. The original prompt was:

```text
You have exactly one available RAH tool.

Use it exactly once with this JSON input:
{"message":"RAH live reviewed commit"}

After receiving the tool result, reply with exactly:
RAH_REPOSITORY_COMMIT_LIVE_OK

Do not request any other tool. Do not call the tool more than once.
```

The persisted Task 139A report retained the lifecycle counts and sequence but
not the raw final assistant text, so no exact final text can be truthfully
reconstructed from the available evidence. Hidden reasoning was not retained.
This missing capture is corrected in the example for a future permitted
attempt.

Before any new repository authorization, the existing harmless
`live_echo_bridge` control was run with the same certified executable. It
advertised one immediately visible dynamic tool:
`{"type":"function","name":"echo","description":"Returns the supplied text unchanged.","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false},"deferLoading":false}`.
The control turn completed without a ToolRequested event; it returned exactly:

```text
Calling the requested echo tool once.Echo tool invocation failed: code-mode host executable was not found.
```

Therefore `CONTROL_DYNAMIC_TOOL_REQUESTED = false`, `CONTROL_TOOL_STARTED =
false`, and `CONTROL_TOOL_FINISHED = false`.
`CODEX_0_149_DYNAMIC_TOOL_CONTROL = FAIL`. No new `repo.commit` fixture was
armed and no second mutating attempt was run.

The runtime source and deterministic bridge tests confirm that `thread/start`
receives `dynamicTools` and that every bridge DynamicToolSpec has
`deferLoading: false`; `turn/start` contains only translated user input,
`approvalPolicy: never`, and a read-only sandbox policy. No existing trusted
baseInstructions/developerInstructions surface is used by this live example,
so no such API was added.

The updated live example records and asserts the intended sole advertised
definition before it can arm a later fixture:

```text
PUBLIC_RAH_TOOL repo.commit
PRIVATE_ALIAS rah_tool_0
DYNAMIC_TOOL_COUNT 1
DYNAMIC_TOOL_NAME rah_tool_0
DYNAMIC_TOOL_DESCRIPTION Commit the currently host-reviewed staged repository snapshot once using the provided message.
DYNAMIC_TOOL_SCHEMA {"type":"object","properties":{"message":{"type":"string","maxLength":16384}},"required":["message"],"additionalProperties":false}
DYNAMIC_TOOL_DEFER_LOADING false
ALLOWED_PERMISSION Execute
```

It also changes only the future live-gate prompt and final-text assertion:
success now requires `RAH_REPOSITORY_COMMIT_LIVE_OK <commit_oid>`, where the
OID exactly matches both the `committed_verified` ToolOutput and independently
verified fixture HEAD. The host alone emits the stable console marker after
those checks. This removes the prior constant-only final-answer loophole but
creates no authority and changes no production bridge, profile, Tool, or ADR
semantics.

Task 139 remains blocked by the broader certified live dynamic-tool/control
failure, not repository commit correctness. Further investigation must address
the code-mode host executable/live dynamic-tool environment before a fresh
repository authorization is considered. ADR 0016 remains authoritative; no
commit effect was forced, no tool call was synthesized, and the exact-one gate
was not weakened. Task 140 remains blocked.

### 2026-08-30 Task 139C certified code-mode host recovery

Task 139C established the runtime-layout root cause without changing RAH
production code, the Generic Tool Bridge, `repo.commit`, ADR 0016, the
certified Codex baseline, or the installed Codex application. The certified
side-by-side directory initially contained the verified `codex.exe` but lacked
its required same-version companion. The original executable remained
`C:\Users\morefunfun11\AppData\Local\RAH\certified-codex\0.149.0\codex.exe`
(`codex-cli 0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`).

The official OpenAI `rust-v0.149.0` release asset
`codex-code-mode-host-x86_64-pc-windows-msvc.exe` was provisioned only as
`C:\Users\morefunfun11\AppData\Local\RAH\certified-codex\0.149.0\codex-code-mode-host.exe`.
Its SHA-256 was verified before use as
`3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`; optional
Authenticode evidence reported `Valid`, signer `OpenAI OpCo, LLC`. This exact
side-by-side layout satisfies the certified executable's InstallContext
fallback without placing a binary in the repository, PATH, System32, or the
installed 0.150.1 application.

With `RAH_CODEX_EXECUTABLE` selected process-locally to that verified 0.149.0
executable, the pre-existing harmless `live_echo_bridge` control then passed:
`ToolRequested = 1`, `ToolStarted = 1`, and `ToolFinished = 1`. It invoked
`echo` once with `{"text":"RAH_TOOL_BRIDGE_OK"}`, returned successful
ToolOutput `RAH_TOOL_BRIDGE_OK`, continued the model response after
ToolFinished, reached terminal `Completed`, and shut down the app-server.
The prior `code-mode host executable was not found` error did not recur.
Accordingly, `CODEX_0_149_CODE_MODE_HOST = AVAILABLE` and
`CODEX_0_149_DYNAMIC_TOOL_CONTROL = PASS`. This control result authorizes one
new, fresh repository-commit fixture and authorization only; it does not
reuse an earlier authorization or relax the exact-one lifecycle gate.

### 2026-08-30 Task 139C fresh live repository-commit result

After the passed harmless control, exactly one new disposable repository was
created with a new baseline, host-reviewed staged change, native Git
inspection/review, and one new `RepositoryCommitControl` authorization. A new
Codex thread and turn used the strengthened output-dependent prompt. The only
model-visible RAH ToolCall was `repo.commit` with exactly
`{"message":"RAH live reviewed commit"}`; no branch, repository, Git, argv,
authorization, or other host-owned field was exposed.

The sole live attempt passed with `ToolRequested = 1`, `ToolStarted = 1`, and
`ToolFinished = 1`; there were no approvals, synthetic calls, host fallback
calls, retries, or replays. Its successful non-error ToolOutput had
`status = committed_verified` and `commit_oid =
13c200c5c772b3e4a0eceb0a2364981c849313e0`. Host verification established
that this was a one-commit branch advance from the fresh old HEAD: its sole
parent was the old HEAD; its tree was the reviewed staged tree; its message was
exactly `RAH live reviewed commit`; author and committer identity were exactly
the trusted profile identity; it was unsigned; the current branch did not
change; other refs did not change; automatic staging did not occur; and the
unrelated untracked file remained present.

The model continued after ToolFinished and completed with exactly
`RAH_REPOSITORY_COMMIT_LIVE_OK 13c200c5c772b3e4a0eceb0a2364981c849313e0`.
The model-reported OID, ToolOutput OID, and independently verified actual HEAD
were equal. The fixture, composition, and Codex app-server were cleaned up;
the example emitted `RAH_REPOSITORY_COMMIT_LIVE_OK` and
`LIVE_REPOSITORY_COMMIT_BRIDGE_PASS`. ADR 0016 remains authoritative.
