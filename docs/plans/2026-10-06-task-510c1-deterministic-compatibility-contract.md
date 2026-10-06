# Task 510C1 — Deterministic Codex compatibility contract and ADR replacement

## Checkpoint, scope and plan

Starting HEAD: `7e2a4619978ad72bd3f1b71f651e98d3bdba7bed`.
Starting branch: master. Exact-head CI independently read live: completed,
success, run 37459991677, head equal to the checkpoint. One untracked file existed:
`docs/plans/2026-10-06-task-510c-compatibility-admission.md`; preserved and
extended with a dated follow-up, without rewriting its historical stop.

Plan: audit authoritative ADRs and current local schema/distribution evidence;
define typed required capabilities and an actual-executable local witness;
accept replacement ADR and reconcile historical pinning references; specify C2
acceptance and Windows controls; validate docs, review/stage only authorized
documentation, commit/push to GitHub master and require exact-head CI.
Production source, admission, preferred baseline, accepted versions, package
versions, dependencies and authority implementation are outside C1.

## Result and decision

**A — DETERMINISTIC COMPATIBILITY CONTRACT DEFINED; ADR 0030 SUPERSEDED**

[ADR 0034 — Artifact-bound deterministic Codex compatibility admission](../adr/0034-artifact-bound-codex-compatibility-admission.md)
is Accepted as a design decision with C2 implementation pending. It explicitly
supersedes ADR 0030's exact-version admission portions and corresponding pins
in ADRs 0005/0006, while retaining process/Tool/security restrictions and immutable
historical certification. Production still admits exactly 0.157.1 today.

The [normative V1 specification](../CODEX_COMPATIBILITY_CONTRACT_V1.md) defines
typed schema, actual method dispatch, lifecycle, distribution, Tool, diagnostic
and catalog requirements. A version-unknown runtime that passes all active
required local checks is usable without an admission allowlist edit. Unknown
incompatible/indeterminate runtime fails closed.

The distinction from the old probe is material: parse structural schema and
references, verify emitted/consumed types and discriminants, then exercise the
actual executable with a scripted loopback Responses peer. Adapter emulators
alone do not establish runtime behavior. This provides deterministic cancellation
and Tool-continuation witnesses without prompting a model. Upstream 0.160.0
dynamic Tool tests demonstrate the scripted-provider technique; RAH Windows
execution and budgets remain C2 acceptance obligations, not C1 test claims.

## Audited evidence and boundaries

Read README, architecture/guardrails, security, changelog and ADRs 0005, 0006,
0030, 0032, 0033. Inspected current adapter process, protocol, connection,
runtime, bridge, experimental lifecycle, catalog and model selection source.
Read Task 510C's stopped name-only probe audit, Task 510B distribution/source
evidence, R5C readiness evidence and R5E final certification closure.

Local generated schemas expose model/list, turn/started, turn/interrupt,
DynamicToolSpec, DynamicToolCallParams/Response and method unions. Dynamic Tool
arguments are arbitrary JSON values on app-server wire, not a required string;
provider-side function arguments are JSON text and get parsed before routing.
The current status enum is completed/interrupted/failed/inProgress. Shutdown
is host-owned process lifecycle, not a discovered shutdown RPC.

The installed `codex-package.json` has layoutVersion/target/entrypoint and
resource/path directories but no complete component requirement list. Retained
source resolves code-mode host resource-first, then package/standalone/exe sibling.
Its exact missing-host warning and two-file Tool evidence prove that component's
active requirement; they do not imply every npm file belongs in identity.
Future requirements are discovered by bounded typed metadata/report plus actual
local required-path provenance; unresolvable discovery is Indeterminate.

No production code or public API is introduced. Wire schemas/DTOs/JSON remain
adapter-private. Neutral composition consumes closed sanitized facts. The
conceptual evidence record carries contract/profile/revision, reported version,
distribution identity/required-component evidence, seven capability results and
overall assessment. Required Pass only admits; definite Fail rejects;
NotObtained blocks admission. External provider/auth/quota/model readiness
remains separate. No compatibility record grants any authority.

