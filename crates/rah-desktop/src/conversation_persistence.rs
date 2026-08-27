use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::atomic::{AtomicU8, Ordering};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub(crate) const SNAPSHOT_FILE: &str = "conversation-transcript-v1.json";
const V2_FILE: &str = "conversation-transcript.json";
const MAX_BYTES: usize = 256 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_RECORDS: usize = 79;
const MAX_PAIRS: usize = 64;
const MAX_EPOCHS: usize = 16;

#[cfg(test)]
static TEST_FAULT: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
const FAIL_CREATE: u8 = 1;
#[cfg(test)]
const FAIL_REPLACE: u8 = 2;
#[cfg(test)]
const FAIL_REMOVE_V1: u8 = 3;
#[cfg(test)]
const FAIL_REMOVE_V2: u8 = 4;

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
struct V1 {
    version: u8,
    records: Vec<Record>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    CompletedPair { user: String, assistant: String },
    ContextSeparator { reason: SeparatorReason },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V2 {
    version: u8,
    epochs: Vec<Epoch>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Epoch {
    id: u64,
    parent_epoch_id: Option<u64>,
    boundary: Option<SeparatorReason>,
    history_trimmed_before: bool,
    pairs: Vec<Pair>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    user: String,
    assistant: String,
}
enum Backing {
    V1(Vec<Record>),
    V2(V2),
}
pub(crate) struct Persistence {
    directory: PathBuf,
    backing: Backing,
    warning: Option<Warning>,
    sequence: u64,
}

impl Persistence {
    pub(crate) fn start(directory: PathBuf) -> Self {
        let _ = fs::create_dir_all(&directory);
        cleanup_temps(&directory);
        let v2 = directory.join(V2_FILE);
        let v1 = directory.join(SNAPSHOT_FILE);
        let mut this = if v2.exists() {
            match load_v2(&v2) {
                Ok(v) => Self::v2(directory, v),
                Err(()) => {
                    quarantine(&v2);
                    Self::failed(directory)
                }
            }
        } else {
            match load_v1(&v1) {
                Ok(Some(v)) => Self::v1(directory, v),
                Ok(None) => Self::v1(directory, vec![]),
                Err(()) => {
                    quarantine(&v1);
                    Self::failed(directory)
                }
            }
        };
        if !this.empty() {
            this.restart();
        }
        this
    }
    fn v1(directory: PathBuf, records: Vec<Record>) -> Self {
        Self {
            directory,
            backing: Backing::V1(records),
            warning: None,
            sequence: 0,
        }
    }
    fn v2(directory: PathBuf, v2: V2) -> Self {
        Self {
            directory,
            backing: Backing::V2(v2),
            warning: None,
            sequence: 0,
        }
    }
    fn failed(directory: PathBuf) -> Self {
        Self {
            directory,
            backing: Backing::V2(V2 {
                version: 2,
                epochs: vec![],
            }),
            warning: Some(Warning::RestoreFailed),
            sequence: 0,
        }
    }
    fn empty(&self) -> bool {
        match &self.backing {
            Backing::V1(r) => r.is_empty(),
            Backing::V2(v) => v.epochs.iter().all(|e| e.pairs.is_empty()),
        }
    }
    fn restart(&mut self) {
        match &mut self.backing {
            Backing::V1(r) => r.push(Record::ContextSeparator {
                reason: SeparatorReason::ApplicationRestarted,
            }),
            Backing::V2(v) => v.epochs.push(Epoch {
                id: next_id(&v.epochs).unwrap_or(0),
                parent_epoch_id: None,
                boundary: Some(SeparatorReason::ApplicationRestarted),
                history_trimmed_before: false,
                pairs: vec![],
            }),
        };
        if self.save().is_err() {
            self.warning = Some(Warning::SaveFailed);
        }
    }
    pub(crate) fn presentation(&self) -> Presentation {
        let mut records = vec![];
        match &self.backing {
            Backing::V1(v) => presentation_v1(v, &mut records),
            Backing::V2(v) => {
                for e in &v.epochs {
                    if e.history_trimmed_before {
                        records.push(PresentationRecord::ContextSeparator {
                            reason: SeparatorReason::HistoryTrimmed,
                        });
                    }
                    if let Some(reason) = e.boundary {
                        records.push(PresentationRecord::ContextSeparator { reason });
                    }
                    for p in &e.pairs {
                        records.push(PresentationRecord::CompletedMessage {
                            role: PresentationRole::User,
                            text: p.user.clone(),
                        });
                        records.push(PresentationRecord::CompletedMessage {
                            role: PresentationRole::Assistant,
                            text: p.assistant.clone(),
                        });
                    }
                }
            }
        };
        Presentation {
            records,
            warning: self.warning,
        }
    }
    pub(crate) fn append_pair(&mut self, user: String, assistant: String) -> Result<(), Warning> {
        self.mutate(Some(Pair { user, assistant }), None)
    }
    pub(crate) fn append_separator(&mut self, reason: SeparatorReason) -> Result<(), Warning> {
        self.mutate(None, Some(reason))
    }
    fn mutate(
        &mut self,
        pair: Option<Pair>,
        separator: Option<SeparatorReason>,
    ) -> Result<(), Warning> {
        if let Backing::V1(records) = &mut self.backing {
            apply_v1(records, pair.as_ref(), separator);
            if !trim_v1(records) {
                return Err(Warning::SaveFailed);
            }
            let mut candidate = normalize_v1(records);
            let sequence = self.next();
            if !trim_v2(&mut candidate) || write_v2(&self.directory, &candidate, sequence).is_err()
            {
                return Err(Warning::SaveFailed);
            }
            self.backing = Backing::V2(candidate);
            return Ok(());
        }
        let Backing::V2(v) = &mut self.backing else {
            unreachable!()
        };
        apply_v2(v, pair, separator)?;
        if !trim_v2(v) || self.save().is_err() {
            Err(Warning::SaveFailed)
        } else {
            Ok(())
        }
    }
    pub(crate) fn clear(&mut self) -> io::Result<()> {
        let v1 = self.directory.join(SNAPSHOT_FILE);
        let v2 = self.directory.join(V2_FILE);
        if v2.exists() {
            remove(&v1)?;
            remove(&v2)?;
        } else {
            remove(&v1)?;
        }
        clean_private(&self.directory);
        self.backing = Backing::V1(vec![]);
        self.warning = None;
        Ok(())
    }
    fn next(&mut self) -> u64 {
        self.sequence = self.sequence.wrapping_add(1);
        self.sequence
    }
    fn save(&mut self) -> io::Result<()> {
        let n = self.next();
        match &self.backing {
            Backing::V1(v) => write_v1(&self.directory, v, n),
            Backing::V2(v) => write_v2(&self.directory, v, n),
        }
    }
}
fn apply_v1(records: &mut Vec<Record>, pair: Option<&Pair>, separator: Option<SeparatorReason>) {
    if let Some(p) = pair {
        records.push(Record::CompletedPair {
            user: p.user.clone(),
            assistant: p.assistant.clone(),
        });
    }
    if let Some(reason) = separator {
        records.push(Record::ContextSeparator { reason });
    }
}
fn apply_v2(
    v: &mut V2,
    pair: Option<Pair>,
    separator: Option<SeparatorReason>,
) -> Result<(), Warning> {
    if let Some(reason) = separator {
        v.epochs.push(Epoch {
            id: next_id(&v.epochs).map_err(|_| Warning::SaveFailed)?,
            parent_epoch_id: None,
            boundary: Some(reason),
            history_trimmed_before: false,
            pairs: vec![],
        });
    }
    if let Some(pair) = pair {
        if v.epochs.is_empty() {
            v.epochs.push(Epoch {
                id: 1,
                parent_epoch_id: None,
                boundary: None,
                history_trimmed_before: false,
                pairs: vec![],
            });
        }
        v.epochs.last_mut().unwrap().pairs.push(pair);
    }
    Ok(())
}
fn normalize_v1(records: &[Record]) -> V2 {
    let mut epochs = vec![Epoch {
        id: 1,
        parent_epoch_id: None,
        boundary: None,
        history_trimmed_before: false,
        pairs: vec![],
    }];
    for record in records {
        match record {
            Record::CompletedPair { user, assistant } => {
                epochs.last_mut().unwrap().pairs.push(Pair {
                    user: user.clone(),
                    assistant: assistant.clone(),
                })
            }
            Record::ContextSeparator { reason } => {
                let id = epochs.last().unwrap().id + 1;
                epochs.push(Epoch {
                    id,
                    parent_epoch_id: None,
                    boundary: Some(*reason),
                    history_trimmed_before: false,
                    pairs: vec![],
                });
            }
        }
    }
    V2 { version: 2, epochs }
}
fn presentation_v1(input: &[Record], out: &mut Vec<PresentationRecord>) {
    for r in input {
        match r {
            Record::CompletedPair { user, assistant } => {
                out.push(PresentationRecord::CompletedMessage {
                    role: PresentationRole::User,
                    text: user.clone(),
                });
                out.push(PresentationRecord::CompletedMessage {
                    role: PresentationRole::Assistant,
                    text: assistant.clone(),
                });
            }
            Record::ContextSeparator { reason } => {
                out.push(PresentationRecord::ContextSeparator { reason: *reason })
            }
        }
    }
}
fn next_id(epochs: &[Epoch]) -> Result<u64, ()> {
    epochs
        .last()
        .map(|e| e.id.checked_add(1).ok_or(()))
        .unwrap_or(Ok(1))
}
fn valid_message(s: &str) -> Result<(), ()> {
    if s.len() > MAX_MESSAGE_BYTES {
        Err(())
    } else {
        Ok(())
    }
}
fn validate_v1(records: &[Record]) -> Result<(), ()> {
    if records.len() > MAX_RECORDS {
        return Err(());
    }
    let mut pairs = 0;
    let mut epochs = 1;
    for r in records {
        match r {
            Record::CompletedPair { user, assistant } => {
                pairs += 1;
                valid_message(user)?;
                valid_message(assistant)?;
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
fn validate_v2(v: &V2) -> Result<(), ()> {
    if v.version != 2 || v.epochs.is_empty() || v.epochs.len() > MAX_EPOCHS {
        return Err(());
    }
    let mut pairs = 0;
    let mut previous = 0;
    for (i, e) in v.epochs.iter().enumerate() {
        if e.id == 0
            || e.id <= previous
            || (i == 0 && !e.history_trimmed_before && e.boundary.is_some())
            || (i == 0 && e.history_trimmed_before && e.boundary.is_none())
            || (i > 0 && e.boundary.is_none())
            || (i > 0 && e.history_trimmed_before)
        {
            return Err(());
        }
        previous = e.id;
        if let Some(parent) = e.parent_epoch_id
            && (e.boundary != Some(SeparatorReason::ApplicationRestarted)
                || parent >= e.id
                || !v.epochs[..i].iter().any(|x| x.id == parent))
        {
            return Err(());
        }
        for p in &e.pairs {
            pairs += 1;
            valid_message(&p.user)?;
            valid_message(&p.assistant)?;
        }
    }
    if pairs > MAX_PAIRS
        || pairs
            + v.epochs.len().saturating_sub(1)
            + usize::from(v.epochs.iter().any(|epoch| epoch.history_trimmed_before))
            > MAX_RECORDS
    {
        Err(())
    } else {
        Ok(())
    }
}
fn read(path: &Path) -> Result<Option<Vec<u8>>, ()> {
    if !path.exists() {
        return Ok(None);
    }
    if fs::metadata(path).map_err(|_| ())?.len() as usize > MAX_BYTES {
        return Err(());
    }
    let mut f = fs::File::open(path).map_err(|_| ())?;
    let mut b = vec![];
    Read::by_ref(&mut f)
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut b)
        .map_err(|_| ())?;
    if b.is_empty() || b.len() > MAX_BYTES {
        Err(())
    } else {
        Ok(Some(b))
    }
}
fn load_v1(path: &Path) -> Result<Option<Vec<Record>>, ()> {
    let Some(b) = read(path)? else {
        return Ok(None);
    };
    let v: V1 = serde_json::from_slice(&b).map_err(|_| ())?;
    if v.version != 1 {
        return Err(());
    }
    validate_v1(&v.records)?;
    Ok(Some(v.records))
}
fn load_v2(path: &Path) -> Result<V2, ()> {
    let Some(b) = read(path)? else {
        return Err(());
    };
    let v: V2 = serde_json::from_slice(&b).map_err(|_| ())?;
    validate_v2(&v)?;
    Ok(v)
}
fn trim_v1(r: &mut Vec<Record>) -> bool {
    while validate_v1(r).is_err() || bytes_v1(r) > MAX_BYTES {
        let Some(i) = r
            .iter()
            .position(|x| matches!(x, Record::ContextSeparator { .. }))
        else {
            return false;
        };
        r.drain(..=i);
        if !matches!(
            r.first(),
            Some(Record::ContextSeparator {
                reason: SeparatorReason::HistoryTrimmed
            })
        ) {
            r.insert(
                0,
                Record::ContextSeparator {
                    reason: SeparatorReason::HistoryTrimmed,
                },
            );
        }
    }
    true
}
fn trim_v2(v: &mut V2) -> bool {
    while validate_v2(v).is_err() || bytes_v2(v) > MAX_BYTES {
        if v.epochs.len() < 2 {
            return false;
        }
        v.epochs.remove(0);
        v.epochs[0].parent_epoch_id = None;
        v.epochs[0].history_trimmed_before = true;
    }
    true
}
fn bytes_v1(r: &[Record]) -> usize {
    serde_json::to_vec(&V1 {
        version: 1,
        records: r.to_vec(),
    })
    .map_or(usize::MAX, |x| x.len())
}
fn bytes_v2(v: &V2) -> usize {
    serde_json::to_vec(v).map_or(usize::MAX, |x| x.len())
}
fn write_v1(d: &Path, r: &[Record], n: u64) -> io::Result<()> {
    validate_v1(r).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid transcript"))?;
    atomic(
        d,
        SNAPSHOT_FILE,
        &serde_json::to_vec(&V1 {
            version: 1,
            records: r.to_vec(),
        })
        .map_err(io::Error::other)?,
        n,
    )
}
fn write_v2(d: &Path, v: &V2, n: u64) -> io::Result<()> {
    validate_v2(v).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid transcript"))?;
    atomic(
        d,
        V2_FILE,
        &serde_json::to_vec(v).map_err(io::Error::other)?,
        n,
    )
}
fn temp(file: &str, n: u64) -> String {
    format!("{file}.tmp-{}-{n}", std::process::id())
}
fn is_temp(name: &str, file: &str) -> bool {
    let Some(s) = name.strip_prefix(&format!("{file}.tmp-")) else {
        return false;
    };
    let mut p = s.split('-');
    matches!((p.next(),p.next(),p.next()),(Some(a),Some(b),None) if a.parse::<u32>().is_ok() && b.parse::<u64>().is_ok())
}
fn cleanup_temps(d: &Path) {
    if let Ok(entries) = fs::read_dir(d) {
        for e in entries.flatten() {
            let n = e.file_name();
            let n = n.to_string_lossy();
            if is_temp(&n, SNAPSHOT_FILE) || is_temp(&n, V2_FILE) {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}
fn remove(path: &Path) -> io::Result<()> {
    #[cfg(test)]
    if (path.file_name().and_then(|name| name.to_str()) == Some(SNAPSHOT_FILE)
        && TEST_FAULT.load(Ordering::SeqCst) == FAIL_REMOVE_V1)
        || (path.file_name().and_then(|name| name.to_str()) == Some(V2_FILE)
            && TEST_FAULT.load(Ordering::SeqCst) == FAIL_REMOVE_V2)
    {
        return Err(io::Error::other("injected removal failure"));
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
fn clean_private(d: &Path) {
    for f in [SNAPSHOT_FILE, V2_FILE] {
        let _ = fs::remove_file(d.join(format!("{f}.corrupt")));
    }
    cleanup_temps(d);
}
fn quarantine(path: &Path) {
    let _ = fs::rename(
        path,
        path.with_file_name(format!(
            "{}.corrupt",
            path.file_name().unwrap().to_string_lossy()
        )),
    );
}
fn atomic(d: &Path, name: &str, bytes: &[u8], n: u64) -> io::Result<()> {
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot too large",
        ));
    }
    let dst = d.join(name);
    let tmp = d.join(temp(name, n));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    #[cfg(test)]
    if (!dst.exists() && TEST_FAULT.load(Ordering::SeqCst) == FAIL_CREATE)
        || (dst.exists() && TEST_FAULT.load(Ordering::SeqCst) == FAIL_REPLACE)
    {
        let _ = fs::remove_file(&tmp);
        return Err(io::Error::other("injected atomic replacement failure"));
    }
    let result = if dst.exists() {
        replace(&dst, &tmp)
    } else {
        move_file(&tmp, &dst)
    };
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
#[cfg(target_os = "windows")]
fn wide(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}
#[cfg(target_os = "windows")]
fn replace(d: &Path, t: &Path) -> io::Result<()> {
    let d = wide(d);
    let t = wide(t);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            d.as_ptr(),
            t.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(target_os = "windows")]
fn move_file(t: &Path, d: &Path) -> io::Result<()> {
    let t = wide(t);
    let d = wide(d);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            t.as_ptr(),
            d.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn fault_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
    fn directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rah-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn pair(text: &str) -> Pair {
        Pair {
            user: text.into(),
            assistant: text.into(),
        }
    }
    fn e(id: u64, b: Option<SeparatorReason>, p: Option<u64>) -> Epoch {
        Epoch {
            id,
            parent_epoch_id: p,
            boundary: b,
            history_trimmed_before: false,
            pairs: vec![],
        }
    }
    #[test]
    fn v2_is_strict_and_validates_lineage() {
        let v = V2 {
            version: 2,
            epochs: vec![
                e(1, None, None),
                e(2, Some(SeparatorReason::ApplicationRestarted), Some(1)),
                e(3, Some(SeparatorReason::ApplicationRestarted), Some(2)),
            ],
        };
        assert!(validate_v2(&v).is_ok());
        for v in [
            V2 {
                version: 2,
                epochs: vec![e(0, None, None)],
            },
            V2 {
                version: 2,
                epochs: vec![
                    e(1, None, None),
                    e(1, Some(SeparatorReason::NewConversation), None),
                ],
            },
            V2 {
                version: 2,
                epochs: vec![
                    e(1, None, None),
                    e(2, Some(SeparatorReason::NewConversation), Some(1)),
                ],
            },
            V2 {
                version: 2,
                epochs: vec![
                    e(1, None, None),
                    e(2, Some(SeparatorReason::ApplicationRestarted), Some(3)),
                ],
            },
        ] {
            assert!(validate_v2(&v).is_err());
        }
        assert!(serde_json::from_slice::<V2>(br#"{"version":2,"epochs":[],"x":1}"#).is_err());
    }
    #[test]
    fn normalization_preserves_presentation() {
        let r = vec![
            Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            },
            Record::ContextSeparator {
                reason: SeparatorReason::RepositoryChanged,
            },
            Record::CompletedPair {
                user: "v".into(),
                assistant: "b".into(),
            },
        ];
        let v = normalize_v1(&r);
        assert_eq!(v.epochs.len(), 2);
        assert!(v.epochs.iter().all(|x| x.parent_epoch_id.is_none()));
        let mut one = vec![];
        presentation_v1(&r, &mut one);
        let two = Persistence::v2(PathBuf::new(), v).presentation().records;
        assert_eq!(
            serde_json::to_string(&one).unwrap(),
            serde_json::to_string(&two).unwrap()
        );
    }
    #[test]
    fn v2_precedence_and_v1_migration() {
        let d = std::env::temp_dir().join(format!("rah-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        write_v1(
            &d,
            &[Record::CompletedPair {
                user: "old".into(),
                assistant: "a".into(),
            }],
            1,
        )
        .unwrap();
        let mut p = Persistence::start(d.clone());
        assert!(!d.join(V2_FILE).exists());
        p.append_pair("new".into(), "a".into()).unwrap();
        assert!(d.join(V2_FILE).exists() && d.join(SNAPSHOT_FILE).exists());
        assert!(matches!(
            Persistence::start(d.clone()).backing,
            Backing::V2(_)
        ));
        let _ = fs::remove_dir_all(d);
    }
    #[test]
    fn invalid_v2_never_falls_back() {
        let d = std::env::temp_dir().join(format!("rah-v2-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        write_v1(
            &d,
            &[Record::CompletedPair {
                user: "old".into(),
                assistant: "a".into(),
            }],
            1,
        )
        .unwrap();
        fs::write(d.join(V2_FILE), b"bad").unwrap();
        let p = Persistence::start(d.clone());
        assert!(p.presentation().records.is_empty());
        assert_eq!(p.warning, Some(Warning::RestoreFailed));
        assert!(d.join(SNAPSHOT_FILE).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn v2_schema_rejects_every_closed_invalid_form() {
        for raw in [
            br#"not json"# as &[u8],
            br#"{"version":2,"epochs":[]}"#,
            br#"{"version":2,"epochs":[{"id":1,"parent_epoch_id":null,"boundary":null,"history_trimmed_before":false,"pairs":[],"extra":true}]}"#,
            br#"{"version":2,"epochs":[{"id":1,"parent_epoch_id":null,"boundary":null,"history_trimmed_before":false,"pairs":[{"user":"u","assistant":"a","extra":true}]}]}"#,
            br#"{"version":2,"epochs":[{"id":1,"parent_epoch_id":null,"boundary":"unknown","history_trimmed_before":false,"pairs":[]}]}"#,
            br#"{"version":2,"epochs":[{"id":1,"parent_epoch_id":null,"boundary":null,"pairs":[]}]}"#,
        ] { assert!(load_v2_from(raw).is_err()); }
        let mut huge = e(1, None, None);
        huge.pairs.push(Pair {
            user: "x".repeat(MAX_MESSAGE_BYTES + 1),
            assistant: "a".into(),
        });
        assert!(
            validate_v2(&V2 {
                version: 2,
                epochs: vec![huge]
            })
            .is_err()
        );
        let mut pairs = e(1, None, None);
        pairs.pairs = (0..MAX_PAIRS + 1)
            .map(|_| Pair {
                user: "u".into(),
                assistant: "a".into(),
            })
            .collect();
        assert!(
            validate_v2(&V2 {
                version: 2,
                epochs: vec![pairs]
            })
            .is_err()
        );
        let epochs = (1..=MAX_EPOCHS as u64 + 1)
            .map(|id| {
                e(
                    id,
                    if id == 1 {
                        None
                    } else {
                        Some(SeparatorReason::NewConversation)
                    },
                    None,
                )
            })
            .collect();
        assert!(validate_v2(&V2 { version: 2, epochs }).is_err());
    }

    #[test]
    fn normalization_maps_every_separator_and_keeps_v1_strict() {
        let reasons = [
            SeparatorReason::NewConversation,
            SeparatorReason::RepositoryChanged,
            SeparatorReason::ModelConfigurationChanged,
            SeparatorReason::RepositoryAndModelChanged,
            SeparatorReason::ApplicationRestarted,
            SeparatorReason::HistoryTrimmed,
        ];
        let mut records = vec![Record::CompletedPair {
            user: "first".into(),
            assistant: "a".into(),
        }];
        for reason in reasons {
            records.push(Record::ContextSeparator { reason });
            records.push(Record::CompletedPair {
                user: format!("{reason:?}"),
                assistant: "a".into(),
            });
        }
        let normalized = normalize_v1(&records);
        assert_eq!(normalized.epochs.len(), 7);
        for (epoch, reason) in normalized.epochs.iter().skip(1).zip(reasons) {
            assert_eq!(epoch.boundary, Some(reason));
            assert!(!epoch.history_trimmed_before);
        }
        assert!(load_v1_from(br#"{"version":1,"records":[],"extra":true}"#).is_err());
    }

    #[test]
    fn trimming_preserves_boundary_and_severs_lineage() {
        let mut a = e(1, None, None);
        a.pairs.push(Pair {
            user: "A".into(),
            assistant: "a".into(),
        });
        let mut b = e(2, Some(SeparatorReason::ApplicationRestarted), Some(1));
        b.pairs = (0..MAX_PAIRS)
            .map(|_| Pair {
                user: "B".into(),
                assistant: "a".into(),
            })
            .collect();
        let c = e(3, Some(SeparatorReason::ApplicationRestarted), Some(2));
        let mut v = V2 {
            version: 2,
            epochs: vec![a, b, c],
        };
        assert!(trim_v2(&mut v));
        assert_eq!(v.epochs[0].id, 2);
        assert!(v.epochs[0].history_trimmed_before);
        assert_eq!(
            v.epochs[0].boundary,
            Some(SeparatorReason::ApplicationRestarted)
        );
        assert_eq!(v.epochs[0].parent_epoch_id, None);
        assert_eq!(v.epochs[1].parent_epoch_id, Some(2));
        assert!(validate_v2(&v).is_ok());
        let p = Persistence::v2(PathBuf::new(), v).presentation();
        assert!(matches!(
            p.records[0],
            PresentationRecord::ContextSeparator {
                reason: SeparatorReason::HistoryTrimmed
            }
        ));
        assert!(matches!(
            p.records[1],
            PresentationRecord::ContextSeparator {
                reason: SeparatorReason::ApplicationRestarted
            }
        ));
    }

    #[test]
    fn clear_covers_both_exact_private_families() {
        let d = std::env::temp_dir().join(format!("rah-clear-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        write_v1(
            &d,
            &[Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            }],
            1,
        )
        .unwrap();
        write_v2(
            &d,
            &normalize_v1(&[Record::CompletedPair {
                user: "v".into(),
                assistant: "a".into(),
            }]),
            2,
        )
        .unwrap();
        for file in [
            format!("{SNAPSHOT_FILE}.corrupt"),
            format!("{V2_FILE}.corrupt"),
            temp(SNAPSHOT_FILE, 3),
            temp(V2_FILE, 4),
            "conversation-transcript.json.tmp-1-2-extra".into(),
            "unrelated.json".into(),
        ] {
            fs::write(d.join(file), b"x").unwrap();
        }
        let mut p = Persistence::start(d.clone());
        p.clear().unwrap();
        assert!(!d.join(SNAPSHOT_FILE).exists() && !d.join(V2_FILE).exists());
        assert!(
            d.join("conversation-transcript.json.tmp-1-2-extra")
                .exists()
                && d.join("unrelated.json").exists()
        );
        assert!(p.presentation().records.is_empty());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn startup_precedence_matrix_is_fail_closed() {
        let d = directory("startup");
        let fresh = Persistence::start(d.clone());
        assert!(fresh.presentation().records.is_empty() && fresh.warning.is_none());
        write_v1(
            &d,
            &[Record::CompletedPair {
                user: "v1".into(),
                assistant: "a".into(),
            }],
            1,
        )
        .unwrap();
        let v1 = Persistence::start(d.clone());
        assert!(matches!(v1.backing, Backing::V1(_)) && !d.join(V2_FILE).exists());
        write_v2(
            &d,
            &V2 {
                version: 2,
                epochs: vec![e(1, None, None)],
            },
            2,
        )
        .unwrap();
        assert!(matches!(
            Persistence::start(d.clone()).backing,
            Backing::V2(_)
        ));
        fs::remove_file(d.join(V2_FILE)).unwrap();
        fs::write(d.join(SNAPSHOT_FILE), b"bad").unwrap();
        let invalid_v1 = Persistence::start(d.clone());
        assert_eq!(invalid_v1.warning, Some(Warning::RestoreFailed));
        write_v2(
            &d,
            &V2 {
                version: 2,
                epochs: vec![e(1, None, None)],
            },
            3,
        )
        .unwrap();
        assert!(matches!(
            Persistence::start(d.clone()).backing,
            Backing::V2(_)
        ));
        fs::write(d.join(V2_FILE), b"bad").unwrap();
        let invalid_v2 = Persistence::start(d.clone());
        assert_eq!(invalid_v2.warning, Some(Warning::RestoreFailed));
        assert!(d.join(format!("{V2_FILE}.corrupt")).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn migration_failure_is_non_rollback_and_retries() {
        let _lock = fault_lock();
        let d = directory("migration-failure");
        write_v1(
            &d,
            &[Record::CompletedPair {
                user: "old".into(),
                assistant: "a".into(),
            }],
            1,
        )
        .unwrap();
        let mut p = Persistence::start(d.clone());
        TEST_FAULT.store(FAIL_CREATE, Ordering::SeqCst);
        assert_eq!(
            p.append_pair("current".into(), "a".into()),
            Err(Warning::SaveFailed)
        );
        assert!(load_v1(&d.join(SNAPSHOT_FILE)).unwrap().is_some());
        assert!(!d.join(V2_FILE).exists());
        assert!(matches!(p.backing, Backing::V1(_)));
        TEST_FAULT.store(0, Ordering::SeqCst);
        p.append_pair("retry".into(), "a".into()).unwrap();
        assert!(matches!(p.backing, Backing::V2(_)));
        let recovered = load_v2(&d.join(V2_FILE)).unwrap();
        assert_eq!(recovered.epochs[1].pairs.len(), 2);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn v2_replace_failure_keeps_disk_snapshot_and_later_catches_up() {
        let _lock = fault_lock();
        let d = directory("replace-failure");
        let mut p = Persistence::v2(
            d.clone(),
            V2 {
                version: 2,
                epochs: vec![e(1, None, None)],
            },
        );
        p.append_pair("one".into(), "a".into()).unwrap();
        TEST_FAULT.store(FAIL_REPLACE, Ordering::SeqCst);
        assert_eq!(
            p.append_pair("two".into(), "a".into()),
            Err(Warning::SaveFailed)
        );
        assert_eq!(load_v2(&d.join(V2_FILE)).unwrap().epochs[0].pairs.len(), 1);
        TEST_FAULT.store(0, Ordering::SeqCst);
        p.append_pair("three".into(), "a".into()).unwrap();
        assert_eq!(load_v2(&d.join(V2_FILE)).unwrap().epochs[0].pairs.len(), 3);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn every_genuine_v1_boundary_mutation_migrates_but_startup_does_not() {
        for reason in [
            SeparatorReason::NewConversation,
            SeparatorReason::RepositoryChanged,
            SeparatorReason::ModelConfigurationChanged,
            SeparatorReason::RepositoryAndModelChanged,
        ] {
            let d = directory("boundary-migration");
            write_v1(
                &d,
                &[Record::CompletedPair {
                    user: "u".into(),
                    assistant: "a".into(),
                }],
                1,
            )
            .unwrap();
            let mut p = Persistence::start(d.clone());
            assert!(!d.join(V2_FILE).exists());
            p.append_separator(reason).unwrap();
            assert!(d.join(V2_FILE).exists());
            let _ = fs::remove_dir_all(d);
        }
    }

    #[test]
    fn deep_and_repeated_lineage_trimming_preserves_graphs() {
        let mut v = V2 {
            version: 2,
            epochs: vec![
                e(1, None, None),
                e(2, Some(SeparatorReason::ApplicationRestarted), Some(1)),
                e(3, Some(SeparatorReason::ApplicationRestarted), Some(2)),
                e(4, Some(SeparatorReason::ApplicationRestarted), Some(3)),
            ],
        };
        v.epochs[0].pairs.push(pair("a"));
        v.epochs[1].pairs = (0..MAX_PAIRS).map(|_| pair("b")).collect();
        assert!(trim_v2(&mut v));
        assert_eq!(
            (
                v.epochs[0].id,
                v.epochs[0].parent_epoch_id,
                v.epochs[1].parent_epoch_id,
                v.epochs[2].parent_epoch_id
            ),
            (2, None, Some(2), Some(3))
        );
        assert!(v.epochs[0].history_trimmed_before && validate_v2(&v).is_ok());
        let before = serde_json::to_vec(&v).unwrap();
        assert!(trim_v2(&mut v));
        assert_eq!(serde_json::to_vec(&v).unwrap(), before);
        let _ = v.epochs.remove(0);
        v.epochs[0].parent_epoch_id = None;
        v.epochs[0].history_trimmed_before = true;
        assert_eq!(v.epochs[0].id, 3);
        assert_eq!(v.epochs[1].parent_epoch_id, Some(3));
        assert!(validate_v2(&v).is_ok());
    }

    #[test]
    fn byte_and_display_limit_trimming_removes_whole_epochs_only() {
        let mut old = e(1, None, None);
        old.pairs.push(pair(&"o".repeat(8_200)));
        let mut retained = e(2, Some(SeparatorReason::ApplicationRestarted), Some(1));
        retained.pairs = (0..15).map(|_| pair(&"x".repeat(8_200))).collect();
        let mut v = V2 {
            version: 2,
            epochs: vec![old, retained],
        };
        assert!(bytes_v2(&v) > MAX_BYTES && trim_v2(&mut v));
        assert_eq!(v.epochs.len(), 1);
        assert!(v.epochs[0].history_trimmed_before && v.epochs[0].parent_epoch_id.is_none());
        assert!(bytes_v2(&v) <= MAX_BYTES && validate_v2(&v).is_ok());
        let before = serde_json::to_vec(&v).unwrap();
        v.epochs[0].pairs.push(pair(&"z".repeat(8_200)));
        assert!(!trim_v2(&mut v));
        assert_ne!(serde_json::to_vec(&v).unwrap(), before);

        let mut epochs = (1..=17)
            .map(|id| {
                e(
                    id,
                    if id == 1 {
                        None
                    } else {
                        Some(SeparatorReason::NewConversation)
                    },
                    None,
                )
            })
            .collect::<Vec<_>>();
        for epoch in epochs.iter_mut().skip(1) {
            epoch.pairs.push(pair("x"));
        }
        epochs[1].pairs.extend((0..49).map(|_| pair("x")));
        let mut display = V2 { version: 2, epochs };
        assert!(trim_v2(&mut display));
        assert!(validate_v2(&display).is_ok());
        assert!(display.epochs.len() <= 15);
        assert_eq!(
            display
                .epochs
                .iter()
                .filter(|e| e.history_trimmed_before)
                .count(),
            1
        );
    }

    #[test]
    fn clear_failure_matrix_preserves_memory_and_orders_primary_deletes() {
        let _lock = fault_lock();
        for fault in [FAIL_REMOVE_V1, FAIL_REMOVE_V2] {
            let d = directory("clear-failure");
            write_v1(
                &d,
                &[Record::CompletedPair {
                    user: "v1".into(),
                    assistant: "a".into(),
                }],
                1,
            )
            .unwrap();
            write_v2(
                &d,
                &V2 {
                    version: 2,
                    epochs: vec![Epoch {
                        pairs: vec![pair("v2")],
                        ..e(1, None, None)
                    }],
                },
                2,
            )
            .unwrap();
            let mut p = Persistence::start(d.clone());
            TEST_FAULT.store(fault, Ordering::SeqCst);
            assert!(p.clear().is_err());
            assert!(!p.presentation().records.is_empty());
            if fault == FAIL_REMOVE_V1 {
                assert!(d.join(V2_FILE).exists());
            }
            if fault == FAIL_REMOVE_V2 {
                assert!(!d.join(SNAPSHOT_FILE).exists() && d.join(V2_FILE).exists());
            }
            TEST_FAULT.store(0, Ordering::SeqCst);
            let _ = fs::remove_dir_all(d);
        }
    }

    #[test]
    fn presentation_remains_closed_and_absent_clear_is_idempotent() {
        let d = directory("closed-presentation");
        let mut p = Persistence::v1(
            d.clone(),
            vec![Record::CompletedPair {
                user: "u".into(),
                assistant: "a".into(),
            }],
        );
        p.warning = Some(Warning::SaveFailed);
        let json = serde_json::to_string(&p.presentation()).unwrap();
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
        let mut fresh = Persistence::start(d.clone());
        fresh.clear().unwrap();
        assert!(fresh.presentation().records.is_empty());
        assert!(fresh.presentation().warning.is_none());
        let _ = fs::remove_dir_all(d);
    }

    fn load_v1_from(bytes: &[u8]) -> Result<(), ()> {
        let v: V1 = serde_json::from_slice(bytes).map_err(|_| ())?;
        if v.version != 1 {
            return Err(());
        }
        validate_v1(&v.records)
    }

    fn load_v2_from(bytes: &[u8]) -> Result<(), ()> {
        let v: V2 = serde_json::from_slice(bytes).map_err(|_| ())?;
        validate_v2(&v)
    }
}
