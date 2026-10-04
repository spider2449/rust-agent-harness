# Task 510A — Newer Codex provider-bound discovery protocol audit

Starting HEAD: `570e93319382343b1fa3d7996c413736d9e13770`.

Research only. Preserve Task 509/509A/509B/509C/509D documentation WIP.
Certified/preferred runtime remains exact `codex-cli 0.157.1`; no picker,
production source, native OpenAI API, runtime admission or baseline update.

## Execution plan

1. Verify baseline and locally installed 0.160.0 artifact identities/provenance.
2. Generate exact candidate schemas; compare discovery/config/startup/thread
   shapes against Task 509D baseline evidence and assert P1/P2/P3 observations.
3. Run serial isolated Inherit/OpenAI/Ollama discovery controls without inference.
4. Run one bounded gpt-6.1-sol direct control per sane artifact through Codex;
   distinguish catalog advertisement, accepted requests and completed inference.
5. Inspect current Desktop admission; exercise its existing path if supported,
   preserving unknown-version fail-closed behavior without bypass.
6. Run bounded deterministic adapter/model checks and artifact rehashes. Complete
   this report, add a reference-only Task 509D note, inspect documentation diff,
   commit the coherent research documentation, push normally and verify exact-head CI.

## Disposition

**C — NEWER CODEX RUNTIME STILL CANNOT PROVIDE PROVIDER-BOUND DISCOVERY**

The exact locally installed 0.160.0 artifact provides no authoritative ownership
association between model/list and the effective upstream provider. P1/P2/P3
remain unsupported for that association. Task 509B/509C remain blocked; waiting
for this runtime refresh does not solve their prerequisite. This research does
not certify 0.160.0 despite its successful bounded direct model control.

## Artifact identity and trust

| Field | Baseline control | Primary candidate |
| --- | --- | --- |
| Reported version | codex-cli 0.157.1 | codex-cli 0.160.0 |
| Executable | `C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe` | `C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe` |
| Bytes | 322515248 | 326872368 |
| SHA-256 | `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574` | `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d` |
| Provenance | Existing manifest v2: host-path, host-provided-exact-version-bundle, archived 2026-09-27T08:00:44.3650915Z | Existing global npm @openai/codex 0.160.0, platform package codex-win32-x64; no acquisition/install in this task |
| Trust/admission | Certified/preferred; baseline helper verify passed | Not saved as trusted baseline; not currently admitted or certified |

Before/after executable hashes and sizes match. Candidate hash also matches the
Task 494 historical artifact record; matching that record is identity evidence,
not certification. Baseline companion SHA remains
`5583872360541856fe6207eba0fb8486d9967083fbf4d40e2126ec9bb9c06ead`;
the baseline helper verified the saved bundle.
Candidate companion codex-code-mode-host.exe was separately hashed as
`1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6`;
this measurement does not admit the bundle. No later version was found in the
inspected PATH npm package; PATH resolves wrappers for this same 0.160.0 package.
The optional later-installed candidate therefore adds no distinct artifact.

Evidence directory: `F:/temp/rah-task510a-protocol`. Artifacts-before/after JSON,
candidate generated experimental schemas, audit/probe scripts, schema comparison
and leaf differences, discovery/live results and test logs are retained there.
Baseline generated schemas and Task 509D evidence remain in
`F:/temp/rah-task509d-protocol`. Task 494's referenced external evidence directory
was not present during this audit; its checked-in historical report is used only
as historical evidence. Its successful control is not substituted for today's
candidate control. No running-image/TOCTOU identity guarantee is claimed.

## Actual protocol inventory and differences

Generated with the exact candidate executable:
`app-server generate-json-schema --experimental --out F:/temp/rah-task510a-protocol/0.160.0`.
Seven relevant parsed schema comparisons were performed:

| Schema | Difference from exact baseline generated schema |
| --- | --- |
| v2/ModelListParams | Identical: cursor, includeHidden, limit only |
| v2/ModelListResponse | Identical: data and nextCursor; Model has no provider/source/config/thread identity |
| v2/ConfigReadParams | Identical: cwd and includeLayers |
| v2/ConfigReadResponse | Identical config/origins/layers and nested definitions, including nullable model_provider |
| v1/InitializeResponse | Identical userAgent/codexHome/platformFamily/platformOs; no provider/catalog identity |
| v2/ThreadStartParams | Identical, including optional model/modelProvider/config/cwd/ephemeral |
| v2/ThreadStartResponse | Same properties/required fields and provider/model/thread association; nested error changes below |

Exact nested ThreadStartResponse differences: CodexErrorInfo string alternatives
add `flexUnavailable` and `tooManyDenials`; existing alternatives remain, shifted
in array order. Turn.error description changes from failed-only to errors of
failed or interrupted turns. No new catalog/source/provider association follows.
Thread metadata is included in the compared response definitions; it supplies no
catalog reference. The eleven existing adapter fixture required-file/property
checks pass against the candidate generated schema. This is a bounded shape
check, not adapter compatibility certification or proof for every changed enum.

