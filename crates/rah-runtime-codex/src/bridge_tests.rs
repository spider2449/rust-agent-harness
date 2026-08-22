use std::{
    fs,
    future::pending,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use futures::StreamExt;
use rah_cli::profile_composition::{EffectiveProfileComposition, compose};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput,
};
use rah_runtime::{AgentHandle, AgentRuntime};
use rah_tools::{
    EchoTool, FsReadTool, Tool, ToolContext, ToolError, ToolRegistry, TrustedStaticProfile,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;
use tokio::time::{Duration, timeout};

use crate::{
    runtime::CodexRuntime,
    test_support::{FakePeer, fake_transport},
};

struct CountingEcho {
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingEcho {
    fn definition(&self) -> ToolDefinition {
        EchoTool::new().definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        EchoTool::new().execute(input, context).await
    }
}

struct BlockingEcho {
    executions: Arc<AtomicUsize>,
    started: Arc<Notify>,
    dropped: Arc<AtomicUsize>,
}

struct LookupTool {
    executions: Arc<AtomicUsize>,
}

struct MutablePermissionTool {
    permission: Arc<AtomicU8>,
    executions: Arc<AtomicUsize>,
}

struct CountingFsRead {
    tool: FsReadTool,
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingFsRead {
    fn definition(&self) -> ToolDefinition {
        self.tool.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.tool.execute(input, context).await
    }
}

static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should follow Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-codex-fs-read-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory should be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct RepositoryFixture {
    _base: TestDirectory,
    root: PathBuf,
    target: PathBuf,
    unrelated: PathBuf,
    profile: PathBuf,
}

impl RepositoryFixture {
    fn new(label: &str) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "rah-test@example.invalid"]);
        git(&root, &["config", "user.name", "RAH bridge test"]);
        let target = root.join("target.txt");
        let unrelated = root.join("unrelated.txt");
        fs::write(&target, b"alpha\nold\nomega\n").expect("target should be written");
        fs::write(&unrelated, b"unrelated\n").expect("unrelated file should be written");
        git(&root, &["add", "--", "target.txt", "unrelated.txt"]);
        git(&root, &["commit", "--quiet", "-m", "fixture"]);

        let profile = root.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "task051-bridge",
            "resources": {
                "executables": { "git": { "path": git_executable(), "kind": "native" } },
                "repositories": { "worktree": { "path": &root } }
            },
            "capabilities": [{
                "name": "repo.patch",
                "enabled": true,
                "permission": "execute",
                "executable": "git",
                "repository": "worktree"
            }]
        });
        fs::write(
            &profile,
            serde_json::to_vec(&document).expect("profile JSON should serialize"),
        )
        .expect("trusted profile should be written");
        Self {
            _base: base,
            root,
            target,
            unrelated,
            profile,
        }
    }

    async fn compose(&self) -> EffectiveProfileComposition {
        compose(TrustedStaticProfile::load(&self.profile).expect("profile should load"))
            .await
            .expect("effective profile should compose")
    }

    fn request(&self, old: &str, replacement: &str) -> Value {
        let bytes = fs::read(&self.target).expect("target should be readable");
        json!({
            "path": "target.txt",
            "expected_file_sha256": sha256_hex(&bytes),
            "expected_file_byte_length": bytes.len(),
            "expected_old_text": old,
            "replacement_text": replacement,
        })
    }

    fn assert_clean_index(&self) {
        git(&self.root, &["diff", "--cached", "--exit-code"]);
    }

    fn assert_original_worktree(&self) {
        assert_eq!(fs::read(&self.target).unwrap(), b"alpha\nold\nomega\n");
        assert_eq!(fs::read(&self.unrelated).unwrap(), b"unrelated\n");
        self.assert_clean_index();
    }
}

struct GatedComposedPatch {
    inner: Arc<dyn Tool>,
    bridge_executions: Arc<AtomicUsize>,
    composed_executions: Arc<AtomicUsize>,
    before_inner: Option<Arc<Notify>>,
    after_inner: Option<Arc<Notify>>,
    before_gate: Option<Arc<Notify>>,
    after_gate: Option<Arc<Notify>>,
}

struct UncertainAfterComposedPatch {
    inner: Arc<dyn Tool>,
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for UncertainAfterComposedPatch {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(input, context).await?;
        Ok(ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": "uncertain",
                "changed": false,
                "uncertain": true,
                "reason": "replacement",
            }))],
            is_error: true,
        })
    }
}

#[async_trait]
impl Tool for GatedComposedPatch {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.bridge_executions.fetch_add(1, Ordering::SeqCst);
        if let Some(before_inner) = &self.before_inner {
            before_inner.notify_one();
        }
        if let Some(gate) = &self.before_gate {
            gate.notified().await;
        }
        self.composed_executions.fetch_add(1, Ordering::SeqCst);
        let output = self.inner.execute(input, context).await;
        if let Some(after_inner) = &self.after_inner {
            after_inner.notify_one();
            if let Some(gate) = &self.after_gate {
                gate.notified().await;
            }
        }
        output
    }
}

