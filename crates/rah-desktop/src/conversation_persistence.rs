//! Host-owned, private SQLite transcript storage. SQL is never exposed outside this module.
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
const V3_FILE: &str = "conversation-transcript-v3.json";
const DB: &str = "conversation-transcript.sqlite3";
const STAGING: &str = "conversation-transcript.sqlite3.importing";
const MAX_NAMESPACES: usize = 64;
// Bounds one legacy V3 JSON migration input, never the SQLite database.
const MAX_BYTES: usize = 256 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_RECORDS: usize = 79;
const MAX_PAIRS: usize = 64;
const MAX_EPOCHS: usize = 16;
const SCHEMA_VERSION: i64 = 1;
const SCHEMA_SQL: &str = "CREATE TABLE schema_metadata(singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL CHECK(schema_version=1),migration_complete INTEGER NOT NULL CHECK(migration_complete IN(0,1)),source_format TEXT NOT NULL CHECK(source_format IN('empty','v3')),imported_namespace_count INTEGER NOT NULL CHECK(imported_namespace_count>=0),imported_epoch_count INTEGER NOT NULL CHECK(imported_epoch_count>=0),imported_pair_count INTEGER NOT NULL CHECK(imported_pair_count>=0));CREATE TABLE namespaces(namespace_key TEXT PRIMARY KEY NOT NULL CHECK(namespace_key='neutral-v1' OR(length(namespace_key)=76 AND substr(namespace_key,1,12)='repo-sha256:' AND substr(namespace_key,13) NOT GLOB '*[^0-9a-f]*')));CREATE TABLE epochs(namespace_key TEXT NOT NULL,epoch_id INTEGER NOT NULL CHECK(epoch_id>0),parent_epoch_id INTEGER,boundary TEXT,history_trimmed_before INTEGER NOT NULL CHECK(history_trimmed_before IN(0,1)),PRIMARY KEY(namespace_key,epoch_id),FOREIGN KEY(namespace_key) REFERENCES namespaces(namespace_key) ON DELETE CASCADE,FOREIGN KEY(namespace_key,parent_epoch_id) REFERENCES epochs(namespace_key,epoch_id),CHECK(parent_epoch_id IS NULL OR parent_epoch_id<epoch_id),CHECK(boundary IS NULL OR boundary IN('new_conversation','repository_changed','model_configuration_changed','repository_and_model_changed','application_restarted','history_trimmed')));CREATE TABLE pairs(namespace_key TEXT NOT NULL,epoch_id INTEGER NOT NULL,pair_index INTEGER NOT NULL CHECK(pair_index>=0),user_text TEXT NOT NULL CHECK(length(CAST(user_text AS BLOB))<=16384),assistant_text TEXT NOT NULL CHECK(length(CAST(assistant_text AS BLOB))<=16384),PRIMARY KEY(namespace_key,epoch_id,pair_index),FOREIGN KEY(namespace_key,epoch_id) REFERENCES epochs(namespace_key,epoch_id) ON DELETE CASCADE);";
const FAULT_CREATE: u8 = 1;
const FAULT_MIGRATION_BEFORE_COMMIT: u8 = 2;
const FAULT_MIGRATION_COMMIT: u8 = 3;
const FAULT_ARCHIVE: u8 = 4;
const FAULT_MUTATION: u8 = 5;
#[cfg(test)]
thread_local! {
    static TEST_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn fault(point: u8) -> Result<(), ()> {
    if TEST_FAULT.with(|fault| fault.get()) == point {
        Err(())
    } else {
        Ok(())
    }
}
#[cfg(not(test))]
fn fault(_: u8) -> Result<(), ()> {
    Ok(())
}
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
    pub resume_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<Warning>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResumePair {
    pub user: String,
    pub assistant: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResumeError {
    Unavailable,
    Incompatible,
    SaveFailed,
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
struct V2 {
    version: u8,
    epochs: Vec<Epoch>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct V3 {
    version: u8,
    namespaces: BTreeMap<String, V2>,
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
pub(crate) struct Persistence {
    connection: Option<Connection>,
    selected_namespace: Option<String>,
    restart_pending: BTreeSet<String>,
    warning: Option<Warning>,
}
impl Persistence {
    pub(crate) fn start(directory: PathBuf) -> Self {
        let _ = fs::create_dir_all(&directory);
        match open_or_migrate(&directory) {
            Ok(c) => {
                let pending = names_with_pairs(&c).unwrap_or_default();
                Self {
                    connection: Some(c),
                    selected_namespace: None,
                    restart_pending: pending,
                    warning: None,
                }
            }
            Err(()) => Self {
                connection: None,
                selected_namespace: None,
                restart_pending: BTreeSet::new(),
                warning: Some(Warning::RestoreFailed),
            },
        }
    }
    pub(crate) fn select_namespace(&mut self, n: String) {
        if !valid_namespace(&n) {
            self.warning = Some(Warning::RestoreFailed);
            return;
        }
        self.selected_namespace = Some(n.clone());
        if self.restart_pending.remove(&n)
            && self
                .separator(&n, SeparatorReason::ApplicationRestarted)
                .is_err()
        {
            self.warning = Some(Warning::SaveFailed)
        }
    }
    pub(crate) fn presentation(&self) -> Presentation {
        let v = self
            .selected_namespace
            .as_deref()
            .and_then(|n| self.load(n).ok());
        let records = v.as_ref().map(present).unwrap_or_default();
        let resume_available = v.as_ref().is_some_and(|v| resume_source(v).is_ok());
        Presentation {
            records,
            resume_available,
            warning: self.warning,
        }
    }
    pub(crate) fn resume_messages(&self) -> Result<Vec<ResumePair>, ResumeError> {
        let v = self
            .load(
                self.selected_namespace
                    .as_deref()
                    .ok_or(ResumeError::Unavailable)?,
            )
            .map_err(|_| ResumeError::Incompatible)?;
        reconstruct(&v, resume_source(&v)?)
    }
    pub(crate) fn commit_resume_lineage(&mut self) -> Result<(), ResumeError> {
        let n = self
            .selected_namespace
            .clone()
            .ok_or(ResumeError::Unavailable)?;
        self.mutate(&n, |v| {
            let s = resume_source(v)?;
            v.epochs
                .last_mut()
                .ok_or(ResumeError::Incompatible)?
                .parent_epoch_id = Some(s);
            Ok(())
        })
    }
    pub(crate) fn append_pair(&mut self, user: String, assistant: String) -> Result<(), Warning> {
        let n = self.selected_namespace.clone().ok_or(Warning::SaveFailed)?;
        self.mutate(&n, |v| {
            apply(v, Some(Pair { user, assistant }), None).map_err(|_| ResumeError::SaveFailed)
        })
        .map_err(|_| Warning::SaveFailed)
    }
    pub(crate) fn append_separator(&mut self, r: SeparatorReason) -> Result<(), Warning> {
        let n = self.selected_namespace.clone().ok_or(Warning::SaveFailed)?;
        self.separator(&n, r).map_err(|_| Warning::SaveFailed)
    }
    fn separator(&mut self, n: &str, r: SeparatorReason) -> Result<(), ResumeError> {
        self.mutate(n, |v| {
            apply(v, None, Some(r)).map_err(|_| ResumeError::SaveFailed)
        })
    }
    pub(crate) fn clear(&mut self) -> io::Result<()> {
        let n = self
            .selected_namespace
            .clone()
            .unwrap_or_else(|| "neutral-v1".into());
        let c = self
            .connection
            .as_mut()
            .ok_or_else(|| io::Error::other("unavailable"))?;
        let t = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(io::Error::other)?;
        t.execute("DELETE FROM namespaces WHERE namespace_key=?1", params![n])
            .map_err(io::Error::other)?;
        t.commit().map_err(io::Error::other)?;
        self.warning = None;
        Ok(())
    }
    fn load(&self, n: &str) -> Result<V2, ()> {
        load(self.connection.as_ref().ok_or(())?, n)
    }
    fn mutate<F>(&mut self, n: &str, f: F) -> Result<(), ResumeError>
    where
        F: FnOnce(&mut V2) -> Result<(), ResumeError>,
    {
        let c = self.connection.as_mut().ok_or(ResumeError::SaveFailed)?;
        let t = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| ResumeError::SaveFailed)?;
        let mut v = load(&t, n).map_err(|_| ResumeError::SaveFailed)?;
        f(&mut v)?;
        if !trim(&mut v) || validate(&v).is_err() {
            return Err(ResumeError::SaveFailed);
        }
        replace(&t, n, &v).map_err(|_| ResumeError::SaveFailed)?;
        fault(FAULT_MUTATION).map_err(|_| ResumeError::SaveFailed)?;
        t.commit().map_err(|_| ResumeError::SaveFailed)
    }
}
fn open_or_migrate(d: &Path) -> Result<Connection, ()> {
    let final_path = d.join(DB);
    if final_path.exists() {
        return open(&final_path).map_err(|()| {
            // A final filename is authoritative even when it cannot be opened.
            // Never resurrect stale V3 data after this point.
            quarantine(&final_path);
        });
    }
    // A prior final database was corrupt. Its V3 predecessor must remain
    // inert: allowing an import here would resurrect stale history.
    if quarantine_path(&final_path).exists() {
        return Err(());
    }
    let staging = d.join(STAGING);
    if staging.exists() {
        if open(&staging).is_ok() {
            fs::rename(&staging, &final_path).map_err(|_| ())?;
            return open(&final_path);
        }
        let _ = fs::rename(&staging, staging.with_extension("importing.corrupt"));
    }
    let legacy = d.join(V3_FILE);
    let v3 = if legacy.exists() {
        Some(read_v3(&legacy)?)
    } else {
        None
    };
    create(&staging, v3.as_ref())?;
    open(&staging)?;
    fs::rename(&staging, &final_path).map_err(|_| ())?;
    if v3.is_some() && fault(FAULT_ARCHIVE).is_ok() {
        let _ = fs::rename(
            legacy,
            d.join("conversation-transcript-v3.json.migrated-v3"),
        );
    }
    open(&final_path)
}
fn open(p: &Path) -> Result<Connection, ()> {
    let c = Connection::open_with_flags(
        p,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| ())?;
    pragmas(&c)?;
    validate_db(&c)?;
    Ok(c)
}
fn pragmas(c: &Connection) -> Result<(), ()> {
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA busy_timeout=250;").map_err(|_|())?;
    let (a, b, c1, d): (i64, String, i64, i64) = (
        c.query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .map_err(|_| ())?,
        c.query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .map_err(|_| ())?,
        c.query_row("PRAGMA synchronous", [], |r| r.get(0))
            .map_err(|_| ())?,
        c.query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .map_err(|_| ())?,
    );
    if a == 1 && b.eq_ignore_ascii_case("delete") && c1 == 2 && d == 250 {
        Ok(())
    } else {
        Err(())
    }
}
fn create(p: &Path, input: Option<&V3>) -> Result<(), ()> {
    let _ = fs::remove_file(p);
    fault(FAULT_CREATE)?;
    let mut c = Connection::open(p).map_err(|_| ())?;
    pragmas(&c)?;
    c.execute_batch(SCHEMA_SQL).map_err(|_| ())?;
    let t = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| ())?;
    let (mut ns, mut es, mut ps) = (0i64, 0i64, 0i64);
    if let Some(v) = input {
        for (n, x) in &v.namespaces {
            replace(&t, n, x).map_err(|_| ())?;
            ns += 1;
            es += x.epochs.len() as i64;
            ps += x.epochs.iter().map(|e| e.pairs.len() as i64).sum::<i64>();
        }
    }
    t.execute(
        "INSERT INTO schema_metadata VALUES(1,?1,1,?2,?3,?4,?5)",
        params![
            SCHEMA_VERSION,
            if input.is_some() { "v3" } else { "empty" },
            ns,
            es,
            ps
        ],
    )
    .map_err(|_| ())?;
    t.execute_batch("PRAGMA user_version=1").map_err(|_| ())?;
    fault(FAULT_MIGRATION_BEFORE_COMMIT)?;
    fault(FAULT_MIGRATION_COMMIT)?;
    t.commit().map_err(|_| ())?;
    Ok(())
}
fn validate_db(c: &Connection) -> Result<(), ()> {
    let v: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| ())?;
    let (m, done): (i64, i64) = c
        .query_row(
            "SELECT schema_version,migration_complete FROM schema_metadata WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| ())?;
    let ok: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|_| ())?;
    if v != SCHEMA_VERSION || m != SCHEMA_VERSION || done != 1 || ok != "ok" || !schema_matches(c) {
        return Err(());
    }
    let mut s = c
        .prepare("SELECT namespace_key FROM namespaces")
        .map_err(|_| ())?;
    let names = s
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|_| ())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    if names.len() > MAX_NAMESPACES {
        return Err(());
    }
    for n in names {
        if !valid_namespace(&n) || validate(&load(c, &n)?).is_err() {
            return Err(());
        }
    }
    Ok(())
}
fn schema_matches(c: &Connection) -> bool {
    let expected = ["schema_metadata", "namespaces", "epochs", "pairs"];
    let actual = c.prepare("SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>());
    let Ok(actual) = actual else { return false };
    if actual.len() != expected.len()
        || actual
            .iter()
            .any(|(name, _)| !expected.contains(&name.as_str()))
    {
        return false;
    }
    let schema = SCHEMA_SQL
        .split(";")
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    schema
        .iter()
        .all(|ddl| actual.iter().any(|(_, sql)| sql.eq_ignore_ascii_case(ddl)))
}
fn quarantine(path: &Path) {
    let corrupt = quarantine_path(path);
    let journal = journal_path(path);
    let corrupt_journal = journal_path(&corrupt);
    // Keep one bounded generation. If rotating it fails, preserve the
    // authoritative files in place and let startup fail closed.
    if corrupt.exists() && fs::remove_file(&corrupt).is_err() {
        return;
    }
    if corrupt_journal.exists() && fs::remove_file(&corrupt_journal).is_err() {
        return;
    }
    if journal.exists() && fs::rename(&journal, &corrupt_journal).is_err() {
        return;
    }
    if fs::rename(path, &corrupt).is_err() {
        // Best effort rollback keeps the main database and its journal paired.
        if corrupt_journal.exists() {
            let _ = fs::rename(&corrupt_journal, &journal);
        }
    }
}
fn quarantine_path(path: &Path) -> PathBuf {
    path.with_extension("sqlite3.corrupt")
}
fn journal_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}-journal", path.display()))
}
fn names_with_pairs(c: &Connection) -> Result<BTreeSet<String>, ()> {
    let mut s=c.prepare("SELECT DISTINCT e.namespace_key FROM epochs e JOIN pairs p ON p.namespace_key=e.namespace_key AND p.epoch_id=e.epoch_id").map_err(|_|())?;
    s.query_map([], |r| r.get(0))
        .map_err(|_| ())?
        .collect::<Result<_, _>>()
        .map_err(|_| ())
}
fn load(c: &Connection, n: &str) -> Result<V2, ()> {
    if !valid_namespace(n) {
        return Err(());
    }
    let mut s=c.prepare("SELECT epoch_id,parent_epoch_id,boundary,history_trimmed_before FROM epochs WHERE namespace_key=?1 ORDER BY epoch_id").map_err(|_|())?;
    let rows = s
        .query_map(params![n], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|_| ())?;
    let mut epochs = vec![];
    for row in rows {
        let (id, parent, boundary, trim) = row.map_err(|_| ())?;
        let mut p=c.prepare("SELECT user_text,assistant_text FROM pairs WHERE namespace_key=?1 AND epoch_id=?2 ORDER BY pair_index").map_err(|_|())?;
        let pairs = p
            .query_map(params![n, id], |r| {
                Ok(Pair {
                    user: r.get(0)?,
                    assistant: r.get(1)?,
                })
            })
            .map_err(|_| ())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ())?;
        epochs.push(Epoch {
            id: id as u64,
            parent_epoch_id: parent.map(|x| x as u64),
            boundary: boundary.as_deref().map(reason).transpose()?,
            history_trimmed_before: trim == 1,
            pairs,
        });
    }
    let v = V2 { version: 2, epochs };
    if !v.epochs.is_empty() {
        validate(&v)?
    }
    Ok(v)
}
fn replace(c: &Connection, n: &str, v: &V2) -> rusqlite::Result<()> {
    c.execute("INSERT OR IGNORE INTO namespaces VALUES(?1)", params![n])?;
    c.execute("DELETE FROM epochs WHERE namespace_key=?1", params![n])?;
    for e in &v.epochs {
        c.execute(
            "INSERT INTO epochs VALUES(?1,?2,?3,?4,?5)",
            params![
                n,
                e.id as i64,
                e.parent_epoch_id.map(|x| x as i64),
                e.boundary.map(name),
                i64::from(e.history_trimmed_before)
            ],
        )?;
        for (i, p) in e.pairs.iter().enumerate() {
            c.execute(
                "INSERT INTO pairs VALUES(?1,?2,?3,?4,?5)",
                params![n, e.id as i64, i as i64, p.user, p.assistant],
            )?;
        }
    }
    Ok(())
}
fn name(r: SeparatorReason) -> &'static str {
    match r {
        SeparatorReason::NewConversation => "new_conversation",
        SeparatorReason::RepositoryChanged => "repository_changed",
        SeparatorReason::ModelConfigurationChanged => "model_configuration_changed",
        SeparatorReason::RepositoryAndModelChanged => "repository_and_model_changed",
        SeparatorReason::ApplicationRestarted => "application_restarted",
        SeparatorReason::HistoryTrimmed => "history_trimmed",
    }
}
fn reason(s: &str) -> Result<SeparatorReason, ()> {
    match s {
        "new_conversation" => Ok(SeparatorReason::NewConversation),
        "repository_changed" => Ok(SeparatorReason::RepositoryChanged),
        "model_configuration_changed" => Ok(SeparatorReason::ModelConfigurationChanged),
        "repository_and_model_changed" => Ok(SeparatorReason::RepositoryAndModelChanged),
        "application_restarted" => Ok(SeparatorReason::ApplicationRestarted),
        "history_trimmed" => Ok(SeparatorReason::HistoryTrimmed),
        _ => Err(()),
    }
}
fn present(v: &V2) -> Vec<PresentationRecord> {
    let mut r = vec![];
    for e in &v.epochs {
        if e.history_trimmed_before {
            r.push(PresentationRecord::ContextSeparator {
                reason: SeparatorReason::HistoryTrimmed,
            })
        }
        if let Some(x) = e.boundary {
            r.push(PresentationRecord::ContextSeparator { reason: x })
        }
        for p in &e.pairs {
            r.push(PresentationRecord::CompletedMessage {
                role: PresentationRole::User,
                text: p.user.clone(),
            });
            r.push(PresentationRecord::CompletedMessage {
                role: PresentationRole::Assistant,
                text: p.assistant.clone(),
            });
        }
    }
    r
}
fn apply(v: &mut V2, p: Option<Pair>, r: Option<SeparatorReason>) -> Result<(), ()> {
    if let Some(r) = r {
        v.epochs.push(Epoch {
            id: v
                .epochs
                .last()
                .map(|e| e.id.checked_add(1).ok_or(()))
                .unwrap_or(Ok(1))?,
            parent_epoch_id: None,
            boundary: Some(r),
            history_trimmed_before: false,
            pairs: vec![],
        })
    }
    if let Some(p) = p {
        if v.epochs.is_empty() {
            v.epochs.push(Epoch {
                id: 1,
                parent_epoch_id: None,
                boundary: None,
                history_trimmed_before: false,
                pairs: vec![],
            })
        }
        v.epochs.last_mut().ok_or(())?.pairs.push(p)
    }
    Ok(())
}
fn trim(v: &mut V2) -> bool {
    while validate(v).is_err() {
        if v.epochs.len() < 2 {
            return false;
        }
        v.epochs.remove(0);
        v.epochs[0].parent_epoch_id = None;
        v.epochs[0].history_trimmed_before = true
    }
    true
}
fn validate(v: &V2) -> Result<(), ()> {
    if v.version != 2 || v.epochs.len() > MAX_EPOCHS {
        return Err(());
    }
    let (mut count, mut prior) = (0, 0);
    for (i, e) in v.epochs.iter().enumerate() {
        if e.id == 0
            || e.id <= prior
            || (i == 0 && !e.history_trimmed_before && e.boundary.is_some())
            || (i == 0 && e.history_trimmed_before && e.boundary.is_none())
            || (i > 0 && (e.boundary.is_none() || e.history_trimmed_before))
        {
            return Err(());
        }
        prior = e.id;
        if let Some(x) = e.parent_epoch_id
            && (e.boundary != Some(SeparatorReason::ApplicationRestarted)
                || x >= e.id
                || !v.epochs[..i]
                    .iter()
                    .any(|x| x.id == e.parent_epoch_id.unwrap()))
        {
            return Err(());
        }
        for p in &e.pairs {
            count += 1;
            if p.user.len() > MAX_MESSAGE_BYTES || p.assistant.len() > MAX_MESSAGE_BYTES {
                return Err(());
            }
        }
    }
    if count > MAX_PAIRS
        || count
            + v.epochs.len().saturating_sub(1)
            + usize::from(v.epochs.iter().any(|e| e.history_trimmed_before))
            > MAX_RECORDS
    {
        Err(())
    } else {
        Ok(())
    }
}
fn valid_namespace(s: &str) -> bool {
    s == "neutral-v1"
        || (s.len() == 76
            && s.starts_with("repo-sha256:")
            && s[12..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
}
fn read_v3(p: &Path) -> Result<V3, ()> {
    let mut b = vec![];
    if fs::metadata(p).map_err(|_| ())?.len() as usize > MAX_BYTES {
        return Err(());
    }
    fs::File::open(p)
        .map_err(|_| ())?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut b)
        .map_err(|_| ())?;
    let v: V3 = serde_json::from_slice(&b).map_err(|_| ())?;
    if v.version == 3
        && v.namespaces.len() <= MAX_NAMESPACES
        && v.namespaces
            .iter()
            .all(|(n, v)| valid_namespace(n) && validate(v).is_ok())
    {
        Ok(v)
    } else {
        Err(())
    }
}
fn resume_source(v: &V2) -> Result<u64, ResumeError> {
    let c = v.epochs.last().ok_or(ResumeError::Unavailable)?;
    if c.boundary != Some(SeparatorReason::ApplicationRestarted)
        || c.parent_epoch_id.is_some()
        || c.history_trimmed_before
    {
        return Err(ResumeError::Unavailable);
    }
    for e in v.epochs[..v.epochs.len() - 1].iter().rev() {
        if e.history_trimmed_before {
            return Err(ResumeError::Incompatible);
        }
        if e.pairs.is_empty() && e.boundary == Some(SeparatorReason::ApplicationRestarted) {
            continue;
        }
        if matches!(
            e.boundary,
            Some(
                SeparatorReason::NewConversation
                    | SeparatorReason::RepositoryChanged
                    | SeparatorReason::ModelConfigurationChanged
                    | SeparatorReason::RepositoryAndModelChanged
                    | SeparatorReason::HistoryTrimmed
            )
        ) {
            return Err(ResumeError::Unavailable);
        }
        return if e.pairs.is_empty() {
            Err(ResumeError::Unavailable)
        } else {
            Ok(e.id)
        };
    }
    Err(ResumeError::Unavailable)
}
fn reconstruct(v: &V2, id: u64) -> Result<Vec<ResumePair>, ResumeError> {
    let (mut out, mut id) = (vec![], Some(id));
    while let Some(x) = id {
        let e = v
            .epochs
            .iter()
            .find(|e| e.id == x)
            .ok_or(ResumeError::Incompatible)?;
        if e.history_trimmed_before {
            return Err(ResumeError::Incompatible);
        }
        out.push(e);
        id = e.parent_epoch_id
    }
    out.reverse();
    Ok(out
        .into_iter()
        .flat_map(|e| &e.pairs)
        .map(|p| ResumePair {
            user: p.user.clone(),
            assistant: p.assistant.clone(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }
    fn directory(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("rah-task127-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn v3(names: &[(&str, &str)]) -> V3 {
        V3 {
            version: 3,
            namespaces: names
                .iter()
                .map(|(n, text)| {
                    (
                        (*n).into(),
                        V2 {
                            version: 2,
                            epochs: vec![Epoch {
                                id: 1,
                                parent_epoch_id: None,
                                boundary: None,
                                history_trimmed_before: false,
                                pairs: vec![Pair {
                                    user: (*text).into(),
                                    assistant: "assistant".into(),
                                }],
                            }],
                        },
                    )
                })
                .collect(),
        }
    }
    fn write_v3(d: &Path, value: &V3) {
        fs::write(d.join(V3_FILE), serde_json::to_vec(value).unwrap()).unwrap();
    }
    fn select(p: &mut Persistence, n: &str) {
        p.select_namespace(n.into());
    }
    fn texts(p: &Persistence) -> Vec<String> {
        p.presentation()
            .records
            .into_iter()
            .filter_map(|r| match r {
                PresentationRecord::CompletedMessage { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }
    fn row_counts(d: &Path) -> (i64, i64, i64) {
        let c = Connection::open(d.join(DB)).unwrap();
        (
            c.query_row("SELECT count(*) FROM namespaces", [], |r| r.get(0))
                .unwrap(),
            c.query_row("SELECT count(*) FROM epochs", [], |r| r.get(0))
                .unwrap(),
            c.query_row("SELECT count(*) FROM pairs", [], |r| r.get(0))
                .unwrap(),
        )
    }

    #[test]
    fn empty_database_has_exact_schema_and_pragmas() {
        let _guard = lock();
        let d = directory("empty");
        let p = Persistence::start(d.clone());
        let c = p.connection.as_ref().unwrap();
        assert!(schema_matches(c));
        assert_eq!(
            c.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                .unwrap()
                .to_ascii_lowercase(),
            "delete"
        );
        assert_eq!(
            c.query_row("PRAGMA synchronous", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            c.query_row("PRAGMA busy_timeout", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            250
        );
        drop(p);
        assert!(d.join(DB).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn migration_archive_failure_is_authoritative_once_after_restart() {
        let _guard = lock();
        let d = directory("migration");
        let key = "repo-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        write_v3(&d, &v3(&[("neutral-v1", "neutral"), (key, "repo")]));
        TEST_FAULT.with(|fault| fault.set(FAULT_ARCHIVE));
        let mut p = Persistence::start(d.clone());
        select(&mut p, key);
        assert_eq!(texts(&p), ["repo", "assistant"]);
        assert!(d.join(DB).exists() && d.join(V3_FILE).exists());
        assert_eq!(row_counts(&d), (2, 3, 2));
        drop(p);
        TEST_FAULT.with(|fault| fault.set(0));
        let mut restart = Persistence::start(d.clone());
        select(&mut restart, key);
        assert_eq!(texts(&restart), ["repo", "assistant",]);
        assert!(d.join(V3_FILE).exists());
        // V3 was never read again: no duplicate namespace, epoch, or pair.
        assert_eq!(row_counts(&d), (2, 4, 2));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn valid_authoritative_sqlite_wins_over_stale_v3() {
        let _guard = lock();
        let d = directory("sqlite-wins");
        write_v3(&d, &v3(&[("neutral-v1", "first")]));
        let mut imported = Persistence::start(d.clone());
        select(&mut imported, "neutral-v1");
        assert_eq!(texts(&imported), ["first", "assistant"]);
        drop(imported);
        write_v3(&d, &v3(&[("neutral-v1", "stale")]));
        let mut reopened = Persistence::start(d.clone());
        select(&mut reopened, "neutral-v1");
        assert_eq!(texts(&reopened), ["first", "assistant"]);
        assert_eq!(row_counts(&d), (1, 3, 1));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn incomplete_migration_is_not_authoritative_and_retries_from_v3() {
        let _guard = lock();
        let d = directory("retry");
        write_v3(&d, &v3(&[("neutral-v1", "v3")]));
        TEST_FAULT.with(|fault| fault.set(FAULT_MIGRATION_BEFORE_COMMIT));
        assert_eq!(
            Persistence::start(d.clone()).warning,
            Some(Warning::RestoreFailed)
        );
        assert!(d.join(V3_FILE).exists() && !d.join(DB).exists());
        TEST_FAULT.with(|fault| fault.set(0));
        let mut p = Persistence::start(d.clone());
        select(&mut p, "neutral-v1");
        assert_eq!(texts(&p), ["v3", "assistant"]);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn corrupt_or_incomplete_authoritative_sqlite_fails_closed_without_v3_fallback() {
        let _guard = lock();
        let d = directory("corrupt");
        write_v3(&d, &v3(&[("neutral-v1", "old")]));
        fs::write(d.join(DB), b"not sqlite").unwrap();
        let mut p = Persistence::start(d.clone());
        select(&mut p, "neutral-v1");
        assert_eq!(p.warning, Some(Warning::RestoreFailed));
        assert!(texts(&p).is_empty());
        assert!(d.join("conversation-transcript.sqlite3.corrupt").exists());
        drop(p);
        let mut reopened = Persistence::start(d.clone());
        select(&mut reopened, "neutral-v1");
        assert_eq!(reopened.warning, Some(Warning::RestoreFailed));
        assert!(texts(&reopened).is_empty());
        assert!(d.join(V3_FILE).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn quarantine_rotates_one_generation_and_moves_delete_journal() {
        let _guard = lock();
        let d = directory("quarantine");
        let db = d.join(DB);
        fs::write(&db, b"corrupt").unwrap();
        fs::write(journal_path(&db), b"journal").unwrap();
        fs::write(quarantine_path(&db), b"old").unwrap();
        fs::write(journal_path(&quarantine_path(&db)), b"old-journal").unwrap();
        quarantine(&db);
        assert_eq!(fs::read(quarantine_path(&db)).unwrap(), b"corrupt");
        assert_eq!(
            fs::read(journal_path(&quarantine_path(&db))).unwrap(),
            b"journal"
        );
        assert!(!db.exists());
        assert!(!journal_path(&db).exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn supported_version_with_malformed_schema_fails_closed() {
        let _guard = lock();
        let d = directory("schema");
        let c = Connection::open(d.join(DB)).unwrap();
        c.execute_batch("CREATE TABLE schema_metadata(singleton INTEGER PRIMARY KEY, schema_version INTEGER, migration_complete INTEGER); INSERT INTO schema_metadata VALUES(1,1,1); PRAGMA user_version=1;").unwrap();
        drop(c);
        let p = Persistence::start(d.clone());
        assert_eq!(p.warning, Some(Warning::RestoreFailed));
        assert!(d.join("conversation-transcript.sqlite3.corrupt").exists());
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn namespace_mutations_are_isolated_and_failed_transaction_is_not_visible() {
        let _guard = lock();
        let d = directory("isolation");
        let a = "repo-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let b = "repo-sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let mut p = Persistence::start(d.clone());
        select(&mut p, a);
        p.append_pair("a".into(), "aa".into()).unwrap();
        select(&mut p, b);
        assert!(texts(&p).is_empty());
        TEST_FAULT.with(|fault| fault.set(FAULT_MUTATION));
        assert_eq!(
            p.append_pair("b".into(), "bb".into()),
            Err(Warning::SaveFailed)
        );
        assert!(texts(&p).is_empty());
        TEST_FAULT.with(|fault| fault.set(0));
        p.append_pair("b".into(), "bb".into()).unwrap();
        select(&mut p, a);
        assert_eq!(texts(&p), ["a", "aa"]);
        p.clear().unwrap();
        assert!(texts(&p).is_empty());
        select(&mut p, b);
        assert_eq!(texts(&p), ["b", "bb"]);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn message_limit_is_measured_in_utf8_bytes() {
        let mut value = V2 {
            version: 2,
            epochs: vec![Epoch {
                id: 1,
                parent_epoch_id: None,
                boundary: None,
                history_trimmed_before: false,
                pairs: vec![Pair {
                    user: "\u{00e9}".repeat(MAX_MESSAGE_BYTES / 2 + 1),
                    assistant: String::new(),
                }],
            }],
        };
        assert!(validate(&value).is_err());
        value.epochs[0].pairs[0].user = "\u{00e9}".repeat(MAX_MESSAGE_BYTES / 2);
        assert!(validate(&value).is_ok());
    }
}
