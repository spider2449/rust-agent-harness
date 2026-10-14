//! `session-titles`: gives sessions a short, meaningful title.
//!
//! After the first completed turn of a session without a title, the plugin
//! asks the `model` service for a title in one tool-free request (without
//! `model.before_request` hooks, so no context, memory or skills are added)
//! and stores it with `session.rename`. Model output is untrusted text: only
//! one cleaned line is kept. If the model fails, the start of the first user
//! message is used instead. A title set in the meantime, for example by the
//! user, is never overwritten, and each session is tried once per process.
//!
//! Configuration: `{ "timeout_secs": 60 }`.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use futures::StreamExt;
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope,
    async_trait,
    model::MODEL,
    session::{SESSION, SessionAppended},
};
use rah_protocol::{
    ContextItem, ModelEvent, ModelRequest, SessionEntry, SessionId, SessionRecord, normalize_title,
};
use serde::Deserialize;

/// How much of the first exchange the model sees.
const EXCERPT_CHARS: usize = 1500;
/// The longest generated title, in characters.
const TITLE_CHARS: usize = 60;
/// The fallback title: the start of the first user message.
const FALLBACK_CHARS: usize = 40;

const INSTRUCTIONS: &str = "You name conversations. Reply with a short title \
(at most 8 words) for the conversation below, in the language the user wrote in. \
Reply with the title only: no quotes, no explanation, no final punctuation.";

pub struct SessionTitlesPlugin {
    manifest: PluginManifest,
}

impl SessionTitlesPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    timeout_secs: Option<u64>,
}

#[async_trait]
impl Plugin for SessionTitlesPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let titler = Arc::new(Titler {
            kernel: scope.kernel(),
            timeout: Duration::from_secs(config.timeout_secs.unwrap_or(60).max(1)),
            tried: Mutex::new(HashSet::new()),
            stop: scope.spawner().cancellation(),
        });
        let spawner = scope.spawner();
        scope.on::<SessionAppended>(move |event| {
            if !matches!(event.entry, SessionEntry::TurnCompleted { .. })
                || !titler.first_try(&event.session)
            {
                return;
            }
            let titler = titler.clone();
            let session = event.session.clone();
            spawner.spawn(async move { titler.title(session).await });
        });
        Ok(())
    }
}

struct Titler {
    kernel: KernelHandle,
    timeout: Duration,
    tried: Mutex<HashSet<SessionId>>,
    stop: CancellationToken,
}

impl Titler {
    fn first_try(&self, session: &SessionId) -> bool {
        self.tried
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session.clone())
    }

    async fn untitled(&self, session: &SessionId) -> bool {
        let Some(sessions) = self.kernel.get(SESSION) else {
            return false;
        };
        matches!(
            sessions.list().await,
            Ok(list) if list.iter().any(|info| info.id == *session && info.title.is_none())
        )
    }

    async fn title(&self, session: SessionId) {
        if !self.untitled(&session).await {
            return;
        }
        let Some(sessions) = self.kernel.get(SESSION) else {
            return;
        };
        let Ok(records) = sessions.read(&session).await else {
            return;
        };
        let Some((input, answer)) = first_exchange(&records) else {
            return;
        };
        let generated = tokio::select! {
            () = self.stop.cancelled() => return,
            generated = tokio::time::timeout(self.timeout, self.generate(&input, &answer)) => {
                generated.ok().flatten()
            }
        };
        let title = generated
            .or_else(|| fallback(&input))
            .and_then(|title| normalize_title(&title));
        // The model call took a while; keep a title set in the meantime.
        if let Some(title) = title {
            let _ = sessions.rename_if_untitled(&session, title).await;
        }
    }

    async fn generate(&self, input: &str, answer: &str) -> Option<String> {
        let model = self.kernel.get(MODEL)?;
        let request = ModelRequest {
            model: None,
            items: vec![
                ContextItem::System {
                    text: INSTRUCTIONS.to_owned(),
                },
                ContextItem::User {
                    text: format!("User: {}\n\nAssistant: {}", excerpt(input), excerpt(answer)),
                },
            ],
            tools: Vec::new(),
        };
        let mut stream = model.stream(request, self.stop.child_token()).await.ok()?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event.ok()? {
                ModelEvent::TextDelta { text: delta } => text.push_str(&delta),
                ModelEvent::Completed => break,
                _ => {}
            }
        }
        clean(&text)
    }
}

