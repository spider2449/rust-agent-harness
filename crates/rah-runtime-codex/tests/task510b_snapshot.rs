#![cfg(feature = "certification-harness")]

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId,
};
use rah_runtime::{experimental::*, experimental_host::HostToolScope};
use rah_runtime_codex::{
    CodexModelProvider,
    certification_support::{
        ExactCodexCandidate, measure_artifact, verify_and_construct_candidate,
    },
};
use std::{error::Error, path::Path, sync::Arc, time::Duration};

const SNAPSHOT: &str =
    "F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe";
const HASH: &str = "fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d";

const R5_BUNDLE: &str =
    "F:/Temp/rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c";

#[test]
#[ignore = "R5A explicit read-only frozen bundle/source identity gate; no runtime launch"]
fn r5_bundle_identity() {
    let descriptor: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("F:/Temp/rah-task510br4-evidence/bundle-descriptor.json").unwrap(),
    )
    .unwrap();
    for (index, name, hash, length, id) in [
        (0, "codex.exe", HASH, 326872368, 0x00110000000c1861),
        (
            1,
            "codex-code-mode-host.exe",
            "1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6",
            74697520,
            0x00180000000c1862,
        ),
    ] {
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(hash.to_uppercase().to_lowercase(), hash);
        assert_eq!(descriptor["members"][index]["bundle_sha256"], hash);
        let measured = measure_artifact(&Path::new(R5_BUNDLE).join(name)).unwrap();
        println!("R5 BUNDLE {name}: {measured:?}");
        assert_eq!(measured.sha256, hash);
        assert_eq!(measured.bytes_read, length);
        assert_eq!(measured.file_id, Some(id));
        let source = measure_artifact(Path::new(
            descriptor["members"][index]["source_canonical_path"]
                .as_str()
                .unwrap(),
        ))
        .unwrap();
        println!("R5 SOURCE {name}: {source:?}");
        assert_eq!(source.sha256, measured.sha256);
        assert_eq!(source.bytes_read, length);
    }
}