#[async_trait]
impl Tool for MutablePermissionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("mutable_permission"),
            description: "Tests execution-time permission lookup.".to_owned(),
            input_schema: json!({"type": "object"}),
            permission: match self.permission.load(Ordering::SeqCst) {
                0 => PermissionLevel::None,
                _ => PermissionLevel::Read,
            },
        }
    }

    async fn execute(
        &self,
        _input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: vec![ToolContent::Text("current permission accepted".to_owned())],
            is_error: false,
        })
    }
}

#[async_trait]
impl Tool for LookupTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("record.lookup"),
            description: "Looks up a deterministic test record.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "fail": { "type": "boolean" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: vec![ToolContent::Json(json!({
                "record": input.0["id"],
            }))],
            is_error: input.0["fail"].as_bool().unwrap_or(false),
        })
    }
}

struct DropMark(Arc<AtomicUsize>);

impl Drop for DropMark {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl Tool for BlockingEcho {
    fn definition(&self) -> ToolDefinition {
        EchoTool::new().definition()
    }

    async fn execute(
        &self,
        _input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        let _drop_mark = DropMark(Arc::clone(&self.dropped));
        self.started.notify_one();
        pending().await
    }
}

#[tokio::test]
async fn experimental_api_and_generic_tool_definitions_are_bridge_only() {
    let (transport, mut peer) = fake_transport();
    let connecting = tokio::spawn(CodexRuntime::from_transport(transport));
    let initialize = peer.respond("initialize", json!({})).await;
    assert_eq!(
        initialize["params"]["capabilities"]["experimentalApi"],
        false
    );
    peer.expect_notification("initialized").await;
    let restricted = connecting.await.expect("connection task").expect("runtime");
    restricted.shutdown().await.expect("shutdown");

    let (runtime, mut peer, initialize) = connected_bridge(
        generic_registry(Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))),
        vec![PermissionLevel::None, PermissionLevel::Read],
    )
    .await;
    assert_eq!(
        initialize["params"]["capabilities"]["experimentalApi"],
        true
    );
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    assert_eq!(
        thread["params"]["dynamicTools"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        thread["params"]["dynamicTools"],
        json!([
        {
            "type": "function",
            "name": "echo",
            "description": "Returns the supplied text unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false
            },
            "deferLoading": false
        },
        {
            "type": "function",
            "name": "rah_tool_1",
            "description": "Looks up a deterministic test record.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "fail": { "type": "boolean" }
                },
                "required": ["id"],
                "additionalProperties": false
            },
            "deferLoading": false
        }
        ])
    );
    assert_restrictions(&thread["params"]);
    finish_turn(&peer, "completed");
    let _ = handle.into_events().collect::<Vec<_>>().await;
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn fs_read_is_advertised_by_private_alias_and_resolves_to_exact_rah_name() {
    let workspace = TestDirectory::new();
    fs::write(workspace.path().join("note.txt"), "workspace text")
        .expect("workspace file should be written");
    let (runtime, mut peer, _) = connected_bridge(
        fs_read_registry(workspace.path(), 64, None),
        vec![PermissionLevel::Read],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let advertised = &thread["params"]["dynamicTools"][0];
    let alias = advertised["name"]
        .as_str()
        .expect("advertised alias should be a string");

    assert_eq!(alias, "rah_tool_0");
    assert_ne!(alias, "fs.read");
    assert!(!alias.contains('.'));
    assert!(
        alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    );
    assert_eq!(
        advertised,
        &json!({
            "type": "function",
            "name": "rah_tool_0",
            "description": "Reads a UTF-8 text file within the configured workspace.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
                "additionalProperties": false
            },
            "deferLoading": false
        })
    );
    assert!(advertised.get("permission").is_none());
    assert_restrictions(&thread["params"]);

    peer.send(tool_request(
        json!(80),
        "private-thread",
        "private-turn",
        "fs-read-success",
        alias,
        json!({"path": "note.txt"}),
    ));
    assert_eq!(
        peer.next_sent().await["result"],
        json!({
            "contentItems": [{ "type": "inputText", "text": "workspace text" }],
            "success": true
        })
    );
    peer.notify("item/started", dynamic_item());
    peer.notify("item/completed", dynamic_item());
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(tool_event_count(&events), 3);
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolRequested { tool_call, .. }
            if tool_call.name == ToolName::new("fs.read")
                && tool_call.input == ToolInput(json!({"path": "note.txt"}))
    )));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn fs_read_requires_explicit_read_permission() {
    for (allowed, should_succeed) in [
        (vec![PermissionLevel::None], false),
        (vec![PermissionLevel::Read], true),
    ] {
        let workspace = TestDirectory::new();
        fs::write(workspace.path().join("note.txt"), "allowed")
            .expect("workspace file should be written");
        let (runtime, mut peer, _) =
            connected_bridge(fs_read_registry(workspace.path(), 64, None), allowed).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(81),
            "private-thread",
            "private-turn",
            "permission",
            "rah_tool_0",
            json!({"path": "note.txt"}),
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], should_succeed);

        let events = if should_succeed {
            finish_turn(&peer, "completed");
            handle.into_events().collect::<Vec<_>>().await
        } else {
            let collecting =
                tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
            peer.respond("turn/interrupt", json!({})).await;
            collecting.await.expect("event collector")
        };
        if should_succeed {
            assert_eq!(tool_event_count(&events), 3);
        } else {
            assert!(matches!(
                events.last(),
                Some(AgentEvent::Failed {
                    code: rah_protocol::AgentErrorCode::PermissionDenied,
                    ..
                })
            ));
            assert_eq!(tool_event_count(&events), 1);
        }
        runtime.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn fs_read_preserves_workspace_input_binary_and_size_enforcement() {
    let base = TestDirectory::new();
    let workspace = base.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace should be created");
    let outside = base.path().join("outside.txt");
    fs::write(&outside, "outside").expect("outside file should be written");
    fs::write(workspace.join("binary.dat"), [0, 1, 2]).expect("binary file should be written");
    fs::write(workspace.join("invalid-utf8.dat"), [0xff])
        .expect("invalid UTF-8 file should be written");
    fs::write(workspace.join("large.txt"), "12345").expect("large file should be written");
    let outside_absolute = outside.to_string_lossy().into_owned();

    for (call_id, arguments) in [
        ("parent-escape", json!({"path": "../outside.txt"})),
        ("absolute-outside", json!({"path": outside_absolute})),
        ("missing-input", json!({})),
        ("missing-file", json!({"path": "missing.txt"})),
        ("binary", json!({"path": "binary.dat"})),
        ("invalid-utf8", json!({"path": "invalid-utf8.dat"})),
        ("maximum-bytes", json!({"path": "large.txt"})),
    ] {
        let (runtime, mut peer, _) = connected_bridge(
            fs_read_registry(&workspace, 4, None),
            vec![PermissionLevel::Read],
        )
        .await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(82),
            "private-thread",
            "private-turn",
            call_id,
            "rah_tool_0",
            arguments,
        ));
        assert_eq!(peer.next_sent().await["result"]["success"], false);
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.expect("event collector");
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Failed {
                code: rah_protocol::AgentErrorCode::Tool,
                ..
            })
        ));
        assert_eq!(tool_event_count(&events), 2);
        runtime.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn duplicate_and_wrong_route_fs_read_requests_execute_exactly_once() {
    let workspace = TestDirectory::new();
    fs::write(workspace.path().join("once.txt"), "once").expect("workspace file should be written");
    let executions = Arc::new(AtomicUsize::new(0));
    let (runtime, mut peer, _) = connected_bridge(
        fs_read_registry(workspace.path(), 64, Some(Arc::clone(&executions))),
        vec![PermissionLevel::Read],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let arguments = json!({"path": "once.txt"});

    for (request_id, thread, turn) in [
        (json!(83), "wrong-thread", "private-turn"),
        (json!(84), "private-thread", "wrong-turn"),
    ] {
        peer.send(tool_request(
            request_id,
            thread,
            turn,
            "wrong-route",
            "rah_tool_0",
            arguments.clone(),
        ));
        assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    }
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    for request_id in [json!(85), json!("duplicate-fs-read")] {
        peer.send(tool_request(
            request_id,
            "private-thread",
            "private-turn",
            "same-fs-read",
            "rah_tool_0",
            arguments.clone(),
        ));
    }
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(first["result"]["success"], true);

    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(tool_event_count(&events), 3);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn aliases_dispatch_distinct_tools_and_map_json_error_outputs() {
    let echo_executions = Arc::new(AtomicUsize::new(0));
    let lookup_executions = Arc::new(AtomicUsize::new(0));
    let (runtime, mut peer, _) = connected_bridge(
        generic_registry(Arc::clone(&echo_executions), Arc::clone(&lookup_executions)),
        vec![PermissionLevel::None, PermissionLevel::Read],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;

    peer.send(tool_request(
        json!(70),
        "private-thread",
        "private-turn",
        "lookup-success",
        "rah_tool_1",
        json!({"id": 7}),
    ));
    assert_eq!(
        peer.next_sent().await["result"],
        json!({
            "contentItems": [{ "type": "inputText", "text": "{\"record\":7}" }],
            "success": true
        })
    );
    peer.send(tool_request(
        json!(71),
        "private-thread",
        "private-turn",
        "lookup-error-output",
        "rah_tool_1",
        json!({"id": 8, "fail": true}),
    ));
    assert_eq!(
        peer.next_sent().await["result"],
        json!({
            "contentItems": [{ "type": "inputText", "text": "{\"record\":8}" }],
            "success": false
        })
    );

    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(echo_executions.load(Ordering::SeqCst), 0);
    assert_eq!(lookup_executions.load(Ordering::SeqCst), 2);
    assert_eq!(tool_event_count(&events), 6);
    assert!(events.iter().all(|event| match event {
        AgentEvent::ToolRequested { tool_call, .. } => {
            tool_call.name == ToolName::new("record.lookup")
        }
        _ => true,
    }));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn permission_is_read_from_the_registered_definition_at_execution_time() {
    let permission = Arc::new(AtomicU8::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(MutablePermissionTool {
            permission: Arc::clone(&permission),
            executions: Arc::clone(&executions),
        }))
        .expect("register mutable permission tool");
    let (runtime, mut peer, _) =
        connected_bridge(Arc::new(registry), vec![PermissionLevel::Read]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;

    permission.store(1, Ordering::SeqCst);
    peer.send(tool_request(
        json!(72),
        "private-thread",
        "private-turn",
        "permission-refresh",
        "mutable_permission",
        json!({}),
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], true);

    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(tool_event_count(&events), 3);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn string_and_integer_request_ids_route_echo_through_registry() {
    let executions = Arc::new(AtomicUsize::new(0));
    let (runtime, mut peer, _) = connected_bridge(
        counting_registry(Arc::clone(&executions)),
        vec![PermissionLevel::None],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;

    for (call_id, request_id, text) in [
        ("call-int", json!(60), "integer"),
        ("call-negative-int", json!(-7), "negative integer"),
        ("call-string", json!("rpc-string"), "string"),
    ] {
        peer.send(tool_request(
            request_id.clone(),
            "private-thread",
            "private-turn",
            call_id,
            "echo",
            json!({"text": text}),
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["id"], request_id);
        assert_eq!(
            response["result"],
            json!({
                "contentItems": [{ "type": "inputText", "text": text }],
                "success": true
            })
        );
    }
    peer.notify("item/started", dynamic_item());
    peer.notify("item/completed", dynamic_item());
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(executions.load(Ordering::SeqCst), 3);
    assert_eq!(tool_event_count(&events), 9);
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolRequested { tool_call, .. }
            if tool_call.input == ToolInput(json!({"text": "integer"}))
    )));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn duplicate_call_executes_once_and_reuses_the_response() {
    let executions = Arc::new(AtomicUsize::new(0));
    let (runtime, mut peer, _) = connected_bridge(
        counting_registry(Arc::clone(&executions)),
        vec![PermissionLevel::None],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let params = (
        "private-thread",
        "private-turn",
        "same-call",
        "echo",
        json!({"text": "once"}),
    );
    peer.send(tool_request(
        json!(1),
        params.0,
        params.1,
        params.2,
        params.3,
        params.4.clone(),
    ));
    peer.send(tool_request(
        json!("duplicate"),
        params.0,
        params.1,
        params.2,
        params.3,
        params.4,
    ));
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert!(first["result"]["success"].as_bool().unwrap_or(false));
    assert_eq!(first["result"], second["result"]);
    peer.send(tool_request(
        json!("duplicate"),
        params.0,
        params.1,
        params.2,
        params.3,
        json!({"text": "once"}),
    ));
    assert!(
        timeout(Duration::from_millis(20), peer.next_sent())
            .await
            .is_err()
    );
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(tool_event_count(&events), 3);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn malformed_wrong_route_and_unknown_tool_fail_closed() {
    let executions = Arc::new(AtomicUsize::new(0));
    let (runtime, mut peer, _) = connected_bridge(
        counting_registry(Arc::clone(&executions)),
        vec![PermissionLevel::None],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;

    peer.send(json!({"id": "malformed", "method": "item/tool/call", "params": null}));
    assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    peer.send(tool_request(
        json!(2),
        "wrong-thread",
        "private-turn",
        "wrong-thread",
        "echo",
        json!({"text": "no"}),
    ));
    assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    peer.send(tool_request(
        json!(3),
        "private-thread",
        "wrong-turn",
        "wrong-turn",
        "echo",
        json!({"text": "no"}),
    ));
    assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    peer.send(tool_request(
        json!(4),
        "private-thread",
        "private-turn",
        "unknown",
        "shell_exec",
        json!({}),
    ));
    let unknown = peer.next_sent().await;
    assert_eq!(unknown["result"]["success"], false);

    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    let interrupt = peer.respond("turn/interrupt", json!({})).await;
    assert_eq!(interrupt["params"]["turnId"], "private-turn");
    let events = collecting.await.expect("event collector");
    assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn invalid_input_and_permission_denial_happen_without_false_completion() {
    for (allowed, arguments, expected_code) in [
        (
            vec![PermissionLevel::None],
            json!({"text": 7}),
            rah_protocol::AgentErrorCode::Tool,
        ),
        (
            Vec::new(),
            json!({"text": "denied"}),
            rah_protocol::AgentErrorCode::PermissionDenied,
        ),
    ] {
        let executions = Arc::new(AtomicUsize::new(0));
        let (runtime, mut peer, _) =
            connected_bridge(counting_registry(Arc::clone(&executions)), allowed).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(8),
            "private-thread",
            "private-turn",
            "failure",
            "echo",
            arguments,
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.expect("event collector");
        assert!(
            matches!(events.last(), Some(AgentEvent::Failed { code, .. }) if *code == expected_code)
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, AgentEvent::ToolFinished { .. }))
        );
        let expected_executions = usize::from(expected_code == rah_protocol::AgentErrorCode::Tool);
        assert_eq!(executions.load(Ordering::SeqCst), expected_executions);
        runtime.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn cancellation_drops_pending_call_and_rejects_late_duplicate() {
    let executions = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicUsize::new(0));
    let registry = blocking_registry(
        Arc::clone(&executions),
        Arc::clone(&started),
        Arc::clone(&dropped),
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::None]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let session_id = handle.session_id().clone();
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(9),
        "private-thread",
        "private-turn",
        "pending",
        "echo",
        json!({"text": "wait"}),
    ));
    started.notified().await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    let mut saw_call_denial = false;
    let mut saw_interrupt = false;
    while !saw_call_denial || !saw_interrupt {
        let message = peer.next_sent().await;
        if message.get("method") == Some(&json!("turn/interrupt")) {
            let id = message["id"].clone();
            peer.send(json!({"id": id, "result": {}}));
            saw_interrupt = true;
        } else if message["id"] == 9 {
            assert_eq!(message["error"]["code"], -32800);
            saw_call_denial = true;
        }
    }
    finish_turn(&peer, "interrupted");
    cancelling.await.expect("cancel task").expect("cancel");
    let events = collecting.await.expect("event collector");
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    peer.send(tool_request(
        json!(10),
        "private-thread",
        "private-turn",
        "pending",
        "echo",
        json!({"text": "wait"}),
    ));
    assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn disconnect_cancels_pending_execution_without_replay() {
    let executions = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicUsize::new(0));
    let registry = blocking_registry(
        Arc::clone(&executions),
        Arc::clone(&started),
        Arc::clone(&dropped),
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::None]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(11),
        "private-thread",
        "private-turn",
        "disconnect",
        "echo",
        json!({"text": "wait"}),
    ));
    started.notified().await;
    peer.fail(crate::CodexAdapterError::ProtocolViolation {
        message: "fixture disconnect".to_owned(),
    });
    let events = collecting.await.expect("event collector");
    assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
    tokio::task::yield_now().await;
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    drop(runtime);
}

#[tokio::test]
async fn trusted_profile_composed_repo_patch_preserves_execute_alias_single_execution_and_redaction()
 {
    let fixture = RepositoryFixture::new("success");
    let composition = fixture.compose().await;
    let definition = composition
        .registry()
        .definitions()
        .into_iter()
        .next()
        .expect("effective composer should publish repo.patch");
    assert_eq!(definition.name, ToolName::new("repo.patch"));
    assert_eq!(definition.permission, PermissionLevel::Execute);

    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let alias = thread["params"]["dynamicTools"][0]["name"]
        .as_str()
        .expect("repo.patch alias should be a string")
        .to_owned();
    assert_eq!(alias, "rah_tool_0");
    assert_ne!(alias, "repo.patch");
    assert_restrictions(&thread["params"]);

    let arguments = fixture.request("old", "new");
    peer.send(tool_request(
        json!(501),
        "private-thread",
        "private-turn",
        "repo-patch-once",
        &alias,
        arguments.clone(),
    ));
    peer.send(tool_request(
        json!("task051-duplicate"),
        "private-thread",
        "private-turn",
        "repo-patch-once",
        &alias,
        arguments,
    ));
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(first["result"]["success"], true);
    let visible = first["result"]["contentItems"][0]["text"]
        .as_str()
        .expect("tool output should be translated as text");
    assert_eq!(
        serde_json::from_str::<Value>(visible).unwrap(),
        json!({"status":"ok","changed":true,"uncertain":false,"reason":"none"})
    );
    for private_value in [
        fixture.root.to_string_lossy().into_owned(),
        fixture.target.to_string_lossy().into_owned(),
        git_executable().to_string_lossy().into_owned(),
        "alpha".to_owned(),
        "old".to_owned(),
        "new".to_owned(),
    ] {
        assert!(
            !visible.contains(&private_value),
            "model-visible response leaked {private_value:?}"
        );
    }
    assert_eq!(fs::read(&fixture.target).unwrap(), b"alpha\nnew\nomega\n");
    assert_eq!(fs::read(&fixture.unrelated).unwrap(), b"unrelated\n");
    fixture.assert_clean_index();

    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::ToolRequested { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::ToolStarted { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::ToolFinished { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolRequested { tool_call, .. }
            if tool_call.name == ToolName::new("repo.patch")
    )));
    runtime.shutdown().await.expect("shutdown");
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_patch_requires_execute_and_refuses_without_mutation() {
    for allowed in [
        vec![PermissionLevel::None],
        vec![PermissionLevel::Read],
        vec![PermissionLevel::Write],
    ] {
        let fixture = RepositoryFixture::new("permission-denied");
        let composition = fixture.compose().await;
        let (runtime, mut peer, _) = connected_bridge(composition.registry_handle(), allowed).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(502),
            "private-thread",
            "private-turn",
            "permission-denied",
            "rah_tool_0",
            fixture.request("old", "new"),
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        assert_eq!(
            response["result"]["contentItems"][0]["text"],
            "RAH permission policy denied the dynamic tool call"
        );
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.expect("event collector");
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Failed {
                code: rah_protocol::AgentErrorCode::PermissionDenied,
                ..
            })
        ));
        assert_eq!(tool_event_count(&events), 1);
        fixture.assert_original_worktree();
        runtime.shutdown().await.expect("shutdown");
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn trusted_profile_composed_repo_patch_refusals_are_translated_without_retry_or_evidence_leakage()
 {
    for (label, request) in [
        ("stale", RequestKind::StaleDigest),
        ("duplicate", RequestKind::DuplicateExpectedText),
        ("staged", RequestKind::StagedTarget),
        ("path", RequestKind::InvalidPath),
    ] {
        let fixture = RepositoryFixture::new(label);
        if matches!(request, RequestKind::StagedTarget) {
            fs::write(&fixture.target, b"alpha\nstaged\nomega\n").unwrap();
            git(&fixture.root, &["add", "--", "target.txt"]);
            fs::write(&fixture.target, b"alpha\nold\nomega\n").unwrap();
        }
        let composition = fixture.compose().await;
        let (runtime, mut peer, _) = connected_bridge(
            composition.registry_handle(),
            vec![PermissionLevel::Execute],
        )
        .await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        let mut arguments = fixture.request("old", "new");
        match request {
            RequestKind::StaleDigest => arguments["expected_file_sha256"] = json!("0".repeat(64)),
            RequestKind::DuplicateExpectedText => arguments["expected_old_text"] = json!("a"),
            RequestKind::StagedTarget => {}
            RequestKind::InvalidPath => arguments["path"] = json!(fixture.target),
        }
        peer.send(tool_request(
            json!(503),
            "private-thread",
            "private-turn",
            label,
            "rah_tool_0",
            arguments,
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        let visible = response["result"]["contentItems"][0]["text"]
            .as_str()
            .expect("translated refusal should be text");
        if matches!(request, RequestKind::InvalidPath) {
            assert_eq!(visible, "RAH tool execution failed");
        } else {
            let public =
                serde_json::from_str::<Value>(visible).expect("refusal should remain JSON");
            assert_eq!(public["status"], "precondition_failed");
            assert_eq!(public["changed"], false);
            assert_eq!(public["uncertain"], false);
        }
        assert!(!visible.contains(fixture.root.to_string_lossy().as_ref()));
        assert!(!visible.contains("alpha"));
        let events = if matches!(request, RequestKind::InvalidPath) {
            let collecting =
                tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
            peer.respond("turn/interrupt", json!({})).await;
            collecting.await.expect("event collector")
        } else {
            finish_turn(&peer, "completed");
            handle.into_events().collect::<Vec<_>>().await
        };
        assert_eq!(
            tool_event_count(&events),
            if matches!(request, RequestKind::InvalidPath) {
                2
            } else {
                3
            }
        );
        assert_eq!(fs::read(&fixture.target).unwrap(), b"alpha\nold\nomega\n");
        assert_eq!(fs::read(&fixture.unrelated).unwrap(), b"unrelated\n");
        if !matches!(request, RequestKind::StagedTarget) {
            fixture.assert_clean_index();
        }
        runtime.shutdown().await.expect("shutdown");
        composition.shutdown().await;
    }
}

#[derive(Clone, Copy)]
enum RequestKind {
    StaleDigest,
    DuplicateExpectedText,
    StagedTarget,
    InvalidPath,
}

#[tokio::test]
async fn trusted_profile_composed_repo_patch_cancellation_never_replays_before_or_after_mutation() {
    let before = RepositoryFixture::new("cancel-before");
    let composition = before.compose().await;
    let bridge_executions = Arc::new(AtomicUsize::new(0));
    let composed_executions = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let gate = Arc::new(Notify::new());
    let registry = gated_composed_patch_registry(
        &composition,
        Arc::clone(&bridge_executions),
        Arc::clone(&composed_executions),
        Some(Arc::clone(&entered)),
        None,
        Some(Arc::clone(&gate)),
        None,
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let session_id = handle.session_id().clone();
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(504),
        "private-thread",
        "private-turn",
        "cancel-before",
        "rah_tool_0",
        before.request("old", "new"),
    ));
    entered.notified().await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    await_cancellation(&mut peer, json!(504)).await;
    finish_turn(&peer, "interrupted");
    cancelling.await.unwrap().expect("cancel should succeed");
    let events = collecting.await.unwrap();
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));
    assert_eq!(bridge_executions.load(Ordering::SeqCst), 1);
    assert_eq!(composed_executions.load(Ordering::SeqCst), 0);
    before.assert_original_worktree();
    runtime.shutdown().await.expect("shutdown");
    composition.shutdown().await;

    let after = RepositoryFixture::new("cancel-after");
    let composition = after.compose().await;
    let bridge_executions = Arc::new(AtomicUsize::new(0));
    let composed_executions = Arc::new(AtomicUsize::new(0));
    let after_inner = Arc::new(Notify::new());
    let after_gate = Arc::new(Notify::new());
    let registry = gated_composed_patch_registry(
        &composition,
        Arc::clone(&bridge_executions),
        Arc::clone(&composed_executions),
        None,
        Some(Arc::clone(&after_inner)),
        None,
        Some(Arc::clone(&after_gate)),
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let session_id = handle.session_id().clone();
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(505),
        "private-thread",
        "private-turn",
        "cancel-after",
        "rah_tool_0",
        after.request("old", "new"),
    ));
    after_inner.notified().await;
    assert_eq!(fs::read(&after.target).unwrap(), b"alpha\nnew\nomega\n");
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    await_cancellation(&mut peer, json!(505)).await;
    finish_turn(&peer, "interrupted");
    cancelling.await.unwrap().expect("cancel should succeed");
    let events = collecting.await.unwrap();
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));
    assert_eq!(bridge_executions.load(Ordering::SeqCst), 1);
    assert_eq!(composed_executions.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&after.unrelated).unwrap(), b"unrelated\n");
    after.assert_clean_index();
    runtime.shutdown().await.expect("shutdown");
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_patch_disconnect_after_mutation_does_not_replay() {
    let fixture = RepositoryFixture::new("disconnect");
    let composition = fixture.compose().await;
    let bridge_executions = Arc::new(AtomicUsize::new(0));
    let composed_executions = Arc::new(AtomicUsize::new(0));
    let after_inner = Arc::new(Notify::new());
    let after_gate = Arc::new(Notify::new());
    let registry = gated_composed_patch_registry(
        &composition,
        Arc::clone(&bridge_executions),
        Arc::clone(&composed_executions),
        None,
        Some(Arc::clone(&after_inner)),
        None,
        Some(Arc::clone(&after_gate)),
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(506),
        "private-thread",
        "private-turn",
        "disconnect-after",
        "rah_tool_0",
        fixture.request("old", "new"),
    ));
    after_inner.notified().await;
    assert_eq!(fs::read(&fixture.target).unwrap(), b"alpha\nnew\nomega\n");
    peer.fail(crate::CodexAdapterError::ProtocolViolation {
        message: "task051 deterministic disconnect".to_owned(),
    });
    let events = collecting.await.unwrap();
    assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
    tokio::task::yield_now().await;
    assert_eq!(bridge_executions.load(Ordering::SeqCst), 1);
    assert_eq!(composed_executions.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&fixture.unrelated).unwrap(), b"unrelated\n");
    fixture.assert_clean_index();
    drop(runtime);
    composition.shutdown().await;
}

