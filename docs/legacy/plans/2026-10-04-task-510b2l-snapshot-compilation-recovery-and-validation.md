# Task 510B2L — Snapshot compilation recovery and validation

Classification: **A — ISOLATED EXACT-BYTE CERTIFICATION SNAPSHOT VALIDATED**.

Starting and ending HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserved Task 510B–510B2K WIP and failed evidence. No commit, push, CI, tag,
release, admission change, or Task 510B resumption.

## Bounded correction and diff guard

Only implementation file changed this task:
`crates/rah-runtime-codex/src/certification_support.rs`.

U1 audit found the private, schema-specific `process.rs::schema_temp_dir`
using PID/counter; no reusable repository directory-ownership helper was found.
U2 found no appropriate existing normal temp-path dependency in this crate.
U3 follows the existing `bridge_tests.rs::TestDirectory` naming convention:
PID, epoch nanoseconds, and atomic counter, followed by `std::fs::create_dir`.
Collision fails without overwrite or retry. This is certification-local naming,
not a public identifier contract. No general naming framework was introduced.
Dev-only uuid remains dev-only; no dependency or manifest changes this task.
Cargo.lock already contained prior WIP and was unchanged relative to task start:
SHA256 `9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe`.

Renamed `SnapshotMismatch.source` and `.snapshot` to `source_measurement` and
`snapshot_measurement`, including construction. Neither diagnostic field is an
Error source. Public Display remains `certification snapshot identity mismatch`.
The real RuntimeFailure → CodexAdapterError → CertificationVerificationError
source chain is unchanged and exercised by focused tests.

Compared against preserved pre-edit source at
`F:/temp/task510b2l-evidence/certification_support.before.rs` before compilation
and again after formatting. Only these two corrections occurred. No snapshot
algorithm, cleanup, lifecycle, admission, baseline, frontend, or picker edit.
The opened source handle still supplies every buffer to both SHA and snapshot
writer; no measure(path)/File::copy(path) replacement. Writer flush/sync/close
precedes independent repeated snapshot measurements.

## Serial gates and deterministic coverage

Source was frozen during each validation command. Both target variables were
`F:/temp/rah-task510b2g-target`.

Exact failed Task 510B2K command rerun:
`cargo test -p rah-runtime-codex --features certification-harness certification_support::tests -- --nocapture`

PASS, exit 0: **5 passed, 0 failed, 0 ignored, 103 filtered out**.
Architecture/live-contract integration targets each selected 0 tests (6 and 11
filtered respectively). Log: `F:/temp/task510b2l-evidence/focused.log`.

Five executed tests: complete stable fixture measurement; exact-byte snapshot
and rejections; exact descriptor rejection; exact factory/version/production
separation; wrong hash cannot probe/start. One snapshot test covers stable
fixture bytes, complete source read, exact destination content, matching SHA,
distinct Windows file IDs, existing-destination AlreadyExists rejection,
wrong expected SHA rejection, snapshot descriptor path, and sanitized public
error text. Same-handle read/hash/write is directly confirmed by source audit.
Typed-cause tests exercise missing artifact, hash mismatch and version mismatch
through the genuine source chain and public diagnostic redaction.

Then ran the existing Task 510B2K campaign exactly once:
`cargo test -p rah-desktop --features certification-harness --test codex-certification certification_tests::task510b2k_snapshot_controls -- --ignored --exact --nocapture --test-threads=1`

PASS, exit 0: **1 passed, 0 failed, 0 ignored, 380 filtered out**;
test duration 126.26s. Log:
`F:/temp/task510b2l-evidence/snapshot-controls.log`.
No fallback, second snapshot, or live retry.

## Live source and independently verified snapshot

