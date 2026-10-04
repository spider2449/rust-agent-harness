# Task 509D — Codex effective-provider model-discovery identity

Reference-only follow-up: [Task 510A](2026-10-04-task-510a-newer-codex-provider-bound-discovery-protocol-audit.md)
audits the exact installed 0.160.0 artifact and retains the baseline limitation.
Task 509D classification/evidence and the certified 0.157.1 baseline remain
unchanged. Task 510A owns subsequent research documentation publication;
no Task 509B/509C picker implementation resumes.

Starting HEAD: `570e93319382343b1fa3d7996c413736d9e13770`.
Certified runtime stays `codex-cli 0.157.1`; RAH stays 0.33.0.
Existing Task 509/509A modified documentation and Task 509B/509C untracked
documentation are preserved. No Task 509C implementation commit exists.

## Plan and disposition

1. Inspect current adapter, accepted ADRs 0030/0032/0033, architecture/security
   documents, and Task 499/503/509B/509C evidence.
2. Generate the actual certified binary's experimental JSON schemas. Audit
   model/list, initialization, configuration reads, and thread-start metadata.
3. Probe isolated, credential-free configurations without thread or turn creation.
   Distinguish observed config from proven catalog source.
4. Implement provider-bound evidence only if P1/P2/P3 establishes ownership.
   Otherwise stop with research classification C and no provider-bound evidence.
5. Run adapter/preflight/neutral deterministic regressions; preserve evidence.
   Product-source changes would require the complete requested validation sequence.

**C — MODEL/LIST CANNOT BE PROVIDER-BOUND ON CODEX 0.157.1**

This is research evidence, not an implemented discovery contract or a production
behavior correction. No adapter-internal effective-provider type or catalog
evidence API is introduced: doing so with a populated catalog would claim
ownership that this audit cannot prove. Task 509C must not consume the existing
raw catalogs as provider-bound evidence. Existing Task 499 preflight and neutral
discovery behavior remain unchanged and have the ownership limitation below.
No picker work resumes. Classification C does not authorize commit/push.

## Admitted artifact and evidence provenance

