//! Closed, byte-safe `repo.status` observer.
//!
//! This module intentionally parses only the one porcelain-v2 command shape
//! owned by `RepositoryObserver`; it is not a Git command or status API.

use std::{path::Path, time::Instant};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    Tool, ToolContext, ToolError,
    git_support::git_error,
    repository_file_info::tagged_path,
    repository_observer::{ObserverCommand, RepositoryObserver, STATUS_OUTPUT_LIMIT},
};

/// Stable name for the fixed, read-only whole-repository status observer.
pub const REPOSITORY_STATUS_TOOL_NAME: &str = "repo.status";

const MAX_ENTRIES: usize = 10_000;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_RECORD_BYTES: usize = 16 * 1024;

/// Reports a bounded, normalized, best-effort status for one host-selected repository.
///
/// The model supplies only `{}`. The host owns the executable, repository,
/// command, environment, timeout, capture bounds, and repository lease.
pub struct RepositoryStatusTool {
    observer: RepositoryObserver,
}

impl RepositoryStatusTool {
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
impl Tool for RepositoryStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_STATUS_TOOL_NAME),
            description:
                "Reports normalized read-only Git status for one host-authorized repository."
                    .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {},
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
        validate_empty_request(&input)?;
        let _lease = self.observer.acquire_lease().await;
        self.observer.revalidate()?;
        let output = self
            .observer
            .run(ObserverCommand::Status, None, Instant::now())
            .await?;
        self.observer.revalidate()?;
        let bytes = successful_status_output(output)?;
        let entries = parse_status(&bytes)?;
        bounded_output(entries)
    }
}

fn validate_empty_request(input: &ToolInput) -> Result<(), ToolError> {
    match input.0.as_object() {
        Some(object) if object.is_empty() => Ok(()),
        _ => Err(ToolError::InvalidInput {
            message: "input must be an empty object".to_owned(),
        }),
    }
}

fn successful_status_output(output: rah_sandbox::HostProcessOutput) -> Result<Vec<u8>, ToolError> {
    if output.exit_code == Some(0)
        && !output.timed_out
        && output.overflow.is_none()
        && output.stdout.len() <= STATUS_OUTPUT_LIMIT
    {
        Ok(output.stdout)
    } else {
        Err(git_error(
            "bounded repository status observation did not complete successfully",
        ))
    }
}

#[derive(Debug)]
struct StatusEntry {
    path: Vec<u8>,
    previous_path: Option<Vec<u8>>,
    record_kind: u8,
    tracked: bool,
    index_state: &'static str,
    worktree_state: &'static str,
    conflict_state: &'static str,
    submodule_state: &'static str,
    head_mode: Option<String>,
    index_mode: Option<String>,
    worktree_mode: Option<String>,
    stages: Vec<(u8, String)>,
}

fn parse_status(bytes: &[u8]) -> Result<Vec<StatusEntry>, ToolError> {
    if bytes.len() > STATUS_OUTPUT_LIMIT {
        return Err(status_error("status output exceeded its processing limit"));
    }
    let records = nul_records(bytes)?;
    let mut entries = Vec::new();
    let mut index = 0;
    while index < records.len() {
        let record = records[index];
        if record.len() > MAX_RECORD_BYTES {
            return Err(status_error("status record exceeded its limit"));
        }
        let entry = match record.first().copied() {
            Some(b'1') => parse_ordinary(record)?,
            Some(b'2') => {
                let previous_path = *records
                    .get(index + 1)
                    .ok_or_else(|| status_error("rename status record was truncated"))?;
                if previous_path.len() > MAX_PATH_BYTES {
                    return Err(status_error("status path exceeded its limit"));
                }
                index += 1;
                parse_rename_or_copy(record, previous_path)?
            }
            Some(b'u') => parse_unmerged(record)?,
            Some(b'?') => parse_untracked(record)?,
            Some(b'!') => return Err(status_error("unexpected ignored status entry")),
            Some(b'#') => return Err(status_error("unexpected status header")),
            _ => return Err(status_error("status record tag was malformed")),
        };
        if entry.path.len() > MAX_PATH_BYTES || entry.path.is_empty() {
            return Err(status_error(
                "status path was malformed or exceeded its limit",
            ));
        }
        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            return Err(status_error("status entry count exceeded its limit"));
        }
        index += 1;
    }
    entries.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.record_kind.cmp(&right.record_kind))
    });
    Ok(entries)
}

