//! Shared, closed, byte-safe repository diff observer foundation.
//!
//! This is deliberately a pair of fixed host-selected observations rather than
//! a generic Git diff API. Raw and numstat records establish file identity;
//! patch sections are opaque presentation bytes associated only after the two
//! machine-readable streams agree exactly.

use std::{collections::BTreeMap, path::Path, time::Instant};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Value, json};

use crate::{
    Tool, ToolContext, ToolError,
    git_support::git_error,
    repository_file_info::tagged_path,
    repository_observer::{DIFF_OUTPUT_LIMIT, ObserverCommand, RepositoryObserver},
};

/// Stable name for the fixed, read-only worktree-versus-index diff observer.
pub const REPOSITORY_DIFF_TOOL_NAME: &str = "repo.diff";

/// Private fixed comparison selected by the host-facing tool construction.
///
/// This is crate-visible only so no model input or public API can choose a Git
/// baseline, revision, or argument list.
#[derive(Clone, Copy)]
pub(crate) enum DiffBaseline {
    WorktreeVsIndex,
    IndexVsHead,
}

const MAX_FILES: usize = 256;
const MAX_PATH_BYTES: usize = 4 * 1024;
const MAX_PATCH_BYTES: usize = 256 * 1024;
const MAX_RESULT_BYTES: usize = 1024 * 1024;

/// Reports a bounded normalized worktree-versus-index diff for one host-selected repository.
///
/// The model supplies only `{}`. The host owns the executable, repository,
/// fixed commands, child environment, capture limits, timeout, and one
/// exclusive RAH repository lease spanning all three observations.
pub struct RepositoryDiffTool {
    observer: RepositoryObserver,
}

impl RepositoryDiffTool {
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
impl Tool for RepositoryDiffTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_DIFF_TOOL_NAME),
            description: "Reports a bounded read-only worktree-versus-index Git diff for one host-authorized repository.".to_owned(),
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
        execute_fixed_diff(&self.observer, &input, DiffBaseline::WorktreeVsIndex).await
    }
}

pub(crate) async fn execute_fixed_diff(
    observer: &RepositoryObserver,
    input: &ToolInput,
    baseline: DiffBaseline,
) -> Result<ToolOutput, ToolError> {
    validate_empty_request(input)?;
    let _lease = observer.acquire_lease().await;
    execute_fixed_diff_while_leased(observer, baseline).await
}

/// Runs the fixed observation while the caller owns this repository's RAH
/// lease. This remains crate-private so host code cannot turn it into a
/// generic Git observation API.
pub(crate) async fn execute_fixed_diff_while_leased(
    observer: &RepositoryObserver,
    baseline: DiffBaseline,
) -> Result<ToolOutput, ToolError> {
    observer.revalidate()?;
    let started = Instant::now();
    let before_head = match baseline {
        DiffBaseline::WorktreeVsIndex => None,
        DiffBaseline::IndexVsHead => Some(observe_head(observer, started).await?),
    };
    let raw = successful_output(
        observer
            .run(ObserverCommand::DiffRaw(baseline), None, started)
            .await?,
        "raw",
    )?;
    let numstat = successful_output(
        observer
            .run(ObserverCommand::DiffNumstat(baseline), None, started)
            .await?,
        "numstat",
    )?;
    let patch = successful_output(
        observer
            .run(ObserverCommand::DiffPatch(baseline), None, started)
            .await?,
        "patch",
    )?;
    if let Some(before_head) = before_head.as_ref() {
        let after_head = observe_head(observer, started).await?;
        ensure_same_head(before_head, after_head)?;
    }
    observer.revalidate()?;
    bounded_output(
        correlate(&raw, &numstat, &patch)?,
        baseline,
        before_head.flatten(),
    )
}

/// Returns whether a successful normalized diff contains any binary entry.
///
/// This keeps consumers of the fixed diff on its structural binary
/// classification rather than attempting to rediscover binary content from
/// paths or patch presentation.
pub(crate) fn contains_binary_content(output: &ToolOutput) -> Result<bool, ToolError> {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return Err(diff_error("normalized result had an unexpected shape"));
    };
    if output.is_error {
        return Err(diff_error("normalized result was an error"));
    }
    let files = value
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| diff_error("normalized result had no files"))?;
    files.iter().try_fold(false, |has_binary, file| {
        file.get("binary")
            .and_then(Value::as_bool)
            .map(|binary| has_binary || binary)
            .ok_or_else(|| diff_error("normalized file had no binary classification"))
    })
}

