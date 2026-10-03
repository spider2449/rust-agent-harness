# Task 506A — absolute-path fixture and validation recovery

Reference-only follow-up: [Task 506B runtime identity audit and validation](2026-10-03-task-506b-codex-runtime-identity-expectation-alignment.md).
The stopped Task 506A evidence below remains unchanged.

Starting HEAD: `fc8d52d631400564083c66c277c1130efed5e3c1`.
Existing dirty Task 506 implementation is preserved.

The default-selection fixture used relative `not-invoked.exe`, rejected by
Codex factory validation. Replace only that argument with deterministic Windows
`PathBuf::from(r"C:\rah-test-fixtures\not-invoked.exe")`. The test validates the
factory but does not create a runtime or invoke this placeholder.

Resume the exact focused test first; stop on failure. Then disabled build/tests,
dependency graph and startup smoke, default regressions and all prescribed gates.
No production correction, feature redesign, authority change or Task 507 work.
ADR-B: existing ADR 0033/current ADRs remain sufficient.

## Validation and required stop

**E — LATER DETERMINISTIC VALIDATION FAILED**

Executed with `CARGO_TARGET_DIR=F:/temp/rah-task504-target`:

```powershell
cargo test -p rah-desktop --bin rah-desktop task506
```

Log: `F:/temp/task506a-default-focused.log`. Native Cargo exit: 101.
Compilation completed in 14.88s. Result: **0 passed; 1 failed; 353 filtered out**.
The absolute executable argument was accepted: configured factory construction
and `factory.validate().unwrap()` completed. The next assertion failed at
`runtime_selection.rs:80`:

```text
left: Some("codex-cli 0.157.1")
right: Some("0.157.1")
```

This is another failure, so the explicit first-validation stop rule applies.
No version assertion correction or production change was made. No test rerun.

Disabled build command intended but NOT RUN:
`cargo build -p rah-desktop --bin rah-desktop --no-default-features`.
Disabled tests/checks, dependency graph re-proof, no-CLI startup/smoke and
no-provider execution proof: NOT RUN in Task 506A.
Default regression, full workspace/Desktop gates, frontend/Tauri/metadata and
static/executable HostExplicit certification: NOT RUN in Task 506A.
Historical Task 506 evidence remains in its plan and does not certify this patch.
HostExplicit = 11 remains the required invariant, not a newly executed result.
ADR-B remains sufficient. Authority/security production code unchanged by 506A.

No classification A, commit, push, exact-head CI or clean-worktree claim.
HEAD remains the starting SHA; Task 506 WIP and this fixture correction are dirty
and preserved. Task 507 was not implemented or authorized by this result.
