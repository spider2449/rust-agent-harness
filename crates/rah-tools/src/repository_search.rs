//! Bounded, host-owned tracked repository discovery.
//!
//! Git supplies only a fixed tracked-file inventory. Every query, path
//! constraint, filesystem eligibility decision, and match is evaluated by
//! RAH-owned Rust code against the selected worktree.

use std::{path::Path, time::Instant};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;

use crate::{
    Tool, ToolContext, ToolError,
    git_support::git_error,
    repository_observer::{
        ObserverCommand, RepositoryObserver, SEARCH_TIMEOUT, TrackedCandidate, TrackedInventory,
        inspect_tracked_candidate, is_safe_repository_path, open_regular_no_follow,
        parse_tracked_inventory, repository_target_path, successful_tracked_inventory,
    },
};

/// Stable name for the fixed, read-only repository discovery observer.
pub const REPOSITORY_SEARCH_TOOL_NAME: &str = "repo.search";

const MAX_REQUEST_BYTES: usize = 4 * 1024;
const MAX_QUERY_BYTES: usize = 256;
const MAX_PREFIX_BYTES: usize = 1024;
const MAX_PATH_RESULTS: usize = 128;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_AGGREGATE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TEXT_RESULT_FILES: usize = 64;
const MAX_LINES_PER_FILE: usize = 8;
const MAX_TOTAL_LINES: usize = 128;
const MAX_RESULT_BYTES: usize = 128 * 1024;

/// One host-configured repository discovery Tool.
pub struct RepositorySearchTool {
    observer: RepositoryObserver,
}

impl RepositorySearchTool {
    /// Creates the observer for one host-selected native Git executable and
    /// repository. The model never supplies either resource.
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
impl Tool for RepositorySearchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_SEARCH_TOOL_NAME),
            description:
                "Finds tracked, present files or matching lines in the selected repository."
                    .to_owned(),
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["mode", "query"],
                "properties": {
                    "mode": {"type": "string", "enum": ["path", "text"]},
                    "query": {"type": "string"},
                    "path_prefix": {"type": "string"}
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
        let request = SearchRequest::parse(&input)?;
        tokio::time::timeout(SEARCH_TIMEOUT, self.execute_bounded(request))
            .await
            .map_err(|_| search_error("repository search exceeded its total timeout"))?
    }
}

impl RepositorySearchTool {
    async fn execute_bounded(&self, request: SearchRequest) -> Result<ToolOutput, ToolError> {
        let started = Instant::now();
        let _lease = self.observer.acquire_lease().await;
        self.observer.revalidate()?;

        let inventory = self
            .observer
            .run(ObserverCommand::TrackedInventory, None, started)
            .await?;
        let inventory = successful_tracked_inventory(inventory)?;
        self.observer.revalidate()?;
        let mut candidates = parse_tracked_inventory(&inventory)?;
        candidates.paths.retain(|path| {
            request
                .path_prefix
                .as_deref()
                .is_none_or(|prefix| path == prefix || path.starts_with(&format!("{prefix}/")))
        });

        let result = match request.mode {
            SearchMode::Path => self.search_paths(&request, candidates, started)?,
            SearchMode::Text => self.search_text(&request, candidates, started).await?,
        };
        self.observer.revalidate()?;
        bounded_output(request.mode, result)
    }

    fn search_paths(
        &self,
        request: &SearchRequest,
        candidates: TrackedInventory,
        started: Instant,
    ) -> Result<SearchResult, ToolError> {
        let mut matches = Vec::new();
        let mut omitted = PathOmitted {
            non_addressable_path: candidates.non_addressable_path,
            ..PathOmitted::default()
        };
        let mut truncation_reason = None;
        for path in candidates.paths {
            ensure_time(started)?;
            let target = repository_target_path(self.observer.root(), &path);
            self.observer.validate_target(&target)?;
            match inspect_tracked_candidate(&target)? {
                TrackedCandidate::Eligible(_) => {}
                TrackedCandidate::Missing => {
                    omitted.changed_or_missing += 1;
                    continue;
                }
                TrackedCandidate::NonRegular => {
                    omitted.non_regular += 1;
                    continue;
                }
            }
            if path.contains(&request.query) {
                matches.push(PathMatch { path });
                if matches.len() == MAX_PATH_RESULTS {
                    truncation_reason = Some(TruncationReason::ResultLimit);
                    break;
                }
            }
        }
        Ok(SearchResult {
            complete: truncation_reason.is_none(),
            truncation_reason,
            matches: SearchMatches::Paths(matches),
            omitted: Omitted::Path(omitted),
        })
    }

