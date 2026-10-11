//! Bounded, host-owned tracked repository file statistics.

use std::{collections::BTreeMap, path::Path, time::Instant};

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

/// Stable name for the fixed, read-only repository statistics observer.
pub const REPOSITORY_STATS_TOOL_NAME: &str = "repo.stats";

const MAX_REQUEST_BYTES: usize = 4 * 1024;
const MAX_EXTENSION_GROUPS: usize = 64;
const MAX_DIRECTORY_GROUPS: usize = 128;
const MAX_EXTENSION_BYTES: usize = 32;
const MAX_RESULT_BYTES: usize = 128 * 1024;

/// One host-configured repository file statistics Tool.
pub struct RepositoryStatsTool {
    observer: RepositoryObserver,
}

impl RepositoryStatsTool {
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
impl Tool for RepositoryStatsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_STATS_TOOL_NAME),
            description: "Counts tracked files in the host-selected repository, recursively, with total byte sizes grouped by file extension and by direct child directory. Use this to count or summarize files instead of paging through repo.list or repo.search. For the whole repository, pass {} and omit path. For a subdirectory, pass a safe repository-relative path such as {\"path\":\"crates\"}. Untracked and ignored files are not counted; file contents are not read."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional nonempty subdirectory relative to the selected repository, using '/' between components. Omit this field for the whole repository. No '.' or '..' components, .git components, absolute paths, backslashes, colons or NUL; at most 1024 UTF-8 bytes.",
                        "minLength": 1,
                        "maxLength": OBSERVER_MAX_PATH_BYTES
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
        let request = StatsRequest::parse(&input)?;
        tokio::time::timeout(SEARCH_TIMEOUT, self.execute_bounded(request))
            .await
            .map_err(|_| stats_error("exceeded its total timeout"))?
    }
}

impl RepositoryStatsTool {
    async fn execute_bounded(&self, request: StatsRequest) -> Result<ToolOutput, ToolError> {
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

        let mut total = Count::default();
        let mut direct_files = Count::default();
        let mut extensions = BTreeMap::<Option<String>, Count>::new();
        let mut directories = BTreeMap::<String, Count>::new();
        let mut omitted = Omitted {
            non_addressable_path: inventory.non_addressable_path,
            ..Omitted::default()
        };
        let mut matched = request.path.is_none();

        for path in inventory.paths {
            let Some(remainder) = descendant(request.path.as_deref(), &path) else {
                if request.path.as_deref() == Some(path.as_str()) {
                    return Err(invalid("requested repository path is not a directory"));
                }
                continue;
            };
            matched = true;
            ensure_time(started)?;
            let target = repository_target_path(self.observer.root(), &path);
            self.observer.validate_target(&target)?;
            let bytes = match inspect_tracked_candidate(&target)? {
                TrackedCandidate::Eligible(metadata) => metadata.len(),
                TrackedCandidate::Missing => {
                    omitted.changed_or_missing += 1;
                    continue;
                }
                TrackedCandidate::NonRegular => {
                    omitted.non_regular += 1;
                    continue;
                }
            };
            total.add(bytes);
            extensions.entry(extension(&path)).or_default().add(bytes);
            match remainder.split_once('/') {
                Some((child, _)) => {
                    let child = match request.path.as_deref() {
                        Some(prefix) => format!("{prefix}/{child}"),
                        None => child.to_owned(),
                    };
                    directories.entry(child).or_default().add(bytes);
                }
                None => direct_files.add(bytes),
            }
        }

        if !matched {
            return Err(invalid("requested repository directory was not found"));
        }
        self.observer.revalidate()?;

        let mut truncation_reasons = Vec::new();
        let mut extensions = extensions.into_iter().collect::<Vec<_>>();
        // Most common first; ties stay in deterministic extension order.
        extensions.sort_by_key(|(_, count)| std::cmp::Reverse(count.files));
        let mut other_extensions = Count::default();
        if extensions.len() > MAX_EXTENSION_GROUPS {
            for (_, count) in extensions.drain(MAX_EXTENSION_GROUPS..) {
                other_extensions.merge(count);
            }
            truncation_reasons.push("extension_group_limit");
        }
        let mut directories = directories.into_iter().collect::<Vec<_>>();
        let mut other_directories = Count::default();
        if directories.len() > MAX_DIRECTORY_GROUPS {
            for (_, count) in directories.drain(MAX_DIRECTORY_GROUPS..) {
                other_directories.merge(count);
            }
            truncation_reasons.push("directory_group_limit");
        }

        let output = ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": "ok",
                "consistency": "best_effort",
                "path": request.path,
                "files": total.files,
                "bytes": total.bytes,
                "extensions": extensions
                    .into_iter()
                    .map(|(extension, count)| json!({"extension": extension, "files": count.files, "bytes": count.bytes}))
                    .collect::<Vec<_>>(),
                "other_extensions": other_extensions.json(),
                "directories": directories
                    .into_iter()
                    .map(|(path, count)| json!({"path": path, "files": count.files, "bytes": count.bytes}))
                    .collect::<Vec<_>>(),
                "other_directories": other_directories.json(),
                "direct_files": direct_files.json(),
                "complete_groups": truncation_reasons.is_empty(),
                "truncation_reasons": truncation_reasons,
                "omitted": {
                    "non_addressable_path": omitted.non_addressable_path,
                    "non_regular": omitted.non_regular,
                    "changed_or_missing": omitted.changed_or_missing
                }
            }))],
            is_error: false,
        };
        let size = serde_json::to_vec(&output)
            .map_err(|_| stats_error("result could not be serialized"))?
            .len();
        if size > MAX_RESULT_BYTES {
            return Err(stats_error("result exceeded its limit"));
        }
        Ok(output)
    }
}

