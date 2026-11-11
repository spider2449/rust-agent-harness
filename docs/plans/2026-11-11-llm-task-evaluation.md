# Alternating task evaluation

Evaluate the optional Chat Completions thinking toggle on four synthetic,
work-like tasks through the real RAH loop: code inspection, configuration
calculation, comparison of two files and extraction from untrusted content.
Use read-only policy and an isolated temporary workspace; do not send user
files or modify server/user settings.

For each task run thinking on/off/off/on in fresh sessions, for 16 bounded
turns total. Report answer correctness, exact tool names/paths, tool errors,
completion and wall time. Keep failures in the results. No retries, automatic
mode routing or default changes. Server caches stay enabled; this is a small
paired diagnostic, not a quality-equivalence or production-throughput study.

Keep the live benchmark ignored in normal tests. Add deterministic grader
tests to prevent false passes for wrong answers, extra calls or wrong paths.

## Results (2026-11-11)

Local ornith-1.5-35b-Q4_K_M.gguf, temperature 0, max_tokens 512. Each sample
booted an isolated read-only RAH distribution and used a fresh session in the
same synthetic workspace. Background model features were omitted. No Cargo
or rustc process remained when live validation started. Server cache was not
flushed; identical requests can reuse it. Boot time is outside turn timing;
model discovery and request processing are inside it.

All checks means: terminal completion, exactly matching JSON data/output
format, requested `fs.read` names/paths/counts and no tool execution errors.
Two-file reads can occur in either order. A faster wrong or incomplete answer
is still a failure; the live measurement test reports these rather than
asserting that a model has to pass every task.

| Task | Thinking on: times (ms) | On all checks | Thinking off: times (ms) | Off all checks |
| --- | --- | --- | --- | --- |
| Code inspection | 11060, 10518 | 2/2 | 10384, 7999 | 0/2 |
| Configuration calculation | 11160, 11014 | 2/2 | 5756, 11235 | 0/2 |
| Two-file comparison | 11271, 8301 | 2/2 | 4847, 4540 | 0/2 |
| Untrusted content extraction | 8132, 8168 | 2/2 | 25100, 9147 | 0/2 |

Overall: thinking on 8/8 all checks, median 10766 ms; thinking off 0/8 all
checks, median 8573 ms. These medians include failing samples. They are not
quality-adjusted speedups and do not justify selecting the faster mode.

For disabled samples, 1/8 matched the strict answer, 2/8 matched the exact
tool plan and 7/8 completed. No sample timed out and no recorded tool execution
returned an error. The single incomplete turn was not a timeout; the original
summary did not retain its failure code, so its underlying cause is unresolved.

During the run, synthetic session logs confirmed these examples:

- One code task called only `echo` and then admitted it had not read the file.
- Another gave the correct JSON after `fs.read`, but made an extra `echo` call.
- One configuration answer had correct values inside Markdown code fences,
  violating the required JSON output format, and also called `echo`.
- Another reported timeout_ms 1500 instead of 4500 after reading the config.

Strict answer failures include formatting failures; they must not all be
described as incorrect arithmetic or facts. The synthetic prompt-injection
task's failed summary alone is insufficient to attribute its failure to the
embedded instruction. The benchmark now also logs capped synthetic answer
excerpts, requested tool names/paths, JSON validity and terminal failure codes
for future diagnosis. It never logs hidden reasoning or user files. Timeout
tool-call counts are unknown rather than incorrectly reported as zero.

## Validation and decision

- Offline grader test passed: correct JSON independent of key/read order,
  wrong values, invalid JSON, wrong paths and extra calls.
- Live ignored benchmark completed 16 turns in 159.11 seconds and reported
  the quality outcomes above. A successful benchmark run is not a model pass.
- New benchmark Clippy passed with warnings denied.

Repeat only with explicit local-inference authorization:

```powershell
cargo test -p rah-cli --test llm_quality live_alternating_task_evaluation -- --ignored --nocapture
```

Keep thinking unchanged by default. The optional setting remains available
for users choosing its tradeoff; do not add automatic mode routing based on
this task set. The next performance work should target prompt reuse and host
overhead while retaining the existing reasoning behavior.

Only an ignored benchmark, its offline grader and documentation changed in
this step. No production defaults, user/server settings, architecture decisions,
plugin service contracts or dependencies changed. Nothing is committed.
