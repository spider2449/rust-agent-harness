//! `GET /api/layout`: where the page places its panels (ADR 0062).
//!
//! The mounted `ui-layout` provider's layout, checked against the panels this
//! page knows; without a provider, or when it does not answer in time,
//! [`Layout::standard`]. Read per request, so mounting or unmounting a
//! provider applies at the next page load.

use std::time::Duration;

use axum::{Json, extract::State};
use rah_api::ui::{Layout, UI_LAYOUT, panels};
use serde_json::json;

use crate::{ApiResult, AppState};

/// How long a provider may take before the standard layout is used.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) async fn get(State(shared): State<AppState>) -> ApiResult {
    let Some(provider) = shared.kernel.get(UI_LAYOUT) else {
        return Ok(Json(
            json!({ "layout": Layout::standard(), "source": "standard", "problems": [] }),
        ));
    };
    let (layout, problems) = match tokio::time::timeout(ANSWER_TIMEOUT, provider.layout()).await {
        Ok(layout) => layout.checked(|id| panels::BUILT_IN.contains(&id)),
        Err(_) => (
            Layout::standard(),
            vec!["the ui-layout provider did not answer; the standard layout is used".to_owned()],
        ),
    };
    Ok(Json(
        json!({ "layout": layout, "source": "ui-layout", "problems": problems }),
    ))
}
