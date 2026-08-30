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

/// Test-only delegation keeps the production bridge generic while proving that
/// a real composed tool enters exactly once for each logical bridge call.
struct CountingComposedTool {
    inner: Arc<dyn Tool>,
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

#[async_trait]
impl Tool for CountingComposedTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(input, context).await
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

/// Disposable repository for Task 138.  Its profile is composed through the
/// real CLI path; only the host-only control may arm the staged snapshot.
struct RepositoryCommitFixture {
    _base: TestDirectory,
    root: PathBuf,
    target: PathBuf,
    profile: PathBuf,
}

impl RepositoryCommitFixture {
    fn new(label: &str) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        git(&root, &["config", "user.name", "fixture"]);
        let target = root.join("target.txt");
        fs::write(&target, b"baseline\n").expect("target should be written");
        git(&root, &["add", "--", "target.txt"]);
        git(&root, &["commit", "--quiet", "-m", "fixture"]);
        fs::write(&target, b"reviewed A\n").expect("staged target should be written");
        git(&root, &["add", "--", "target.txt"]);
        let profile = root.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "task138-repository-commit-bridge",
            "resources": {
                "executables": { "git": { "path": git_executable(), "kind": "native" } },
                "repositories": { "repo": { "path": &root } }
            },
            "capabilities": [{
                "name": "repo.commit", "enabled": true, "permission": "execute",
                "executable": "git", "repository": "repo",
                "identity_name": "RAH bridge host",
                "identity_email": "bridge-host@example.invalid"
            }]
        });
        fs::write(&profile, serde_json::to_vec(&document).unwrap()).unwrap();
        Self {
            _base: base,
            root,
            target,
            profile,
        }
    }

    async fn compose(&self) -> EffectiveProfileComposition {
        compose(TrustedStaticProfile::load(&self.profile).expect("profile should load"))
            .await
            .expect("effective profile should compose")
    }

    fn head(&self) -> String {
        String::from_utf8(git_output(&self.root, &["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_owned()
    }

    fn stage(&self, contents: &[u8]) {
        fs::write(&self.target, contents).unwrap();
        git(&self.root, &["add", "--", "target.txt"]);
    }

    fn assert_commit(&self, old_head: &str, message: &str) {
        let head = self.head();
        assert_eq!(
            String::from_utf8(git_output(&self.root, &["rev-parse", "HEAD^"]))
                .unwrap()
                .trim(),
            old_head
        );
        assert_eq!(
            String::from_utf8(git_output(
                &self.root,
                &["show", "-s", "--format=%B", "HEAD"]
            ))
            .unwrap()
            .trim_end(),
            message
        );
        assert_eq!(
            String::from_utf8(git_output(
                &self.root,
                &["show", "-s", "--format=%an <%ae>", "HEAD"]
            ))
            .unwrap()
            .trim(),
            "RAH bridge host <bridge-host@example.invalid>"
        );
        assert!(
            !String::from_utf8(git_output(&self.root, &["cat-file", "-p", &head]))
                .unwrap()
                .contains("gpgsig ")
        );
    }
}

impl RepositoryFixture {
    fn new(label: &str) -> Self {
        Self::new_with_target(label, b"alpha\nold\nomega\n")
    }

    fn new_with_target(label: &str, target_contents: &[u8]) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "rah-test@example.invalid"]);
        git(&root, &["config", "user.name", "RAH bridge test"]);
        let target = root.join("target.txt");
        let unrelated = root.join("unrelated.txt");
        fs::write(&target, target_contents).expect("target should be written");
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

    fn multi_request(&self, replacements: &[(&str, &str)]) -> Value {
        let bytes = fs::read(&self.target).expect("target should be readable");
        json!({
            "path": "target.txt",
            "expected_file_sha256": sha256_hex(&bytes),
            "expected_file_byte_length": bytes.len(),
            "replacements": replacements.iter().map(|(old, new)| json!({
                "expected_old_text": old,
                "replacement_text": new,
            })).collect::<Vec<_>>(),
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

struct RepositoryCreateFileFixture {
    _base: TestDirectory,
    root: PathBuf,
    target: PathBuf,
    unrelated: PathBuf,
    profile: PathBuf,
}

struct RepositoryMultiFileEditFixture {
    _base: TestDirectory,
    root: PathBuf,
    a: PathBuf,
    b: PathBuf,
    profile: PathBuf,
}

impl RepositoryMultiFileEditFixture {
    fn new(label: &str) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "rah-test@example.invalid"]);
        git(
            &root,
            &["config", "user.name", "RAH multi-file bridge test"],
        );
        let a = root.join("a.txt");
        let b = root.join("b.txt");
        fs::write(&a, b"alpha old\n").expect("a target should be written");
        fs::write(&b, b"beta old\n").expect("b target should be written");
        git(&root, &["add", "--", "a.txt", "b.txt"]);
        git(&root, &["commit", "--quiet", "-m", "fixture"]);
        let profile = root.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "task095-multi-file-bridge",
            "resources": {
                "executables": { "git": { "path": git_executable(), "kind": "native" } },
                "repositories": { "repo": { "path": &root } }
            },
            "capabilities": [{
                "name": "repo.edit-files", "enabled": true, "permission": "execute",
                "executable": "git", "repository": "repo"
            }]
        });
        fs::write(&profile, serde_json::to_vec(&document).unwrap()).unwrap();
        Self {
            _base: base,
            root,
            a,
            b,
            profile,
        }
    }

    async fn compose(&self) -> EffectiveProfileComposition {
        compose(TrustedStaticProfile::load(&self.profile).expect("profile should load"))
            .await
            .expect("effective profile should compose")
    }

    fn request(&self) -> Value {
        let target = |path: &Path, logical: &str, old: &str, new: &str| {
            let bytes = fs::read(path).unwrap();
            json!({"path":logical,"expected_file_sha256":sha256_hex(&bytes),"expected_file_byte_length":bytes.len(),"replacements":[{"expected_old_text":old,"replacement_text":new}]})
        };
        json!({"targets":[
            target(&self.b, "b.txt", "beta old", "beta new"),
            target(&self.a, "a.txt", "alpha old", "alpha new")
        ]})
    }

    fn snapshot(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            git_output(&self.root, &["rev-parse", "HEAD"]),
            git_output(&self.root, &["show-ref"]),
            fs::read(self.root.join(".git/index")).unwrap(),
        )
    }
}

