use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Uniquely identifies an agent session.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

#[expect(
    clippy::new_without_default,
    reason = "an identifier has no meaningful default value"
)]
impl SessionId {
    /// Creates a new random session identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Uniquely identifies an agent request.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RequestId(Uuid);

#[expect(
    clippy::new_without_default,
    reason = "an identifier has no meaningful default value"
)]
impl RequestId {
    /// Creates a new random request identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Uniquely identifies a request sent to a model backend.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ModelRequestId(Uuid);

#[expect(
    clippy::new_without_default,
    reason = "an identifier has no meaningful default value"
)]
impl ModelRequestId {
    /// Creates a new random model request identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for ModelRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Uniquely identifies a tool call.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolCallId(Uuid);

#[expect(
    clippy::new_without_default,
    reason = "an identifier has no meaningful default value"
)]
impl ToolCallId {
    /// Creates a new random tool call identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for ToolCallId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Uniquely identifies an approval request.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ApprovalId(Uuid);

#[expect(
    clippy::new_without_default,
    reason = "an identifier has no meaningful default value"
)]
impl ApprovalId {
    /// Creates a new random approval identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for ApprovalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use serde::{Serialize, de::DeserializeOwned};

    use super::{ApprovalId, ModelRequestId, RequestId, SessionId, ToolCallId};

    fn assert_serde_round_trip<T>(value: &T)
    where
        T: Debug + DeserializeOwned + PartialEq + Serialize,
    {
        let serialized = serde_json::to_string(value).expect("identifier should serialize");
        let deserialized =
            serde_json::from_str(&serialized).expect("identifier should deserialize");

        assert_eq!(value, &deserialized);
    }

    #[test]
    fn identifiers_round_trip_through_serde() {
        assert_serde_round_trip(&SessionId::new());
        assert_serde_round_trip(&RequestId::new());
        assert_serde_round_trip(&ModelRequestId::new());
        assert_serde_round_trip(&ToolCallId::new());
        assert_serde_round_trip(&ApprovalId::new());
    }

    #[test]
    fn newly_generated_identifiers_are_distinct() {
        assert_ne!(SessionId::new(), SessionId::new());
        assert_ne!(RequestId::new(), RequestId::new());
        assert_ne!(ModelRequestId::new(), ModelRequestId::new());
        assert_ne!(ToolCallId::new(), ToolCallId::new());
        assert_ne!(ApprovalId::new(), ApprovalId::new());
    }

    #[test]
    fn identifiers_display_as_their_uuid() {
        let session_id = SessionId::new();
        let request_id = RequestId::new();
        let model_request_id = ModelRequestId::new();
        let tool_call_id = ToolCallId::new();
        let approval_id = ApprovalId::new();

        assert_eq!(session_id.to_string(), session_id.0.to_string());
        assert_eq!(request_id.to_string(), request_id.0.to_string());
        assert_eq!(model_request_id.to_string(), model_request_id.0.to_string());
        assert_eq!(tool_call_id.to_string(), tool_call_id.0.to_string());
        assert_eq!(approval_id.to_string(), approval_id.0.to_string());
    }
}
