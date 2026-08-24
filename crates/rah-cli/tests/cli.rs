use std::{
    fs,
    path::{Path, PathBuf},
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

    fn fixture(&self, binary: &str, label: &str) -> PathBuf {
        let target = std::env::var_os("RAH_TEST_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root().join("target"));
        let source = target
            .join("debug")
            .join(format!("{binary}{}", std::env::consts::EXE_SUFFIX));
        let executable = self
            .0
            .join(format!("{label}{}", std::env::consts::EXE_SUFFIX));
        fs::copy(&source, &executable).expect("fixture executable should be copied");
        fs::write(executable.with_extension("lifecycle-request"), b"observe")
            .expect("fixture lifecycle observation should be enabled");
        executable
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

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CLI crate is nested below workspace")
        .to_owned()
}

fn external_profile(directory: &Path, mcp: &Path, plugin: &Path) -> String {
    let path = |value: &Path| value.to_string_lossy().replace('\\', "\\\\");
    format!(
        r#"{{"profile_version":1,"profile_id":"cli-external","resources":{{"executables":{{"mcp":{{"path":"{}","kind":"native"}},"plugin":{{"path":"{}","kind":"native"}}}},"repositories":{{"workspace":{{"path":"{}"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"mcp-a","executable":"mcp","tools":[{{"remote_name":"echo","permission":"None","input_schema":{{"type":"object","properties":{{"text":{{"type":"string"}}}},"required":["text"],"additionalProperties":false}}}}]}}],"process_plugins":[{{"id":"plugin-b","executable":"plugin","tools":[{{"remote_name":"echo","permission":"Read","input_schema":{{"type":"object","properties":{{"value":{{}}}},"required":["value"],"additionalProperties":false}}}}]}}]}}"#,
        path(mcp),
        path(plugin),
        path(directory)
    )
}

fn git_executable() -> PathBuf {
    #[cfg(windows)]
    let output = Command::new("where.exe")
        .arg("git.exe")
        .output()
        .expect("where should locate Git");
    #[cfg(not(windows))]
    let output = Command::new("which")
        .arg("git")
        .output()
        .expect("which should locate Git");
    assert!(
        output.status.success(),
        "Git should be available for fixtures"
    );
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("Git path should be UTF-8")
            .lines()
            .next()
            .expect("Git path should be present"),
    )
}

fn git_status(directory: &Path) -> Vec<u8> {
    let output = Command::new(git_executable())
        .args(["status", "--porcelain=v1"])
        .current_dir(directory)
        .output()
        .expect("Git status should run");
    assert!(output.status.success(), "Git status should succeed");
    output.stdout
}

fn repo_patch_profile(directory: &Path) -> String {
    let path = |value: &Path| value.to_string_lossy().replace('\\', "\\\\");
    format!(
        r#"{{"profile_version":1,"profile_id":"cli-repo-patch","resources":{{"executables":{{"git":{{"path":"{}","kind":"native"}}}},"repositories":{{"worktree":{{"path":"{}"}}}}}},"capabilities":[{{"name":"repo.patch","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#,
        path(&git_executable()),
        path(directory)
    )
}

fn repository_observer_profile(directory: &Path) -> String {
    let path = |value: &Path| value.to_string_lossy().replace('\\', "\\\\");
    format!(
        r#"{{"profile_version":1,"profile_id":"cli-repository-observers","resources":{{"executables":{{"git":{{"path":"{}","kind":"native"}}}},"repositories":{{"worktree":{{"path":"{}"}}}}}},"capabilities":[{{"name":"repo.file-info","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}},{{"name":"repo.status","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}},{{"name":"repo.diff","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}},{{"name":"repo.diff-staged","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#,
        path(&git_executable()),
        path(directory)
    )
}

fn lifecycle(path: &Path) -> String {
    fs::read_to_string(path.with_extension("lifecycle")).unwrap_or_default()
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

#[test]
fn profile_repo_patch_static_and_effective_validation_are_non_mutating_and_redacted() {
    let directory = TestDirectory::new();
    let initialized = Command::new(git_executable())
        .args(["init", "--quiet"])
        .current_dir(&directory.0)
        .status()
        .expect("Git should initialize the owned fixture repository");
    assert!(initialized.success());
    let target = directory.0.join("target.txt");
    fs::write(&target, b"repo.patch must not execute during validation\n")
        .expect("fixture target should be written");
    let profile = directory.profile(&repo_patch_profile(&directory.0));
    let target_before = fs::read(&target).expect("fixture target should be readable");
    let status_before = git_status(&directory.0);

    let static_output = rah()
        .args(["profile", "validate"])
        .arg(&profile)
        .output()
        .expect("rah should launch static validation");
    let static_stdout = String::from_utf8_lossy(&static_output.stdout);
    assert!(
        static_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&static_output.stderr)
    );
    assert!(static_stdout.contains(
        "capability name=repo.patch enabled=true registered=false permission=Execute resources=git,worktree validation=configured"
    ));
    assert!(!static_stdout.contains(directory.0.to_string_lossy().as_ref()));
    assert_eq!(fs::read(&target).unwrap(), target_before);
    assert_eq!(git_status(&directory.0), status_before);

    let effective_output = rah()
        .args(["profile", "validate-effective"])
        .arg(&profile)
        .output()
        .expect("rah should launch effective validation");
    let effective_stdout = String::from_utf8_lossy(&effective_output.stdout);
    assert!(
        effective_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&effective_output.stderr)
    );
    assert!(effective_stdout.contains(
        "capability name=repo.patch enabled=true registered=true permission=Execute resources=git,worktree validation=validated"
    ));
    assert!(effective_stdout.contains("tool name=repo.patch permission=Execute"));
    for secret in [
        directory.0.to_string_lossy().as_ref(),
        profile.to_string_lossy().as_ref(),
        git_executable().to_string_lossy().as_ref(),
    ] {
        assert!(
            !effective_stdout.contains(secret),
            "inventory leaked {secret}"
        );
    }
    assert_eq!(fs::read(&target).unwrap(), target_before);
    assert_eq!(git_status(&directory.0), status_before);
}

