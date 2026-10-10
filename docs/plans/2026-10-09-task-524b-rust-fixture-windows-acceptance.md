# Task 524B - Rust fixture correction and Windows Desktop acceptance

Starting HEAD: 80fc345b1ff63e7f7c787abf6c15b785bee5e6e0. Preserve four-file WIP, historical reports and private evidence. No publication before human acceptance; no Task 525.

1. Correct fixture only: E0308 at main_tests.rs:19247 expects Arc<ToolRegistry>, receives Arc<Arc<ToolRegistry>>. desktop_tool_registry returns Result<Arc<ToolRegistry>, ToolError>; HostToolScope::new accepts Arc<ToolRegistry>. Remove redundant Arc::new.
2. E0277 at main_tests.rs:19305-19310: listen_for_test_event returns (Arc<Mutex<Vec<String>>>, EventId); payload is String, JSON field assertions require serde_json::Value. Parse with serde_json::from_str and assert successful parsing, preserve assertions.
3. Run fmt, fixture compilation, direct/host fs.read, frontend, affected runtime/Desktop, workspace and strict Clippy gates sequentially. Stop first genuine failure, preserve logs in target/task524b/private.
4. Inspect Activity lifecycle coverage and terminal semantics. Build production only after all gates; normally close exact old owned instance, verify exit, launch hashed new executable visibly and leave open.
5. Report READY_FOR_HUMAN_ACCEPTANCE and request read-only A/B/C tests. Commit/push/exact-head CI only after human acceptance. No dependency, ADR, permission, HostExplicit 11, version, tag or release changes.

## B - validation stopped at formatting gate

Two fixture corrections applied; production Rust interfaces untouched. First required command cargo fmt --all -- --check returned exit 1: main_tests.rs:19242 registry binding must be formatted on one line. Exact stdout/stderr and retained-process exit metadata preserved in target/task524b/private/fmt-all.*. No retry or further source correction after this failure.

Fixture compilation/execution, frontend, runtime/Desktop, workspace, Clippy and production build NOT RUN for this candidate. Prior green results do not certify this patch. No process closure or launch; old Desktop remains untouched. No new human acceptance, commit/push/CI, version/tag/release changes. HEAD remains starting checkpoint. Next authorized work must correct the recorded formatting failure and resume gates.
