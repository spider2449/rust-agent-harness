# ADR 0034 — Artifact-bound deterministic Codex compatibility admission

Status: Accepted (design decision; production implementation pending Task 510C2)

Date: 2026-10-06

## Context and disposition

ADR 0030 decisions 1, 3, 4 and 6 made exact-version certification the current
admission authority. This was a conservative response to observed provider and
protocol drift. Historical release certification remains immutable. Task 510B
subsequently certified an exact 0.160.0 distribution without admitting it;
Task 510C correctly stopped because its field-name probe was insufficient.

This ADR supersedes the **admission/version-pinning portions** of ADR 0030
decisions 1, 3, 4 and 6, ADR 0005 decision 1 and its version-check consequence,
and ADR 0006 decision 7 and its executable/schema-pin consequences. Their
process boundary, experimental opt-in, execution restrictions, historical
certification and authority rules remain in force. ADRs 0032/0033 remain in force.
The old implementation stays fail-closed until C2 satisfies this decision.
Acceptance of this design is not admission of another version or artifact.

## Decision

Codex runtime compatibility shall be established by an artifact-bound,
deterministic local `CodexCompatibilityContractV1`. Reported version is evidence
and diagnostics, not the primary admission authority. Every required capability
for the host-selected feature profile must pass. Unknown/new versions that pass
are compatible without source-code version allowlist edits; unknown/new versions
that fail are incompatible. Missing evidence never becomes a pass.

Schema checks, actual-executable local dispatch/behavior probes, and adapter
fixture tests are complementary. A fake app-server proves adapter behavior;
it cannot by itself admit a real executable. Model prompting, account state,
quota and remote provider success are not compatibility evidence.

The normative [V1 specification](../CODEX_COMPATIBILITY_CONTRACT_V1.md) defines
the seven layers, typed surface, local behavioral witness and distribution
discovery. The [C1 report/C2 matrix](../plans/2026-10-06-task-510c1-deterministic-compatibility-contract.md)
defines implementation closure. These are part of this decision.

## Admission and evidence ownership

The Codex adapter/runtime factory owns discovery, private wire translation,
measurement and assessment. Evaluate before returning a normal runtime for
conversation use; a disposable probe thread is not a user conversation.
Apply the same gate to host override, installed candidate and fallback paths.
Desktop presents sanitized status; it does not own admission. Neutral
composition consumes closed RAH-owned assessment/selection facts.

Conceptual internal record (names are design vocabulary, not new public APIs):

```text
RuntimeCompatibilityEvidence {
    contract_version, feature_profile, probe_revision,
    reported_version: optional bounded string,
    artifact_identity: DistributionIdentity,
    required_components: [ComponentEvidence],
    schema_result, dispatch_result, lifecycle_result,
    tool_result, diagnostic_result, catalog_result,
    overall: Compatible | Incompatible | Indeterminate
}
```

Each capability has requirement class, Pass/Fail/NotObtained/NotApplicable,
typed local reason, witness kind, identity binding and bounded evidence digest.
NotApplicable is allowed only for an inactive conditional or optional capability.
Unknown enum values, unavailable required schema and contradictory witnesses
cannot pass. A definitive required failure yields Incompatible even if other
checks are unobtainable. Otherwise incomplete required evidence yields
Indeterminate; only all required Pass yields Compatible.

Public projections contain closed enums, identifiers, hashes and sanitized
diagnostics only: no `serde_json::Value`, Codex DTOs, app-server transport types,
raw stderr, paths containing sensitive data, provider bodies or source errors.
Preserve Task 498/ADR 0032 `RuntimeFailure` with an adapter-private typed source
and sanitized `RuntimeDiagnostic`. Reuse current operation/kind mappings where
accurate; C2 must review any necessary neutral diagnostic addition explicitly.

Compatibility, provider availability, authentication, quota and selected-model
advertisement/availability are separate assessments. A remote outage is not
Incompatible and does not invalidate a compatible artifact. If local required
evidence cannot be obtained, result is Indeterminate, never optimistic admission.

## Identity, cache and known-good artifacts