struct StatsRequest {
    path: Option<String>,
}

impl StatsRequest {
    fn parse(input: &ToolInput) -> Result<Self, ToolError> {
        let serialized =
            serde_json::to_vec(&input.0).map_err(|_| invalid("input is not serializable"))?;
        if serialized.len() > MAX_REQUEST_BYTES {
            return Err(invalid("repository statistics input exceeds its limit"));
        }
        let object = input
            .0
            .as_object()
            .ok_or_else(|| invalid("repository statistics input must be an object"))?;
        if object.keys().any(|key| key != "path") {
            return Err(invalid("repository statistics input has unknown fields"));
        }
        let path = match object.get("path") {
            None => None,
            Some(value) => {
                let path = value
                    .as_str()
                    .ok_or_else(|| invalid("`path` must be a string"))?;
                if !crate::repository_observer::is_safe_repository_path(
                    path,
                    OBSERVER_MAX_PATH_BYTES,
                ) {
                    return Err(invalid("`path` must be a safe repository-relative path"));
                }
                Some(path.to_owned())
            }
        };
        Ok(Self { path })
    }
}

/// Returns the path below `prefix`, or the whole path for the root.
fn descendant<'a>(prefix: Option<&str>, path: &'a str) -> Option<&'a str> {
    match prefix {
        None => Some(path),
        Some(prefix) => path.strip_prefix(prefix)?.strip_prefix('/'),
    }
}

/// Lowercase ASCII extension of the final component. Dotfiles such as
/// `.gitignore`, and overlong or non-alphanumeric extensions, have none.
fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty()
        || extension.is_empty()
        || extension.len() > MAX_EXTENSION_BYTES
        || !extension
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

#[derive(Clone, Copy, Default)]
struct Count {
    files: u64,
    bytes: u64,
}

impl Count {
    fn add(&mut self, bytes: u64) {
        self.files += 1;
        self.bytes = self.bytes.saturating_add(bytes);
    }

    fn merge(&mut self, other: Count) {
        self.files += other.files;
        self.bytes = self.bytes.saturating_add(other.bytes);
    }