#[test]
fn profile_repository_observers_static_and_effective_validation_are_redacted_and_non_mutating() {
    let directory = TestDirectory::new();
    let initialized = Command::new(git_executable())
        .args(["init", "--quiet"])
        .current_dir(&directory.0)
        .status()
        .expect("Git should initialize the owned fixture repository");
    assert!(initialized.success());
    let target = directory.0.join("target.txt");
    fs::write(
        &target,
        b"observer validation must not execute observations\n",
    )
    .expect("fixture target should be written");
    let profile = directory.profile(&repository_observer_profile(&directory.0));
    let target_before = fs::read(&target).expect("fixture target should be readable");
    let status_before = git_status(&directory.0);

    let static_output = rah()
        .args(["profile", "validate"])
        .arg(&profile)
        .output()
        .expect("rah should launch static validation");
    let static_stdout = String::from_utf8_lossy(&static_output.stdout);
    assert!(static_output.status.success());
    for name in [
        "repo.file-info",
        "repo.status",
        "repo.diff",
        "repo.diff-staged",
    ] {
        assert!(static_stdout.contains(&format!(
            "capability name={name} enabled=true registered=false permission=Execute resources=git,worktree validation=configured"
        )));
    }
    assert_eq!(fs::read(&target).unwrap(), target_before);
    assert_eq!(git_status(&directory.0), status_before);

    let effective_output = rah()
        .args(["profile", "validate-effective"])
        .arg(&profile)
        .output()
        .expect("rah should launch effective validation");
    let effective_stdout = String::from_utf8_lossy(&effective_output.stdout);
    assert!(
        effective_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&effective_output.stderr)
    );
    for name in [
        "repo.file-info",
        "repo.status",
        "repo.diff",
        "repo.diff-staged",
    ] {
        assert!(effective_stdout.contains(&format!(
            "capability name={name} enabled=true registered=true permission=Execute resources=git,worktree validation=validated"
        )));
        assert!(effective_stdout.contains(&format!("tool name={name} permission=Execute")));
    }
    for secret in [
        directory.0.to_string_lossy().as_ref(),
        profile.to_string_lossy().as_ref(),
        git_executable().to_string_lossy().as_ref(),
    ] {
        assert!(
            !effective_stdout.contains(secret),
            "inventory leaked {secret}"
        );
    }
    assert_eq!(fs::read(&target).unwrap(), target_before);
    assert_eq!(git_status(&directory.0), status_before);
}

