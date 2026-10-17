//! Opening a model request again when the provider could not take it
//! (ADR 0051).
//!
//! Only the opening is retried: [`ModelError::Unavailable`] and
//! [`ModelError::Transport`] returned before any event reached the caller.
//! Nothing has been shown or recorded by then, and a model request has no
//! external effect (a tool call comes only after a completed stream), so it
//! is safe to send again. A stream that fails after it opened is never
//! retried, and neither are rejections, protocol violations or cancellation.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rah_protocol::ModelRequest;
use tokio_util::sync::CancellationToken;

use rah_kernel::KernelHandle;

use crate::{
    hooks::ModelOverflow,
    model::{ModelError, ModelProvider, ModelStream},
};

/// How often and how long to wait before opening a request again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries after the first attempt; 0 never retries.
    pub retries: u32,
    /// The wait before the first retry; it doubles up to `max_gap`.
    pub base: Duration,
    pub max_gap: Duration,
    /// The longest `Retry-After` honored, so a hostile or buggy value cannot
    /// stall a session for hours.
    pub max_retry_after: Duration,
}

/// Retries a provider may be given at most.
pub const MAX_RETRIES: u32 = 100;

impl RetryPolicy {
    /// Ten retries, waiting 2, 4, 8 and then 10 seconds (about 84 seconds in
    /// all, 67 to 101 with jitter), and honoring `Retry-After` up to 15
    /// minutes.
    pub fn standard() -> Self {
        Self::new(10, Duration::from_secs(2))
    }

    /// `retries` retries, the first after `base`, doubling up to five times
    /// `base`.
    pub fn new(retries: u32, base: Duration) -> Self {
        Self {
            retries: retries.min(MAX_RETRIES),
            base,
            max_gap: base.saturating_mul(5),
            max_retry_after: Duration::from_secs(15 * 60),
        }
    }

    /// The policy a loop's `retries` and `retry_delay_ms` settings describe;
    /// unset values keep [`RetryPolicy::standard`].
    pub fn from_settings(retries: Option<u32>, delay_ms: Option<u64>) -> Result<Self, String> {
        let standard = Self::standard();
        if retries.is_some_and(|retries| retries > MAX_RETRIES) {
            return Err(format!("`retries` must be at most {MAX_RETRIES}"));
        }
        if delay_ms == Some(0) {
            return Err("`retry_delay_ms` must be at least 1".into());
        }
        Ok(Self::new(
            retries.unwrap_or(standard.retries),
            delay_ms.map_or(standard.base, Duration::from_millis),
        ))
    }

    /// The wait before retry number `attempt` (1 for the first), without
    /// jitter. The provider's `Retry-After` can only lengthen it.
    pub fn gap(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        let doubling = 1_u32
            .checked_shl(attempt.saturating_sub(1))
            .unwrap_or(u32::MAX);
        let schedule = self.base.saturating_mul(doubling).min(self.max_gap);
        retry_after.map_or(schedule, |asked| {
            schedule.max(asked.min(self.max_retry_after))
        })
    }
}

/// What is about to happen: a retry is waiting.
#[derive(Clone, Copy, Debug)]
pub struct Retrying<'a> {
    /// 1 for the first retry.
    pub attempt: u32,
    pub retries: u32,
    pub delay: Duration,
    pub reason: &'a str,
}

fn retryable(error: &ModelError) -> Option<Option<Duration>> {
    match error {
        ModelError::Unavailable { retry_after, .. } => Some(*retry_after),
        ModelError::Transport(_) => Some(None),
        _ => None,
    }
}

/// Up to ±20% so that sessions that failed together do not return together.
/// The wait never gets shorter than the provider asked.
fn jittered(gap: Duration, floor: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let factor = 0.8 + f64::from(nanos % 1000) / 1000.0 * 0.4;
    gap.mul_f64(factor).max(floor)
}