fn nul_records(bytes: &[u8]) -> Result<Vec<&[u8]>, ToolError> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(status_error("status output had a truncated NUL record"));
    }
    Ok(bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect())
}

fn parse_ordinary(record: &[u8]) -> Result<StatusEntry, ToolError> {
    let fields = fields(record, 9)?;
    let [
        tag,
        xy,
        submodule,
        head_mode,
        index_mode,
        worktree_mode,
        head_id,
        index_id,
        path,
    ] = fields.as_slice()
    else {
        return Err(status_error("ordinary status record was malformed"));
    };
    if *tag != b"1"
        || !valid_xy(xy)
        || !valid_submodule(submodule)
        || !valid_mode(head_mode)
        || !valid_mode(index_mode)
        || !valid_mode(worktree_mode)
        || !valid_object_id(head_id)
        || !valid_object_id(index_id)
    {
        return Err(status_error("ordinary status record was malformed"));
    }
    Ok(StatusEntry {
        path: path.to_vec(),
        previous_path: None,
        record_kind: b'1',
        tracked: true,
        index_state: state(xy[0])?,
        worktree_state: state(xy[1])?,
        conflict_state: "none",
        submodule_state: submodule_state(submodule)?,
        head_mode: Some(ascii(head_mode)?),
        index_mode: Some(ascii(index_mode)?),
        worktree_mode: Some(ascii(worktree_mode)?),
        stages: Vec::new(),
    })
}

fn parse_rename_or_copy(record: &[u8], previous_path: &[u8]) -> Result<StatusEntry, ToolError> {
    let fields = fields(record, 10)?;
    let [
        tag,
        xy,
        submodule,
        head_mode,
        index_mode,
        worktree_mode,
        head_id,
        index_id,
        score,
        path,
    ] = fields.as_slice()
    else {
        return Err(status_error("rename status record was malformed"));
    };
    if *tag != b"2"
        || !valid_xy(xy)
        || !valid_submodule(submodule)
        || !valid_mode(head_mode)
        || !valid_mode(index_mode)
        || !valid_mode(worktree_mode)
        || !valid_object_id(head_id)
        || !valid_object_id(index_id)
        || !valid_score(score)
        || previous_path.is_empty()
    {
        return Err(status_error("rename status record was malformed"));
    }
    Ok(StatusEntry {
        path: path.to_vec(),
        previous_path: Some(previous_path.to_vec()),
        record_kind: b'2',
        tracked: true,
        index_state: state(xy[0])?,
        worktree_state: state(xy[1])?,
        conflict_state: "none",
        submodule_state: submodule_state(submodule)?,
        head_mode: Some(ascii(head_mode)?),
        index_mode: Some(ascii(index_mode)?),
        worktree_mode: Some(ascii(worktree_mode)?),
        stages: Vec::new(),
    })
}

fn parse_unmerged(record: &[u8]) -> Result<StatusEntry, ToolError> {
    let fields = fields(record, 11)?;
    let [
        tag,
        xy,
        submodule,
        mode1,
        mode2,
        mode3,
        worktree_mode,
        id1,
        id2,
        id3,
        path,
    ] = fields.as_slice()
    else {
        return Err(status_error("unmerged status record was malformed"));
    };
    if *tag != b"u"
        || !valid_xy(xy)
        || !valid_submodule(submodule)
        || ![mode1, mode2, mode3, worktree_mode]
            .into_iter()
            .all(|mode| valid_mode(mode))
        || ![id1, id2, id3].into_iter().all(|id| valid_object_id(id))
    {
        return Err(status_error("unmerged status record was malformed"));
    }
    Ok(StatusEntry {
        path: path.to_vec(),
        previous_path: None,
        record_kind: b'u',
        tracked: true,
        index_state: "unmerged",
        worktree_state: "unmerged",
        conflict_state: conflict_state(xy)?,
        submodule_state: submodule_state(submodule)?,
        head_mode: None,
        index_mode: None,
        worktree_mode: Some(ascii(worktree_mode)?),
        stages: vec![(1, ascii(mode1)?), (2, ascii(mode2)?), (3, ascii(mode3)?)],
    })
}

