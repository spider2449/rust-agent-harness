# Task 509B-R2 — Runtime-advertised picker, Custom, freshness and Connect

Recovery reference: [Task 509B-R3](2026-10-04-task-509b-r3-artifact-fingerprint-dependency-recovery.md)
audits the artifact dependency boundary and records resumed validation. The
stopped R2 evidence below remains unchanged.

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`; clean worktree.

## Plan

1. Inspect Task 509E policy, v4 persistence, accepted ADRs and current composition.
2. Add a bounded Desktop-owned source snapshot with generation and request ownership.
3. Implement Advertised/Custom/Inherit presentation and backend Connect rechecks.
4. Test stale success/error/loading, restoration, adapter/provider changes and gating.
5. After focused PASS, freeze source and run full workspace, canonical Windows
   Desktop, frontend/static, Tauri inventory, metadata and HostExplicit gates.
6. Perform bounded local acceptance without inference or live OpenAI. Only A
   authorizes commit, normal master push and natural exact-head CI closure.

P1: Codex catalogs are runtime advertisements, never provider compatibility proof.
Compatibility remains Unverified. Native OpenAI is configured-snapshot only.
No new runtime certification, version change, UI redesign or authority expansion.

Same-generation rule: newest backend request ID wins. Both generation and request
must match at publication; replacement withdraws prior loading/error/catalog state.
No wall-clock time participates in correctness.

Implementation and validation evidence will be recorded below before closure.

## Stopped implementation and source design

Uncommitted implementation is preserved, not certified:

- Desktop-private `model_source::Owner` and bounded `Snapshot` carry adapter,
  measured/admitted artifact file SHA-256, requested Codex upstream provider,
  generation/request, explicit mode/ID, source state, eligibility and Unverified
  compatibility. Requested provider is context, never catalog ownership.
- Adapter-local `measured_artifact` resolves the native selector and hashes the
  file; normal process startup retains exact version/schema admission. This is
  a file measurement, not a running-image or TOCTOU guarantee. The separate
  `advertised_catalog` path opens no conversation or inference and shuts down.
- Context includes adapter, Codex configuration/selection, host artifact selector
  and native configured model. Binding a changed context advances generation;
  measured artifact replacement also advances it. Newest request ID wins in a
  generation. Lifecycle-coordinated publication rejects both stale successes
  and errors before they can replace catalog/loading/eligibility state.
- Source states: Loading, AdvertisedCatalog, EmptyCatalog, Unavailable, Error,
  RuntimeDefault, Configured and NoRuntime. Loading withdraws normal validity.
- Advertised restoration waits for fresh membership. Missing IDs remain
  Advertised and invalid; no automatic Custom conversion or substitution.
- Explicit `Custom model ID…` reveals the input. Saved Custom remains Custom
  even when advertised. Local validation reuses the existing 256-byte,
  nonempty, no control/NUL or leading/trailing whitespace contract. Catalog
  error/empty does not itself reject structurally valid Custom after loading.
- Inherit requires no explicit ID/mode. An explicit upstream provider is
  required for an explicit model. Runtime-default compatibility is Unverified.
- Native OpenAI resolution uses only the backend configured model: no presets,
  Custom, discovery, credentials or HTTP path. Missing configuration blocks.
  None yields NoRuntime with no active model and blocks Connect.
- Eligibility is a discriminated enum, separate from source and compatibility.
  Fresh Advertised membership, structural Custom, admitted Inherit and configured
  native state can permit Connect. Missing/malformed selection, Loading,
  absent Advertised, unavailable Advertised source and known rejection block.
- Connect drafts refresh evidence; publication rechecks generation, request,
  artifact identity and current mode-aware eligibility under lifecycle
  coordination. Existing composition generations remain checked. Independent
  exact-context rejection is represented; relabeling the same ID Custom must
  preserve it, while real provider/runtime/model context changes clear it.
- Disconnect revokes old request ownership. Connected picker/input controls are
  disabled; existing backend lifecycle remains authoritative. Provider controls
  retain staged configuration behavior. No hot runtime recomposition is added.

These describe intended code paths only. Compilation failed before any of their
tests executed; completeness, behavior and lifecycle safety remain unproven.
No manual Desktop acceptance result is claimed.

## Focused tests authored, not executed

Seventeen new Rust tests cover stale success/error/loading, newest request in
one generation, loading withdrawal, restored Advertised membership, catalog-
relative `gpt-6.1-sol` absence, empty/error/unavailable distinctions, Custom
restoration and structural checks, Inherit, provider/artifact changes, exact-
context rejection including Custom relabeling, native/None resolution, adapter
roundtrip and Disconnect revocation.

The new frontend picker test covers source options/provenance, missing
Advertised restoration, Custom restoration/action, hidden input outside Custom,
Connect eligibility, native configured-only/None, late success/error/loading
and connected control immutability. The existing runtime-state frontend fixture
was migrated for the source DTO and command. Neither frontend test ran.

The `gpt-6.1-sol` test uses a bounded 0.157.1-relative catalog fixture; it is not
a permanent version/model rule. Its blocked/no-inference result is unvalidated.

## First stable-source validation failure

Evidence: `F:/temp/task509br2-evidence/`.

`cargo fmt` executed successfully before source freeze. This was formatting,
not the requested full `cargo fmt --check` validation gate.

Both target variables: `F:/temp/rah-task509a2-target-run2`.
Frozen source hashes: `source-freeze.txt`.

Command:
`cargo test -p rah-desktop model_source::tests -- --test-threads=1`

Exit **101**, log `focused-source.log`, exit file `focused-source.exit`:

```text
error[E0432]: unresolved import `sha2`
  --> crates/rah-runtime-codex/src/experimental.rs:43:13
