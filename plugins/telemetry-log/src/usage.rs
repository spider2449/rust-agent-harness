//! The `usage` view (ADR 0062): turns, model requests, tokens and tool calls
//! summed from the records this process wrote, shown by the Web UI as a
//! panel, with a button that starts the sums again. Like the records, it
//! holds no message text, tool output or arguments.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
    time::SystemTime,
};

use rah_api::{
    async_trait,
    ui::{ActionOutcome, KeyValue, Node, UiError, UiViews, View, ViewContext, ViewSpec},
};
use serde_json::Value;

use crate::{Record, Tokens, timestamp};

pub(crate) const VIEW: &str = "usage";
const RESET: &str = "reset";
/// The most tools listed by name; the rest are summed as one row.
const MAX_TOOLS: usize = 200;
const OTHER_TOOLS: &str = "(other tools)";

#[derive(Default)]
struct ToolTotals {
    calls: u64,
    errors: u64,
    /// Denied by the policy or rejected by the user.
    not_run: u64,
}

pub(crate) struct Totals {
    since: String,
    turns: BTreeMap<&'static str, u64>,
    model_requests: u64,
    retries: u64,
    tokens: Tokens,
    tools: BTreeMap<String, ToolTotals>,
}

impl Totals {
    pub(crate) fn new() -> Self {
        Self {
            since: timestamp(SystemTime::now()),
            turns: BTreeMap::new(),
            model_requests: 0,
            retries: 0,
            tokens: Tokens::default(),
            tools: BTreeMap::new(),
        }
    }

    pub(crate) fn add(&mut self, record: &Record) {
        match record {
            Record::Turn { outcome, .. } => *self.turns.entry(outcome).or_default() += 1,
            Record::ModelRequest {
                retries, tokens, ..
            } => {
                self.model_requests += 1;
                self.retries += u64::from(*retries);
                self.tokens.sum(tokens);
            }
            Record::Tool {
                tool,
                decision,
                is_error,
                ..
            } => {
                let name = if self.tools.contains_key(tool) || self.tools.len() < MAX_TOOLS {
                    tool.clone()
                } else {
                    OTHER_TOOLS.to_owned()
                };
                let totals = self.tools.entry(name).or_default();
                totals.calls += 1;
                totals.errors += u64::from(*is_error);
                totals.not_run += u64::from(matches!(*decision, "denied" | "rejected"));
            }
        }
    }

    fn view(&self) -> View {
        let turns = |outcome: &str| self.turns.get(outcome).copied().unwrap_or(0);
        let all_turns: u64 = self.turns.values().sum();
        let count =
            |value: Option<u64>| value.map_or_else(|| "not reported".to_owned(), |n| n.to_string());
        let mut tools: Vec<(&String, &ToolTotals)> = self.tools.iter().collect();
        tools.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(b.0)));
        let mut nodes = vec![
            Node::Text {
                text: format!("Since {}, in this process.", self.since),
            },
            Node::KeyValue {
                items: vec![
                    KeyValue::new(
                        "Turns",
                        format!(
                            "{all_turns} ({} completed, {} failed, {} cancelled)",
                            turns("completed"),
                            turns("failed"),
                            turns("cancelled")
                        ),
                    ),
                    KeyValue::new(
                        "Model requests",
                        format!("{} ({} retries)", self.model_requests, self.retries),
                    ),
                    KeyValue::new("Input tokens", count(self.tokens.input_tokens)),
                    KeyValue::new(
                        "Cached input tokens",
                        count(self.tokens.cached_input_tokens),
                    ),
                    KeyValue::new("Output tokens", count(self.tokens.output_tokens)),
                    KeyValue::new(
                        "Tool calls",
                        self.tools
                            .values()
                            .map(|t| t.calls)
                            .sum::<u64>()
                            .to_string(),
                    ),
                ],
            },
        ];
        if !tools.is_empty() {
            nodes.push(Node::Table {
                columns: ["Tool", "Calls", "Errors", "Not run"]
                    .map(str::to_owned)
                    .to_vec(),
                rows: tools
                    .into_iter()
                    .map(|(name, totals)| {
                        vec![
                            name.clone(),
                            totals.calls.to_string(),
                            totals.errors.to_string(),
                            totals.not_run.to_string(),
                        ]
                    })
                    .collect(),
            });
        }
        nodes.push(Node::Button {
            action: RESET.to_owned(),
            label: "Reset".to_owned(),
        });
        View { nodes }
    }
}

/// The `ui-views` provider of one `telemetry-log` instance.
pub(crate) struct UsageViews(pub(crate) Arc<Mutex<Totals>>);

impl UsageViews {
    fn totals(&self) -> std::sync::MutexGuard<'_, Totals> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl UiViews for UsageViews {
    fn views(&self) -> Vec<ViewSpec> {
        vec![ViewSpec::new(VIEW, "Usage")]
    }

    async fn render(&self, view: &str, _cx: &ViewContext) -> Result<View, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        Ok(self.totals().view())
    }

    async fn act(
        &self,
        view: &str,
        action: &str,
        _input: Value,
        _cx: &ViewContext,
    ) -> Result<ActionOutcome, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        if action != RESET {
            return Err(UiError::UnknownAction(action.to_owned()));
        }
        *self.totals() = Totals::new();
        Ok(ActionOutcome {
            message: Some("Usage sums start again from now.".to_owned()),
            rerender: true,
        })
    }
}
