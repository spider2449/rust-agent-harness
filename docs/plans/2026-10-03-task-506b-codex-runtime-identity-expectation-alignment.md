# Task 506B — Codex runtime identity expectation alignment

Starting HEAD: `fc8d52d631400564083c66c277c1130efed5e3c1`.
Task 506 dirty, uncommitted WIP and Task 506A absolute fixture are preserved.

## Semantic audit and correction

Observed assertion: actual `Some("codex-cli 0.157.1")`, expected
`Some("0.157.1")`, at runtime_selection.rs:80. The assertion was stale.

The source is `rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION: &str`,
whose value is the exact certified CLI identity `codex-cli 0.157.1`.
`runtime_selection::configured_version() -> Option<&'static str>` wraps it
unchanged. There is no parser or normalization at this mapping.
`current_app_status` stores it in serialized `AppStatus.codex_version` on
Connected; `ConnectionResult::connected` stores it in serialized `version`.
It is configured certified identity/presentation metadata, not a parsed semver
or a newly measured executable identity.

Executable origin: process.rs `verify_version` runs fixed `--version`, decodes
stdout with `String::from_utf8_lossy`, trims surrounding whitespace and passes
the resulting String to `check_version`. Successful exit and exact membership
in `CURRENT_CERTIFIED_CODEX_VERSIONS` are mandatory. No prefix removal or
semver range occurs. Captured schema `SchemaContract.codex_version: String`
must also belong to that exact set. Runtime tests use the preferred constant;
`exact_version_is_required` expressly rejects bare `0.157.1`, extra suffixes,
unknown and historical versions. Current live Desktop assertions use the
preferred constant. Historical ignored probes retain full `codex-cli 0.149.0`
identities; they do not define current admission and are not changed.

Baseline storage separately distinguishes manifest `version` (numeric storage
label) from `reported_version` (full exact identity). `supported_baseline_version`
removes the fixed prefix and validates three numeric components only for the
baseline directory/manifest layout. `verify_baseline` checks both fields and
the executable's full reported identity independently. This is not a general
semantic-version parser and does not normalize the asserted Desktop field.

Correction: only the new default-selection assertion now expects
`Some(rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION)`, exact equality.
Production mapping, admission, model preflight, neutral contracts, frontend,
Tool authority, ADR 0030 and ADR 0033 remain unchanged by Task 506B.
ADR-B: existing decisions suffice; no new ADR. No Task 507/OpenAI implementation.

## Validation sequence

Run exact focused `cargo test -p rah-desktop --bin rah-desktop task506` first;
stop on another failure. If PASS, disabled build, disabled tests, dependency
graph and no-CLI smoke, then default regressions and all prescribed workspace,
Desktop/frontend/Tauri/metadata/HostExplicit gates serially. Record native exits,
counts and logs. No source edits while validation runs. Only classification A
permits commit, normal GitHub master push and natural exact-head CI closure.

## Executed results and required stop

All Cargo commands ran serially on frozen source. Native exits are recorded
separately from PowerShell's stderr `NativeCommandError` formatting.

Default target: `CARGO_TARGET_DIR=F:/temp/rah-task504-target`.
Disabled target: `CARGO_TARGET_DIR=F:/temp/rah-task506-disabled-target`.
Full Desktop runs also set `RAH_TEST_TARGET_DIR` equal to their Cargo target.

| Check | Result | Evidence under `F:/temp/` |
| --- | --- | --- |
| Exact `cargo test -p rah-desktop --bin rah-desktop task506` | exit 0; 1 passed, 0 failed, 353 filtered | task506b-default-focused.log |
| `cargo build -p rah-desktop --bin rah-desktop --no-default-features` | exit 0; 34 warnings retained | task506b-disabled-build.log |
| Disabled `cargo test -p rah-desktop --bin rah-desktop --no-default-features task506` | exit 0; 1 passed, 0 failed, 317 filtered | task506b-disabled-focused.log |
| Canonical helper preparation, disabled target, `-PrepareOnly` | PASS, exit 0 | task506b-disabled-helper-gate/20261003-172516-030-78ec5434662d4790b0e1ff71ee0deca2 |
| Full `cargo test -p rah-desktop --bin rah-desktop --no-default-features` | exit 0; 307 passed, 0 failed, 11 ignored; 251.38s | task506b-disabled-tests.log |
| Default `cargo test -p rah-desktop --bin rah-desktop model_preflight::tests` | exit 0; 4 passed, 0 failed, 350 filtered | task506b-default-regression-1.log |
| Default `cargo test -p rah-desktop --bin rah-desktop runtime_composition::tests` | exit 0; 4 passed, 0 failed, 350 filtered | task506b-default-regression-2.log |
| `cargo test -p rah-runtime-codex --lib` | exit 0; 102 passed, 0 failed, 1 ignored | task506b-default-regression-3.log |
| `cargo fmt --check` | exit 0 | task506b-workspace-1.log |
| `cargo check --workspace` | exit 0; one unused-import warning retained | task506b-workspace-2.log |
| `cargo test --workspace` | exit 0; aggregate 1,047 passed, 0 failed, 24 ignored across 56 result groups | task506b-workspace-3.log |
| Default Desktop within workspace run | 334 passed, 0 failed, 20 ignored; 256.08s | task506b-workspace-3.log |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 101; required stop | task506b-workspace-4.log |
| Closure `git diff --check` | exit 0 | closure tool output |

