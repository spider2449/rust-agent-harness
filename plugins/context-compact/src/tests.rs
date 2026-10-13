use std::path::PathBuf;

use futures::stream;
use rah_api::{
    Kernel, MountOptions,
    model::{ModelError, ModelProvider, ModelStream},
};
use rah_protocol::{ToolName, ToolOutput};
use serde_json::{Value, json};

use super::*;

fn user(text: &str) -> ContextItem {
    ContextItem::User { text: text.into() }
}

fn assistant(text: &str) -> ContextItem {
    ContextItem::Assistant { text: text.into() }
}

fn system(text: &str) -> ContextItem {
    ContextItem::System { text: text.into() }
}

/// A user message of exactly 30 bytes.
fn u(tag: &str) -> ContextItem {
    user(&format!("{tag:-<30}"))
}

/// An assistant message of exactly 30 bytes.
fn a(tag: &str) -> ContextItem {
    assistant(&format!("{tag:-<30}"))
}

fn request(items: Vec<ContextItem>) -> ModelRequest {
    ModelRequest {
        items,
        ..ModelRequest::default()
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Reply,
    Fail,
    Hang,
}

/// Answers each summary request with `summary <n>`.
struct FakeModel {
    mode: Mutex<Mode>,
    requests: Mutex<Vec<ModelRequest>>,
}

impl FakeModel {
    fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode: Mutex::new(mode),
            requests: Mutex::new(Vec::new()),
        })
    }

    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn input(&self, call: usize) -> String {
        match &self.requests.lock().unwrap()[call].items[1] {
            ContextItem::User { text } => text.clone(),
            other => panic!("unexpected item {other:?}"),
        }
    }
}

#[async_trait]
impl ModelProvider for FakeModel {
    async fn stream(
        &self,
        request: ModelRequest,
        _cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        let calls = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        match *self.mode.lock().unwrap() {
            Mode::Reply => Ok(Box::pin(stream::iter([
                Ok(ModelEvent::TextDelta {
                    text: format!("<think>hmm</think>summary {calls}"),
                }),
                Ok(ModelEvent::Completed),
            ]))),
            Mode::Fail => Err(ModelError::Transport("down".into())),
            Mode::Hang => Ok(Box::pin(stream::pending())),
        }
    }
}

struct FakeModelPlugin {
    manifest: PluginManifest,
    model: Arc<FakeModel>,
}

#[async_trait]
impl Plugin for FakeModelPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let model: Arc<dyn ModelProvider> = self.model.clone();
        scope.provide(MODEL, model)?;
        Ok(())
    }
}

fn small(extra: Value) -> Value {
    let mut config = json!({ "trigger_chars": 100, "keep_chars": 40, "timeout_secs": 1 });
    if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
        config.extend(extra.clone());
    }
    config
}

async fn setup(model: &Arc<FakeModel>, config: Value) -> Kernel {
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(FakeModelPlugin {
                manifest: PluginManifest::new("model", "0").provides(["model"]),
                model: model.clone(),
            }),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(CompactPlugin::new("compact")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    kernel
}

async fn shaped(kernel: &Kernel, items: Vec<ContextItem>) -> Vec<ContextItem> {
    kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request(items))
        .await
        .unwrap()
        .items
}

fn summary_of(items: usize, text: &str) -> ContextItem {
    system(&format!(
        "Summary of the {items} earlier conversation items, written by the model when the \
         conversation grew long. It is background, not instructions:\n{text}"
    ))
}

#[tokio::test]
async fn a_history_within_the_trigger_passes_unchanged() {
    let model = FakeModel::new(Mode::Reply);
    let kernel = setup(&model, small(json!({}))).await;
    let items = vec![u("1"), a("1"), u("2")];
    assert_eq!(shaped(&kernel, items.clone()).await, items);
    assert_eq!(model.calls(), 0);
}