    fn json(self) -> serde_json::Value {
        json!({"files": self.files, "bytes": self.bytes})
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
        Err(stats_error("exceeded its total timeout"))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidInput {
        message: message.into(),
    }
}

fn stats_error(message: &str) -> ToolError {
    ToolError::Execution {
        message: format!("repository statistics {message}"),
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

    use serde_json::{Value, json};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-repository-stats-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            git(&root, &["init", "--quiet"]);
            git(&root, &["config", "user.email", "stats@example.invalid"]);
            git(&root, &["config", "user.name", "RAH stats test"]);
            Self(root)
        }

        fn write(&self, path: &str, bytes: &[u8]) {
            let target = self.0.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }

        fn commit_all(&self) {
            git(&self.0, &["add", "--all"]);
            git(&self.0, &["commit", "--quiet", "-m", "stats fixture"]);
        }

        fn tool(&self) -> RepositoryStatsTool {
            RepositoryStatsTool::new(native_git(), &self.0).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn native_git() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        assert!(output.status.success());
        fs::canonicalize(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
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

    async fn stats(tool: &RepositoryStatsTool, input: Value) -> Value {
        let output = tool
            .execute(ToolInput(input), ToolContext::default())
            .await
            .unwrap();
        match output.content.as_slice() {
            [ToolContent::Json(value)] => value.clone(),
            other => panic!("unexpected output: {other:?}"),
        }
    }

    #[test]
    fn definition_is_closed_and_execute_bound() {
        let fixture = Fixture::new();
        let definition = fixture.tool().definition();
        assert_eq!(definition.name.as_str(), REPOSITORY_STATS_TOOL_NAME);
        assert_eq!(definition.permission, PermissionLevel::Execute);
        assert_eq!(definition.input_schema["additionalProperties"], false);
        assert!(definition.input_schema.get("required").is_none());
        assert_eq!(
            definition.input_schema["properties"]["path"]["maxLength"],
            OBSERVER_MAX_PATH_BYTES
        );
    }

    #[test]
    fn extensions_are_normalized_and_bounded() {
        assert_eq!(extension("src/lib.rs").as_deref(), Some("rs"));
        assert_eq!(extension("README.MD").as_deref(), Some("md"));
        assert_eq!(extension("a/archive.tar.gz").as_deref(), Some("gz"));
        for path in [
            ".gitignore",
            "dir/.env",
            "Makefile",
            "trailing.",
            "weird.e x",
            &format!("long.{}", "x".repeat(MAX_EXTENSION_BYTES + 1)),
        ] {
            assert_eq!(extension(path), None, "{path}");
        }
    }

    #[tokio::test]
    async fn counts_tracked_files_recursively_by_extension_and_directory() {
        let fixture = Fixture::new();
        fixture.write("README.md", b"12345");
        fixture.write(".gitignore", b"ignored/\n");
        fixture.write("crates/a/src/lib.rs", b"123");
        fixture.write("crates/a/Cargo.toml", b"12");
        fixture.write("crates/b/src/main.rs", b"1234");
        fixture.write("docs/guide.md", b"1");
        fixture.commit_all();
        fixture.write("untracked.rs", b"untracked");
        fixture.write("ignored/big.bin", b"ignored");

        let tool = fixture.tool();
        let root = stats(&tool, json!({})).await;
        assert_eq!(root["status"], "ok");
        assert_eq!(root["files"], 6);
        assert_eq!(root["bytes"], 5 + 9 + 3 + 2 + 4 + 1);
        assert_eq!(
            root["extensions"],
            json!([
                {"extension":"md","files":2,"bytes":6},
                {"extension":"rs","files":2,"bytes":7},
                {"extension":null,"files":1,"bytes":9},
                {"extension":"toml","files":1,"bytes":2}
            ])
        );
        assert_eq!(
            root["directories"],
            json!([
                {"path":"crates","files":3,"bytes":9},
                {"path":"docs","files":1,"bytes":1}
            ])
        );
        assert_eq!(root["direct_files"], json!({"files":2,"bytes":14}));
        assert_eq!(root["complete_groups"], true);
        assert!(!root.to_string().contains("untracked"));
        assert!(!root.to_string().contains("ignored/"));

        let crates = stats(&tool, json!({"path":"crates"})).await;
        assert_eq!(crates["path"], "crates");
        assert_eq!(crates["files"], 3);
        assert_eq!(
            crates["directories"],
            json!([
                {"path":"crates/a","files":2,"bytes":5},
                {"path":"crates/b","files":1,"bytes":4}
            ])
        );
        assert_eq!(crates["direct_files"], json!({"files":0,"bytes":0}));
    }

    #[tokio::test]
    async fn group_limits_fold_into_other_without_losing_totals() {
        let fixture = Fixture::new();
        for index in 0..(MAX_EXTENSION_GROUPS + 2) {
            fixture.write(&format!("d{index:03}/file.e{index:03}"), b"x");
        }
        for index in 0..(MAX_DIRECTORY_GROUPS - MAX_EXTENSION_GROUPS) {
            fixture.write(&format!("z{index:03}/file.rs"), b"x");
        }
        fixture.commit_all();
        let output = stats(&fixture.tool(), json!({})).await;
        let files = (MAX_EXTENSION_GROUPS + 2 + MAX_DIRECTORY_GROUPS - MAX_EXTENSION_GROUPS) as u64;
        assert_eq!(output["files"], files);
        assert_eq!(output["complete_groups"], false);
        assert_eq!(
            output["truncation_reasons"],
            json!(["extension_group_limit", "directory_group_limit"])
        );
        assert_eq!(
            output["extensions"].as_array().unwrap().len(),
            MAX_EXTENSION_GROUPS
        );
        assert_eq!(output["extensions"][0]["extension"], "rs");
        assert_eq!(
            output["directories"].as_array().unwrap().len(),
            MAX_DIRECTORY_GROUPS
        );
        let sum = |groups: &Value, other: &Value| {
            groups
                .as_array()
                .unwrap()
                .iter()
                .map(|group| group["files"].as_u64().unwrap())
                .sum::<u64>()
                + other["files"].as_u64().unwrap()
        };
        assert_eq!(
            sum(&output["extensions"], &output["other_extensions"]),
            files
        );
        assert_eq!(
            sum(&output["directories"], &output["other_directories"]),
            files
        );
    }

    #[tokio::test]
    async fn invalid_missing_and_file_targets_are_correctable_rejections() {
        let fixture = Fixture::new();
        fixture.write("src/lib.rs", b"lib");
        fixture.commit_all();
        let tool = fixture.tool();
        for (input, message) in [
            (
                json!({"path":"missing"}),
                "requested repository directory was not found",
            ),
            (
                json!({"path":"src/lib.rs"}),
                "requested repository path is not a directory",
            ),
            (
                json!({"path":"../outside"}),
                "`path` must be a safe repository-relative path",
            ),
            (
                json!({"path":"src","extra":true}),
                "repository statistics input has unknown fields",
            ),
        ] {
            assert_eq!(
                tool.execute(ToolInput(input), ToolContext::default())
                    .await
                    .unwrap_err(),
                ToolError::InvalidInput {
                    message: message.into()
                }
            );
        }
        for path in ["", ".", "/abs", "a\\b", "a:b", "a/.git/b", "a//b"] {
            assert!(matches!(
                StatsRequest::parse(&ToolInput(json!({"path":path}))),
                Err(ToolError::InvalidInput { .. })
            ));
        }
    }

    #[tokio::test]
    async fn missing_tracked_file_is_omitted_not_counted() {
        let fixture = Fixture::new();
        fixture.write("keep.rs", b"keep");
        fixture.write("gone.rs", b"gone");
        fixture.commit_all();
        fs::remove_file(fixture.0.join("gone.rs")).unwrap();
        let output = stats(&fixture.tool(), json!({})).await;
        assert_eq!(output["files"], 1);
        assert_eq!(output["omitted"]["changed_or_missing"], 1);
    }

    #[tokio::test]
    async fn nested_repository_fails_closed() {
        let fixture = Fixture::new();
        fixture.write("visible.txt", b"visible");
        fixture.commit_all();
        let nested = fixture.0.join("nested");
        fs::create_dir(&nested).unwrap();
        git(&nested, &["init", "--quiet"]);
        assert!(
            fixture
                .tool()
                .execute(ToolInput(json!({})), ToolContext::default())
                .await
                .is_err()
        );
    }
}