fn ensure_same_head(before: &Option<String>, after: Option<String>) -> Result<(), ToolError> {
    if before != &after {
        return Err(diff_error("HEAD changed during staged observation"));
    }
    Ok(())
}

fn validate_empty_request(input: &ToolInput) -> Result<(), ToolError> {
    match input.0.as_object() {
        Some(object) if object.is_empty() => Ok(()),
        _ => Err(ToolError::InvalidInput {
            message: "input must be an empty object".to_owned(),
        }),
    }
}

/// `rev-parse --verify -q HEAD` exits one with no output for an unborn HEAD.
/// Any other result is a fixed-command observation failure, never model-visible
/// Git diagnostics.
async fn observe_head(
    observer: &RepositoryObserver,
    started: Instant,
) -> Result<Option<String>, ToolError> {
    let output = observer.run(ObserverCommand::Head, None, started).await?;
    if output.exit_code == Some(1)
        && !output.timed_out
        && output.overflow.is_none()
        && output.stdout.is_empty()
    {
        return Ok(None);
    }
    let stdout = successful_output(output, "HEAD")?;
    let value = std::str::from_utf8(&stdout)
        .ok()
        .and_then(|value| value.strip_suffix('\n'))
        .filter(|value| valid_object_id(value.as_bytes()))
        .ok_or_else(|| diff_error("HEAD identity was malformed"))?;
    Ok(Some(value.to_owned()))
}

