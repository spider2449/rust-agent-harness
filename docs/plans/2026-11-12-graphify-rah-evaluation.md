# Graphify evaluation through RAH

## Scope

The user requires evaluation inside RAH rather than a separate Python runner.
Replace the exploratory runner with a Rust integration test that boots the
existing distribution, builds an AST-only graph through the sandbox service,
and mounts Graphify through the existing tools-mcp plugin. Do not change the
kernel, service contracts, provider defaults or user configuration.

## Acceptance

- A dependency-only ignored smoke test builds the workspace graph and exercises
  a model-requested graph query through the real loop, ToolInvoker, policy and
  MCP client. Confirm that removing the permission assignment denies the call.
- An ignored local-model comparison runs the same source-grounded tasks in
  fresh sessions, baseline/graph/graph/baseline, with identical model settings.
  Both groups retain RAH search/read tools; the graph group additionally uses
  MCP for orientation, then reads source for verification.
- Capture provider-reported token usage, wall time, requested tools, tool
  failures, completion, answer correctness and required successful source reads.
  Missing usage stays unknown. Timeouts keep the streamed events already seen.
- Keep normal tests deterministic and offline, with negative grader cases.
- Pin Graphify 0.9.107, use uvx offline after a one-time isolated cache setup,
  and do not install assistant hooks. Build/session data are temporary; compact
  reports live under ignored target/. No shell interpolation or detached work.
- Preserve the earlier exploratory findings as historical evidence, remove
  the standalone Python script, and update its documentation references.

## Validation

Run the offline integration tests, the Graphify smoke test, and the bounded
local-model comparison when localhost:8080 is available. Run formatting,
targeted Clippy and diff checks. Report benchmark quality failures separately
from test harness execution success. No commit is authorized.

## Running the evaluation

Cache the pinned external Graphify tool once (no global installation or
assistant configuration changes):

```powershell
uvx --from 'graphifyy[mcp]==0.9.107' graphify --help
```

Run the native tests from the repository:

```powershell
cargo test -p rah-cli --test graphify_evaluation
cargo test -p rah-cli --test graphify_evaluation graphify_workspace_smoke -- --ignored --exact --nocapture
cargo test -p rah-cli --test graphify_evaluation live_graphify_workspace_evaluation -- --ignored --exact --nocapture
```

The first command is deterministic and needs neither Graphify nor a model.
The smoke test needs uvx and the cached Graphify MCP extra. The last command
additionally uses the local OpenAI-compatible model at `127.0.0.1:8080`, with
temperature 0, thinking enabled, 1024 maximum output tokens per model request,
eight tool rounds, no provider retries, and a 120-second turn deadline.
It writes `target/graphify-evaluation.json`; execution success is not a quality
pass. All samples, including timeouts and wrong answers, are retained.

RAH clears the child environment and inherits only the sandbox's platform
defaults plus `UV_CACHE_DIR` and `UV_PYTHON_INSTALL_DIR`. Graphify extraction is
AST-only and offline after cache setup. The index lives outside the source
tree in the test's temporary data directory. Its Python runtime is an upstream
Graphify implementation detail; there is no separate Python evaluation script.

Both groups boot new distributions and sessions, use the same read-only
policy and source/search tools, and disable memory/background model features.
The graph group additionally exposes only `mcp.graphify.query_graph`, explicitly
assigned read permission, and is prompted to query it before verifying source.
This compares two prompted workflows, not a blinded tool-selection study.
Setup/build and MCP startup are outside turn timing. Server caches are not
flushed. The two RAH-specific questions do not establish general repo quality.

## Results (2026-11-12)

The native integration works without a dedicated Graphify plugin or Python
evaluation runner. The smoke test passed through mock model -> loop-react ->
ToolInvoker -> policy-authority -> tools-mcp -> Graphify. Removing the explicit
read assignment denied the model-requested call before any ToolStarted event.
That test took 18.62 s, including an 11.833 s graph build.

The live run used the available local
`ornith-1.5-35b-Q4_K_M.gguf` model and completed the eight-sample diagnostic in
699.86 s. Its separate graph build took 23.991 s. Both builds emitted the
previously observed 44 symbol-free-file warning; extraction exited successfully.
The runtime report is `target/graphify-evaluation.json` (ignored by Git).

| Task | Method | Turn times (ms) | Completed | Strict answer / all checks | Input tokens by sample |
| --- | --- | --- | --- | --- | --- |
| Tool authorization | baseline | 120001, 120014 | 0/2 | 0/2 | unknown, unknown |
| Tool authorization | Graphify | 71590, 63464 | 2/2 | 0/2 | 32161, 32161 |
| Missing tool result | baseline | 72952, 67621 | 2/2 | 0/2 | 31799, 39967 |
| Missing tool result | Graphify | 78791, 71446 | 2/2 | 0/2 | 46680, 46680 |

Graphify completed 4/4 turns, while baseline completed 2/4. All eight samples
successfully read the required implementation source, all four Graphify samples
successfully queried MCP, and no tool execution errors were recorded. Both
baseline authorization turns reached the 120-second deadline while still
working; their streamed tool calls were retained. A request was still open, so
complete token totals are unknown rather than reported as zero.

Strict `answer_ok` requires exactly the requested JSON value without Markdown
or additional prose. All six completed answers put the expected values inside
JSON fences; the authorization answers also added explanation. Manual inspection
confirmed those six JSON payloads matched the expected facts, but the output
format violated the prompt. Thus both methods scored 0/4 on strict answers and
all checks, rather than six factual errors. The offline negative grader cases
cover bad JSON, wrong values, failed graph calls, wrong source paths, incomplete
turns and timeouts. These model limitations are recorded in KNOWN_ISSUES.md.

There is no established token-saving result. On the task both methods completed,
Graphify used 46,680 input tokens per turn versus baseline's 31,799 and 39,967,
and made six tool requests versus four. Baseline output usage was 415/423 tokens;
Graphify output usage was 504/504. On authorization, Graphify output usage was
546/546, while complete baseline totals are unknown. These are sums of actual
provider usage over model requests, including repeated context and cached input,
not output-size estimates. Cached input is separately retained in the report;
missing cache counts remain unknown.

The experiment suggests useful orientation for the authorization task, but does
not justify automatic graph-first routing or a default change. The replay task
demonstrates that a graph query can add work without avoiding later searches.
The sample is tiny, unblinded, and not held out: the evaluation code and expected
answers are visible in the indexed checkout. The source-read requirement checks
actual implementation evidence, but this remains a diagnostic rather than an
independent generalization benchmark. Continue with a larger task set only after
addressing formatting compliance and the known incremental metadata issue.

## Completed validation and impact

- `cargo test -p rah-cli --test graphify_evaluation --no-fail-fast`: two offline
  grader/usage tests passed; two external/live tests ignored by default.
- `graphify_workspace_smoke -- --ignored --exact --nocapture`: passed, including
  allowed and missing-assignment paths through the actual RAH loop.
- `live_graphify_workspace_evaluation -- --ignored --exact --nocapture`: the
  diagnostic completed and saved all eight rows; quality failures are above.
- `cargo fmt --check`, targeted Clippy with `-D warnings`, and `git diff --check`
  passed. This test/documentation change did not require a workspace release
  test run.

No production Rust behavior, dependency edge, ADR, plugin protocol or service
contract changed. User configuration and model defaults were not modified.
The Python runner was removed, historical retrieval results were preserved,
and no commit was created.
