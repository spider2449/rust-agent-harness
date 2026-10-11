# Task 510B2K — Exact-byte Codex certification snapshot

Status: **G — LATER DETERMINISTIC VALIDATION FAILED**. Stopped at the first focused build failure. No live controls were attempted.

Starting and ending HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserved Task 510B–510B2J WIP and reports. No commit, push, CI, tag, or release.

## Boundary and implementation WIP

The historical live-path SHA `3e4520220a642fbd7c80dad7c99579afb5bb515b83fd2a0127d47b811d7f8f91` remains unexplained. The npm path is not a trusted certification execution boundary.

Changed this task:
- `crates/rah-runtime-codex/src/certification_support.rs`: single-open source stream hash/copy, create-new destination, flush/sync/close, source handle pre/post metadata comparison, independent repeated snapshot measurement, distinct Windows identity requirement, readonly snapshot, frozen snapshot selector, typed local errors, fixture test.
- `crates/rah-desktop/src/certification_tests.rs`: formatting of existing certification WIP and separate ignored snapshot campaign control, with no persistence teardown or remove_dir_all. Planned sequence is snapshot creation, independent PowerShell hash, version, real adapter handshake/shutdown, real neutral Desktop connection/shutdown, repeated measurements.
- `crates/rah-runtime-codex/src/process.rs`: certification-feature process-local spawn path/PID evidence for snapshot directories. No shutdown behavior changed.

The source is opened once for copying; each buffer is hashed and written from the same read. No hash(path) followed by File::copy(path) is used. Initial handle length, identity, timestamps, attributes, volume serial and supplied/canonical paths are retained. Post-read checks use the same handle. Rejected outputs are abandoned and never returned as descriptors. This patch is unvalidated and does not establish the requested trust boundary yet.

## First failed deterministic gate

Command:
`cargo test -p rah-runtime-codex --features certification-harness certification_support::tests -- --nocapture`

Target and fixture target: `F:/temp/rah-task510b2g-target`.
Log: `F:/temp/task510b2k-evidence/focused.log`.

Compilation failed before tests ran:
- E0433: `uuid` is unavailable in the normal library dependency graph; the new isolated-directory helper refers to a currently dev-only dependency.
- E0599: thiserror interprets the `SnapshotMismatch.source: ArtifactMeasurement` evidence field as an error source. ArtifactMeasurement does not implement std::error::Error.

No source was edited while the build ran. No retry or post-failure implementation repair was performed. The wrapper returned 0 after displaying the log; the Cargo result was unequivocally `could not compile rah-runtime-codex (lib) due to 2 previous errors`, not a passing test gate.

## Measurements and live controls

No current source measurement was obtained. Historical Task 510B2J evidence is preserved, not remeasured here:
- Source: `C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`.
- Historical source SHA: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
- Historical bytes/length: `326872368`.
- Historical source file ID: `0x0000000000000000001d00000000b96d`.

Snapshot path, SHA, bytes, length, file ID, and pre/post stability: unavailable; no snapshot created.
Independent PowerShell snapshot hash: not run.
Version-only snapshot control: not run.
App-server snapshot control: not run.
Desktop snapshot composition control: not run.
Sanitization fixture assertions: added, not executed because compilation failed.
Inference = 0; Tools = 0. No executable was launched for this task's controls.

## Checks, impact, and next task

`cargo fmt` formatted existing unpublished certification-test WIP without intended behavior changes to that existing test. `cargo fmt --check`: PASS. `git diff --check`: PASS. These checks do not override the failed focused gate.

Admission/preferred baseline remain exact `codex-cli 0.157.1`; no baseline, admission, frontend/config, cleanup, model-picker, authority, or shutdown change. No crate dependency edge added. ADR-B: stronger artifact boundary under ADR 0030; no new ADR or authority model. Task 498 sanitization contract remains unchanged, but new snapshot assertions are not proven.

Next task must first correct the two narrow compilation defects and validate Task 510B2K snapshot support, then perform its one-attempt live snapshot controls. Task 510B2 seam closure is not authorized by this result. After a future snapshot classification A, seam closure scope is deterministic persistence teardown, smoke twice, presentation/typed regressions, feature graphs, production rejection, full workspace/Desktop, Tauri 47, HostExplicit 11, metadata, and authorized commit/push/exact-head CI. Task 510B runtime certification and gpt-6.1-sol/Tool turns remain paused.

Reference only: [Task 510B2L](2026-10-04-task-510b2l-snapshot-compilation-recovery-and-validation.md) corrected the two bounded compilation defects and validated the isolated snapshot, classification A. This Task 510B2K failed-gate evidence and classification remain unchanged.