/// The first user message and the assistant text of that turn.
fn first_exchange(records: &[SessionRecord]) -> Option<(String, String)> {
    let mut input = None;
    let mut answer = String::new();
    for record in records {
        match &record.entry {
            SessionEntry::TurnStarted { input: text, .. } if input.is_none() => {
                input = Some(text.clone());
            }
            SessionEntry::TurnStarted { .. } => break,
            SessionEntry::AssistantMessage { text, .. } if input.is_some() => {
                answer.push_str(text);
            }
            _ => {}
        }
    }
    input.map(|input| (input, answer))
}

fn excerpt(text: &str) -> String {
    text.chars().take(EXCERPT_CHARS).collect()
}

fn fallback(input: &str) -> Option<String> {
    let line = normalize_title(input)?;
    let mut title: String = line.chars().take(FALLBACK_CHARS).collect();
    if line.chars().count() > FALLBACK_CHARS {
        title.push('…');
    }
    Some(title)
}

/// One title line from untrusted model text.
fn clean(text: &str) -> Option<String> {
    let mut text = text.to_owned();
    // Reasoning that a provider left in the text.
    while let Some(start) = text.find("<think>") {
        match text[start..].find("</think>") {
            Some(end) => text.replace_range(start..start + end + "</think>".len(), ""),
            None => text.truncate(start),
        }
    }
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let line = line.trim_start_matches(['#', '*', '-', ' ']);
    let line = ["Title:", "title:", "標題:", "標題："]
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix))
        .unwrap_or(line);
    // Quotes and final punctuation can wrap each other (`"Title".`).
    let mut line = line.trim();
    loop {
        let trimmed = line
            .trim_matches(['"', '\'', '`', '*', '「', '」', '“', '”'])
            .trim_end_matches(['.', '。', '!', '！', '?', '？', ':', '：'])
            .trim();
        if trimmed == line {
            break;
        }
        line = trimmed;
    }
    let title: String = line.chars().take(TITLE_CHARS).collect();
    normalize_title(&title)
}

impl SessionTitlesPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "timeout_secs": 60
}"#;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::RequestId;

    #[test]
    fn model_text_is_reduced_to_one_clean_line() {
        assert_eq!(
            clean("<think>The user wants…</think>\n\n\"Trip planning to Kyoto\".\nmore").as_deref(),
            Some("Trip planning to Kyoto")
        );
        assert_eq!(
            clean("**Title:** 「京都旅遊規劃」。").as_deref(),
            Some("京都旅遊規劃")
        );
        assert_eq!(clean("<think>never closed").as_deref(), None);
        assert_eq!(clean("  \n ").as_deref(), None);
        assert_eq!(
            clean(&"word ".repeat(40)).map(|t| t.chars().count() <= TITLE_CHARS),
            Some(true)
        );
    }

    #[test]
    fn the_first_exchange_and_the_fallback_come_from_the_records() {
        let turn = RequestId::new();
        let records = [
            SessionRecord {
                seq: 1,
                entry: SessionEntry::TurnStarted {
                    turn: turn.clone(),
                    input: "Help me plan a trip to Kyoto next spring".into(),
                },
            },
            SessionRecord {
                seq: 2,
                entry: SessionEntry::AssistantMessage {
                    turn,
                    text: "Sure.".into(),
                },
            },
            SessionRecord {
                seq: 3,
                entry: SessionEntry::TurnStarted {
                    turn: RequestId::new(),
                    input: "second".into(),
                },
            },
        ];
        assert_eq!(
            first_exchange(&records),
            Some((
                "Help me plan a trip to Kyoto next spring".into(),
                "Sure.".into()
            ))
        );
        assert_eq!(
            fallback("Help me plan a trip to Kyoto next spring please").as_deref(),
            Some("Help me plan a trip to Kyoto next spring…")
        );
        assert_eq!(fallback("short").as_deref(), Some("short"));
    }
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(SessionTitlesPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
