//! Bounded, host-owned tracked repository structure listing.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    path::Path,
    time::Instant,
};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    Tool, ToolContext, ToolError,
    repository_observer::{
        OBSERVER_MAX_PATH_BYTES, ObserverCommand, RepositoryObserver, SEARCH_TIMEOUT,
        TrackedCandidate, inspect_tracked_candidate, parse_tracked_inventory,
        repository_target_path, successful_tracked_inventory,
    },
};

/// Stable name for the fixed, read-only repository structure observer.
pub const REPOSITORY_LIST_TOOL_NAME: &str = "repo.list";

const MAX_REQUEST_BYTES: usize = 4 * 1024;
const MAX_ENTRIES: usize = 128;
const MAX_RESULT_BYTES: usize = 128 * 1024;

/// One host-configured repository structure listing Tool.
pub struct RepositoryListTool {
    observer: RepositoryObserver,
}

impl RepositoryListTool {
    /// Creates the observer for one host-selected native Git executable and
    /// repository. The model supplies neither resource.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            observer: RepositoryObserver::new(git_executable.as_ref(), repository_root.as_ref())?,
        })
    }
}

#[async_trait]
impl Tool for RepositoryListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_LIST_TOOL_NAME),
            description: "Lists direct children of the selected tracked repository structure."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string"}
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
        let request = ListRequest::parse(&input)?;
        tokio::time::timeout(SEARCH_TIMEOUT, self.execute_bounded(request))
            .await
            .map_err(|_| list_error("repository listing exceeded its total timeout"))?
    }
}

impl RepositoryListTool {
    async fn execute_bounded(&self, request: ListRequest) -> Result<ToolOutput, ToolError> {
        let started = Instant::now();
        let _lease = self.observer.acquire_lease().await;
        self.observer.revalidate()?;
        self.observer.validate_observation()?;

        let inventory = self
            .observer
            .run(ObserverCommand::TrackedInventory, None, started)
            .await
            .and_then(successful_tracked_inventory)?;
        self.observer.revalidate()?;
        let inventory = parse_tracked_inventory(&inventory)?;

        let mut entries = BTreeMap::new();
        let mut omitted = Omitted {
            non_addressable_path: inventory.non_addressable_path,
            ..Omitted::default()
        };
        let mut has_eligible_descendant = request.path.is_none();
        let mut visible_file_target = false;

        for path in inventory.paths {
            let Some(child) = direct_child(request.path.as_deref(), &path) else {
                continue;
            };
            ensure_time(started)?;
            let target = repository_target_path(self.observer.root(), &path);
            self.observer.validate_target(&target)?;
            match inspect_tracked_candidate(&target)? {
                TrackedCandidate::Eligible(_) => {
                    if request.path.as_deref() == Some(path.as_str()) {
                        visible_file_target = true;
                        continue;
                    }
                    has_eligible_descendant = true;
                    let kind = if child == path { "file" } else { "directory" };
                    insert_entry(&mut entries, child, kind)?;
                }
                TrackedCandidate::Missing => omitted.changed_or_missing += 1,
                TrackedCandidate::NonRegular => omitted.non_regular += 1,
            }
        }

        if visible_file_target {
            return Err(list_error("requested repository path is not a directory"));
        }
        if !has_eligible_descendant {
            return Err(list_error("requested repository directory was not found"));
        }

        self.observer.revalidate()?;
        let mut complete = true;
        let mut truncation_reason = None;
        let mut entries = entries
            .into_iter()
            .map(|(path, kind)| json!({"path": path, "kind": kind}))
            .collect::<Vec<_>>();
        if entries.len() > MAX_ENTRIES {
            entries.truncate(MAX_ENTRIES);
            complete = false;
            truncation_reason = Some("result_limit");
        }

        let output = ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": "ok",
                "consistency": "best_effort",
                "path": request.path,
                "complete": complete,
                "truncation_reason": truncation_reason,
                "entries": entries,
                "omitted": {
                    "non_addressable_path": omitted.non_addressable_path,
                    "non_regular": omitted.non_regular,
                    "changed_or_missing": omitted.changed_or_missing
                }
            }))],
            is_error: false,
        };
        let size = serde_json::to_vec(&output)
            .map_err(|_| list_error("repository listing result could not be serialized"))?
            .len();
        if size > MAX_RESULT_BYTES {
            return Err(list_error("repository listing result exceeded its limit"));
        }
        Ok(output)
    }
}

