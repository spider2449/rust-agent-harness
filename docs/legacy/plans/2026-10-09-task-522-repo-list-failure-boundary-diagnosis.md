# Task 522 - repo.list native Desktop failure boundary diagnosis

Date: 2026-10-09. Diagnostic task; no implementation correction established.

## Decision

**F - ROOT CAUSE UNRESOLVED**

The actual native Desktop workflow failure is established by Task 521, but its
arguments and typed backend failure were not captured. Current disposable Tool,
production registry/permission and native continuation fixtures pass. No Tool,
continuation, presentation, input or external-environment cause is proven for
the actual failure. No production correction or regression claim is made.

## Plan

1. Verify starting master and exact-head CI; preserve release and evidence.
2. Inspect listing, observer, authorization, native continuation and Desktop events.
3. Run disposable direct-Tool fixtures and production-registry focused diagnostics.
4. Exercise native deterministic continuation; obtain one fresh human Desktop probe.
5. Record supported boundaries and gaps; publish only this report unless a narrow
   demonstrated defect requires correction and all required acceptance passes.

## Starting checkpoint

Local HEAD, origin/master, advertised origin master and GitHub API master matched
`1aba86e9afa1c10247c888bf74b24e005d5f5198`.
CI run 37895604531 was completed/success for that exact head.
The three historical untracked Task 514, 514C and 518 reports were present and
their hashes matched Task 521. No ignored evidence was removed or rewritten.
The local v0.34.0 tag object is `504c298947f81f531364d99c1189b8281ddf5a6f`,
targeting `7b972988f2f116b87b3770d66be1ef970e20b726`.

## Contract inspected

Inspected `repository_list.rs`, `repository_observer.rs`, repository boundary
validation, `ToolRegistry::execute`, authorized dispatch, `HostToolScope`, native
Responses request/continuation, Desktop runtime composition, chat event mapping
and frontend `status.js`. README, architecture/guardrails, security, ADRs 0032
and 0033 and Task 375-377, 380-382 and 409-411 records informed the inspection.
Historical failures are preserved: Task 380's sparse/missing-parent correction
was followed by Task 381's link-boundary rejection and 381-A/B hardening; Task
382 subsequently passed with explicit model/symlink nonclaims. Task 409 stopped
at its Desktop failures; Task 410 disposition and Task 411 certification do not
erase that stop. These older results do not certify native model continuation.

The actual registered definition is `repo.list`, Execute permission, with:

```json
{"type":"object","additionalProperties":false,"properties":{"path":{"type":"string"}}}
```

`{}` means root. Optional `path` is a nonempty safe repository-relative string,
at most 1024 UTF-8 bytes. Empty string, `.`, `..`, absolute paths, leading or
trailing slash, empty components, backslash, colon, NUL and case-insensitive
`.git` components fail closed. Non-object input, unknown fields and non-string
paths are invalid. Serialized input is bounded to 4096 bytes.

The host selects Git and the admitted active repository. The only inventory
command is `git --no-pager ls-files --cached --deduplicate -z --full-name --`.
Inventory stdout is bounded to 4 MiB and 100000 records; the total listing
deadline is 15 seconds. Identity and nested/link/reparse boundaries are
revalidated. No request-controlled root, executable, argv, timeout or recursion
exists. Direct children are projected from tracked present eligible files;
directories are synthesized from eligible descendants. Ordering is deterministic
repository-relative UTF-8 byte order. Missing/nonregular/nonaddressable entries
are omitted with counts, without expanding visibility.

Successful output is `ToolOutput { is_error: false, content: [Json(...)] }`:

```json
{"status":"ok","consistency":"best_effort","path":null,"complete":true,
 "truncation_reason":null,"entries":[{"path":"nested","kind":"directory"}],
 "omitted":{"non_addressable_path":0,"non_regular":0,"changed_or_missing":0}}
```

This is a shape example, not private repository output. Entries have only `path`
and `kind` (`file`/`directory`). At most 128 entries are returned, with
`complete:false` and `truncation_reason:"result_limit"` on saturation; serialized
ToolOutput is capped at 128 KiB. No file content is read by listing.

Parser failures are `ToolError::InvalidInput`. A visible tracked file target is
an Execution error (`requested repository path is not a directory`); missing or
untracked-only directory prefixes are Execution errors (`requested repository
directory was not found`). Timeout, conflicting projections, output overflow,
inventory/layout and unsafe-boundary failures remain errors, never partial
success. These typed classifications follow the implementation; the existing
Layer A rejection test asserts errors rather than downcasting each case.

Effective authority is ReadOnly / RepositoryObservation / Execute /
repository_bound=true. `host_kind("repo.list")` is None; HostExplicit remains
exactly 11. Authorized dispatch compares the advertised and current definition
and requires its permission in host-composed permissions. Registry lookup alone
is not a permission decision. Native binding uses the same registry and policy
through a revocable active-turn HostToolScope, with host-generated ToolCallId.

