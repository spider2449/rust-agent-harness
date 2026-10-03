# Task 506 ? Codex-optional Desktop build composition

## Task 506A recovery result

Task 506A corrected only the relative executable fixture to
`std::path::PathBuf::from(r"C:\rah-test-fixtures\not-invoked.exe")`.
The exact `cargo test -p rah-desktop --bin rah-desktop task506` rerun accepted
the absolute path and then failed at the configured-version assertion:
actual `Some("codex-cli 0.157.1")`, expected `Some("0.157.1")`.
Result: 0 passed / 1 failed / 353 filtered; native Cargo exit 101;
log `F:/temp/task506a-default-focused.log`.
Classification **E — LATER DETERMINISTIC VALIDATION FAILED**. Per the explicit
Task 506A stop rule, no stacked fix, further validation, commit or push occurred.
See the Task 506A recovery plan for the current evidence and missing gates.
The original stopped Task 506 evidence below is preserved as history.

Starting HEAD: `fc8d52d631400564083c66c277c1130efed5e3c1` (clean); supplied CI `37110123418` PASS.

## Audit and design

Workspace resolver 3; workspace, Desktop, rah-runtime and rah-runtime-codex had no
features/default features. Desktop's Windows Codex path dependency was unconditional.
rah-runtime has no Codex edge; Codex depends inward on runtime/protocol/tools.
No other Desktop dependency pulls Codex in. Existing transitive features are preserved.

New default `provider-codex = ["dep:rah-runtime-codex"]`; optional Windows dependency.
Ordinary builds retain Codex. Disabled Windows command:
`cargo build -p rah-desktop --bin rah-desktop --no-default-features`.
The workspace retains the independently buildable Codex crate.

C1: model translation, configured factory, prepared connection/production composition.
C2: codex_baseline executable selection, version/SHA/schema certification, saved artifacts.
C3: closed typed error mapping and retained frontend/IPC metadata. C4: main_tests,
model_preflight and runtime_composition typed probes and baseline fixtures.
C5: none found. Runtime storage is neutral DesktopRuntime; executable source is
presentation metadata only, never executable ownership or authority.

Private codex_composition contains admission/connection composition and factory
construction, compiled only with provider-codex. runtime_composition keeps neutral
preflight, conversation binding, host Tool port and lifecycle available in both modes.
Static selection uses Codex by default; disabled Connect returns RuntimeAdapterUnavailable
before admission/activation. No resolver, PATH search, download or fallback exists.
Future provider features may coexist; host-owned explicit static startup selection
chooses one compiled factory, retains Codex default and fails closed for unavailable
selection. No mutual exclusion rule or speculative provider-openai feature.

Codex tests retained and individually gated; neutral tests remain compiled. IPC,
permissions and saved preferences retained. Frontend adds only neutral error text.
Build script/permissions/packaging do not require Codex executables/resources.
No alternate adapter, keys, live network/provider calls, version/tag/release work.

ADR-B: ADR 0033/current ADRs sufficient. No public contract/dependency direction or
Tool/profile/repository/lease/mutation/remembered-workspace authority expansion.
HostExplicit must remain 11. Version remains 0.33.0.

## Validation and required stop

**E ? DETERMINISTIC VALIDATION FAILED**

The patch is unfinished and preserved. No classification A, operational optionality,
full regression, commit, push, new exact-head CI or clean-worktree claim.

Latest source default command (CARGO_TARGET_DIR=F:/temp/rah-task504-target):

```powershell
cargo test -p rah-desktop --bin rah-desktop task506
```

Log: `F:/temp/task506-default-focused3.log`.
Compilation finished successfully in 1m21s. Exact test output:

```text
running 1 test
runtime_selection::tests::task506_default_selects_configured_codex_factory_without_invocation ... FAILED
panicked at crates\rah-desktop\src\runtime_selection.rs:78:28:
called Result::unwrap() on an Err value: RuntimeFailure {
  diagnostic: RuntimeDiagnostic { operation: Connection, kind: Protocol, rpc_code: None }, ..
}
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 353 filtered out
```

Cause established directly: the new test supplies relative `not-invoked.exe`.
Existing CodexFactory::validate rejects non-absolute executables at
crates/rah-runtime-codex/src/experimental.rs:78-85. No create()/CLI operation occurs
in this test. This is an incorrect test input, not evidence of a production adapter
regression. User instruction 35 requires stopping after deterministic failure;
no correction or repeat run was made after this failure.
The shell wrapper returned 0 after printing the log; native Cargo exit code was
not separately saved. The FAILED summary is authoritative failure evidence.

