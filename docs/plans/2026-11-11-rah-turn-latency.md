# Real RAH turn latency

Add optional first-reasoning timing to the telemetry plugin's model-request
records, alongside existing first-text timing. This distinguishes provider
progress from visible text without logging reasoning content. No model event,
kernel or service contract change is needed.

Add deterministic tracker tests and an explicitly ignored, opt-in localhost
integration measurement. Run the real loop, default tool/context plugins,
policy and session log against the user-authorized loopback model server in
an isolated temporary workspace. Read only synthetic data. Compare a tool
round and a follow-up in the same session; report cached usage and timings.

Do not change provider/server reasoning, context or permission defaults for
production. Do not claim a before/after speedup from this diagnostic change.

## Results

The explicitly invoked live integration test passed all three turns against
ornith-1.5-35b-Q4_K_M.gguf via Chat Completions. It mounted the real base
tool/context plugins with read-only policy and telemetry, using an isolated
temporary workspace and 128 synthetic file records. Background titles,
scheduler, skills and plugin builder were omitted. Temperature was 0 and
the per-request output cap was 512. No build ran concurrently with inference.
All answers contained the verification code, and only the two requested
`fs.read` calls executed, one per read turn.

| Turn | Model request | Input tokens | Cached tokens | First reasoning (ms) | First text (ms) | Request duration (ms) |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Initial read | Tool selection | 2970 | 0 | 12825 | none | 19783 |
| Initial read | Answer from file | 5331 | 2966 | 8794 | 9487 | 9865 |
| Same-session follow-up | Answer from history | 5363 | 2454 | 11208 | 12640 | 13031 |
| Fresh session, same read | Tool selection | 2970 | 2454 | 2584 | none | 9394 |
| Fresh session, same read | Answer from file | 5331 | 2966 | 8738 | 9407 | 9774 |

Turn durations were 29653, 13033 and 19172 ms respectively. Each `fs.read`
took 1 ms. Per-request first-text times are not turn-level time to first text:
the initial read must finish tool selection and execution before its answer
request begins. These are single samples, not a statistically controlled
before/after performance comparison. Requests include hooks/HTTP/streaming;
telemetry does not expose server-only prompt-processing duration.

### Confirmed template prefix change

The local server's `/props` exposes its served template. Assistant messages
after `last_query_index` receive `<think>\n...\n</think>`; older assistant
messages do not. A follow-up user message changes `last_query_index` and hence
the rendered representation of the earlier assistant tool-call message.

Two synthetic `/apply-template` calls confirmed this without inference: an
unchanged system/user/tool-call/tool-result history rendered to 381 characters;
adding an assistant answer and follow-up user message rendered to 457. Their
common prefix ended at character 128, where the old history began a `think`
block and the new history began `tool_call`. This explains a mechanism for
cache loss, but does not quantify its exclusive contribution to the live
timing difference. No template or reasoning setting was changed.

The issue is recorded in `docs/KNOWN_ISSUES.md`. RAH should not change a
model's prompt template or reasoning policy as an implicit optimization.

## Validation

- `cargo test -p rah-plugin-telemetry-log`: 11 passed.
- `cargo test -p rah-cli --test telemetry -- --nocapture`: deterministic
  integration passed; live test ignored by default.
- `cargo test -p rah-cli --test telemetry live_loopback_turn_latency_with_tools_and_history -- --ignored --nocapture`:
  live measurement explicitly run and passed in 61.90 seconds.
- Telemetry Clippy with warnings denied, formatting and diff checks passed.

The telemetry change adds only an optional `first_reasoning_ms` JSON field.
There is no kernel, model service contract, protocol or dependency change.
Existing telemetry record consumers should tolerate additive JSON fields.
All prior uncommitted optimizations remain in the worktree; nothing is committed.

Next useful experiment: an explicitly chosen alternate reasoning/template
configuration, compared on both response latency and tool/answer correctness.
That is a behavior/quality tradeoff, not a transparent host-side optimization.
