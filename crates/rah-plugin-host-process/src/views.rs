//! Views of protocol 4 (ADR 0063). A plugin that provides `ui-views` lists
//! its views in the manifest; the host sends `views/render {view, session}`
//! and `views/act {view, action, input, session}`, and turns the plugin's
//! `ui/changed {view}` notification into a [`UiChanged`] event. Requests are
//! never retried.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use rah_api::{
    CancellationToken, KernelHandle, async_trait,
    ui::{ActionOutcome, UiChanged, UiError, UiViews, View, ViewContext, ViewSpec},
};
use serde_json::{Value, json};

use crate::{Notified, Peer, PeerError};

/// The most views one plugin offers.
pub const MAX_VIEWS: usize = 32;
/// The notification a plugin sends when a view changed.
pub const CHANGED: &str = "ui/changed";

/// Checks the `views` of a manifest.
pub(crate) fn check(views: &[ViewSpec]) -> Result<(), String> {
    if views.len() > MAX_VIEWS {
        return Err(format!("more than {MAX_VIEWS} views"));
    }
    let mut ids = BTreeSet::new();
    for view in views {
        if !ViewSpec::valid_id(&view.id) {
            return Err(format!(
                "invalid view id `{}`: lowercase letters, digits, `-` and `_`",
                view.id
            ));
        }
        if !ids.insert(view.id.as_str()) {
            return Err(format!("duplicate view `{}`", view.id));
        }
    }
    Ok(())
}

/// Emits [`UiChanged`] for `ui/changed {view}` of a view in `views`.
pub(crate) fn notified(kernel: KernelHandle, instance: String, views: &[ViewSpec]) -> Notified {
    let ids: BTreeSet<String> = views.iter().map(|view| view.id.clone()).collect();
    Arc::new(move |method, params| {
        if method != CHANGED {
            return;
        }
        if let Some(view) = params.get("view").and_then(Value::as_str)
            && ids.contains(view)
        {
            kernel.emit(&UiChanged {
                instance: instance.clone(),
                view: view.to_owned(),
            });
        }
    })
}

/// The `ui-views` provider of a running process plugin.
pub(crate) struct ProcessViews {
    pub peer: Peer,
    pub views: Vec<ViewSpec>,
    pub timeout: Duration,
}

impl ProcessViews {
    fn known(&self, view: &str) -> Result<(), UiError> {
        if self.views.iter().any(|spec| spec.id == view) {
            Ok(())
        } else {
            Err(UiError::UnknownView(view.to_owned()))
        }
    }

    async fn request(&self, method: &'static str, params: Value) -> Result<Value, UiError> {
        self.peer
            .request(method, params, self.timeout, &CancellationToken::new())
            .await
            .map_err(failure)
    }
}

fn failure(error: PeerError) -> UiError {
    match error {
        PeerError::Remote(fault) => {
            let kind = fault
                .data
                .as_ref()
                .and_then(|data| data.get("kind"))
                .and_then(Value::as_str);
            match kind {
                Some("unknown_view") => UiError::UnknownView(fault.message),
                Some("unknown_action") => UiError::UnknownAction(fault.message),
                Some("invalid_input") => UiError::InvalidInput(fault.message),
                _ => UiError::Failed(format!("the plugin failed: {}", fault.message)),
            }
        }
        PeerError::TimedOut => UiError::Failed("the plugin did not answer in time".into()),
        error => UiError::Failed(format!("the plugin is not reachable: {error}")),
    }
}

fn session(cx: &ViewContext) -> Value {
    cx.session
        .as_ref()
        .map_or(Value::Null, |id| Value::String(id.to_string()))
}

#[async_trait]
impl UiViews for ProcessViews {
    fn views(&self) -> Vec<ViewSpec> {
        self.views.clone()
    }

    async fn render(&self, view: &str, cx: &ViewContext) -> Result<View, UiError> {
        self.known(view)?;
        let result = self
            .request(
                "views/render",
                json!({ "view": view, "session": session(cx) }),
            )
            .await?;
        serde_json::from_value(result).map_err(|error| {
            UiError::Failed(format!("the plugin returned a malformed view: {error}"))
        })
    }

    async fn act(
        &self,
        view: &str,
        action: &str,
        input: Value,
        cx: &ViewContext,
    ) -> Result<ActionOutcome, UiError> {
        self.known(view)?;
        let params =
            json!({ "view": view, "action": action, "input": input, "session": session(cx) });
        let result = self.request("views/act", params).await?;
        serde_json::from_value(result).map_err(|error| {
            UiError::Failed(format!(
                "the plugin returned a malformed action result: {error}"
            ))
        })
    }
}
