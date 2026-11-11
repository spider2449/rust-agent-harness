# LLM latency breakdown

Extend the opt-in loopback measurement script with synthetic context lengths,
time to first reasoning/content delta, usage and numeric server timing fields.
Do not retain reasoning text or alter provider/server defaults.

Run sequential short and longer prompt samples without concurrent builds to
distinguish prompt processing, hidden reasoning and generation. Repeated runs
can use server prompt caching; report the server's timing fields separately
from wall-clock measurements. Synthetic records are not real session context.

Add deterministic standard-library tests for measurement and error handling.
Use the findings to choose the next optimization without speculative changes
to the kernel, service contracts or inference engine.

## Findings

### Connection overhead in the original measurement

Three health probes using Python's standard-library HTTP client took
2025-2045 ms to connect to `localhost`, versus 0.31-0.45 ms to `127.0.0.1`.
The hostname/address difference is observed; its underlying cause was not
traced. This is not evidence of an equivalent delay in RAH's reqwest client.
RAH's built-in llama.cpp endpoint already defaults to IPv4 loopback.

The script now defaults to `http://127.0.0.1:8080` and reports `connect_ms`
separately. Earlier wall-clock measurements using `localhost` include this
connection overhead and must not be interpreted as inference-only latency.

### Direct IPv4 measurements

No Rust build ran concurrently with these sequential model requests. Each
request uses temperature 0 and a 128-token output cap; all ended with `length`.
No cache flush, server setting change or model setting change was performed.

| Prompt tokens | Cached tokens | First reasoning (ms) | First visible text (ms) | Total (ms) | Server prompt (ms) | Server generation (tokens/s) |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 18470 | 18466 | 425.57 | 1535.74 | 5652.77 | 382.198 | 24.30 |
| 18470 | 18466 | 167.73 | 1392.15 | 5785.20 | 145.803 | 22.61 |
| 29 | 0 | 640.53 | 1850.78 | 5763.97 | 487.828 | 24.79 |
| 29 | 25 | 136.31 | 1360.38 | 5030.97 | 130.261 | 25.95 |
| 4646 | 0 | 15591.32 | 16629.55 | 20531.08 | 15561.422 | 25.71 |
| 4646 | 4642 | 151.36 | 1371.36 | 5681.64 | 140.551 | 22.96 |

An earlier 18470-token request, over `localhost`, reused 4130 tokens and
processed 14340 in 49300.199 ms (290.87 tokens/s). It is a partially cached
sample, not a fully cold baseline. The IPv4 4646-token sample processed its
entire prompt at 298.56 tokens/s. Generation stayed around 22-26 tokens/s.

First visible text follows 26-31 reasoning fragments in these samples.
The diagnostic records only fragment counts and times, never reasoning text.
Prompt processing and hidden reasoning explain different parts of the wait.
Results are synthetic single-request samples, not concurrent throughput or
before/after product benchmarks.

### Prompt stability inspection

- `ToolInvoker::definitions` keeps provider registration order; kernel
  `get_all` sorts by registration sequence.
- `skills-fs` sorts directories and the final skill list.
- `context-default` injects the UTC date, not a per-request clock time.
- Instruction files and memory are deliberately re-read so edits apply.
  Window movement, file edits and memory edits can legitimately change prefixes.

No accidental per-request prompt-order randomization was found in these paths.
Do not freeze changing user instructions or change reasoning defaults merely
to improve a synthetic benchmark.

## Validation and repeat instructions

Seven deterministic offline tests passed: reasoning/content timing and numeric
diagnostics, plain content without usage, EOF, missing finish reason, malformed
JSON, HTTP failure and oversized lines. All assert connection closure.

```powershell
python -B -m unittest discover -s scripts -p test_measure_llm.py
$catalog = Invoke-RestMethod 'http://127.0.0.1:8080/v1/models'
python -B scripts/measure-llm.py --model $catalog.data[0].id --runs 2 --context-lines 256
```

Use `--context-lines 0`, `256` and `1024` for synthetic length comparisons.
The cache survives between script invocations; inspect `cached_prompt_tokens`
and `server_timings` rather than assuming the first run is cold. This utility
opens one connection per measurement; RAH can reuse its provider HTTP client.

No production Rust or service contracts changed in this step. Existing
uncommitted SSE/frontend optimizations remain intact. Next useful work is
capturing the same timing breakdown around a representative RAH turn, including
its tools and session context, before changing context limits or reasoning.
