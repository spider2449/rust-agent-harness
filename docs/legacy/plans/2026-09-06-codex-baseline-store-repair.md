# Task 217A - Codex Baseline Store Repair

## Result

Task 217A: PASS pending the exact-head CI result for the implementation commit.
Task 217 remains INCONCLUSIVE until its Connect phase is resumed after the
certified baseline is repaired.

## Plan

- Keep the current Desktop verifier strict: only manifest v2 and a complete,
  independently verified Codex/code-mode-host pair are accepted.
- Add an explicit host-only `repair` command to `scripts/codex-baseline.ps1`.
- Preserve `save` as creation-only and leave Desktop startup and Connect free of
  downloads or automatic repair.
- Validate the complete requested-version source pair before destination
  mutation, stage a fresh v2 directory, verify it, and use bounded same-volume
  backup replacement for invalid destinations only.
- Extend the isolated PowerShell tests with the legacy Task 217 reproduction,
  idempotence, source rejection, replacement failure, and cleanup cases.
- Repair and verify the real 0.149.0 store, then run the established baseline
  resolver/live readiness gate.

## Authority boundary

Baseline repair is an explicit host/operator maintenance action. It is not a
model Tool, provider authority, Trusted Profile authority, repository authority,
runtime authorization, or new permission. Its optional npm acquisition is
host-only and occurs only when the operator invokes the command.

## Implementation evidence

The script preserves `save` as creation-only and `verify` as strict v2
verification. `repair` classifies the destination internally as `Absent`,
`ValidCurrent`, `LegacyManifest`, `InvalidCurrent`, or `Incomplete`; this is
operator diagnostics only and is not model-visible. A valid current baseline is
idempotent for matching source hashes and refuses a different acquired pair.

Replacement uses a fresh sibling staging directory, complete source-pair copy,
staged verification, destination-to-backup move, staging-to-destination move,
final verification, and backup removal. Bounded retry handles transient
Windows executable scanning locks. If replacement fails after backup movement,
the command attempts bounded restoration and reports recovery as best-effort;
it never force-terminates Codex/Desktop processes.

Source validation requires both native x64 PE files, no reparse points, exact
requested `codex-cli <version>` output from `codex.exe`, and independently
computed SHA-256 values. A legacy v1 manifest is diagnostic evidence only; it
is never edited into v2 and cannot supply the missing code-mode host or hash.

## Deterministic validation

`powershell -NoProfile -ExecutionPolicy Bypass -File
scripts/test-codex-baseline.ps1 -NativeCodex <installed-exact-native-codex>`
passed using the installed 0.153.4 full pair. It covers absent creation,
valid idempotence, the legacy v1 Task 217 reproduction, missing companion,
malformed manifest, wrong source version, non-PE source, invalid source host,
staged verification failure, first and second replacement-move failures,
different-source refusal for a valid destination, wrong destination hash, and
successful-operation cleanup.

The old real store was inspected as `LegacyManifest`: manifest v1, certified
`codex.exe` hash present, and missing `codex-code-mode-host.exe`. Current
`verify 0.149.0` rejected it before repair.

## Real certified recovery

Command:

```powershell
.\scripts\codex-baseline.ps1 repair 0.149.0
.\scripts\codex-baseline.ps1 verify 0.149.0
```

The command used isolated exact npm acquisition of
`@openai/codex@0.149.0-win32-x64`; no manual store edit was used. Final
evidence:

- `manifest_version = 2`;
- `version = 0.149.0`;
- `reported_version = codex-cli 0.149.0`;
- `codex.exe` SHA-256 =
  `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`;
- `codex-code-mode-host.exe` SHA-256 =
  `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`;
- final script verification: PASS;
- no repair staging or backup sibling remained.

`scripts/codex-live-gate.ps1 -Version 0.149.0 -PrepareOnly` also passed with
the expected certified hash, isolated `CODEX_HOME`, zero MCP servers, disabled
plugins/apps/code mode/browser/computer/image-generation features, and the
accepted `gpt-5.6-terra` model configuration. Existing Desktop resolver tests
continued to pass their `CertifiedBaseline` contract cases; no resolver or
PATH-fallback behavior was changed.

## Repository validation

- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; 13 packages, 0.17.0,
  edition 2024.
- `cargo test -p rah-runtime-codex`: isolated failing test rerun PASS (the
  first concurrent run had one unrelated repository-fixture interference).
- `cargo test -p rah-desktop`: 171 passed, 1 known Windows foreign-owner Git
  fixture failure, 2 ignored; the same fixture fails in isolation.
- `cargo test --workspace`: same known Desktop foreign-owner Git fixture
  failure; it was not hidden or changed.
- `git diff --check`: PASS.

No Cargo/dependency/version change, Desktop auto-repair, Connect download,
model-visible operation, new authority, permission, or ADR was added.

## Sequencing and next step

Task 217 evidence checkpoint:

- commit `2c8d347c44c98d5423a640b16e6a555552b4331a`;
- message `docs: record inconclusive trusted profile lifecycle validation`;
- exact-head CI `33999201033`: PASS;
- pushed and clean before Task 217A implementation.

Task 217A implementation files are the baseline script, deterministic test
script, baseline management documentation, and this plan. The next step after
the implementation commit and CI is to resume Task 217 from Connect and prove
Effective MCP/Plugin authority, Forget while connected, normal Disconnect,
shutdown/unlock, and restart after Forget. Task 218 is not started.
