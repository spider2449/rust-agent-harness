# Task 509 — Provider-aware model picker and connection preflight

## 1. Checkpoint and disposition

Starting/local final HEAD: `44666d4de3e514339918fab4143f440d35892c8e`.
Live `git ls-remote origin refs/heads/master` independently matched this SHA.
Starting worktree was clean. Checkpoint CI `37127188517` PASS is supplied task
evidence, not a newly executed or independently refreshed CI result.
Version remains 0.33.0; certified Codex baseline remains 0.157.1.

**B — MODEL STATE/PERSISTENCE COUPLING REQUIRES A BOUNDED STATE-MODEL CORRECTION**

This is an audit disposition, not completion of the requested picker. Only this
plan changes. No implementation, commit, push, tag, release or version change.
Classification A is not established; Task 510 is not authorized by this result.

## 2. Existing provider/model state architecture

Three different values currently govern the product:

- `DesktopAppState.runtime_adapter` is selected at construction by
  `runtime_selection::selected_adapter`, using compiled features and
  `RAH_RUNTIME_PROVIDER`. It is not mutable through the model control.
- `DesktopModelState.selection` is desired configuration with a generation.
  `DesktopModelProvider` has Inherit/OpenAi/Ollama/LmStudio/LlamaCpp, no Codex
  variant. `codex_composition::codex_model_config` translates this selection
  into a provider *inside Codex*. In particular, OpenAi means Codex OpenAI.
- Connected composition captures a model generation. A later desired-state
  change marks it `reconnect required`; it does not rebuild the live runtime.

Evidence: `main.rs:411,587,1586,1702,1766,2016,5044,9487`,
`runtime_selection.rs:70-127`, `codex_composition.rs:135-182`.

## 3. Existing failure mechanism and bounded reproduction

`index.html:50-59` exposes upstream Codex providers and a free-form input with
a datalist. It does not expose Codex/native OpenAI adapter selection.
`status.js:2519-2526` changes disabled/visible/hint state without clearing the
model, refreshing its source or validating compatibility.
`status.js:2096` gates Connect on transition/chat state, not catalog validity.
The datalist is populated from Connect-time evidence by
`renderModelPreflight`, rather than a preselection discovery lifecycle.

Executed the actual extracted change handler with Node VM in
`F:/temp/task509-evidence/provider-state-audit.cjs`: three cases (OpenAI to
Ollama, Ollama to OpenAI, OpenAI to Inherit) preserve `gpt-6.1-sol`; Inherit
disables the retained input. Exit 0, zero network requests/inference starts.
These reproduce current upstream-provider coupling, not successful required
Codex/native OpenAI switches.

Native OpenAI has a separate mismatch: `configured_factory` ignores the UI
model and takes `RAH_OPENAI_MODEL` from backend environment configuration.
Changing the UI provider does not change the adapter used by `connect_codex`.

## 4. Codex model source

Existing neutral `RuntimeInstance::discover_models` and
`runtime_composition::create_and_preflight` consume a complete selected-runtime
catalog and emit Advertised/NotAdvertised. Explicit absent selection is blocked
before conversation binding; RuntimeDefault/unsupported discovery is NotChecked.
Task 499 remains unchanged. There is sufficient catalog data at the runtime seam;
this result is B, not a claim that catalog consumption is impossible (C).

Required correction: an ephemeral backend picker observation tied to the selected
adapter, admitted executable/runtime configuration and request generation. Discover
without opening a conversation or activating external Tool providers, own and
shut down the discovery instance, reject stale results, and refresh after runtime
selection changes. Do not add a second frontend Codex list or certification version.

## 5. OpenAI model source

Task 507 discovery is unsupported. Native OpenAI currently requires a backend
explicit model. No existing native OpenAI preset picker policy was found in the
Desktop selection path. The primary blocker precedes preset selection: the UI
and persistence cannot identify native OpenAI separately from Codex's OpenAI
provider. No public model-list calls, key reads, Responses calls or API-key UI
were performed/added during this audit. Task 508L remains paused.

For the correction, choose an explicit backend supported-preset policy, label it
`Supported presets`, and report local configuration readiness separately from
remote availability. Native OpenAI model configuration must agree with the staged
selection; backend configuration must not silently override the displayed model.

## 6. Custom-model decision

No custom-model implementation in this disposition. Recommended correction:
omit custom IDs initially; keep unknown stored values as inactive preferences,
never fabricate catalog membership. No unrestricted default text input.

## 7. Provider-switch policy and prerequisite

Current required Codex/native OpenAI switch is unrepresentable through the control.
Do not relabel legacy `openai` as native OpenAI: that changes existing preferences'
meaning and switches their execution route.

The bounded correction must separate runtime adapter choice from legacy upstream
Codex provider configuration; model validity/source must belong to the adapter
and runtime configuration. Immediately invalidate selection on provider change;
restore only an explicitly matching valid preference, otherwise leave empty unless
an existing policy defines a default. Never carry a prior provider's validity.

## 8. Runtime-switch policy

Executable selection is host-owned, resolved for connection; there is no picker
catalog freshness state keyed to that executable. No runtime was switched/upgraded.
Correction must refresh/revalidate the catalog for each newly selected/admitted
runtime and reject late responses from its predecessor. Connect must retain fresh
runtime preflight even after successful picker discovery.

## 9. Connect gating

Current explicit Codex Connect-time gate remains intact; picker gating is absent.
Required eligibility is backend-confirmed provider/runtime configuration plus a
current valid model selection. Loading/empty/unavailable/error states fail closed.
No catalog evidence may validate a different adapter or runtime configuration.
OpenAI local/preset checks must never claim discovery, entitlement or inference proof.