SHA-256/length/role/layout/resolution identities bind evidence and positive
process-local caches; contract/profile/probe/platform/configuration changes
invalidate reuse. Same version/different bytes inherits nothing. KnownGoodArtifact
records do not bypass V1; exact anchors and full V1 evidence are required for
fallback. Future preferred fallback is certified 0.160.0 after C2 passes those
requirements; 0.157.1 remains rollback. No baseline switch occurs in C1.

Fallback is construction-only on incompatible/indeterminate candidate, before
user conversation creation. Verify certified anchor and required distribution;
missing/changed fallback fails closed. Never switch/replay an active conversation
or choose another runtime merely because the provider/model/auth/quota failed.
ToolRegistry, HostToolScope, currentness, leases, repo selection, Trusted Profiles,
permission/mutation boundaries and HostExplicit exactly 11 remain unchanged.

## Task 510C2 — Implement artifact-bound deterministic Codex compatibility gate

Authorized as the bounded next implementation task by classification A; not
started automatically. Implement adapter-owned V1 assessment before normal
runtime construction, structural schema validator, minimal distribution discovery,
actual-executable local peer and optional process-local positive cache. Integrate
neutral pre-conversation selection/fallback and sanitized diagnostics. Remove
exact-version authority only after the following matrix passes. No full-package
admission, provider-owned authority, live-model admission gate or mid-turn replay.

| Acceptance case | Deterministic evidence / expected result |
| --- | --- |
| 1. Certified 0.157.1 control | Exact known-good distribution + full V1 schema/dispatch/local lifecycle/Tools as enabled -> Compatible |
| 2. Certified 0.160.0 control | Exact main/required host anchors + actual local behavior, including active code-mode path where used -> Compatible; exe-only active-path fixture rejected |
| 3. Unknown compatible | Real process fixture reports unlisted future version with complete typed schema/dispatch/witnesses -> Compatible, no source version edit |
| 4. Unknown incompatible | Missing required method/event, changed field/result type or invalid terminal contract -> Incompatible |
| 5. Same version/different artifact | Modify main or companion bytes with version unchanged -> old verdict unusable, fresh assessment required |
| 6. Missing required companion | Proven active role absent -> Incompatible; unreadable role -> Indeterminate; unused optional file absent does not reject |
| 7. Provider unavailable/auth/quota | Compatible local gate followed by separate scripted production readiness failure -> separate provider diagnostic; no executable fallback |
| 8. Model unavailable | Well-formed catalog lacking selected model -> NotAdvertised, not Incompatible; no model-name admission pin |
| 9. Pre-conversation fallback | Candidate incompatible/indeterminate -> anchored and V1-verified fallback before creation; missing/mismatched fallback rejects; active-turn failure never switches/replays |
| 10. HostExplicit | Existing production inventory/regressions assert exactly 11; unknown compatible runtime has identical permission/Tool denial policy |
| Typed schema mutations | Name-only impostor, unresolved ref, changed scalar/array/union, added required request field, missing Tool event, malformed method union all fail/indeterminate as specified |
| JSON-RPC dispatch | Handshake sequencing, valid method/result correlation, typed invalid method/params, malformed or dual envelopes, foreign IDs and server ID echo |
| Cancellation | Hold local stream; matching started inProgress + peer receipt; interrupt ack and one interrupted terminal -> neutral Cancelled; wrong/stale IDs, early started, no terminal and late completion regressions |
| Tools | Actual candidate request/host dispatch/result exactly 1/1/1; exact alias/object argument preservation; provider continuation sentinel and completion; bad namespace/misroute/replay/cancelled Tool denied |
| Diagnostics | Typed local compatibility reason retained in RuntimeFailure; serialized RuntimeDiagnostic never leaks secret-bearing message/data/stderr/path; valid provider errors stay separate |
| Catalog/config | Selector uses model rather than id; empty/duplicate/bounded/paginated/invalid entries; effective cwd/model/provider reflection; Codex openai semantics unchanged |
| Discovery | Manifest absent/unknown/malformed, resource vs sibling precedence, changed role set, new generic required resource, path escape/reparse and ambient package fallback controls |
| Cache/TOCTOU | Contract/profile/revision/metadata/target changes invalidate; changed bytes between measure/probe/spawn fail closed; stat/mtime match cannot substitute for hash |
| Shutdown/abort | Main/owned companion/peer/tasks reaped under shared deadlines; pipe loss/abort/oversize yields no pass; no inherited user auth/environment |
| Conditional profiles | No-Tools versus Tools-enabled, resume enabled, image-output enabled; no reuse of weaker verdict when enabling capability |
| Budget | Cold <=30s plus <=5s cleanup; per RPC <=5s under shared budget; warm hash-bound target <2s measured on controls; fixed transcript/schema/member/thread/exchange limits |