/// Opens one streamed request, opening it again while the provider cannot
/// take it. `on_retry` is called before each wait. A cancelled `cancel` ends
/// the wait with [`ModelError::Cancelled`]. When the retries are used up, the
/// last error is returned.
///
/// When the provider says the request does not fit its context window
/// ([`ModelError::ContextOverflow`]) and a `kernel` is given, the
/// `model.overflow` hook may return a smaller request (ADR 0052). It is sent
/// once, at once, and does not count as a retry; a request the hook leaves
/// unchanged ends with the overflow error.
pub async fn stream(
    model: &dyn ModelProvider,
    kernel: Option<&KernelHandle>,
    mut request: ModelRequest,
    cancel: &CancellationToken,
    policy: &RetryPolicy,
    mut on_retry: impl FnMut(Retrying<'_>) + Send,
) -> Result<ModelStream, ModelError> {
    let mut attempt = 0;
    let mut shrunk = false;
    loop {
        let error = match model.stream(request.clone(), cancel.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(error) => error,
        };
        if let (ModelError::ContextOverflow(_), Some(kernel), false) = (&error, kernel, shrunk) {
            shrunk = true;
            let smaller = kernel.waterfall::<ModelOverflow>(request.clone()).await;
            if let Ok(smaller) = smaller
                && smaller != request
            {
                request = smaller;
                on_retry(Retrying {
                    attempt: 1,
                    retries: 1,
                    delay: Duration::ZERO,
                    reason: "the request is larger than the model's context window; \
                             sending it again with the history made smaller",
                });
                continue;
            }
            return Err(error);
        }
        let Some(retry_after) = retryable(&error) else {
            return Err(error);
        };
        if attempt >= policy.retries {
            return Err(error);
        }
        attempt += 1;
        let gap = policy.gap(attempt, retry_after);
        let floor = retry_after.map_or(Duration::ZERO, |asked| asked.min(policy.max_retry_after));
        let delay = jittered(gap, floor);
        on_retry(Retrying {
            attempt,
            retries: policy.retries,
            delay,
            reason: &error.to_string(),
        });
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            () = tokio::time::sleep(delay) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_schedule_doubles_to_ten_seconds() {
        let policy = RetryPolicy::standard();
        let gaps: Vec<u64> = (1..=10).map(|n| policy.gap(n, None).as_secs()).collect();
        assert_eq!(gaps, [2, 4, 8, 10, 10, 10, 10, 10, 10, 10]);
        assert_eq!(gaps.iter().sum::<u64>(), 84);
        assert_eq!(policy.gap(400, None), Duration::from_secs(10));
    }

    #[test]
    fn retry_after_only_lengthens_and_is_bounded() {
        let policy = RetryPolicy::standard();
        let second = Some(Duration::from_secs(1));
        assert_eq!(policy.gap(1, second), Duration::from_secs(2));
        assert_eq!(
            policy.gap(1, Some(Duration::from_secs(30))),
            Duration::from_secs(30)
        );
        assert_eq!(
            policy.gap(1, Some(Duration::from_secs(86_400))),
            Duration::from_secs(900)
        );
    }

    #[test]
    fn the_retry_count_is_bounded() {
        assert_eq!(
            RetryPolicy::new(u32::MAX, Duration::from_millis(1)).retries,
            MAX_RETRIES
        );
    }

    #[test]
    fn jitter_stays_within_a_fifth_and_above_the_floor() {
        for _ in 0..50 {
            let delay = jittered(Duration::from_secs(10), Duration::ZERO);
            assert!(delay >= Duration::from_secs(8) && delay <= Duration::from_secs(12));
        }
        let floor = Duration::from_secs(30);
        assert!(jittered(Duration::from_secs(30), floor) >= floor);
    }

    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    };

    use async_trait::async_trait;
    use rah_protocol::{ModelEvent, ModelRequest};

    /// Fails with the scripted errors in order, then opens an empty stream.
    struct Scripted {
        errors: Mutex<Vec<ModelError>>,
        opened: AtomicU32,
    }

    impl Scripted {
        fn new(mut errors: Vec<ModelError>) -> Arc<Self> {
            errors.reverse();
            Arc::new(Self {
                errors: Mutex::new(errors),
                opened: AtomicU32::new(0),
            })
        }
    }

    #[async_trait]
    impl ModelProvider for Scripted {
        async fn stream(
            &self,
            _request: ModelRequest,
            _cancel: CancellationToken,
        ) -> Result<ModelStream, ModelError> {
            self.opened.fetch_add(1, Ordering::SeqCst);
            match self.errors.lock().unwrap().pop() {
                Some(error) => Err(error),
                None => Ok(Box::pin(futures::stream::iter(vec![Ok(
                    ModelEvent::Completed,
                )]))),
            }
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            model: None,
            items: Vec::new(),
            tools: Vec::new(),
        }
    }

    fn unavailable(retry_after: Option<Duration>) -> ModelError {
        ModelError::Unavailable {
            message: "HTTP 503".into(),
            retry_after,
        }
    }

    fn fast(retries: u32) -> RetryPolicy {
        RetryPolicy::new(retries, Duration::from_millis(1))
    }

    #[tokio::test(start_paused = true)]
    async fn unavailable_and_transport_failures_are_opened_again() {
        let model = Scripted::new(vec![
            unavailable(None),
            ModelError::Transport("reset".into()),
            unavailable(Some(Duration::from_secs(30))),
        ]);
        let mut notices = Vec::new();
        let started = tokio::time::Instant::now();
        let opened = stream(
            model.as_ref(),
            None,
            request(),
            &CancellationToken::new(),
            &RetryPolicy::standard(),
            |notice| {
                notices.push((
                    notice.attempt,
                    notice.retries,
                    notice.delay,
                    notice.reason.to_owned(),
                ))
            },
        )
        .await;
        assert!(opened.is_ok());
        assert_eq!(model.opened.load(Ordering::SeqCst), 4);
        assert_eq!(notices.len(), 3);
        assert_eq!(notices[0].0, 1);
        assert_eq!(notices[0].1, 10);
        assert!(notices[0].3.contains("503"), "{}", notices[0].3);
        // The third wait honored the 30 seconds the provider asked for.
        assert!(
            notices[2].2 >= Duration::from_secs(30),
            "{:?}",
            notices[2].2
        );
        assert!(started.elapsed() >= Duration::from_secs(30 + 2));
    }

    #[tokio::test(start_paused = true)]
    async fn rejections_protocol_errors_and_cancellation_are_not_retried() {
        for error in [
            ModelError::Rejected("HTTP 400".into()),
            ModelError::Protocol("bad".into()),
            ModelError::Cancelled,
        ] {
            let model = Scripted::new(vec![error.clone()]);
            let opened = stream(
                model.as_ref(),
                None,
                request(),
                &CancellationToken::new(),
                &fast(5),
                |_| panic!("retried {error:?}"),
            )
            .await;
            assert_eq!(opened.err(), Some(error));
            assert_eq!(model.opened.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_last_error_is_returned_when_the_retries_are_used_up() {
        let model = Scripted::new(vec![unavailable(None); 10]);
        let mut count = 0;
        let opened = stream(
            model.as_ref(),
            None,
            request(),
            &CancellationToken::new(),
            &fast(3),
            |_| count += 1,
        )
        .await;
        assert!(matches!(opened.err(), Some(ModelError::Unavailable { .. })));
        assert_eq!((count, model.opened.load(Ordering::SeqCst)), (3, 4));

        let none = Scripted::new(vec![unavailable(None)]);
        let opened = stream(
            none.as_ref(),
            None,
            request(),
            &CancellationToken::new(),
            &fast(0),
            |_| panic!("retries are off"),
        )
        .await;
        assert!(opened.is_err());
        assert_eq!(none.opened.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_ends_the_wait() {
        let model = Scripted::new(vec![unavailable(Some(Duration::from_secs(600)))]);
        let cancel = CancellationToken::new();
        let waiter = {
            let cancel = cancel.clone();
            tokio::spawn(async move {
                stream(
                    model.as_ref(),
                    None,
                    request(),
                    &cancel,
                    &RetryPolicy::standard(),
                    |_| {},
                )
                .await
                .err()
            })
        };
        tokio::time::sleep(Duration::from_secs(5)).await;
        cancel.cancel();
        assert_eq!(waiter.await.unwrap(), Some(ModelError::Cancelled));
    }

    use crate::hooks::ModelOverflow;
    use rah_kernel::{Kernel, MountOptions, Plugin, PluginError, PluginManifest, Scope};

    struct Shrinker(PluginManifest, bool);

    #[async_trait]
    impl Plugin for Shrinker {
        fn manifest(&self) -> &PluginManifest {
            &self.0
        }

        async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
            let shrink = self.1;
            scope.hook_waterfall::<ModelOverflow, _, _>(0, move |mut request| async move {
                if shrink {
                    request.items.clear();
                }
                request
            });
            Ok(())
        }
    }

    async fn kernel_with(shrink: bool) -> Kernel {
        let kernel = Kernel::new();
        kernel
            .mount(
                Arc::new(Shrinker(PluginManifest::new("shrinker", "1"), shrink)),
                MountOptions::default(),
            )
            .await
            .unwrap();
        kernel
    }

    fn big_request() -> ModelRequest {
        ModelRequest {
            model: None,
            items: vec![rah_protocol::ContextItem::User {
                text: "long".into(),
            }],
            tools: Vec::new(),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_overflow_is_sent_once_more_with_what_the_hook_returns() {
        let kernel = kernel_with(true).await;
        let handle = kernel.handle();
        let model = Scripted::new(vec![ModelError::ContextOverflow("too long".into())]);
        let mut notices = Vec::new();
        let opened = stream(
            model.as_ref(),
            Some(&handle),
            big_request(),
            &CancellationToken::new(),
            &fast(0),
            |notice| notices.push((notice.attempt, notice.delay, notice.reason.to_owned())),
        )
        .await;
        assert!(opened.is_ok());
        assert_eq!(model.opened.load(Ordering::SeqCst), 2);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].1, Duration::ZERO);
        assert!(notices[0].2.contains("context window"), "{}", notices[0].2);
    }

    #[tokio::test(start_paused = true)]
    async fn an_overflow_the_hook_cannot_shrink_ends_the_request() {
        let kernel = kernel_with(false).await;
        let handle = kernel.handle();
        for kernel in [Some(&handle), None] {
            let model = Scripted::new(vec![ModelError::ContextOverflow("too long".into())]);
            let opened = stream(
                model.as_ref(),
                kernel,
                big_request(),
                &CancellationToken::new(),
                &fast(5),
                |_| panic!("nothing to send again"),
            )
            .await;
            assert!(matches!(opened.err(), Some(ModelError::ContextOverflow(_))));
            assert_eq!(model.opened.load(Ordering::SeqCst), 1);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_second_overflow_is_not_shrunk_again() {
        let kernel = kernel_with(true).await;
        let handle = kernel.handle();
        let model = Scripted::new(vec![
            ModelError::ContextOverflow("one".into()),
            ModelError::ContextOverflow("two".into()),
        ]);
        let opened = stream(
            model.as_ref(),
            Some(&handle),
            big_request(),
            &CancellationToken::new(),
            &fast(5),
            |_| {},
        )
        .await;
        assert!(matches!(opened.err(), Some(ModelError::ContextOverflow(m)) if m == "two"));
        assert_eq!(model.opened.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn provider_overflow_messages_are_recognized() {
        use crate::model::is_context_overflow;
        for body in [
            r#"{"error":{"code":"context_length_exceeded","message":"x"}}"#,
            "This model's Maximum Context Length is 8192 tokens",
            r#"{"error":{"type":"exceed_context_size_error"}}"#,
            "the request exceeds the available context size (4096)",
            r#"{"type":"error","error":{"message":"prompt is too long: 250000 tokens > 200000"}}"#,
        ] {
            assert!(is_context_overflow(body), "{body}");
        }
        for body in ["", "rate limit reached", r#"{"error":"invalid api key"}"#] {
            assert!(!is_context_overflow(body), "{body}");
        }
    }
}