fn parse_untracked(record: &[u8]) -> Result<StatusEntry, ToolError> {
    let path = record
        .strip_prefix(b"? ")
        .ok_or_else(|| status_error("untracked status record was malformed"))?;
    Ok(StatusEntry {
        path: path.to_vec(),
        previous_path: None,
        record_kind: b'?',
        tracked: false,
        index_state: "untracked",
        worktree_state: "untracked",
        conflict_state: "none",
        submodule_state: "none",
        head_mode: None,
        index_mode: None,
        worktree_mode: None,
        stages: Vec::new(),
    })
}

fn fields(record: &[u8], count: usize) -> Result<Vec<&[u8]>, ToolError> {
    let result = record
        .splitn(count, |byte| *byte == b' ')
        .collect::<Vec<_>>();
    if result.len() != count || result.iter().any(|field| field.is_empty()) {
        return Err(status_error("status record was missing fields"));
    }
    Ok(result)
}

fn valid_xy(value: &[u8]) -> bool {
    value.len() == 2
        && value
            .iter()
            .all(|state| matches!(state, b'.' | b'M' | b'A' | b'D' | b'R' | b'C' | b'T' | b'U'))
}

fn valid_submodule(value: &[u8]) -> bool {
    value == b"N..."
        || (value.len() == 4
            && value[0] == b'S'
            && value[1..]
                .iter()
                .all(|byte| matches!(byte, b'.' | b'M' | b'U')))
}

fn valid_mode(value: &[u8]) -> bool {
    value.len() == 6 && value.iter().all(|byte| matches!(byte, b'0'..=b'7'))
}

fn valid_object_id(value: &[u8]) -> bool {
    matches!(value.len(), 40 | 64) && value.iter().all(u8::is_ascii_hexdigit)
}

fn valid_score(value: &[u8]) -> bool {
    value.len() == 4 && matches!(value[0], b'R' | b'C') && value[1..].iter().all(u8::is_ascii_digit)
}

fn state(value: u8) -> Result<&'static str, ToolError> {
    match value {
        b'.' => Ok("unmodified"),
        b'A' => Ok("added"),
        b'M' => Ok("modified"),
        b'D' => Ok("deleted"),
        b'R' => Ok("renamed"),
        b'C' => Ok("copied"),
        b'T' => Ok("type_changed"),
        b'U' => Ok("unmerged"),
        _ => Err(status_error("status state was malformed")),
    }
}

fn conflict_state(xy: &[u8]) -> Result<&'static str, ToolError> {
    match xy {
        b"AA" => Ok("both_added"),
        b"DD" => Ok("both_deleted"),
        b"AU" => Ok("added_by_us"),
        b"UD" => Ok("deleted_by_them"),
        b"UA" => Ok("added_by_them"),
        b"DU" => Ok("deleted_by_us"),
        b"UU" => Ok("both_modified"),
        _ => Err(status_error("unmerged status state was malformed")),
    }
}

fn submodule_state(value: &[u8]) -> Result<&'static str, ToolError> {
    if value == b"N..." {
        Ok("none")
    } else if valid_submodule(value) {
        Ok("modified")
    } else {
        Err(status_error("submodule status was malformed"))
    }
}

fn ascii(value: &[u8]) -> Result<String, ToolError> {
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| status_error("status metadata was not ASCII"))
}

