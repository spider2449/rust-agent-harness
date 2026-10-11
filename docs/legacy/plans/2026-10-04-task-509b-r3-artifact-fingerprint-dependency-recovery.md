# Task 509B-R3 — Artifact fingerprint dependency recovery

Recovery reference: [Task 509B-R4](2026-10-04-task-509b-r4-custom-provenance-rendering-and-final-validation.md)
records the narrow provenance correction and resumed gates; stopped R3 evidence
below remains unchanged.

Starting HEAD: `1907ee69a8ab9b959cf07e04f43c2f3e966121e5`.
Preserved R2 checkpoint: 13 modified tracked files, 3 untracked, no staged files.

## Plan

1. Audit ADR 0030, baseline hash storage, runtime admission and R2 identity use.
2. Apply exactly one justified dependency-boundary correction; preserve semantics.
3. Freeze implementation and rerun the exact failed focused command.
4. Stop on another compile error or test failure; otherwise resume R2 focused,
   workspace, Desktop, frontend/static, inventory, authority and manual gates.
5. Only classification A permits commit, normal master push and exact-head CI.

## Admission audit and D2 decision

R2 failed with E0432 at `experimental.rs:43`: production imports `sha2`, but
the owning crate declared it only under dev-dependencies; zero tests executed.

ADR 0030 decision 6 explicitly separates baseline manifest/hash verification
from runtime admission. `scripts/codex-baseline.ps1` computes file SHA-256 via
`Get-Hash`, stores it in `manifest.json` as `sha256` (and separately
`code_mode_host_sha256`), and `Verify-BaselineDirectory` recomputes/verifies it.
These are baseline-store verification values, not Rust admission output.
`ProcessTransport::start` resolves the executable, verifies its exact version
and generated schema, then starts app-server. It retains no SHA-256 identity.
Desktop's artifact selector can use a baseline, host override or PATH; a stored
baseline hash cannot identify every selected executable. Task 509E also records
that current admission does not return an executable fingerprint.

D2 selected: production must measure the resolved file for snapshot freshness
and Connect comparison. D1 has no existing authoritative admission fingerprint
to reuse. D3 would remove required production artifact replacement detection.
The unchanged adapter-local `CodexFactory::measured_artifact` computes exact
whole-file SHA-256, lowercase hexadecimal; Desktop calls it through
`spawn_blocking`. Version/schema admission remains separate and mandatory.
This is file measurement, not a running-image or TOCTOU guarantee. Candidate
0.160.0 evidence is never admission authority; baseline remains 0.157.1.

Only code/Cargo correction: move `sha2.workspace = true` from dev-dependencies
to dependencies in `rah-runtime-codex/Cargo.toml`. Workspace version remains
`0.10`; production edge is `rah-runtime-codex -> sha2`. No Rust implementation,
picker policy, public API, crypto algorithm or runtime certification change.
ADR-B: routine dependency placement; no new ADR required.

## Frozen-source validation

Evidence: `F:/temp/task509br3-evidence/`. Both target variables used
`F:/temp/rah-task509a2-target-run2`. `source-freeze.txt` records modified and
untracked implementation file hashes before validation;
`source-freeze-verification.txt` confirms they remained unchanged after failure.
No Rust or frontend source was edited during validation. Cargo.lock has no diff.

Exact resumed command:
`cargo test -p rah-desktop model_source::tests -- --test-threads=1`.
Compilation succeeded, exit 0: **17 passed / 0 failed / 0 ignored**, 361 filtered
out. Log: `focused-source.log`; exit: `focused-source.exit`.

Focused deterministic results:
- Snapshot ownership, artifact-generation change and newest same-generation
  request ownership passed; loading withdraws prior normal validity.
- Stale success and stale error rejection passed; Disconnect revocation passed.
- Provider switches and adapter roundtrip passed, including compatibility reset.
- Restored Advertised requires fresh membership; restored Custom remains Custom
  even when catalog-advertised. No absence-to-Custom inference.
- Inherit/no explicit ID and explicit-provider checks passed.
- Native configured-only and no-runtime resolution passed without Codex leakage.
- Structural Custom, unavailable/error/empty Advertised gating, exact-context
  rejection and resistance to Custom relabeling passed.
- Absent-catalog `gpt-6.1-sol` fixture blocks and remains Advertised, passed.
These are deterministic fixture results, not live model compatibility proof.