    async fn search_text(
        &self,
        request: &SearchRequest,
        candidates: TrackedInventory,
        started: Instant,
    ) -> Result<SearchResult, ToolError> {
        let mut matches = Vec::new();
        let mut omitted = TextOmitted {
            non_addressable_path: candidates.non_addressable_path,
            ..TextOmitted::default()
        };
        let mut aggregate_bytes = 0_u64;
        let mut total_lines = 0_usize;
        let mut truncation_reason = None;

        for (index, path) in candidates.paths.iter().enumerate() {
            ensure_time(started)?;
            let target = repository_target_path(self.observer.root(), path);
            self.observer.validate_target(&target)?;
            let metadata = match inspect_tracked_candidate(&target)? {
                TrackedCandidate::Eligible(metadata) => metadata,
                TrackedCandidate::Missing => {
                    omitted.changed_or_missing += 1;
                    continue;
                }
                TrackedCandidate::NonRegular => {
                    omitted.non_regular += 1;
                    continue;
                }
            };
            if metadata.len() > MAX_FILE_BYTES {
                omitted.oversized += 1;
                continue;
            }
            if aggregate_bytes.saturating_add(metadata.len()) > MAX_AGGREGATE_BYTES {
                truncation_reason = Some(TruncationReason::ScanByteLimit);
                break;
            }

            let remaining = SEARCH_TIMEOUT
                .checked_sub(started.elapsed())
                .ok_or_else(|| search_error("repository search exceeded its total timeout"))?;
            let read = tokio::time::timeout(remaining, read_current_file(&target, metadata.len()))
                .await
                .map_err(|_| search_error("repository search exceeded its total timeout"))?;
            aggregate_bytes = aggregate_bytes.saturating_add(read.bytes_read);
            let bytes = match read.result {
                ReadResult::Changed => {
                    omitted.changed_or_missing += 1;
                    continue;
                }
                ReadResult::BinaryOrInvalid => {
                    omitted.non_text += 1;
                    continue;
                }
                ReadResult::Text(bytes) => bytes,
            };
            let lines = matching_lines(&bytes, request.query.as_bytes());
            if lines.is_empty() {
                if aggregate_bytes == MAX_AGGREGATE_BYTES && index + 1 < candidates.paths.len() {
                    truncation_reason = Some(TruncationReason::ScanByteLimit);
                    break;
                }
                continue;
            }
            if lines.len() > MAX_LINES_PER_FILE {
                let remaining_lines = MAX_TOTAL_LINES.saturating_sub(total_lines);
                if remaining_lines == 0 {
                    truncation_reason = Some(TruncationReason::ResultLimit);
                    break;
                }
                let lines = lines[..MAX_LINES_PER_FILE.min(remaining_lines)].to_vec();
                matches.push(TextMatch {
                    path: path.clone(),
                    lines,
                });
                truncation_reason = Some(TruncationReason::ResultLimit);
                break;
            }
            if matches.len() == MAX_TEXT_RESULT_FILES
                || total_lines.saturating_add(lines.len()) > MAX_TOTAL_LINES
            {
                if matches.len() == MAX_TEXT_RESULT_FILES {
                    truncation_reason = Some(TruncationReason::ResultLimit);
                    break;
                }
                let remaining_lines = MAX_TOTAL_LINES.saturating_sub(total_lines);
                if remaining_lines == 0 {
                    truncation_reason = Some(TruncationReason::ResultLimit);
                    break;
                }
                let lines = lines[..remaining_lines].to_vec();
                matches.push(TextMatch {
                    path: path.clone(),
                    lines,
                });
                truncation_reason = Some(TruncationReason::ResultLimit);
                break;
            }
            total_lines += lines.len();
            matches.push(TextMatch {
                path: path.clone(),
                lines,
            });
            if matches.len() == MAX_TEXT_RESULT_FILES || total_lines == MAX_TOTAL_LINES {
                truncation_reason = Some(TruncationReason::ResultLimit);
                break;
            }
            if aggregate_bytes == MAX_AGGREGATE_BYTES && index + 1 < candidates.paths.len() {
                truncation_reason = Some(TruncationReason::ScanByteLimit);
                break;
            }
        }

        Ok(SearchResult {
            complete: truncation_reason.is_none(),
            truncation_reason,
            matches: SearchMatches::Text(matches),
            omitted: Omitted::Text(omitted),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchMode {
    Path,
    Text,
}

impl SearchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Text => "text",
        }
    }
}

struct SearchRequest {
    mode: SearchMode,
    query: String,
    path_prefix: Option<String>,
}

impl SearchRequest {
    fn parse(input: &ToolInput) -> Result<Self, ToolError> {
        let serialized =
            serde_json::to_vec(&input.0).map_err(|_| invalid("input is not serializable"))?;
        if serialized.len() > MAX_REQUEST_BYTES {
            return Err(invalid("repository search input exceeds its limit"));
        }
        let object = input
            .0
            .as_object()
            .ok_or_else(|| invalid("repository search input must be an object"))?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "mode" | "query" | "path_prefix"))
            || !object.contains_key("mode")
            || !object.contains_key("query")
        {
            return Err(invalid(
                "repository search input has unknown or missing fields",
            ));
        }
        let mode = match object.get("mode").and_then(Value::as_str) {
            Some("path") => SearchMode::Path,
            Some("text") => SearchMode::Text,
            _ => return Err(invalid("`mode` must be `path` or `text`")),
        };
        let query = object
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("`query` must be a string"))?;
        validate_query(query)?;
        let path_prefix = match object.get("path_prefix") {
            None => None,
            Some(value) => {
                let prefix = value
                    .as_str()
                    .ok_or_else(|| invalid("`path_prefix` must be a string"))?;
                validate_path(prefix, MAX_PREFIX_BYTES, "path_prefix")?;
                Some(prefix.to_owned())
            }
        };
        Ok(Self {
            mode,
            query: query.to_owned(),
            path_prefix,
        })
    }
}

