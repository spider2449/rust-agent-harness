use rah_protocol::RuntimeDiagnostic;
use rah_runtime::{ModelPreflight, RuntimeFailure};

use serde::Serialize;

use crate::FrontendError;

pub(crate) struct ScopedModelPreflight {
    pub adapter: crate::runtime_model_state::RuntimeAdapterIdentity,
    pub connection_generation: u64,
    pub model_generation: u64,
    pub state: ModelPreflightState,
}

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
}
