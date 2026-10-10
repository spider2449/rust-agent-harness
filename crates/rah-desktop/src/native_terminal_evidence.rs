//! Closed process-local terminal evidence; never format errors or event payloads.
use rah_protocol::AgentEvent;
use rah_runtime::{RuntimeEvent, RuntimeFailure};

#[derive(Default)]
pub(crate) struct Counters {
    model_requests: u64,
    completed_tools: u64,
    last_tool_round: Option<u64>,
}

impl Counters {
    pub(crate) fn observe(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::ModelRequestStarted { .. } => {
                self.model_requests = self.model_requests.saturating_add(1);
            }
            AgentEvent::ToolFinished { .. } => {
                self.completed_tools = self.completed_tools.saturating_add(1);
                self.last_tool_round = Some(self.model_requests);
            }
            _ => {}
        }
    }

    pub(crate) fn terminal(&self, event: &RuntimeEvent) -> Option<serde_json::Value> {
        let state = match event.event() {
            AgentEvent::Failed { .. } => "failed",
            AgentEvent::Completed { .. } => "completed",
            AgentEvent::Cancelled { .. } => "cancelled",
            _ => return None,
        };
        let failure = event.failure();
        let (variant, status) = classify(failure);
        Some(serde_json::json!({
            "event": "native_terminal",
            "chat_terminal": state,
            "model_requests": self.model_requests,
            "completed_tools": self.completed_tools,
            "last_model_round": self.model_requests,
            "last_tool_round": self.last_tool_round,
            "model_request_after_last_completed_tool": self.last_tool_round.map(|round| self.model_requests > round),
            "input_history_bytes": null,
            "operation": failure.map(|f| f.diagnostic().operation),
            "kind": failure.map(|f| f.diagnostic().kind),
            "typed_source": variant,
            "http_status": status,
        }))
    }
}

fn classify(failure: Option<&RuntimeFailure>) -> (&'static str, Option<u16>) {
    #[cfg(any(feature = "provider-openai", feature = "provider-llamacpp"))]
    {
        use rah_runtime_openai::OpenAiAdapterError as E;
        use std::error::Error;
        if let Some(source) = failure
            .and_then(|f| f.source())
            .and_then(|s| s.downcast_ref::<E>())
        {
            return match source {
                E::Configuration => ("Configuration", None),
                E::Transport => ("Transport", None),
                E::HttpStatus(status) => ("HttpStatus", Some(*status)),
                E::Api => ("Api", None),
                E::Sse => ("Sse", None),
                E::EventJson => ("EventJson", None),
                E::Protocol => ("Protocol", None),
                E::ToolSchema => ("ToolSchema", None),
                E::ContinuationLimit => ("ContinuationLimit", None),
                E::Cancelled => ("Cancelled", None),
                E::Shutdown => ("Shutdown", None),
            };
        }
    }
    let _ = failure;
    ("unknown_source", None)
}

#[cfg(all(test, any(feature = "provider-openai", feature = "provider-llamacpp")))]
mod tests {
    use super::*;
    use rah_protocol::{AgentErrorCode, RuntimeOperation, SessionId};
    use rah_runtime_openai::OpenAiAdapterError as E;

    #[test]
    fn exact_native_variants() {
        for (source, name, status) in [
            (E::ContinuationLimit, "ContinuationLimit", None),
            (E::Protocol, "Protocol", None),
            (E::Transport, "Transport", None),
            (E::HttpStatus(413), "HttpStatus", Some(413)),
            (E::Sse, "Sse", None),
            (E::EventJson, "EventJson", None),
            (E::Api, "Api", None),
            (E::Cancelled, "Cancelled", None),
            (E::Shutdown, "Shutdown", None),
            (E::Configuration, "Configuration", None),
            (E::ToolSchema, "ToolSchema", None),
        ] {
            assert_eq!(
                classify(Some(&source.into_runtime_failure(RuntimeOperation::Turn))),
                (name, status)
            );
        }
    }

    #[test]
    fn counters_track_observed_rounds_without_payloads() {
        let mut counters = Counters::default();
        counters.observe(&AgentEvent::ModelRequestStarted {
            session_id: SessionId::new(),
            model_request_id: rah_protocol::ModelRequestId::new(),
        });
        counters.observe(&AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: rah_protocol::ToolCallId::new(),
            output: rah_protocol::ToolOutput {
                content: vec![rah_protocol::ToolContent::Text(
                    "PRIVATE content arguments paths".into(),
                )],
                is_error: false,
            },
        });
        counters.observe(&AgentEvent::ModelRequestStarted {
            session_id: SessionId::new(),
            model_request_id: rah_protocol::ModelRequestId::new(),
        });
        let event = RuntimeEvent::failed(
            SessionId::new(),
            AgentErrorCode::Model,
            E::Protocol.into_runtime_failure(RuntimeOperation::Turn),
        );
        let record = counters.terminal(&event).unwrap();
        assert_eq!(record["model_requests"], 2);
        assert_eq!(record["completed_tools"], 1);
        assert_eq!(record["last_tool_round"], 1);
        assert_eq!(record["model_request_after_last_completed_tool"], true);
        assert!(!record.to_string().contains("PRIVATE"));
        counters.model_requests = u64::MAX;
        counters.completed_tools = u64::MAX;
        counters.observe(&AgentEvent::ModelRequestStarted {
            session_id: SessionId::new(),
            model_request_id: rah_protocol::ModelRequestId::new(),
        });
        assert_eq!(counters.model_requests, u64::MAX);
    }

    #[test]
    fn unknown_source_and_payload_never_serialized() {
        let diagnostic = E::Protocol
            .into_runtime_failure(RuntimeOperation::Turn)
            .diagnostic()
            .clone();
        assert_eq!(classify(None), ("unknown_source", None));
        assert_eq!(
            classify(Some(&RuntimeFailure::without_source(diagnostic.clone()))),
            ("unknown_source", None)
        );
        let failure = RuntimeFailure::new(
            diagnostic,
            std::io::Error::other("PRIVATE credential path response"),
        );
        let event = RuntimeEvent::failed(SessionId::new(), AgentErrorCode::Model, failure);
        let mut counters = Counters::default();
        counters.observe(&AgentEvent::ModelDelta {
            session_id: SessionId::new(),
            model_request_id: rah_protocol::ModelRequestId::new(),
            delta: "PRIVATE prompt assistant tool arguments result".into(),
        });
        let record = counters.terminal(&event).unwrap();
        assert_eq!(record["typed_source"], "unknown_source");
        assert!(!record.to_string().contains("PRIVATE"));
        assert!(record["input_history_bytes"].is_null());
    }
}