### Windows validation and implementation gate order

1. Check in minimal schema/dispatch/lifecycle fixtures with provenance/digests
   for current compatible and future-compatible unknown versions and mutations.
   Establish structural validator and adapter emulator regressions first.
2. Before deleting version admission, run **real binary, local-only** Windows
   probes against exact certified 0.157.1 and 0.160.0 distributions. Verify
   anchors, resolution, Tool path, cancellation and cleanup; record timings.
   The generic local peer must not need per-CLI-version source branches.
3. Exercise one current installed candidate if available, with newly measured
   identities and the same local gate. It may match a control but must still
   prove installed resolution. No install/download required. Report unavailable
   separately. Stop on concrete drift/prerequisite failure and preserve evidence;
   do not perform a new Task 510B deep certification for every unknown version.
4. Only after required local proof passes, activate the production replacement
   and anchored construction-only fallback; run focused adapter/neutral/Desktop
   selection, diagnostics and authority tests, then required repository closure
   for the implementation boundary. Use isolated targets where fixtures need them.
   No paid/live inference is required by this C2 matrix; any additional external
   validation requires its own explicit authorization.

If actual executable Tool/cancellation witness, distribution identity or budget
cannot be implemented/proven, C2 must retain existing exact production gate and
report the concrete gap. A passing emulator/schema cannot override that stop.

## C1 validation and publication

`git diff --check` is required. Repository CI has no docs-specific workflow or
ADR checker; ordinary CI runs on every push and includes fmt/check/test/Clippy
and Desktop Tauri permission validation. Check local Markdown links and ADR
number/status/supersession consistency as focused design review. No local
workspace tests, runtime probes, inference or Tool execution are required/run.

Authorized documentation paths: new ADR 0034, normative V1 specification, this
plan/report, preserved Task 510C audit follow-up, ADRs 0005/0006/0030/0033
cross-references, architecture and security design/current-state distinction.
No dependency/authority delta. Production Rust and Cargo/package files untouched.

Publication: one docs commit `docs: define Codex compatibility admission contract`,
normal push to GitHub origin/master, no tag/release/version bump or mirror push.
Require local HEAD = origin/master and GitHub CI success for that exact head.
Final commit/push/CI facts are reported by C1 closure, not inferred from this plan.

Pre-publication C1 checks executed: `git diff --check` PASS; focused local
Markdown link check PASS (7 links across the four new/report documents), balanced
code fences PASS, ADR 0034 number uniqueness PASS. Manual review reconciled
supersession/current-implementation wording across ADRs 0005/0006/0030/0033,
architecture and security. Staged whitespace/scope review follows before commit.
No local workspace test or runtime execution is claimed. Ordinary exact-head
GitHub CI is required after the documentation push.
