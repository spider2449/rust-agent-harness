//! The Web UI's feature switches (ADR 0069): `GET /api/features` lists the
//! distribution's features and `PUT /api/features/{name} {enabled}` turns one
//! on or off at once, through the `admin` service, then keeps the choice in
//! the configuration file (`persist`, ADR 0067). A feature that does not
//! start changes nothing in the file.

use axum::{
    Json,
    extract::{Path, State},
};
use rah_api::admin::ConfigChange;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiResult, AppState, manage::admin, settings};

/// `GET /api/features`.
pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    let admin = admin(&shared)?;
    let features: Vec<Value> = admin
        .features()
        .into_iter()
        .map(|feature| {
            json!({
                "name": feature.name,
                "summary": feature.summary,
                "enabled": feature.enabled,
            })
        })
        .collect();
    Ok(Json(json!(features)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Switch {
    enabled: bool,
}

/// `PUT /api/features/{name}`.
pub(crate) async fn set(
    State(shared): State<AppState>,
    Path(name): Path<String>,
    Json(switch): Json<Switch>,
) -> ApiResult {
    let admin = admin(&shared)?;
    admin.set_feature(&name, switch.enabled).await?;
    let mut reply = json!({ "name": name, "enabled": switch.enabled });
    let change = ConfigChange::Feature {
        name,
        enabled: switch.enabled,
    };
    settings::remembered(
        &mut reply,
        settings::persist(admin.as_ref(), vec![change]).await,
    );
    Ok(Json(reply))
}