#[tokio::test]
async fn uncertain_composed_patch_outcome_is_returned_once_without_bridge_retry() {
    let fixture = RepositoryFixture::new("uncertain");
    let composition = fixture.compose().await;
    let executions = Arc::new(AtomicUsize::new(0));
    let inner = composition
        .registry()
        .get(&ToolName::new("repo.patch"))
        .expect("composer should own repo.patch");
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(UncertainAfterComposedPatch {
            inner,
            executions: Arc::clone(&executions),
        }))
        .unwrap();
    let (runtime, mut peer, _) =
        connected_bridge(Arc::new(registry), vec![PermissionLevel::Execute]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let request = fixture.request("old", "new");
    for id in [json!(507), json!("uncertain-duplicate")] {
        peer.send(tool_request(
            id,
            "private-thread",
            "private-turn",
            "uncertain-once",
            "rah_tool_0",
            request.clone(),
        ));
    }
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(first["result"]["success"], false);
    assert_eq!(
        serde_json::from_str::<Value>(first["result"]["contentItems"][0]["text"].as_str().unwrap())
            .unwrap(),
        json!({"status":"uncertain","changed":false,"uncertain":true,"reason":"replacement"})
    );
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&fixture.target).unwrap(), b"alpha\nnew\nomega\n");
    fixture.assert_clean_index();
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(tool_event_count(&events), 3);
    runtime.shutdown().await.expect("shutdown");
    composition.shutdown().await;
}

