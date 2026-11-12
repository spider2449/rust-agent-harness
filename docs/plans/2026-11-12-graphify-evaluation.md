# Graphify AST-only retrieval evaluation

Historical exploratory results. The standalone Python runner was removed at
the user's request. The current evaluation runs inside RAH; see
[Graphify evaluation through RAH](2026-11-12-graphify-rah-evaluation.md).

## Scope

Evaluate Graphify 0.9.107 against the RAH source tree without changing runtime
behavior, installing assistant hooks, modifying user configuration, or calling
an inference provider. Use `uvx` for an isolated, pinned tool environment and
keep generated graphs and raw measurements outside the repository.

## Method

- Freeze eight architecture/navigation questions and required source files
  before the measured queries. Derive references from source, not graph output.
- Compare Graphify natural-language BFS queries (depth 2, budget 2000) with
  ripgrep using explicit, question-derived symbol terms, scoped to Rust source
  under crates/ and plugins/, including tests. This is a symbol-informed
  baseline, not an autonomous agent.
- Record three alternating CLI samples per query, exit codes, wall time,
  UTF-8 output bytes, approximate output tokens (characters / 3), required-file
  recall, and source-file precision. Preserve raw output for inspection.
- Measure fresh and unchanged cached AST-only builds with Cargo introspection
  and no clustering. Record versions, source revision, graph counts, parse
  warnings, and errors. Exclude package download/setup time from build timing.
- Inspect selected graph relationships against source and test a small isolated
  changed-file update fixture. Never modify the real source to test indexing.

## Interpretation

Required-file recall measures navigation, not factual answer accuracy. A file
reference does not establish that the answer is present or correct. Output
token estimates are not tokenizer counts, billed tokens, or end-to-end agent
cost. CLI timing includes startup and graph loading; it does not measure a
persistent MCP server. No-cluster results do not evaluate community quality.
The question set is small, selected with repository knowledge, and unblinded.

## Deliverables

A compact result report and external raw outputs. The original standalone
measurement script has been superseded by the native RAH integration test.

## Results (2026-11-12)

Decision: keep Graphify as an optional structural exploration tool. This small
diagnostic does not justify replacing text search, enabling graph-first hooks,
or implementing a dedicated RAH plugin. A later agent trial can use the existing
`tools-mcp` plugin with explicit read permissions and source verification.

Measured source revision: `d57f0e46611f878ca321eb7dbc101d0df7ea8c80`.
The evaluation script was an additional untracked code file during extraction;
no production Rust files changed. The result JSON includes SHA-256 hashes of
all 203 Rust files under crates/ and plugins/. The graph also includes other
supported code/configuration files, whereas ripgrep searches Rust files only.

Environment: Windows, uv 0.11.8, ripgrep 15.2.0, Python 3.11.15 for the runner
and Python 3.14.0 in the uvx Graphify environment. Package setup was completed
before timing. The runner removes inherited `PYTHONHOME` only from child
environments: the local Python launcher otherwise causes an `SRE module
mismatch` when its Python 3.11 home leaks into uvx's Python 3.14 environment.
No host configuration was changed.

### Build and update

The final measured fresh build took 9.599 s and found 277 code/configuration
files, producing 6,460 nodes and 18,210 edges. The unchanged scan took 8.688 s,
reported all 277 files unchanged, and produced 6,421 nodes and 18,105 edges.
The earlier exploratory run took 16.724 s fresh and 10.128 s unchanged, so these
single-run build timings should not be treated as stable throughput estimates.
Queries below use the final unchanged graph (6,247,048 bytes).

The unchanged build removed 39 `pkg_*` package nodes and 105 edges. The separate
39 `crate:*` nodes and 104 `crate_depends_on` edges survived. A minimal one-package
fixture reproduced the package-node loss with `--cargo`: five nodes became four
without a source change. The same fixture without `--cargo` preserved its node
set. This is recorded in [KNOWN_ISSUES.md](../KNOWN_ISSUES.md).

The fixture also changed `entry() -> helper()` into `entry() -> replacement()`.
The old function disappeared and the new call edge was present, in about 0.95 s.
This checks replacement within one source file; it does not validate all
cross-file rename, deletion, watcher, or concurrent-update behavior.

The tool warned that 44 classified files yielded no symbols, mostly manifests
and module-only files. That warning alone does not establish parse failure.
The no-cluster output has no `graph.schema_version` or `graph.graphify_version`
metadata; consumers of this raw output should pin the tool/format rather than
assume the README's versioned clustered output contract applies.