## 10. Persistence behavior

`desktop-preferences.json` versions 1..3 restore a structurally checked provider,
model and optional endpoint; current writes are version 3. The provider is the
legacy upstream provider; no runtime-adapter discriminator is persisted.
`desktop_preferences.rs:373-438,450-523` and
`main_tests.rs:17093` establish inactive desired-state restore, not catalog validity.

Correction needs backward-compatible interpretation of those values, a distinct
adapter-scoped preference representation, and ephemeral validated selection.
Do not overwrite/delete a syntactically valid but currently absent model merely
because catalog revalidation fails. Preserve identity/Trusted Profile preferences.

## 11. Connected-state behavior

Current controls disable during chat, not for the whole connected lifecycle.
Applying preferences while idle can stage changes, increment model generation,
and require reconnect. Connected runtime is not implicitly recomposed.
Existing tests assert generation separation and rejection during chat.
Correction should disable selection during connection/connected/disconnection,
or retain explicit staging with clear active-versus-next-selection semantics.
Disconnect should restore editability and trigger current-source revalidation.
No live runtime or lifecycle behavior changed in this audit.

## 12. `gpt-6.1-sol` regression

Reproduction confirms that the existing UI preserves this unverified string on
upstream-provider changes. It does not prove the requested corrected dropdown.
Existing source gates any explicit absent ID for a complete catalog; the existing
deterministic runtime fixture tests zero conversation opens/sends for `absent`.
Those Rust tests were inspected, not executed in this turn.

Correction must add a named 0.157.1-representative fixture omitting `gpt-6.1-sol`:
no normal dropdown choice, no catalog-valid Connect, zero inference starts. Do not
generalize that fixture into a permanent version/model prohibition.

## 13. Focused validation actually executed

- Existing `node crates/rah-desktop/frontend/model_preflight_test.js`: PASS,
  exit 0 (one script; no individual test-count report).
- `node --check crates/rah-desktop/frontend/status.js`: PASS, exit 0.
- Current-handler audit: 3 cases reproduced stale retention, exit 0.

No new provider-aware implementation/tests were introduced. Required Codex/native
OpenAI switches, refresh, Connect gating and invalid persisted-pair acceptance
are NOT validated. This is a state-model blocker, not deterministic gate failure E.

## 14. Full validation

Workspace fmt/check/test/clippy, canonical Windows Desktop, complete frontend/static,
Tauri inventory, metadata and executable HostExplicit gates were NOT run in this
audit disposition. Counts are unavailable, not zero-pass certification. No source
patch exists to certify. Final `git diff --check` and plan whitespace check are
recorded in the final return. Historical Task 508 counts are not Task 509 evidence.

## 15. Production/manual acceptance

Not performed. No Desktop runtime connect, discovery process or inference was
started. Source/Node handler audit is not GUI or production-backend acceptance.

## 16. HostExplicit

Inspected `host_invocation::host_kind` and `host_allowlist_is_exact`: exactly 11
closed supported names. Source unchanged. Executable test not rerun; no executable
certification claim. No Tool registration, policy or permission edits.

## 17. Authority/security impact

None: documentation-only patch. ToolRegistry authorization, repository authority
and switching, leases, permissions, Trusted Profiles, remembered workspace semantics
and mutation uncertainty remain unchanged. No live OpenAI action, credential access,
runtime upgrade, host configuration or layout/navigation/settings redesign.

## 18. ADR decision

ADR-B for this audit: no new ADR. ADR 0033 host-owned composition and ADR 0030
exact runtime admission remain intact. The prerequisite is bounded Desktop state
and preference correction, not new public runtime/provider protocol contracts.

## 19. Publication and final classification

**B — MODEL STATE/PERSISTENCE COUPLING REQUIRES A BOUNDED STATE-MODEL CORRECTION**

No commit/push authorization under Task 509's classification-A-only rule.
HEAD/master remain the checkpoint; no new exact-head CI run is requested.
Worktree intentionally retains this untracked plan for review; it is not clean.

## 20. Exact next scope and Task 510 recommendation

Next prerequisite: bounded Task 509 state-model correction, separating host runtime
adapter preference, legacy Codex upstream-provider preference, current provider-scoped
picker validity and connected composition; migrate legacy persistence honestly,
add backend source/freshness state, then finish this task's picker, preflight,
deterministic gates and bounded production acceptance. No overall UI redesign.

Only after classification A and its normal push/natural exact-head CI PASS:
Task 510 — Codex runtime refresh/certification. Audit a proposed newer exact CLI's
schema/model catalog, admission and artifact hashes; run deterministic regressions
and required direct/Desktop certification under ADR 0030; update the exact current
certification set/preferred baseline only from that evidence. Keep OpenAI live work
and Task 508L paused, HostExplicit exactly 11, version 0.33.0, no tag/release and
no full Desktop UI redesign. Do not start Task 510 from this classification B.

## Task 509A reference-only follow-up

The bounded state-model prerequisite is tracked in [Task 509A](2026-10-04-task-509a-runtime-adapter-model-provider-state-separation.md). This reference does not revise Task 509's classification or historical evidence.

## Task 509B reference-only follow-up

[Task 509B](2026-10-04-task-509b-provider-aware-model-picker-catalog-freshness-and-connect-gating.md)
records the preselection backend catalog/source contract gap and classification B.
This reference does not revise this plan's historical findings or classification.