fn successful_output(
    output: rah_sandbox::HostProcessOutput,
    label: &str,
) -> Result<Vec<u8>, ToolError> {
    if output.exit_code == Some(0)
        && !output.timed_out
        && output.overflow.is_none()
        && output.stdout.len() <= DIFF_OUTPUT_LIMIT
    {
        Ok(output.stdout)
    } else {
        Err(diff_error(format!(
            "bounded repository {label} observation did not complete successfully"
        )))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct DiffKey {
    old_path: Option<Vec<u8>>,
    new_path: Option<Vec<u8>>,
    change_kind: ChangeKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum ChangeKind {
    Added,
    Deleted,
    Modified,
    TypeChanged,
    GitlinkChanged,
}

impl ChangeKind {
    fn name(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Modified => "modified",
            Self::TypeChanged => "type_changed",
            Self::GitlinkChanged => "gitlink_changed",
        }
    }
}

#[derive(Debug)]
struct RawEntry {
    key: DiffKey,
    old_mode: String,
    new_mode: String,
}

#[derive(Debug)]
struct NumstatEntry {
    path: Vec<u8>,
    binary: bool,
    added_lines: Option<u64>,
    deleted_lines: Option<u64>,
}

#[derive(Debug)]
struct DiffEntry {
    raw: RawEntry,
    numstat: NumstatEntry,
    patch: Option<Vec<u8>>,
}

fn correlate(raw: &[u8], numstat: &[u8], patch: &[u8]) -> Result<Vec<DiffEntry>, ToolError> {
    let raw = parse_raw(raw)?;
    let numstat = parse_numstat(numstat)?;
    if raw.len() != numstat.len() {
        return Err(diff_error("raw and numstat file counts disagreed"));
    }
    let mut numstat_by_path = BTreeMap::new();
    for entry in numstat {
        if numstat_by_path.insert(entry.path.clone(), entry).is_some() {
            return Err(diff_error("numstat contained duplicate paths"));
        }
    }
    let sections = split_patch_sections(patch)?;
    if raw.is_empty() {
        if !sections.is_empty() {
            return Err(diff_error(
                "patch contained a file absent from raw metadata",
            ));
        }
        return Ok(Vec::new());
    }
    if sections.len() != raw.len() {
        return Err(diff_error(
            "raw metadata and patch section counts disagreed",
        ));
    }

    let mut entries = Vec::with_capacity(raw.len());
    for (raw, section) in raw.into_iter().zip(sections) {
        let path = raw
            .key
            .new_path
            .as_ref()
            .or(raw.key.old_path.as_ref())
            .ok_or_else(|| diff_error("raw record had no correlation path"))?;
        let numstat = numstat_by_path
            .remove(path)
            .ok_or_else(|| diff_error("raw metadata had no matching numstat path"))?;
        if !numstat.binary && looks_like_binary_patch(&section) {
            return Err(diff_error(
                "numstat text classification contradicted patch binary marker",
            ));
        }
        let patch = if numstat.binary { None } else { Some(section) };
        entries.push(DiffEntry {
            raw,
            numstat,
            patch,
        });
    }
    if !numstat_by_path.is_empty() {
        return Err(diff_error(
            "numstat contained a path absent from raw metadata",
        ));
    }
    Ok(entries)
}

fn looks_like_binary_patch(section: &[u8]) -> bool {
    section
        .windows(b"\nBinary files ".len())
        .any(|window| window == b"\nBinary files ")
        || section
            .windows(b"\nGIT binary patch".len())
            .any(|window| window == b"\nGIT binary patch")
}

fn parse_raw(bytes: &[u8]) -> Result<Vec<RawEntry>, ToolError> {
    let records = nul_records(bytes, "raw output")?;
    if records.len() % 2 != 0 {
        return Err(diff_error("raw output had a missing path"));
    }
    let mut entries = Vec::with_capacity(records.len() / 2);
    let mut records = records.into_iter();
    while let Some(header) = records.next() {
        let path = records
            .next()
            .ok_or_else(|| diff_error("raw output had a missing path"))?;
        if path.is_empty() || path.len() > MAX_PATH_BYTES {
            return Err(diff_error("raw path was malformed or exceeded its limit"));
        }
        let fields = header.split(|byte| *byte == b' ').collect::<Vec<_>>();
        let [old_mode, new_mode, old_id, new_id, status] = fields.as_slice() else {
            return Err(diff_error("raw record was malformed"));
        };
        let old_mode = old_mode
            .strip_prefix(b":")
            .ok_or_else(|| diff_error("raw modes were malformed"))?;
        if !valid_mode(old_mode)
            || !valid_mode(new_mode)
            || !valid_object_id(old_id)
            || !valid_object_id(new_id)
        {
            return Err(diff_error(
                "raw record contained malformed mode or object ID",
            ));
        }
        let status = *status
            .first()
            .filter(|_| status.len() == 1)
            .ok_or_else(|| diff_error("raw status was malformed"))?;
        let old_mode = ascii(old_mode, "raw mode")?;
        let new_mode = ascii(new_mode, "raw mode")?;
        let key = match status {
            b'A' => DiffKey {
                old_path: None,
                new_path: Some(path.to_vec()),
                change_kind: ChangeKind::Added,
            },
            b'D' => DiffKey {
                old_path: Some(path.to_vec()),
                new_path: None,
                change_kind: ChangeKind::Deleted,
            },
            b'M' | b'T' => DiffKey {
                old_path: Some(path.to_vec()),
                new_path: Some(path.to_vec()),
                change_kind: if status == b'M' && (old_mode == "160000" || new_mode == "160000") {
                    ChangeKind::GitlinkChanged
                } else if status == b'T' {
                    ChangeKind::TypeChanged
                } else {
                    ChangeKind::Modified
                },
            },
            b'U' => {
                return Err(diff_error(
                    "unmerged worktree/index diff is not safely correlatable",
                ));
            }
            _ => return Err(diff_error("raw status was unsupported")),
        };
        if entries.iter().any(|entry: &RawEntry| entry.key == key) {
            return Err(diff_error(
                "raw output contained duplicate correlation records",
            ));
        }
        entries.push(RawEntry {
            key,
            old_mode,
            new_mode,
        });
        if entries.len() > MAX_FILES {
            return Err(diff_error("raw file count exceeded its limit"));
        }
    }
    Ok(entries)
}

fn parse_numstat(bytes: &[u8]) -> Result<Vec<NumstatEntry>, ToolError> {
    let records = nul_records(bytes, "numstat output")?;
    let mut entries = Vec::with_capacity(records.len());
    for record in records {
        let fields = record.splitn(3, |byte| *byte == b'\t').collect::<Vec<_>>();
        let [added, deleted, path] = fields.as_slice() else {
            return Err(diff_error("numstat record was malformed"));
        };
        if path.is_empty() || path.len() > MAX_PATH_BYTES {
            return Err(diff_error(
                "numstat path was malformed or exceeded its limit",
            ));
        }
        let binary = *added == b"-" && *deleted == b"-";
        if (*added == b"-") != (*deleted == b"-") {
            return Err(diff_error("numstat binary markers disagreed"));
        }
        let (added_lines, deleted_lines) = if binary {
            (None, None)
        } else {
            (Some(parse_count(added)?), Some(parse_count(deleted)?))
        };
        entries.push(NumstatEntry {
            path: path.to_vec(),
            binary,
            added_lines,
            deleted_lines,
        });
        if entries.len() > MAX_FILES {
            return Err(diff_error("numstat file count exceeded its limit"));
        }
    }
    Ok(entries)
}

fn split_patch_sections(bytes: &[u8]) -> Result<Vec<Vec<u8>>, ToolError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.starts_with(b"diff --git ") {
        return Err(diff_error(
            "patch did not begin with a top-level diff section",
        ));
    }
    let mut starts = vec![0];
    for position in 1..bytes.len() {
        if bytes[position - 1] == b'\n' && bytes[position..].starts_with(b"diff --git ") {
            starts.push(position);
        }
    }
    let mut sections = Vec::with_capacity(starts.len());
    for (index, start) in starts.iter().copied().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(bytes.len());
        let section = bytes[start..end].to_vec();
        if section.len() > MAX_PATCH_BYTES {
            return Err(diff_error("per-file patch section exceeded its limit"));
        }
        sections.push(section);
    }
    Ok(sections)
}