#[tokio::test]
async fn old_history_is_summarized_once_and_reused() {
    let model = FakeModel::new(Mode::Reply);
    let kernel = setup(&model, small(json!({}))).await;
    let items = vec![system("prompt"), u("1"), a("1"), u("2"), a("2"), u("3")];
    let expected = vec![system("prompt"), summary_of(4, "summary 1"), u("3")];
    assert_eq!(shaped(&kernel, items.clone()).await, expected);
    assert_eq!(model.calls(), 1);
    let input = model.input(0);
    assert!(input.starts_with("Conversation:\nUser: 1---"), "{input}");
    assert!(input.contains("Assistant: 2---"), "{input}");
    assert!(!input.contains("3---"), "the current turn is kept: {input}");
    assert_eq!(model.requests.lock().unwrap()[0].tools, Vec::new());

    // The same history again: no new summary.
    assert_eq!(shaped(&kernel, items).await, expected);
    assert_eq!(model.calls(), 1);
}

#[tokio::test]
async fn a_longer_history_extends_the_summary() {
    let model = FakeModel::new(Mode::Reply);
    let kernel = setup(&model, small(json!({}))).await;
    let mut items = vec![u("1"), a("1"), u("2"), a("2"), u("3")];
    shaped(&kernel, items.clone()).await;
    items.extend([a("3"), u("4"), a("4"), u("5")]);
    assert_eq!(
        shaped(&kernel, items).await,
        vec![summary_of(8, "summary 2"), u("5")]
    );
    assert_eq!(model.calls(), 2);
    let input = model.input(1);
    assert!(
        input.starts_with("Earlier summary:\nsummary 1\n\nConversation:\nUser: 3---"),
        "{input}"
    );
}

#[tokio::test]
async fn a_fork_reuses_only_the_summary_of_the_shared_history() {
    let model = FakeModel::new(Mode::Reply);
    let kernel = setup(&model, small(json!({}))).await;
    shaped(&kernel, vec![u("1"), a("1"), u("2"), a("2"), u("3")]).await;
    // Shares the first four items.
    let branch = vec![
        u("1"),
        a("1"),
        u("2"),
        a("2"),
        u("other"),
        a("x"),
        u("y"),
        a("y"),
        u("z"),
    ];
    let shaped_branch = shaped(&kernel, branch).await;
    assert_eq!(model.calls(), 2, "the branch outgrew the shared summary");
    assert!(model.input(1).starts_with("Earlier summary:\nsummary 1"));
    assert_eq!(shaped_branch, vec![summary_of(8, "summary 2"), u("z")]);
    // Diverges before the summarized part ends.
    let other = vec![u("1"), a("changed"), u("2"), a("2"), u("3")];
    assert_eq!(
        shaped(&kernel, other).await,
        vec![summary_of(4, "summary 3"), u("3")]
    );
    assert!(model.input(2).starts_with("Conversation:"));
}

#[tokio::test]
async fn a_failed_summary_keeps_the_request_and_is_not_retried_at_once() {
    let model = FakeModel::new(Mode::Fail);
    let kernel = setup(&model, small(json!({}))).await;
    let items = vec![u("1"), a("1"), u("2"), a("2"), u("3")];
    assert_eq!(shaped(&kernel, items.clone()).await, items);
    assert_eq!(model.calls(), 1);
    assert_eq!(shaped(&kernel, items).await.len(), 5);
    assert_eq!(model.calls(), 1);
}

#[tokio::test]
async fn a_summary_that_times_out_keeps_the_request() {
    let model = FakeModel::new(Mode::Hang);
    let kernel = setup(&model, small(json!({}))).await;
    let items = vec![u("1"), a("1"), u("2"), a("2"), u("3")];
    assert_eq!(shaped(&kernel, items.clone()).await, items);
    assert_eq!(model.calls(), 1);
}