struct ListRequest {
    path: Option<String>,
}

impl ListRequest {
    fn parse(input: &ToolInput) -> Result<Self, ToolError> {
        let serialized =
            serde_json::to_vec(&input.0).map_err(|_| invalid("input is not serializable"))?;
        if serialized.len() > MAX_REQUEST_BYTES {
            return Err(invalid("repository listing input exceeds its limit"));
        }
        let object = input
            .0
            .as_object()
            .ok_or_else(|| invalid("repository listing input must be an object"))?;
        if object.keys().any(|key| key != "path") {
            return Err(invalid("repository listing input has unknown fields"));
        }
        let path = match object.get("path") {
            None => None,
            Some(value) => {
                let path = value
                    .as_str()
                    .ok_or_else(|| invalid("`path` must be a string"))?;
                if !is_valid_path(path) {
                    return Err(invalid("`path` must be a safe repository-relative path"));
                }
                Some(path.to_owned())
            }
        };
        Ok(Self { path })
    }
}

fn is_valid_path(path: &str) -> bool {
    if path.len() > OBSERVER_MAX_PATH_BYTES {
        return false;
    }
    crate::repository_observer::is_safe_repository_path(path, OBSERVER_MAX_PATH_BYTES)
}

fn direct_child(prefix: Option<&str>, path: &str) -> Option<String> {
    match prefix {
        None => Some(path.split('/').next()?.to_owned()),
        Some(prefix) if path == prefix => Some(path.to_owned()),
        Some(prefix) => {
            let remainder = path.strip_prefix(prefix)?.strip_prefix('/')?;
            let first = remainder.split('/').next()?;
            Some(format!("{prefix}/{first}"))
        }
    }
}

fn insert_entry(
    entries: &mut BTreeMap<String, &'static str>,
    path: String,
    kind: &'static str,
) -> Result<(), ToolError> {
    match entries.entry(path) {
        Entry::Vacant(entry) => {
            entry.insert(kind);
            Ok(())
        }
        Entry::Occupied(entry) if *entry.get() == kind => Ok(()),
        Entry::Occupied(_) => Err(list_error(
            "repository structure contained conflicting file and directory entries",
        )),
    }
}

#[derive(Default)]
struct Omitted {
    non_addressable_path: u64,
    non_regular: u64,
    changed_or_missing: u64,
}