#[tokio::test]
async fn codex_owned_requests_and_items_remain_denied_in_bridge_mode() {
    for method in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "mcpServer/elicitation/request",
        "future/experimental/request",
    ] {
        let (runtime, mut peer, _) = connected_bridge(
            counting_registry(Arc::new(AtomicUsize::new(0))),
            vec![PermissionLevel::None],
        )
        .await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(json!({"id": "denied", "method": method, "params": {"threadId": "private-thread", "turnId": "private-turn"}}));
        let denial = peer.next_sent().await;
        assert_eq!(denial["error"]["code"], -32601);
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.expect("event collector");
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Failed {
                code: rah_protocol::AgentErrorCode::PermissionDenied,
                ..
            })
        ));
        assert_eq!(tool_event_count(&events), 0);
        runtime.shutdown().await.expect("shutdown");
    }

    for item_type in ["commandExecution", "fileChange", "mcpToolCall"] {
        let (runtime, mut peer, _) = connected_bridge(
            counting_registry(Arc::new(AtomicUsize::new(0))),
            vec![PermissionLevel::None],
        )
        .await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.notify("item/started", json!({"threadId": "private-thread", "turnId": "private-turn", "item": {"id": "blocked", "type": item_type}}));
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.expect("event collector");
        assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
        assert_eq!(tool_event_count(&events), 0);
        runtime.shutdown().await.expect("shutdown");
    }
}