fn bounded_output(entries: Vec<StatusEntry>) -> Result<ToolOutput, ToolError> {
    let entries = entries
        .into_iter()
        .map(|entry| {
            json!({
                "path": tagged_path(&entry.path),
                "previous_path": entry.previous_path.as_deref().map(tagged_path),
                "tracked": entry.tracked,
                "index_state": entry.index_state,
                "worktree_state": entry.worktree_state,
                "conflict_state": entry.conflict_state,
                "submodule_state": entry.submodule_state,
                "head_mode": entry.head_mode,
                "index_mode": entry.index_mode,
                "worktree_mode": entry.worktree_mode,
                "stages": entry.stages.into_iter().map(|(stage, mode)| json!({"stage": stage, "mode": mode})).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let output = ToolOutput {
        content: vec![ToolContent::Json(json!({
            "status": "ok",
            "consistency": "best_effort",
            "entries": entries,
            "sparse_index_flags": "not_enumerated"
        }))],
        is_error: false,
    };
    if serde_json::to_vec(&output)
        .map_err(|error| status_error(format!("status result could not be serialized: {error}")))?
        .len()
        > STATUS_OUTPUT_LIMIT
    {
        return Err(status_error("normalized status result exceeded its limit"));
    }
    Ok(output)
}

fn status_error(message: impl Into<String>) -> ToolError {
    git_error(format!("repository status {0}", message.into()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{MAX_ENTRIES, MAX_RECORD_BYTES, parse_status};

    const ID: &str = "0123456789012345678901234567890123456789";

    #[test]
    fn parses_all_porcelain_record_shapes_without_lossy_paths() {
        let ordinary = format!("1 M. N... 100644 100755 100755 {ID} {ID} tab\tname\0");
        let renamed =
            format!("2 R. N... 100644 100644 100644 {ID} {ID} R100 new\nname\0old name\0");
        let conflict = format!("u UU N... 100644 100644 100644 100644 {ID} {ID} {ID} conflict\0");
        let mut bytes = ordinary.into_bytes();
        bytes.extend_from_slice(renamed.as_bytes());
        bytes.extend_from_slice(conflict.as_bytes());
        bytes.extend_from_slice(b"? bad\xff\0");
        let entries = parse_status(&bytes).unwrap();
        assert_eq!(entries.len(), 4);
        let output = super::bounded_output(entries).unwrap();
        let rah_protocol::ToolContent::Json(value) = &output.content[0] else {
            panic!("expected JSON")
        };
        assert_eq!(
            value["entries"][0]["path"],
            json!({"encoding":"base64","value":"YmFk/w=="})
        );
        assert_eq!(value["entries"][1]["conflict_state"], "both_modified");
        assert_eq!(value["entries"][2]["previous_path"]["value"], "old name");
        assert_eq!(value["entries"][2]["path"]["value"], "new\nname");
    }

    #[test]
    fn malformed_or_unexpected_records_fail_closed() {
        let valid = format!("1 M. N... 100644 100644 100644 {ID} {ID} path\0");
        for bytes in [
            b"1 M. N... 100644".as_slice(),
            b"x anything\0".as_slice(),
            b"1 XX N... 100644 100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 p\0".as_slice(),
            b"1 M. N... 10064x 100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 p\0".as_slice(),
            b"2 R. N... 100644 100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 R100 new\0".as_slice(),
            b"# branch.oid deadbeef\0".as_slice(),
            b"! ignored\0".as_slice(),
        ] {
            assert!(parse_status(bytes).is_err());
        }
        let oversized = format!("{valid}{}\0", "x".repeat(MAX_RECORD_BYTES));
        assert!(parse_status(oversized.as_bytes()).is_err());
    }

    #[test]
    fn parser_enforces_count_bound_and_unmerged_vocabulary() {
        let mut bytes = Vec::new();
        for number in 0..=MAX_ENTRIES {
            bytes.extend_from_slice(format!("? item-{number}\0").as_bytes());
        }
        assert!(parse_status(&bytes).is_err());
        for state in [b"AA", b"DD", b"AU", b"UD", b"UA", b"DU", b"UU"] {
            let record = format!(
                "u {} N... 100644 100644 100644 100644 {ID} {ID} {ID} path\0",
                std::str::from_utf8(state).unwrap()
            );
            assert!(parse_status(record.as_bytes()).is_ok());
        }
    }
}