fn nul_records<'a>(bytes: &'a [u8], label: &str) -> Result<Vec<&'a [u8]>, ToolError> {
    if bytes.len() > DIFF_OUTPUT_LIMIT {
        return Err(diff_error(format!("{label} exceeded its processing limit")));
    }
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(diff_error(format!("{label} had a truncated NUL record")));
    }
    Ok(bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect())
}

fn valid_mode(value: &[u8]) -> bool {
    value.len() == 6 && value.iter().all(|byte| matches!(byte, b'0'..=b'7'))
}

fn valid_object_id(value: &[u8]) -> bool {
    matches!(value.len(), 40 | 64) && value.iter().all(u8::is_ascii_hexdigit)
}

fn parse_count(value: &[u8]) -> Result<u64, ToolError> {
    if value.is_empty() || !value.iter().all(u8::is_ascii_digit) {
        return Err(diff_error("numstat count was malformed"));
    }
    std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| diff_error("numstat count was malformed"))
}

fn ascii(value: &[u8], label: &str) -> Result<String, ToolError> {
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| diff_error(format!("{label} was not ASCII")))
}

fn bounded_output(
    entries: Vec<DiffEntry>,
    baseline: DiffBaseline,
    head: Option<String>,
) -> Result<ToolOutput, ToolError> {
    let files = entries
        .into_iter()
        .map(|entry| {
            json!({
                "old_path": entry.raw.key.old_path.as_deref().map(tagged_path),
                "new_path": entry.raw.key.new_path.as_deref().map(tagged_path),
                "change_kind": entry.raw.key.change_kind.name(),
                "old_mode": entry.raw.old_mode,
                "new_mode": entry.raw.new_mode,
                "binary": entry.numstat.binary,
                "added_lines": entry.numstat.added_lines,
                "deleted_lines": entry.numstat.deleted_lines,
                "patch": entry.patch.as_deref().map(tagged_path),
            })
        })
        .collect::<Vec<Value>>();
    let output = ToolOutput {
        content: vec![ToolContent::Json(json!({
            "status": "ok",
            "consistency": "best_effort",
            "comparison": match baseline {
                DiffBaseline::WorktreeVsIndex => "worktree_to_index",
                DiffBaseline::IndexVsHead => "index_to_head",
            },
            "base": match baseline {
                DiffBaseline::WorktreeVsIndex => "index",
                DiffBaseline::IndexVsHead if head.is_some() => "head",
                DiffBaseline::IndexVsHead => "empty_tree",
            },
            "files": files,
        }))],
        is_error: false,
    };
    if serde_json::to_vec(&output)
        .map_err(|error| diff_error(format!("diff result could not be serialized: {error}")))?
        .len()
        > MAX_RESULT_BYTES
    {
        return Err(diff_error("normalized diff result exceeded its limit"));
    }
    Ok(output)
}