### Dependency graph and no-CLI evidence

Exact command:
`cargo tree -p rah-desktop --target x86_64-pc-windows-msvc --no-default-features -e normal,build,dev`.
Exit 0; `task506b-disabled-tree.log` contains zero `rah-runtime-codex` entries.
This excludes the adapter from the resolved production/build/dev Desktop graph;
workspace membership and lockfile presence are separate.

Executed the built disabled binary by absolute path with process-local PATH
set to the empty string and restored in `finally`:
`F:/temp/rah-task506-disabled-target/debug/rah-desktop.exe --runtime-composition-smoke`.
Exit 0; `task506b-disabled-smoke.log` records exactly:

```json
{"adapter_available":false,"connection_error":"runtime_adapter_unavailable","passed":true,"runtime_status":"not connected"}
```

The focused disabled test constructs production `DesktopAppState`, verifies
no selected adapter/configured identity, initial NotConnected state, exact
RuntimeAdapterUnavailable result and Error state, no panic, and zero resolver
and runtime-construction counters. Status has no Codex identity. The full
disabled suite also passes inert startup/preference and neutral lifecycle tests.

Structural proof complements execution: main.rs feature-gates both
codex_baseline and codex_composition; the adapter dependency is absent;
disabled selection returns None and the production `connect_codex` command
routes directly to `connect_unavailable`. The smoke calls that same bounded
state/selection/unavailable helper and returns before Tauri startup. Thus this
headless path performs no Codex CLI invocation, Codex PATH discovery or Codex
executable admission. No GUI startup/process-observer certification is claimed.

### Default behavior, authority and ADR

Cargo.toml retains `default = ["provider-codex"]`. The focused default test
selects Codex and validates its configured factory without invoking the fixture.
Task 499 preflight, Task 503 adapter tests/conformance and Task 504 neutral
Desktop lifecycle pass as above. Exact-version admission rejection tests pass;
the adapter source and schema fixture have no diff against starting HEAD.
No live provider/model certification or certification-policy update occurred.

HostExplicit static `host_kind` routes exactly 11 names. Executable
`host_invocation::tests::host_allowlist_is_exact` passes in both full disabled
and default workspace Desktop runs. No Tool authority changed. ADR-B;
ADR 0030 and ADR 0033 remain unchanged and no new ADR is required.

### Clippy failure and unexecuted gates

Stable-source failure diagnostics:

- main.rs:8: unused import `codex_composition::*` (production binary).
- codex_composition.rs:69-70: `clippy::empty_line_after_doc_comments`.
- main.rs:7991-7992: `clippy::empty_line_after_doc_comments`.
- main.rs:8131-8132: `clippy::empty_line_after_doc_comments`.

Production binary reports four errors; test binary reports three. Preserve the
full log and source; no stacked fix or rerun occurred. Later canonical Windows
Desktop test gate, frontend/static suites, Tauri permission inventory, Cargo
metadata sanity and standalone HostExplicit gate were NOT RUN in Task 506B.
Helper-only preparation is not a canonical Desktop test-gate PASS. Historical
Task 506 inventory evidence does not replace current execution.

## Final classification and Git state

**B — TEST EXPECTATION WAS STALE BUT LATER VALIDATION FAILED**

Identity alignment is complete; the remaining Clippy defect prevents Task 506
completion/publication. HEAD remains
`fc8d52d631400564083c66c277c1130efed5e3c1`. No commit, staging, push, new exact-head
CI, tag, release or version bump. Worktree is intentionally dirty with preserved
Task 506/506A WIP, the one assertion correction and this report/reference note.
Task 507 is not authorized by this classification and was not implemented.

Next task: separately authorize narrow disposition of these four lint sites,
then resume the remaining validation gates on frozen source. Do not infer
full-gate success or production GUI certification from the passing tests here.


## Task 506C reference

See [Task 506C](2026-10-03-task-506c-lint-correction-and-final-validation.md) for the subsequent lint-only attempt and required stop. This report's historical evidence remains unchanged.
