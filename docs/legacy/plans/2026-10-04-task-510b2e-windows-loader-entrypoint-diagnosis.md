# Task 510B2E — Windows loader entrypoint diagnosis

Reference-only follow-up: Task 510B2F applied the bounded M1 test-target
resource-linkage correction and recorded its subsequent classification D after
the exact candidate smoke failed during temporary-workspace cleanup (Windows
error 32). See `2026-10-04-task-510b2f-certification-test-common-controls-v6-manifest.md`.

Starting and final HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.
Preserved Task 510B–510B2D source and reports. No Rust, Cargo, dependency,
linker, build.rs, production behavior, admission or preferred-baseline edits.
Evidence directory: `F:/temp/task510b2e-evidence`.

## Diagnosis procedure and original evidence

Record environment before retry; use a previously nonexistent target; run the
fully qualified presentation filter; on reproduced loader failure inspect direct
launch, imports, exports, manifest and a same-feature sibling. Stop at L3.
Only L1/L2 recovery permits resumed seam validation. No automatic Task 510B resume.

Original log remains `F:/temp/task510b2d-evidence/presentation-test.log`.
SHA-256 before and after diagnosis:
`90800d28faf23f852a0a6525a23dbd62777b50bfe8b8aec8fde90c4b30febcdc`.
Original target was neither cleaned nor deleted.

Original executable:
`F:/temp/rah-task504-target/debug/deps/codex_certification-d52d592a41ace069.exe`.
Size 54,955,008 bytes; timestamp 2026-10-04 17:34:40 local.
SHA-256 `98251dd59cf37445283e48d1bfe5e47aa5d82be86034e71febe5eb76f49c72b9`.
Original command (both target variables `F:/temp/rah-task504-target`):

```powershell
cargo test -p rah-desktop --features certification-harness --test codex-certification certification_frontend_mapping_preserves_private_sources -- --exact --nocapture
```

Original result: linked, then `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`,
signed executable status -1073741511, before tests. Its short exact filter
would not select the fully qualified test even if the executable started.

## Environment recorded before retry

Full PATH, `where.exe` output, shell/OS architecture, target variables and
original artifact metadata: `environment.log`. PATH was not changed before
the first fresh build or direct launch.

- rustc 1.98.1 (48a229cea 2026-09-01), commit
  `48a229ceaefd4985c50990b14116b6d856af0985`, LLVM 22.1.8,
  host x86_64-pc-windows-msvc.
- cargo 1.98.1 (797e8a9bc 2026-08-05).
- rustc/cargo resolve to `F:/rust/.cargo/bin/rustc.exe` and `cargo.exe`.
- Shell and Windows architecture: X64/X64. Initial CARGO_TARGET_DIR and
  RAH_TEST_TARGET_DIR unset. No active VS developer environment; cl/link
  not discoverable via PATH.
- Installed Visual Studio Build Tools 2026 18.9.2 / 18.9.12120.119;
  `C:/Program Files (x86)/Microsoft Visual Studio/18/BuildTools`.
  MSVC 14.51.36231; compiler 19.51.36256 for x64.
  `vswhere.json` and `msvc.log` preserve evidence.
- PATH includes Codex/npm/Copilot, CUDA, System32/Windows, Git, Node,
  Rust, WindowsApps, VS Code and Bun. Full ordered value is in environment.log.
- System32 VC runtimes: VCRUNTIME140.dll and VCRUNTIME140_1.dll,
  file version 14.51.36247.0; search audit in `dll-search.log`.

## Fresh target and direct launch

`F:/temp/rah-task510b2e-target` did not exist before invocation.
Set CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR to this directory for Cargo.
No source changes during compilation or controls.

```powershell
cargo test -p rah-desktop --features certification-harness --test codex-certification certification_tests::certification_frontend_mapping_preserves_private_sources -- --exact --nocapture
```

