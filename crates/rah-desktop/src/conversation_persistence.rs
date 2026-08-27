use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) const SNAPSHOT_FILE: &str = "conversation-transcript-v1.json";
const MAX_BYTES: usize = 256 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_RECORDS: usize = 79;
const MAX_PAIRS: usize = 64;
const MAX_EPOCHS: usize = 16;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Warning {
    RestoreFailed,
    SaveFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Presentation {
    pub records: Vec<PresentationRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<Warning>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PresentationRecord {
    CompletedMessage {
        role: PresentationRole,
        text: String,
    },
    ContextSeparator {
        reason: SeparatorReason,
    },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PresentationRole {
    User,
    Assistant,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SeparatorReason {
    NewConversation,
    RepositoryChanged,
    ModelConfigurationChanged,
    RepositoryAndModelChanged,
    ApplicationRestarted,
    HistoryTrimmed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    version: u8,
    records: Vec<Record>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    CompletedPair { user: String, assistant: String },
    ContextSeparator { reason: SeparatorReason },
}

pub(crate) struct Persistence {
    directory: PathBuf,
    records: Vec<Record>,
    warning: Option<Warning>,
    sequence: u64,
}

impl Persistence {
    pub(crate) fn start(directory: PathBuf) -> Self {
        let _ = fs::create_dir_all(&directory);
        cleanup_temps(&directory);
        let path = directory.join(SNAPSHOT_FILE);
        let mut this = match load(&path) {
            Ok(Some(records)) => Self {
                directory,
                records,
                warning: None,
                sequence: 0,
            },
            Ok(None) => Self {
                directory,
                records: Vec::new(),
                warning: None,
                sequence: 0,
            },
            Err(()) => {
                quarantine(&path);
                Self {
                    directory,
                    records: Vec::new(),
                    warning: Some(Warning::RestoreFailed),
                    sequence: 0,
                }
            }
        };
        if !this.records.is_empty() {
            this.records.push(Record::ContextSeparator {
                reason: SeparatorReason::ApplicationRestarted,
            });
            if this.save().is_err() {
                this.warning = Some(Warning::SaveFailed);
            }
        }
        this
    }
    pub(crate) fn presentation(&self) -> Presentation {
        let mut records = Vec::new();
        for record in &self.records {
            match record {
                Record::CompletedPair { user, assistant } => {
                    records.push(PresentationRecord::CompletedMessage {
                        role: PresentationRole::User,
                        text: user.clone(),
                    });
                    records.push(PresentationRecord::CompletedMessage {
                        role: PresentationRole::Assistant,
                        text: assistant.clone(),
                    });
                }
                Record::ContextSeparator { reason } => {
                    records.push(PresentationRecord::ContextSeparator { reason: *reason })
                }
            }
        }
        Presentation {
            records,
            warning: self.warning,
        }
    }
    pub(crate) fn append_pair(&mut self, user: String, assistant: String) -> Result<(), Warning> {
        self.records.push(Record::CompletedPair { user, assistant });
        self.commit()
    }
    pub(crate) fn append_separator(&mut self, reason: SeparatorReason) -> Result<(), Warning> {
        self.records.push(Record::ContextSeparator { reason });
        self.commit()
    }
    fn commit(&mut self) -> Result<(), Warning> {
        if !trim(&mut self.records) {
            return Err(Warning::SaveFailed);
        }
        self.save().map_err(|_| Warning::SaveFailed)
    }
    fn save(&mut self) -> io::Result<()> {
        validate(&self.records)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid transcript"))?;
        let bytes = serde_json::to_vec(&Snapshot {
            version: 1,
            records: self.records.clone(),
        })
        .map_err(io::Error::other)?;
        if bytes.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot too large",
            ));
        }
        self.sequence = self.sequence.wrapping_add(1);
        atomic_write(&self.directory, &bytes, self.sequence)
    }
}

fn load(path: &Path) -> Result<Option<Vec<Record>>, ()> {
    if !path.exists() {
        return Ok(None);
    }
    if fs::metadata(path).map_err(|_| ())?.len() as usize > MAX_BYTES {
        return Err(());
    }
    let mut file = fs::File::open(path).map_err(|_| ())?;
    let mut bytes = Vec::with_capacity(MAX_BYTES.min(8192));
    Read::by_ref(&mut file)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(());
    }
    let snapshot: Snapshot = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if snapshot.version != 1 {
        return Err(());
    }
    validate(&snapshot.records)?;
    Ok(Some(snapshot.records))
}