## Layered diagnostics

Only focused diagnostics were run; no workspace release gate, production build
or paid/provider inference was run by the agent. Each Cargo process was retained
through WaitForExit and its same-object ExitCode, with stdout/stderr retained
privately. Cargo used `target/task518` explicitly; Cargo rebuilt affected targets
from the checkpoint/source under test rather than directly invoking an old
executable. All commands used serial test execution. Temporary diagnostic test
additions are retained privately and removed from source before publication.

| Layer | Request and route | Result and limit |
| --- | --- | --- |
| A | Existing `repository_list::tests` on disposable Git fixtures, direct Tool | Exit 0; 8 passed, 0 failed. Root `{}`, nested `{"path":"crates/alpha"}`, deep ordinary/linked paths, expected tracked entries and binary metadata passed. File target, missing prefix and unsafe inputs returned errors. No actual Task 521 request was replayed because its arguments are unknown. |
| B registry | Existing ignored `tests::task379_windows_repository_list_live_certification`, explicitly opted in | Exit 0; 1 passed. Actual admission/selection -> production registry -> effective composition -> registry dispatch passed. Root/nested, sparse/deleted/staged, linked isolation, bounds, junction/reparse, binary privacy and repo.search smoke checks passed. Windows symlink remains a nonclaim. This harness alone does not exercise native model permission dispatch. |
| B host | Temporary `tests::task522_diagnostic_composed_host_real_list`, adapted from existing Task 508 native production fixture | Exit 0; 1 passed. Actual Desktop admission/activation, production Connect and host-composed registry/permissions executed `repo.list` with `{}` through the live port. One Started/Finished and one Completed; returned entries were exactly the tracked directory and file. Captured continuation had the matching `fixture-call` provider ID and no inactive-repository marker. A separate ordinary send_chat in that fixture returned to Idle. Disconnect revoked the runtime. Uses local OpenAI-format Responses fixture at the common native transport, not live llama.cpp UI proof. |
| C llama.cpp | Temporary `tests::task522_diagnostic_real_list_llama_continuation`, real RepositoryListTool registered behind Execute-authorized HostToolScope, existing LocalServer Responses facilities | Exit 0; 1 passed. Model fixture selected `repo.list` with `{}`. One Requested, Started and Finished correlated to the same host call ID; output matched the expected direct children and `is_error:false`. Continuation carried the matching provider `call_id` (`task522-provider-call`) and completed once. No runtime failure. One replay message / 12 bytes, below Desktop limits. Synthetic local HTTP server, not actual model inference or Desktop UI acceptance. |

Commands:

```text
cargo test -p rah-tools repository_list::tests -- --test-threads=1
cargo test -p rah-desktop tests::task379_windows_repository_list_live_certification -- --ignored --exact --nocapture --test-threads=1
cargo test -p rah-desktop --features openai-fixture tests::task522_diagnostic_composed_host_real_list -- --exact --nocapture --test-threads=1
cargo test -p rah-runtime-openai task522_diagnostic_real_list_llama_continuation -- --nocapture --test-threads=1
```

The registry harness used `RAH_RUN_TASK379_REPOSITORY_LIST_LIVE=1`. Temporary
fixtures execute existing host authorization rather than bypassing it. Their
test code is diagnostic-only, not a proposed implementation correction.

The existing host terminal test also passed (exit 0; 1 passed):
`cargo test -p rah-runtime experimental_host::tests::permission_and_tool_sources_are_typed_and_live_even_on_error -- --exact --nocapture --test-threads=1`.
It independently verifies denied dispatch has zero effects and returned Tool
errors retain their typed source and emit a terminal Failed event. This uses a
failing fixture Tool; it is not reproduction of the unknown actual list error.

Private diagnostic source fingerprints:

- llama.cpp fixture: `0A488129CA264AD607DACFECDCBBD677DCAEE96560C3F1F9AD1904B2B123FB7E`.
- composed Desktop host fixture: `6E9979E9C608DDF8893378B32D0CC1D3582562A887C320E2C62D3B95239F2526`.

Original Desktop and native-runtime test files were restored with identical
SHA-256 to their pre-diagnostic backups. Their Git diff is empty. Raw logs,
retained-process result records, diagnostic sources and hash manifest remain
private outside the checkout; no historical evidence was replaced.

## Search/list comparison and event boundary

Search requires `mode` (`path`/`text`) and `query`, with optional `path_prefix`;
listing has only optional `path`. Both schemas retain their exact meaning at
the adapter edge and are advertised non-strict because not every property is
required. The adapter does not insert a root spelling or normalize invalid
paths. JSON decoding is not validation of the listing contract; ListRequest
performs that validation inside execution.