Source supplied path:
`C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe`.
Canonical source path is the same location with Windows `\\?\` prefix.

Snapshot supplied path:
`F:\Temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex.exe`.
Canonical path:
`\\?\F:\temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex.exe`.

Both SHA256 values exactly equal expected:
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

| Measurement | Source opened handle | Independent readonly snapshot |
| --- | --- | --- |
| Bytes read | 326872368 | 326872368 |
| Pre-read length | 326872368 | 326872368 |
| Post-read length | 326872368 | 326872368 |
| File ID (decimal) | 8162774324656493 | 12947848929378465 |
| File ID (hex) | 0x0000000000000000001d00000000b96d | 0x0000000000000000002e0000000a80a1 |
| Volume serial | 2929214197 | 2114581433 |
| Creation FILETIME | 134354135300330586 | 134355905977089664 |
| Last-write FILETIME | 134354135384901097 | 134355906084729114 |
| Attributes | 32 (Archive) | 33 (Readonly, Archive) |

Successful snapshot construction proves source pre/post handle identity,
length and volume stable, bytes read equal source length, expected source SHA,
and distinct source/snapshot identity. Independent snapshot reads were complete.

PowerShell `Get-FileHash -Algorithm SHA256 -LiteralPath` on this snapshot
returned the expected SHA; child exit 0. Version-only execution of this exact
snapshot returned `codex-cli 0.160.0`, exit 0. The full ArtifactMeasurement
after version execution equaled the initial readonly snapshot measurement.

## Real runtime and Desktop controls

Real Codex adapter app-server handshake/shutdown PASS. `process.rs` logged
the actual Command executable after spawn:
`F:\Temp\rah-codex-certification-snapshot-18440-1791116997708427800-0\codex.exe`,
PID **14272**. Runtime was alive after creation and not alive after shutdown.
Snapshot full measurement remained unchanged after shutdown.

Real adapter + neutral runtime + Desktop backend composition PASS. The same
exact spawn path was logged with PID **15720**. Bounded Connect used
RuntimeDefault, reached Connected with ChatState::Idle, then bounded Disconnect
reached NotConnected and runtime not alive. Both recorded PIDs were absent at
post-gate process inspection. No PATH or npm executable fallback.

The full measurement immediately before Desktop construction (after adapter
shutdown) and after Desktop disconnect was equal to the initial readonly
snapshot: expected SHA, 326872368 bytes and pre/post lengths, snapshot file ID
12947848929378465, volume, timestamps, attributes and canonical path unchanged.
The final logged independent measurement also matched exactly.
Inference **0**; Tool execution **0**. No model selection or turn was run.
Snapshot and Desktop persistence roots were retained; the live campaign has no
remove_dir_all(root) or persistence teardown experiment.

## Isolation, final checks and next scope

Snapshot module/factory selector and spawn evidence remain behind non-default
`certification-harness`. Desktop campaign is feature-gated test-only. Normal
production has no snapshot creation/configuration path or unknown-version
admission. Detailed measurements remain process-local certification evidence;
public error strings contain no paths, hashes or file IDs. Existing Task 498
sanitization remains intact.

Ran `cargo fmt` on unpublished WIP; no additional formatting diff or behavior
change. `cargo fmt --check`: PASS, exit 0.
`git diff --check`: PASS, exit 0. Final status/stat inspected; prior dirty WIP
remains preserved. This task adds this plan and a reference-only 510B2K note.

Production preferred/current certified runtime remains **0.157.1**;
**0.160.0 remains unadmitted / uncertified**. This validates the isolated
artifact execution seam, not full runtime/model/Tool certification.
Dependency/authority impact: none this task. **ADR-B**; ADR 0030 sufficient,
no new ADR. No workspace/Desktop/Tauri/HostExplicit closure was run.

Exact next task: **Task 510B2 seam closure using this preserved verified
snapshot artifact**, with deterministic persistence teardown, smoke twice,
presentation/typed regressions, feature graphs, production rejection, full
workspace/Desktop gates, Tauri 47, HostExplicit 11 and metadata review.
Any commit/push/exact-head CI workflow needs its own explicit authorized
closure scope; none was performed here. Do not resume Task 510B or paid/live
model/Tool turns automatically.
