//! `session-jsonl`: append-only JSONL session logs.
//!
//! Each session is `<dir>/<session-id>.jsonl`: one header line, then one line
//! per record. A fork writes only a header naming its parent and fork point;
//! reading a fork concatenates the parent prefix with the fork's own records.
//! A torn final line (no trailing newline, as after a crash) is ignored when
//! reading and removed before the next write.
//!
//! Titles are metadata lines (`{"type":"title","title":...}`) appended like
//! records; the last one wins, and records are never rewritten. Deleting a
//! session moves its file to `<dir>/deleted/`, so it can be recovered by
//! moving it back; a session with forks cannot be deleted. Every append emits
//! [`SessionAppended`].
//!
//! Configuration: `{ "dir": "<path>" }`, default `.rah/sessions`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::SANDBOX,
    session::{SESSION, SessionAppended, SessionError, SessionLog},
};
use rah_protocol::{
    ForkPoint, SessionEntry, SessionId, SessionInfo, SessionRecord, normalize_title,
};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const FORMAT_VERSION: u32 = 1;
const MAX_FORK_DEPTH: usize = 64;
const PREVIEW_CHARS: usize = 120;
const DELETED_DIR: &str = "deleted";

pub struct JsonlSessionPlugin {
    manifest: PluginManifest,
}

impl JsonlSessionPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["session"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    dir: Option<PathBuf>,
}

#[async_trait]
impl Plugin for JsonlSessionPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<Config>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        let dir = config.dir.unwrap_or_else(|| PathBuf::from(".rah/sessions"));
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("session directory `{}`: {error}", dir.display()))?;
        let sessions = JsonlSessions::new(dir).with_events(scope.kernel());
        scope.provide(SESSION, Arc::new(sessions))?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Line {
    Header {
        version: u32,
        session: SessionId,
        parent: Option<ForkPoint>,
        created: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
    },
    Record {
        seq: u64,
        entry: SessionEntry,
    },
    Title {
        title: Option<String>,
    },
}

struct Parsed {
    info: SessionInfo,
    records: Vec<SessionRecord>,
    /// Byte length of the complete-line prefix, if a torn tail follows it.
    torn_at: Option<u64>,
}

/// JSONL-backed [`SessionLog`]. All operations are serialized.
pub struct JsonlSessions {
    dir: PathBuf,
    /// Next sequence number per session, loaded lazily.
    next_seq: Mutex<HashMap<SessionId, u64>>,
    events: Option<KernelHandle>,
    /// A fixed workspace for new sessions, instead of the sandbox root.
    workspace: Option<String>,
}