Discovery request remains
`{"method":"model/list","params":{"limit":100,"includeHidden":true}}`
with a connection-owned numeric id. Configuration inspection uses config/read,
not an assumed config/get operation. Provider selection uses process `-c
model_provider="openai"` or `"ollama"`; no config write was needed. Thread/start
has an independently selected modelProvider and returns concrete modelProvider.
There is no catalog/configuration ID, revision, endpoint or thread reference in
model/list's request or response. Startup metadata likewise supplies none.
Existing identical ThreadStartParams includes allowProviderModelFallback:
its description permits replacing an unavailable requested model with a static
catalog default. This is thread fallback policy, not discovery ownership input
or output; no such fallback was enabled by the control and the returned model
was checked exactly. Other providerId fields in ClientRequest concern external
thread import attribution, not catalog discovery, and grant no ownership proof.

Official documentation describes model/list as potentially bundled/cached and
separates catalog advertisement from successful inference. This corroborates
the need for caution but does not establish exact version semantics:
[Codex app-server documentation](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server).
Candidate conclusions below rely on the actual artifact schemas and observations.

## P1/P2/P3, explicit provider and Inherit controls

| Mechanism | 0.157.1 baseline | 0.160.0 candidate |
| --- | --- | --- |
| P1: discovery ownership input | No provider/config/runtime/thread scope input | Identical; unsupported |
| P2: startup configuration association | Explicit config observable; catalog ownership absent | Explicit config observable; catalog ownership absent |
| P3: thread/config metadata association | Concrete thread provider has no catalog association | Same; successful OpenAI thread below does not bind model/list |
| Drift classification | R4 control: baseline limitation | R4 — no meaningful improvement for provider-bound discovery |

Six serial credential-free discovery controls ran with separate new empty
CODEX_HOME/cwd directories, credential-bearing environment names filtered,
fixed executable/argv, 15-second RPC deadlines and 2 MiB stdout bound. They sent
initialize/initialized, config/read and model/list only: zero threads, turns,
server Tool requests or inference. Processes were terminated and awaited.

| Artifact | Selection | config/read model_provider | Catalog count | gpt-6.1-sol advertised | Provider/source identity |
| --- | --- | --- | --- | --- | --- |
| 0.157.1 | Inherit | null | 11 | No | Absent |
| 0.157.1 | OpenAI | openai | 11 | No | Absent |
| 0.157.1 | Ollama | ollama | 11 | No | Absent |
| 0.160.0 | Inherit | null | 11 | Yes | Absent |
| 0.160.0 | OpenAI | openai | 11 | Yes | Absent |
| 0.160.0 | Ollama | ollama | 11 | Yes | Absent |

All baseline ordered selector arrays hash to
`d3725fee286c2980699c760fda7fd0b0fe609aa7fbad7d399a67ce8b7bbe8c40`.
All candidate isolated arrays hash to
`004d53fa0d45176c91db1b5c826c5047fe23224df2ce1199be6333007ebe399d`.
Both versions return indistinguishable OpenAI/Ollama catalogs despite distinct
config readback. Across versions 0.160.0 adds gpt-6.1-sol and drops gpt-5.4 in
these isolated observations. Neither equality nor contents establish ownership.
These results do not prove a durable provider-independent implementation rule.

Inherit null remains unobservable as an actual effective provider. It cannot be
converted to OpenAI by default inference. A concrete inherited configuration
name would still not solve the absent catalog association. Explicit cases also
remain unbound, so classification D is not warranted. R2 would overstate an
improvement: explicit config readback already existed in the baseline and is
not new effective catalog-provider evidence.

## Bounded gpt-6.1-sol compatibility and Desktop admission

Baseline direct failure is reused from Task 494 (same recorded exact hash), not
retried here. Thread/start and turn/start were accepted but turn/completed was
failed: `The 'gpt-6.1-sol' model is not supported when using Codex with a ChatGPT
account.` Reported codexErrorInfo was `other`, upstream status 400. Baseline
catalog absence is freshly observed above; baseline Desktop model execution was
not probed in this task. The historical failed turn is an expected baseline
control, not a new Task 510A compatibility gate failure.

One fresh authenticated candidate direct app-server control ran on the exact
0.160.0 artifact, explicit OpenAI. It used a separate empty temporary home with
the existing Codex auth file copied locally for this control and removed in
finally cleanup; no auth content was logged or published, no native OpenAI API
key was supplied, and normal user config/auth was not edited. Ambient
credential-bearing environment names were removed. Startup mcp_servers was
empty. The connection uses Codex stdio initialize/config/read/model/list/
thread/start/turn/start, with no retry, fallback, other model or non-OpenAI turn.

