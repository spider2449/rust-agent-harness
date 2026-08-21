use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_tools::{
    CARGO_VERSION_TOOL_NAME, CargoVersionTool, Tool, ToolContext, ToolError, ToolRegistry,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-cargo-version-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..5 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rah_execute_fixture"))
}

fn copied_fixture(root: &Path) -> PathBuf {
    let extension = fixture()
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let copy = root.join(format!("configured-cargo{extension}"));
    fs::copy(fixture(), &copy).expect("fixture executable should be copied");
    copy
}

fn json_content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("Cargo version output should contain one JSON value")
    };
    value
}

#[tokio::test]
async fn definition_and_execution_are_capability_specific() {
    let cwd = TestDirectory::new("definition");
    let tool = CargoVersionTool::new(fixture(), &cwd.0).expect("fixture policy should be valid");
    let definition = tool.definition();

    assert_eq!(definition.name, ToolName::new(CARGO_VERSION_TOOL_NAME));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    );
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(
            CargoVersionTool::new(fixture(), &cwd.0).expect("fixture policy should be valid"),
        ))
        .expect("Cargo version capability should register");
    assert_eq!(registry.definitions(), vec![definition.clone()]);
    assert!(
        registry
            .definitions()
            .iter()
            .all(|registered| registered.name.as_str() != "shell.exec")
    );

    for forbidden in [
        json!({"executable": "cargo"}),
        json!({"program": "cargo"}),
        json!({"command": "cargo build"}),
        json!({"args": ["build"]}),
        json!({"argv": ["test"]}),
        json!({"cwd": "."}),
        json!({"environment": {"PATH": "attacker"}}),
        json!({"env": {"RUSTUP_HOME": "attacker"}}),
        json!({"timeout": 60}),
        json!({"shell": true}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(forbidden), ToolContext::default())
                .await,
            Err(ToolError::InvalidInput { .. })
        ));
    }

    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect("exact Cargo version capability should execute");
    let content = json_content(&output);
    assert!(!output.is_error);
    assert_eq!(content["status"], "exited");
    assert_eq!(content["exit_code"], 0);
    assert_eq!(content["timed_out"], false);
    assert_eq!(content["termination_attempted"], false);
    assert!(content["stdout"].as_str().unwrap().starts_with("cargo "));
    assert_eq!(content["stderr"], "");
    assert!(serde_json::to_vec(&output).unwrap().len() <= 768 * 1024);
}

#[test]
fn executable_must_be_absolute_and_native() {
    let cwd = TestDirectory::new("absolute");
    let fixture = fixture();
    let relative = fixture
        .file_name()
        .expect("fixture should have a file name");
    let error = CargoVersionTool::new(relative, &cwd.0)
        .err()
        .expect("relative executable must be rejected");
    assert!(error.to_string().contains("PATH lookup is disabled"));

    #[cfg(windows)]
    {
        let command_file = cwd.0.join("cargo.cmd");
        fs::write(&command_file, "@echo cargo unsafe").expect("command fixture should be created");
        let error = CargoVersionTool::new(command_file, &cwd.0)
            .err()
            .expect("command interpreter association must be rejected");
        assert!(error.to_string().contains("native .exe"));
    }
}

#[tokio::test]
async fn configured_executable_identity_is_revalidated_before_spawn() {
    let cwd = TestDirectory::new("identity");
    let executable = copied_fixture(&cwd.0);
    let tool =
        CargoVersionTool::new(&executable, &cwd.0).expect("copied fixture policy should be valid");

    let mut replacement = fs::read(&executable).expect("fixture copy should be readable");
    replacement.push(0);
    fs::write(&executable, replacement).expect("fixture identity should be replaceable");

    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("changed executable identity must fail before spawn");
    assert!(error.to_string().contains("executable identity changed"));
}

#[tokio::test]
#[ignore = "requires RAH_CARGO_VERSION_EXECUTABLE to name an absolute native Cargo executable"]
async fn explicitly_configured_local_cargo_reports_its_real_version() {
    let cargo = std::env::var_os("RAH_CARGO_VERSION_EXECUTABLE")
        .expect("RAH_CARGO_VERSION_EXECUTABLE must be set for this opt-in test");
    let cwd = TestDirectory::new("local");
    let tool = CargoVersionTool::new(cargo, &cwd.0)
        .expect("explicit local Cargo configuration should be valid");

    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect("configured local Cargo should execute directly");
    let content = json_content(&output);
    assert!(!output.is_error, "unexpected Cargo result: {content}");
    assert_eq!(content["exit_code"], 0);
    assert_eq!(content["stderr"], "");
    assert!(content["stdout"].as_str().unwrap().starts_with("cargo "));
}
