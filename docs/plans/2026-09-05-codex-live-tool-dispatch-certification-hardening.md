# Task 207A — Codex live Tool-dispatch certification hardening

## Objective

Harden the opt-in Windows Codex 0.149.0 live certification for RAH-owned MCP
and Process Plugin tools so a correct final assistant message cannot substitute
for observed Tool lifecycle and provider execution evidence.

## Why Task 207 was paused

Task 207 was inconclusive because `scripts/codex-live-gate.ps1` defaulted to
`gpt-5.4`, which the ChatGPT-auth Codex path rejected as unsupported. The
canonical MCP test also used the predictable `RAH_MCP_BRIDGE_OK` echo. A live
run with `gpt-5.6-terra` returned that marker directly with
`requested=0`, `started=0`, and `finished=0`. The existing standalone gate
correctly failed, but its proof could be satisfied by model text if the
lifecycle checks were weakened.

## Codex 0.149.0 tool-forcing research

The certified binary was queried locally with `codex app-server --help` and
`codex app-server generate-json-schema --experimental`. The generated
0.149.0 schema contains `thread/start.dynamicTools`, `DynamicToolCallParams`,
and `DynamicToolCallResponse`. `TurnStartParams` contains only `threadId` and
`input`; there is no `tool_choice`, required-tool flag, exact-tool selector, or
equivalent host request. The CLI help also exposes no such app-server option.

Result: Codex 0.149.0 supports neither exact Tool choice nor generic required
Tool dispatch at this adapter boundary. Tool invocation remains model-selected.
Prompt text cannot serve as proof of Tool execution. The adapter and Desktop
production semantics were therefore left unchanged.

## Hardened proof design

The live-only MCP and Process Plugin fixture modes accept a fixed bounded
request, `{"request":"certification-token"}`, while the trusted live harness
generates a fresh UUID-v4 nonce. The nonce is passed only as a fixture process
argument and is validated to a bounded hexadecimal format. It is absent from
the prompt, RAH Tool description, input schema, provider metadata, repository
files, and model-visible environment. After actual provider execution, the
fixture returns the nonce and an auditable execution count. Normal fixture echo
behavior remains the default.

The shared live proof requires exactly one RAH public Tool identity and exact
arguments, one requested/started/finished lifecycle, successful ToolFinished
output containing the hidden nonce, provider execution count one, a model
delta after ToolFinished, and terminal `Completed`. It rejects the private
`rah_tool_N` alias as public evidence. Copied provider executables are renamed
after shutdown to prove child reaping and Windows handle release.

## Model and binary baseline

The live gate now explicitly selects `gpt-5.6-terra`; it remains in the gate
output and configuration fingerprint, with isolated temporary `CODEX_HOME`, no
fallback, and Codex-owned MCP, plugins, apps, code mode, browser/computer, and
image-generation surfaces disabled. The selected model was present in the
local ChatGPT-auth model catalog and completed live generation. Official OpenAI
model documentation lists GPT-5.6 Terra with function calling and tool support:
<https://developers.openai.com/api/docs/models/gpt-5.6-terra>.

The Codex baseline remains exactly `codex-cli 0.149.0`, SHA-256
`14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

## Deterministic validation

The shared proof tests cover hidden-token placement, bounds and malformed
tokens, wrong identity and arguments, requested-zero marker claims, missing
start/finish, ToolFinished errors, duplicate calls, provider counts other than
one, missing continuation, nonterminal completion, and private aliases. MCP
and Process Plugin fixture tests cover malformed/oversized live configuration
and copied-executable renameability after shutdown.

The focused and workspace validation results are recorded in the completion
report below.

## Windows live results

MCP with the certified 0.149.0 binary and explicit `gpt-5.6-terra` reached the
model but Codex completed without a dynamic-tool request. Observed lifecycle
was `requested=0`, `started=0`, `finished=0`; provider execution was zero.
The copied provider child recorded spawn/shutdown/exit and was renameable after
shutdown. The hardened gate failed closed.

Process Plugin produced the same external result: `requested=0`, `started=0`,
`finished=0`, provider execution zero, then clean child shutdown and rename.
Both live certifications are INCONCLUSIVE because the model did not provide
the required proof. This is not a reproducible RAH implementation defect.

## Security, non-goals, and status

This task changes certification fixtures and proof assertions only. It does
not add authority, change Trusted Profile or ToolRegistry enforcement, enable
Codex-owned capabilities, add shell/process/filesystem/network authority, or
alter reviewed repository mutation boundaries. It does not claim OS sandboxing
inside provider children. Live validation is Windows-only; normal tests remain
offline and credential-free. Task 207 remains INCONCLUSIVE and Task 208 is not
started.

## Scope and non-goals

This is certification infrastructure only. It does not change RAH authority
composition, Trusted Profiles, ToolRegistry authorization, Codex-owned tools,
or repository mutation boundaries. It does not begin Task 208.

## Validation evidence

- Codex forcing research: complete; no exact or required-tool control in the
  pinned app-server contract.
- Live model: `gpt-5.6-terra`; old default `gpt-5.4` rejected by the prior live
  path.
- Deterministic proof, MCP, Process Plugin, and Desktop tests: passed as listed
  in the final report; the full validation commands are run for this task.
- Windows MCP live: INCONCLUSIVE, `0/0/0`, provider execution `0`, clean child
  reaping/unlock.
- Windows Process Plugin live: INCONCLUSIVE, `0/0/0`, provider execution `0`,
  clean child reaping/unlock.

### Commands

- `cargo fmt --check`: PASS
- `cargo check --workspace`: PASS
- `cargo test --workspace`: PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS
- `git diff --check`: PASS
- `cargo metadata --no-deps --format-version 1`: PASS
- `cargo test -p rah-runtime-codex`: PASS, 82 passed, 1 ignored
- `cargo test -p rah-tools-mcp`: PASS, 31 integration tests passed
- `cargo test -p rah-tools-plugin --test process_echo`: PASS, 15 passed
- `cargo test -p rah-desktop`: PASS, 157 passed, 2 ignored

No commit, push, or exact-head CI run was performed because Task 207A did not
meet its PASS rule. Task 207 remains INCONCLUSIVE; the next task is the Windows
Desktop External Provider Live Certification, and Task 208 remains unopened.