`fresh-presentation-1.log`: newly compiled/linked, finished in 3m 38s,
then identical `0xc0000139`, recorded exit -1073741511. Tests did not run.
Fresh executable:
`F:/temp/rah-task510b2e-target/debug/deps/codex_certification-d52d592a41ace069.exe`.
Size 54,955,008 bytes; timestamp 2026-10-04 17:43:08 local;
SHA-256 `1b93e712fcad5e50e07e2934bd54c337de6ca8c2d03d21ff11a7b99dba9b3533`.
Metadata in `fresh-executable.log`.

Direct launch of this exact executable with the same fully qualified filter,
`--exact --nocapture`, outside Cargo: -1073741511 (`direct-launch.log`).
Command-local PATH reduced to `C:/Windows/System32;C:/Windows`:
same -1073741511 (`controlled-path-launch.log`). Original PATH restored
in finally. No persistent environment or host configuration correction.
The second same-target PASS confirmation rule is inapplicable: first run failed.

## Native dependency / symbol evidence

MSVC dumpbin /dependents and /imports: `dependents.log`, `imports.log`.
Imported DLL names:
bcrypt.dll, bcryptprimitives.dll, advapi32.dll, ntdll.dll, kernel32.dll,
api-ms-win-core-synch-l1-2-0.dll, ole32.dll, comctl32.dll, shlwapi.dll,
user32.dll, shell32.dll, gdi32.dll, dwmapi.dll, oleaut32.dll, crypt32.dll,
ws2_32.dll, KERNEL32.dll, userenv.dll, VCRUNTIME140.dll, VCRUNTIME140_1.dll,
and api-ms-win-crt-{string,runtime,convert,environment,stdio,heap,utility,
math,time,locale}-l1-1-0.dll.

The integration executable imports `comctl32!TaskDialogIndirect` by name,
alongside RemoveWindowSubclass, DefSubclassProc and SetWindowSubclass.
System32 comctl32.dll (version 5.82) exports the three subclass symbols,
but NOT TaskDialogIndirect (`comctl32-system-exports.log`).
The installed Common Controls v6 DLL exports TaskDialogIndirect (ordinal 345):
`C:/Windows/WinSxS/amd64_microsoft.windows.common-controls_6595b64144ccf1df_6.0.26100.9278_none_3e0d1ba8e3303201/comctl32.dll`
(`comctl32-v6-exports.log`). This identifies the incompatible import/export;
no debugger trace of the failed process's loaded modules was collected.

Windows SDK mt.exe extraction of manifest resource #1 from the integration
executable fails: image contains no resource section
(`presentation-manifest-extract.log`). Therefore it lacks the embedded
Common Controls v6 activation declaration found in the binary sibling.
`where.exe comctl32.dll` finds only `C:/Windows/System32/comctl32.dll`.
No additional comctl32 or VC runtime copies were found in the executable
directory, repository cwd or ordered PATH directories (`dll-search.log`).
This is not evidence of a Node/CUDA/Git PATH shadowing problem.

Actual Desktop build-script output emits:
`cargo:rustc-link-arg-bins=F:/temp/rah-task510b2e-target/debug/build/rah-desktop-fc9803dfaca74452/out/resource.lib`.
See `build-link-directives.log`. Generated resource.rc declares
Microsoft.Windows.Common-Controls version 6.0.0.0. The new Cargo [[test]]
target reuses src/main.rs but does not receive the binary-only resource link.
Thus it imports the existing native API without the necessary activation
manifest. The underlying native dependency already exists in Desktop:
`rfd 0.16.0 -> tauri-plugin-dialog 2.7.2 -> rah-desktop 0.33.0`
(`native-dependency-chain.log`); local rfd source calls TaskDialogIndirect.
Adapter certification-harness is an empty Rust feature; Desktop's feature
enables provider-codex and that adapter feature. The defect is integration
target linkage, not a new native DLL introduced by verification Rust code.