```

Exact cause: the new production `CodexFactory::measured_artifact` imports
`sha2`, but `rah-runtime-codex/Cargo.toml` declares it only in
`[dev-dependencies]`. The adapter library cannot resolve that production import.
This is a source/dependency placement defect, not stale-target/linker corruption.
No fresh-target isolation or retry was warranted. No source correction was made
after the frozen failure. There may be further defects hidden by this first
compilation error; no single-defect or otherwise-green claim is made.

Focused counts: **0 tests executed**; no PASS. Full workspace check/test/clippy,
canonical Windows Desktop, complete frontend/static, Tauri inventory and cargo
metadata gates did not run because focused PASS was not achieved. No workspace
or Desktop test counts, executable HostExplicit result or manual acceptance.
Generated Tauri permission output may exist from the failed build; inventory
consistency is not certified. Closure `git diff --check` is recorded separately.

Closure: frozen implementation hashes unchanged after failure;
`git diff --check` exit 0 (`closure-diff-check.log` and `.exit`). Git status
shows 13 modified tracked files and three untracked task files, all preserved.
Final HEAD equals starting HEAD; no publication was attempted.

## Authority, ADR, disposition and publication

Static `host_kind` remains exactly **11** named routes; executable certification
was not reached. No changes to ToolRegistry, repository authority/selection,
leases, Trusted Profiles, permission levels, mutation uncertainty or remembered
workspace semantics. The one new IPC permission is a model-source presentation
read/refresh, not an executable Tool or HostExplicit grant. Lifecycle code was
modified for source publication/revocation and must still be validated.

ADR-B: this implements existing 509E policy and v4 provenance; no new ADR was
created. Cargo manifests/lockfile, dependency edges, version and Codex admission
constants remain unchanged. That unchanged dependency declaration is precisely
the production import defect above. No authority/security expansion is intended;
no validated absence of lifecycle regressions is claimed.

**E — DETERMINISTIC VALIDATION FAILED**

Task 509 is incomplete. No commit, push, new CI, tag, release or version bump.
HEAD remains `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`. Worktree deliberately
retains the stopped implementation, new tests and documentation; it is not clean.
No Codex 0.160.0 certification, live OpenAI request or inference ran.

Next recovery must first resolve production fingerprint dependency placement
without moving provider details into neutral APIs, then compile and validate the
entire preserved implementation. Do not assume the remaining gates will pass.

Exact Task 510B scope remains deferred until 509 achieves A and publication
closure: runtime certification only under ADR 0030, with exact artifact/schema,
deterministic regression, direct and Desktop certification evidence and any
separately authorized admission/baseline update. It does not authorize a full UI
redesign or live native OpenAI work. This E result does not authorize starting it.
