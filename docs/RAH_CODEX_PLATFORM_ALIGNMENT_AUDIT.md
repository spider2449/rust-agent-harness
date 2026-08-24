# RAH Codex Platform Alignment Audit

Date: 2026-08-24
Task: 075 — research/design/alignment only
RAH baseline: Task 074 `e7f84a43088d1a26e18ec3e2de8ed8af39c6cf07`; CI `32695735996` successful
Certified Codex live baseline: native `codex-cli 0.149.0`

## Executive decision

**RECOMMENDED: keep Codex app-server as RAH's primary Codex runtime boundary.**
The new platform framing confirms, rather than contradicts, ADRs 0001, 0002,
0003, 0004, 0005, 0006, 0007, and 0008. It assigns the harness the agent loop,
thread state, streaming, tool interaction, sandbox/approval execution policy,
and cross-turn work; it assigns the application its interface, context, tools,
operational boundaries, approvals, observation, and system-of-record updates.
That division maps directly to RAH's adapter and host-authority architecture.

No production Rust or ADR change is justified by this audit. Two release-tooling
items are justified before v0.7 release preparation: immutable reusable baseline
management and explicit host-selected certified-binary use for live gates. These
are not a change to RAH's runtime or authority architecture.

### Evidence classes

* **Official documented behavior** is limited to the cited OpenAI pages.
* **Observed RAH behavior** comes from the Task 074 baseline and repository code
  inspected for this audit.
* **Recommendations** are RAH design conclusions; they do not claim extra Codex
  security guarantees.

## Official platform evidence

