//! Public adapter for the bounded multi-file repository edit policy.

use std::path::Path;

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    Tool, ToolContext, ToolError,
    repository_multi_file_preflight::{
        MultiFileEditOutcome, MultiFileEditStatus, MultiFileEffectState,
        RepositoryMultiFileMutationPolicy,
    },
};

/// Stable name for the bounded multi-file repository edit capability.
pub const REPOSITORY_EDIT_FILES_TOOL_NAME: &str = "repo.edit-files";

/// Host-constructed capability for bounded edits to existing repository files.
///
/// The private policy owns the repository, Git executable, validation, commit
/// ordering, and native replacement authority. Model input supplies only
/// bounded conditional text replacements.
pub struct RepositoryMultiFileEditTool {
    policy: RepositoryMultiFileMutationPolicy,
}

impl RepositoryMultiFileEditTool {
    /// Creates a `repo.edit-files` tool for one trusted non-bare repository.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        RepositoryMultiFileMutationPolicy::new(git_executable.as_ref(), repository_root.as_ref())
            .map(|policy| Self { policy })
            .map_err(|_| ToolError::Execution {
                message: "repository multi-file edit policy rejected host authority".to_owned(),
            })
    }
}

#[async_trait]
impl Tool for RepositoryMultiFileEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_EDIT_FILES_TOOL_NAME),
            description: "Applies exact conditional text replacements to up to four clean tracked repository files in deterministic host-owned order.".to_owned(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["targets"],
                "properties": {
                    "targets": {
                        "type": "array", "minItems": 1, "maxItems": 4,
                        "items": {
                            "type": "object", "additionalProperties": false,
                            "required": ["path", "expected_file_sha256", "expected_file_byte_length", "replacements"],
                            "properties": {
                                "path": {"type": "string", "minLength": 1, "maxLength": 1024},
                                "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                                "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1048576},
                                "replacements": {
                                    "type": "array", "minItems": 1, "maxItems": 16,
                                    "items": {
                                        "type": "object", "additionalProperties": false,
                                        "required": ["expected_old_text", "replacement_text"],
                                        "properties": {
                                            "expected_old_text": {"type": "string", "minLength": 1, "maxLength": 65536},
                                            "replacement_text": {"type": "string", "maxLength": 65536}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        match self.policy.commit(&input).await {
            Ok(outcome) => Ok(output_for_outcome(outcome)),
            Err(error) => Ok(status_output(error.public_status())),
        }
    }
}

fn status_output(status: &'static str) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Json(json!({"status": status}))],
        is_error: true,
    }
}

fn output_for_outcome(outcome: MultiFileEditOutcome) -> ToolOutput {
    let status = match outcome.status {
        MultiFileEditStatus::Ok => "ok",
        MultiFileEditStatus::InvalidTarget => "invalid_target",
        MultiFileEditStatus::PreconditionFailed => "precondition_failed",
        MultiFileEditStatus::FailedKnownNoEffect => "failed_known_no_effect",
        MultiFileEditStatus::PartialEffect => "partial_effect",
        MultiFileEditStatus::Uncertain => "uncertain",
    };
    let effects = outcome
        .effects
        .into_iter()
        .map(|effect| {
            let state = match effect.state {
                MultiFileEffectState::CommittedVerified => "committed_verified",
                MultiFileEffectState::UnchangedVerified => "unchanged_verified",
                MultiFileEffectState::NotAttempted => "not_attempted",
                MultiFileEffectState::Uncertain => "uncertain",
            };
            json!({"path": effect.logical_path, "state": state})
        })
        .collect::<Vec<_>>();
    ToolOutput {
        content: vec![ToolContent::Json(
            json!({"status": status, "effects": effects}),
        )],
        is_error: status != "ok",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    use rah_protocol::{ToolCall, ToolCallId};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        Tool, ToolRegistry,
        repository_multi_file_preflight::{CommitTestPhase, test_commit_hook},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> (Self, PathBuf, PathBuf) {
            let base = std::env::temp_dir().join(format!(
                "rah-multi-tool-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir(&base).unwrap();
            let root = base.join("repository");
            fs::create_dir(&root).unwrap();
            let git = git_executable();
            git_run(&git, &root, &["init", "--quiet"]);
            for (path, bytes) in [
                ("a.rs", b"A old\n".as_slice()),
                ("b.rs", b"B old\n".as_slice()),
                ("c.rs", b"C old\n".as_slice()),
                ("d.rs", b"D old\n".as_slice()),
                ("sentinel.txt", b"sentinel\n".as_slice()),
            ] {
                fs::write(root.join(path), bytes).unwrap();
            }
            git_run(&git, &root, &["add", "."]);
            git_run(
                &git,
                &root,
                &[
                    "-c",
                    "user.name=RAH",
                    "-c",
                    "user.email=rah@example.invalid",
                    "commit",
                    "--quiet",
                    "-m",
                    "initial",
                ],
            );
            (Self(base), git, root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn direct_tool_success_is_host_ordered_and_preserves_git_metadata() {
        let (_fixture, git, root) = Fixture::new();
        let before = git_state(&git, &root);
        let tool = RepositoryMultiFileEditTool::new(&git, &root).unwrap();
        let output = run(&tool, request(&root, &["d.rs", "b.rs", "a.rs", "c.rs"])).await;

        assert!(!output.is_error);
        assert_eq!(content(&output)["status"], "ok");
        assert_eq!(effect_paths(&output), ["a.rs", "b.rs", "c.rs", "d.rs"]);
        assert!(
            content(&output)["effects"]
                .as_array()
                .unwrap()
                .iter()
                .all(|effect| effect["state"] == "committed_verified")
        );
        for path in ["a.rs", "b.rs", "c.rs", "d.rs"] {
            assert_eq!(
                fs::read(root.join(path)).unwrap(),
                format!(
                    "{} new\n",
                    (path.as_bytes()[0] as char).to_ascii_uppercase()
                )
                .as_bytes()
            );
        }
        assert_eq!(fs::read(root.join("sentinel.txt")).unwrap(), b"sentinel\n");
        let after = git_state(&git, &root);
        assert_eq!(after.0, before.0, "raw index changed");
        assert_eq!(after.1, before.1, "HEAD changed");
        assert_eq!(after.2, before.2, "refs changed");
        assert_eq!(after.3, " M a.rs\0 M b.rs\0 M c.rs\0 M d.rs\0");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_and_precondition_failures_are_closed_and_effect_free() {
        let (_fixture, git, root) = Fixture::new();
        let tool = RepositoryMultiFileEditTool::new(&git, &root).unwrap();
        for input in [
            json!({"targets": []}),
            json!({"targets": vec![target(&root, "a.rs"); 5]}),
            json!({"targets": [target(&root, "a.rs")], "extra": true}),
            json!({"targets": [{"path": "a.rs", "expected_old_text": "old", "replacement_text": "new"}]}),
            json!({"targets": [target(&root, "/a.rs")]}),
            json!({"targets": [target(&root, "a.rs"), target(&root, "a.rs")]}),
        ] {
            let output = run(&tool, input).await;
            assert!(output.is_error, "unexpected output: {:?}", content(&output));
            assert_closed_status(&output, "invalid_target");
            assert_eq!(fs::read(root.join("a.rs")).unwrap(), b"A old\n");
        }
        let mut mismatch = target(&root, "a.rs");
        mismatch["expected_file_sha256"] = json!("0".repeat(64));
        for input in [
            json!({"targets": [mismatch]}),
            json!({"targets": [target(&root, "missing.rs")]}),
        ] {
            let output = run(&tool, input).await;
            assert!(output.is_error);
            assert_closed_status(&output, "precondition_failed");
        }
        fs::write(root.join("a.rs"), b"changed before execution\n").unwrap();
        let output = run(&tool, request(&root, &["a.rs"])).await;
        assert_closed_status(&output, "precondition_failed");

        let (_fixture, git, root) = Fixture::new();
        let tool = RepositoryMultiFileEditTool::new(&git, &root).unwrap();
        let mut invalid_sha = target(&root, "a.rs");
        invalid_sha["expected_file_sha256"] = json!("not-a-sha");
        assert_closed_status(
            &run(&tool, json!({"targets": [invalid_sha]})).await,
            "invalid_target",
        );
        fs::write(root.join("untracked.rs"), b"untracked old\n").unwrap();
        assert_closed_status(
            &run(&tool, request(&root, &["untracked.rs"])).await,
            "precondition_failed",
        );

        let (_fixture, git, root) = Fixture::new();
        let tool = RepositoryMultiFileEditTool::new(&git, &root).unwrap();
        fs::write(root.join("a.rs"), b"A staged old\n").unwrap();
        git_run(&git, &root, &["add", "--", "a.rs"]);
        assert_closed_status(
            &run(&tool, request(&root, &["a.rs"])).await,
            "precondition_failed",
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn native_outcomes_are_redacted_and_never_retried() {
        for (phase, failed, status, states) in [
            (
                CommitTestPhase::KnownNoEffectFailure,
                0,
                "failed_known_no_effect",
                vec!["unchanged_verified", "not_attempted", "not_attempted"],
            ),
            (
                CommitTestPhase::KnownNoEffectFailure,
                1,
                "partial_effect",
                vec!["committed_verified", "unchanged_verified", "not_attempted"],
            ),
            (
                CommitTestPhase::UncertainNativeOutcome,
                1,
                "uncertain",
                vec!["committed_verified", "uncertain", "not_attempted"],
            ),
        ] {
            let (_fixture, git, root) = Fixture::new();
            let tool = RepositoryMultiFileEditTool::new(&git, &root).unwrap();
            let policy_root = tool.policy.test_root();
            test_commit_hook::install(policy_root, phase, failed);
            let output = run(&tool, request(&root, &["c.rs", "a.rs", "b.rs"])).await;
            assert!(output.is_error, "unexpected output: {:?}", content(&output));
            assert_eq!(content(&output)["status"], status);
            assert_eq!(effect_states(&output), states);
            for index in 0..=failed {
                assert_eq!(test_commit_hook::attempts(policy_root, index), 1);
            }
            for index in failed + 1..3 {
                assert_eq!(test_commit_hook::attempts(policy_root, index), 0);
            }
            assert_no_private_fields(content(&output));
            test_commit_hook::clear(policy_root);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn definition_and_registry_dispatch_are_generic_and_explicit() {
        let (_fixture, git, root) = Fixture::new();
        let tool = Arc::new(RepositoryMultiFileEditTool::new(&git, &root).unwrap());
        let definition = tool.definition();
        assert_eq!(definition.name.as_str(), REPOSITORY_EDIT_FILES_TOOL_NAME);
        assert_eq!(definition.permission, PermissionLevel::Execute);
        let mut registry = ToolRegistry::new();
        registry.register(tool.clone()).unwrap();
        assert_eq!(
            registry
                .get(&ToolName::new(REPOSITORY_EDIT_FILES_TOOL_NAME))
                .unwrap()
                .definition(),
            definition
        );
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(REPOSITORY_EDIT_FILES_TOOL_NAME),
                    input: ToolInput(request(&root, &["a.rs"])),
                },
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(content(&output)["status"], "ok");
        assert!(RepositoryMultiFileEditTool::new(&git, Path::new("relative")).is_err());
        assert!(RepositoryMultiFileEditTool::new(&git, root.join("missing")).is_err());
    }

    fn git_executable() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        fs::canonicalize(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }

    fn git_run(git: &Path, root: &Path, args: &[&str]) {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_state(git: &Path, root: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>, String) {
        let output = |args: &[&str]| {
            Command::new(git)
                .args(args)
                .current_dir(root)
                .output()
                .unwrap()
                .stdout
        };
        (
            fs::read(root.join(".git/index")).unwrap(),
            fs::read(root.join(".git/HEAD")).unwrap(),
            output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"]),
            String::from_utf8(output(&["status", "--porcelain=v1", "-z"])).unwrap(),
        )
    }

    fn request(root: &Path, paths: &[&str]) -> Value {
        json!({"targets": paths.iter().map(|path| target(root, path)).collect::<Vec<_>>()})
    }

    fn target(root: &Path, path: &str) -> Value {
        let bytes = fs::read(root.join(path)).unwrap_or_else(|_| b"missing\n".to_vec());
        json!({"path": path, "expected_file_sha256": format!("{:x}", Sha256::digest(&bytes)), "expected_file_byte_length": bytes.len(), "replacements": [{"expected_old_text": "old", "replacement_text": "new"}]})
    }

    async fn run(tool: &RepositoryMultiFileEditTool, input: Value) -> ToolOutput {
        tool.execute(ToolInput(input), ToolContext::default())
            .await
            .unwrap()
    }

    fn content(output: &ToolOutput) -> &Value {
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("expected one JSON result")
        };
        value
    }

    fn effect_paths(output: &ToolOutput) -> Vec<&str> {
        content(output)["effects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|effect| effect["path"].as_str().unwrap())
            .collect()
    }

    fn effect_states(output: &ToolOutput) -> Vec<&str> {
        content(output)["effects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|effect| effect["state"].as_str().unwrap())
            .collect()
    }

    fn assert_closed_status(output: &ToolOutput, status: &str) {
        assert_eq!(content(output), &json!({"status": status}));
        assert_no_private_fields(content(output));
    }

    fn assert_no_private_fields(value: &Value) {
        for forbidden in [
            "reason",
            "error",
            "message",
            "stderr",
            "repository",
            "root",
            "git",
            "temporary",
            "native_code",
            "retryable",
            "rollback",
        ] {
            assert!(value.get(forbidden).is_none(), "leaked {forbidden}");
        }
    }
}