Distribution identity includes main executable and only required companions or
proven required resources: role, canonical resolution identity, relative layout,
SHA-256 and byte length. Bind the resolution metadata digest and relevant
configuration/profile affecting capability behavior. Exclude unrelated npm
files and ordinary platform OS components; include a bundled non-system library
if needed by a required path. Version alone is never an identity or cache key.

Cache key: distribution identity + resolution evidence digest + contract version
+ feature profile + probe revision + relevant platform/configuration fingerprint.
Before reuse, rediscover the required set and verify concrete bytes/resolution.
Same version with different bytes, companion changes, missing files, changed
layout or changed contract invalidates the verdict. Prevent measure/spawn races
using retained identity/immutable snapshots or equivalent checked ownership;
stat/mtime alone is insufficient. This is not a claim of a general OS sandbox.

Initially allow only process-local positive caching. Do not persist a verdict
from an unsigned editable cache or cache an Indeterminate result as Compatible.
Persistent cache trust requires a separate design. Certification records for
0.157.1 and 0.160.0 are `KnownGoodArtifact` evidence and rollback controls, not
version grants. No certification-only fast path skips V1. After a concrete
distribution earns full V1 evidence, ordinary identity-bound caching may reuse
it just like any other compatible artifact.

The contract itself must be versioned. A future contract expansion requires
new evidence; V1 cannot prove V2. Additive unused upstream features do not
require a new RAH contract or source edit.

## Selection and fallback

Resolve a host-selected candidate deterministically; test it before conversation
creation. If Incompatible or Indeterminate, a host-configured exact certified
fallback may be selected only after its certified distribution identity AND
required V1 evidence are verified. Missing/changed fallback fails closed.
Do not download, repair or broaden discovery automatically. Report candidate
disposition and fallback selection visibly with sanitized facts.

C2 should make the certified 0.160.0 **distribution identity** the preferred
fallback after both its certification anchor and V1 controls pass. 0.157.1
remains explicit rollback evidence. This is a future implementation decision;
C1 does not change today's preferred 0.157.1 baseline or accepted versions.
The existing two-file store is sufficient for those controls, but a store
manifest's self-declared hashes are not a certification anchor.

No mid-conversation runtime switching, silent failover or turn replay is
permitted. A runtime/turn failure preserves Task 500 conversation identity and
mutation uncertainty. Provider/model/auth/quota failure alone does not select
another executable. A later explicit reconnect constructs a new runtime.

## Security and limits

Admission grants runtime execution compatibility only. It grants no repository,
filesystem, Tool, permission, Trusted Profile or model-selected capability
authority. Unknown compatible artifacts receive exactly the same restrictions:
host-owned repository context, revocable Tool port, ToolRegistry, permission
policy, leases/currentness and sandbox boundaries. HostExplicit remains exactly
11. Execute permission does not imply narrower mutation authority.

The probe uses a disposable directory and home, no user repository, credentials,
external model or production Tool registry. A scripted local peer is a test
endpoint, not a new model-controlled endpoint Tool or public provider mode.
Any probe Tool uses ordinary host-mediated dispatch with a bounded in-memory
response; it creates no production authority. Shell/file/MCP/web/app capabilities
stay disabled; unexpected requests are denied. Do not copy upstream cancellation
tests that enable shell execution. Process supervision is not OS isolation.

Compatibility cannot certify maliciousness, every dormant upstream code path,
account entitlement or remote service behavior. Actual-executable scripted local
witnesses are the strongest deterministic discriminator for cancellation and
Tools, not promises about live provider timing. C2 must demonstrate these on
Windows; if it cannot, retain exact production admission and report the gap.
Never downgrade required behavioral evidence to schema/fixture-only evidence.

## Transition

Task 510C1 is documentation/design only. C2 is authorized as the next bounded
implementation task, but must not start automatically during C1. C2 must pass
its matrix before activating replacement admission. It must not merely add
0.160.0 to an exact-version table. Version chasing ends when a discovered
0.161.x/0.170.x/later artifact passing V1 is usable without an admission source
edit. Concrete protocol or required-component drift still fails closed.