impl RepositoryCreateFileFixture {
    fn new(label: &str) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "rah-test@example.invalid"]);
        git(
            &root,
            &["config", "user.name", "RAH create-file bridge test"],
        );
        fs::create_dir(root.join("src")).expect("existing target parent should be created");
        let unrelated = root.join("unrelated.txt");
        fs::write(&unrelated, b"unrelated\n").expect("sentinel should be written");
        git(&root, &["add", "--", "unrelated.txt"]);
        git(&root, &["commit", "--quiet", "-m", "fixture"]);
        let profile = root.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "task086-create-file-bridge",
            "resources": {
                "executables": { "git": { "path": git_executable(), "kind": "native" } },
                "repositories": { "worktree": { "path": &root } }
            },
            "capabilities": [{
                "name": "repo.create-file",
                "enabled": true,
                "permission": "execute",
                "executable": "git",
                "repository": "worktree"
            }]
        });
        fs::write(&profile, serde_json::to_vec(&document).unwrap()).unwrap();
        Self {
            _base: base,
            target: root.join("src").join("created.rah"),
            root,
            unrelated,
            profile,
        }
    }

    async fn compose(&self) -> EffectiveProfileComposition {
        compose(TrustedStaticProfile::load(&self.profile).expect("profile should load"))
            .await
            .expect("effective profile should compose")
    }

    fn request(&self) -> Value {
        json!({"path":"src/created.rah","content":"created by deterministic bridge\n"})
    }

    fn snapshot(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            git_output(&self.root, &["rev-parse", "HEAD"]),
            git_output(&self.root, &["show-ref"]),
            fs::read(self.root.join(".git").join("index")).unwrap(),
        )
    }

    fn assert_metadata_unchanged(&self, before: &(Vec<u8>, Vec<u8>, Vec<u8>)) {
        assert_eq!(self.snapshot(), *before);
        assert_eq!(fs::read(&self.unrelated).unwrap(), b"unrelated\n");
    }
}

struct RepositoryObserverFixture {
    _base: TestDirectory,
    root: PathBuf,
    tracked: PathBuf,
    profile: PathBuf,
}

#[derive(Debug, Eq, PartialEq)]
struct RepositoryObserverSnapshot {
    head: Vec<u8>,
    refs: Vec<u8>,
    index: Vec<u8>,
    tracked: Vec<u8>,
    untracked: Vec<u8>,
}