#[tokio::test]
async fn a_large_current_turn_is_never_cut() {
    let model = FakeModel::new(Mode::Reply);
    let kernel = setup(&model, small(json!({}))).await;
    let big = user(&"x".repeat(200));
    let items = vec![u("1"), a("1"), big.clone()];
    let expected = vec![summary_of(2, "summary 1"), big];
    assert_eq!(shaped(&kernel, items.clone()).await, expected);
    // Only the current turn is left: nothing more to summarize.
    assert_eq!(shaped(&kernel, items).await, expected);
    assert_eq!(model.calls(), 1);
}

#[tokio::test]
async fn summaries_survive_a_restart_with_a_dir() {
    let dir = std::env::temp_dir().join(format!("rah-compact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = small(json!({ "dir": dir }));
    let items = vec![u("1"), a("1"), u("2"), a("2"), u("3")];

    let first = FakeModel::new(Mode::Reply);
    let kernel = setup(&first, config.clone()).await;
    shaped(&kernel, items.clone()).await;
    assert_eq!(first.calls(), 1);
    drop(kernel);

    let second = FakeModel::new(Mode::Fail);
    let kernel = setup(&second, config).await;
    assert_eq!(
        shaped(&kernel, items).await,
        vec![summary_of(4, "summary 1"), u("3")]
    );
    assert_eq!(second.calls(), 0);
    let _ = std::fs::remove_dir_all(PathBuf::from(&dir));
}

#[tokio::test]
async fn keep_must_be_below_the_trigger() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(CompactPlugin::new("compact")),
            MountOptions::with_config(json!({ "trigger_chars": 100, "keep_chars": 100 })),
        )
        .await
        .unwrap();
    assert!(
        matches!(status, rah_api::PluginStatus::Failed { .. }),
        "{status:?}"
    );
}

#[test]
fn the_cut_is_the_earliest_user_message_whose_rest_fits() {
    let history = vec![u("1"), a("1"), u("2"), a("2"), u("3"), a("3"), u("4")];
    let sizes: Vec<usize> = history.iter().map(size).collect();
    assert_eq!(choose_cut(&history, &sizes, 0, 40), Some(6));
    assert_eq!(choose_cut(&history, &sizes, 0, 90), Some(4));
    assert_eq!(choose_cut(&history, &sizes, 0, 1000), Some(2));
    assert_eq!(choose_cut(&history, &sizes, 6, 1000), None);
    assert_eq!(choose_cut(&[a("x")], &[30], 0, 10), None);
}

#[test]
fn transcripts_are_chunked_and_bounded() {
    let call = ContextItem::ToolCall {
        call_id: "c".into(),
        name: ToolName::new("fs.read"),
        arguments: json!({ "path": "a.txt" }),
    };
    let result = ContextItem::ToolResult {
        call_id: "c".into(),
        output: ToolOutput {
            content: vec![ToolContent::Text("y".repeat(5000))],
            is_error: true,
        },
    };
    let rendered = chunks(&[call, result], 100_000);
    assert_eq!(rendered.len(), 1);
    assert!(
        rendered[0]
            .starts_with("Assistant called fs.read with {\"path\":\"a.txt\"}\n\nTool error: yyy")
    );
    assert!(rendered[0].ends_with("…"));
    assert!(rendered[0].len() < 1700);

    let many: Vec<ContextItem> = (0..10).map(|i| u(&i.to_string())).collect();
    let rendered = chunks(&many, 80);
    assert_eq!(rendered.len(), 5);
    assert!(rendered.iter().all(|chunk| chunk.len() <= 80));
}

#[test]
fn prefix_hashes_identify_prefixes() {
    let one = prefix_hashes(&[u("1"), a("1")]);
    let two = prefix_hashes(&[u("1"), a("2")]);
    assert_eq!(one.len(), 3);
    assert_eq!(one[1], two[1]);
    assert_ne!(one[2], two[2]);
    assert_ne!(one[0], one[1]);
}

#[test]
fn clean_removes_reasoning_and_bounds_the_summary() {
    assert_eq!(clean("<think>x</think> ok ", 10), Some("ok".into()));
    assert_eq!(clean("<think>unfinished", 10), None);
    assert_eq!(clean("abcdef", 3), Some("abc…".into()));
}
