#![cfg(feature = "live-test-support")]

use futures::StreamExt;
use rah_protocol::{AgentEvent, PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_runtime::{experimental::ToolRequest, experimental_host::HostToolScope};
use rah_tools::{RepositoryStatusTool, ToolRegistry};

// The private observer control separately tests the exact status runner.
struct SlowObservation(rah_tools::HostExecutionPolicy);

#[async_trait::async_trait]
impl rah_tools::Tool for SlowObservation {
    fn definition(&self) -> rah_protocol::ToolDefinition {
        rah_protocol::ToolDefinition {
            name: ToolName::new("repo.status"),
            description: "Test-owned timeout control".into(),
            input_schema: serde_json::json!({"type":"object"}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _: rah_tools::ToolContext,
    ) -> Result<rah_protocol::ToolOutput, rah_tools::ToolError> {
        let total = Duration::from_secs(10);
        let started = Instant::now() - Duration::from_secs(9);
        let remaining = total.checked_sub(started.elapsed()).unwrap();
        let tool = rah_tools::HostExecutionTool::new(
            "test.slow-child",
            "Native test-owned child",
            self.0.clone().with_timeout(remaining)?,
        );
        let output =
            rah_tools::Tool::execute(&tool, input, rah_tools::ToolContext::default()).await?;
        let ToolContent::Json(value) = &output.content[0] else {
            panic!("expected process result")
        };
        assert_eq!(value["timed_out"], true);
        assert_eq!(value["termination_attempted"], true);
        Err(rah_tools::ToolError::Execution {
            message: "test-owned observation child timed out after termination/reap".into(),
        })
    }
}

#[tokio::test]
#[ignore = "R4H explicit native forced-timeout host dispatch"]
async fn forced_timeout_dispatch() {
    let root = std::env::temp_dir().join(format!("rah-r4h-slow-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let fixture = std::fs::canonicalize(std::env::var_os("RAH_R4H_FIXTURE").unwrap()).unwrap();
    let policy = rah_tools::HostExecutionPolicy::new(
        &fixture,
        rah_tools::HostArgumentPolicy::Exact(vec![
            "pid-delay".into(),
            root.join("child.pid").display().to_string(),
            "30000".into(),
        ]),
        &root,
        ".",
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(SlowObservation(policy)))
        .unwrap();
    let scope = HostToolScope::new(Arc::new(registry), vec![PermissionLevel::Execute]);
    let port = scope.port();
    let mut lease = port.admit_turn().await.unwrap();
    let wall = Instant::now();
    let result = port
        .request_live(ToolRequest {
            session_id: lease.session_id.clone(),
            name: ToolName::new("repo.status"),
            input: ToolInput(serde_json::json!({})),
        })
        .await;
    assert!(result.is_err());
    assert!(root.join("child.pid").exists());
    // <=1 second remainder plus existing 2-second reap and two pipe graces.
    assert!(wall.elapsed() < Duration::from_secs(7));
    for expected in 0..3 {
        let event = lease.events.next().await.unwrap();
        assert!(matches!(
            (expected, event.event()),
            (0, AgentEvent::ToolRequested { .. })
                | (1, AgentEvent::ToolStarted { .. })
                | (2, AgentEvent::Failed { .. })
        ));
    }
    scope.revoke();
    scope.drained().await;
    eprintln!(
        "R4H forced_dispatch wall={:?} pid={} result={result:?}",
        wall.elapsed(),
        std::fs::read_to_string(root.join("child.pid")).unwrap()
    );
    std::fs::remove_dir_all(root).unwrap();
}
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "R4H explicit six samples or designated final host-only control; no Codex"]
async fn host_status_sample() {
    let label = std::env::var("RAH_R4H_SAMPLE").expect("explicit sample label");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap();
    let resolved = std::process::Command::new("where.exe")
        .arg("git.exe")
        .output()
        .unwrap();
    assert!(resolved.status.success());
    let resolved = String::from_utf8(resolved.stdout).unwrap();
    let git = resolved.lines().next().unwrap();
    assert_eq!(git, r"C:\Program Files\Git\cmd\git.exe");
    let whole = Instant::now();
    let tool = RepositoryStatusTool::new(git, &root).unwrap();
    let setup = whole.elapsed();
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(tool)).unwrap();
    let scope = HostToolScope::new(Arc::new(registry), vec![PermissionLevel::Execute]);
    let port = scope.port();
    let mut lease = port.admit_turn().await.unwrap();
    let dispatch = Instant::now();
    let (result, provenance) = rah_sandbox::provenance::capture(async {
        let result = port
            .request_live(ToolRequest {
                session_id: lease.session_id.clone(),
                name: ToolName::new("repo.status"),
                input: ToolInput(serde_json::json!({})),
            })
            .await;
        if let Err(error) = &result {
            rah_sandbox::provenance::error("RuntimeFailure", error);
            if let Some(source) = std::error::Error::source(error)
                .and_then(|e| e.downcast_ref::<rah_tools::AuthorizedDispatchError>())
            {
                rah_sandbox::provenance::error("AuthorizedDispatchError", source);
                if let rah_tools::AuthorizedDispatchError::Tool(tool) = source {
                    rah_sandbox::provenance::error("ToolError", tool);
                }
            }
        }
        result
    })
    .await;
    let dispatch_wall = dispatch.elapsed();
    for record in &provenance {
        eprintln!("R4I {record}");
    }
    if let Err(error) = &result {
        let mut source = std::error::Error::source(error);
        while let Some(error) = source {
            eprintln!("R4H retained_source debug={error:?} display={error}");
            source = error.source();
        }
    }
    eprintln!(
        "R4H SAMPLE label={label} test_pid={} git={git:?} setup={setup:?} dispatch={dispatch_wall:?} success={} result={result:?}",
        std::process::id(),
        result.is_ok()
    );
    scope.revoke();
    scope.drained().await;
    eprintln!("R4I scope drained before result assertion");
    let output = result.expect("failed sample is preserved; no replacement");
    assert!(!output.is_error);
    let ToolContent::Json(value) = &output.content[0] else {
        panic!("expected JSON status")
    };
    let entries = value["entries"].as_array().unwrap().len();
    assert!(entries > 0, "dirty WIP is valid input");
    for expected in 0..3 {
        let event = lease.events.next().await.unwrap();
        assert!(matches!(
            (expected, event.event()),
            (0, AgentEvent::ToolRequested { .. })
                | (1, AgentEvent::ToolStarted { .. })
                | (2, AgentEvent::ToolFinished { .. })
        ));
    }
    scope.revoke();
    scope.drained().await;
    eprintln!(
        "R4H SAMPLE_COMPLETE label={label} entries={entries} dispatch={dispatch_wall:?} whole={:?}",
        whole.elapsed()
    );
    // C2 is not a hard dispatch deadline; retain measured dispatch time above.
}

struct ProvenanceFailure;
#[async_trait::async_trait]
impl rah_tools::Tool for ProvenanceFailure {
    fn definition(&self) -> rah_protocol::ToolDefinition {
        rah_protocol::ToolDefinition {
            name: ToolName::new("repo.status"),
            description: "provenance control".into(),
            input_schema: serde_json::json!({"type":"object"}),
            permission: PermissionLevel::Execute,
        }
    }
    async fn execute(
        &self,
        _: ToolInput,
        _: rah_tools::ToolContext,
    ) -> Result<rah_protocol::ToolOutput, rah_tools::ToolError> {
        let spec = rah_sandbox::HostProcessSpec {
            executable: std::env::temp_dir().join("rah-r4i-deliberately-missing.exe"),
            args: vec!["--r4i-exact-command".into()],
            cwd: std::env::temp_dir(),
            environment: Default::default(),
            timeout: Duration::from_secs(1),
            output_limits: rah_sandbox::OutputLimits::RECOMMENDED,
        };
        let error = rah_sandbox::provenance::phase(
            "ProvenanceFailure::execute",
            Some(Duration::from_secs(1)),
            rah_sandbox::execute_host_process(spec),
        )
        .await
        .unwrap_err();
        rah_sandbox::provenance::error("SandboxError -> ToolError", &error);
        Err(rah_tools::ToolError::Execution {
            message: error.to_string(),
        })
    }
}
#[tokio::test]
async fn provenance_retains_failed_dispatch() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ProvenanceFailure)).unwrap();
    let scope = HostToolScope::new(Arc::new(registry), vec![PermissionLevel::Execute]);
    let port = scope.port();
    let lease = port.admit_turn().await.unwrap();
    let (result, records) = rah_sandbox::provenance::capture(async {
        let result = port
            .request_live(ToolRequest {
                session_id: lease.session_id.clone(),
                name: ToolName::new("repo.status"),
                input: ToolInput(serde_json::json!({})),
            })
            .await;
        let error = result.as_ref().unwrap_err();
        let source = std::error::Error::source(error)
            .unwrap()
            .downcast_ref::<rah_tools::AuthorizedDispatchError>()
            .unwrap();
        rah_sandbox::provenance::error("AuthorizedDispatchError", source);
        let rah_tools::AuthorizedDispatchError::Tool(tool) = source else {
            panic!("expected admitted failure")
        };
        rah_sandbox::provenance::error("ToolError", tool);
        result
    })
    .await;
    assert!(result.is_err());
    let trace = records.join("\n");
    for expected in [
        "ProvenanceFailure::execute",
        "--r4i-exact-command",
        "Command::spawn",
        "core::io::error::Error",
        "NotFound",
        "io_fields",
        "SandboxError",
        "ToolError",
        "AuthorizedDispatchError",
        "process_return",
    ] {
        assert!(trace.contains(expected), "missing {expected}: {trace}");
    }
    assert!(!trace.contains("spawn_pid"));
    scope.revoke();
    scope.drained().await;
    eprintln!("R4I deterministic retained trace={trace}");
}
