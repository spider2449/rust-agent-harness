//! `GET /api/layout`: where the page places its panels (ADR 0062).
//!
//! The mounted `ui-layout` provider's layout, checked against the panels this
//! page knows: the built-in ones and the views of mounted `ui-views`
//! providers (see [`crate::views`]). Without a provider, or when it does not
//! answer in time, [`Layout::standard`]. A view of a provider that is not
//! mounted is left out without a problem, since it may come back; a view no
//! column names is added to the right column. Read per request.
//!
//! The reply also lists `panels`: every panel a layout can place, built-in
//! or a mounted view, with its title and the column it is in now (`null`
//! when no column shows it), so a user can see which ids exist.
//!
//! [`watch`] follows plugin changes (mount, unmount, restart, configure):
//! when the reply would differ, every window hears `{type: "layout"}` and
//! applies the new layout without a reload.

use std::{sync::Arc, time::Duration};

use axum::{Json, extract::State};
use rah_api::{
    Scope,
    kernel::PluginStatusChanged,
    ui::{Layout, MAX_PANELS, UI_LAYOUT, panels},
};
use serde_json::{Value, json};

use crate::{ApiResult, AppState, Shared, views};

/// How long a provider may take before the standard layout is used.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

/// The tab label of each built-in panel, as the page shows it.
const BUILT_IN_TITLES: [(&str, &str); 3] = [
    (panels::SESSIONS, "Sessions"),
    (panels::FILES, "Workspace"),
    (panels::PLUGINS, "Plugins"),
];

pub(crate) async fn get(State(shared): State<AppState>) -> ApiResult {
    Ok(Json(current(&shared).await))
}

/// How long plugin changes must pause before the layout is compared, so a
/// restart or a replace is seen once it settled.
const SETTLE: Duration = Duration::from_millis(200);

/// Tells every window `{type: "layout"}` when a plugin change alters the
/// layout or the panels. The task belongs to this plugin's scope.
pub(crate) fn watch(scope: &mut Scope, shared: AppState) {
    let changed = Arc::new(tokio::sync::Notify::new());
    let signal = changed.clone();
    scope.on::<PluginStatusChanged>(move |_| signal.notify_one());
    scope.spawn(async move {
        let mut last = current(&shared).await;
        loop {
            changed.notified().await;
            tokio::time::sleep(SETTLE).await;
            let now = current(&shared).await;
            if now != last {
                shared.approvals.notify(json!({ "type": "layout" }));
                last = now;
            }
        }
    });
}

/// The reply of `GET /api/layout`.
async fn current(shared: &Shared) -> Value {
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
    let column = |id: &str| {
        if layout.left.iter().any(|placed| placed == id) {
            Some("left")
        } else if layout.right.iter().any(|placed| placed == id) {
            Some("right")
        } else {
            None
        }
    };
    let built_in = BUILT_IN_TITLES.iter().map(
        |(id, title)| json!({ "id": id, "title": title, "kind": "built-in", "column": column(id) }),
    );
    let contributed = views.iter().map(|view| {
        json!({
            "id": view.id,
            "title": view.spec.title,
            "kind": "view",
            "plugin": view.instance,
            "column": column(&view.id),
        })
    });
    let panels: Vec<Value> = built_in.chain(contributed).collect();
    json!({
        "layout": layout,
        "source": source,
        "problems": problems,
        "panels": panels,
    })
}