async fn connected_bridge(
    registry: Arc<ToolRegistry>,
    allowed: Vec<PermissionLevel>,
) -> (Arc<CodexRuntime>, FakePeer, Value) {
    let (transport, mut peer) = fake_transport();
    let connecting = tokio::spawn(CodexRuntime::from_transport_bridge(
        transport, registry, allowed,
    ));
    let initialize = peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let runtime = connecting
        .await
        .expect("connection task")
        .expect("bridge runtime");
    (Arc::new(runtime), peer, initialize)
}

async fn start_bridge(runtime: &Arc<CodexRuntime>, peer: &mut FakePeer) -> (AgentHandle, Value) {
    let starting = {
        let runtime = Arc::clone(runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    let thread = peer
        .respond("thread/start", json!({"thread": {"id": "private-thread"}}))
        .await;
    peer.respond("turn/start", json!({"turn": {"id": "private-turn"}}))
        .await;
    let handle = starting.await.expect("start task").expect("agent handle");
    (handle, thread)
}

fn counting_registry(executions: Arc<AtomicUsize>) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(CountingEcho { executions }))
        .expect("register echo");
    Arc::new(registry)
}

fn generic_registry(
    echo_executions: Arc<AtomicUsize>,
    lookup_executions: Arc<AtomicUsize>,
) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(CountingEcho {
            executions: echo_executions,
        }))
        .expect("register echo");
    registry
        .register(Arc::new(LookupTool {
            executions: lookup_executions,
        }))
        .expect("register lookup");
    Arc::new(registry)
}

