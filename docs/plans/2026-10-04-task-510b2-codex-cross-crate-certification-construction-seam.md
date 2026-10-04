# Task 510B2 — Codex cross-crate certification construction seam

Reference-only follow-up: [Task 510B2A](2026-10-04-task-510b2a-certification-error-assertion-and-harness-validation.md)
stopped under B after finding that the verification cause omits exact hash,
version and path evidence. The historical failed run below remains unchanged.

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
The two stopped reports are preserved. Task 510B remains stopped.

Plan: implement S2, a dedicated Desktop test target requiring an explicit,
non-default `certification-harness` feature. Avoid dev-dependency feature
unification into ordinary tests. Add an exact absolute path/version/SHA-256
descriptor and sealed verified candidate factory; retain schema, real adapter
and neutral Desktop composition. Verify identity again at create. Run focused
deterministic and exact-candidate no-turn smoke, production graph proof, full
deterministic gates and canonical Desktop gate. Stop on failure. Only A permits
commit/push and natural exact-head CI.

Artifact identity is not protocol/model/Desktop certification or production
admission. Preferred/admitted 0.157.1 and its baseline remain unchanged.

## Stop: E — DETERMINISTIC VALIDATION FAILED

First completed focused run:

```powershell
$env:CARGO_TARGET_DIR='F:/temp/rah-task504-target'
$env:RAH_TEST_TARGET_DIR='F:/temp/rah-task504-target'
cargo test -p rah-runtime-codex --features certification-harness certification_support -- --nocapture
```

Build succeeded; **2 passed / 1 failed / 0 ignored / 103 filtered out**, exit 1,
test time 0.57s. Failure:
`certification_support::tests::exact_factory_version_and_production_separation`,
`certification_support.rs:216`: expected neutral failure Display to contain
`SHA-256 mismatch` after changing the executable bytes. No retry or post-failure
source correction. Source was unchanged throughout this completed run.
An earlier default-target command was interrupted after waiting on a build lock;
it produced no test results. The selected target was idle before this run.

Diagnosis: `rah-runtime/src/failure.rs:44` displays only sanitized
RuntimeDiagnostic; Error::source retains the adapter cause. The assertion must
inspect that typed source, preserving sanitization. The unrun installed smoke
contains the same assertion pattern. The failed assertion did receive an error,
but did not capture its typed source, so its precise subtype is not proven.

## Construction and graph review

The Task 510B1 dependency-cfg(test) visibility blocker is resolved in source by
feature-gated, doc-hidden `rah_runtime_codex::certification_support`. Minimal
surface: `ExactCodexCandidate { path, expected_version, expected_sha256 }`,
`verify_and_construct_candidate(candidate, provider, workspace)` and sealed
`VerifiedCertificationCandidate`, implementing ConfiguredRuntimeFactory.
Every identity field is mandatory. Verification requires an absolute existing
native executable, exact numeric version and complete lowercase SHA-256;
streamed hashing precedes the bounded version probe. No PATH resolution or
launcher substitution. Create repeats identity verification before the real
schema/app-server path. Verification is artifact identity, not certification.

S2: dedicated Desktop integration test `codex-certification`, requiring the
explicit non-default Desktop `certification-harness` forwarding feature.
It reuses `src/main.rs` to access private Connect/Disconnect/backend composition;
Cargo warns about the shared source between bin and test targets. There is no
dev-dependency feature activation or package #15. Desktop smoke code is cfg(test)
only, even when all features compile. The normal factory's candidate field is
always None; compiling support never selects candidate authorization. No ordinary
constructor visibility was expanded and no neutral API was changed.

```powershell
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc -e features -i rah-runtime-codex
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc -e features -i rah-runtime-codex --features certification-harness
cargo metadata --no-deps --format-version 1
```

Inverse graph: default has only adapter default, certification-harness disabled;
explicit certification graph enables it via the command-line Desktop feature.
Metadata: 14 workspace members/packages, all 0.33.0. Cargo.lock has no diff;
no existing dependency edge or version changed. Graph/metadata evidence and
source freeze hashes are retained in `F:/temp/task510b2-evidence`.

## Validation ledger

| Requirement | Result |
| --- | --- |
| Exact descriptor, relative/missing path and missing identity | Focused test PASS |
| Wrong hash before process probe | Focused test PASS on test executable |
| Wrong expected version | Assertion passed before later failure in fixture test; whole test failed |
| Production rejection of 0.160.0 | Typed VersionMismatch assertion passed on deterministic identity-probe fixture; exact installed-artifact test not run |
| Verified candidate construction, neutral trait, exact supplied fixture path | Assertions passed before later failure; whole test failed |
| Admission unchanged | Fixture equality assertion passed before failure; admission constants unchanged |
| Conflicting PATH | Not run |
| Artifact change after construction | Returned error; assertion on sanitized Display failed |
| Real 0.160.0 identity/schema/startup/adapter/neutral runtime/Desktop backend/shutdown | Not run; smoke target not compiled or executed |
| Installed candidate pre/post hashes | Not measured |
| Inference / Tool execution | 0 / 0 |
| Formatting | cargo fmt executed; cargo fmt --check not run |
| Full workspace fmt/check/test/clippy, all-features | Not run after focused failure |
| Default build/tests and canonical Windows Desktop | Not run; no workspace/Desktop passing counts |
| Tauri 47/47/47/47/47 | Inventory not run; command/permission/frontend sources unchanged |
| HostExplicit 11 | Implementation unchanged; static/executable gates not run |
| Metadata / default feature graph | PASS as recorded above |
| git diff --check | PASS |

Failed fixture is preserved at
`F:/Temp/rah-certification-e11436fb-7740-4092-bd3a-747f9d27f313`.
No discard/reset/stash/cleanup or retry was performed.

## Security, ADR and resume boundary

Production/preferred `codex-cli 0.157.1` and supplied baseline SHA-256
`8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`
remain unchanged. Baseline storage was not remeasured. Candidate 0.160.0 is not
admitted. No global override, mutable admission, environment switch, CLI flag,
Tauri command, frontend invoke, configuration/preference field or certification
Tool was added. Identity authorization is local to the constructed factory;
ToolRegistry, permission policy, repository authority and HostExplicit source
remain unchanged. Security certification is incomplete.

ADR-B: existing ADR 0030 exact-artifact policy and ADR 0033 neutral composition
are sufficient. No new ADR or public neutral contract change.

Narrow follow-up: correct source-based rejection assertions in fixture/smoke,
review/compile Desktop target, complete exact-path/conflicting-PATH coverage,
then focused tests, installed no-turn smoke and full default/workspace/canonical
gates. Preserve failed evidence and do not weaken sanitization.

Only subsequent Task 510B2 A permits Task 510B to resume its exact-artifact
schema/deterministic/direct gpt-6.1-sol/Tool/cancellation/diagnostic/Desktop turn
gates with pre/post hashes. Admission/preferred changes remain separate work.
This E authorizes no Task 510B resume or publication.

No commit/push/CI/tag/release/version bump. HEAD remains starting checkpoint.
Worktree intentionally dirty: six tracked implementation edits, two new Rust
files and three untracked plans. Stop reports preserved with reference-only notes.
