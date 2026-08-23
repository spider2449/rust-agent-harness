use std::{fs, io::Read, path::Path, time::Instant};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    Tool, ToolContext, ToolError,
    git_support::git_error,
    repository_observer::{FileInfoCommand, RepositoryObserver, reject_link_or_reparse},
};

/// Stable name for the fixed, read-only one-path repository observer.
pub const REPOSITORY_FILE_INFO_TOOL_NAME: &str = "repo.file-info";

const MAX_REQUEST_BYTES: usize = 4 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_DIGEST_BYTES: u64 = 1024 * 1024;
const MAX_RESULT_BYTES: usize = 128 * 1024;

/// Reports normalized Git and direct-entry facts for one logical repository path.
///
/// The Git executable and repository are trusted host configuration. Model input
/// may select only one validated UTF-8 repository-relative path.
pub struct RepositoryFileInfoTool {
    observer: RepositoryObserver,
}

impl RepositoryFileInfoTool {
    /// Creates the observer for one host-selected native Git executable and repository.
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
impl Tool for RepositoryFileInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_FILE_INFO_TOOL_NAME),
            description: "Reports normalized read-only Git state for one repository-relative path."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string", "maxLength": MAX_PATH_BYTES}},
                "required": ["path"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = FileInfoRequest::parse(&input)?;
        let _lease = self.observer.acquire_lease().await;
        self.observer.revalidate()?;
        let started = Instant::now();

        let index = required_output(
            self.observer
                .run(FileInfoCommand::Index, Some(&request.path), started)
                .await?,
            "index",
        )?;
        let head_output = self
            .observer
            .run(FileInfoCommand::Head, None, started)
            .await?;
        let head = parse_head_result(head_output)?;
        let tree = if head.is_some() {
            required_output(
                self.observer
                    .run(FileInfoCommand::HeadTree, Some(&request.path), started)
                    .await?,
                "HEAD tree",
            )?
        } else {
            Vec::new()
        };
        let status = required_output(
            self.observer
                .run(FileInfoCommand::Status, Some(&request.path), started)
                .await?,
            "status",
        )?;
        self.observer.revalidate()?;

        let index = parse_index(&index, request.path.as_bytes())?;
        let head_entry = parse_tree(&tree, request.path.as_bytes())?;
        let status = parse_status(&status, request.path.as_bytes())?;
        let worktree = observe_worktree(self.observer.root(), &request.path)?;
        let normalized = normalize(&request.path, head, head_entry, index, status, worktree)?;
        bounded_output(normalized)
    }
}

struct FileInfoRequest {
    path: String,
}

impl FileInfoRequest {
    fn parse(input: &ToolInput) -> Result<Self, ToolError> {
        let serialized = serde_json::to_vec(&input.0).map_err(|error| ToolError::InvalidInput {
            message: format!("input could not be serialized: {error}"),
        })?;
        if serialized.len() > MAX_REQUEST_BYTES {
            return Err(invalid(
                "input exceeds the repository file-info request limit",
            ));
        }
        let object = input
            .0
            .as_object()
            .ok_or_else(|| invalid("input must be an object"))?;
        if object.len() != 1 || !object.contains_key("path") {
            return Err(invalid("input must contain only required field `path`"));
        }
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("`path` must be a string"))?;
        validate_path(path)?;
        Ok(Self {
            path: path.to_owned(),
        })
    }
}

fn validate_path(path: &str) -> Result<(), ToolError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains('\0') {
        return Err(invalid(
            "`path` must be nonempty, at most 1024 UTF-8 bytes, and contain no NUL",
        ));
    }
    if path.starts_with('/') || path.starts_with('\\') || path.contains('\\') || path.contains(':')
    {
        return Err(invalid(
            "`path` must be a slash-separated repository-relative path",
        ));
    }
    for component in path.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.eq_ignore_ascii_case(".git")
        {
            return Err(invalid(
                "`path` contains a forbidden repository path component",
            ));
        }
    }
    Ok(())
}

fn required_output(
    output: rah_sandbox::HostProcessOutput,
    label: &str,
) -> Result<Vec<u8>, ToolError> {
    if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
        Ok(output.stdout)
    } else {
        Err(git_error(format!(
            "bounded {label} observation did not complete successfully"
        )))
    }
}

fn parse_head_result(output: rah_sandbox::HostProcessOutput) -> Result<Option<String>, ToolError> {
    if output.timed_out || output.overflow.is_some() {
        return Err(git_error(
            "bounded HEAD observation did not complete successfully",
        ));
    }
    if output.exit_code == Some(1) && output.stdout.is_empty() {
        return Ok(None);
    }
    if output.exit_code != Some(0) {
        return Err(git_error(
            "bounded HEAD observation did not complete successfully",
        ));
    }
    let value = std::str::from_utf8(&output.stdout)
        .map_err(|_| git_error("Git HEAD observation was not UTF-8"))?
        .trim_end_matches(['\r', '\n']);
    if !is_object_id(value) {
        return Err(git_error("Git HEAD observation was malformed"));
    }
    Ok(Some(value.to_owned()))
}