impl RepositoryObserverFixture {
    fn new(label: &str, include_patch: bool) -> Self {
        let base = TestDirectory::new();
        let root = base.path().join(label);
        fs::create_dir(&root).expect("repository directory should be created");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "rah-test@example.invalid"]);
        git(&root, &["config", "user.name", "RAH observer bridge test"]);
        let tracked = root.join("tracked.txt");
        let initial = if include_patch {
            b"alpha = 1\nbeta = 2\ngamma = 3\nsentinel = unchanged\n".as_slice()
        } else {
            b"base\n".as_slice()
        };
        fs::write(&tracked, initial).expect("tracked file should be written");
        git(&root, &["add", "--", "tracked.txt"]);
        git(&root, &["commit", "--quiet", "-m", "fixture"]);

        let mut capabilities = vec![
            json!({"name":"repo.file-info", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}),
            json!({"name":"repo.diff", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}),
            json!({"name":"repo.diff-staged", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}),
        ];
        if include_patch {
            capabilities.push(json!({"name":"repo.patch", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}));
        }
        let profile = base.path().join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "task064-observer-bridge",
            "resources": {
                "executables": { "git": { "path": git_executable(), "kind": "native" } },
                "repositories": { "workspace": { "path": &root } }
            },
            "capabilities": capabilities,
        });
        fs::write(
            &profile,
            serde_json::to_vec(&document).expect("profile JSON should serialize"),
        )
        .expect("trusted profile should be written");
        Self {
            _base: base,
            root,
            tracked,
            profile,
        }
    }

    async fn compose(&self) -> EffectiveProfileComposition {
        compose(TrustedStaticProfile::load(&self.profile).expect("profile should load"))
            .await
            .expect("effective profile should compose")
    }

    fn snapshot(&self) -> RepositoryObserverSnapshot {
        RepositoryObserverSnapshot {
            head: git_output(&self.root, &["rev-parse", "HEAD"]),
            refs: git_output(&self.root, &["show-ref"]),
            index: fs::read(self.root.join(".git").join("index"))
                .expect("index should be readable"),
            tracked: fs::read(&self.tracked).expect("tracked worktree should be readable"),
            untracked: fs::read(self.root.join("untracked.txt")).unwrap_or_default(),
        }
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

struct ForcedComposedCreateFileResult {
    inner: Arc<dyn Tool>,
    executions: Arc<AtomicUsize>,
    status: &'static str,
}

#[async_trait]
impl Tool for ForcedComposedCreateFileResult {
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
            content: vec![ToolContent::Json(json!({"status": self.status}))],
            is_error: true,
        })
    }
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
async fn trusted_profile_composed_repo_create_file_preserves_schema_permission_dedupe_and_metadata()
{
    let fixture = RepositoryCreateFileFixture::new("success");
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let definition = composition
        .registry()
        .definitions()
        .into_iter()
        .next()
        .expect("effective composer should publish repo.create-file");
    assert_eq!(definition.name, ToolName::new("repo.create-file"));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","additionalProperties":false,"required":["path","content"],"properties":{"path":{"type":"string"},"content":{"type":"string"}}})
    );
    assert!(
        !format!("{:?}", composition.effective_profile())
            .contains(fixture.root.to_string_lossy().as_ref())
    );
    assert!(
        !fixture.target.exists(),
        "effective composition must not create a target"
    );

    let executions = Arc::new(AtomicUsize::new(0));
    let registry =
        counting_composed_registry(&composition, "repo.create-file", Arc::clone(&executions));
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let dynamic = &thread["params"]["dynamicTools"][0];
    let alias = dynamic["name"].as_str().unwrap();
    assert_eq!(alias, "rah_tool_0");
    assert_eq!(dynamic["inputSchema"], definition.input_schema);
    assert_restrictions(&thread["params"]);

    for id in [json!(860), json!("duplicate-create-file-delivery")] {
        peer.send(tool_request(
            id,
            "private-thread",
            "private-turn",
            "create-file-once",
            alias,
            fixture.request(),
        ));
    }
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(first["result"]["success"], true, "{first}");
    let visible = first["result"]["contentItems"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(visible).unwrap(),
        json!({
            "status":"ok",
            "path":"src/created.rah",
            "length":32,
            "sha256":sha256_hex(b"created by deterministic bridge\n")
        })
    );
    assert!(!visible.contains(fixture.root.to_string_lossy().as_ref()));
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(
        fs::read(&fixture.target).unwrap(),
        b"created by deterministic bridge\n"
    );
    assert!(
        git_output(
            &fixture.root,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )
        .windows(b"?? src/created.rah\n".len())
        .any(|line| line == b"?? src/created.rah\n")
    );
    fixture.assert_metadata_unchanged(&before);

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
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_create_file_requires_execute_before_mutation() {
    for allowed in [
        vec![PermissionLevel::None],
        vec![PermissionLevel::Read],
        vec![PermissionLevel::Write],
    ] {
        let fixture = RepositoryCreateFileFixture::new("permission-denied");
        let before = fixture.snapshot();
        let composition = fixture.compose().await;
        let (runtime, mut peer, _) = connected_bridge(composition.registry_handle(), allowed).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(861),
            "private-thread",
            "private-turn",
            "denied-create-file",
            "rah_tool_0",
            fixture.request(),
        ));
        assert_eq!(peer.next_sent().await["result"]["success"], false);
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.unwrap();
        assert!(matches!(
            events.last(),
            Some(AgentEvent::Failed {
                code: rah_protocol::AgentErrorCode::PermissionDenied,
                ..
            })
        ));
        assert!(!fixture.target.exists());
        fixture.assert_metadata_unchanged(&before);
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn trusted_profile_composed_repo_edit_files_uses_generic_bridge_dispatch_once() {
    let fixture = RepositoryMultiFileEditFixture::new("edit-files-success");
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let definition = composition
        .registry()
        .definitions()
        .into_iter()
        .next()
        .expect("effective composer should publish repo.edit-files");
    assert_eq!(definition.name, ToolName::new("repo.edit-files"));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema["properties"]["targets"]["maxItems"],
        4
    );
    assert!(
        !format!("{:?}", composition.effective_profile())
            .contains(fixture.root.to_string_lossy().as_ref())
    );

    let executions = Arc::new(AtomicUsize::new(0));
    let registry =
        counting_composed_registry(&composition, "repo.edit-files", Arc::clone(&executions));
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let alias = thread["params"]["dynamicTools"][0]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(alias, "rah_tool_0");
    assert_ne!(alias, "repo.edit-files");
    assert_eq!(
        thread["params"]["dynamicTools"][0]["inputSchema"],
        definition.input_schema
    );

    let request = fixture.request();
    for id in [json!(950), json!("duplicate-delivery")] {
        peer.send(tool_request(
            id,
            "private-thread",
            "private-turn",
            "edit-files-once",
            &alias,
            request.clone(),
        ));
    }
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    let visible = first["result"]["contentItems"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(visible).unwrap(),
        json!({"status":"ok","effects":[
            {"path":"a.txt","state":"committed_verified"},
            {"path":"b.txt","state":"committed_verified"}
        ]})
    );
    assert!(!visible.contains(fixture.root.to_string_lossy().as_ref()));
    assert_eq!(fs::read(&fixture.a).unwrap(), b"alpha new\n");
    assert_eq!(fs::read(&fixture.b).unwrap(), b"beta new\n");
    assert_eq!(fixture.snapshot(), before);
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(tool_event_count(&events), 3);
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_edit_files_requires_execute_and_forwards_refusals() {
    let fixture = RepositoryMultiFileEditFixture::new("edit-files-refusal");
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let (runtime, mut peer, _) =
        connected_bridge(composition.registry_handle(), vec![PermissionLevel::Read]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    peer.send(tool_request(
        json!(951),
        "private-thread",
        "private-turn",
        "denied",
        "rah_tool_0",
        fixture.request(),
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], false);
    assert_eq!(fs::read(&fixture.a).unwrap(), b"alpha old\n");
    assert_eq!(fs::read(&fixture.b).unwrap(), b"beta old\n");
    assert_eq!(fixture.snapshot(), before);
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.respond("turn/interrupt", json!({})).await;
    let _ = collecting.await.unwrap();
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_edit_files_forwards_invalid_and_precondition_statuses() {
    for (label, mutate) in [("invalid", false), ("stale", true)] {
        let fixture = RepositoryMultiFileEditFixture::new(label);
        let before = fixture.snapshot();
        let composition = fixture.compose().await;
        let executions = Arc::new(AtomicUsize::new(0));
        let registry =
            counting_composed_registry(&composition, "repo.edit-files", Arc::clone(&executions));
        let (runtime, mut peer, _) =
            connected_bridge(registry, vec![PermissionLevel::Execute]).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        let mut request = fixture.request();
        let expected_status = if mutate {
            request["targets"][0]["expected_file_sha256"] = json!("0".repeat(64));
            "precondition_failed"
        } else {
            request["targets"] = json!([]);
            "invalid_target"
        };
        peer.send(tool_request(
            json!(952),
            "private-thread",
            "private-turn",
            label,
            "rah_tool_0",
            request,
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        assert_eq!(
            serde_json::from_str::<Value>(
                response["result"]["contentItems"][0]["text"]
                    .as_str()
                    .unwrap()
            )
            .unwrap(),
            json!({"status": expected_status})
        );
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(fs::read(&fixture.a).unwrap(), b"alpha old\n");
        assert_eq!(fs::read(&fixture.b).unwrap(), b"beta old\n");
        assert_eq!(fixture.snapshot(), before);
        finish_turn(&peer, "completed");
        let _ = handle.into_events().collect::<Vec<_>>().await;
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn repo_create_file_uncertain_and_write_failure_results_are_not_replayed() {
    for status in ["uncertain", "write_failed_known"] {
        let fixture = RepositoryCreateFileFixture::new(status);
        let before = fixture.snapshot();
        let composition = fixture.compose().await;
        let executions = Arc::new(AtomicUsize::new(0));
        let inner = composition
            .registry()
            .get(&ToolName::new("repo.create-file"))
            .unwrap();
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(ForcedComposedCreateFileResult {
                inner,
                executions: Arc::clone(&executions),
                status,
            }))
            .unwrap();
        let (runtime, mut peer, _) =
            connected_bridge(Arc::new(registry), vec![PermissionLevel::Execute]).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        for id in [json!(862), json!("same-logical-create-file")] {
            peer.send(tool_request(
                id,
                "private-thread",
                "private-turn",
                "no-replay",
                "rah_tool_0",
                fixture.request(),
            ));
        }
        let first = peer.next_sent().await;
        let second = peer.next_sent().await;
        assert_eq!(first["result"], second["result"]);
        assert_eq!(
            serde_json::from_str::<Value>(
                first["result"]["contentItems"][0]["text"].as_str().unwrap()
            )
            .unwrap(),
            json!({"status":status})
        );
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert!(
            fixture.target.exists(),
            "no failure cleanup or replay is permitted"
        );
        fixture.assert_metadata_unchanged(&before);
        finish_turn(&peer, "completed");
        let _ = handle.into_events().collect::<Vec<_>>().await;
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
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
async fn trusted_profile_bridge_applies_multi_replacement_once_and_preserves_invariants() {
    let original = b"alpha = 1\nbeta = 2\ngamma = 3\nsentinel = unchanged\n";
    let expected = b"alpha = 10\nbeta = 20\ngamma = 30\nsentinel = unchanged\n";
    let fixture = RepositoryFixture::new_with_target("multi-replacement", original);
    let head = git_output(&fixture.root, &["rev-parse", "HEAD"]);
    let refs = git_output(&fixture.root, &["show-ref"]);
    let index = fs::read(fixture.root.join(".git").join("index")).unwrap();
    let composition = fixture.compose().await;
    let executions = Arc::new(AtomicUsize::new(0));
    let registry = counting_composed_registry(&composition, "repo.patch", Arc::clone(&executions));
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;

    let tool = &thread["params"]["dynamicTools"][0];
    assert_eq!(tool["name"], "rah_tool_0");
    assert_eq!(tool["inputSchema"]["oneOf"].as_array().unwrap().len(), 2);
    assert_eq!(
        tool["inputSchema"]["oneOf"][0]["additionalProperties"],
        false
    );
    assert_eq!(
        tool["inputSchema"]["oneOf"][1]["additionalProperties"],
        false
    );
    assert_eq!(
        tool["inputSchema"]["oneOf"][1]["properties"]["replacements"]["minItems"],
        1
    );
    assert_eq!(
        tool["inputSchema"]["oneOf"][1]["properties"]["replacements"]["maxItems"],
        16
    );
    assert_restrictions(&thread["params"]);

    let multi = fixture.multi_request(&[
        ("alpha = 1", "alpha = 10"),
        ("beta = 2", "beta = 20"),
        ("gamma = 3", "gamma = 30"),
    ]);
    for id in [json!(730), json!("duplicate-delivery")] {
        peer.send(tool_request(
            id,
            "private-thread",
            "private-turn",
            "multi-once",
            "rah_tool_0",
            multi.clone(),
        ));
    }
    let first = peer.next_sent().await;
    let second = peer.next_sent().await;
    assert_eq!(first["result"], second["result"]);
    assert_eq!(first["result"]["success"], true);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    assert_eq!(fs::read(&fixture.target).unwrap(), expected);
    assert_eq!(fs::read(&fixture.unrelated).unwrap(), b"unrelated\n");
    assert_eq!(git_output(&fixture.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(git_output(&fixture.root, &["show-ref"]), refs);
    assert_eq!(
        fs::read(fixture.root.join(".git").join("index")).unwrap(),
        index
    );
    fixture.assert_clean_index();

    let legacy_after_multi = fixture.request("alpha = 10", "alpha = 100");
    peer.send(tool_request(
        json!(731),
        "private-thread",
        "private-turn",
        "new-call",
        "rah_tool_0",
        legacy_after_multi,
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], false);
    assert_eq!(
        executions.load(Ordering::SeqCst),
        2,
        "a new call ID is distinct"
    );
    assert_eq!(fs::read(&fixture.target).unwrap(), expected);
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(tool_event_count(&events), 6);
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_bridge_rejects_invalid_multi_replacement_forms_without_mutation() {
    let original = b"A B\nrepeated repeated\n";
    for label in [
        "empty",
        "too-many",
        "duplicate",
        "overlap",
        "generated-match",
        "repeated-source",
        "stale",
        "mixed",
    ] {
        let fixture = RepositoryFixture::new_with_target(label, original);
        let composition = fixture.compose().await;
        let executions = Arc::new(AtomicUsize::new(0));
        let registry =
            counting_composed_registry(&composition, "repo.patch", Arc::clone(&executions));
        let (runtime, mut peer, _) =
            connected_bridge(registry, vec![PermissionLevel::Execute]).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        let request = match label {
            "empty" => fixture.multi_request(&[]),
            "too-many" => fixture.multi_request(&[("A", "X"); 17]),
            "duplicate" => fixture.multi_request(&[("A", "X"), ("A", "Y")]),
            "overlap" => fixture.multi_request(&[("A B", "X"), ("B", "Y")]),
            "generated-match" => fixture.multi_request(&[("A", "X"), ("X", "Y")]),
            "repeated-source" => fixture.multi_request(&[("repeated", "once")]),
            "stale" => {
                let mut request = fixture.multi_request(&[("A", "X")]);
                request["expected_file_sha256"] = json!("0".repeat(64));
                request
            }
            "mixed" => {
                let mut request = fixture.multi_request(&[("A", "X")]);
                request["expected_old_text"] = json!("A");
                request["replacement_text"] = json!("X");
                request
            }
            _ => unreachable!("case list is closed"),
        };
        peer.send(tool_request(
            json!(732),
            "private-thread",
            "private-turn",
            label,
            "rah_tool_0",
            request,
        ));
        assert_eq!(
            peer.next_sent().await["result"]["success"],
            false,
            "{label} must fail closed"
        );
        assert_eq!(
            executions.load(Ordering::SeqCst),
            1,
            "{label} should reach the real tool once"
        );
        assert_eq!(
            fs::read(&fixture.target).unwrap(),
            original,
            "{label} mutated the target"
        );
        assert_eq!(fs::read(&fixture.unrelated).unwrap(), b"unrelated\n");
        fixture.assert_clean_index();
        finish_turn(&peer, "completed");
        let _ = handle.into_events().collect::<Vec<_>>().await;
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn composed_repository_observers_verify_multi_patch_post_state() {
    let fixture = RepositoryObserverFixture::new("multi-observers", true);
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    assert_eq!(
        thread["params"]["dynamicTools"].as_array().unwrap().len(),
        5
    );

    let bytes = fs::read(&fixture.tracked).unwrap();
    let patch = json!({
        "path": "tracked.txt",
        "expected_file_sha256": sha256_hex(&bytes),
        "expected_file_byte_length": bytes.len(),
        "replacements": [
            {"expected_old_text":"alpha = 1", "replacement_text":"alpha = 10"},
            {"expected_old_text":"beta = 2", "replacement_text":"beta = 20"},
            {"expected_old_text":"gamma = 3", "replacement_text":"gamma = 30"}
        ]
    });
    assert_eq!(
        bridge_call(&mut peer, json!(733), "patch", "rah_tool_3", patch).await["changed"],
        true
    );
    let info = bridge_call(
        &mut peer,
        json!(734),
        "info",
        "rah_tool_2",
        json!({"path":"tracked.txt"}),
    )
    .await;
    assert_eq!(
        info["content"]["byte_length"],
        fs::read(&fixture.tracked).unwrap().len()
    );
    let status = bridge_call(&mut peer, json!(735), "status", "rah_tool_4", json!({})).await;
    assert!(
        status["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["path"] == json!({"encoding":"utf8","value":"tracked.txt"}))
    );
    let diff = bridge_call(&mut peer, json!(736), "diff", "rah_tool_0", json!({})).await;
    assert!(
        diff["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"tracked.txt"}))
    );
    assert_eq!(fixture.snapshot().head, before.head);
    assert_eq!(fixture.snapshot().refs, before.refs);
    assert_eq!(fixture.snapshot().index, before.index);
    finish_turn(&peer, "completed");
    let _ = handle.into_events().collect::<Vec<_>>().await;
    runtime.shutdown().await.unwrap();
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
            fixture.multi_request(&[("old", "new")]),
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

#[tokio::test]
async fn trusted_profile_composed_observers_advertise_canonical_schemas_and_observe_read_only() {
    let fixture = RepositoryObserverFixture::new("observer-success", false);
    fs::write(&fixture.tracked, b"unstaged-only\n").unwrap();
    fs::write(fixture.root.join("staged.txt"), b"staged-only\n").unwrap();
    git(&fixture.root, &["add", "--", "staged.txt"]);
    fs::write(fixture.root.join("untracked.txt"), b"untracked-only\n").unwrap();
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let definitions = composition.registry().definitions();
    assert_eq!(
        definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "repo.diff",
            "repo.diff-staged",
            "repo.file-info",
            "repo.status"
        ]
    );
    assert!(
        definitions
            .iter()
            .all(|definition| definition.permission == PermissionLevel::Execute)
    );

    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    assert_restrictions(&thread["params"]);
    let advertised = thread["params"]["dynamicTools"].as_array().unwrap();
    for (index, (definition, tool)) in definitions.iter().zip(advertised).enumerate() {
        assert_eq!(tool["name"], format!("rah_tool_{index}"));
        assert_ne!(tool["name"], definition.name.as_str());
        assert_eq!(tool["inputSchema"], definition.input_schema);
    }
    assert!(advertised.iter().all(|tool| tool["name"] != "repo.patch"));

    let file_info = bridge_call(
        &mut peer,
        json!(640),
        "file-info",
        "rah_tool_2",
        json!({"path":"tracked.txt"}),
    )
    .await;
    assert_eq!(
        file_info["path"],
        json!({"encoding":"utf8","value":"tracked.txt"})
    );
    let status = bridge_call(&mut peer, json!(641), "status", "rah_tool_3", json!({})).await;
    assert_eq!(status["status"], "ok");
    let paths = status["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["path"].clone())
        .collect::<Vec<_>>();
    assert!(paths.contains(&json!({"encoding":"utf8","value":"tracked.txt"})));
    assert!(paths.contains(&json!({"encoding":"utf8","value":"staged.txt"})));
    assert!(paths.contains(&json!({"encoding":"utf8","value":"untracked.txt"})));
    let diff = bridge_call(&mut peer, json!(642), "diff", "rah_tool_0", json!({})).await;
    assert_eq!(diff["comparison"], "worktree_to_index");
    assert_eq!(diff["base"], "index");
    assert!(
        diff["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"tracked.txt"}))
    );
    assert!(
        !diff["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"staged.txt"}))
    );
    let staged = bridge_call(
        &mut peer,
        json!(643),
        "diff-staged",
        "rah_tool_1",
        json!({}),
    )
    .await;
    assert_eq!(staged["comparison"], "index_to_head");
    assert_eq!(staged["base"], "head");
    assert!(
        staged["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"staged.txt"}))
    );
    assert!(
        !staged["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"tracked.txt"}))
    );
    for response in [&file_info, &status, &diff, &staged] {
        let visible = response.to_string();
        for secret in [
            fixture.root.to_string_lossy().into_owned(),
            git_executable().to_string_lossy().into_owned(),
            std::env::var("HOME").unwrap_or_default(),
            "HostExecutionPolicy".to_owned(),
            "RepositoryObserver".to_owned(),
            fixture.profile.to_string_lossy().into_owned(),
        ] {
            assert!(
                secret.is_empty() || !visible.contains(&secret),
                "response leaked {secret:?}"
            );
        }
    }
    assert_eq!(
        fixture.snapshot(),
        before,
        "observers must not intentionally mutate the repository"
    );
    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert_eq!(tool_event_count(&events), 12);
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn composed_observers_require_execute_before_entry_and_validate_input_without_retries() {
    for (name, alias, valid) in [
        (
            "repo.file-info",
            "rah_tool_2",
            json!({"path":"tracked.txt"}),
        ),
        ("repo.status", "rah_tool_3", json!({})),
        ("repo.diff", "rah_tool_0", json!({})),
        ("repo.diff-staged", "rah_tool_1", json!({})),
    ] {
        for allowed in [
            PermissionLevel::None,
            PermissionLevel::Read,
            PermissionLevel::Write,
        ] {
            let fixture = RepositoryObserverFixture::new("observer-denied", false);
            let before = fixture.snapshot();
            let composition = fixture.compose().await;
            let executions = Arc::new(AtomicUsize::new(0));
            let registry = counting_composed_registry(&composition, name, Arc::clone(&executions));
            let (runtime, mut peer, _) = connected_bridge(registry, vec![allowed]).await;
            let (handle, _) = start_bridge(&runtime, &mut peer).await;
            peer.send(tool_request(
                json!(644),
                "private-thread",
                "private-turn",
                "denied",
                alias,
                valid.clone(),
            ));
            let response = peer.next_sent().await;
            assert_eq!(response["result"]["success"], false);
            assert_eq!(
                response["result"]["contentItems"][0]["text"],
                "RAH permission policy denied the dynamic tool call"
            );
            assert_eq!(
                executions.load(Ordering::SeqCst),
                0,
                "{name} entered despite {allowed:?}"
            );
            let collecting =
                tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
            peer.respond("turn/interrupt", json!({})).await;
            let events = collecting.await.unwrap();
            assert!(matches!(
                events.last(),
                Some(AgentEvent::Failed {
                    code: rah_protocol::AgentErrorCode::PermissionDenied,
                    ..
                })
            ));
            assert_eq!(fixture.snapshot(), before);
            runtime.shutdown().await.unwrap();
            composition.shutdown().await;
        }
    }

    for (name, alias, input) in [
        ("repo.file-info", "rah_tool_2", json!({"path":"../escape"})),
        ("repo.status", "rah_tool_3", json!({"unexpected":true})),
        ("repo.diff", "rah_tool_0", json!({"unexpected":true})),
        ("repo.diff-staged", "rah_tool_1", json!({"unexpected":true})),
    ] {
        let fixture = RepositoryObserverFixture::new("observer-invalid", false);
        let before = fixture.snapshot();
        let composition = fixture.compose().await;
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_composed_registry(&composition, name, Arc::clone(&executions));
        let (runtime, mut peer, _) =
            connected_bridge(registry, vec![PermissionLevel::Execute]).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        peer.send(tool_request(
            json!(645),
            "private-thread",
            "private-turn",
            "invalid",
            alias,
            input,
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        assert_eq!(
            response["result"]["contentItems"][0]["text"],
            "RAH tool execution failed"
        );
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        let collecting =
            tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
        peer.respond("turn/interrupt", json!({})).await;
        let events = collecting.await.unwrap();
        assert_eq!(tool_event_count(&events), 2);
        assert_eq!(fixture.snapshot(), before);
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn composed_observer_deduplication_is_call_identity_not_input_memoization() {
    for (name, alias, input) in [
        (
            "repo.file-info",
            "rah_tool_2",
            json!({"path":"tracked.txt"}),
        ),
        ("repo.diff", "rah_tool_0", json!({})),
    ] {
        let fixture = RepositoryObserverFixture::new("observer-dedupe", false);
        fs::write(&fixture.tracked, b"changed\n").unwrap();
        let before = fixture.snapshot();
        let composition = fixture.compose().await;
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_composed_registry(&composition, name, Arc::clone(&executions));
        let (runtime, mut peer, _) =
            connected_bridge(registry, vec![PermissionLevel::Execute]).await;
        let (handle, _) = start_bridge(&runtime, &mut peer).await;
        for id in [json!(646), json!("duplicate")] {
            peer.send(tool_request(
                id,
                "private-thread",
                "private-turn",
                "same-call",
                alias,
                input.clone(),
            ));
        }
        let first = peer.next_sent().await;
        let second = peer.next_sent().await;
        assert_eq!(first["result"], second["result"]);
        assert_eq!(
            executions.load(Ordering::SeqCst),
            1,
            "{name} should enter once for a duplicate call"
        );
        peer.send(tool_request(
            json!(647),
            "private-thread",
            "private-turn",
            "new-call",
            alias,
            input,
        ));
        assert_eq!(peer.next_sent().await["result"]["success"], true);
        assert_eq!(
            executions.load(Ordering::SeqCst),
            2,
            "{name} should execute for a new call ID"
        );
        assert_eq!(fixture.snapshot(), before);
        finish_turn(&peer, "completed");
        let events = handle.into_events().collect::<Vec<_>>().await;
        assert_eq!(tool_event_count(&events), 6);
        runtime.shutdown().await.unwrap();
        composition.shutdown().await;
    }
}

#[tokio::test]
async fn composed_observer_aliases_are_exact_and_patch_authority_remains_separate() {
    let fixture = RepositoryObserverFixture::new("observer-routing", true);
    let composition = fixture.compose().await;
    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let aliases = thread["params"]["dynamicTools"].as_array().unwrap();
    assert_eq!(aliases.len(), 5);
    assert_eq!(aliases[0]["name"], "rah_tool_0");
    assert_eq!(aliases[4]["name"], "rah_tool_4");
    peer.send(tool_request(
        json!(648),
        "private-thread",
        "private-turn",
        "unknown",
        "not-a-tool",
        json!({}),
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], false);
    peer.send(tool_request(
        json!(649),
        "private-thread",
        "private-turn",
        "canonical",
        "repo.status",
        json!({}),
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], false);
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.respond("turn/interrupt", json!({})).await;
    let events = collecting.await.unwrap();
    assert_eq!(tool_event_count(&events), 0);
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn composed_diff_cancellation_before_entry_does_not_observe_or_replay() {
    let fixture = RepositoryObserverFixture::new("observer-cancel", false);
    fs::write(&fixture.tracked, b"changed\n").unwrap();
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let bridge_executions = Arc::new(AtomicUsize::new(0));
    let composed_executions = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let gate = Arc::new(Notify::new());
    let registry = gated_composed_observer_registry(
        &composition,
        "repo.diff",
        Arc::clone(&bridge_executions),
        Arc::clone(&composed_executions),
        Arc::clone(&entered),
        Arc::clone(&gate),
    );
    let (runtime, mut peer, _) = connected_bridge(registry, vec![PermissionLevel::Execute]).await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let session_id = handle.session_id().clone();
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.send(tool_request(
        json!(650),
        "private-thread",
        "private-turn",
        "cancel",
        "rah_tool_0",
        json!({}),
    ));
    entered.notified().await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    await_cancellation(&mut peer, json!(650)).await;
    finish_turn(&peer, "interrupted");
    cancelling.await.unwrap().unwrap();
    let events = collecting.await.unwrap();
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));
    assert_eq!(bridge_executions.load(Ordering::SeqCst), 1);
    assert_eq!(composed_executions.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.snapshot(), before);
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn composed_diff_reports_binary_without_payload_or_path_leakage() {
    let fixture = RepositoryObserverFixture::new("observer-binary", false);
    fs::write(&fixture.tracked, [0_u8, 159, 146, 150, 1]).unwrap();
    let before = fixture.snapshot();
    let composition = fixture.compose().await;
    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, _) = start_bridge(&runtime, &mut peer).await;
    let diff = bridge_call(&mut peer, json!(651), "binary", "rah_tool_0", json!({})).await;
    let entry = diff["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["new_path"] == json!({"encoding":"utf8","value":"tracked.txt"}))
        .unwrap();
    assert_eq!(entry["binary"], true);
    assert!(entry["patch"].is_null());
    assert!(!diff.to_string().contains("159"));
    assert!(
        !diff
            .to_string()
            .contains(fixture.root.to_string_lossy().as_ref())
    );
    assert_eq!(fixture.snapshot(), before);
    finish_turn(&peer, "completed");
    let _ = handle.into_events().collect::<Vec<_>>().await;
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_commit_is_message_only_host_armed_and_never_replayed() {
    let fixture = RepositoryCommitFixture::new("repo-commit-bridge");
    let old_head = fixture.head();
    let composition = fixture.compose().await;
    let definition = composition
        .registry()
        .definitions()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(definition.name, ToolName::new("repo.commit"));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","properties":{"message":{"type":"string","maxLength":16384}},"required":["message"],"additionalProperties":false})
    );
    assert!(
        definition
            .description
            .contains("host-reviewed staged repository snapshot once")
    );
    assert!(!definition.description.contains("Git argv"));
    assert!(composition.repository_commit_control().is_some());
    assert_eq!(composition.registry().definitions().len(), 1);

    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let dynamic = &thread["params"]["dynamicTools"][0];
    let alias = dynamic["name"].as_str().unwrap().to_owned();
    assert!(alias.starts_with("rah_tool_"));
    assert_ne!(alias, "repo.commit");
    assert!(
        alias
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    );
    assert_eq!(dynamic["description"], definition.description);
    assert_eq!(dynamic["inputSchema"], definition.input_schema);
    let schema = dynamic["inputSchema"].to_string();
    for forbidden in [
        "authorization",
        "token",
        "reviewed_snapshot",
        "expected_head",
        "branch",
        "index",
        "tree",
        "repository",
        "path",
        "argv",
        "identity",
        "signing",
        "credential",
    ] {
        assert!(!schema.contains(forbidden), "schema leaked {forbidden}");
    }
    assert_restrictions(&thread["params"]);

    // Composition and bridge advertisement do not arm.  Execute is necessary,
    // but not sufficient, for this host-owned operation.
    for (call, message) in [
        ("unarmed", "bridge unarmed"),
        ("unarmed-again", "still unarmed"),
    ] {
        peer.send(tool_request(
            json!(call),
            "private-thread",
            "private-turn",
            call,
            &alias,
            json!({"message": message}),
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        assert_eq!(response_json(&response)["status"], "precondition_failed");
        assert_eq!(fixture.head(), old_head);
    }

    // Invalid JSON-shaped input cannot consume the pending host review.
    composition
        .repository_commit_control()
        .unwrap()
        .authorize_current_reviewed_snapshot()
        .await
        .unwrap();
    for (call, input) in [
        (
            "extra",
            json!({"message":"valid", "branch":"refs/heads/other"}),
        ),
        ("missing", json!({})),
        ("wrong-type", json!({"message":123})),
        ("oversized", json!({"message":"x".repeat(16 * 1024 + 1)})),
    ] {
        peer.send(tool_request(
            json!(call),
            "private-thread",
            "private-turn",
            call,
            &alias,
            input,
        ));
        let response = peer.next_sent().await;
        assert_eq!(response["result"]["success"], false);
        assert_eq!(response_json(&response)["status"], "invalid_input");
        assert_eq!(fixture.head(), old_head);
    }

    peer.send(tool_request(
        json!(901),
        "private-thread",
        "private-turn",
        "commit-once",
        &alias,
        json!({"message":"bridge verified commit"}),
    ));
    let committed = peer.next_sent().await;
    assert_eq!(committed["result"]["success"], true);
    let committed_output = response_json(&committed);
    assert_eq!(committed_output["status"], "committed_verified");
    assert_eq!(committed_output["commit_oid"], fixture.head());
    fixture.assert_commit(&old_head, "bridge verified commit");
    assert!(
        !committed
            .to_string()
            .contains(fixture.root.to_string_lossy().as_ref())
    );
    assert!(
        !committed
            .to_string()
            .contains(git_executable().to_string_lossy().as_ref())
    );

    // Completed exact replay is cached by the bridge, not sent through the
    // registry. A fresh ID is dispatched but the consumed authorization fails closed.
    peer.send(tool_request(
        json!(902),
        "private-thread",
        "private-turn",
        "commit-once",
        &alias,
        json!({"message":"bridge verified commit"}),
    ));
    let replay = peer.next_sent().await;
    assert_eq!(replay["result"], committed["result"]);
    assert_eq!(fixture.head(), committed_output["commit_oid"]);
    peer.send(tool_request(
        json!(903),
        "private-thread",
        "private-turn",
        "commit-once",
        &alias,
        json!({"message":"different"}),
    ));
    assert_eq!(peer.next_sent().await["error"]["code"], -32602);
    assert_eq!(fixture.head(), committed_output["commit_oid"]);
    peer.send(tool_request(
        json!(904),
        "private-thread",
        "private-turn",
        "fresh-after-consumption",
        &alias,
        json!({"message":"fresh"}),
    ));
    assert_eq!(
        response_json(&peer.next_sent().await)["status"],
        "precondition_failed"
    );
    assert_eq!(fixture.head(), committed_output["commit_oid"]);

    finish_turn(&peer, "completed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(events.windows(3).any(|events| matches!(events, [AgentEvent::ToolRequested { tool_call, .. }, AgentEvent::ToolStarted { .. }, AgentEvent::ToolFinished { .. }] if tool_call.name == ToolName::new("repo.commit"))));
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
}

#[tokio::test]
async fn trusted_profile_composed_repo_commit_preserves_armed_authorization_across_bridge_denial_and_rejects_stale_snapshot()
 {
    let fixture = RepositoryCommitFixture::new("repo-commit-permission");
    let old_head = fixture.head();
    let composition = fixture.compose().await;
    composition
        .repository_commit_control()
        .unwrap()
        .authorize_current_reviewed_snapshot()
        .await
        .unwrap();
    let (denied, mut peer, _) = connected_bridge(composition.registry_handle(), vec![]).await;
    let (handle, thread) = start_bridge(&denied, &mut peer).await;
    let alias = thread["params"]["dynamicTools"][0]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    peer.send(tool_request(
        json!(910),
        "private-thread",
        "private-turn",
        "denied",
        &alias,
        json!({"message":"denied"}),
    ));
    assert_eq!(peer.next_sent().await["result"]["success"], false);
    let collecting = tokio::spawn(async move { handle.into_events().collect::<Vec<_>>().await });
    peer.respond("turn/interrupt", json!({})).await;
    let events = collecting.await.unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolStarted { .. }))
    );
    denied.shutdown().await.unwrap();

    // Permission denial never entered the Tool, so the same host authorization
    // remains usable once when a separately configured bridge permits Execute.
    let (allowed, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&allowed, &mut peer).await;
    let alias = thread["params"]["dynamicTools"][0]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    peer.send(tool_request(
        json!(911),
        "private-thread",
        "private-turn",
        "allowed",
        &alias,
        json!({"message":"after denial"}),
    ));
    assert_eq!(
        response_json(&peer.next_sent().await)["status"],
        "committed_verified"
    );
    fixture.assert_commit(&old_head, "after denial");
    finish_turn(&peer, "completed");
    let _ = handle.into_events().collect::<Vec<_>>().await;
    allowed.shutdown().await.unwrap();
    composition.shutdown().await;

    let fixture = RepositoryCommitFixture::new("repo-commit-stale");
    let old_head = fixture.head();
    let original = fs::read(&fixture.target).unwrap();
    let composition = fixture.compose().await;
    composition
        .repository_commit_control()
        .unwrap()
        .authorize_current_reviewed_snapshot()
        .await
        .unwrap();
    fixture.stage(b"reviewed B\n");
    let (runtime, mut peer, _) = connected_bridge(
        composition.registry_handle(),
        vec![PermissionLevel::Execute],
    )
    .await;
    let (handle, thread) = start_bridge(&runtime, &mut peer).await;
    let alias = thread["params"]["dynamicTools"][0]["name"]
        .as_str()
        .unwrap()
        .to_owned();
    peer.send(tool_request(
        json!(912),
        "private-thread",
        "private-turn",
        "stale",
        &alias,
        json!({"message":"stale"}),
    ));
    assert_eq!(
        response_json(&peer.next_sent().await)["status"],
        "precondition_failed"
    );
    fixture.stage(&original);
    peer.send(tool_request(
        json!(913),
        "private-thread",
        "private-turn",
        "stale-restored",
        &alias,
        json!({"message":"restored"}),
    ));
    assert_eq!(
        response_json(&peer.next_sent().await)["status"],
        "precondition_failed"
    );
    assert_eq!(fixture.head(), old_head);
    finish_turn(&peer, "completed");
    let _ = handle.into_events().collect::<Vec<_>>().await;
    runtime.shutdown().await.unwrap();
    composition.shutdown().await;
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

fn counting_composed_registry(
    composition: &EffectiveProfileComposition,
    counted_name: &str,
    executions: Arc<AtomicUsize>,
) -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    for definition in composition.registry().definitions() {
        let tool = composition
            .registry()
            .get(&definition.name)
            .expect("effective composition should retain every definition");
        let tool: Arc<dyn Tool> = if definition.name == ToolName::new(counted_name) {
            Arc::new(CountingComposedTool {
                inner: tool,
                executions: Arc::clone(&executions),
            })
        } else {
            tool
        };
        registry
            .register(tool)
            .expect("test registry should preserve names");
    }
    Arc::new(registry)
}

fn gated_composed_observer_registry(
    composition: &EffectiveProfileComposition,
    name: &str,
    bridge_executions: Arc<AtomicUsize>,
    composed_executions: Arc<AtomicUsize>,
    entered: Arc<Notify>,
    gate: Arc<Notify>,
) -> Arc<ToolRegistry> {
    let inner = composition.registry().get(&ToolName::new(name)).unwrap();
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(GatedComposedPatch {
            inner,
            bridge_executions,
            composed_executions,
            before_inner: Some(entered),
            after_inner: None,
            before_gate: Some(gate),
            after_gate: None,
        }))
        .unwrap();
    Arc::new(registry)
}

async fn bridge_call(
    peer: &mut FakePeer,
    id: Value,
    call: &str,
    alias: &str,
    input: Value,
) -> Value {
    peer.send(tool_request(
        id,
        "private-thread",
        "private-turn",
        call,
        alias,
        input,
    ));
    let response = peer.next_sent().await;
    assert_eq!(
        response["result"]["success"], true,
        "bridge call should succeed: {response}"
    );
    serde_json::from_str(
        response["result"]["contentItems"][0]["text"]
            .as_str()
            .expect("observer response should be JSON text"),
    )
    .expect("observer response should retain JSON")
}

fn response_json(response: &Value) -> Value {
    serde_json::from_str(
        response["result"]["contentItems"][0]["text"]
            .as_str()
            .expect("bridge ToolOutput should be JSON text"),
    )
    .expect("bridge ToolOutput should preserve JSON")
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

fn git_output(root: &Path, arguments: &[&str]) -> Vec<u8> {
    let output = Command::new(git_executable())
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("Git command should start");
    assert!(
        output.status.success(),
        "Git command {arguments:?} should succeed"
    );
    output.stdout
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
    assert_eq!(params["config"]["features"]["memories"], false);
    assert_eq!(params["config"]["tools"]["web_search"], false);
    assert_eq!(params["config"]["tools"]["view_image"], false);
    assert_eq!(params["config"]["apps"]["_default"]["enabled"], false);
    assert_eq!(params["config"]["mcp_servers"], json!({}));
}