### Retrieval

Each method ran three samples per question, alternating method order. All 48
query commands returned exit code zero. Each method found every required file
for all eight questions. This is file navigation success, not answer accuracy.

| Question | Required files | rg median ms | Graphify median ms | rg output bytes | Graphify output bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| ToolInvoker authorization / ToolBeforeInvoke | 2 | 23.5 | 1273.5 | 10,565 | 2,290 |
| Process host service access / requires | 1 | 23.7 | 1313.7 | 13,240 | 6,784 |
| rebuild_context / missing tool result | 1 | 20.5 | 1288.4 | 1,259 | 5,744 |
| ShellToolsPlugin / ExecEnv | 2 | 23.6 | 1149.1 | 6,054 | 6,689 |
| ContextPlugin / ReactLoop / ModelBeforeRequest | 2 | 22.8 | 1270.4 | 6,076 | 6,721 |
| stop_cascade / scope disposal | 2 | 22.7 | 1267.3 | 27,603 | 6,822 |
| ToolInvoker / uncertain effects | 1 | 21.0 | 1295.4 | 20,226 | 6,653 |
| Architecture dependency tests | 1 | 21.3 | 1389.4 | 180 | 7,012 |

Across the 24 samples per method, median CLI latency was 22.435 ms for ripgrep
and 1283.485 ms for Graphify, approximately 57 times slower. This includes CLI
startup and graph loading, so it does not predict warm persistent MCP latency.

Average output was approximately 3,550 vs 2,029 tokens using characters / 3,
about 43% less for Graphify. This comparison gives Graphify a soft 2,000-token
budget but leaves ripgrep unbounded. It is a description of these outputs,
not evidence of end-to-end token or billing savings. No model was called, and
follow-up source reads were not included. Graphify output exceeded its nominal
budget because headers and truncation notices add text.

The mean fraction of returned files on the minimal required list was 19.31%
for ripgrep and 19.15% for Graphify. Other returned files may also be relevant;
these are not adjudicated relevance-precision scores. Graphify returned a
stable output across repetitions; parallel ripgrep traversal sometimes changed
line order while preserving file sets and output sizes.

### Source audit and limits

- Correct structural edges included `ToolInvoker -> .invoke()` ownership,
  the loop-react import of `ToolInvoker`, `.apply() -> host_services()`, and
  `stop_cascade() -> dispose()`, checked against source locations.
- The measured graph did not connect loop-react's `.tool_round()` to
  `ToolInvoker.invoke`, despite the call at `plugins/loop-react/src/lib.rs:488`.
  Likewise `.decide()` did not have a resolved call edge for the generic
  `kernel.bail::<ToolBeforeInvoke>` invocation. File recall does not establish
  a complete tool authorization or service/hook execution path.
- The authorization question selected an unrelated HTTP `.authorized()` seed.
  The architecture-test question expanded to 1,304 nodes and displayed only
  69, despite including both exact test names. A precise ripgrep lookup found
  the two declarations in 180 bytes. Natural-language graph search can expand
  far beyond the useful answer.
- AST-only extraction excludes ADR/document semantics. Neither architectural
  compliance nor behavioral correctness follows from a graph edge or its
  `EXTRACTED` label. Continue to inspect code and run architecture tests.
- Questions include known symbols, use English, and were selected with RAH
  knowledge. Results do not cover blind onboarding, Chinese queries, agent
  task completion, answer correctness, MCP transport, or source-read costs.

### Historical artifacts

The removed exploratory runner used a pinned tool in uv's isolated cache,
without assistant skills/hooks or RAH configuration changes. Retained outputs
include raw stdout/stderr, full fresh/unchanged graph snapshots, per-command
timing, references, metrics, source hashes and the update fixture. These are
historical retrieval measurements, not native RAH agent performance results.

The retained final run is at
`D:\spider\working\graphify-rah-evaluation-2026-11-12\final\results.json`.
Its `fixture_without_cargo` is the control; `fixture` contains the subsequent
`--cargo` reproduction under `final/cargo-repro/`. Graph snapshots and raw
outputs stay outside Git. Current test commands are in the native evaluation
plan linked above.

Validation completed: two final AST-only builds, 48 retrieval commands,
fixture initial/unchanged/changed assertions, source relationship inspection,
and Python syntax/metric checks. The unchanged-package invariant failed as
reported above. No Rust behavior, dependency, ADR or service contract changed;
workspace compilation/tests were not required for this research-only patch.