#[derive(Clone, Debug)]
struct Entry {
    stage: u8,
    mode: String,
    object_id: String,
    tag: u8,
}

fn parse_index(bytes: &[u8], expected_path: &[u8]) -> Result<Vec<Entry>, ToolError> {
    let records = nul_records(bytes, "Git index observation")?;
    if records.len() > 3 {
        return Err(git_error("Git index observation has too many stages"));
    }
    let mut entries = Vec::new();
    for record in records {
        if record.len() < 3 || record[1] != b' ' {
            return Err(git_error("Git index observation was malformed"));
        }
        let tag = record[0];
        let rest = &record[2..];
        let Some(tab) = rest.iter().position(|byte| *byte == b'\t') else {
            return Err(git_error("Git index observation was malformed"));
        };
        if &rest[tab + 1..] != expected_path {
            return Err(git_error(
                "Git index observation selected an unexpected path",
            ));
        }
        let fields = rest[..tab].split(|byte| *byte == b' ').collect::<Vec<_>>();
        let [mode, object_id, stage] = fields.as_slice() else {
            return Err(git_error("Git index observation was malformed"));
        };
        let mode = ascii(mode, "Git index mode")?;
        let object_id = ascii(object_id, "Git index object id")?;
        let stage = ascii(stage, "Git index stage")?
            .parse::<u8>()
            .map_err(|_| git_error("Git index stage was malformed"))?;
        if !matches!(stage, 0..=3) || !is_mode(&mode) || !is_object_id(&object_id) {
            return Err(git_error("Git index observation was malformed"));
        }
        entries.push(Entry {
            stage,
            mode,
            object_id,
            tag,
        });
    }
    entries.sort_by_key(|entry| entry.stage);
    if entries
        .windows(2)
        .any(|pair| pair[0].stage == pair[1].stage)
        || (entries.len() > 1 && entries.iter().any(|entry| entry.stage == 0))
    {
        return Err(git_error("Git index observation has contradictory stages"));
    }
    Ok(entries)
}

fn parse_tree(bytes: &[u8], expected_path: &[u8]) -> Result<Option<Entry>, ToolError> {
    let records = nul_records(bytes, "Git HEAD tree observation")?;
    let Some(record) = records.first() else {
        return Ok(None);
    };
    if records.len() != 1 {
        return Err(git_error(
            "Git HEAD tree observation selected multiple paths",
        ));
    }
    let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
        return Err(git_error("Git HEAD tree observation was malformed"));
    };
    if &record[tab + 1..] != expected_path {
        return Err(git_error(
            "Git HEAD tree observation selected an unexpected path",
        ));
    }
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    let [mode, kind, object_id, size] = fields.as_slice() else {
        return Err(git_error("Git HEAD tree observation was malformed"));
    };
    let mode = ascii(mode, "Git HEAD mode")?;
    let kind = ascii(kind, "Git HEAD kind")?;
    let object_id = ascii(object_id, "Git HEAD object id")?;
    let _size = ascii(size, "Git HEAD size")?;
    if !is_mode(&mode)
        || !is_object_id(&object_id)
        || !matches!(kind.as_str(), "blob" | "commit" | "tree")
    {
        return Err(git_error("Git HEAD tree observation was malformed"));
    }
    Ok(Some(Entry {
        stage: 0,
        mode,
        object_id,
        tag: b'H',
    }))
}

#[derive(Default)]
struct PathStatus {
    staged: Option<char>,
    worktree: Option<char>,
    untracked: bool,
    conflicted: bool,
}

fn parse_status(bytes: &[u8], expected_path: &[u8]) -> Result<PathStatus, ToolError> {
    let records = nul_records(bytes, "Git status observation")?;
    let mut result = PathStatus::default();
    for record in records {
        if record.starts_with(b"? ") {
            if &record[2..] != expected_path {
                return Err(git_error("Git status selected an unexpected path"));
            }
            if result.untracked || result.staged.is_some() {
                return Err(git_error("Git status was contradictory"));
            }
            result.untracked = true;
            continue;
        }
        let fields = record.split(|byte| *byte == b' ').collect::<Vec<_>>();
        if fields.len() < 3 || fields[0].len() != 1 || fields[1].len() != 2 {
            return Err(git_error("Git status observation was malformed"));
        }
        let kind = fields[0][0];
        let xy = fields[1];
        if !matches!(kind, b'1' | b'u')
            || !matches!(xy[0], b'.' | b'M' | b'A' | b'D' | b'R' | b'C' | b'U')
            || !matches!(xy[1], b'.' | b'M' | b'A' | b'D' | b'R' | b'C' | b'U')
        {
            return Err(git_error("Git status observation was malformed"));
        }
        let path = match kind {
            b'1' if fields.len() == 9 => fields[8],
            b'u' if fields.len() == 11 => fields[10],
            _ => return Err(git_error("Git status observation was malformed")),
        };
        if path != expected_path || result.untracked || result.staged.is_some() {
            return Err(git_error("Git status was contradictory"));
        }
        result.staged = Some(xy[0] as char);
        result.worktree = Some(xy[1] as char);
        result.conflicted = kind == b'u' || xy.contains(&b'U');
    }
    Ok(result)
}