fn blocking_registry(
    executions: Arc<AtomicUsize>,
    started: Arc<Notify>,
    dropped: Arc<AtomicUsize>,
) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(BlockingEcho {
            executions,
            started,
            dropped,
        }))
        .expect("register echo");
    Arc::new(registry)
}

fn fs_read_registry(
    workspace: &Path,
    max_bytes: usize,
    executions: Option<Arc<AtomicUsize>>,
) -> Arc<ToolRegistry> {
    let tool = FsReadTool::new(workspace, max_bytes).expect("workspace should be valid");
    let mut registry = ToolRegistry::new();
    match executions {
        Some(executions) => registry
            .register(Arc::new(CountingFsRead { tool, executions }))
            .expect("register fs.read"),
        None => registry.register(Arc::new(tool)).expect("register fs.read"),
    }
    Arc::new(registry)
}

fn gated_composed_patch_registry(
    composition: &EffectiveProfileComposition,
    bridge_executions: Arc<AtomicUsize>,
    composed_executions: Arc<AtomicUsize>,
    before_inner: Option<Arc<Notify>>,
    after_inner: Option<Arc<Notify>>,
    before_gate: Option<Arc<Notify>>,
    after_gate: Option<Arc<Notify>>,
) -> Arc<ToolRegistry> {
    let inner = composition
        .registry()
        .get(&ToolName::new("repo.patch"))
        .expect("effective composer should register repo.patch");
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(GatedComposedPatch {
            inner,
            bridge_executions,
            composed_executions,
            before_inner,
            after_inner,
            before_gate,
            after_gate,
        }))
        .expect("test-only delegate should preserve repo.patch registration");
    Arc::new(registry)
}