fn ensure_time(started: Instant) -> Result<(), ToolError> {
    if started.elapsed() >= SEARCH_TIMEOUT {
        Err(list_error("repository listing exceeded its total timeout"))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidInput {
        message: message.into(),
    }
}

fn list_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!("repository listing {}", message.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    use rah_protocol::ToolContent;
    use serde_json::{Value, json};

    use crate::{Tool, ToolContext};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-repository-list-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            git(&root, &["init", "--quiet"]);
            git(&root, &["config", "user.email", "list@example.invalid"]);
            git(&root, &["config", "user.name", "RAH list test"]);
            Self(root)
        }

        fn commit_all(&self) {
            git(&self.0, &["add", "--all"]);
            git(&self.0, &["commit", "--quiet", "-m", "list fixture"]);
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn native_git() -> PathBuf {
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .map(PathBuf::from)
            .unwrap()
    }

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new(native_git())
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn json_output(output: ToolOutput) -> Value {
        match output.content.as_slice() {
            [ToolContent::Json(value)] => value.clone(),
            other => panic!("unexpected output: {other:?}"),
        }
    }

    async fn list(tool: &RepositoryListTool, input: Value) -> Value {
        json_output(
            tool.execute(ToolInput(input), ToolContext::default())
                .await
                .unwrap(),
        )
    }

    #[test]
    fn definition_is_closed_and_execute_bound() {
        let fixture = Fixture::new();
        let tool = RepositoryListTool::new(native_git(), &fixture.0).unwrap();
        assert_eq!(tool.definition().name.as_str(), REPOSITORY_LIST_TOOL_NAME);
        assert_eq!(tool.definition().permission, PermissionLevel::Execute);
        assert_eq!(
            tool.definition().input_schema,
            json!({
                "type":"object",
                "additionalProperties":false,
                "properties":{"path":{"type":"string"}}
            })
        );
    }

    #[tokio::test]
    async fn root_nested_direct_projection_and_visibility_are_bounded() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.0.join("crates/alpha/src")).unwrap();
        fs::create_dir_all(fixture.0.join("ignored-only")).unwrap();
        fs::write(fixture.0.join("README.md"), b"readme").unwrap();
        fs::write(fixture.0.join("crates/alpha/src/lib.rs"), b"lib").unwrap();
        fs::write(fixture.0.join("crates/alpha/README.md"), b"alpha").unwrap();
        fs::write(fixture.0.join("ignored-only/secret.txt"), b"ignored").unwrap();
        fs::write(
            fixture.0.join(".gitignore"),
            b"ignored-only/\ntracked-ignore.txt\n",
        )
        .unwrap();
        fs::write(fixture.0.join("tracked-ignore.txt"), b"tracked").unwrap();
        fixture.commit_all();
        git(&fixture.0, &["add", "-f", "--", "tracked-ignore.txt"]);
        git(
            &fixture.0,
            &["commit", "--quiet", "-m", "force tracked ignore match"],
        );
        fs::create_dir_all(fixture.0.join("untracked-only")).unwrap();
        fs::write(fixture.0.join("untracked-only/secret.txt"), b"secret").unwrap();
        fs::write(fixture.0.join("README.md"), b"modified").unwrap();
        fs::write(fixture.0.join("staged-new.txt"), b"staged").unwrap();
        git(&fixture.0, &["add", "--", "staged-new.txt"]);

        let tool = RepositoryListTool::new(native_git(), &fixture.0).unwrap();
        let root = list(&tool, json!({})).await;
        assert_eq!(
            root["entries"],
            json!([
                {"path":".gitignore","kind":"file"},
                {"path":"README.md","kind":"file"},
                {"path":"crates","kind":"directory"},
                {"path":"staged-new.txt","kind":"file"},
                {"path":"tracked-ignore.txt","kind":"file"}
            ])
        );
        assert!(!root.to_string().contains("untracked-only"));
        assert!(!root.to_string().contains("ignored-only"));
        let nested = list(&tool, json!({"path":"crates/alpha"})).await;
        assert_eq!(
            nested["entries"],
            json!([
                {"path":"crates/alpha/README.md","kind":"file"},
                {"path":"crates/alpha/src","kind":"directory"}
            ])
        );
        assert_eq!(nested["path"], "crates/alpha");
        assert_eq!(root["consistency"], "best_effort");
    }

    #[tokio::test]
    async fn request_validation_and_target_failures_are_closed() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("file.txt"), b"file").unwrap();
        fixture.commit_all();
        let tool = RepositoryListTool::new(native_git(), &fixture.0).unwrap();
        for input in [
            json!({"path":""}),
            json!({"path":"."}),
            json!({"path":".."}),
            json!({"path":"/absolute"}),
            json!({"path":"leading/"}),
            json!({"path":"trailing/"}),
            json!({"path":"a//b"}),
            json!({"path":"a\\b"}),
            json!({"path":"a:b"}),
            json!({"path":"a/.git/b"}),
            json!({"path":"a/.GIT/b"}),
            json!({"path":"a\0b"}),
            json!({"path":"file.txt","query":"unexpected"}),
            json!({"path":null}),
            json!("not an object"),
        ] {
            assert!(
                tool.execute(ToolInput(input), ToolContext::default())
                    .await
                    .is_err()
            );
        }
        assert!(
            tool.execute(ToolInput(json!({"path":"missing"})), ToolContext::default(),)
                .await
                .is_err()
        );
        assert!(
            tool.execute(
                ToolInput(json!({"path":"file.txt"})),
                ToolContext::default(),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn direct_child_saturation_is_deterministic() {
        let fixture = Fixture::new();
        for index in 0..130 {
            fs::write(fixture.0.join(format!("entry-{index:03}.txt")), b"entry").unwrap();
        }
        fixture.commit_all();
        let tool = RepositoryListTool::new(native_git(), &fixture.0).unwrap();
        let output = list(&tool, json!({})).await;
        assert_eq!(output["complete"], false);
        assert_eq!(output["truncation_reason"], "result_limit");
        let entries = output["entries"].as_array().unwrap();
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0]["path"], "entry-000.txt");
        assert_eq!(entries[127]["path"], "entry-127.txt");
    }

    #[tokio::test]
    async fn deep_tracked_path_lists_in_ordinary_and_linked_worktrees() {
        let ordinary = Fixture::new();
        fs::create_dir_all(ordinary.0.join("a/b/c")).unwrap();
        fs::write(ordinary.0.join("a/b/c/file.txt"), b"ordinary\n").unwrap();
        ordinary.commit_all();
        let ordinary_tool = RepositoryListTool::new(native_git(), &ordinary.0).unwrap();

        let linked = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
        fs::create_dir_all(linked.linked_a.join("a/b/c")).unwrap();
        fs::write(linked.linked_a.join("a/b/c/file.txt"), b"linked\n").unwrap();
        crate::repository_git_layout::test_fixture::run(
            &linked.git,
            &linked.linked_a,
            &["add", "--all"],
        );
        crate::repository_git_layout::test_fixture::run(
            &linked.git,
            &linked.linked_a,
            &["commit", "--quiet", "-m", "deep path"],
        );
        let linked_tool = RepositoryListTool::new(native_git(), &linked.linked_a).unwrap();

        let ordinary_expected_root = json!([{"path":"a","kind":"directory"}]);
        let linked_expected_root =
            json!([{"path":"a","kind":"directory"},{"path":"tracked.txt","kind":"file"}]);
        for (tool, expected_root, expected_entries) in [
            (&ordinary_tool, "a", ordinary_expected_root),
            (&linked_tool, "a", linked_expected_root),
        ] {
            let root = list(tool, json!({})).await;
            assert_eq!(root["entries"], expected_entries);
            let a = list(tool, json!({"path":"a"})).await;
            assert_eq!(a["entries"], json!([{"path":"a/b","kind":"directory"}]));
            let b = list(tool, json!({"path":"a/b"})).await;
            assert_eq!(b["entries"], json!([{"path":"a/b/c","kind":"directory"}]));
            let c = list(tool, json!({"path":"a/b/c"})).await;
            assert_eq!(
                c["entries"],
                json!([{"path":"a/b/c/file.txt","kind":"file"}])
            );
            assert_eq!(root["entries"][0]["path"], expected_root);
        }

        fs::remove_dir_all(ordinary.0.join("a")).unwrap();
        let omitted = list(&ordinary_tool, json!({})).await;
        assert_eq!(omitted["entries"], json!([]));
        assert_eq!(omitted["omitted"]["changed_or_missing"], 1);
    }

    #[test]
    fn conflicting_file_and_directory_projection_fails_closed() {
        let mut entries = BTreeMap::new();
        insert_entry(&mut entries, "a".to_owned(), "file").unwrap();
        insert_entry(&mut entries, "a".to_owned(), "file").unwrap();
        assert!(insert_entry(&mut entries, "a".to_owned(), "directory").is_err());
        assert_eq!(entries, BTreeMap::from([(String::from("a"), "file")]));
    }

    #[tokio::test]
    async fn nested_repository_fails_without_partial_entries_and_observation_does_not_mutate() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("visible.txt"), b"visible").unwrap();
        fixture.commit_all();
        let nested = fixture.0.join("nested");
        fs::create_dir(&nested).unwrap();
        git(&nested, &["init", "--quiet"]);
        let before = Command::new(native_git())
            .args(["status", "--porcelain=v1"])
            .current_dir(&fixture.0)
            .output()
            .unwrap()
            .stdout;
        let tool = RepositoryListTool::new(native_git(), &fixture.0).unwrap();
        assert!(
            tool.execute(ToolInput(json!({})), ToolContext::default())
                .await
                .is_err()
        );
        let after = Command::new(native_git())
            .args(["status", "--porcelain=v1"])
            .current_dir(&fixture.0)
            .output()
            .unwrap()
            .stdout;
        assert_eq!(before, after);
    }
}