#[test]
fn profile_validate_effective_fails_without_partial_inventory_or_path_leakage() {
    let directory = TestDirectory::new();
    let missing_executable = directory.0.join("CLI_MCP_SECRET.exe");
    let profile = directory.profile(&format!(
        r#"{{"profile_version":1,"profile_id":"mcp-cli","resources":{{"executables":{{"mcp":{{"path":"{}","kind":"native"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"local","executable":"mcp","tools":[{{"remote_name":"echo","permission":"None","input_schema":{{"type":"object"}}}}]}}]}}"#,
        missing_executable.to_string_lossy().replace('\\', "\\\\")
    ));
    let output = rah()
        .args(["profile", "validate-effective"])
        .arg(&profile)
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(stderr.contains("trusted profile external provider could not be composed"));
    assert!(!stderr.contains(&profile.to_string_lossy().to_string()));
    assert!(!stderr.contains(&missing_executable.to_string_lossy().to_string()));
}

#[test]
fn profile_static_validation_never_spawns_mixed_external_fixtures() {
    let directory = TestDirectory::new();
    let mcp = directory.fixture("rah-mcp-echo-server", "mcp");
    let plugin = directory.fixture("rah-plugin-echo", "plugin");
    let profile = directory.profile(&external_profile(&directory.0, &mcp, &plugin));
    let output = rah()
        .args(["profile", "validate"])
        .arg(profile)
        .output()
        .expect("rah should launch");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(lifecycle(&mcp).is_empty(), "static validation spawned MCP");
    assert!(
        lifecycle(&plugin).is_empty(),
        "static validation spawned Plugin"
    );
}

#[test]
fn profile_effective_validation_spawns_redacts_and_shuts_down_mixed_external_fixtures() {
    let directory = TestDirectory::new();
    let mcp = directory.fixture("rah-mcp-echo-server", "mcp");
    let plugin = directory.fixture("rah-plugin-echo", "plugin");
    let profile = directory.profile(&external_profile(&directory.0, &mcp, &plugin));
    let output = rah()
        .args(["profile", "validate-effective"])
        .arg(&profile)
        .output()
        .expect("rah should launch");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(lifecycle(&mcp).contains("spawn"));
    assert!(lifecycle(&plugin).contains("spawn"));
    assert!(stdout.contains("provider kind=mcp id=mcp-a status=validated"));
    assert!(stdout.contains("provider kind=process_plugin id=plugin-b status=validated"));
    assert!(stdout.contains("tool name=mcp.mcp-a.echo permission=None"));
    assert!(stdout.contains("tool name=plugin.plugin-b.echo permission=Read"));
    for secret in [
        mcp.to_string_lossy().as_ref(),
        plugin.to_string_lossy().as_ref(),
        directory.0.to_string_lossy().as_ref(),
    ] {
        assert!(!stdout.contains(secret), "inventory leaked {secret}");
    }
    let moved = mcp.with_extension("released");
    fs::rename(&mcp, &moved).expect("MCP fixture must be reaped before CLI returns");
    fs::rename(&moved, &mcp).expect("fixture should be restored");
}
