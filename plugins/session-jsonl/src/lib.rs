//! `session-jsonl`: append-only JSONL session logs.
//!
//! Each session is `<dir>/<session-id>.jsonl`: one header line, then one line
//! per record. A fork writes only a header naming its parent and fork point;
//! reading a fork concatenates the parent prefix with the fork's own records.
//! A torn final line (no trailing newline, as after a crash) is ignored when
//! reading and removed before the next append.
//!
//! Configuration: `{ "dir": "<path>" }`, default `.rah/sessions`.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    session::{SESSION, SessionError, SessionLog},
};
use rah_protocol::{ForkPoint, SessionEntry, SessionId, SessionInfo, SessionRecord};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const FORMAT_VERSION: u32 = 1;
const MAX_FORK_DEPTH: usize = 64;

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

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let dir = config.dir.unwrap_or_else(|| PathBuf::from(".rah/sessions"));
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("session directory `{}`: {error}", dir.display()))?;
        scope.provide(SESSION, Arc::new(JsonlSessions::new(dir)))?;
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
    },
    Record {
        seq: u64,
        entry: SessionEntry,
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
}

impl JsonlSessions {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            next_seq: Mutex::new(HashMap::new()),
        }
    }

    fn path(&self, session: &SessionId) -> PathBuf {
        self.dir.join(format!("{session}.jsonl"))
    }

    async fn parse(&self, session: &SessionId) -> Result<Parsed, SessionError> {
        let text = match tokio::fs::read_to_string(self.path(session)).await {
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
        let info = match serde_json::from_str(header) {
            Ok(Line::Header {
                version: FORMAT_VERSION,
                session: id,
                parent,
                created,
            }) if id == *session => SessionInfo {
                id,
                parent,
                created,
            },
            _ => return Err(corrupt("invalid header".into())),
        };
        let mut records = Vec::new();
        for (index, line) in lines.enumerate() {
            match serde_json::from_str(line) {
                Ok(Line::Record { seq, entry }) => records.push(SessionRecord { seq, entry }),
                _ => return Err(corrupt(format!("invalid record line {}", index + 2))),
            }
        }
        Ok(Parsed {
            info,
            records,
            torn_at,
        })
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

    async fn write_new(&self, info: &SessionInfo) -> Result<(), SessionError> {
        let header = Line::Header {
            version: FORMAT_VERSION,
            session: info.id.clone(),
            parent: info.parent.clone(),
            created: info.created,
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

#[async_trait]
impl SessionLog for JsonlSessions {
    async fn create(&self) -> Result<SessionInfo, SessionError> {
        let mut next = self.next_seq.lock().await;
        let info = SessionInfo {
            id: SessionId::new(),
            parent: None,
            created: now(),
        };
        self.write_new(&info).await?;
        next.insert(info.id.clone(), 1);
        Ok(info)
    }

    async fn append(&self, session: &SessionId, entry: SessionEntry) -> Result<u64, SessionError> {
        let mut next = self.next_seq.lock().await;
        let seq = match next.get(session) {
            Some(seq) => *seq,
            None => {
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
                let last = parsed
                    .records
                    .last()
                    .map(|r| r.seq)
                    .or(parsed.info.parent.map(|p| p.at))
                    .unwrap_or(0);
                last + 1
            }
        };
        let mut line = serde_json::to_string(&Line::Record { seq, entry })
            .map_err(|e| SessionError::Io(e.to_string()))?;
        line.push('\n');
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(self.path(session))
            .await
            .map_err(|_| SessionError::NotFound(session.clone()))?;
        file.write_all(line.as_bytes())
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        file.flush()
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        next.insert(session.clone(), seq + 1);
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
        let info = SessionInfo {
            id: SessionId::new(),
            parent: Some(ForkPoint {
                session: session.clone(),
                at,
            }),
            created: now(),
        };
        self.write_new(&info).await?;
        next.insert(info.id.clone(), at + 1);
        Ok(info)
    }

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        let _guard = self.next_seq.lock().await;
        let mut entries = tokio::fs::read_dir(&self.dir)
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?;
        let mut sessions = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::Io(e.to_string()))?
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name
                .strip_suffix(".jsonl")
                .and_then(|stem| stem.parse::<SessionId>().ok())
            else {
                continue;
            };
            if let Ok(parsed) = self.parse(&id).await {
                sessions.push(parsed.info);
            }
        }
        sessions.sort_by_key(|info| (info.created, info.id.to_string()));
        Ok(sessions)
    }
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
        assert_eq!(reopened.list().await.unwrap(), [info]);
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
        assert_eq!(log.list().await.unwrap().len(), 3);
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
}
