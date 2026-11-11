# Incremental live output

The Web frontend replaces all accumulated live text on every model delta.
Append incoming fragments to a single text node instead, and release the text
when a turn ends. Preserve plain-text rendering and scroll-follow behavior.

## Validation

- Browser regression coverage for many fragments, literal HTML, node reuse,
  scrolling and clearing between turns.
- Offline browser comparison of full-text replacement and incremental append.
- Check the user-authorized localhost:8080 service when available.
- Run formatting, relevant tests and diff checks.

No kernel, model service contract, dependencies or inference settings change.

## Results (2026-11-11)

- Edge browser regression test passed, including 1,000 Unicode/HTML fragments,
  preserving a single text node, clearing between turns, following output and
  preserving a user's scroll position.
- Detached DOM comparison: 3,000 fragments of 64 ASCII characters, one warmup
  per implementation and three measured samples with alternating order.
  Full replacement: 233.3, 231.3, 231.5 ms; append: 6.9, 6.2, 6.7 ms.
  This isolates mutation cost; it excludes layout, network and inference.
  Timing is diagnostic and is not a pass/fail test threshold.
- localhost:8080 reported healthy, serving ornith-1.5-35b-Q4_K_M.gguf.
  Three direct Chat Completions streams with a 128-token cap reported:

| Run | First visible text (ms) | Total (ms) | Completion tokens | Tokens/s including prefill |
| --- | ---: | ---: | ---: | ---: |
| 1 | 4354.07 | 8498.58 | 128 | 15.06 |
| 2 | 4387.64 | 8849.98 | 128 | 14.46 |
| 3 | 3977.71 | 8882.19 | 128 | 14.41 |

All streams ended with `length` because the benchmark deliberately caps output.
These are initial server baselines, not before/after RAH comparisons. Rust
compilation was running concurrently; repeat on an idle machine for a controlled
baseline. Identical prompts may benefit from server prompt caching. First
visible text can follow reasoning output, so it is not necessarily first token.

Follow-up measurements in [LLM latency breakdown](2026-11-11-llm-latency-breakdown.md)
found about two seconds of connection overhead in this Python client's
`localhost` requests. The values above include that overhead and are not
inference-only times. Use IPv4 loopback and the updated script's connection and
reasoning diagnostics for subsequent measurements.

Repeat the explicitly opt-in live measurement with Python's standard library:

```powershell
$catalog = Invoke-RestMethod 'http://127.0.0.1:8080/v1/models'
python scripts/measure-llm.py --model $catalog.data[0].id --runs 3 --max-tokens 128
```

The script does not retry failed requests or change server settings. Offline
checks covered successful usage reporting, EOF without DONE, malformed JSON,
HTTP failure and connection closure. Formatting, JavaScript syntax and diff
checks passed. The earlier provider test run passed all 49 tests.

Next: measure realistic long-context requests on an idle machine to separate
prompt processing from generation and hidden reasoning time before considering
any model or server configuration change.
