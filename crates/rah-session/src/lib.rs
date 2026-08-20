//! Provider-neutral session state for RAH.

use std::collections::BTreeMap;

use rah_protocol::{Message, SessionId, ToolOutput};
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod store;

pub use store::{MemorySessionStore, SessionStore, SessionStoreError};

/// Durable state associated with one agent session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Session {
    /// Stable identifier used to load and update the session.
    pub id: SessionId,
    /// Current lifecycle state.
    pub status: SessionStatus,
    /// Provider-neutral conversation and tool context.
    pub context: AgentContext,
}

/// Lifecycle state of an agent session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// The runtime is actively processing the session.
    Running,
    /// Execution is paused pending an external approval decision.
    WaitingApproval,
    /// The session finished successfully.
    Completed,
    /// The session was cancelled before completion.
    Cancelled,
    /// The session terminated because of an error.
    Failed,
}

/// Context retained across operations in a session.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentContext {
    /// Ordered conversation history.
    pub messages: Vec<Message>,
    /// Ordered outputs returned by executed tools.
    pub tool_results: Vec<ToolOutput>,
    /// Host-defined, provider-neutral structured metadata.
    pub metadata: BTreeMap<String, Value>,
}

#[cfg(test)]
mod tests {
    use rah_protocol::{Message, MessageRole, SessionId, ToolContent, ToolOutput};

    use super::{AgentContext, Session, SessionStatus};

    #[test]
    fn session_state_round_trips_through_json() {
        let session = Session {
            id: SessionId::new(),
            status: SessionStatus::WaitingApproval,
            context: AgentContext {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: "inspect the repository".to_owned(),
                }],
                tool_results: vec![ToolOutput {
                    content: vec![ToolContent::Text("complete".to_owned())],
                    is_error: false,
                }],
                metadata: [("branch".to_owned(), serde_json::json!("main"))]
                    .into_iter()
                    .collect(),
            },
        };

        let encoded = serde_json::to_string(&session).expect("session should serialize");
        let decoded: Session = serde_json::from_str(&encoded).expect("session should deserialize");

        assert_eq!(decoded, session);
    }

    #[test]
    fn agent_context_defaults_to_empty_history() {
        let context = AgentContext::default();

        assert!(context.messages.is_empty());
        assert!(context.tool_results.is_empty());
        assert!(context.metadata.is_empty());
    }
}
