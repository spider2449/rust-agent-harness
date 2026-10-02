use rah_protocol::RuntimeDiagnostic;
use rah_runtime::{ModelPreflight, RuntimeFailure};

use serde::Serialize;

use crate::FrontendError;

/// Ephemeral discovery evidence only. No authority or persisted configuration.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelPreflightPresentation {
    pub outcome: &'static str,
    pub selected_model: Option<String>,
    pub advertised_models: Vec<String>,
    pub diagnostic: Option<RuntimeDiagnostic>,
}

pub(crate) fn present(
    selected_model: Option<String>,
    result: Result<ModelPreflight, RuntimeFailure>,
) -> (ModelPreflightState, Result<(), FrontendError>) {
    let failure = result.as_ref().err().cloned();
    let (outcome, advertised_models, diagnostic, gate) = match result {
        Ok(ModelPreflight::NotChecked) => ("not_checked", vec![], None, Ok(())),
        Ok(ModelPreflight::Advertised(catalog)) => {
            ("model_advertised", catalog.models, None, Ok(()))
        }
        Ok(ModelPreflight::NotAdvertised(catalog)) => (
            "model_not_advertised",
            catalog.models,
            None,
            Err(FrontendError::ModelNotAdvertised),
        ),
        Err(error) => (
            "model_catalog_unavailable",
            vec![],
            Some(error.diagnostic().clone()),
            Err(FrontendError::ModelCatalogUnavailable),
        ),
    };
    (
        ModelPreflightState {
            failure,
            presentation: ModelPreflightPresentation {
                outcome,
                selected_model,
                advertised_models,
                diagnostic,
            },
        },
        gate,
    )
}

/// Process-local owner; only presentation is serialized.
#[derive(Clone, Debug)]
pub(crate) struct ModelPreflightState {
    pub presentation: ModelPreflightPresentation,
    pub failure: Option<RuntimeFailure>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_runtime::ModelCatalog;
    use rah_runtime_codex::CodexAdapterError;

    #[test]
    fn advertised_connect_proceeds_and_missing_stops_with_alternatives() {
        let (view, gate) = present(
            Some("selected".into()),
            Ok(ModelPreflight::Advertised(ModelCatalog {
                models: vec!["selected".into()],
            })),
        );
        assert!(gate.is_ok());
        assert_eq!(view.presentation.outcome, "model_advertised");
        let (view, gate) = present(
            Some("missing".into()),
            Ok(ModelPreflight::NotAdvertised(ModelCatalog {
                models: vec!["alternative".into()],
            })),
        );
        assert_eq!(gate, Err(FrontendError::ModelNotAdvertised));
        assert_eq!(view.presentation.selected_model.as_deref(), Some("missing"));
        assert_eq!(view.presentation.advertised_models, ["alternative"]);
    }

    #[test]
    fn inherit_does_not_claim_validation_and_catalog_failure_is_neutral() {
        let (view, gate) = present(None, Ok(ModelPreflight::NotChecked));
        assert!(gate.is_ok());
        assert_eq!(view.presentation.outcome, "not_checked");
        let (view, gate) = present(
            Some("selected".into()),
            Err(CodexAdapterError::JsonRpc {
                code: -32000,
                message: "token=secret C:/private/path".into(),
            }
            .into_runtime_failure(rah_protocol::RuntimeOperation::ModelDiscovery)),
        );
        assert_eq!(gate, Err(FrontendError::ModelCatalogUnavailable));
        use std::error::Error;
        assert!(matches!(
            view.failure
                .as_ref()
                .unwrap()
                .source()
                .unwrap()
                .downcast_ref::<CodexAdapterError>(),
            Some(CodexAdapterError::JsonRpc { code: -32000, .. })
        ));
        let serialized = serde_json::to_string(&view.presentation).unwrap();
        assert!(serialized.contains("-32000"));
        for forbidden in ["secret", "private", "retired", "outdated", "authentication"] {
            assert!(!serialized.contains(forbidden));
        }
    }
    #[tokio::test]
    async fn preflight_gates_connection_factory_before_publication() {
        for (outcome, expected) in [
            (
                ModelPreflight::Advertised(ModelCatalog {
                    models: vec!["selected".into()],
                }),
                Ok(()),
            ),
            (
                ModelPreflight::NotAdvertised(ModelCatalog {
                    models: vec!["alternative".into()],
                }),
                Err(FrontendError::ModelNotAdvertised),
            ),
            (ModelPreflight::NotChecked, Ok(())),
        ] {
            let prepared = crate::PreparedCodexConnection {
                executable: "unused-fake-executable".into(),
                model_config: rah_runtime_codex::CodexModelConfig::Inherit,
                source: crate::codex_baseline::CodexExecutableSource::Override,
            };
            let result = crate::connect_prepared_codex(prepared, |_| async move {
                let (_, gate) = present(Some("selected".into()), Ok(outcome));
                gate?;
                Ok("ready-runtime")
            })
            .await;
            match expected {
                Ok(()) => assert_eq!(result.unwrap().0, "ready-runtime"),
                Err(error) => assert_eq!(result.unwrap_err(), error),
            }
        }
    }

    #[test]
    fn status_hides_stale_observations_and_preserves_local_catalog_failure() {
        let directory = std::env::temp_dir().join(format!(
            "rah-task499-preflight-{}",
            rah_protocol::SessionId::new()
        ));
        let state = crate::DesktopAppState::new(directory);
        let (observation, _) = present(
            Some("selected".into()),
            Err(CodexAdapterError::JsonRpc {
                code: -32001,
                message: "Authorization: Bearer SECRET; raw provider body".into(),
            }
            .into_runtime_failure(rah_protocol::RuntimeOperation::ModelDiscovery)),
        );
        *state.connection.lock().unwrap() =
            crate::ConnectionState::Error(FrontendError::ModelCatalogUnavailable);
        *state.model_preflight.lock().unwrap() = Some((0, 0, observation));
        let status = state.status();
        assert_eq!(status.codex_status, "error");
        assert_eq!(
            status.model_preflight.unwrap().outcome,
            "model_catalog_unavailable"
        );
        assert!(
            !serde_json::to_string(&state.status())
                .unwrap()
                .contains("SECRET")
        );
        *state.next_connection_generation.lock().unwrap() = 1;
        assert!(state.status().model_preflight.is_none());
    }
}