Both use fixed tracked inventory and the same selected-repository observation
and Execute permission. Search can restrict candidates by prefix, stop at result
limits and, for text, read bounded content. Listing inspects eligible descendants
to synthesize direct children before truncating the final entries. Both cap
results at 128 KiB, but their output structures and candidate work differ. A
successful search does not prove valid list arguments, identical candidate
coverage, deadline margin or successful provider consumption of list output.

Native continuation serializes the returned ToolOutput into a string on a
Responses `function_call_output` item with the original provider call ID.
The next request is bounded by the native adapter's separate 4 MiB history bound
and eight Tool-round maximum. Those are distinct from Desktop conversation replay.

HostToolScope emits Requested before authorization and Started after admission.
For `Ok(ToolOutput)`, including `is_error:true`, it emits ToolFinished; Desktop
maps that to Completed or Failed according to `is_error`. For a returned
`ToolError`, it emits a turn Failed with code Tool and retained
AuthorizedDispatchError::Tool source; it does not emit ToolFinished. Permission
rejection emits turn Failed with code PermissionDenied before Started.
The adapter propagates request_live failure before appending a result or making
the continuation HTTP request.

Desktop handles turn failure, clears pending Tool-call tracking and emits Chat
Failed. Activity appends separate lifecycle rows, and does not synthesize a
terminal Tool row from turn Failed. Thus Requested/Running without Finished is
compatible with a completed error return and a failed turn. It is not evidence
that execution is still running, nor proof of a continuation/presentation defect.
Conversely, a successful Tool may precede a subsequent provider failure; that
requires a captured Finished result and continuation evidence to establish.

The generic operation failure in Task 521 is compatible with host dispatch
failure or continuation-round exhaustion. Common adapter transport, rejection
and protocol errors use different diagnostic categories. This narrows possible
paths from source, but an uncorrelated screenshot is not a typed source capture.

## Conversation and live observation boundary

Desktop retains `MAX_CONVERSATION_REPLAY_MESSAGES = 8` and
`MAX_CONVERSATION_REPLAY_BYTES = 32 * 1024`. The guard rejects greater values;
it counts replay messages/content bytes, not the llama.cpp model's native token
capacity. No replay limits or context architecture were changed.

Task 521's screenshot after New Conversation and successful search suggests
the earlier replay-limit banner did not cause that listing failure. It does not
measure the failed turn's history. The deterministic C fixture succeeds below
the limits; it does not reproduce the actual Desktop failure.

One fresh-conversation human Desktop listing was requested. Actual validated
arguments, repository/runtime generations, session/turn IDs, backend Tool result
and provider continuation outcome were not supplied before publication. The
human confirmed Desktop availability and willingness to perform one listing;
this is not an observed listing outcome. The bounded instruction asked for one
New Conversation root listing with `{}`, but prompt wording is not evidence of
the model's actual arguments. No agent inference or repeated model retry occurred.
A local census during diagnostics found llama-server, and a later census found
no rah-desktop process; neither establishes an external cause or a Desktop result.
Layer D is unverified, not failed or passed. Existing optional
`RAH_LIVE_EVIDENCE_PATH` records context counters and broad failure stage; it
does not capture validated arguments or typed ToolError. Public reports must not
include credentials, private prompts, full paths or raw Tool output.

| Required actual-failure fact | Supported result |
| --- | --- |
| Exact failing Tool request shape | Unknown; do not substitute the valid fixture `{}` or human instruction. |
| Whether repo.list executed | Task 521 Running suggests admission, but execution/result are not independently captured. Fixtures execute successfully. |
| Whether the Tool returned a terminal result | Actual result unknown; screenshot lacks a visible terminal row. Fixture ToolFinished is proven. A ToolError can terminate through turn Failed without ToolFinished. |
| Whether continuation failed | Unknown for the actual turn. Fixtures completed continuation with matching call IDs. |
| Failure below replay limits | Not established for the actual turn. Fresh-conversation probe has no supplied outcome or measured history; the successful deterministic fixture is below limits. |
| Specific external cause | None established. Server presence alone proves neither provider health nor cause. |

The next diagnostic step is the same bounded actual Desktop listing with a
process-local capture of Tool name, validated argument shape, host call/session
correlation, generation counters, typed dispatch error or output classification,
and continuation terminal. Preserve sanitization and authority; do not add a
retry, root alias, visibility expansion or successful terminal to conceal errors.
This is a diagnostic recommendation, not a v0.35 capability selection.

## Publication boundary

No production correction, new dependency, ADR, authority, permission, version,
tag, release or v0.35 selection is authorized by these results. Publish only this
report after restoring temporary test additions, checking diff integrity and
preservation hashes. All five focused diagnostic commands exited 0 (8 + 1 + 1 +
1 + 1 tests passed); no required deterministic failure occurred. HostExplicit is
unchanged at 11; no authority checks were weakened. Only this report is committed.
The three historical untracked reports remain excluded with original hashes;
all ignored historical evidence remains untouched. Commit/push and exact-head CI are post-commit facts to be
reported at closure; no claim is made before they occur.
