use std::{error::Error, fmt};

use futures::StreamExt;
use rah_protocol::{
    AgentErrorCode, RuntimeDiagnostic, RuntimeFailureKind, RuntimeOperation, SessionId,
};
use rah_runtime::{AgentError, AgentHandle, RuntimeEvent, RuntimeFailure};

const SECRET: &str = "SOURCE_ONLY_SECRET_498A";

// Deliberately not Clone; this represents any provider's local error.
#[derive(Debug)]
struct PrivateError(&'static str);
impl fmt::Display for PrivateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for PrivateError {}

fn envelope() -> RuntimeFailure {
    RuntimeFailure::new(
        RuntimeDiagnostic {
            operation: RuntimeOperation::SessionStart,
            kind: RuntimeFailureKind::Protocol,
            rpc_code: Some(-32600),
        },
        PrivateError(SECRET),
    )
}

#[test]
fn source_downcast_and_clone_share_non_clone_original() {
    fn assert_thread_safe<T: Send + Sync + 'static>() {}
    assert_thread_safe::<RuntimeFailure>();
    assert_thread_safe::<AgentError>();
    let failure = envelope();
    let clone = failure.clone();
    assert_eq!(failure.diagnostic(), clone.diagnostic());
    let original = failure
        .source()
        .unwrap()
        .downcast_ref::<PrivateError>()
        .unwrap();
    let shared = clone
        .source()
        .unwrap()
        .downcast_ref::<PrivateError>()
        .unwrap();
    assert!(std::ptr::eq(original, shared));
    assert_eq!(shared.0, SECRET);
    let outer = AgentError::Failure { failure: clone };
    assert!(
        outer
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<PrivateError>()
    );
}

#[test]
fn diagnostic_serialization_and_default_formatting_redact_source() {
    let failure = envelope();
    let json = serde_json::to_string(failure.diagnostic()).unwrap();
    let restored: RuntimeDiagnostic = serde_json::from_str(&json).unwrap();
    assert_eq!(&restored, failure.diagnostic());
    let event = RuntimeEvent::failed(SessionId::new(), AgentErrorCode::Internal, failure.clone());
    let projected = serde_json::to_string(event.event()).unwrap();
    for safe in [
        json,
        projected,
        format!("{failure:?}"),
        failure.to_string(),
        format!("{event:?}"),
    ] {
        assert!(!safe.contains(SECRET), "source leaked: {safe}");
    }
    assert!(
        event
            .failure()
            .unwrap()
            .source()
            .unwrap()
            .is::<PrivateError>()
    );
}

#[tokio::test]
async fn handle_preserves_local_source_until_explicit_projection() {
    let id = SessionId::new();
    let event = RuntimeEvent::failed(id.clone(), AgentErrorCode::Internal, envelope());
    let handle = AgentHandle::with_runtime_events(id, Box::pin(futures::stream::iter([event])));
    let mut local = handle.into_runtime_events();
    let item = local.next().await.unwrap();
    assert!(
        item.failure()
            .unwrap()
            .source()
            .unwrap()
            .is::<PrivateError>()
    );
    assert!(local.next().await.is_none());
    assert!(
        !serde_json::to_string(&item.into_event())
            .unwrap()
            .contains(SECRET)
    );
}
