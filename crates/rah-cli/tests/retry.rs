//! Model requests the provider cannot take are opened again by the loops
//! (ADR 0051): the turn waits, reports each retry as an event and goes on, and
//! the session log shows nothing of the failed attempts.

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn, turn_with};
use rah_api::{
    CancellationToken, Plugin, PluginError, PluginManifest, Scope, async_trait,
    kernel::MountOptions,
    model::{MODEL, ModelError, ModelProvider, ModelStream},
};
use rah_protocol::{AgentEvent, ModelEvent, ModelRequest};
use serde_json::json;

/// Cannot take the first `failures` requests, then answers `ok`.
struct Flaky {
    failures: u32,
    opened: AtomicU32,
    error: ModelError,
}

#[async_trait]
impl ModelProvider for Flaky {
    async fn stream(
        &self,
        _request: ModelRequest,
        _cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        if self.opened.fetch_add(1, Ordering::SeqCst) < self.failures {
            return Err(self.error.clone());
        }
        Ok(Box::pin(futures::stream::iter(vec![
            Ok(ModelEvent::TextDelta { text: "ok".into() }),
            Ok(ModelEvent::Completed),
        ])))
    }
}

struct FlakyPlugin(PluginManifest, Arc<Flaky>);

#[async_trait]
impl Plugin for FlakyPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(MODEL, self.1.clone() as Arc<dyn ModelProvider>)?;
        Ok(())
    }
}

async fn boot_with(
    name: &str,
    loop_kind: &str,
    failures: u32,
    error: ModelError,
    retries: serde_json::Value,
) -> (rah_api::Kernel, TempDir, Arc<Flaky>) {
    let dir = TempDir::new(name);
    let mut distribution = headless(&dir);
    for spec in &mut distribution.plugin {
        if spec.kind == "loop-react" {
            spec.kind = loop_kind.into();
        }
    }
    distribution.set_config(loop_kind, "retry_delay_ms", json!(5));
    distribution.set_config(loop_kind, "retries", retries);
    let kernel = kernel(&distribution).await;
    kernel.unmount(&"model-mock".into()).await.unwrap();
    let flaky = Arc::new(Flaky {
        failures,
        opened: AtomicU32::new(0),
        error,
    });
    kernel
        .mount(
            Arc::new(FlakyPlugin(
                PluginManifest::new("flaky", "1").provides(["model"]),
                flaky.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    (kernel, dir, flaky)
}

fn unavailable() -> ModelError {
    ModelError::Unavailable {
        message: "HTTP response status 503".into(),
        retry_after: None,
    }
}

#[tokio::test]
async fn both_loops_retry_and_report_each_wait() {
    for loop_kind in ["loop-react", "loop-plan"] {
        let (kernel, _dir, flaky) = boot_with(
            &format!("retry-{loop_kind}"),
            loop_kind,
            2,
            unavailable(),
            json!(5),
        )
        .await;
        let handle = kernel.handle();
        let session = new_session(&handle).await;
        let events = turn(&handle, &session, "hello").await;
        assert!(
            matches!(events.last(), Some(AgentEvent::Completed { .. })),
            "{loop_kind}: {events:?}"
        );
        assert!(reply(&events).contains("ok"), "{loop_kind}");
        let retries: Vec<(u32, u32, String)> = events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ModelRetrying {
                    attempt,
                    retries,
                    reason,
                    ..
                } => Some((*attempt, *retries, reason.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(retries.len(), 2, "{loop_kind}");
        assert_eq!((retries[0].0, retries[0].1), (1, 5));
        assert_eq!(retries[1].0, 2);
        assert!(retries[0].2.contains("503"), "{}", retries[0].2);
        // The first model request of the turn took three attempts.
        assert!(flaky.opened.load(Ordering::SeqCst) >= 3);
        // The log does not show the failed attempts.
        assert!(
            !entries(&handle, &session)
                .await
                .iter()
                .any(|entry| entry.contains("retry")),
            "{loop_kind}"
        );
    }
}

#[tokio::test]
async fn a_rejection_is_not_retried_and_retries_can_be_turned_off() {
    let (kernel, _dir, flaky) = boot_with(
        "retry-rejected",
        "loop-react",
        1,
        ModelError::Rejected("HTTP response status 400".into()),
        json!(5),
    )
    .await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
    assert!(!kinds(&events).contains(&"retry"));
    assert_eq!(flaky.opened.load(Ordering::SeqCst), 1);

    let (kernel, _dir, flaky) =
        boot_with("retry-off", "loop-react", 1, unavailable(), json!(0)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    match events.last() {
        Some(AgentEvent::Failed { message, .. }) => {
            assert!(message.contains("temporarily unavailable"), "{message}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(flaky.opened.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn the_last_error_ends_the_turn_when_the_retries_are_used_up() {
    let (kernel, _dir, flaky) = boot_with(
        "retry-exhausted",
        "loop-react",
        100,
        unavailable(),
        json!(2),
    )
    .await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
    assert_eq!(kinds(&events).iter().filter(|k| **k == "retry").count(), 2);
    assert_eq!(flaky.opened.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn cancelling_ends_a_turn_that_is_waiting_to_retry() {
    let dir = TempDir::new("retry-cancel");
    let mut distribution = headless(&dir);
    distribution.set_config("loop-react", "retry_delay_ms", json!(60_000));
    let kernel = kernel(&distribution).await;
    kernel.unmount(&"model-mock".into()).await.unwrap();
    let flaky = Arc::new(Flaky {
        failures: 100,
        opened: AtomicU32::new(0),
        error: unavailable(),
    });
    kernel
        .mount(
            Arc::new(FlakyPlugin(
                PluginManifest::new("flaky", "1").provides(["model"]),
                flaky,
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let cancel = CancellationToken::new();
    let stopper = {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            cancel.cancel();
        })
    };
    let started = std::time::Instant::now();
    let events = turn_with(&handle, &session, "hello", cancel).await;
    stopper.await.unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert!(
        matches!(
            events.last(),
            Some(AgentEvent::Cancelled { .. } | AgentEvent::Failed { .. })
        ),
        "{events:?}"
    );
}