#[derive(Default)]
struct Worktree {
    present: bool,
    kind: Option<&'static str>,
    size: Option<u64>,
    digest: Option<Value>,
}

fn observe_worktree(root: &Path, logical_path: &str) -> Result<Worktree, ToolError> {
    let mut path = root.to_owned();
    let components = logical_path.split('/').collect::<Vec<_>>();
    for component in &components[..components.len().saturating_sub(1)] {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(git_error("worktree path ancestor is not a directory"));
                }
                reject_link_or_reparse(&path, "worktree path ancestor")?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Worktree::default());
            }
            Err(error) => return Err(git_error(error.to_string())),
        }
    }
    path.push(components.last().expect("validated nonempty path"));
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Worktree::default());
        }
        Err(error) => return Err(git_error(error.to_string())),
    };
    let kind = if metadata.file_type().is_symlink() {
        "symlink"
    } else if metadata.is_file() {
        "regular_file"
    } else if metadata.is_dir() {
        "directory"
    } else {
        "other"
    };
    let digest = if kind == "regular_file" && metadata.len() <= MAX_DIGEST_BYTES {
        Some(digest(&path, metadata.len())?)
    } else {
        None
    };
    Ok(Worktree {
        present: true,
        kind: Some(kind),
        size: Some(metadata.len()),
        digest,
    })
}

fn digest(path: &Path, expected_len: u64) -> Result<Value, ToolError> {
    let mut file = open_regular_no_follow(path)?;
    let metadata = file
        .metadata()
        .map_err(|error| git_error(error.to_string()))?;
    if !metadata.is_file() || metadata.len() != expected_len {
        return Err(git_error("worktree file changed during bounded digest"));
    }
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut chunk = [0_u8; 8192];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|error| git_error(error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes = bytes.saturating_add(count as u64);
        if bytes > MAX_DIGEST_BYTES {
            return Err(git_error("worktree file exceeded digest limit during read"));
        }
        hasher.update(&chunk[..count]);
    }
    if bytes != expected_len {
        return Err(git_error("worktree file changed during bounded digest"));
    }
    Ok(json!({"byte_length": bytes, "sha256": format!("{:x}", hasher.finalize())}))
}

fn open_regular_no_follow(path: &Path) -> Result<fs::File, ToolError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
        .open(path)
        .map_err(|error| git_error(error.to_string()))
}