#[test]
#[ignore = "R4 explicit exact runtime bundle creation; never launches a runtime"]
fn r4_create_bundle() {
    use rah_runtime_codex::certification_support::create_snapshot_at;
    let installed = Path::new(
        "C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin",
    );
    let root = Path::new("F:/Temp").join(format!(
        "rah-codex-certification-bundle-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut members = Vec::new();
    for (name, hash, length) in [
        ("codex.exe", HASH, 326872368),
        (
            "codex-code-mode-host.exe",
            "1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6",
            74697520,
        ),
    ] {
        let source = ExactCodexCandidate {
            path: installed.join(name),
            expected_version: "0.160.0".into(),
            expected_sha256: hash.into(),
        };
        let copy = create_snapshot_at(&source, &root.join(name)).unwrap();
        assert_eq!(copy.source.bytes_read, length);
        assert_eq!(copy.snapshot.bytes_read, length);
        assert_ne!(copy.source.file_id, copy.snapshot.file_id);
        members.push(serde_json::json!({"relative_path": name, "source_canonical_path": copy.source.canonical_path, "source_sha256": copy.source.sha256, "source_length": copy.source.bytes_read, "source_file_id": format!("0x{:032x}", copy.source.file_id.unwrap()), "source_volume": copy.source.volume_serial, "bundle_path": copy.snapshot.canonical_path, "bundle_sha256": copy.snapshot.sha256, "bundle_length": copy.snapshot.bytes_read, "bundle_file_id": format!("0x{:032x}", copy.snapshot.file_id.unwrap()), "bundle_volume": copy.snapshot.volume_serial, "package_version": "0.160.0-win32-x64", "runtime_manifest_version": "0.160.0"}));
    }
    let descriptor = serde_json::json!({"root": root, "members": members});
    let path = Path::new("F:/Temp/rah-task510br4-evidence/bundle-descriptor.json");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(
        serde_json::to_string_pretty(&descriptor)
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    file.sync_all().unwrap();
    println!("R4 BUNDLE {}", root.display());
}

#[tokio::test]
#[ignore = "R4 offline prerequisite only: loopback provider, no inference or Tool dispatch"]
async fn r4_bundle_prerequisite() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let descriptor: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string("F:/Temp/rah-task510br4-evidence/bundle-descriptor.json").unwrap(),
    )
    .unwrap();
    let root = Path::new(descriptor["root"].as_str().unwrap());
    let main = root.join("codex.exe");
    let companion = root.join("codex-code-mode-host.exe");
    assert_eq!(measure_artifact(&main).unwrap().sha256, HASH);
    assert_eq!(
        measure_artifact(&companion).unwrap().sha256,
        "1d448bfde19e7a280d600d8d0bcddf77afbe9feaec1e804905becc5f39bc9db6"
    );
    let home = Path::new("F:/Temp/rah-task510br4-evidence")
        .join(format!("offline-home-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&home).unwrap();
    // A closed loopback port cannot perform model inference. This provider has
    // no credentials and no remote endpoint; it stops after local preflight.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let mut child = tokio::process::Command::new(&main)
        .arg("app-server")
        .env("CODEX_HOME", &home)
        .env("PATH", "")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_MANAGED_BY_NPM")
        .env_remove("CODEX_MANAGED_BY_BUN")
        .env_remove("CODEX_MANAGED_BY_PNPM")
        .env_remove("CODEX_MANAGED_BY_VITE_PLUS")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    println!(
        "R4 MAIN path={} pid={:?}; companion resolution sibling={}; PATH empty; isolated home={}",
        main.display(),
        child.id(),
        companion.display(),
        home.display()
    );
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut evidence = Vec::new();
    let init = serde_json::json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"rah-r4-offline","version":"0.1"},"capabilities":{"experimentalApi":true}}});
    stdin
        .write_all(format!("{init}\n").as_bytes())
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(line) = lines.next_line().await.unwrap() {
            let message: serde_json::Value = serde_json::from_str(&line).unwrap();
            evidence.push(message.clone());
            if message["id"] == 1 {
                assert!(message.get("error").is_none(), "{message}");
                let params = serde_json::json!({"model":"gpt-6.1-sol","modelProvider":"r4-local","cwd":home,"approvalPolicy":"never","sandbox":"read-only","config":{"features":{"code_mode_only":true,"code_mode_host":true,"shell_tool":false,"unified_exec":false,"memories":false},"tools":{"web_search":false,"view_image":false},"apps":{"_default":{"enabled":false}},"mcp_servers":{},"model_providers":{"r4-local":{"name":"R4 offline prerequisite","base_url":format!("http://{address}/v1"),"wire_api":"responses","requires_openai_auth":false,"request_max_retries":0,"stream_max_retries":0}}}});
                let start = serde_json::json!({"id":2,"method":"thread/start","params":params});
                stdin.write_all(format!("{{\"method\":\"initialized\"}}\n{start}\n").as_bytes()).await.unwrap();
            }
            if message["id"] == 2 {
                assert!(message.get("error").is_none(), "{message}");
                let turn = serde_json::json!({"id":3,"method":"turn/start","params":{"threadId":message["result"]["thread"]["id"],"input":[{"type":"text","text":"Offline prerequisite control; no Tools."}],"approvalPolicy":"never","sandboxPolicy":{"type":"readOnly"}}});
                stdin.write_all(format!("{turn}\n").as_bytes()).await.unwrap();
            }
            if message["id"] == 3 { assert!(message.get("error").is_none(), "{message}"); }
            // Stop at the first connection failure, before provider retries.
            if message["method"] == "error" { return; }
        }
        panic!("runtime exited before completing local prerequisite");
    }).await;
    let inventory = std::process::Command::new("C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe")
        .args(["-NoProfile", "-Command", &format!("Get-CimInstance Win32_Process | Where-Object {{ $_.ProcessId -eq {} -or $_.Name -eq 'codex-code-mode-host.exe' }} | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json", child.id().unwrap())])
        .output().unwrap();
    assert!(inventory.status.success());
    std::fs::write(
        "F:/Temp/rah-task510br4-evidence/prerequisite-processes.json",
        &inventory.stdout,
    )
    .unwrap();
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    let mut stderr = String::new();
    use tokio::io::AsyncReadExt;
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .await
        .unwrap();
    std::fs::write(
        "F:/Temp/rah-task510br4-evidence/offline-protocol.json",
        serde_json::to_string_pretty(&evidence).unwrap(),
    )
    .unwrap();
    std::fs::write(
        "F:/Temp/rah-task510br4-evidence/offline-stderr.log",
        &stderr,
    )
    .unwrap();
    outcome.unwrap();
    let text = serde_json::to_string(&evidence).unwrap();
    assert!(!text.contains("host executable was not found"));
    assert!(!stderr.contains("host executable was not found"));
    assert!(evidence.iter().any(|m| m["method"] == "turn/started"));
    assert!(!text.contains("item/tool/call"));
    assert!(
        text.contains("Connection failed"),
        "must stop at connection preflight: {text}"
    );
    println!(
        "R4 offline local preflight PASS; missing companion warning absent; no Tool call or model inference"
    );
}

