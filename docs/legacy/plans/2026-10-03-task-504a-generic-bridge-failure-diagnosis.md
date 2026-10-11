# Task 504A — Generic bridge failure diagnosis

## Checkpoint and final classification

Starting and final HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`.

**D — ROOT CAUSE STILL INCOMPLETE.** The frozen failures cannot be attributed
to an exact underlying rejection from the retained historical payloads. All
three individual diagnostic executions and one Phase A Codex library diagnostic
execution succeeded. A later success does not explain an earlier failure.
No corrective change is recommended or authorized by this evidence.

The preserved Task 504 WIP has seven modified tracked source files, one new
Desktop source module, ADR 0033 and its original report. No reset, stash,
clean, worktree recreation, remote modification or history modification occurred.
Initial `git status --short` recorded that state; initial `git diff --check`
exited 0. Task 504's historical E classification remains unchanged.

## Evidence and execution bounds

Historical command: `cargo test -p rah-runtime -p rah-runtime-codex`, exit 101.
Historical results: rah-runtime 18 passed; rah-runtime-codex library 99 passed,
3 failed, 1 ignored. Original log remains
`F:/temp/rah-task504-evidence/phase-a.log`.

Task 504A evidence directory: `F:/temp/rah-task504a-evidence`.
Before execution, SHA256 was captured for ten directly relevant files in
`source-before.json`. Temporary diagnostic logging was added only to
`bridge.rs`, `repository_create_directory.rs`, and `repository_git_layout.rs`.
It prints dispatch identity and typed result, directory repository validation
errors, and fixed Git probe results. No input, precondition, permission,
correlation, result, timeout or execution policy was changed. Exact original
bytes were backed up before instrumentation; its patch is retained in
`diagnostic-instrumentation.patch`. No source edit occurred during Cargo execution.

Commands used `CARGO_TARGET_DIR` and `RAH_TEST_TARGET_DIR` equal to
`F:/temp/rah-task504-target`, the existing Task 504 target:

| Command suffix after `cargo test -p rah-runtime-codex` | Log | Exit/result |
| --- | --- | --- |
| `--lib bridge_tests::repository_search_dispatches_through_the_generic_bridge -- --exact --nocapture` | search.log | 0; 1 passed, 102 filtered |
| `--lib bridge_tests::repository_list_dispatches_through_the_generic_bridge -- --exact --nocapture` | list.log | 0; 1 passed, 102 filtered |
| `--lib bridge_tests::host_composed_repo_create_directory_uses_generic_bridge_once -- --exact --nocapture` | directory.log | 0; 1 passed, 102 filtered |
| `--lib -- --nocapture` | phase-a-codex-diagnostic.log | 0; 102 passed, 0 failed, 1 ignored |

The library execution retained the suite context after the individually selected
tests failed to reproduce the historical outcomes. No further passing rerun was
performed. Diagnostic logging changes timing and these passes are not stability
certification. No Desktop, full workspace, Clippy, production, live provider,
alternate-provider or executable inventory validation ran.

Instrumentation was removed after all commands exited. All ten source hashes
match their starting bytes (`source-stability.json`). No production source fix
or retained instrumentation exists. Closure `git diff --check` passes.

## Actual tested path and authoritative master comparison

The premise that these tests exercise the neutral host port is incorrect.
Each uses `connected_bridge` in `bridge_tests.rs`, which calls
`CodexRuntime::from_transport_bridge`, then `start_bridge` calls the legacy
`AgentRuntime::start`. They do not instantiate `CodexFactory`, DesktopRuntime,
HostToolScope, HostToolPort, or HostTurnLease.

Actual path:

```text
fixture-bound Tool objects -> registry supplied to legacy CodexRuntime
fake item/tool/call -> bridge handle_request -> alias/snapshot/session resolution
-> authorize_tool_dispatch -> ToolRequested/ToolStarted
-> authorized_tool_dispatch(ToolContext::default()) -> ToolRegistry::execute
-> actual Tool::execute -> ExecutionResult(key, session, call, typed result)
-> finish_execution -> original waiting RPC IDs -> adapter output_response
```

After restoration, `git diff --name-only HEAD --` for bridge.rs, bridge_tests.rs,
test_support.rs, connection.rs and all of crates/rah-tools is empty. The only
Task 504 runtime.rs change makes `verify_effective_workspace_context` pub(crate);
its function body is unchanged. The experimental adapter's workspace changes
are in a separate path. Thus the three fixtures, dispatch semantics, Tool
implementations and result mapping are identical to authoritative master source.
No executable master rerun or master checkout was performed; the comparison
is exact current-source versus `HEAD`, not a claim of historical master test success.

Before Task 504 and in Task 504 WIP, the same inputs/context expect successful
dispatch. No migration-specific semantic divergence is present in these paths.
This rules out attributing the observed failures to the new neutral port merely
from their test names; it does not identify the historical Tool rejection.

## Independent search trace

Fixture: new unique ordinary Git repository, committed `bridge-search.txt`
containing `BRIDGE_SEARCH_SENTINEL\n`. RepositorySearchTool is constructed with
the fixture root and the host-resolved Git executable; Execute is allowed.
Dynamic alias comes from the advertised Tool snapshot. Request RPC ID is 914;
private thread/turn are `private-thread`/`private-turn`, call ID
`repository-search`; canonical name is `repo.search`; arguments are
`{"mode":"text","query":"BRIDGE_SEARCH_SENTINEL"}`.

Historical response `RAH tool execution failed` is produced specifically by
`finish_execution`'s `Err(AuthorizedDispatchError::Tool(_))` branch. It proves
the request reached registry execution and the underlying Tool returned an
error, rather than an admission rejection or host-scope unavailability.
The branch discards the underlying ToolError before publishing the wrapper.
The historical log does not retain that ToolError or observer probe output.

Individual diagnostic identity: SessionId
`7d04c41a-f830-48a3-8ff6-cb508a832bd9`, ToolCallId
`8094acda-d232-4a49-b1d0-c320f957c023`. Structured result was
`Ok(ToolOutput { is_error:false, ... status:"ok", complete:true,
matches:[{path:"bridge-search.txt",lines:[1]}], consistency:"best_effort" })`.
The suite diagnostic also returned that successful result (session
`c9d13d1f-0add-437a-946c-1bfe96babbe5`, call
`70ec86d9-30a8-4454-98cd-ef54bed98c63`). Lookup, both authorization checks,
Tool execution, response correlation and the three-event assertion succeeded.

**Historical first concrete divergence:** actual repo.search execution returned
ToolError. Its precise inner cause remains unavailable; no R1–R7 code is proven.
R6 is a possible location classification, not an established root cause.

## Independent list trace

Fixture: independently created ordinary Git repository with committed README.md
and src/lib.rs. RepositoryListTool captures that root/Git executable; Execute
is allowed. Request RPC ID 915, private call ID `repository-list`, same private
thread/turn spellings, canonical name `repo.list`, arguments `{}`.

The historical wrapper independently proves the
`AuthorizedDispatchError::Tool(_)` completion branch. It does not establish
the same underlying failure as search. RepositoryListTool validates observation
and inventories tracked entries; the historical observer result was discarded.

Individual diagnostic identity: SessionId
`f38ae454-bc13-418c-bd0d-2189d567c0c6`, ToolCallId
`762e322a-07e2-464c-a51b-bbd62cb11426`. Structured result was successful,
complete, best_effort, status ok, entries
`[{path:"README.md",kind:"file"},{path:"src",kind:"directory"}]`.
The suite diagnostic independently returned the same projection (session
`f47061ff-1615-48a5-9e1c-5865aedf9f58`, call
`ede6ce69-84b7-451a-84a6-4347530e3dd8`). All assertions passed.

**Historical first concrete divergence:** actual repo.list execution returned
ToolError. Its precise inner cause remains unavailable; no R1–R7 code is proven.
Do not unify its cause with search or infer a deadline failure from elapsed time.

## Independent create-directory trace

Fixture creates `existing`, commits anchor.txt, and leaves
`existing/new-directory` absent. A profile with no capabilities does not publish
this Tool. Separate host RepositoryDirectoryCreationAuthority, bound to the
fixture root/Git executable, is supplied to composition to publish it. A counting
wrapper delegates to the real composed Tool. Request RPC ID 1840, alias
`rah_tool_0`, private call ID `create-directory-once`, canonical name
`repo.create-directory`, arguments `{"path":"existing/new-directory"}`.

Historical structured `precondition_failed`, `uncertain:false` originates in
RepositoryDirectoryCreationTool::execute, not bridge admission. Parsing
succeeded. Exactly two pre-native branches produce this status: initial
`policy.capture` rejection or `policy.revalidate` rejection. Neither branch
calls native `create_directory`. Thus the real Tool was reached and no native
directory-creation attempt occurred for that historical request. The status
does not distinguish which capture predicate failed or which of those branches
was selected. Capture checks repository identity/Git layout, existing ordinary
parent/reparse/nested boundary, absent target, parent identity/native parent and
Git snapshot; revalidation repeats capture and checks path/parent/Git equality.

Individual diagnostic records target_exists=false at Tool entry, matching the
fixture's legitimate absent-target invariant. SessionId
`51bc2266-cfd6-48e7-af83-77861066d7c7`, ToolCallId
`b2801361-3faf-4b79-b7c2-8f6bf0259e8f`. Actual result was
`directory_created_verified`, uncertain=false, git_metadata_changed=false.
Counter was exactly one and target was a directory, as the passed assertions
prove. The suite also returned verified success (session
`465b8e44-4a49-4174-bcb2-09ce2ff6dcfb`, call
`23d21cef-7d73-447d-9cf0-0a1fdbeb8584`).

**Historical deterministic rejection boundary:** real Tool pre-native capture
or revalidation. Exact failing predicate is not recoverable from the frozen
result and did not recur; no R1–R7 root-cause code is proven.

## Host port, context, scope and correlation audits

In these tests no neutral call/lease identity exists. Legacy SessionId is
allocated by the runtime and resolves from private thread/turn session records.
ToolContext is an empty struct (`rah-tools/src/lib.rs`); both legacy bridge and
neutral port use its default. The three Tools ignore it. Repository authority
lives in host-constructed Tool objects, not request SessionId or adapter cwd.
Each fixture supplies its own registry; no arbitrary root is accepted in inputs.
Neither generic bridge normalizes these arguments nor selects the repository.

Legacy ExecutionResult retains the same CallKey, session and ToolCall; completion
looks up that key and responds to its waiting RPC IDs. Historical responses have
the intended RPC IDs 914/915/1840. Diagnostic typed completion logs retain the
intended logical call keys and canonical names. No evidence of cross-request
result substitution exists. There is no neutral result translation in this path.

Separate static neutral production audit: Desktop captures selected repository
and generations during connection composition; desktop_tool_registry constructs
the bound Tools. configured_codex_factory canonicalizes the host workspace;
bind_conversation retains the same composed registry/allowed permissions in
HostToolScope and supplies a weak revocable port to the neutral conversation.
The adapter verifies effective thread cwd. Ready publication retains generation
checks. No global cwd mutation or provider-selected root is introduced.

HostToolScope starts with no active turn. admit_turn creates the host SessionId,
event stream and owned lease; adapter send stores the lease on its route, returns
that SessionId, and request_live uses it. Host admission checks live weak owner,
not revoked, accepting matching session and capacity; reserves execution under
the same lock as revocation. Host assigns fresh ToolCallId and performs existing
authorization and registry dispatch. Terminal/stream-drop invalidates the route
and releases/stops its lease; Desktop revokes before executable composition
withdrawal. Already admitted executions remain host-owned and drained.
Neutral adapter correlation uses `(route.session, provider callId)` and retained
RPC IDs; identical replay coalesces, conflicting replay rejects. Host requests
carry canonical name and unchanged input. This is static evidence, not Desktop
live or deterministic lifecycle certification.

No neutral missing/revoked lease can explain these legacy test failures.
No production host-binding defect was demonstrated by them. Neutral lifetime
and selected-repository safety still need the previously stopped Desktop gates
in a separately authorized task.

## Fixture and observer findings

These are intentionally legacy bridge fixtures; direct CodexRuntime construction
is implementation-specific coverage of a still-existing legacy adapter path,
not proof of a stale fixture. The absent-target and tracked-file expectations
remain legitimate Tool invariants. No fixture correction is justified.

RepositoryObserver is the Tool's fixed Git observation envelope, distinct from
host lifecycle event observers. Historical search/list reached Tool-error
completion; historical directory returned a real error ToolOutput. None is
merely a final assertion reading an incorrect successful projection. Diagnostic
passes also verified expected lifecycle events and directory execution count.
Frozen-file hash equality proves source stability, not successful Tool execution.

## Root-cause and migration decisions

| Test | Proven historical boundary | R1–R7 classification |
| --- | --- | --- |
| search | AuthorizedDispatchError::Tool; underlying ToolError discarded | Withheld; exact cause incomplete |
| list | Independently AuthorizedDispatchError::Tool; underlying ToolError discarded | Withheld; exact cause incomplete |
| directory | Real Tool's pre-native capture/revalidation rejection; zero native attempts | Withheld; exact predicate incomplete |

Assigning R6 solely because code is unchanged, or R7 solely because later runs
pass, would overstate the evidence. The report therefore uses the explicitly
permitted task-level D outcome instead of manufacturing per-test causes.
Whether all three share a cause, only search/list share a cause, or all are
independent remains unproven. Timing/contention/environment hypotheses are not
root-cause findings.

**V1/V2/V3/V4:** no final viability classification is established. Static audit
found no authority/architecture contradiction, but V1 requires an identified
bounded correction and V2 requires proven fixture/observer-only causes; neither
exists. No evidence supports V3 or V4. Architecture remains a viable unvalidated
candidate; these legacy failures do not prove that it needs revision.

**Bounded next action:** evidence capture only, no product correction. In a
separately scoped diagnostic continuation, capture a naturally failing invocation
with typed dispatch result and stage-specific directory capture/revalidation
errors before wrapper erasure; retain fixed Git probe output and identity checks.
Do not force a failure, relax deadlines/preconditions, or rerun passing suites
for certification. Historical exact predicates cannot be reconstructed by
observing fresh successful fixtures. No Task 504 correction is ready.

ADR 0033 remains byte-for-byte unchanged: no durable assumption was disproved.
HostExplicit remains statically exactly 11 (HostInvocationKind enum and existing
routing unchanged). No executable inventory was run. No new Tool, permission,
dependency edge, authority category, provider, bypass or adapter repository
authority was added. No source correction, Desktop validation resumption,
commit, staging, push, tag, release, version bump or Task 504/504A CI occurred.

Final worktree preserves Task 504 WIP and adds this report plus a reference in
the Task 504 report. It remains dirty and uncommitted. Diagnosis is incomplete;
the optional docs-only completion commit was not created.

## Recorded source SHA256

All values below matched after instrumentation removal.

| File | SHA256 |
| --- | --- |
| crates/rah-runtime-codex/src/bridge.rs | 54ca0cf9a6cdca31844f09f60048a39811d96ac3acca2496a5f15533fdfa4e01 |
| crates/rah-runtime-codex/src/bridge_tests.rs | 5eb4b6bf408434681b5541533772fe0b74d565de9a4dcbafaf536748813b37a6 |
| crates/rah-tools/src/repository_search.rs | f61cd5f845a2cc40fd8f1b17a449565bec08fd5603fa475ed6663ccdcbc403e1 |
| crates/rah-tools/src/repository_list.rs | 6c9e7f1c2a5dd909f7404952d882fdf7672423ec8eb6b1a2475eb1c66680fc5f |
| crates/rah-tools/src/repository_create_directory.rs | 19cad743229eb8c4bc31db9699a3a78085147c777424e8863454d0cc5b42ba33 |
| crates/rah-tools/src/repository_observer.rs | 934eea663afba3c7656c1b51e3f587714ce00766619aab5f6b7604b0604a1e0a |
| crates/rah-tools/src/repository_git_layout.rs | a22c5436d659b639a175039c9a779b7b4cfd0cd52d2b3c273b4612f16cb078da |
| crates/rah-runtime/src/experimental_host.rs | 41d14b70d8d39c265f43cbdadd2736d78194222767701bcef3a3aa0341bda0de |
| crates/rah-runtime-codex/src/experimental.rs | 2c0454b045bdc32485a256ab9b02d6d2839a5e8ce1097fed88ee48b5e8f3f8d1 |
| crates/rah-desktop/src/runtime_composition.rs | d9bbf44962aa10c8c5d7e2419c015d4cc467eca2f090510590859c36a71672e4 |

## Diagnostic continuation reference

See [Task 504B natural failure evidence capture](2026-10-03-task-504b-natural-failure-evidence-capture.md)
for the bounded reproduction matrix and instrumentation restoration evidence.
