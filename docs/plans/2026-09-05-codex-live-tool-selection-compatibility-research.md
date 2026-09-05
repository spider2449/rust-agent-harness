# Task 207B — Codex live Tool-selection compatibility research

## Result

**Task 207B: PASS.** The tested evidence supports an external model/runtime
selection limitation, not a reproducible RAH defect and not a Codex 0.149
dynamic-tool protocol incompatibility.

The current ChatGPT-auth path accepted both tested visible models on the pinned
0.149.0 baseline. `gpt-5.6-terra` and `gpt-5.6-sol` both completed without a
dynamic-tool request in both hardened provider fixtures. The installed newer
`codex-cli 0.153.4` produced the same negative result with Terra through the
same adapter path and restricted configuration. The model-generated statements
that it would call, or had called, a tool were not treated as evidence.

Task 207 remains **BLOCKED / INCONCLUSIVE** on the external model-selected
Tool invocation requirement. Task 208 was not started.

## Starting state and checkpoint

- Task 207A: INCONCLUSIVE; no dynamic Tool was invoked by the current
  ChatGPT-auth model runs.
- Task 207A checkpoint commit:
  `514fbb91eb8c0c8150c47aa190e84123b1646e3a`
  (`test: checkpoint hardened Codex live certification`).
- Starting `HEAD` for research:
  `514fbb91eb8c0c8150c47aa190e84123b1646e3a`.
- Worktree was clean before research.
- Certified baseline remained exactly `codex-cli 0.149.0`, binary SHA-256
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.

## Model evidence

The local ChatGPT-auth model cache listed these visible coding candidates:
`gpt-6-astra`, `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.5`,
and `gpt-5.4-mini`. The smallest useful matrix selected the already accepted
Terra model plus the sibling Sol model. Hidden models and the reserve/review
entries were not used. No model name was guessed.

Every live run used the Task 207A hardened fixture contract:

- fresh per-run hidden nonce passed only to the provider fixture;
- nonce absent from prompt, Tool description, schema, metadata, and model
  environment;
- exact public RAH Tool identity and exact
  `{"request":"certification-token"}` arguments;
- provider execution audit and hidden nonce output verification;
- one requested, started, and finished lifecycle;
- model continuation after ToolFinished and terminal `Completed`;
- isolated temporary Codex home with copied auth, no Codex-owned MCP, plugins,
  apps, code mode, browser/computer, or image-generation surfaces;
- provider child shutdown/reap and post-shutdown executable rename proof.

## Codex 0.149.0 model matrix

All runs below were accepted by the isolated ChatGPT-auth path. A lifecycle
value of `0/0/0` means requested/started/finished respectively.

| Model | Fixture | Codex | Lifecycle | Provider execution | Terminal | Result |
|---|---|---|---:|---:|---|---|
| `gpt-5.6-terra` | standalone MCP | 0.149.0 | 0/0/0 | 0 | Completed | failed closed; model said tool host was disabled |
| `gpt-5.6-terra` | standalone Process Plugin | 0.149.0 | 0/0/0 | 0 | Completed | failed closed; model said tool host was disabled |
| `gpt-5.6-sol` | standalone MCP | 0.149.0 | 0/0/0 | 0 | Completed | failed closed; model said code-mode host was disabled |
| `gpt-5.6-sol` | standalone Process Plugin | 0.149.0 | 0/0/0 | 0 | Completed | failed closed; model said code-mode host was disabled |

None of these runs satisfied Tool-dispatch success. The provider lifecycle
audits recorded spawn, shutdown, and exit with zero calls, and copied fixture
executables were renameable after shutdown.

## Newer Codex comparison

The newer native Codex was already installed globally and was not copied into
or made the certified baseline:

- version: `codex-cli 0.153.4`;
- source: global npm package `@openai/codex@0.153.4`, Windows x64 native
  binary under its `codex-win32-x64` package;
- binary SHA-256:
  `444a3f0008050605cae73cd9b7a2dcac61294062dfaab56dd20430fd6498518b`;
- model: `gpt-5.6-terra`;
- auth/config: same copied ChatGPT-auth mode and restricted temporary config;
- RAH path: same hardened MCP and Process Plugin fixtures and same adapter
  behavior, exercised with a temporary research-only version pin that was
  fully reverted before documentation.

| Fixture | Codex 0.153.4 lifecycle | Provider execution | Terminal | Result |
|---|---:|---:|---|---|
| standalone MCP | 0/0/0 | 0 | Completed | no dynamic-tool request |
| standalone Process Plugin | 0/0/0 | 0 | Completed | no dynamic-tool request |

The newer comparison did not change actual Tool dispatch. A repeat was not
required by the migration rule because the comparison was negative in both
provider modes; there was no positive result to distinguish from a one-off.