Workspace commands executed serially and passed, exit 0:
`cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`git diff --check`. Logs and exit files retained. Workspace test/doc-test sum:
**1085 passed / 0 failed / 24 ignored**. Desktop segment: **358/0/20**.
No LNK1103 or other corruption occurred; no fresh-target retry was required.

Canonical command:
`powershell -NoProfile -File scripts/windows-desktop-test-gate.ps1 -TargetDirectory F:/temp/rah-task509a2-target-run2 -OutputDirectory F:/temp/task509br3-evidence/desktop`.
Run `20261004-145832-443-5f44a2b55c0345db88f6e5d79bcbf410`:
**358 passed / 0 failed / 20 ignored; exit 0**, helper build exit 0,
no watchdog timeout. Status and stdout/stderr retained under `desktop/`.

## First later stable-source failure

Syntax checks passed for all eight frontend JS files and
`tauri_permission_test.js` (nine files total). Complete frontend execution
started alphabetically and stopped at its first suite, `model_picker_test.js`.
Exit **1**, `frontend-model_picker_test.js.log` and `.exit`:

```text
AssertionError [ERR_ASSERTION] at model_picker_test.js:41:8
actual: 'Custom ? unverified. Advertised by Codex runtime ? provider compatibility Unverified'
expected: /Custom · unverified/
```

`status.js:1365` contains a literal question mark in `Custom ? unverified. `;
the fixture expects the middle dot. Other new labels also contain literal
question marks. The exact mismatch is in the preserved R2 frontend source and
fixture, not SHA-256, dependency resolution, a Rust compile failure or linker
corruption. No source correction or retry was made. This assertion terminated
before the later async frontend stale-result checks could execute; those frontend
checks remain unvalidated despite their Rust counterparts passing.

Remaining six frontend suites were not executed in this run. Tauri inventory
was not reached: **47 matching commands is not certified**. Builds generated
`permissions/autogenerated/model_source_snapshot.toml`; preserve that new file
as build output, not an additional manually authored permission grant. No complete
permission-drift result is claimed. Cargo metadata gate was not reached:
14 packages/members and all-0.33.0 metadata certification are not claimed.
Executed Cargo output retained 0.33.0 for compiled packages; no version edits.
The explicit audited production edge is `rah-runtime-codex -> sha2` using the
existing workspace `0.10` declaration, with no lockfile churn or new crypto crate.

Manual acceptance was not reached. No real dropdown/catalog, Connect, Custom,
switching or native OpenAI Desktop acceptance result is claimed. No live OpenAI
request, API-key usage, inference or Codex 0.160.0 certification ran.

## Authority, ADR and final disposition

HostExplicit static allowlist remains exactly **11** (`host_invocation.rs`,
closed `host_kind` match). Executable `host_allowlist_is_exact` and eleventh-tool
tests passed in workspace and canonical Desktop suites. No hashing correction
changes ToolRegistry authorization, repository authority/switching, leases,
Trusted Profiles, permissions, mutation uncertainty or remembered workspaces.
R3 changes only dependency placement and reports; the preserved R2 presentation
IPC and lifecycle WIP remain subject to full completion gates. No new authority
or lifecycle regression was observed by executed tests; incomplete frontend and
permission validation prevent an overall product-completion claim.

ADR-B; no new ADR. Task 509E policy, v4 modes, runtime admission set and preferred
0.157.1 baseline remain unchanged. Raw measured file identity is existing R2 DTO
behavior; R3 introduces no certification internals or frontend identity API.

**E — LATER DETERMINISTIC VALIDATION FAILED**

Task 509 remains incomplete. No commit SHA, push or exact-head CI exists for this
WIP. HEAD remains the starting SHA. Worktree deliberately remains dirty:
14 modified tracked files, 5 untracked, no staged files. Entire initial WIP,
dependency correction, recovery report and generated permission file preserved.
Closure `git diff --check` is recorded separately after this report update.

Next recovery is the narrow frontend label/fixture mismatch, followed by resumed
complete frontend/static, Tauri/metadata and bounded Desktop acceptance; retain
all required gates before classification A and publication. Do not start Task
510B. Its exact deferred scope is runtime certification under ADR 0030: exact
artifact/schema audit, deterministic regression, direct and Desktop certification
evidence, and separately authorized admission/baseline update. No broad UI
redesign or live native OpenAI work is authorized.