fn validate(records: &[Record]) -> Result<(), ()> {
    if records.len() > MAX_RECORDS {
        return Err(());
    }
    let mut pairs = 0;
    let mut epochs = 1;
    for record in records {
        match record {
            Record::CompletedPair { user, assistant } => {
                pairs += 1;
                if user.len() > MAX_MESSAGE_BYTES || assistant.len() > MAX_MESSAGE_BYTES {
                    return Err(());
                }
            }
            Record::ContextSeparator { .. } => epochs += 1,
        }
    }
    if pairs > MAX_PAIRS || epochs > MAX_EPOCHS {
        Err(())
    } else {
        Ok(())
    }
}

// Removes a complete oldest epoch plus the transition that started the next one.
fn trim(records: &mut Vec<Record>) -> bool {
    if validate(records).is_ok() && serialized_len(records) <= MAX_BYTES {
        return true;
    }
    loop {
        let Some(separator) = records
            .iter()
            .position(|r| matches!(r, Record::ContextSeparator { .. }))
        else {
            return false;
        };
        records.drain(..=separator);
        if !matches!(
            records.first(),
            Some(Record::ContextSeparator {
                reason: SeparatorReason::HistoryTrimmed
            })
        ) {
            records.insert(
                0,
                Record::ContextSeparator {
                    reason: SeparatorReason::HistoryTrimmed,
                },
            );
        }
        if validate(records).is_ok() && serialized_len(records) <= MAX_BYTES {
            return true;
        }
        if records.len() == 1 {
            return false;
        }
    }
}
fn serialized_len(records: &[Record]) -> usize {
    serde_json::to_vec(&Snapshot {
        version: 1,
        records: records.to_vec(),
    })
    .map_or(usize::MAX, |v| v.len())
}
fn temp_name(sequence: u64) -> String {
    format!("{SNAPSHOT_FILE}.tmp-{}-{sequence}", std::process::id())
}
fn is_private_temp_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix(&format!("{SNAPSHOT_FILE}.tmp-")) else {
        return false;
    };
    let mut parts = suffix.split('-');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(process), Some(sequence), None)
            if process.parse::<u32>().is_ok() && sequence.parse::<u64>().is_ok()
    )
}

