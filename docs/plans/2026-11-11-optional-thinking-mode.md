# Optional thinking mode for Chat Completions

Expose a typed, optional `enable_thinking` configuration in the Chat Completions
provider. When explicitly configured, send only the provider-specific
`chat_template_kwargs.enable_thinking` field. When unset, preserve the current
request exactly. Do not change the model service contract, server properties,
prompt templates or saved user configuration.

llama.cpp documents this per-request template parameter:
https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md
The user-authorized local server's `/apply-template` also confirmed that false
closes the initial thinking block and true leaves it open.

Validate typed config rejection, omitted/false/true wire behavior, cancellation
and existing provider fixtures. Compare real read-only RAH tool/history turns
with thinking enabled and disabled; include a small arithmetic correctness
probe. These limited probes cannot establish general quality equivalence.

Document the opt-in setting and tradeoff. Preserve the default and record the
measured outcome, including failures. No architecture or dependency change.

## Implementation

- `model-chat-completions`: `Option<bool>` config and an exact, provider-local
  `chat_template_kwargs` mapping. No arbitrary request-field injection.
- Unset omits the entire field; false and true remain distinct.
- Configuration documentation and plugin help describe support and tradeoffs.
- The ignored live comparison reports per-probe answer correctness and exact
  tool-plan adherence. Completion is a measurement success, not a quality gate.
  The default-mode live regression still requires every quality check to pass.

## Live results (2026-11-11)

Same local ornith-1.5-35b-Q4_K_M.gguf server, read-only temporary workspace,
synthetic file, temperature 0 and max_tokens 512. Both modes used the same
workspace path in a sequential run, true followed by false. No build ran during
inference. The server cache was not cleared and sample order was not randomized.
These are individual observations, not general latency guarantees.

| Task | Thinking on (ms) | Thinking off (ms) | On/off output tokens | On/off answer and exact tool-plan check |
| --- | ---: | ---: | --- | --- |
| Read file and report code | 15546 | 13010 | 104 / 38 | pass / pass |
| Follow up from same-session history | 12982 | 11673 | 46 / 11 | pass / pass |
| Same read in a fresh session | 15449 | 13034 | 104 / 38 | pass / pass |
| Arithmetic (17 * 23) | 12112 | 11895 | 19 / 4 | pass / pass |

The enabled read requests reused 2454 then 2966 input tokens; disabled requests
reused 2454 then 2998. Follow-up and arithmetic requests reused only 2454 tokens
in both modes. Disabled responses had no reasoning signal. Prompt processing
and the existing history-template cache issue still dominated many requests:
disabling reasoning did not remove that cost.

### Observed quality failure

The first comparison run failed its original strict tool-plan assertion after
70.83 seconds: disabling thinking requested `echo` and then `fs.read`, despite
the prompt requiring only `fs.read`. The answer did contain the correct code.
The benchmark was changed to report per-probe quality mismatches and continue,
so all probes can be measured. A subsequent full run passed all four probes
in both modes and completed in 105.79 seconds. The earlier failure remains
recorded in `docs/KNOWN_ISSUES.md` and must not be counted as a pass.

These probes establish neither broad quality equivalence nor a regression
rate. Preserve the existing default; make the tradeoff explicit to users.

## Validation

- Provider fixtures: 28 tests passed, including default omission, false/true
  wire values and invalid non-boolean configuration.
- Deterministic CLI telemetry integration passed; live tests stay ignored
  unless explicitly requested.
- Explicit live mode comparison completed with per-probe results above.
- CLI library tests: 58 passed. Provider all-target Clippy and CLI library/
  telemetry-test Clippy passed with warnings denied; formatting and diff
  checks passed.

Repeat live validation only when a local model is available and inference is
authorized:

```powershell
cargo test -p rah-cli --test telemetry live_loopback_thinking_mode_comparison -- --ignored --nocapture
```

No saved user settings, global server properties, kernel/API/protocol contracts
or crate dependency edges changed. All work remains uncommitted. Next useful
evaluation is a representative task set with repeated, alternating mode order;
do not make this the default based on these small synthetic samples.
