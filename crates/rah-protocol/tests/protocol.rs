use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, ApprovalId, ApprovalRequest, ContextItem, Message,
    MessageRole, ModelEvent, ModelRequest, ModelRequestId, ModelUsage, PermissionLevel,
    RAH_PROTOCOL_VERSION, RequestId, SessionId, ToolCall, ToolCallId, ToolContent, ToolDefinition,
    ToolInput, ToolName, ToolOutput,
};
use serde_json::{Value, json};

fn message(role: MessageRole, content: &str) -> Message {
    Message {
        role,
        content: content.to_owned(),
    }
}

fn tool_call() -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new("example.tool"),
        input: ToolInput(json!({
            "nested": {"enabled": true},
            "items": [1, "two", null]
        })),
    }
}

fn tool_output() -> ToolOutput {
    ToolOutput {
        content: vec![
            ToolContent::Text("done".to_owned()),
            ToolContent::Json(json!({"count": 1})),
        ],
        is_error: false,
    }
}

#[test]
fn representative_events_round_trip_through_serde() {
    let session_id = SessionId::new();
    let request_id = RequestId::new();
    let model_request_id = ModelRequestId::new();
    let call = tool_call();
    let call_id = call.id.clone();
    let output = tool_output();
    let approval = ApprovalRequest {
        id: ApprovalId::new(),
        tool_call: call.clone(),
        reason: "execution requires approval".to_owned(),
    };

    let events = vec![
        AgentEvent::Started {
            session_id: session_id.clone(),
            request_id,
        },
        AgentEvent::ModelRequestStarted {
            session_id: session_id.clone(),
            model_request_id: model_request_id.clone(),
        },
        AgentEvent::ModelReasoningStarted {
            session_id: session_id.clone(),
            model_request_id: model_request_id.clone(),
        },
        AgentEvent::ModelDelta {
            session_id: session_id.clone(),
            model_request_id,
            delta: "partial".to_owned(),
        },
        AgentEvent::ToolRequested {
            session_id: session_id.clone(),
            tool_call: call,
        },
        AgentEvent::ToolStarted {
            session_id: session_id.clone(),
            tool_call_id: call_id.clone(),
        },
        AgentEvent::ToolFinished {
            session_id: session_id.clone(),
            tool_call_id: call_id,
            output,
        },
        AgentEvent::ApprovalRequired {
            session_id: session_id.clone(),
            request: approval,
        },
        AgentEvent::Completed {
            session_id: session_id.clone(),
            output: AgentOutput {
                message: message(MessageRole::Assistant, "complete"),
            },
        },
        AgentEvent::Failed {
            session_id: session_id.clone(),
            code: AgentErrorCode::Internal,
            message: "failed".to_owned(),
        },
        AgentEvent::Cancelled { session_id },
    ];

    for event in events {
        let serialized = serde_json::to_string(&event).expect("event should serialize");
        let deserialized = serde_json::from_str(&serialized).expect("event should deserialize");

        assert_eq!(event, deserialized);
    }
}

#[test]
fn tool_calls_accept_arbitrary_json_arguments() {
    let call = tool_call();
    let serialized = serde_json::to_value(&call).expect("tool call should serialize");
    let deserialized: ToolCall =
        serde_json::from_value(serialized).expect("tool call should deserialize");

    assert_eq!(call, deserialized);
}

#[test]
fn protocol_messages_require_no_provider_specific_fields() {
    let request = ModelRequest {
        model: None,
        items: vec![ContextItem::User {
            text: "hello".into(),
        }],
        tools: Vec::new(),
    };
    let definition = ToolDefinition {
        name: ToolName::new("example.tool"),
        description: "Example tool".to_owned(),
        input_schema: json!({"type": "object"}),
        permission: PermissionLevel::None,
    };

    for value in [
        serde_json::to_value(request).expect("request should serialize"),
        serde_json::to_value(definition).expect("definition should serialize"),
    ] {
        let object = value
            .as_object()
            .expect("protocol message should serialize as an object");

        assert!(!object.contains_key("provider"));
        assert!(!object.contains_key("codex"));
        assert!(!object.contains_key("openai"));
    }
}

#[test]
fn protocol_version_starts_at_one() {
    assert_eq!(RAH_PROTOCOL_VERSION, 1);
}

#[test]
fn tool_input_preserves_json_value_kinds() {
    let input = ToolInput(json!({"boolean": true, "number": 7, "text": "value"}));
    let ToolInput(value) = input;

    assert!(matches!(value["boolean"], Value::Bool(true)));
    assert!(value["number"].is_number());
    assert!(value["text"].is_string());
}

#[test]
fn usage_omits_unknown_counts_on_the_wire() {
    let usage = ModelUsage {
        input_tokens: Some(12),
        output_tokens: None,
        cached_input_tokens: Some(4),
    };
    let event = ModelEvent::Usage { usage };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(
        value,
        json!({"type": "usage", "usage": {"input_tokens": 12, "cached_input_tokens": 4}})
    );
    assert_eq!(serde_json::from_value::<ModelEvent>(value).unwrap(), event);
    assert!(ModelUsage::default().is_empty());
    assert!(!usage.is_empty());

    let session_id = SessionId::new();
    let model_request_id = ModelRequestId::new();
    let event = AgentEvent::ModelUsage {
        session_id: session_id.clone(),
        model_request_id: model_request_id.clone(),
        usage,
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["type"], "model_usage");
    assert_eq!(value["usage"]["input_tokens"], 12);
    assert_eq!(serde_json::from_value::<AgentEvent>(value).unwrap(), event);
}

#[test]
fn model_retrying_carries_the_wait_and_the_reason() {
    let event = AgentEvent::ModelRetrying {
        session_id: SessionId::new(),
        model_request_id: ModelRequestId::new(),
        attempt: 2,
        retries: 10,
        delay_ms: 4000,
        reason: "the model is temporarily unavailable: HTTP response status 503".into(),
    };
    let value = serde_json::to_value(&event).unwrap();
    assert_eq!(value["type"], "model_retrying");
    assert_eq!(value["attempt"], 2);
    assert_eq!(value["delay_ms"], 4000);
    assert_eq!(serde_json::from_value::<AgentEvent>(value).unwrap(), event);
}
