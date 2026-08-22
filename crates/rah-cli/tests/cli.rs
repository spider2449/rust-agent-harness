use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow epoch")
            .as_nanos();
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("rah-cli-profile-{stamp}-{sequence}",));
        fs::create_dir(&path).expect("test directory should be created");
        Self(path)
    }

    fn profile(&self, contents: &str) -> PathBuf {
        let path = self.0.join("profile.json");
        fs::write(&path, contents).expect("profile should be written");
        path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fs_read_profile(workspace: &str) -> String {
    format!(
        r#"{{
            "profile_version": 1,
            "profile_id": "cli-test",
            "resources": {{"repositories": {{"workspace": {{"path": "{workspace}"}}}}}},
            "capabilities": [{{
                "name": "fs.read",
                "enabled": true,
                "permission": "read",
                "workspace": "workspace",
                "max_bytes": 1024
            }}]
        }}"#,
        workspace = workspace.replace('\\', "\\\\")
    )
}

fn rah() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rah"))
}

#[test]
fn doctor_reports_healthy_configuration() {
    let output = rah().arg("doctor").output().expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "RAH doctor: ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn tools_lists_deterministic_registry() {
    let output = rah().arg("tools").output().expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "echo\nfs.read\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_reads_workspace_manifest_through_fs_tool() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CLI crate is nested under workspace/crates");
    let output = rah()
        .current_dir(workspace)
        .env("RUST_LOG", "rah=debug")
        .args([
            "run",
            "read Cargo.toml and report the workspace package information",
        ])
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "workspace version {}, edition 2024\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert!(stderr.contains("tool_name=fs.read"), "stderr: {stderr}");
    assert!(
        stderr.contains("tool execution finished"),
        "stderr: {stderr}"
    );
}

#[test]
fn run_streams_deterministic_text() {
    let output = rah()
        .args(["run", "hello from rah"])
        .output()
        .expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from rah\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_command_writes_stderr_and_fails() {
    let output = rah().arg("unknown").output().expect("rah should launch");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn debug_logging_contains_runtime_correlation_fields() {
    let output = rah()
        .env("RUST_LOG", "rah=debug")
        .args(["run", "trace me"])
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert!(
        stderr.contains("session_id="),
        "expected tracing on stderr; stdout: {}; stderr: {stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(stderr.contains("request_id="));
    assert!(stderr.contains("model_request_id="));
    assert!(stderr.contains("tool_call_id="));
}

#[test]
fn profile_validate_prints_only_redacted_effective_inventory() {
    let directory = TestDirectory::new();
    let profile = directory.profile(&fs_read_profile(&directory.0.to_string_lossy()));
    let output = rah()
        .args(["profile", "validate"])
        .arg(&profile)
        .output()
        .expect("rah should launch");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("profile_version=1"));
    assert!(stdout.contains("profile_id=cli-test"));
    assert!(stdout.contains("source_class=trusted_static_file"));
    assert!(stdout.contains(
        "capability name=fs.read enabled=true registered=true permission=Read resources=workspace validation=validated"
    ));
    assert!(!stdout.contains(&profile.to_string_lossy().to_string()));
    assert!(!stdout.contains(&directory.0.to_string_lossy().to_string()));
    assert!(output.stderr.is_empty());
}

#[test]
fn profile_validate_rejects_invalid_profiles_without_inventory_or_sensitive_values() {
    let directory = TestDirectory::new();
    let secret = "CLI_PROFILE_SECRET_VALUE";
    let cases = [
        (
            "unsupported-version",
            r#"{"profile_version":2,"profile_id":"test","resources":{},"capabilities":[]}"#,
            "trusted profile version is unsupported",
        ),
        (
            "unknown-capability",
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"unknown.tool","enabled":false,"permission":"read"}]}"#,
            "trusted profile capability is unsupported",
        ),
        (
            "unknown-resource",
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"fs.read","enabled":true,"permission":"read","workspace":"missing","max_bytes":32}]}"#,
            "trusted profile references an unavailable symbolic resource",
        ),
        (
            "duplicate-capability",
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"fs.read","enabled":false,"permission":"read"},{"name":"fs.read","enabled":false,"permission":"read"}]}"#,
            "trusted profile declares a capability more than once",
        ),
        (
            "secret-unknown-field",
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[],"token":"CLI_PROFILE_SECRET_VALUE"}"#,
            "trusted profile is invalid: schema",
        ),
    ];

    for (name, contents, expected_error) in cases {
        let profile = directory.0.join(format!("{name}.json"));
        fs::write(&profile, contents).expect("profile should be written");
        let output = rah()
            .args(["profile", "validate"])
            .arg(&profile)
            .output()
            .expect("rah should launch");
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success(), "{name} should fail");
        assert!(
            output.stdout.is_empty(),
            "{name} exposed a partial inventory"
        );
        assert!(stderr.contains(expected_error), "stderr: {stderr}");
        assert!(!stderr.contains(&profile.to_string_lossy().to_string()));
        assert!(!stderr.contains(&directory.0.to_string_lossy().to_string()));
        assert!(!stderr.contains(secret));
    }
}

#[test]
fn profile_validate_rejects_missing_profile_without_leaking_path() {
    let directory = TestDirectory::new();
    let missing = directory.0.join("CLI_PROFILE_MISSING_SECRET.json");
    let output = rah()
        .args(["profile", "validate"])
        .arg(&missing)
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("trusted profile source could not be read"));
    assert!(!stderr.contains(&missing.to_string_lossy().to_string()));
    assert!(!stderr.contains("CLI_PROFILE_MISSING_SECRET"));
}