fn request(text: &str) -> TurnInput {
    TurnInput::TextReplay(AgentRequest {
        request_id: RequestId::new(),
        options: AgentOptions::default(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: text.into(),
            }],
        },
    })
}

#[tokio::test]
#[ignore = "Task 510B explicitly authorized live Tool/cancellation/diagnostic certification"]
async fn snapshot_tool() {
    snapshot_phase("Tool").await;
}

#[tokio::test]
#[ignore = "Task 510B explicitly authorized live cancellation certification"]
async fn snapshot_cancellation() {
    snapshot_phase("cancellation").await;
}

#[tokio::test]
#[ignore = "Task 510B explicitly authorized live diagnostic certification"]
async fn snapshot_diagnostic() {
    snapshot_phase("diagnostic").await;
}

fn repository_root() -> std::path::PathBuf {
    // Same layout convention as tests/architecture.rs; independent of process cwd.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("adapter crate is nested under workspace/crates")
        .canonicalize()
        .unwrap();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("[workspace]"));
    assert!(root.join("crates/rah-runtime-codex/Cargo.toml").is_file());
    let git = std::fs::metadata(root.join(".git")).unwrap();
    assert!(git.is_file() || git.is_dir());
    root
}

fn host_registry(root: &Path) -> rah_tools::ToolRegistry {
    let git = std::process::Command::new("where.exe")
        .arg("git.exe")
        .output()
        .unwrap();
    assert!(git.status.success());
    let git = String::from_utf8(git.stdout).unwrap();
    let git = git.lines().next().unwrap();
    let tool = rah_tools::RepositoryStatusTool::new(git, root)
        .expect("normal host Git-layout validation must accept the selected authority root");
    let mut registry = rah_tools::ToolRegistry::new();
    registry.register(Arc::new(tool)).unwrap();
    let definitions = registry.definitions();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].name.as_str(), "repo.status");
    println!(
        "HOST SETUP PASS: authority root={root:?}; normal Git-layout validation accepted; registry=repo.status only"
    );
    registry
}

#[test]
fn repository_host_setup() {
    let root = repository_root();
    host_registry(&root);
}