The baseline helper `scripts/codex-baseline.ps1 path 0.157.1` verified and resolved
`C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe`.
Its separately recomputed SHA-256 is
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`.
The manifest also identifies companion `codex-code-mode-host.exe` with SHA-256
`5583872360541856fe6207eba0fb8486d9967083fbf4d40e2126ec9bb9c06ead`.
Version/schema admission remains ADR 0030's exact-version policy. Actual
`ProcessTransport` admission resolves a canonical executable and checks version
and required schema; it does not return an artifact fingerprint and does not
hash the executable as part of process startup. Baseline manifest verification
and runtime admission are distinct; this report does not invent an existing
adapter fingerprint API or claim version alone is artifact identity.

Generated schemas, probe script and bounded results are retained under
`F:/temp/rah-task509d-protocol`. Schema command:

```powershell
& 'C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe' app-server generate-json-schema --experimental --out 'F:/temp/rah-task509d-protocol'
```

The installed artifact's generated schema is primary protocol evidence. An
attempt to retrieve the exact upstream `rust-v0.157.1` message-processor source
returned 404; no newer source or CLI is substituted. No conclusion about
undocumented internal implementation follows from that unavailable source.

## Exact protocol audit

Repository discovery sends the JSON-RPC request body:

```json
{"id":1,"method":"model/list","params":{"limit":100,"includeHidden":true}}
```

The ID is a connection-assigned integer; 1 is illustrative. `ModelListParams` has optional `cursor`, `includeHidden`,
and `limit`. No provider, config, thread, endpoint or source identifier exists.
`ModelListResponse` has `data` (required) and `nextCursor` (optional/null).
Each Model has selector `model`, separate `id`, display/description, default and
hidden flags, reasoning, modality, personality, service-tier, upgrade, specialty,
access-program and multi-agent metadata. Neither top-level nor Model properties
identify the effective provider, endpoint, configuration revision or thread.
The repository requires an explicit null nextCursor, bounds to 100 entries and
256-byte selectors, deduplicates selectors, and discards display metadata.
These completeness/bounds rules do not establish source ownership.

Repository initialization sends `initialize` with `clientInfo` name/version and
`capabilities.experimentalApi` plus `requestAttestation:false`, then notification
`initialized {}`. `InitializeResponse` requires `userAgent`, `codexHome`,
`platformFamily`, `platformOs`; it has no effective provider or catalog identity.

Configuration read exists: `config/read` with optional `cwd` and `includeLayers`.
The audited request was `{"includeLayers":false}`. Response requires `config`
and `origins`, optionally `layers`. Config includes nullable string
`model_provider`, alongside nullable model and other settings. A config read
can resolve project layers for its requested cwd; it supplies no catalog source
reference or proof that model/list uses those layers. The repository currently
does not use this operation. Config value/batch write operations also exist;
mutating external user configuration is unnecessary and was not performed.

`thread/start` accepts optional `model`, `modelProvider`, `config`, `cwd`,
`ephemeral`, and other controls. Its response requires `thread`, `cwd`, `model`,
`modelProvider`, `approvalPolicy`, `approvalsReviewer`, `sandbox`. The adapter
already verifies returned model/provider for explicit selection and verifies
host-selected cwd. Its restricted params disable owned shell/web/image/apps/MCP
features and apply OpenAI/Ollama/LM Studio provider IDs or bounded custom
provider definitions. This is authoritative thread metadata, not catalog
metadata. No thread ID/config reference is accepted by model/list. The presence
of ephemeral does not prove initialization has no other effects; no thread was
created as an ownership probe.

## P1/P2/P3 and observability

| Mechanism | Actual evidence | Result |
| --- | --- | --- |
| P1: provider on model/list | Generated request schema has only pagination/hidden fields | Unsupported |
| P2: isolated startup config | app-server supports `-c key=value`; config/read observes explicit startup provider | Configuration observable, catalog ownership unproven |
| P3: provider-bound thread | thread/start returns required modelProvider; model/list has no thread/config association | Cannot tie catalog to thread; no probe thread created |

Three serial probes used separate newly created empty CODEX_HOME directories,
temporary cwd, filtered credential-bearing environment variable names, fixed
certified executable/argv and bounded RPC deadlines. Requested providers were
Inherit, explicit `openai`, and explicit `ollama`. No user auth/config was read
from the normal Codex home and no credentials were supplied. Only initialize,
config/read and model/list were requested. Output retains schema keys, provider
names, model counts and selector hashes; raw config/auth is not published.

| Startup selection | config/read model_provider | model/list count | Source identity |
| --- | --- | --- | --- |
| Inherit, empty isolated home | null | 11 | Absent |
| `-c model_provider="openai"` | openai | 11 | Absent |
| `-c model_provider="ollama"` | ollama | 11 | Absent |

All three ordered selector arrays have SHA-256
`d3725fee286c2980699c760fda7fd0b0fe609aa7fbad7d399a67ce8b7bbe8c40`.
Thus a successful model/list plus an observed config name is insufficient to
prove provider-specific ownership. Identical arrays are not, alone, proof of
provider-independent semantics either: that durable rule was not established.
Explicit OpenAI receives no special assumption. Non-OpenAI remains unsupported
for provider-bound discovery. Inherit cannot be resolved by interpreting null as
OpenAI; explicit inherited config names would still lack a catalog association.
No supported provider-bound ownership case was established.

## Contract, caches, contamination and side effects

The safe Task 509C source state for every audited configuration is discovery
unavailable/unbound, with no provider-bound catalog. It must not be represented
by empty models, the default catalog, or cached models. This report establishes
that policy for the blocked work; it does not claim the existing adapter already
enforces it. Existing legacy `ModelDiscovery::Catalog`/`ModelPreflight` results
remain unbound observations and are not the requested evidence contract.

If future protocol evidence permits binding, an adapter-owned result needs actual
admitted artifact identity plus authoritative effective upstream identity and
bounded models. Requested `CodexModelProvider` is not observed identity; native
RAH OpenAI adapter identity is a different concept. No neutral runtime API change
is justified here. Missing identity must produce explicit Unsupported/unbound,
even when model/list returned models. No model-string special case is warranted.

The adapter currently performs each raw discovery anew; no adapter catalog cache
was found. CLI-internal caching is not proven by schemas/probes. A future owned
catalog cache must key by actual admitted artifact and effective provider
configuration, including applicable endpoint/context, and observe inherited
changes per discovery or keep Inherit unsupported. Version-only/model-only keys
are insufficient. No indefinitely cached inherited evidence is authorized.

One adapter process can receive different model/provider thread parameters:
`restricted_thread_params` is per thread, not a process-level provider lock.
Existing deterministic provider-param/mismatch tests exercise this code path;
the audit does not certify real multi-provider thread execution. Startup probes
use separate processes and cannot establish a stronger per-process ownership
rule. Since model/list has no thread association, thread A's provider cannot
label a catalog for thread B. Cross-provider probe equality is direct evidence
against treating local requested-provider equality as source proof. No catalog
evidence is constructed or reused for either provider/artifact. Two admitted
artifact fixtures and an implemented contamination guard were not introduced;
there is no bound-evidence path to certify.

Probe side effects: temporary homes and generated schema/log/result files,
three short-lived supervised app-server processes. Zero thread/start, turn/start,
Tool requests/executions, user-visible conversations or inference. Child
processes were terminated in finally cleanup. Empty homes do not prove network
isolation; no such guarantee is claimed. No native OpenAI API client/credentials,
runtime refresh, external config mutation or user-home edits were used.

## Validation

All executed commands exited 0. Target directory was
`F:/temp/rah-task509a2-target-run2`; logs are under the protocol evidence directory.

| Executed check | Result |
| --- | --- |
| `cargo test -p rah-runtime-codex --lib -- --nocapture` | 102 passed, 0 failed, 1 ignored |
| Catalog tests included above | 5 passed, 0 failed |
| Task 503 experimental adapter tests included above | 6 passed, 0 failed |
| Explicit builtin/custom provider params and effective-model/provider verification included above | PASS |
| `cargo test -p rah-desktop --bin rah-desktop model_preflight` | 4 passed, 0 failed, 0 ignored |
| `cargo test -p rah-desktop --bin rah-desktop host_invocation::tests::host_allowlist_is_exact` | 1 passed, 0 failed, 0 ignored |
| `node F:/temp/rah-task509d-protocol/audit-assertions.cjs` | Six schema/probe assertions passed |
| `git diff --check` | PASS |

Existing deterministic tests validate legacy catalog/parser/preflight, explicit
provider response verification and Task 503 neutral adaptation; they do not
certify a new ownership contract. The six external assertions verify actual
schema/probe observations, including provider-free catalog shapes and different
observed config providers producing identical catalog selectors. They are not
an implemented no-fallback publication guard. New supported ownership and
artifact/provider contamination contract tests are not claimed because no
product contract was implemented. Task 499's advertised/absent-model gating and
zero-inference rejection regressions passed unchanged; provider ownership is
an additional unresolved limitation, not newly certified by those tests.

Full workspace fmt/check/test/Clippy, canonical Desktop, complete frontend/static,
Tauri inventory and metadata were not run: no product source changed and the
ownership prerequisite stopped implementation. No workspace or canonical Desktop
counts are claimed. No deterministic failure or linker corruption occurred.
Closure inspected status, diff stat and diff check; HEAD remains the starting SHA.

## Authority, ADR and next prerequisite

HostExplicit remains statically exactly 11 in unchanged HostInvocationKind.
Executable exact-allowlist regression passed (1/0/0); this is not live authority
certification.
No ToolRegistry, repository authority/switching, lease, Trusted Profile,
permission, mutation uncertainty, remembered workspace, dependency, neutral API
or frontend change. Typed/sanitized runtime error handling remains unchanged;
no raw provider config/auth/error is added to serialized product state.

ADR-B: refinement of observation requirements under ADR 0030/0033, no accepted
ADR edited and no new durable claim that default catalogs are provider-independent.
No new ADR is warranted by unsupported observation.

Exact Task 509C recommendation: **do not resume snapshot/picker implementation
using 0.157.1 catalog evidence**. Evaluate moving Task 510's pinned protocol and
artifact audit earlier as a prerequisite; require a documented authoritative
catalog/provider association before implementing bound discovery. A refresh
must still pass ADR 0030's separate certification and is not authorized or begun
by this report. Alternatively a separately scoped product correction may expose
explicit unbound/Unsupported discovery without claiming a provider catalog;
its Task 499 admission limitation must be deliberate and fully validated.
Task 508L remains paused. No commit, push, CI, tag, release or version bump.
Worktree intentionally retains documentation WIP.