async fn await_cancellation(peer: &mut FakePeer, call_id: Value) {
    let mut saw_call_denial = false;
    let mut saw_interrupt = false;
    while !saw_call_denial || !saw_interrupt {
        let message = peer.next_sent().await;
        if message.get("method") == Some(&json!("turn/interrupt")) {
            let id = message["id"].clone();
            peer.send(json!({"id": id, "result": {}}));
            saw_interrupt = true;
        } else if message["id"] == call_id {
            assert_eq!(message["error"]["code"], -32800);
            saw_call_denial = true;
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .expect("Git path should be UTF-8")
            .lines()
            .next()
            .expect("Git path should be present"),
    )
    .expect("Git path should canonicalize")
}

fn git(root: &Path, arguments: &[&str]) {
    let output = Command::new(git_executable())
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("Git command should start");
    assert!(
        output.status.success(),
        "Git command {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn tool_request(
    id: Value,
    thread: &str,
    turn: &str,
    call: &str,
    tool: &str,
    arguments: Value,
) -> Value {
    json!({
        "id": id,
        "method": "item/tool/call",
        "params": {
            "threadId": thread,
            "turnId": turn,
            "callId": call,
            "namespace": null,
            "tool": tool,
            "arguments": arguments
        }
    })
}

fn dynamic_item() -> Value {
    json!({
        "threadId": "private-thread",
        "turnId": "private-turn",
        "item": { "id": "dynamic", "type": "dynamicToolCall" }
    })
}

fn finish_turn(peer: &FakePeer, status: &str) {
    peer.notify(
        "turn/completed",
        json!({
            "threadId": "private-thread",
            "turn": { "id": "private-turn", "status": status, "items": [] }
        }),
    );
}

fn sample_request() -> AgentRequest {
    AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: "Use echo.".to_owned(),
            }],
        },
        options: AgentOptions::default(),
    }
}

fn tool_event_count(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AgentEvent::ToolRequested { .. }
                    | AgentEvent::ToolStarted { .. }
                    | AgentEvent::ToolFinished { .. }
            )
        })
        .count()
}

fn assert_restrictions(params: &Value) {
    assert_eq!(params["approvalPolicy"], "never");
    assert_eq!(params["sandbox"], "read-only");
    assert_eq!(params["config"]["features"]["shell_tool"], false);
    assert_eq!(params["config"]["features"]["unified_exec"], false);
    assert_eq!(params["config"]["tools"]["web_search"], false);
    assert_eq!(params["config"]["tools"]["view_image"], false);
    assert_eq!(params["config"]["apps"]["_default"]["enabled"], false);
    assert_eq!(params["config"]["mcp_servers"], json!({}));
}
