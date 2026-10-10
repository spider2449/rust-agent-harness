# Task 524C - Correct fixture formatting and launch Windows RAH

Starting HEAD: 80fc345b1ff63e7f7c787abf6c15b785bee5e6e0. Preserve four-file WIP, all historical reports and private evidence.

1. Capture pre-edit diff/hashes and inspect recorded rustfmt finding. Apply Desktop rustfmt; verify formatting-only change. Require new all-workspace fmt check exit 0.
2. Compile Desktop tests; execute actual registered Task 524 fixtures and existing direct/host fs.read and path/authority tests. Run all frontend regression suites. Stop first genuine failure with exact diagnostics.
3. Identify and normally close old Desktop, verify exit; release-build new executable and record source/build/hash provenance.
4. Launch exact new executable visibly; verify PID/path/hash/window and existing llama.cpp availability without submitting prompts. Leave running at D - READY_FOR_HUMAN_ACCEPTANCE.
5. Await human read-only fs.read/repo.list and naturally occurring failed-turn Activity acceptance. No invented backend proof.
6. Only after acceptance: remaining Desktop/workspace/Clippy/Tauri/diff gates, exact four-file review, commit/push and exact-head CI. No release/tag/version/dependency/authority change; HostExplicit stays 11. Task 525 excluded.

Evidence: target/task524c/private/.

## B - TEST BLOCKER

Starting four-file diff and hashes retained privately. `cargo fmt -p rah-desktop`: exit 0; only the known registry declaration became one line. Other three hashes unchanged, typed results/assertions preserved. `cargo fmt --all -- --check`: exit 0.

`cargo test -p rah-desktop --bin rah-desktop --no-run`: exit 0; E0308/E0277 absent. Registered test inventory retained.

First focused command `cargo test -p rah-desktop --bin rah-desktop task524 -- --nocapture`: exit 101. Two passed: tests::task524_repository_fs_read_direct_and_host_dispatch and tests::task524a_unresolved_activity_is_correlated_and_drained_once. tests::task524a_failed_host_dispatch_closes_activity_without_claiming_a_result failed at main_tests.rs:19296:18: unexpected host lifecycle: started. Full stdout/stderr and retained-process exit metadata preserved in target/task524c/private/desktop-fixtures.*. Lifecycle root cause and live Tool/provider defect not established.

Stopped immediately; no retries, further source correction or assertion weakening. Remaining fs.read bounds/authority tests, frontend regressions, release build, new launch, human acceptance and final integration gates NOT RUN. Previous frontend PASS is historical only.

Old RAH was observed at PID 16032 and target/release/rah-desktop.exe before fixtures; final read-only check found PID 16032 absent. No close/kill action was issued, and cause of exit is unclassified. No new executable or visible live RAH window is claimed. Existing executable is historical, not updated candidate proof.

Four-file WIP and all historical/private evidence preserved. HEAD remains 80fc345b1ff63e7f7c787abf6c15b785bee5e6e0. No commit/push/CI, dependency/ADR/authority/version/tag/release changes; HostExplicit remains unchanged at 11. Task 525 excluded. Next task: diagnose this exact fixture lifecycle mismatch before an authorized correction/new validation attempt.
