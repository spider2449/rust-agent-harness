use futures::executor::block_on;
use rah_protocol::SessionId;
use rah_session::{AgentContext, MemorySessionStore, Session, SessionStatus, SessionStore};

async fn assert_session_store_conformance(store: &dyn SessionStore) {
    let first_id = SessionId::new();
    let second_id = SessionId::new();
    assert!(
        store
            .load(first_id.clone())
            .await
            .expect("missing load")
            .is_none()
    );
    let mut first = session(first_id.clone(), SessionStatus::Running);
    let second = session(second_id.clone(), SessionStatus::Completed);
    store.save(&first).await.expect("save first");
    store.save(&second).await.expect("save second");
    assert_eq!(
        store.load(first_id.clone()).await.expect("load first"),
        Some(first.clone())
    );
    assert_eq!(
        store.load(second_id).await.expect("load second"),
        Some(second)
    );
    first.status = SessionStatus::Cancelled;
    store.save(&first).await.expect("update first");
    assert_eq!(
        store.load(first_id).await.expect("load update"),
        Some(first)
    );
}

fn session(id: SessionId, status: SessionStatus) -> Session {
    Session {
        id,
        status,
        context: AgentContext::default(),
    }
}

#[test]
fn memory_store_satisfies_session_store_contract() {
    block_on(assert_session_store_conformance(&MemorySessionStore::new()));
}