fn validate_query(query: &str) -> Result<(), ToolError> {
    if query.is_empty()
        || query.len() > MAX_QUERY_BYTES
        || query.contains('\0')
        || query.contains(['\r', '\n'])
    {
        return Err(invalid(
            "`query` must be 1..=256 UTF-8 bytes without NUL, CR, or LF",
        ));
    }
    Ok(())
}

fn validate_path(path: &str, limit: usize, label: &str) -> Result<(), ToolError> {
    if path.is_empty()
        || path.len() > limit
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains(':')
        || path.contains('\0')
    {
        return Err(invalid(format!(
            "`{label}` must be a slash-separated repository-relative path"
        )));
    }
    if !is_safe_repository_path(path, limit) {
        return Err(invalid(format!(
            "`{label}` contains a forbidden path component"
        )));
    }
    Ok(())
}

struct ReadFile {
    result: ReadResult,
    bytes_read: u64,
}

enum ReadResult {
    Text(Vec<u8>),
    BinaryOrInvalid,
    Changed,
}

async fn read_current_file(path: &Path, expected_len: u64) -> ReadFile {
    let file = match open_regular_no_follow(path) {
        Ok(file) => file,
        Err(_) => {
            return ReadFile {
                result: ReadResult::Changed,
                bytes_read: 0,
            };
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.is_file() && metadata.len() == expected_len => metadata,
        _ => {
            return ReadFile {
                result: ReadResult::Changed,
                bytes_read: 0,
            };
        }
    };
    let mut file = tokio::fs::File::from_std(file);
    let mut limited = file.take(expected_len);
    let mut bytes = Vec::with_capacity(expected_len as usize);
    if limited.read_to_end(&mut bytes).await.is_err() {
        return ReadFile {
            bytes_read: bytes.len() as u64,
            result: ReadResult::Changed,
        };
    }
    file = limited.into_inner();
    let after = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(_) => {
            return ReadFile {
                bytes_read: bytes.len() as u64,
                result: ReadResult::Changed,
            };
        }
    };
    if !after.is_file() || after.len() != metadata.len() || bytes.len() as u64 != expected_len {
        return ReadFile {
            bytes_read: bytes.len() as u64,
            result: ReadResult::Changed,
        };
    }
    if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
        return ReadFile {
            bytes_read: bytes.len() as u64,
            result: ReadResult::BinaryOrInvalid,
        };
    }
    ReadFile {
        bytes_read: bytes.len() as u64,
        result: ReadResult::Text(bytes),
    }
}

