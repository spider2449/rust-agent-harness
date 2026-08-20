use serde_json::{Value, json};

use crate::CodexAdapterError;

pub(crate) const INITIALIZE: &str = "initialize";
pub(crate) const INITIALIZED: &str = "initialized";

#[derive(Clone, Debug)]
pub(crate) enum Incoming {
    Response { id: u64, result: Value },
    ErrorResponse { id: u64, code: i64, message: String },
    Notification { method: String, params: Value },
    Request { id: Value, method: String },
}

pub(crate) fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "id": id, "method": method, "params": params })
}

pub(crate) fn notification(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params })
}

pub(crate) fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "id": id, "error": { "code": code, "message": message } })
}

pub(crate) fn parse(value: Value) -> Result<Incoming, CodexAdapterError> {
    let object = value
        .as_object()
        .ok_or_else(|| violation("message must be a JSON object"))?;
    if let Some(id) = object.get("id") {
        if let Some(error) = object.get("error") {
            let id = id
                .as_u64()
                .ok_or_else(|| violation("response ID must be an unsigned integer"))?;
            return Ok(Incoming::ErrorResponse {
                id,
                code: error.get("code").and_then(Value::as_i64).unwrap_or(-1),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("missing JSON-RPC error message")
                    .to_owned(),
            });
        }
        if let Some(result) = object.get("result") {
            let id = id
                .as_u64()
                .ok_or_else(|| violation("response ID must be an unsigned integer"))?;
            return Ok(Incoming::Response {
                id,
                result: result.clone(),
            });
        }
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| violation("message with ID must be a response or server request"))?;
        return Ok(Incoming::Request {
            id: id.clone(),
            method: method.to_owned(),
        });
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| violation("notification is missing a method"))?;
    Ok(Incoming::Notification {
        method: method.to_owned(),
        params: object.get("params").cloned().unwrap_or(Value::Null),
    })
}

fn violation(message: &str) -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Incoming, parse};

    #[test]
    fn parses_correlated_response() {
        let Incoming::Response { id, result } =
            parse(json!({"id": 7, "result": {"ok": true}})).expect("valid response")
        else {
            panic!("expected response");
        };
        assert_eq!(id, 7);
        assert_eq!(result, json!({"ok": true}));
    }
}