OpenAI describes the reusable Codex component as the harness/agent loop, which
manages conversation state, streamed execution, tools, configured sandbox and
approval policies, and work across turns. It says app-server exposes documented
thread, turn, event, tool, interruption, and approval handling to a product
client. It also says the host chooses the product interface, context and tools,
where work runs, allowed files/tools, approval requirements, observation, and
system-of-record return path. [Codex as a platform](https://developers.openai.com/blog/codex-as-a-platform)

The same article distinguishes integration layers: `codex exec` for bounded
scripts/CI/one-off work; SDKs for programmatic start/resume/stream workflows;
and app-server when the agent is part of the product and direct lifecycle/UI
control is needed. It uses Relay to illustrate application-owned MCP data/actions
with Codex supplying the agent loop. This is an integration pattern, not a claim
that MCP or sandboxing independently authorizes host operations.

## Runtime-boundary decision

| Option | Assessment for RAH | Decision |
| --- | --- | --- |
| Direct app-server adapter | Native process, documented JSON-RPC, direct thread/turn/event/interrupt/tool/approval lifecycle control; RAH can pin and schema-check the executable. | **Keep primary.** |
| Official Codex SDK | Higher-level TypeScript/Python convenience surface over local Codex workflows; useful if a future adapter needs its ergonomics, but it adds Node/Python coupling and hides protocol detail RAH intentionally controls. | Optional future adapter research only. |
| `codex exec` | Well suited to scripts, CI, and bounded background jobs. It has less product-lifecycle and interactive-control fit. | Not the RAH runtime. |
| Codex internal/core libraries | Tightly coupled implementation surface and incompatible with RAH's adapter isolation/versioned binary policy. | Do not depend on them. |

The SDK is a **higher-level convenience wrapper**, not a reason to replace the
RAH runtime. Current official documentation says its TypeScript library starts,
continues, and resumes local threads and requires Node 18+, while its Python
library controls local app-server JSON-RPC and normally uses a pinned runtime.
Its explicit `codex_bin` configuration demonstrates that binary selection remains
a useful operational concern. [Codex SDK](https://developers.openai.com/codex/sdk/)

The current app-server specification exposes connection initialization,
`thread/start`, `thread/resume`, `thread/fork`, `thread/read`, `thread/archive`,
turn streaming, `turn/interrupt`, dynamic tools, and server-initiated approvals.
That is the lifecycle and raw-protocol access RAH needs for a Generic Tool Bridge
without changing `AgentRuntime`. [Codex App Server](https://developers.openai.com/codex/app-server/)

### Interface matrix

| Capability | Direct app-server | SDK | `codex exec` | Internal libraries |
| --- | --- | --- | --- | --- |
| Process ownership | RAH owns native child | SDK owns/abstracts local runtime | CLI invocation owns one bounded run | Compile-time coupled |
| Thread create/resume | Direct | Documented convenience | Not primary product API | Possible but unstable |
| Fork/archive/read | Exposed protocol features | Not established as portable SDK surface | Not suitable | Implementation-specific |
| Stream/events and cancellation | Raw events and `turn/interrupt` | Convenience streaming where supported | Bounded command output | Internal |
| Tool/approval exposure | Raw dynamic tool and approval requests | Higher-level, language-specific | Limited fit | Internal |
| Schema/lifecycle control | Full, version-pinnable | Reduced/indirect | Low | Unstable implementation |
| Compatibility burden | Adapter owns it explicitly | SDK + runtime coupling | CLI-output compatibility | Highest |
| Language/runtime coupling | Rust subprocess/JSON-RPC only | Node 18+ or Python 3.10+ | CLI only | Rust internals |
| Windows/native pin | Direct native `.exe` selection | SDK policy-dependent | Possible but not RAH lifecycle | Build/package coupled |
| Generic Tool Bridge / RAH separation | Strongest fit | Requires a second translation boundary | Insufficient lifecycle control | Violates isolation intent |

## ADR and no-inference-engine conclusion

`AgentRuntime -> CodexRuntime -> native app-server` is **confirmed** by the
platform framing. ADR 0001 remains RAH-owned runtime abstraction; ADR 0002 keeps
Codex optional/adapter-local; ADR 0005 chooses the documented product-facing
interface. No clarification or change is required now.

ADR 0004 is also **confirmed**. Codex positions its reusable value as the harness
and agent loop, not an invitation for RAH to build inference, weights, kernels,
tokenizers, or a competing generic reasoning loop. RAH should continue to use
external runtimes/backends and keep its distinct value in neutral protocol,
host-authorized capabilities, composition, and integration.

## Tools, MCP, plugins, and authority

The current model remains aligned:

```text
Host application / RAH
  -> Codex app-server
  -> Generic Tool Bridge
  -> RAH ToolRegistry
  -> built-ins / repo.patch / repository observers / MCP providers / process-plugin providers
```

MCP and process plugins remain **Tool providers, not runtimes**. Application-owned
MCP in the Relay example reinforces that an application supplies its own data and
actions while Codex runs the agent loop. Therefore RAH should expose capabilities
to Codex exclusively through the Generic Tool Bridge for tool-bearing Codex
threads, with a fresh host-composed `ToolRegistry`. There is no official evidence
that justifies letting Codex independently configure unrestricted MCP providers.

ADRs 0003, 0007, and 0008 are **confirmed**. Canonical RAH `ToolName`, definition,
permission, provider identity, process lifecycle, and policy remain host-owned;
remote metadata, model output, aliases, and approval prompts cannot elevate them.
Task 074's live `RAH_MULTI_PATCH_LIVE_OK` path is consistent with this rule.

### Approval and authority layering

Codex policy is the harness/runtime's execution policy. RAH authority is the
host/business authorization boundary expressed through `PermissionLevel`,
`ExternalToolPermissionPolicy`, `HostExecutionPolicy`,
`RepositoryMutationPolicy`, `WorkspacePolicy`, `TrustedStaticProfile`, and trusted
capability composition. Both may deny work, but neither substitutes for the other.

App-server can ask a client to approve command execution, file changes, requested
permissions, or app/MCP interactions. Such a request is a runtime/user-consent
signal; it cannot grant a RAH capability. In particular, if `repo.patch` is absent
from the trusted profile, a Codex approval cannot create it. Model request is not
authorization; approval request is not authority grant.

Recommended future documented invariant:

```text
Effective action allowed =
  RAH authority permits capability
  AND Codex/runtime policy permits execution
  AND required user approval is satisfied.
```

Any later approval mapping must map only a pre-authorized RAH capability and fail
closed. It must never implement `Codex approval => RAH authority`.

Codex-owned shell, filesystem writes, MCP, process, web/network, apps, and similar
native capabilities should remain disabled by default. The current restricted
adapter sets approval policy to never and read-only sandbox settings, with native
tools disabled, while the explicit bridge dispatches only RAH tools. The article
does not recommend enabling any of these by default.

## Threads, sessions, and persistence

**Recommendation: RAH Session wraps/references a runtime-specific Codex thread;
they must not be identical.** A Codex thread is harness conversation/execution
state. A RAH Session also represents host authority context, workspace/repository
identity, tool inventory identity, workflow/audit/mutation state, and external
provider state.

App-server supports stored thread state and history, including read, list, fork,
and archive; its documentation specifies persisted JSONL thread logs for archive.
It can therefore own Codex conversation/context, turn/event history, and resume
identity. RAH must persist (when persistence is later designed) selected workspace
and repository, trusted-profile identity/version/digest, recomputable authority
decisions, provider/tool inventory identities, workflow checkpoint, mutation/audit
outcome, and the adapter/thread association.

Required future resume rule:

```text
Codex thread identity may persist.
RAH authority must be recomputed from current trusted host state on resume.
```

Do not serialize live `ToolRegistry`, external provider process handles, permission
policy objects, dedupe cache, or model-facing aliases. Persist trusted identities
and configuration references only; recompose a fresh registry, regenerate private
aliases, revalidate provider configuration, and reconnect processes. This prevents
an old thread from silently restoring stale authority.

## Bridge, sandbox, and protocol conclusions

ADR 0006 remains **confirmed**: the Generic Tool Bridge stays provider/tool
agnostic. Its private deterministic aliases, canonical-name preservation,
registry dispatch, permission gating, dedupe, cancellation/disconnect handling,
response translation, and no-replay semantics require no SDK-specific semantics.

Codex sandboxing and RAH host execution/repository authority are complementary,
not interchangeable. Documentation should continue to say exactly what RAH
enforces. Codex sandbox availability does not prove OS sandboxing for external
providers, rollback, repository transactional isolation, or network isolation
unless independently configured and demonstrated.

Manual JSON-RPC handling is intentionally retained control, not undifferentiated
technical debt: initialize/initialized, native child ownership, correlation,
thread/turn lifecycle, events, interruption, schema validation, and shutdown are
appropriate direct-client responsibilities. SDK delegation could be evaluated only
as an optional future adapter. The audited adapter already has explicit native
executable input (`CodexRuntime::connect(executable)` and
`connect_tool_bridge(executable, ...)`), Windows resolution to a canonical native
`.exe`, exact version verification, and captured-contract validation. Classification:
**A — already available and sufficient for live examples**, but not yet a dedicated
host configuration surface for release gates.

App-server can generate version-exact TypeScript or JSON Schema artifacts via
`codex app-server generate-ts` and `codex app-server generate-json-schema`.
Future release tooling should add an **app-server protocol compatibility audit**
that diffs certified and candidate schemas for removed methods, required-field and
event-shape changes, tool-schema constraints, and reviewed additive fields. This
is release tooling research, not Task 075 implementation.

## Certified baseline policy and Task 076 recommendation

Adopt a three-tier policy:

1. **Certified release baseline:** one exact native binary/version plus hash
   (currently `0.149.0`), used directly for release and regression evidence.
2. **Development version:** globally installed/latest Codex for daily work.
3. **Compatibility window:** newer versions become eligible only after explicit
   schema, deterministic, live-adapter, bridge/tool-live, and caveat review.

A daily `0.150.x` or newer binary must never silently become release evidence.
Promotion is: candidate -> schema diff -> deterministic adapter tests -> live
adapter gate -> bridge/tool live gates -> platform-caveat review -> certified
baseline update -> release-document update.

**Implement reusable baseline management before v0.7 release preparation.** It is
a release-tooling blocker for reproducible cross-machine evidence, not an
architecture blocker for v0.7 itself. Task 076 should provide
`scripts/codex-baseline.ps1` with `save`, `verify`, `path`, `list`, and
`verify-all`, storing e.g. `%LOCALAPPDATA%\\codex-baselines\\0.149.0\\codex.exe`
and a manifest with version, SHA-256, platform/target, binary filename, known
source/origin, and archive timestamp.

Rules: accept only a native `.exe`, verify `--version`, hash before/after copy,
never silently replace an existing baseline, and fail closed if the same version
has a different hash. The store is per-user/no-admin, has no repository-specific
absolute path, and the model never chooses the binary. Explicit host selection
must take precedence over PATH discovery.

For Task 076, prefer a typed host configuration field over `RAH_CODEX_BIN`; an
environment variable may be a narrowly documented CI/launcher input to that field
only. The field is clearer in authority ownership and test construction. If an
override exists, require an absolute canonical native Windows `.exe`, reject a
missing/non-native path, apply defined reparse-point policy, verify version and
hash/manifest, and use PATH only when no override is configured. Host/CI, never
model output, controls inherited environment and selection.

For fresh-machine reproducibility on Windows x64, prefer isolated acquisition of
the exact `@openai/codex@<version>` package (or verified official exact release
artifact when available), resolve its platform package/native binary, verify it,
then archive the native executable and manifest. Copying an already global
installation is a useful `save` source but is insufficient as the only bootstrap
path. Do not commit binaries to RAH Git: it harms repository size, provenance,
platform coverage, and update security. Windows x64 is first scope; ARM64 must
acquire/validate its own matching artifact and cannot assume x64 portability.
Future Unix support needs separate target/path and executable-permission rules.

Intended workflow:

```powershell
codex --version # latest daily development tool
.\scripts\codex-baseline.ps1 verify 0.149.0
$baseline = .\scripts\codex-baseline.ps1 path 0.149.0
& $baseline --version # codex-cli 0.149.0
```

Release gates must pass the certified native path directly, never rely on global
PATH.

## Architecture delta and decision register

| Area | Current RAH | Official platform evidence | Alignment | Change needed |
| --- | --- | --- | --- | --- |
| Agent loop | Optional Codex runtime; RAH does not recreate it | Codex harness is reusable loop | Aligned | KEEP |
| Runtime boundary | Native app-server adapter | App-server is product-embedding/control layer | Aligned | KEEP |
| Thread state | Private RAH-session/Codex-thread mapping | Harness owns conversation across turns | Aligned | DOCUMENT resume rule |
| Tools | RAH registry and Generic Tool Bridge | App supplies tools/actions | Aligned | KEEP |
| MCP | RAH MCP provider; Codex MCP disabled | App-owned MCP pattern | Aligned | KEEP |
| Approval | Codex approvals denied; RAH owns authority | App-server exposes consent requests | Aligned | DOCUMENT composition invariant |
| Sandbox | RAH policies plus restricted Codex config | Harness has configured sandbox policy | Aligned | DOCUMENT non-equivalence |
| Authority | Trusted composition/policies are host-owned | Host owns operational boundaries | Aligned | KEEP |
| Persistence | Deferred host session persistence | Codex retains thread history | Aligned | RESEARCH LATER |
| Protocol | Pinned JSON-RPC/schema contract | Version-specific schema generation | Aligned | IMPLEMENT BEFORE v0.7 RELEASE: audit tooling |
| Version management | Exact binary version but no reusable manager | No contrary evidence | Operational gap | IMPLEMENT BEFORE v0.7 RELEASE |

| Finding | Classification |
| --- | --- |
| Keep app-server primary; do not switch to SDK/internal libraries | KEEP |
| Preserve `AgentRuntime`, no inference engine, ToolRegistry/MCP/plugin model | KEEP |
| Approval/authority and resume-security invariant | DOCUMENT |
| SDK as optional future convenience adapter | RESEARCH LATER |
| Session/workflow persistence design | DEFER POST-v0.7 |
| Baseline manager, explicit host binary selection, and schema compatibility audit | IMPLEMENT BEFORE v0.7 RELEASE |

## Release conclusions

**Does the platform article block RAH v0.7 release? No.** It confirms the current
architecture and exposes no authority or security conflict.

**Must reusable Codex baseline management be implemented before v0.7 release
preparation? Yes.** Cross-machine, immutable, host-selected release evidence is
otherwise not reproducible enough. Treat this as release tooling, with no change
to RAH authority architecture.

**Recommended next task:** Task 076 — Reusable Codex Baseline Manager and Explicit
Certified Binary Selection. Scope: PowerShell baseline manager, isolated exact
version acquisition/archive, immutable SHA-256 manifest, `save/verify/path/list`,
explicit host-selected native executable for live gates, daily global Codex free to
upgrade, practical deterministic script tests, and no model authority or
app-server architecture change. After Task 076: rerun certified live smoke,
milestone audit, then release preparation.
