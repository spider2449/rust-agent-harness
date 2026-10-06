# Task 510C — Compatibility admission audit and stop

## Checkpoint and plan

Starting HEAD and origin/master: `7e2a4619978ad72bd3f1b71f651e98d3bdba7bed`.
Worktree clean and index empty before this report. GitHub run `37459991677`
was read live: completed, success, head SHA equal to this checkpoint.

Intended sequence: audit version authority and ADRs; establish deterministic
local protocol and distribution evidence; implement adapter-owned admission and
construction-boundary fallback; validate deterministic controls, bounded Windows
controls and complete closure; publish only after classification A or B.

## Result

**C — EXACT-VERSION AUTHORITY CANNOT YET BE REMOVED SAFELY**

This is an audit-only stop, not an implementation or an assertion that a
deterministic replacement is impossible. Task 510C remains incomplete.

The exact implementation dependency is `ProcessTransport::start` ->
`verify_version` -> `start_verified` -> `verify_schema`. Removing the version
predicate would leave `collect_missing_fields` as the local protocol authority.
That function checks required/property names only. Existing deterministic test
`schema_contract_detects_missing_payload_fields` accepts a required-name array
without property definitions and an empty `dynamicTools` definition. It does
not check field types, referenced payloads, request method dispatch, catalog
shape, provider-active turn readiness, or distribution prerequisites. The
captured contract omits model catalog and turn-started notification schemas.

Consequently this existing probe is not sufficient to replace certification
as the admission trust boundary. A structurally incompatible runtime can meet
its name-only checks. No version check was removed, and no compatible verdict
was manufactured from this evidence. Required next implementation work is a
replacement local contract validator, distribution-requirement evidence and
typed assessment before construction. This report does not establish an
upstream inability to expose deterministic evidence.

## Version-use audit

| Class | Locations | Current role |
| --- | --- | --- |
| V1 | `rah-runtime-codex/src/lib.rs`: certified version table and membership function | Exact production compatibility authority |
| V1 | `rah-runtime-codex/src/process.rs`: `check_version`, `validate_captured_contract` | Reject unknown executable versions; bind captured schema to exact admission table |
| V1/V3 | `rah-desktop/src/codex_baseline.rs`: `resolve_with`, `supported_baseline_version`, `verify_baseline` | Preferred baseline selected before PATH; exact layout/version and manifest-bound two-file hashes |
| V2 | `rah-runtime-codex/src/errors.rs`: `VersionMismatch` and diagnostic mapping | Typed local version failure and sanitized public diagnostic |
| V3 | `scripts/codex-baseline.ps1` | Exact artifact storage, verification and non-destructive repair; not adapter admission |
| V4 | `runtime_tests.rs`, process tests, certification support/tests, schema fixture, plans and release records | Historical contract/certification and current policy assertions |

The catalog and model configuration modules do not use the certified version
table. Task 499 model preflight and Task 509 advertised catalog remain separate.

## ADR and target policy

**ADR-A required.** Accepted ADR 0030 decision 3 explicitly requires exact
certified versions and rejects unknown versions; decision 4 makes the preferred
baseline an exact member. ADR 0005 also describes version validation. No ADR
was amended or accepted by this audit. A replacement decision must accompany
actual implementation, rather than documenting unimplemented policy as current.

Target policy: version is evidence; compatibility is behavioral/protocol-based;
authority remains host-owned. This target is not implemented by this report.
Required evidence includes reported version, concrete executable/distribution
identity, relevant members, protocol/lifecycle coverage and a typed result.
Provider and model availability must remain separate from compatibility.

Proposed phases: resolve configured candidate; inspect local distribution and
protocol; initialize without granting Tools; record local compatibility;
perform separate catalog/model readiness; construct conversations only after
admission. On runtime/distribution incompatibility only, select an exact
verified certified fallback at construction, visibly recording selection.
Never replay an active conversation or turn through another runtime.

No compatibility cache exists in the audited startup path. Any future
process-local cache must bind executable and required-member identities;
same-version changed bytes must invalidate it. No new cache was introduced.

## Fallback audit and runtime dispositions

Existing baseline manifest version 2 already represents `codex.exe` and
`codex-code-mode-host.exe` with separate SHA-256 values. The script accepts
explicit source paths for both, verifies each payload and refuses differing
replacement of valid stored bytes. Therefore no inherent two-file storage gap
has been established. Desktop verification is currently hardwired to the
0.157.1 preference; it is not a candidate/fallback selector. Manifest hashes
are read from the manifest rather than a production certified-artifact anchor.
Certified fallback implementation must distinguish manifest integrity from
membership in exact certified evidence.

The default LOCALAPPDATA store contains 0.149.0 and 0.157.1 directories; no
0.160.0 directory was observed. Directory presence is not fresh hash
verification. No store was modified, package downloaded or F:/Temp bundle
referenced by production. 0.157.1 remains preferred and retained. Task 510B's
exact two-file 0.160.0 certification remains historical evidence, without
production admission or fallback materialization.

## Controls and closure

Unknown-compatible/incompatible production controls, provider-down,
model-unavailable, artifact-change invalidation, fallback/missing-fallback,
authority and cancellation controls: not implemented or executed in this audit.
No automated gate live Windows run was performed. No inference or Tool probe
was run. No claims of future-version compatibility are made.

Workspace fmt/check/test/Clippy, canonical Desktop, frontend/static, Tauri
47/47/47/47/47, metadata 14/0.33.0/edition 2024 and executable HostExplicit 11:
not rerun. Task 510B counts are not presented as Task 510C validation.
Cargo.lock, package/dependency declarations and production source unchanged.
No authority implementation changed: ToolRegistry, repository selection,
HostToolScope, leases, Trusted Profiles, permissions, mutation uncertainty and
remembered workspace rules retain their existing code. No new authority
certification is claimed.

Only this report was added. `git diff --check` is the document closure check.
No commit, push, tag, release or new CI: C does not authorize publication.
Final HEAD remains the starting checkpoint, with this report as untracked WIP.
The next work is completing the replacement deterministic admission boundary
within Task 510C, not another Codex-version certification campaign.

## Task 510C1 follow-up — 2026-10-06

The historical classification C and all unrun implementation controls above
remain accurate for Task 510C. [Task 510C1](2026-10-06-task-510c1-deterministic-compatibility-contract.md)
now defines the deterministic local contract and accepts
[ADR 0034](../adr/0034-artifact-bound-codex-compatibility-admission.md), explicitly
superseding ADR 0030's exact-version admission authority in design. Typed
schema, actual-executable scripted local lifecycle/Tool witnesses and bounded
distribution discovery are required; the existing field-name probe is not
admission authority. Classification A authorizes bounded Task 510C2
implementation next, without starting it automatically. Production admission
and baseline remain unchanged during C1. This dated extension does not rewrite
the earlier audit or claim its missing implementation tests passed.