## Protocol comparison

Both binaries generated app-server schemas successfully with the experimental
flag. The selected artifacts had these results:

| Artifact | 0.149.0 vs 0.153.4 |
|---|---|
| `DynamicToolCallParams.json` | byte-identical; properties `arguments`, `callId`, `namespace`, `threadId`, `tool`, `turnId`; required `arguments`, `callId`, `threadId`, `tool`, `turnId` |
| `DynamicToolCallResponse.json` | byte-identical; properties/required fields `contentItems`, `success` |
| `v2/ThreadStartParams.json` | byte-identical; includes experimental `dynamicTools`; function/namespace dynamic-tool shapes and `deferLoading` are unchanged |
| `v1/InitializeParams.json` | byte-identical; `clientInfo` remains required |
| `v2/TurnStartParams.json` | additive fields only in 0.153.4: `cyberAccessProgram`, `serviceTierForTurn`, `toolOutput`, `turnTrigger` |

The newer schema also added unrelated protocol files and notifications. It did
not add `tool_choice`, an exact-tool selector, a generic required-tool flag, or
another host-side forcing field. The 0.153.4 runtime accepted RAH's existing
`thread/start.dynamicTools`, `turn/start` fields, dynamic function shape, and
restricted configuration, then emitted ordinary additive notifications while
the model completed without requesting the advertised Tool.

The current official app-server documentation still describes `dynamicTools`
as an experimental `thread/start` field and describes the `item/started` →
`item/tool/call` → client response → `item/completed` lifecycle. It documents
no host-side exact or required Tool selection control. See:

- <https://developers.openai.com/codex/app-server/>;
- <https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md>.

Therefore RAH's 0.149 adapter assumptions remain semantically valid for the
tested 0.153.4 protocol surface. Additive schemas do not explain the missing
Tool request.

## Compatibility conclusion

The evidence points to model-selected Tool usage being refused or suppressed
by the current ChatGPT-auth Codex model/runtime service path under this
restricted configuration. It does not show that 0.149 fails to advertise or
transport dynamic Tools: both versions accepted the advertisement and protocol
contract, and the 0.149 adapter reached successful model turns. It also does
not show a newer CLI resolving selection: 0.153.4 reproduced the same result
with the same model and both external provider fixtures.

This is a clear negative compatibility conclusion for the tested versions, but
not a claim that every current model behaves identically. No baseline migration
is justified solely by this issue.

## RAH product defect review

**No.** No reproducible RAH defect was found.

- The advertised schema was the expected private Codex alias mapped to the
  expected public RAH identity.
- Both app-server versions accepted the existing dynamic-tool fields.
- The hardened bridge recorded no dropped `item/tool/call`, no malformed
  request, no result-translation error, and no provider execution to lose.
- The exact hidden provider nonce never entered model-visible data.
- Provider and Codex child lifecycle cleanup completed and was independently
  checked.

If a future run produces `ToolRequested` without the subsequent RAH/provider
proof, it must be investigated as a separate adapter or fixture defect rather
than converted into a pass.

## Baseline migration decision

Evidence is **insufficient** for migration. The required positive condition was
not observed: newer Codex plus the same model and hardened fixture did not
produce a real Tool lifecycle where 0.149 did not.

Do not migrate the certified baseline in Task 207B. A future
`Task 207C — Codex Baseline Migration Research / Revalidation` is warranted only
if a later newer version repeatedly produces the complete hidden-nonce-backed
Tool lifecycle and protocol evidence explains the difference without an RAH
defect.

## Security and non-goals

No authority or production behavior changed. This research preserved:

- RAH as an orchestration layer, not an inference engine;
- model request as non-authoritative;
- `Tool`/`ToolRegistry` as the execution boundary;
- host permission and Trusted Profile authority composition;
- disabled Codex-owned shell, process, filesystem, Git, MCP, apps, browser,
  computer, image, and approval paths;
- no network MCP, plugin installation/download, hot reload, OS-sandbox claim,
  rollback claim, or network-isolation claim;
- no replay of uncertain external effects.

Task 208 was not started.

## Validation

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS.
- Required exact `cargo test --workspace`: FAILED on two runs with different
  unrelated Windows fixture/timing failures; the first had 146/148 `rah-tools`
  tests passing and the second had 152/157 `rah-desktop` tests passing.
- `cargo test --workspace -- --test-threads=1`: FAILED once on one unrelated
  race-oriented `rah-tools` test (`stale_target_between_validation_phases`),
  with 147/148 `rah-tools` tests passing; its exact isolated rerun passed.
- The final isolated reruns of all observed failing tests passed. No failure
  implicated the Codex adapter, hardened live fixtures, or this documentation.