fn diff_error(message: impl Into<String>) -> ToolError {
    git_error(format!("repository diff {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789012345678901234567890123456789";

    fn raw(status: char, path: &str) -> Vec<u8> {
        format!(":100644 100644 {ID} {ID} {status}\0{path}\0").into_bytes()
    }

    #[test]
    fn correlates_machine_readable_records_and_keeps_patch_opaque() {
        let raw = raw('M', "a.txt");
        let numstat = b"2\t1\ta.txt\0";
        let patch = b"diff --git a.txt a.txt\nindex 1..2 100644\n--- a.txt\n+++ a.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let output = bounded_output(
            correlate(&raw, numstat, patch).unwrap(),
            DiffBaseline::WorktreeVsIndex,
            None,
        )
        .unwrap();
        let ToolContent::Json(value) = &output.content[0] else {
            panic!("expected JSON")
        };
        assert_eq!(value["files"][0]["change_kind"], "modified");
        assert_eq!(value["files"][0]["patch"]["encoding"], "utf8");
        assert_eq!(value["files"][0]["added_lines"], 2);
    }

    #[test]
    fn binary_is_structural_and_has_no_patch_payload() {
        let raw = raw('M', "binary.bin");
        let entries = correlate(
            &raw,
            b"-\t-\tbinary.bin\0",
            b"diff --git binary.bin binary.bin\nBinary files binary.bin and binary.bin differ\n",
        )
        .unwrap();
        let output = bounded_output(entries, DiffBaseline::WorktreeVsIndex, None).unwrap();
        let ToolContent::Json(value) = &output.content[0] else {
            panic!("expected JSON")
        };
        assert_eq!(value["files"][0]["binary"], true);
        assert!(value["files"][0]["patch"].is_null());
    }

    #[test]
    fn malformed_raw_numstat_and_patch_observations_fail_closed() {
        let valid_raw = raw('M', "a.txt");
        for bytes in [
            b":100644 100644 1 2 M\0a\0".as_slice(),
            b":10064x 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 M\0a\0".as_slice(),
            b":100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 X\0a\0".as_slice(),
            b":100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 M\0".as_slice(),
            b":100644 100644 0123456789012345678901234567890123456789 0123456789012345678901234567890123456789 T\0a\0extra\0".as_slice(),
        ] { assert!(parse_raw(bytes).is_err()); }
        for bytes in [
            b"1\t2\0".as_slice(),
            b"-\t2\ta\0".as_slice(),
            b"x\t2\ta\0".as_slice(),
        ] {
            assert!(parse_numstat(bytes).is_err());
        }
        assert!(correlate(&valid_raw, b"1\t1\tb.txt\0", b"diff --git a a\n").is_err());
        assert!(
            correlate(
                &valid_raw,
                b"1\t1\ta.txt\0",
                b"diff --git a a\n\ndiff --git b b\n"
            )
            .is_err()
        );
        assert!(
            correlate(
                &valid_raw,
                b"1\t1\ta.txt\0",
                b"diff --git a a\nBinary files a and a differ\n"
            )
            .is_err()
        );
        assert!(split_patch_sections(b"not a patch").is_err());
    }

    #[test]
    fn staged_head_race_checks_reject_born_and_changed_head_states() {
        assert!(ensure_same_head(&None, Some(ID.to_owned())).is_err());
        assert!(ensure_same_head(&Some(ID.to_owned()), None).is_err());
        assert!(ensure_same_head(&Some(ID.to_owned()), Some("f".repeat(40))).is_err());
        assert!(ensure_same_head(&Some(ID.to_owned()), Some(ID.to_owned())).is_ok());
    }

    #[test]
    fn bounds_and_unmerged_records_fail_closed() {
        let mut bytes = Vec::new();
        for number in 0..=MAX_FILES {
            bytes.extend(raw('M', &format!("file-{number}")));
        }
        assert!(parse_raw(&bytes).is_err());
        let unmerged = format!(":000000 000000 {ID} {ID} U\0conflict\0");
        assert!(parse_raw(unmerged.as_bytes()).is_err());
        let patch = format!("diff --git a a\n{}", "x".repeat(MAX_PATCH_BYTES));
        assert!(split_patch_sections(patch.as_bytes()).is_err());
    }
}