fn matching_lines(bytes: &[u8], query: &[u8]) -> Vec<u32> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut line = 1_u32;
    for end in 0..=bytes.len() {
        if end != bytes.len() && bytes[end] != b'\n' {
            continue;
        }
        let mut current = &bytes[start..end];
        if current.ends_with(b"\r") {
            current = &current[..current.len() - 1];
        }
        if current.windows(query.len()).any(|window| window == query) {
            result.push(line);
        }
        if end == bytes.len() {
            break;
        }
        line += 1;
        start = end + 1;
    }
    result
}

fn ensure_time(started: Instant) -> Result<(), ToolError> {
    if started.elapsed() >= SEARCH_TIMEOUT {
        Err(search_error("repository search exceeded its total timeout"))
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct PathOmitted {
    non_addressable_path: u64,
    non_regular: u64,
    changed_or_missing: u64,
}

#[derive(Default)]
struct TextOmitted {
    non_addressable_path: u64,
    non_regular: u64,
    oversized: u64,
    non_text: u64,
    changed_or_missing: u64,
}

enum SearchMatches {
    Paths(Vec<PathMatch>),
    Text(Vec<TextMatch>),
}

struct PathMatch {
    path: String,
}

struct TextMatch {
    path: String,
    lines: Vec<u32>,
}

enum Omitted {
    Path(PathOmitted),
    Text(TextOmitted),
}

#[derive(Clone, Copy)]
enum TruncationReason {
    ResultLimit,
    ScanByteLimit,
}

struct SearchResult {
    complete: bool,
    truncation_reason: Option<TruncationReason>,
    matches: SearchMatches,
    omitted: Omitted,
}

fn bounded_output(mode: SearchMode, result: SearchResult) -> Result<ToolOutput, ToolError> {
    let truncation_reason = result.truncation_reason.map(|reason| match reason {
        TruncationReason::ResultLimit => "result_limit",
        TruncationReason::ScanByteLimit => "scan_byte_limit",
    });
    let (matches, omitted) = match result.matches {
        SearchMatches::Paths(matches) => (
            matches
                .into_iter()
                .map(|entry| json!({"path": entry.path}))
                .collect::<Vec<_>>(),
            match result.omitted {
                Omitted::Path(value) => json!({
                    "non_addressable_path": value.non_addressable_path,
                    "non_regular": value.non_regular,
                    "changed_or_missing": value.changed_or_missing
                }),
                Omitted::Text(_) => {
                    return Err(search_error("repository search result shape changed"));
                }
            },
        ),
        SearchMatches::Text(matches) => (
            matches
                .into_iter()
                .map(|entry| json!({"path": entry.path, "lines": entry.lines}))
                .collect::<Vec<_>>(),
            match result.omitted {
                Omitted::Text(value) => json!({
                    "non_addressable_path": value.non_addressable_path,
                    "non_regular": value.non_regular,
                    "oversized": value.oversized,
                    "non_text": value.non_text,
                    "changed_or_missing": value.changed_or_missing
                }),
                Omitted::Path(_) => {
                    return Err(search_error("repository search result shape changed"));
                }
            },
        ),
    };
    let output = ToolOutput {
        content: vec![ToolContent::Json(json!({
            "status": "ok",
            "consistency": "best_effort",
            "mode": mode.as_str(),
            "complete": result.complete,
            "truncation_reason": truncation_reason,
            "matches": matches,
            "omitted": omitted
        }))],
        is_error: false,
    };
    let size = serde_json::to_vec(&output)
        .map_err(|_| search_error("repository search result could not be serialized"))?
        .len();
    if size > MAX_RESULT_BYTES {
        return Err(search_error("repository search result exceeded its limit"));
    }
    Ok(output)
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidInput {
        message: message.into(),
    }
}

fn search_error(message: impl Into<String>) -> ToolError {
    git_error(format!("repository search {}", message.into()))
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
    use serde_json::json;

    use crate::repository_observer::OBSERVER_MAX_RECORDS;
    use crate::{Tool, ToolContext};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-repository-search-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            let git = native_git();
            run(&git, &root, &["init", "--quiet"]);
            run(&git, &root, &["config", "user.name", "RAH Test"]);
            run(
                &git,
                &root,
                &["config", "user.email", "rah@example.invalid"],
            );
            run(&git, &root, &["config", "core.autocrlf", "false"]);
            Self(root)
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

    fn run(git: &Path, root: &Path, args: &[&str]) {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Git fixture command failed: {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn json_output(output: ToolOutput) -> Value {
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("expected one JSON ToolOutput")
        };
        value.clone()
    }

    #[test]
    fn request_validation_is_closed_and_bounded() {
        assert!(SearchRequest::parse(&ToolInput(json!({"mode":"path","query":"x"}))).is_ok());
        assert!(
            SearchRequest::parse(&ToolInput(
                json!({"mode":"text","query":"x","path_prefix":"src"})
            ))
            .is_ok()
        );
        for input in [
            json!({"mode":"glob","query":"x"}),
            json!({"mode":"path","query":""}),
            json!({"mode":"path","query":"x\r"}),
            json!({"mode":"path","query":"x\n"}),
            json!({"mode":"path","query":"x\u{0000}"}),
            json!({"mode":"path","query":"x","extra":true}),
            json!({"mode":"path","query":"x","path_prefix":"../src"}),
            json!({"mode":"path","query":"x","path_prefix":"/src"}),
            json!({"mode":"path","query":"x","path_prefix":"."}),
            json!({"mode":"path","query":"x","path_prefix":".."}),
            json!({"mode":"path","query":"x","path_prefix":"src\\lib"}),
            json!({"mode":"path","query":"x","path_prefix":"src:lib"}),
            json!({"mode":"path","query":"x","path_prefix":"src/./lib"}),
            json!({"mode":"path","query":"x","path_prefix":"src/.git/lib"}),
            json!({"mode":"path","query":"x","path_prefix":"src\u{0000}lib"}),
        ] {
            assert!(SearchRequest::parse(&ToolInput(input)).is_err());
        }
        assert!(
            SearchRequest::parse(&ToolInput(json!({
                "mode":"path","query":"x".repeat(MAX_QUERY_BYTES + 1)
            })))
            .is_err()
        );
        assert!(
            SearchRequest::parse(&ToolInput(json!({
                "mode":"path","query":"x","path_prefix":"x".repeat(MAX_REQUEST_BYTES)
            })))
            .is_err()
        );
    }

    #[test]
    fn inventory_is_nul_safe_sorted_deduplicated_and_fail_closed() {
        let inventory = parse_tracked_inventory(b"z.txt\0src/a.rs\0src/a.rs\0").unwrap();
        assert_eq!(inventory.paths, ["src/a.rs", "z.txt"]);
        assert!(parse_tracked_inventory(b"src/a.rs").is_err());
        assert!(parse_tracked_inventory(b"src/a.rs\0\0").is_err());
        let invalid = parse_tracked_inventory(b"ok.txt\0bad\xff\0").unwrap();
        assert_eq!(invalid.paths, ["ok.txt"]);
        assert_eq!(invalid.non_addressable_path, 1);

        let mut oversized = Vec::new();
        for _ in 0..=OBSERVER_MAX_RECORDS {
            oversized.extend_from_slice(b"tracked\0");
        }
        assert!(parse_tracked_inventory(&oversized).is_err());
    }

    #[test]
    fn text_lines_are_literal_single_line_and_crlf_aware() {
        let bytes = "one x x\r\ntwo x\nthree\n".as_bytes();
        assert_eq!(matching_lines(bytes, b"x"), [1, 2]);
        assert_eq!(matching_lines(bytes, b".*[]()"), Vec::<u32>::new());
    }

    #[test]
    fn text_output_preserves_the_non_regular_omission_category() {
        let output = bounded_output(
            SearchMode::Text,
            SearchResult {
                complete: true,
                truncation_reason: None,
                matches: SearchMatches::Text(Vec::new()),
                omitted: Omitted::Text(TextOmitted {
                    non_regular: 1,
                    ..TextOmitted::default()
                }),
            },
        )
        .unwrap();
        let value = json_output(output);
        assert_eq!(value["omitted"]["non_regular"], 1);
    }

    #[tokio::test]
    async fn searches_current_tracked_worktree_and_preserves_repository_state() {
        let fixture = Fixture::new();
        let git = native_git();
        fs::create_dir_all(fixture.0.join("src")).unwrap();
        fs::write(fixture.0.join("tracked.txt"), b"old\n").unwrap();
        fs::write(fixture.0.join("src/lib.rs"), b"library\n").unwrap();
        fs::write(fixture.0.join(".gitignore"), b"ignored.txt\n").unwrap();
        run(
            &git,
            &fixture.0,
            &["add", "--", "tracked.txt", "src/lib.rs", ".gitignore"],
        );
        run(&git, &fixture.0, &["commit", "--quiet", "-m", "initial"]);
        fs::write(fixture.0.join("tracked.txt"), b"current sentinel\n").unwrap();
        fs::write(fixture.0.join("ignored.txt"), b"ignored sentinel\n").unwrap();
        fs::write(fixture.0.join("untracked.txt"), b"untracked sentinel\n").unwrap();

        let head_before = run_output(&git, &fixture.0, &["rev-parse", "HEAD"]);
        let status_before = run_output(&git, &fixture.0, &["status", "--porcelain=v1"]);
        let index_before = fs::read(fixture.0.join(".git/index")).unwrap();
        let tool = RepositorySearchTool::new(&git, &fixture.0).unwrap();
        let definition = tool.definition();
        assert_eq!(definition.name.as_str(), REPOSITORY_SEARCH_TOOL_NAME);
        assert_eq!(definition.permission, PermissionLevel::Execute);
        assert_eq!(
            definition.input_schema,
            json!({
                "type":"object",
                "additionalProperties":false,
                "required":["mode","query"],
                "properties":{
                    "mode":{"type":"string","enum":["path","text"]},
                    "query":{"type":"string"},
                    "path_prefix":{"type":"string"}
                }
            })
        );

        let path = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"lib"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(path["complete"], true);
        assert_eq!(path["matches"], json!([{"path":"src/lib.rs"}]));

        let text = json_output(
            tool.execute(
                ToolInput(json!({"mode":"text","query":"sentinel"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(text["matches"], json!([{"path":"tracked.txt","lines":[1]}]));
        assert!(!text.to_string().contains("ignored.txt"));
        assert!(!text.to_string().contains("untracked.txt"));
        let old_text = json_output(
            tool.execute(
                ToolInput(json!({"mode":"text","query":"old"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(old_text["matches"], json!([]));

        let prefixed = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"rs","path_prefix":"src"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(prefixed["matches"], json!([{"path":"src/lib.rs"}]));
        assert_eq!(
            run_output(&git, &fixture.0, &["rev-parse", "HEAD"]),
            head_before
        );
        assert_eq!(
            run_output(&git, &fixture.0, &["status", "--porcelain=v1"]),
            status_before
        );
        assert_eq!(
            fs::read(fixture.0.join(".git/index")).unwrap(),
            index_before
        );
        fs::create_dir_all(fixture.0.join("nested/.git")).unwrap();
        assert!(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"tracked"})),
                ToolContext::default(),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn path_search_is_literal_case_sensitive_and_result_bounded() {
        let fixture = Fixture::new();
        let git = native_git();
        for index in 0..130 {
            fs::write(
                fixture.0.join(format!("match-{index:03}.txt")),
                b"literal[marker]\n",
            )
            .unwrap();
        }
        fs::write(fixture.0.join("MATCH.txt"), b"literal[marker]\n").unwrap();
        run(&git, &fixture.0, &["add", "--all"]);

        let tool = RepositorySearchTool::new(&git, &fixture.0).unwrap();
        let output = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"match-"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(output["complete"], false);
        assert_eq!(output["truncation_reason"], "result_limit");
        assert_eq!(
            output["matches"].as_array().unwrap().len(),
            MAX_PATH_RESULTS
        );
        assert_eq!(output["matches"][0]["path"], "match-000.txt");

        let case_sensitive = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"MATCH"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(case_sensitive["matches"], json!([{"path":"MATCH.txt"}]));

        let literal = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"[marker]"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(literal["matches"], json!([]));
    }

    #[tokio::test]
    async fn linked_worktree_search_isolated_from_main_and_sibling() {
        let fixture = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
        let git = &fixture.git;
        fs::write(fixture.main.join("main-only.txt"), b"main sentinel\n").unwrap();
        run(git, &fixture.main, &["add", "--", "main-only.txt"]);
        run(
            git,
            &fixture.main,
            &["commit", "--quiet", "-m", "main-only"],
        );
        fs::write(fixture.linked_a.join("a-only.txt"), b"A sentinel\n").unwrap();
        run(git, &fixture.linked_a, &["add", "--", "a-only.txt"]);
        run(
            git,
            &fixture.linked_a,
            &["commit", "--quiet", "-m", "a-only"],
        );
        fs::write(fixture.linked_b.join("b-only.txt"), b"B sentinel\n").unwrap();
        run(git, &fixture.linked_b, &["add", "--", "b-only.txt"]);
        run(
            git,
            &fixture.linked_b,
            &["commit", "--quiet", "-m", "b-only"],
        );

        let heads_before = [
            run_output(git, &fixture.main, &["rev-parse", "HEAD"]),
            run_output(git, &fixture.linked_a, &["rev-parse", "HEAD"]),
            run_output(git, &fixture.linked_b, &["rev-parse", "HEAD"]),
        ];
        let statuses_before = [
            run_output(git, &fixture.main, &["status", "--porcelain=v1"]),
            run_output(git, &fixture.linked_a, &["status", "--porcelain=v1"]),
            run_output(git, &fixture.linked_b, &["status", "--porcelain=v1"]),
        ];
        let tool = RepositorySearchTool::new(git, &fixture.linked_a).unwrap();
        let output = json_output(
            tool.execute(
                ToolInput(json!({"mode":"text","query":"sentinel"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(
            output["matches"],
            json!([{"path":"a-only.txt","lines":[1]}])
        );
        let path_output = json_output(
            tool.execute(
                ToolInput(json!({"mode":"path","query":"-only"})),
                ToolContext::default(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(path_output["matches"], json!([{"path":"a-only.txt"}]));
        assert!(!output.to_string().contains("main-only"));
        assert!(!output.to_string().contains("b-only"));
        assert!(!path_output.to_string().contains("main-only"));
        assert!(!path_output.to_string().contains("b-only"));
        assert_eq!(
            [
                run_output(git, &fixture.main, &["rev-parse", "HEAD"]),
                run_output(git, &fixture.linked_a, &["rev-parse", "HEAD"]),
                run_output(git, &fixture.linked_b, &["rev-parse", "HEAD"]),
            ],
            heads_before
        );
        assert_eq!(
            [
                run_output(git, &fixture.main, &["status", "--porcelain=v1"]),
                run_output(git, &fixture.linked_a, &["status", "--porcelain=v1"]),
                run_output(git, &fixture.linked_b, &["status", "--porcelain=v1"]),
            ],
            statuses_before
        );
    }

    fn run_output(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success());
        output.stdout
    }
}