async fn snapshot_phase(phase: &str) {
    let capture = rah_runtime_codex::certification_support::ProtocolCapture::default();
    let r5 = std::env::var("RAH_R5_BUNDLE").is_ok();
    let path = if r5 {
        "F:/Temp/rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c/codex.exe"
    } else {
        SNAPSHOT
    };
    let before = measure_artifact(Path::new(path)).unwrap();
    assert_eq!(before.sha256, HASH);
    assert_eq!(before.bytes_read, 326872368);
    assert_eq!(
        before.file_id,
        Some(if r5 {
            0x00110000000c1861
        } else {
            0x002e0000000a80a1
        })
    );
    let workspace = repository_root();
    {
        let registry = if phase == "Tool" {
            host_registry(&workspace)
        } else {
            rah_tools::ToolRegistry::new()
        };
        let factory = verify_and_construct_candidate(
            ExactCodexCandidate {
                path: path.into(),
                expected_version: "0.160.0".into(),
                expected_sha256: HASH.into(),
            },
            CodexModelProvider::OpenAi,
            &workspace,
        )
        .await
        .unwrap()
        .with_protocol_capture(capture.clone());
        let runtime = tokio::time::timeout(Duration::from_secs(60), factory.create())
            .await
            .unwrap()
            .unwrap();
        let scope = HostToolScope::new(
            Arc::new(registry),
            vec![rah_protocol::PermissionLevel::Execute],
        );
        let result = async {
            let model = if phase == "diagnostic" {
                "rah-task510b-invalid-model"
            } else {
                "gpt-6.1-sol"
            };
            let conversation = runtime
                .open(ConversationSeed {
                    id: ConversationId::new(format!("task510b-{phase}")),
                    model: ModelSelection::Explicit(model.into()),
                    tools: scope.port(),
                })
                .await?;
            let prompt = match phase {
                "Tool" => {
                    "Call repo.status exactly once with {}. Use only the available RAH Tool. After the result, reply RAH510B_TOOL_OK. Do not call any other tool."
                }
                "cancellation" => {
                    "Write a detailed 10000 word fictional story about a lighthouse. Do not use tools."
                }
                _ => "Reply OK. Do not use tools.",
            };
            let turn = conversation.send(request(prompt)).await;
            if phase == "diagnostic" {
                let failure = match turn {
                    Err(error) => error,
                    Ok(mut turn) => {
                        let mut failure = None;
                        while let Some(event) = turn.events.next().await {
                            println!("diagnostic EVENT {:?}", event.event());
                            if let Some(error) = event.failure() {
                                failure = Some(error.clone());
                                break;
                            }
                        }
                        failure.expect("invalid model must fail")
                    }
                };
                let public = serde_json::to_string(failure.diagnostic()).unwrap();
                let cause = failure
                    .source()
                    .unwrap()
                    .downcast_ref::<rah_runtime_codex::CodexAdapterError>()
                    .expect("typed adapter cause");
                assert!(matches!(
                    cause,
                    rah_runtime_codex::CodexAdapterError::JsonRpc { .. }
                        | rah_runtime_codex::CodexAdapterError::TurnFailed { .. }
                ));
                assert!(!public.contains(model));
                assert!(!public.contains(SNAPSHOT));
                assert!(!public.contains(path));
                assert!(!public.contains(HASH));
                println!("diagnostic sanitized {public}; typed provider cause retained");
                conversation.close().await?;
                return Ok::<_, rah_runtime::RuntimeFailure>(());
            }
            let mut turn = turn?;
            let mut executions = 0;
            let mut requested = 0;
            let mut finished = 0;
            let mut completed = false;
            let mut cancelled = false;
            let mut cancel_sent = false;
            while let Some(event) = turn.events.next().await {
                println!("{phase} EVENT {:?}", event.event());
                if phase == "cancellation" {
                    println!(
                        "R5C NEUTRAL {} {:?}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros(),
                        event.event()
                    );
                }
                if let Some(failure) = event.failure() {
                    return Err(failure.clone());
                }
                match event.event() {
                    AgentEvent::ToolRequested { tool_call, .. } => {
                        requested += 1;
                        assert_eq!(tool_call.name.as_str(), "repo.status");
                        assert_eq!(requested, 1);
                    }
                    AgentEvent::ToolStarted { .. } => {
                        executions += 1;
                        assert_eq!(executions, 1);
                    }
                    AgentEvent::ToolFinished { output, .. } => {
                        finished += 1;
                        assert!(!output.is_error);
                        let status = serde_json::to_string(output).unwrap();
                        assert!(status.contains(
                            "docs/plans/2026-10-04-task-510b-codex-0-160-0-runtime-certification.md"
                        ));
                        assert!(
                            status.contains("crates/rah-runtime-codex/tests/task510b_snapshot.rs")
                        );
                    }
                    AgentEvent::Completed { .. } => completed = true,
                    AgentEvent::Cancelled { .. } => cancelled = true,
                    AgentEvent::ModelRequestStarted { .. }
                        if phase == "cancellation" && !cancel_sent =>
                    {
                        let (thread, provider_turn) = capture
                            .wait_for_active_turn()
                            .await
                            .expect("provider-active readiness");
                        println!("R5C provider active thread={thread} turn={provider_turn}");
                        cancel_sent = true;
                        assert_eq!(turn.control.cancel().await?, CancelOutcome::Stopped);
                    }
                    _ => {}
                }
            }
            if phase == "Tool" {
                assert_eq!((requested, executions, finished), (1, 1, 1));
                assert!(completed);
                println!(
                    "Tool requests/executions/results={requested}/{executions}/{finished}; intended repository WIP observed"
                );
            } else {
                assert!(cancel_sent && cancelled && !completed);
                assert_eq!(executions, 0);
                assert_eq!(turn.control.cancel().await?, CancelOutcome::AlreadyTerminal);
            }
            conversation.close().await?;
            if r5 {
                let port = scope.port();
                let lease = port.admit_turn().await?;
                drop(lease);
                println!("R5 fresh host turn lease accepted after terminal turn/close");
            }
            Ok(())
        };
        let result = tokio::time::timeout(Duration::from_secs(180), result).await;
        scope.revoke();
        scope.drained().await;
        let shutdown = tokio::time::timeout(Duration::from_secs(20), runtime.shutdown()).await;
        if phase == "cancellation" {
            std::fs::write(
                "F:/Temp/rah-task510br5c-evidence/cancellation-wire.json",
                serde_json::to_string_pretty(
                    &capture
                        .timeline()
                        .into_iter()
                        .map(|(timestamp, outgoing, message)| {
                            (
                                timestamp,
                                outgoing,
                                serde_json::from_str::<serde_json::Value>(&message).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
            )
            .unwrap();
            if let Ok(Err(failure)) = &result {
                println!("R5C PRIVATE CAUSE {:?}", failure.source());
            }
        }
        assert_eq!(measure_artifact(Path::new(path)).unwrap(), before);
        println!(
            "{phase} shutdown={shutdown:?}; alive={}; POST identity unchanged",
            runtime.is_alive()
        );
        assert!(matches!(shutdown, Ok(Ok(()))));
        assert!(matches!(result, Ok(Ok(()))), "{phase} FAILED: {result:?}");
        println!("{phase} PASS");
    }
}

#[tokio::test]
#[ignore = "Task 510B-R2 bounded raw-protocol A/B diagnosis only"]
async fn tool_path_diagnosis() {
    use rah_runtime_codex::certification_support::ProtocolCapture;
    let baseline = "C:/Users/morefunfun/AppData/Local/codex-baselines/0.157.1/codex.exe";
    let version = std::env::var("RAH_R2_VERSION").unwrap();
    let stronger = std::env::var("RAH_R2_STRONGER").is_ok();
    let r5 = std::env::var("RAH_R5_BUNDLE").is_ok();
    if r5 {
        assert_eq!(version, "0.160.0");
        assert!(!stronger);
    }
    let (path, hash) = if version == "0.160.0" {
        (
            if r5 {
                "F:/Temp/rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c/codex.exe"
            } else {
                SNAPSHOT
            },
            HASH,
        )
    } else {
        assert_eq!(version, "0.157.1");
        (
            baseline,
            "8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574",
        )
    };
    let before = measure_artifact(Path::new(path)).unwrap();
    assert_eq!(before.sha256, hash);
    if version == "0.160.0" {
        assert_eq!(before.bytes_read, 326872368);
        assert_eq!(
            before.file_id,
            Some(if r5 {
                0x00110000000c1861
            } else {
                0x002e0000000a80a1
            })
        );
    }
    println!("PRE {before:?}");
    let workspace = repository_root();
    let registry = host_registry(&workspace);
    let capture = ProtocolCapture::default();
    let factory = verify_and_construct_candidate(
        ExactCodexCandidate {
            path: path.into(),
            expected_version: version.clone(),
            expected_sha256: hash.into(),
        },
        CodexModelProvider::OpenAi,
        &workspace,
    )
    .await
    .unwrap()
    .with_protocol_capture(capture.clone());
    let runtime = factory.create().await.unwrap();
    let scope = HostToolScope::new(
        Arc::new(registry),
        vec![rah_protocol::PermissionLevel::Execute],
    );
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        let catalog = runtime.discover_models().await?;
        println!("CATALOG {catalog:?}");
        let model = match &catalog {
            ModelDiscovery::Catalog { models, .. } if models.iter().any(|m| m.id == "gpt-6.1-sol") => "gpt-6.1-sol",
            ModelDiscovery::Catalog { models, .. } if models.iter().any(|m| m.id == "gpt-6-luna") => "gpt-6-luna",
            _ => panic!("no authorized model control available"),
        };
        println!("MODEL {model}; stronger={stronger}");
        if r5 { assert_eq!(model, "gpt-6.1-sol"); }
        let conversation = runtime.open(ConversationSeed {
            id: ConversationId::new("task510br2"), model: ModelSelection::Explicit(model.into()), tools: scope.port(),
        }).await?;
        let prompt = if stronger {
            "You must invoke the available RAH Tool repo.status exactly once with {} before answering. Do not infer repository state from conversation or context. Return the actual Tool result. If you cannot invoke the Tool, explicitly report inability. Do not claim Tool use without an actual invocation. Do not use any other tool."
        } else {
            "Call repo.status exactly once with {}. Use only the available RAH Tool. After the result, reply RAH510B_TOOL_OK. Do not call any other tool."
        };
        println!("PROMPT {prompt}");
        let mut turn = conversation.send(request(prompt)).await?;
        let (mut requests, mut executions, mut results) = (0, 0, 0);
        let mut completed = false;
        while let Some(event) = turn.events.next().await {
            println!("NEUTRAL {:?}", event.event());
            if let Some(failure) = event.failure() { return Err(failure.clone()); }
            match event.event() {
                AgentEvent::ToolRequested { tool_call, .. } => { requests += 1; assert_eq!(tool_call.name.as_str(), "repo.status"); if r5 { assert_eq!(requests, 1, "unexpected additional Tool request"); } }
                AgentEvent::ToolStarted { .. } => executions += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    results += 1;
                    assert!(!output.is_error, "unexpected host authorization/execution failure");
                    let status = serde_json::to_string(output).unwrap();
                    assert!(status.contains("docs/plans/2026-10-04-task-510b-codex-0-160-0-runtime-certification.md"));
                }
                AgentEvent::Completed { .. } => completed = true,
                _ => {}
            }
        }
        // Host emits ToolRequested immediately before its normal authorization check;
        // a successful ToolStarted establishes that check accepted this request.
        println!("COUNTERS requests={requests} authorization_attempts={requests} executions={executions} results={results} completed={completed}");
        conversation.close().await?;
        Ok::<_, rah_runtime::RuntimeFailure>(())
    }).await;
    scope.revoke();
    scope.drained().await;
    let shutdown = tokio::time::timeout(Duration::from_secs(20), runtime.shutdown()).await;
    let suffix = if stronger { "strong" } else { "original" };
    let evidence = if r5 {
        "F:/Temp/rah-task510br5a-evidence/r5-tool-wire.json".to_owned()
    } else {
        format!("F:/Temp/rah-task510br2-evidence/{version}-{suffix}-wire.json")
    };
    std::fs::write(
        &evidence,
        serde_json::to_vec_pretty(
            &capture
                .records()
                .into_iter()
                .map(|(outgoing, message)| {
                    (
                        outgoing,
                        serde_json::from_str::<serde_json::Value>(&message).unwrap(),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .unwrap(),
    )
    .unwrap();
    let after = measure_artifact(Path::new(path)).unwrap();
    println!(
        "POST {after:?}; shutdown={shutdown:?}; alive={}; result={result:?}; wire={evidence}",
        runtime.is_alive()
    );
    assert_eq!(before, after);
    assert!(matches!(shutdown, Ok(Ok(()))));
    assert!(matches!(result, Ok(Ok(()))));
}
#[tokio::test]
#[ignore = "Task 510B-R5D fresh ordinary production rejection; no inference"]
async fn r5d_production_rejection() {
    use rah_runtime::experimental::ConfiguredRuntimeFactory;
    use std::error::Error;
    let path = std::path::PathBuf::from(
        "F:/Temp/rah-codex-certification-bundle-b2cdfb57-d558-466d-a6cb-83a1c9d3224c/codex.exe",
    );
    let factory = rah_runtime_codex::experimental::CodexFactory::new(
        path,
        rah_runtime_codex::CodexModelProvider::OpenAi,
    );
    let error = match factory.create().await {
        Err(error) => error,
        Ok(runtime) => {
            let _ = runtime.shutdown().await;
            panic!("ordinary production admitted uncertified candidate");
        }
    };
    assert!(
        matches!(error.source().and_then(|source| source.downcast_ref::<rah_runtime_codex::CodexAdapterError>()), Some(rah_runtime_codex::CodexAdapterError::VersionMismatch { actual, .. }) if actual == "codex-cli 0.160.0")
    );
    assert_eq!(
        rah_runtime_codex::CURRENT_CERTIFIED_CODEX_VERSIONS,
        &["codex-cli 0.157.1"]
    );
    assert_eq!(
        rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION,
        "codex-cli 0.157.1"
    );
    println!(
        "PRODUCTION REJECTION PASS: version validation before app-server startup; inference=0; Tool=0"
    );
}