Earlier development evidence (not final-source certification):

- Initial `cargo check -p rah-desktop --no-default-features` finished successfully;
  log F:/temp/task506-disabled-check.log, with unused/dead-code warnings.
- `cargo test -p rah-desktop --bin rah-desktop --no-default-features --no-run`
  compiled the earlier patch successfully; F:/temp/task506-disabled-tests-compile3.log,
  38 warnings, executable F:/temp/rah-task504-target/debug/deps/rah_desktop-36b459300ac83e7d.exe.
  Disabled tests have **not run**, and later source has not been disabled-mode checked.
- Earlier compiler errors during module/test extraction are preserved in
  task506-disabled-tests-compile.log/compile2.log and default-focused.log/focused2.log.
- `cargo fmt` executed. `cargo fmt --check` full gate not run.
- `git diff --check` passed before final report and is repeated at stop closure.

Dependency graph commands executed successfully on Windows:

```powershell
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc -e normal,build
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc --no-default-features -e normal,build
cargo tree -p rah-desktop --target x86_64-pc-windows-msvc --no-default-features -e normal,build,dev
```

Full outputs preserved at F:/temp/rah-task506-evidence/default-tree.txt,
disabled-tree.txt and disabled-test-tree.txt. Default contains
`rah-runtime-codex v0.33.0`; both disabled graphs contain zero such entries.
This proves the resolved Desktop dependency edge is excluded; it does not prove
an isolated final binary/link artifact. No binary/link inspection or cargo metadata
closure was performed. Workspace membership/lockfile presence is independent.

`node crates/rah-desktop/tauri_permission_test.js` PASS:
47 runtime, 47 manifest, 47 generated, 47 default allows, 47 frontend commands.
Other frontend/static suites and canonical Windows Desktop gate NOT RUN.
Build-script/Tauri packaging audit found no required Codex resource or environment
probe; no build.rs or permission/resource declaration changed.

No-provider implementation returns RuntimeAdapterUnavailable and leaves connection
in Error/non-ready without a configured adapter. A disabled-only headless startup
smoke flag and neutral test are implemented but **unexecuted**. No process-observed
GUI/headless startup or no-CLI-invocation proof is claimed. No provider network calls.

Task 499 model-preflight, Task 503 adapter conformance, Task 504 Desktop neutral
regressions, full default workspace check/test/clippy, disabled test execution/build,
canonical Windows Desktop, metadata and executable HostExplicit gates NOT RUN.
Default focused counts: 0 passed / 1 failed / 353 filtered. Workspace counts unavailable.
Disabled runtime/test counts unavailable (compile-only earlier development evidence).

HostExplicit static audit: unchanged host_kind match at
crates/rah-desktop/src/host_invocation.rs:523-537 has exactly 11 names. No authorization,
ToolRegistry, profile, repository selection, leases, mutation uncertainty or remembered
workspace semantics were edited. Executable authority certification remains pending.
ADR-B remains sufficient; no new ADR or capability selected.

Remaining concrete references are feature-contained codex_baseline/codex_composition,
selected version/error branches in runtime_selection and gated Codex test probes in
main_tests/model_preflight/runtime_composition. Generic lifecycle and effective-authority
source metadata use RuntimeArtifactSource. Legacy Codex IPC/status/presentation and
configuration labels remain; this task does not redesign provider UX.

## Git state and next action

HEAD remains `fc8d52d631400564083c66c277c1130efed5e3c1`. No commit or push. Supplied
starting CI 37110123418 is historical checkpoint evidence, not validation of this patch.
Eight tracked Task 506 files modified; two new private source modules and this plan
untracked. Worktree intentionally dirty, no staged files. No tag/release/version bump.
Only report/evidence recording followed the failure; implementation is frozen.

Immediate recommendation: a narrow authorized Task 506 follow-up to correct the new
test's placeholder to an absolute uninvoked path, then run stable-source focused gates
and all remaining prescribed validation before classification A/publication. Task 507
is **not authorized by this result**.

## Exact Task 507 handoff

Native OpenAI adapter core plus deterministic conformance in an isolated adapter crate,
using existing neutral contracts, bounded Responses/SSE/function loop, typed sanitized
failures, owned cancellation and host Tool port. No Desktop UX, paid live inference,
default change or automatic implementation. Task 508 owns production composition
and Codex-free operational proof.