Authenticated catalog: 10 entries, gpt-6.1-sol advertised; ordered selectors hash
`a47a5a873e6c1421455f4e1755def05d5eaf00e91f89fcf0fac937567819f503`.
This differs from the isolated catalog and still has no provider/source identity.
Thread/start requested model gpt-6.1-sol, provider openai, ephemeral true,
research cwd, never approval and read-only sandbox; shell/unified execution,
memories, web/image/app defaults were disabled and MCP definitions empty.
Returned model/provider exactly matched. Turn/start accepted the request;
turn/completed status was completed, response exactly `RAH510A_OK`. Observed
item types: userMessage/agentMessage. Zero server requests and Tool items.
Deadline: 60 seconds for completion. This proves only that one direct request
succeeded with this artifact/account/provider at this time.

Current Desktop neutral composition delegates to the same exact admission in
ProcessTransport before starting app-server. Current certified set remains
only `codex-cli 0.157.1`. The existing live_smoke example with a process-local
RAH_CODEX_EXECUTABLE candidate override exited 1 at admission:
`unsupported Codex version: current preferred codex-cli 0.157.1, found codex-cli 0.160.0`.
This expected rejection started no model turn. Desktop's unsupported-version
frontend projection deterministic test also passed. A real candidate Desktop
connection/inference was not attempted because admission blocks it; no bypass
or admission/source change was made. Desktop neutral acceptance remains unproven.
Expected admission rejection is not classification E or F, nor proof of R5
protocol incompatibility. No new protocol support requiring adapter consumption
was found, so classification B is not warranted.

## Runtime-artifact association and security

Research records tie every catalog/control to before/after exact artifact hashes.
This host-side measurement cannot transform an unbound catalog into
`CatalogEvidence { admitted_runtime_artifact, effective_provider, models }`.
The baseline manifest checks hashes separately; current process admission checks
version/schema and does not return an executable fingerprint. Candidate is not
admitted. Required authoritative provider ownership is absent for both artifacts.
No CatalogEvidence implementation or artifact admission API was introduced.

Certified/preferred 0.157.1 and all historical hashes/status remain unchanged.
Unknown versions still fail closed. ToolRegistry, repository authority/switching,
leases, Trusted Profiles, permissions, mutation uncertainty and remembered
workspace semantics are unchanged. HostExplicit remains **exactly 11**, verified
by the unchanged exact-allowlist test. No dependency, Rust/frontend/UI/picker,
configuration preference, version, release or tag change. Temporary homes/schema/
logs and one authenticated direct conversation are research side effects;
ephemeral/read-only settings do not imply network isolation or rollback.

ADR result: **ADR-B**, existing ADR 0030/0033 research; no new/edited ADR.

## Executed validation and publication scope

| Check | Result |
| --- | --- |
| Baseline helper verify 0.157.1 | PASS saved bundle |
| Exact executable SHA-256/size before and after | Both unchanged |
| Protocol assertions | 6 discovery shape, 7 probe, 11 fixture shape checks PASS |
| cargo test -p rah-runtime-codex --lib -- --nocapture | 102 passed / 0 failed / 1 ignored |
| cargo test -p rah-desktop --bin rah-desktop model_preflight | 4 passed / 0 failed |
| cargo test -p rah-desktop --bin rah-desktop unsupported_codex_version_frontend_error_has_one_adapter_origin | 1 passed / 0 failed |
| cargo test -p rah-desktop --bin rah-desktop host_invocation::tests::host_allowlist_is_exact | 1 passed / 0 failed |
| Candidate existing live_smoke admission | Expected VersionMismatch, exit 1, zero inference |
| Candidate bounded direct model turn | Completed, expected reply, zero Tool requests |

Tests used existing `F:/temp/rah-task509a2-target-run2` CARGO_TARGET_DIR and
RAH_TEST_TARGET_DIR. No product source edits occurred while checks ran. Two
external audit-harness errors (nonexistent standalone Thread.json, then inline
Node shell quoting) were corrected in temporary scripts; final assertions passed.
No product deterministic/live compatibility failure occurred. No full release
gate or workspace validation was run for documentation-only work.

Closure requires git diff --check, status and diff stat; commit/push and natural
exact-head CI are reported in the final return, because this report cannot record
its own future commit hash. Task 510A explicitly authorizes the coherent research
documentation commit including preserved Task 509 WIP; that publication does not
alter earlier task classifications or certify picker implementation.

## Exact next task

**Task 509E — decide provider-aware picker product policy for unbound Codex catalogs.**
Keep Task 509B/509C implementation blocked until that policy is explicitly chosen.
Task 510A does not design the fallback. Task 510B certification/adaptation is not
the ownership prerequisite's next task: 0.160.0 did not solve discovery binding.
Any separately desired runtime certification must own trust, full compatibility,
regression, production model validation and baseline policy, independently of
this report's direct compatibility success.
