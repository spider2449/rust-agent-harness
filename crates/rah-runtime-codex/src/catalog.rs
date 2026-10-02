use serde_json::{Value, json};

use crate::{CodexAdapterError, connection::AppServerConnection};

const MAX_MODELS: usize = 100;
const MAX_MODEL_BYTES: usize = 256;

use rah_runtime::ModelCatalog;

pub(crate) async fn discover(
    connection: &AppServerConnection,
) -> Result<ModelCatalog, CodexAdapterError> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        connection.request(
            "model/list",
            json!({"limit": MAX_MODELS, "includeHidden": true}),
        ),
    )
    .await
    .map_err(|_| CodexAdapterError::CatalogDeadline)??;
    parse_catalog(result)
}

fn invalid_catalog() -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: "model catalog result is malformed, incomplete, or exceeds bounds".to_owned(),
    }
}

fn parse_catalog(result: Value) -> Result<ModelCatalog, CodexAdapterError> {
    // One observation cannot establish absence in an incomplete page.
    if result.get("nextCursor") != Some(&Value::Null) {
        return Err(invalid_catalog());
    }
    let data = result
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(invalid_catalog)?;
    if data.len() > MAX_MODELS {
        return Err(invalid_catalog());
    }
    let mut models = Vec::with_capacity(data.len());
    for entry in data {
        // `model` is the inference selector; `id` is separately catalog metadata.
        let model = entry
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(invalid_catalog)?;
        if model.is_empty()
            || model.len() > MAX_MODEL_BYTES
            || !model
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-/".contains(&byte))
        {
            return Err(invalid_catalog());
        }
        if !models.iter().any(|existing| existing == model) {
            models.push(model.to_owned());
        }
    }
    Ok(ModelCatalog { models })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_tests::adapter_source;
    use crate::{
        CodexModelConfig, CodexModelProvider, CodexModelSelection, CodexRuntime,
        test_support::fake_transport,
    };
    use rah_runtime::ModelPreflight;

    #[test]
    fn parses_selector_without_trusting_display_metadata() {
        let catalog = parse_catalog(json!({"data":[{"id":"opaque", "model":"example-model", "displayName":"ignored"}],"nextCursor":null})).unwrap();
        assert_eq!(catalog.models, ["example-model"]);
    }

    #[test]
    fn rejects_unexpected_incomplete_and_unbounded_results() {
        for result in [
            json!({}),
            json!({"data":{},"nextCursor":null}),
            json!({"data":[],"nextCursor":"more"}),
            json!({"data":[{"id":"model"}],"nextCursor":null}),
            json!({"data":[{"model":"token=secret"}],"nextCursor":null}),
            json!({"data":vec![json!({"model":"example"});101],"nextCursor":null}),
        ] {
            assert!(parse_catalog(result).is_err());
        }
    }

    #[tokio::test]
    async fn selected_present_absent_and_rpc_failure_without_inference() {
        for (models, fails) in [
            (vec!["example-model"], false),
            (vec!["alternative"], false),
            (vec![], true),
        ] {
            let advertised = models.contains(&"example-model");
            let (transport, mut peer) = fake_transport();
            let peer_task = tokio::spawn(async move {
                peer.respond("initialize", json!({})).await;
                peer.expect_notification("initialized").await;
                let request = peer.next_sent().await;
                assert_eq!(request["method"], "model/list");
                assert_eq!(request["params"], json!({"limit":100,"includeHidden":true}));
                if fails {
                    peer.send(json!({"id":request["id"],"error":{"code":-32000,"message":"sensitive backend detail"}}));
                } else {
                    peer.send(json!({"id":request["id"],"result":{"data":models.iter().map(|model|json!({"model":model})).collect::<Vec<_>>(),"nextCursor":null}}));
                }
                // Shutdown sends no thread/start or turn/start message.
                while !peer.stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    peer.assert_no_outbound_request();
                    tokio::task::yield_now().await;
                }
            });
            let config = CodexModelConfig::Explicit(
                CodexModelSelection::new("example-model", CodexModelProvider::OpenAi).unwrap(),
            );
            let runtime = CodexRuntime::from_transport_with_model_config(transport, config)
                .await
                .unwrap();
            let result = runtime.preflight_selected_model().await;
            if fails {
                let failure = result.unwrap_err();
                assert!(
                    matches!(adapter_source(&failure), CodexAdapterError::JsonRpc { code: -32000, message } if message == "sensitive backend detail")
                );
                assert_eq!(failure.diagnostic().rpc_code, Some(-32000));
                assert_eq!(
                    failure.diagnostic().operation,
                    rah_protocol::RuntimeOperation::ModelDiscovery
                );
                assert!(
                    !serde_json::to_string(failure.diagnostic())
                        .unwrap()
                        .contains("sensitive")
                );
                assert!(!format!("{failure:?} {failure}").contains("sensitive"));
            } else if advertised {
                assert_eq!(
                    result.unwrap(),
                    ModelPreflight::Advertised(ModelCatalog {
                        models: vec!["example-model".into()]
                    })
                );
            } else {
                assert_eq!(
                    result.unwrap(),
                    ModelPreflight::NotAdvertised(ModelCatalog {
                        models: vec!["alternative".into()]
                    })
                );
            }
            runtime.shutdown().await.unwrap();
            peer_task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn malformed_result_and_uncorrelated_response_do_not_validate_model() {
        for correlated in [true, false] {
            let (transport, mut peer) = fake_transport();
            let task = tokio::spawn(async move {
                peer.respond("initialize", json!({})).await;
                peer.expect_notification("initialized").await;
                let request = peer.next_sent().await;
                assert_eq!(request["method"], "model/list");
                let id = request["id"].as_u64().unwrap() + if correlated { 0 } else { 100 };
                peer.send(json!({"id": id, "result": {"unexpected":true}}));
                while !peer.stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    peer.assert_no_outbound_request();
                    tokio::task::yield_now().await;
                }
            });
            let config = CodexModelConfig::Explicit(
                CodexModelSelection::new("example", CodexModelProvider::OpenAi).unwrap(),
            );
            let runtime = CodexRuntime::from_transport_with_model_config(transport, config)
                .await
                .unwrap();
            let failure = runtime.preflight_selected_model().await.unwrap_err();
            assert!(matches!(
                adapter_source(&failure),
                CodexAdapterError::ProtocolViolation { .. }
            ));
            let _ = runtime.shutdown().await;
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn inherit_and_other_providers_send_no_catalog_or_inference_probe() {
        for config in [
            CodexModelConfig::Inherit,
            CodexModelConfig::Explicit(
                CodexModelSelection::new("local", CodexModelProvider::Ollama).unwrap(),
            ),
        ] {
            let (transport, mut peer) = fake_transport();
            let task = tokio::spawn(async move {
                peer.respond("initialize", json!({})).await;
                peer.expect_notification("initialized").await;
                while !peer.stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    peer.assert_no_outbound_request();
                    tokio::task::yield_now().await;
                }
            });
            let runtime = CodexRuntime::from_transport_with_model_config(transport, config)
                .await
                .unwrap();
            assert_eq!(
                runtime.preflight_selected_model().await.unwrap(),
                ModelPreflight::NotChecked
            );
            runtime.shutdown().await.unwrap();
            task.await.unwrap();
        }
    }
}