impl JsonlSessions {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            next_seq: Mutex::new(HashMap::new()),
            events: None,
            workspace: None,
        }
    }

    /// Emits [`SessionAppended`] through `kernel` after each append, and
    /// records the `sandbox` service's root as the workspace of new
    /// sessions.
    pub fn with_events(mut self, kernel: KernelHandle) -> Self {
        self.events = Some(kernel);
        self
    }

    /// Records `workspace` for new sessions instead of the sandbox root.
    pub fn with_workspace(mut self, workspace: impl Into<String>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    fn current_workspace(&self) -> Option<String> {
        self.workspace.clone().or_else(|| {
            let root = self.events.as_ref()?.get(SANDBOX)?.root()?;
            Some(root.display().to_string())
        })
    }

    fn path(&self, session: &SessionId) -> PathBuf {
        self.dir.join(format!("{session}.jsonl"))
    }

    async fn parse(&self, session: &SessionId) -> Result<Parsed, SessionError> {
        let path = self.path(session);
        let text = match tokio::fs::read_to_string(&path).await {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionError::NotFound(session.clone()));
            }
            Err(error) => return Err(SessionError::Io(error.to_string())),
        };
        let corrupt = |detail: String| SessionError::Corrupt {
            session: session.clone(),
            detail,
        };
        let complete = text.rfind('\n').map_or(0, |i| i + 1);
        let torn_at = (complete < text.len()).then_some(complete as u64);
        let mut lines = text[..complete].lines().filter(|l| !l.trim().is_empty());
        let header = lines
            .next()
            .ok_or_else(|| corrupt("missing header".into()))?;
        let mut info = match serde_json::from_str(header) {
            Ok(Line::Header {
                version: FORMAT_VERSION,
                session: id,
                parent,
                created,
                workspace,
            }) if id == *session => SessionInfo {
                id,
                parent,
                created,
                title: None,
                updated: created,
                preview: None,
                workspace,
            },
            _ => return Err(corrupt("invalid header".into())),
        };
        let mut records = Vec::new();
        for (index, line) in lines.enumerate() {
            match serde_json::from_str(line) {
                Ok(Line::Record { seq, entry }) => records.push(SessionRecord { seq, entry }),
                Ok(Line::Title { title }) => info.title = title,
                _ => return Err(corrupt(format!("invalid record line {}", index + 2))),
            }
        }
        if let Some(modified) = tokio::fs::metadata(&path)
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        {
            info.updated = info.updated.max(modified.as_secs());
        }
        Ok(Parsed {
            info,
            records,
            torn_at,
        })
    }

    /// Parses a session and removes a torn tail, so a new line can follow.
    async fn repair(&self, session: &SessionId) -> Result<Parsed, SessionError> {
        let parsed = self.parse(session).await?;
        if let Some(length) = parsed.torn_at {
            let file = tokio::fs::OpenOptions::new()
                .write(true)
                .open(self.path(session))
                .await
                .map_err(|e| SessionError::Io(e.to_string()))?;
            file.set_len(length)
                .await
                .map_err(|e| SessionError::Io(e.to_string()))?;
        }
        Ok(parsed)
    }

    async fn write_line(&self, session: &SessionId, line: &Line) -> Result<(), SessionError> {
        let mut text = serde_json::to_string(line).map_err(|e| SessionError::Io(e.to_string()))?;
        text.push('\n');
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(self.path(session))
            .await
            .map_err(|_| SessionError::NotFound(session.clone()))?;
        file.write_all(text.as_bytes())
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        file.flush()
            .await
            .map_err(|e| SessionError::Io(e.to_string()))
    }

    /// Visible records, following fork parents.
    async fn visible(&self, session: &SessionId) -> Result<Vec<SessionRecord>, SessionError> {
        let mut chain = Vec::new();
        let mut current = session.clone();
        let mut limit = None;
        loop {
            if chain.len() >= MAX_FORK_DEPTH {
                return Err(SessionError::Corrupt {
                    session: session.clone(),
                    detail: "fork chain is too deep".into(),
                });
            }
            let parsed = self.parse(&current).await?;
            let mut own = parsed.records;
            if let Some(at) = limit {
                own.retain(|r: &SessionRecord| r.seq <= at);
            }
            let parent = parsed.info.parent;
            chain.push(own);
            match parent {
                Some(fork) => {
                    limit = Some(limit.map_or(fork.at, |l: u64| l.min(fork.at)));
                    current = fork.session;
                }
                None => break,
            }
        }
        Ok(chain.into_iter().rev().flatten().collect())
    }

    /// Session info with the preview of its first user message.
    async fn described(&self, session: &SessionId) -> Result<SessionInfo, SessionError> {
        let mut info = self.parse(session).await?.info;
        info.preview = self
            .visible(session)
            .await?
            .iter()
            .find_map(|record| match &record.entry {
                SessionEntry::TurnStarted { input, .. } => normalize_title(input)
                    .map(|line| line.chars().take(PREVIEW_CHARS).collect::<String>()),
                _ => None,
            });
        Ok(info)
    }

    /// Ids of the sessions stored in the directory (not deleted ones).
    async fn ids(&self) -> Result<Vec<SessionId>, SessionError> {
        let mut entries = tokio::fs::read_dir(&self.dir)
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        let mut ids = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name
                .strip_suffix(".jsonl")
                .and_then(|stem| stem.parse::<SessionId>().ok())
            {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    async fn write_new(&self, info: &SessionInfo) -> Result<(), SessionError> {
        let header = Line::Header {
            version: FORMAT_VERSION,
            session: info.id.clone(),
            parent: info.parent.clone(),
            created: info.created,
            workspace: info.workspace.clone(),
        };
        let mut line =
            serde_json::to_string(&header).map_err(|e| SessionError::Io(e.to_string()))?;
        line.push('\n');
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path(&info.id))
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        file.flush()
            .await
            .map_err(|e| SessionError::Io(e.to_string()))
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn new_info(parent: Option<ForkPoint>, workspace: Option<String>) -> SessionInfo {
    let created = now();
    SessionInfo {
        id: SessionId::new(),
        parent,
        created,
        title: None,
        updated: created,
        preview: None,
        workspace,
    }
}

#[async_trait]
impl SessionLog for JsonlSessions {
    async fn create(&self) -> Result<SessionInfo, SessionError> {
        let mut next = self.next_seq.lock().await;
        let info = new_info(None, self.current_workspace());
        self.write_new(&info).await?;
        next.insert(info.id.clone(), 1);
        Ok(info)
    }

    async fn append(&self, session: &SessionId, entry: SessionEntry) -> Result<u64, SessionError> {
        let mut next = self.next_seq.lock().await;
        let seq = match next.get(session) {
            Some(seq) => *seq,
            None => {
                let parsed = self.repair(session).await?;
                let last = parsed
                    .records
                    .last()
                    .map(|r| r.seq)
                    .or(parsed.info.parent.map(|p| p.at))
                    .unwrap_or(0);
                last + 1
            }
        };
        let line = Line::Record {
            seq,
            entry: entry.clone(),
        };
        self.write_line(session, &line).await?;
        next.insert(session.clone(), seq + 1);
        drop(next);
        if let Some(kernel) = &self.events {
            kernel.emit(&SessionAppended {
                session: session.clone(),
                seq,
                entry,
            });
        }
        Ok(seq)
    }

    async fn read(&self, session: &SessionId) -> Result<Vec<SessionRecord>, SessionError> {
        let _guard = self.next_seq.lock().await;
        self.visible(session).await
    }

    async fn fork(&self, session: &SessionId, at: u64) -> Result<SessionInfo, SessionError> {
        let mut next = self.next_seq.lock().await;
        let last = self
            .visible(session)
            .await?
            .last()
            .map_or(0, |record| record.seq);
        if at > last {
            return Err(SessionError::InvalidFork {
                session: session.clone(),
                at,
                last,
            });
        }
        // A fork continues the parent's work, in the parent's workspace.
        let workspace = self.parse(session).await?.info.workspace;
        let info = new_info(
            Some(ForkPoint {
                session: session.clone(),
                at,
            }),
            workspace.or_else(|| self.current_workspace()),
        );
        self.write_new(&info).await?;
        next.insert(info.id.clone(), at + 1);
        Ok(info)
    }

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        let _guard = self.next_seq.lock().await;
        let mut sessions = Vec::new();
        for id in self.ids().await? {
            if let Ok(info) = self.described(&id).await {
                sessions.push(info);
            }
        }
        sessions.sort_by_key(|info| (info.created, info.id.to_string()));
        Ok(sessions)
    }

    async fn rename(
        &self,
        session: &SessionId,
        title: Option<String>,
    ) -> Result<SessionInfo, SessionError> {
        let title = match title {
            Some(title) => Some(normalize_title(&title).ok_or(SessionError::InvalidTitle)?),
            None => None,
        };
        let _guard = self.next_seq.lock().await;
        self.repair(session).await?;
        self.write_line(session, &Line::Title { title }).await?;
        self.described(session).await
    }

    async fn rename_if_untitled(
        &self,
        session: &SessionId,
        title: String,
    ) -> Result<bool, SessionError> {
        let title = normalize_title(&title).ok_or(SessionError::InvalidTitle)?;
        let _guard = self.next_seq.lock().await;
        if self.repair(session).await?.info.title.is_some() {
            return Ok(false);
        }
        self.write_line(session, &Line::Title { title: Some(title) })
            .await?;
        Ok(true)
    }

    async fn delete(&self, session: &SessionId) -> Result<(), SessionError> {
        let mut next = self.next_seq.lock().await;
        let path = self.path(session);
        if !tokio::fs::try_exists(&path)
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?
        {
            return Err(SessionError::NotFound(session.clone()));
        }
        let mut forks = Vec::new();
        for id in self.ids().await? {
            if let Ok(parsed) = self.parse(&id).await
                && parsed
                    .info
                    .parent
                    .is_some_and(|parent| parent.session == *session)
            {
                forks.push(id);
            }
        }
        if !forks.is_empty() {
            return Err(SessionError::HasForks {
                session: session.clone(),
                forks,
            });
        }
        let deleted = self.dir.join(DELETED_DIR);
        tokio::fs::create_dir_all(&deleted)
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        tokio::fs::rename(&path, deleted.join(format!("{session}.jsonl")))
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        next.remove(session);
        Ok(())
    }
}

impl JsonlSessionPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "dir": "sessions"
}"#;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::RequestId;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("rah-session-jsonl-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn started(input: &str) -> SessionEntry {
        SessionEntry::TurnStarted {
            turn: RequestId::new(),
            input: input.into(),
        }
    }

    fn inputs(records: &[SessionRecord]) -> Vec<(u64, String)> {
        records
            .iter()
            .map(|r| match &r.entry {
                SessionEntry::TurnStarted { input, .. } => (r.seq, input.clone()),
                other => (r.seq, format!("{other:?}")),
            })
            .collect()
    }

    #[tokio::test]
    async fn append_read_and_reopen_continue_the_sequence() {
        let dir = TempDir::new("reopen");
        let log = JsonlSessions::new(dir.0.clone());
        let info = log.create().await.unwrap();
        assert_eq!(log.append(&info.id, started("a")).await.unwrap(), 1);
        assert_eq!(log.append(&info.id, started("b")).await.unwrap(), 2);

        let reopened = JsonlSessions::new(dir.0.clone());
        assert_eq!(reopened.append(&info.id, started("c")).await.unwrap(), 3);
        assert_eq!(
            inputs(&reopened.read(&info.id).await.unwrap()),
            [(1, "a".into()), (2, "b".into()), (3, "c".into())]
        );
        let listed = reopened.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (&listed[0].id, listed[0].preview.as_deref()),
            (&info.id, Some("a"))
        );
        assert!(listed[0].updated >= info.created);
    }

    #[tokio::test]
    async fn forks_share_the_parent_prefix_and_diverge() {
        let dir = TempDir::new("fork");
        let log = JsonlSessions::new(dir.0.clone());
        let parent = log.create().await.unwrap();
        for input in ["a", "b", "c"] {
            log.append(&parent.id, started(input)).await.unwrap();
        }
        let fork = log.fork(&parent.id, 2).await.unwrap();
        assert_eq!(fork.parent.as_ref().map(|p| p.at), Some(2));
        assert_eq!(log.append(&fork.id, started("x")).await.unwrap(), 3);
        let nested = log.fork(&fork.id, 3).await.unwrap();
        log.append(&nested.id, started("y")).await.unwrap();

        assert_eq!(
            inputs(&log.read(&fork.id).await.unwrap()),
            [(1, "a".into()), (2, "b".into()), (3, "x".into())]
        );
        assert_eq!(
            inputs(&log.read(&nested.id).await.unwrap()),
            [
                (1, "a".into()),
                (2, "b".into()),
                (3, "x".into()),
                (4, "y".into())
            ]
        );
        assert_eq!(log.read(&parent.id).await.unwrap().len(), 3);
        assert!(matches!(
            log.fork(&parent.id, 9).await,
            Err(SessionError::InvalidFork { last: 3, .. })
        ));
        let listed = log.list().await.unwrap();
        assert_eq!(listed.len(), 3);
        // A fork previews the first message of its parent prefix.
        assert!(
            listed
                .iter()
                .all(|info| info.preview.as_deref() == Some("a"))
        );
    }

    #[tokio::test]
    async fn torn_tail_is_ignored_and_repaired_before_append() {
        let dir = TempDir::new("torn");
        let log = JsonlSessions::new(dir.0.clone());
        let info = log.create().await.unwrap();
        log.append(&info.id, started("a")).await.unwrap();
        let path = dir.0.join(format!("{}.jsonl", info.id));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, b"{\"type\":\"record\",\"seq\":2,\"en").unwrap();
        drop(file);

        let reopened = JsonlSessions::new(dir.0.clone());
        assert_eq!(reopened.read(&info.id).await.unwrap().len(), 1);
        assert_eq!(reopened.append(&info.id, started("b")).await.unwrap(), 2);
        assert_eq!(
            inputs(&reopened.read(&info.id).await.unwrap()),
            [(1, "a".into()), (2, "b".into())]
        );
    }

    #[tokio::test]
    async fn missing_and_corrupt_sessions_are_reported() {
        let dir = TempDir::new("errors");
        let log = JsonlSessions::new(dir.0.clone());
        let missing = SessionId::new();
        assert_eq!(
            log.read(&missing).await,
            Err(SessionError::NotFound(missing.clone()))
        );
        assert!(matches!(
            log.append(&missing, started("a")).await,
            Err(SessionError::NotFound(_))
        ));
        assert!(matches!(
            log.rename(&missing, Some("x".into())).await,
            Err(SessionError::NotFound(_))
        ));
        assert!(matches!(
            log.delete(&missing).await,
            Err(SessionError::NotFound(_))
        ));
        let info = log.create().await.unwrap();
        let path = dir.0.join(format!("{}.jsonl", info.id));
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, b"not json\n").unwrap();
        assert!(matches!(
            log.read(&info.id).await,
            Err(SessionError::Corrupt { .. })
        ));
    }

    #[tokio::test]
    async fn titles_are_appended_metadata_and_survive_reopening() {
        let dir = TempDir::new("title");
        let log = JsonlSessions::new(dir.0.clone());
        let info = log.create().await.unwrap();
        log.append(&info.id, started("a")).await.unwrap();
        let renamed = log
            .rename(&info.id, Some("  Plan\nthe   trip ".into()))
            .await
            .unwrap();
        assert_eq!(renamed.title.as_deref(), Some("Plan the trip"));
        // Records keep their sequence around a title line.
        assert_eq!(log.append(&info.id, started("b")).await.unwrap(), 2);
        assert_eq!(
            log.rename(&info.id, Some(" \n ".into())).await,
            Err(SessionError::InvalidTitle)
        );

        let reopened = JsonlSessions::new(dir.0.clone());
        assert_eq!(
            reopened.list().await.unwrap()[0].title.as_deref(),
            Some("Plan the trip")
        );
        assert_eq!(reopened.append(&info.id, started("c")).await.unwrap(), 3);
        assert_eq!(reopened.read(&info.id).await.unwrap().len(), 3);
        assert_eq!(
            reopened
                .rename_if_untitled(&info.id, "Generated".into())
                .await,
            Ok(false)
        );
        let cleared = reopened.rename(&info.id, None).await.unwrap();
        assert_eq!(cleared.title, None);
        assert_eq!(
            reopened
                .rename_if_untitled(&info.id, "Generated".into())
                .await,
            Ok(true)
        );
        assert_eq!(
            reopened.list().await.unwrap()[0].title.as_deref(),
            Some("Generated")
        );
        let long = "x".repeat(500);
        let renamed = reopened.rename(&info.id, Some(long)).await.unwrap();
        assert_eq!(
            renamed.title.map(|t| t.chars().count()),
            Some(rah_protocol::MAX_TITLE_CHARS)
        );
    }

    #[tokio::test]
    async fn sessions_record_their_workspace_and_forks_keep_it() {
        let dir = TempDir::new("workspace");
        let first = JsonlSessions::new(dir.0.clone()).with_workspace(r"D:\work\alpha");
        let info = first.create().await.unwrap();
        assert_eq!(info.workspace.as_deref(), Some(r"D:\work\alpha"));
        first.append(&info.id, started("a")).await.unwrap();

        // Another process, now in another workspace: the fork stays where
        // its parent's work happened; a new session takes the new one.
        let second = JsonlSessions::new(dir.0.clone()).with_workspace(r"D:\work\beta");
        let fork = second.fork(&info.id, 1).await.unwrap();
        assert_eq!(fork.workspace.as_deref(), Some(r"D:\work\alpha"));
        let other = second.create().await.unwrap();
        assert_eq!(other.workspace.as_deref(), Some(r"D:\work\beta"));

        let listed = second.list().await.unwrap();
        let workspace = |id: &SessionId| {
            listed
                .iter()
                .find(|listed| listed.id == *id)
                .and_then(|listed| listed.workspace.clone())
        };
        assert_eq!(workspace(&fork.id).as_deref(), Some(r"D:\work\alpha"));
        assert_eq!(workspace(&other.id).as_deref(), Some(r"D:\work\beta"));
        // Without a sandbox or a fixed workspace, none is recorded.
        let unknown = JsonlSessions::new(dir.0.clone()).create().await.unwrap();
        assert_eq!(unknown.workspace, None);
    }

    #[tokio::test]
    async fn deleting_moves_the_log_aside_and_refuses_parents_of_forks() {
        let dir = TempDir::new("delete");
        let log = JsonlSessions::new(dir.0.clone());
        let parent = log.create().await.unwrap();
        log.append(&parent.id, started("a")).await.unwrap();
        let fork = log.fork(&parent.id, 1).await.unwrap();

        match log.delete(&parent.id).await {
            Err(SessionError::HasForks { forks, .. }) => {
                assert_eq!(forks, std::slice::from_ref(&fork.id))
            }
            other => panic!("expected HasForks, got {other:?}"),
        }
        log.delete(&fork.id).await.unwrap();
        log.delete(&parent.id).await.unwrap();
        assert!(log.list().await.unwrap().is_empty());
        assert!(matches!(
            log.read(&parent.id).await,
            Err(SessionError::NotFound(_))
        ));
        let kept = dir.0.join(DELETED_DIR).join(format!("{}.jsonl", parent.id));
        assert!(kept.exists(), "the deleted log is kept for recovery");
    }
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(JsonlSessionPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