fn normalize(
    path: &str,
    head: Option<String>,
    head_entry: Option<Entry>,
    entries: Vec<Entry>,
    status: PathStatus,
    worktree: Worktree,
) -> Result<Value, ToolError> {
    let conflicted = entries.iter().any(|entry| entry.stage != 0) || status.conflicted;
    let tracked = !entries.is_empty();
    let stage_zero = entries.iter().find(|entry| entry.stage == 0);
    let skip_worktree = stage_zero.is_some_and(|entry| entry.tag.eq_ignore_ascii_case(&b'S'));
    let assume_unchanged = stage_zero.is_some_and(|entry| entry.tag == b'h');
    let intent_to_add = stage_zero.is_some_and(|entry| is_zero_id(&entry.object_id));
    if intent_to_add && entries.len() != 1 {
        return Err(git_error("intent-to-add index state was contradictory"));
    }
    let sparse_state = if conflicted {
        "conflicted"
    } else if !tracked {
        "not_tracked"
    } else if skip_worktree && !worktree.present {
        "sparse_omitted"
    } else if skip_worktree {
        "skip_worktree_present"
    } else if stage_zero.is_some() {
        "normal"
    } else {
        "skip_worktree_unknown"
    };
    let staged_vs_head = if conflicted {
        Value::Null
    } else if let Some(index) = stage_zero {
        Value::Bool(match &head_entry {
            Some(tree) => index.mode != tree.mode || index.object_id != tree.object_id,
            None => true,
        })
    } else {
        Value::Bool(head_entry.is_some())
    };
    let worktree_modified_vs_index = if conflicted || (skip_worktree && !worktree.present) {
        Value::Null
    } else {
        Value::Bool(status.worktree.is_some_and(|state| state != '.'))
    };
    let index_entries = entries
        .iter()
        .map(|entry| {
            json!({
                "stage": entry.stage,
                "mode": entry.mode,
                "object_id": entry.object_id,
                "kind": mode_kind(&entry.mode),
                "executable": entry.mode == "100755"
            })
        })
        .collect::<Vec<_>>();
    let head = match (head, head_entry) {
        (Some(_), Some(entry)) => {
            json!({"present": true, "mode": entry.mode, "object_id": entry.object_id, "kind": mode_kind(&entry.mode), "executable": entry.mode == "100755"})
        }
        _ => {
            json!({"present": false, "mode": Value::Null, "object_id": Value::Null, "kind": Value::Null, "executable": false})
        }
    };
    let mut output = Map::new();
    output.insert("status".into(), json!("ok"));
    output.insert("consistency".into(), json!("best_effort"));
    output.insert("path".into(), tagged_path(path.as_bytes()));
    output.insert("head".into(), head);
    output.insert("index".into(), json!({
        "tracked": tracked, "entries": index_entries, "intent_to_add": intent_to_add,
        "assume_unchanged": assume_unchanged, "skip_worktree": skip_worktree, "conflicted": conflicted
    }));
    output.insert(
        "worktree".into(),
        json!({
            "present": worktree.present, "kind": worktree.kind, "size_bytes": worktree.size
        }),
    );
    output.insert("sparse_state".into(), json!(sparse_state));
    output.insert("staged_vs_head".into(), staged_vs_head);
    output.insert(
        "worktree_modified_vs_index".into(),
        worktree_modified_vs_index,
    );
    if let Some(digest) = worktree.digest {
        output.insert("content".into(), digest);
    }
    Ok(Value::Object(output))
}

fn bounded_output(value: Value) -> Result<ToolOutput, ToolError> {
    let output = ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: false,
    };
    if serde_json::to_vec(&output)
        .map_err(|error| git_error(error.to_string()))?
        .len()
        > MAX_RESULT_BYTES
    {
        return Err(git_error(
            "normalized repository file-info result exceeded its limit",
        ));
    }
    Ok(output)
}

fn nul_records<'a>(bytes: &'a [u8], label: &str) -> Result<Vec<&'a [u8]>, ToolError> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(git_error(format!("{label} was truncated")));
    }
    Ok(bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect())
}
fn ascii(bytes: &[u8], label: &str) -> Result<String, ToolError> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| git_error(format!("{label} was not ASCII")))
}
fn is_mode(value: &str) -> bool {
    matches!(value, "100644" | "100755" | "120000" | "160000")
}
fn mode_kind(value: &str) -> &'static str {
    match value {
        "100644" | "100755" => "regular_file",
        "120000" => "symlink",
        "160000" => "gitlink",
        _ => "other",
    }
}
fn is_object_id(value: &str) -> bool {
    (40..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn is_zero_id(value: &str) -> bool {
    value.bytes().all(|byte| byte == b'0')
}
fn tagged_path(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(value) => json!({"encoding":"utf8", "value": value}),
        Err(_) => json!({"encoding":"base64", "value": base64(bytes)}),
    }
}
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        result.push(TABLE[((value >> 18) & 63) as usize] as char);
        result.push(TABLE[((value >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}
fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidInput {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_aliases_and_namespaces() {
        for path in [
            "",
            "/absolute",
            "a/../b",
            "./a",
            ".git/x",
            "a/.GIT/x",
            "a\\b",
            "C:/x",
            "a:b",
        ] {
            assert!(validate_path(path).is_err(), "{path}");
        }
        assert!(validate_path(&"a".repeat(1025)).is_err());
        assert!(validate_path("unicode/檔案.txt").is_ok());
    }

    #[test]
    fn malformed_or_truncated_git_records_fail_closed() {
        assert!(parse_index(b" H 100644 deadbeef 0\tpath", b"path").is_err());
        assert!(parse_index(b" H 100644 xyz 0\tpath\0", b"path").is_err());
        assert!(parse_tree(b"100644 blob abc\tpath\0", b"path").is_err());
        assert!(parse_status(b"1 M. bad\0", b"path").is_err());
        assert!(parse_status(b"? other\0", b"path").is_err());
    }

    #[test]
    fn tagged_path_preserves_invalid_utf8_without_loss() {
        assert_eq!(tagged_path(b"ok"), json!({"encoding":"utf8","value":"ok"}));
        assert_eq!(
            tagged_path(b"bad\xff"),
            json!({"encoding":"base64","value":"YmFk/w=="})
        );
    }
}
