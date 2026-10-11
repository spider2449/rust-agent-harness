# Task 504D — Frozen-source validation restart

Starting HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`.
Task 504C G1 authorizes restart without product correction. Task 504 WIP is preserved dirty and uncommitted. Initial git status --short and git diff --check executed; diff check exit 0 (line-ending warnings only).

Evidence: `F:/temp/rah-task504d-evidence`.
Task 504B capture procedure, instrumentation patch and scripts are available in `F:/temp/rah-task504b-evidence`. Instrumentation was restored in 504B and is absent from the frozen source. This limits lossless typed failure evidence from an uninstrumented invocation; no diagnostics are claimed that this invocation does not emit. No instrumentation or product source edits are authorized here.

## Frozen SHA256

| File | SHA256 |
| --- | --- |
| crates/rah-desktop/src/main.rs | `773ee47c89de8f13637d495729f4b399e8559a73f43fe99233bf9f4340fee76b` |
| crates/rah-desktop/src/main_tests.rs | `618ddfbe95d11205e328f31a03d34277c320bd0586df8a5909ce1c7be964f36c` |
| crates/rah-desktop/src/runtime_composition.rs | `d9bbf44962aa10c8c5d7e2419c015d4cc467eca2f090510590859c36a71672e4` |
| crates/rah-runtime-codex/src/experimental.rs | `2c0454b045bdc32485a256ab9b02d6d2839a5e8ce1097fed88ee48b5e8f3f8d1` |
| crates/rah-runtime-codex/src/experimental/tests.rs | `f59b6cf238957a5b029649f220c40581483269022614ac1a77cdfc2fe7adc39a` |
| crates/rah-runtime-codex/src/runtime.rs | `c6da1776f4c8136c0ca6e9035d86dd154291655249ee71f4976fcf80cc967039` |
| crates/rah-runtime/src/experimental.rs | `f006bc15c96380661b6a7385624145bead09e842643afc88cbb16a5aad277946` |
| crates/rah-runtime/src/experimental_host.rs | `41d14b70d8d39c265f43cbdadd2736d78194222767701bcef3a3aa0341bda0de` |
| docs/adr/0033-neutral-runtime-composition-and-host-tool-lifetime.md | `5bd930e3cab70b4b215d1bb08a71d5fd601b4665fa0bf8d9927425503baac11e` |

## Phase A — PASS

Command: `cargo test -p rah-runtime -p rah-runtime-codex`.
CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR: `F:/temp/rah-task504-target`.
Exit 0. Complete result groups: 8, 6, 1, 3, 102, 6, 11 passed;
all have zero failures; Codex library has 1 ignored. Both doctest groups:
0 passed, 0 failed, 0 ignored. Aggregate: **137 passed, 0 failed, 1 ignored**.
All three historical bridge failures passed; none recurred. No retry/matrix.
Log: `phase-a.log`; status: `phase-a.exit` in the evidence directory.
Task 503 coverage retained in this complete run: six experimental adapter tests
plus legacy discovery/runtime/bridge tests passed, covering discovery,
conversation, Tool correlation, cancellation, shutdown, stale handles and typed
errors. This is adapter conformance evidence, not migrated Desktop certification.

## Phase B — deterministic source compilation failure

Fixture preparation: `scripts/windows-desktop-test-gate.ps1 -TargetDirectory
'F:/temp/rah-task504-target' -OutputDirectory
'F:/temp/rah-task504d-evidence/prepare' -PrepareOnly`: PASS, exit 0.

Exact focused command:

```powershell
cargo test -p rah-desktop --bin rah-desktop -- task504 model_preflight::tests:: desktop_model_selection_maps_only_closed_provider_choices connected_repository_selection_rejects_direct_activation_without_changing_authority local_runtime_failures_project_closed_frontend_codes activation_publication_requires_repository_model_profile_and_connection_currentness
```

Exit **101**. **No tests executed**; passed/failed/ignored test counts unavailable.
Log: `F:/temp/rah-task504d-evidence/phase-b.log` (complete UTF-16LE Cargo output).
Four source diagnostics:

| Code | Location | Exact mismatch |
| --- | --- | --- |
| E0308 | crates/rah-desktop/src/main.rs:8513 | prepared.executable is OsString; configured_codex_factory requires PathBuf |
| E0308 | crates/rah-desktop/src/main_tests.rs:6316 | same OsString / PathBuf mismatch |
| E0599 | crates/rah-desktop/src/main_tests.rs:6498 | neutral experimental::TurnHandle has no into_events method |
| E0308 | crates/rah-desktop/src/main_tests.rs:6812 | same OsString / PathBuf mismatch |

These are Rust type/API errors in frozen migration source, not non-product
compiler/linker infrastructure failure. Stopped immediately after this command;
no retry, correction or subsequent product gate ran. Historical bridge failure
recurrence diagnostics were not triggered because those tests passed.

## Remaining required validation — NOT RUN after stop

| Requirement | Result |
| --- | --- |
| Canonical full Desktop gate and counts | NOT RUN |
| Workspace fmt/check/test/Clippy and counts | NOT RUN |
| Frontend/static tests | NOT RUN |
| Tauri permission inventory | NOT RUN |
| Workspace metadata sanity / v0.33.0 inventory | NOT RUN; compilation labels are not metadata certification |
| Task 499 advertised/absent/default production preflight | NOT RUN; Desktop tests never started |
| Production Connect, model selection and ready publication | NOT RUN; no model used |
| Ordinary production turn | NOT RUN |
| Production Tool round trip | NOT RUN |
| Disconnect/revocation/shutdown | NOT RUN |
| Production absent-model case | NOT RUN |
| Connected/disconnected repository switching and stale Tool refusal | NOT RUN |
| Migrated Desktop typed-error projection | NOT RUN |
| HostExplicit executable verification | NOT RUN |

Static HostInvocationKind inventory remains exactly **11**: FsRead, RepoFileInfo,
RepoStatus, RepoDiff, RepoDiffStaged, RepoCreateBranch, RepoPatch, RepoEditFiles,
RepoCreateFile, RepoDeleteFile, RepoRenameFile. This read-only observation does
not replace executable verification.

## Direct Desktop Codex coupling audit

| References | Classification |
| --- | --- |
| main.rs config/provider imports and PreparedCodexConnection | composition/configuration only |
| runtime_composition.rs CodexFactory/config | composition/configuration only |
| codex_baseline.rs and preferred-version use | artifact certification only |
| main.rs CodexAdapterError downcast and frontend_error mapping | closed error presentation |
| main_tests.rs concrete runtime probes; model_preflight.rs/runtime_composition.rs fixture errors | historical/test-only |

No unexpected concrete production lifecycle storage found in this read-only
audit. Compilation failure prevents certification of production behavior.

## Freeze, authority and closure

Eight of eight migration source SHA256 hashes and ADR 0033 match their initial
values after both invocations (`source-stability.json`). **ADR 0033 unchanged**.
No product source, dependency, permission, Tool, authority, dispatch or policy
was changed during Task 504D. No adapter authority expansion was introduced by
this validation task. ToolRegistry, repository leases/switching, Trusted Profiles,
permissions, uncertainty and remembered-workspace semantics were not dynamically
certified in Desktop because validation stopped before tests started.

Only this report and the Task 504 report append were changed by Task 504D.
Closure git status --short, git diff --stat and git diff --check executed;
diff check exit 0 (line-ending warnings only). All migration WIP is preserved.
HEAD remains `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`.
No staging, commit, push, new CI, tag, release or version bump. No remote writes.
Final worktree remains dirty/uncommitted; it is not clean.

**B — DETERMINISTIC VALIDATION FAILED**

Task 504 remains incomplete. Recommend a separately authorized narrow Task 504E
correction for the three executable-path type mismatches and the stale test
TurnHandle API call, followed by a newly frozen validation restart. No correction
or alternate-provider work is started by this report.

## Task 504E reference

[Task 504E](2026-10-03-task-504e-desktop-neutral-migration-compile-corrections.md)
records the three lossless executable PathBuf conversions and use of the neutral
owned events field. Its exact Phase B invocation exited 101 before tests ran,
exposing 11 RuntimeEvent/AgentEvent consumer mismatches. Classification E;
no further correction or validation followed. This reference does not replace
Task 504D's historical stop evidence or its preserved Phase A PASS.