fn cleanup_temps(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if is_private_temp_name(&entry.file_name().to_string_lossy()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
fn quarantine(path: &Path) {
    let target = path.with_file_name(format!("{SNAPSHOT_FILE}.corrupt"));
    let _ = fs::rename(path, target);
}

fn atomic_write(dir: &Path, bytes: &[u8], sequence: u64) -> io::Result<()> {
    let destination = dir.join(SNAPSHOT_FILE);
    let temp = dir.join(temp_name(sequence));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    let result = if destination.exists() {
        replace_file(&destination, &temp)
    } else {
        move_file(&temp, &destination)
    };
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(target_os = "windows")]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
#[cfg(target_os = "windows")]
fn replace_file(destination: &Path, temp: &Path) -> io::Result<()> {
    let d = wide(destination);
    let t = wide(temp);
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            d.as_ptr(),
            t.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(target_os = "windows")]
fn move_file(temp: &Path, destination: &Path) -> io::Result<()> {
    let t = wide(temp);
    let d = wide(destination);
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            t.as_ptr(),
            d.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_is_closed_and_round_trips() {
        let records = vec![Record::CompletedPair {
            user: "u".into(),
            assistant: "a".into(),
        }];
        let bytes = serde_json::to_vec(&Snapshot {
            version: 1,
            records,
        })
        .unwrap();
        let dir = std::env::temp_dir().join(format!("rah-transcript-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(SNAPSHOT_FILE), bytes).unwrap();
        assert!(load(&dir.join(SNAPSHOT_FILE)).unwrap().is_some());
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn rejects_invalid_versions_and_fields() {
        for raw in [
            br#"{"version":0,"records":[]}"# as &[u8],
            br#"{"version":2,"records":[]}"#,
            br#"{"version":1,"records":[],"extra":1}"#,
            br#"{"version":1,"records":[{"kind":"other"}]}"#,
        ] {
            let path = std::env::temp_dir().join(format!("rah-invalid-{}", raw.len()));
            fs::write(&path, raw).unwrap();
            assert!(load(&path).is_err());
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn rejects_empty_truncated_oversized_messages_and_limits() {
        let path = std::env::temp_dir().join(format!("rah-empty-{}", std::process::id()));
        for raw in [b"" as &[u8], b"{\"version\":1", b"not json"] {
            fs::write(&path, raw).unwrap();
            assert!(load(&path).is_err());
        }
        let large = Record::CompletedPair {
            user: "x".repeat(MAX_MESSAGE_BYTES + 1),
            assistant: "a".into(),
        };
        assert!(validate(&[large]).is_err());
        let pairs = (0..MAX_PAIRS + 1)
            .map(|_| Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            })
            .collect::<Vec<_>>();
        assert!(validate(&pairs).is_err());
        let separators = (0..MAX_EPOCHS)
            .map(|_| Record::ContextSeparator {
                reason: SeparatorReason::NewConversation,
            })
            .collect::<Vec<_>>();
        assert!(validate(&separators).is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn trimming_removes_whole_old_epoch_and_marks_boundary() {
        let mut records = vec![
            Record::CompletedPair {
                user: "old".into(),
                assistant: "old".into(),
            },
            Record::ContextSeparator {
                reason: SeparatorReason::NewConversation,
            },
        ];
        records.extend((0..MAX_PAIRS).map(|_| Record::CompletedPair {
            user: "u".into(),
            assistant: "a".into(),
        }));
        assert!(trim(&mut records));
        assert!(matches!(
            records.first(),
            Some(Record::ContextSeparator {
                reason: SeparatorReason::HistoryTrimmed
            })
        ));
        assert!(
            !records.iter().any(
                |record| matches!(record, Record::CompletedPair { user, .. } if user == "old")
            )
        );
    }

    #[test]
    fn current_epoch_that_exceeds_bounds_is_not_truncated() {
        let mut records = (0..MAX_PAIRS + 1)
            .map(|_| Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            })
            .collect::<Vec<_>>();
        let before = records.clone();
        assert!(!trim(&mut records));
        assert_eq!(records.len(), before.len());
    }

    #[test]
    fn presentation_is_closed_and_temp_cleanup_is_exact() {
        let presentation = Persistence {
            directory: PathBuf::new(),
            records: vec![Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            }],
            warning: Some(Warning::SaveFailed),
            sequence: 0,
        }
        .presentation();
        let json = serde_json::to_string(&presentation).unwrap();
        for forbidden in [
            "path",
            "generation",
            "session",
            "thread",
            "request",
            "endpoint",
            "credential",
            "token",
            "executable",
            "permission",
            "registry",
        ] {
            assert!(!json.contains(forbidden));
        }
        assert!(is_private_temp_name(
            "conversation-transcript-v1.json.tmp-12-3"
        ));
        assert!(!is_private_temp_name(
            "conversation-transcript-v1.json.tmp-12-3-extra"
        ));
        assert!(!is_private_temp_name("unrelated.tmp-12-3"));
    }
}