## Same-target / same-feature sibling control

```powershell
cargo test -p rah-desktop --features certification-harness --bin rah-desktop -- --list
```

Same fresh target and cwd, same feature set: linked and started successfully;
380 tests listed, exit 0 (`sibling-build-launch.log`). Sibling executable:
`F:/temp/rah-task510b2e-target/debug/deps/rah_desktop-5b665985f162d0aa.exe`.
Direct existing test under the same System32-only PATH:
`tests::adapter_errors_are_sanitized_for_the_frontend --exact --nocapture`.
PASS: 1 passed / 0 failed / 0 ignored / 379 filtered; exit 0
(`sibling-control.log`). This is a sibling control, not the new regression.

Sibling DLL dependency names exactly match the failing integration target
(`sibling-dependents.log`, `sibling-imports.log`). Its manifest extraction
SUCCEEDS and contains Common Controls 6.0.0.0 (`sibling.manifest`).
No unrelated minimal Rust control or default-feature rebuild was needed:
same-feature sibling success isolates the missing resource linkage.

## L3 and required stop

**L3 — reproducible feature/linkage defect.** Fresh target, controlled PATH,
same failure, exact missing named export identified, healthy same-feature
sibling, and certification integration-target resource omission established.
L1 excluded by fresh failure; L2 PATH shadowing excluded by controlled launch
and matching dependencies; L4 unsupported by healthy fresh sibling.
No source/linker/resource correction or artifact mutation was attempted.
Recommend a separate task for narrowly restoring the certification test
target's Common Controls v6 manifest/resource linkage, then re-enter the
fully qualified presentation regression before any remaining seam gates.

## Resumption gates and nonclaims

Presentation regression: executable does NOT start; no assertion count/PASS.
Typed verification rerun and all-features rerun: NOT RUN after L3 stop.
Prior Task 510B2D report records 3/0/103 filtered and all-features checks PASS;
these are historical evidence, not refreshed validation in this task.
Default-disabled / explicit-enabled feature graphs: not rerun here; prior
510B2D evidence retained. Same-feature sibling is not graph/admission proof.

Installed 0.160.0 production rejection, wrong hash/version/missing path
controls, exact candidate smoke, real adapter/neutral runtime/Desktop
composition, handshake, shutdown and pre/post candidate hash: NOT RUN.
Inference=0 and Tool execution=0 in this diagnosis; no candidate launch.
Workspace gates and canonical Desktop suite: NOT RUN; no aggregate counts.
Only sibling control count 1/0/379 and listing count 380 are current evidence.
Tauri 47/47/47/47/47, metadata 14/all 0.33.0, static/executable HostExplicit
exactly 11: NOT revalidated after stop. Cargo.lock unchanged; no dependency drift
introduced. git diff --check PASS.

No changes to production admission/preferred 0.157.1, ToolRegistry, repository
authority/switching, permissions, Trusted Profiles, leases, mutation uncertainty
or remembered workspace semantics. No product behavior changed in this task.
ADR-B: existing Task 498 / ADR 0030 context retained; no new ADR.

**D — CERTIFICATION FEATURE INTRODUCES A REAL NATIVE LINKAGE DEFECT.**
Precisely: the certification integration target omits Desktop's existing
Common Controls v6 manifest linkage; the feature's Rust verification code
does not itself introduce a new native dependency. Full seam validation remains
incomplete; Codex 0.160.0 is neither admitted nor certified.

No commit, push, exact-head CI, tag, release or version bump. Worktree remains
intentionally dirty with preserved WIP and these documentation changes.
Task 510B remains paused. After separate linkage correction and complete
classification A validation, its permitted resume scope is exact-artifact
schema/deterministic/direct gpt-6.1-sol/Tool round trips/cancellation/live
error-envelope/Desktop turn gates with pre/post hashes. Do not resume
automatically; admission/preferred-baseline changes remain separate work.
