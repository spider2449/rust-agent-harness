use std::{
    collections::HashMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use async_trait::async_trait;
use rah_protocol::SessionId;
use thiserror::Error;

use crate::Session;

/// Persistence boundary for provider-neutral session state.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Loads a session when the identifier exists.
    async fn load(&self, id: SessionId) -> Result<Option<Session>, SessionStoreError>;

    /// Creates or replaces a session by identifier.
    async fn save(&self, session: &Session) -> Result<(), SessionStoreError>;
}

/// Error produced by a session store implementation.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionStoreError {
    /// In-memory state became inaccessible after a lock owner panicked.
    #[error("session store lock is poisoned")]
    LockPoisoned,
}

/// Process-local session store for deterministic use and testing.
#[derive(Debug, Default)]
pub struct MemorySessionStore {
    sessions: RwLock<HashMap<SessionId, Session>>,
}

impl MemorySessionStore {
    /// Creates an empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn read_sessions(
        &self,
    ) -> Result<RwLockReadGuard<'_, HashMap<SessionId, Session>>, SessionStoreError> {
        self.sessions
            .read()
            .map_err(|_error| SessionStoreError::LockPoisoned)
    }

    fn write_sessions(
        &self,
    ) -> Result<RwLockWriteGuard<'_, HashMap<SessionId, Session>>, SessionStoreError> {
        self.sessions
            .write()
            .map_err(|_error| SessionStoreError::LockPoisoned)
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn load(&self, id: SessionId) -> Result<Option<Session>, SessionStoreError> {
        Ok(self.read_sessions()?.get(&id).cloned())
    }

    async fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
        self.write_sessions()?
            .insert(session.id.clone(), session.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use rah_protocol::SessionId;

    use crate::{AgentContext, Session, SessionStatus};

    use super::{MemorySessionStore, SessionStore};

    fn session(id: SessionId, status: SessionStatus) -> Session {
        Session {
            id,
            status,
            context: AgentContext::default(),
        }
    }

    #[test]
    fn missing_session_loads_as_none() {
        let store = MemorySessionStore::new();

        let loaded = block_on(store.load(SessionId::new())).expect("load should succeed");

        assert!(loaded.is_none());
    }

    #[test]
    fn saved_session_round_trips_by_identifier() {
        let store = MemorySessionStore::new();
        let expected = session(SessionId::new(), SessionStatus::Running);

        block_on(store.save(&expected)).expect("save should succeed");
        let loaded = block_on(store.load(expected.id.clone())).expect("load should succeed");

        assert_eq!(loaded, Some(expected));
    }

    #[test]
    fn saving_existing_identifier_replaces_session() {
        let store = MemorySessionStore::new();
        let id = SessionId::new();
        block_on(store.save(&session(id.clone(), SessionStatus::Running)))
            .expect("initial save should succeed");
        let updated = session(id.clone(), SessionStatus::Completed);

        block_on(store.save(&updated)).expect("update should succeed");

        assert_eq!(
            block_on(store.load(id)).expect("load should succeed"),
            Some(updated)
        );
    }
}
