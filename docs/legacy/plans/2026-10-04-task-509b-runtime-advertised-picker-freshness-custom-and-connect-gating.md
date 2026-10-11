# Task 509B-R — Runtime-advertised picker, freshness, Custom and Connect gating

Reference-only follow-up: [Task 509B-R2](2026-10-04-task-509b-r2-runtime-advertised-picker-custom-freshness-and-connect.md)
resumed implementation after preferences v4 publication, then stopped with E
on the first focused compile failure. Its preserved WIP is not validated and
does not change this report's historical classification or authorize Task 510B.

Starting HEAD: `8c88bfac36d96bb16958375936a315170a2a6cb8`; clean worktree.
Task 509E policy A is authoritative. Certified/preferred Codex remains 0.157.1.

## Plan and mandatory stop

1. Verify checkpoint and read Task 509E policy.
2. Audit existing Codex model persistence before implementing Custom.
3. If persistence can safely distinguish modes, implement host source snapshot,
   generation/request freshness, picker and mode-aware Connect gates; run focused,
   full and Desktop acceptance gates before classification A and publication.
4. If a schema migration is required, stop under task section 21 and classify C.

**C — CUSTOM MODE CANNOT BE PERSISTED WITHOUT SCHEMA MIGRATION**

Step 2 establishes the explicit stop condition. No product implementation or
schema migration was performed. Task 509 remains incomplete.

## Exact persistence evidence

- `crates/rah-desktop/src/main.rs:1710`: `DesktopModelSelection` contains only
  upstream provider, optional model string and optional llama.cpp endpoint. It
  carries no Advertised/Custom discriminator.
- `crates/rah-desktop/src/desktop_preferences.rs:368`: canonical serialization
  writes provider/model/endpoint and emits preferences version 3 at line 401.
  The same provider and model string serialize identically regardless of intended
  normal versus Custom provenance.
- Lines 450–453 define the persisted model object without selection mode.
- Lines 487–489 admit only versions 1 through 3.
- Lines 548–552 implement closed objects: unknown and duplicate fields fail;
  the model map admits only provider, model and endpoint.

Adding a mode field changes this closed schema; existing readers reject it.
Encoding a mode in a model ID or provider changes their existing meaning.
Inferring Custom from catalog absence violates the task. An in-memory mode loses
explicit user intent on restart. Thus the existing schema cannot safely restore
explicit Custom as required by sections 19–21 and 37.

## Bounded migration recommendation

Authorize a separate preferences schema migration with an explicit Codex model
selection discriminator (Advertised or Custom), retaining Inherit/default semantics.
Read legacy versions 1–3 as historical normal preferences, never inferred Custom;
revalidate them against the current catalog before Connect. Write a new version
with explicit mode. Preserve native OpenAI configured-only behavior, inactive
Codex preferences, commit identity, remembered Trusted Profile path, byte bounds,
closed/duplicate-field rejection and atomic persistence behavior.

Require deterministic legacy restoration, explicit Custom round-trip, malformed
mode rejection, reset behavior and preservation of unrelated preference fields.
Do not persist compatibility validation. Resume this task only after the bounded
migration prerequisite is resolved.

## Implementation and validation disposition

Backend source snapshot, artifact measurement, freshness ownership and
same-generation request policy: not implemented. Proposed resumed rule is newest
request wins, with both context generation and request identity checked before
publishing either success or error; this is not current behavior certification.

Advertised picker, normal selection restoration, Custom UI/persistence, Inherit
gating, native OpenAI picker and Connect matrix: not implemented. Task 509E policy
remains unchanged; no provider-bound catalog claim or ProviderValidated evidence.

Runtime/provider switching, stale success/error rejection and the absent
`gpt-6.1-sol` fixture regression: no new tests or acceptance performed.
Focused/workspace/Desktop/frontend/static gates, Tauri inventory, metadata and
HostExplicit executable checks: not run because the mandatory prerequisite stop
occurred before implementation. No test PASS or failure classification is claimed.
Documentation whitespace check is recorded separately at closure.

Manual acceptance: not performed. No runtime discovery, Connect, conversation,
inference, native OpenAI network request or API-key use occurred.

Authority/security impact: documentation only; no ToolRegistry, repository
authority/switching, lease, permission, Trusted Profile, uncertainty or remembered
workspace implementation changes. HostExplicit source is unchanged; no new count
certification. ADR-B: existing product policy, no new ADR or dependency edge.

Git disposition: two documentation files only; no commit/push/CI under non-A.
Worktree has intentional uncommitted documentation changes, not a clean closure.
No tag, release, version bump, runtime certification or UI redesign.

Task 510B remains separate and is not authorized by this result. After eventual
509B classification A and exact-head publication closure, its scope is exact
candidate artifact/schema admission and runtime certification under ADR 0030,
including required deterministic and direct/Desktop evidence. Do not start it
here; keep 0.157.1 preferred and Task 508L paused.

## Task 509F reference-only follow-up

[Task 509F](2026-10-04-task-509f-preferences-v4-model-selection-mode-migration.md)
records the separately authorized v4 persistence prerequisite. This reference
preserves the historical C disposition above. Resume requires Task 509F A and
publication closure; no picker, freshness or Connect work is resumed here.
