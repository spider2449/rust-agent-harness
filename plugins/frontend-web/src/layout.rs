//! `GET /api/layout`: where the page places its panels (ADR 0062).
//!
//! The mounted `ui-layout` provider's layout, checked against the panels this
//! page knows: the built-in ones and the views of mounted `ui-views`
//! providers (see [`crate::views`]). Without a provider, or when it does not
//! answer in time, [`Layout::standard`]. A view of a provider that is not
//! mounted is left out without a problem, since it may come back; a view no
//! column names is added to the right column. Read per request, so mounting
//! or unmounting a provider applies at the next page load.

use std::time::Duration;

use axum::{Json, extract::State};
use rah_api::ui::{Layout, MAX_PANELS, UI_LAYOUT, panels};
use serde_json::json;

use crate::{ApiResult, AppState, views};

/// How long a provider may take before the standard layout is used.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) async fn get(State(shared): State<AppState>) -> ApiResult {
    let views = views::listed(&shared.kernel);
    let is_view = |id: &str| views.iter().any(|view| view.id == id);
    let (mut layout, problems, source) = match shared.kernel.get(UI_LAYOUT) {
        None => (Layout::standard(), Vec::new(), "standard"),
        Some(provider) => match tokio::time::timeout(ANSWER_TIMEOUT, provider.layout()).await {
            Ok(mut layout) => {
                for column in [&mut layout.left, &mut layout.right] {
                    column.retain(|id| !id.contains('/') || is_view(id));
                }
                let (layout, problems) =
                    layout.checked(|id| panels::BUILT_IN.contains(&id) || is_view(id));
                (layout, problems, "ui-layout")
            }
            Err(_) => (
                Layout::standard(),
                vec![
                    "the ui-layout provider did not answer; the standard layout is used".to_owned(),
                ],
                "ui-layout",
            ),
        },
    };
    for view in &views {
        let placed = layout.left.len() + layout.right.len();
        if placed < MAX_PANELS
            && !layout.left.contains(&view.id)
            && !layout.right.contains(&view.id)
        {
            layout.right.push(view.id.clone());
        }
    }
    Ok(Json(
        json!({ "layout": layout, "source": source, "problems": problems }),
    ))
}
