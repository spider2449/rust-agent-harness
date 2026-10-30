//! The `ui-layout` and `ui-views` services: where a frontend places its
//! panels, and views plugins contribute as panels (ADR 0062).
//!
//! A [`Layout`] is data: the panels of the left and the right column, in
//! order. The conversation is always the center, and the header, the
//! approvals and the status bar are not panels, so no layout can move or hide
//! them. A frontend uses [`Layout::standard`] when no `ui-layout` provider is
//! mounted, and checks a provider's layout with [`Layout::checked`] before
//! using it.
//!
//! A [`View`] is data too: nodes from a fixed vocabulary that the frontend
//! renders as text, with buttons and forms that call the provider's
//! [`UiViews::act`] on the user's click. Views carry no markup or code.

use std::collections::HashSet;

use async_trait::async_trait;
use rah_kernel::ServiceKey;
use serde::{Deserialize, Serialize};

/// Service name `ui-layout` providers declare in their manifest's `provides`.
pub const UI_LAYOUT_SERVICE: &str = "ui-layout";

pub const UI_LAYOUT: ServiceKey<dyn UiLayout> = ServiceKey::new(UI_LAYOUT_SERVICE);

/// The most panels a layout places; further ones are dropped.
pub const MAX_PANELS: usize = 32;
/// The longest panel id kept.
pub const MAX_PANEL_ID_BYTES: usize = 128;

/// The built-in panels of the Web UI.
pub mod panels {
    /// The session list.
    pub const SESSIONS: &str = "sessions";
    /// The workspace: files, source control, open files and the activity log.
    pub const FILES: &str = "files";
    /// Every plugin and its configuration, and the tools.
    pub const PLUGINS: &str = "plugins";

    pub const BUILT_IN: [&str; 3] = [SESSIONS, FILES, PLUGINS];
}

/// Where a frontend places its panels. Each column shows its panels as tabs,
/// the first one selected; an empty column is not shown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    #[serde(default)]
    pub left: Vec<String>,
    #[serde(default)]
    pub right: Vec<String>,
    /// Whether the right column starts open in a window without a stored
    /// choice; `None` lets the frontend decide (the Web UI opens it in wide
    /// windows). The user's choice, once made, wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_open: Option<bool>,
}

impl Layout {
    /// The Web UI's own layout: the sessions on the left, the workspace and
    /// the plugins on the right.
    pub fn standard() -> Self {
        Self {
            left: vec![panels::SESSIONS.to_owned()],
            right: vec![panels::FILES.to_owned(), panels::PLUGINS.to_owned()],
            right_open: None,
        }
    }

    /// This layout with only the panels `known` accepts, each placed once
    /// (the first place wins), and at most [`MAX_PANELS`] of them; with one
    /// line per dropped panel.
    pub fn checked(&self, known: impl Fn(&str) -> bool) -> (Self, Vec<String>) {
        let mut seen = HashSet::new();
        let mut problems = Vec::new();
        let mut keep = |panels: &[String]| {
            let mut kept = Vec::new();
            for id in panels {
                let shown: String = id.chars().take(64).collect();
                if id.is_empty() || id.len() > MAX_PANEL_ID_BYTES {
                    problems.push(format!("panel id `{shown}` is empty or too long"));
                } else if !known(id) {
                    problems.push(format!("unknown panel `{shown}`"));
                } else if !seen.insert(id.clone()) {
                    problems.push(format!("panel `{shown}` is placed twice"));
                } else if seen.len() > MAX_PANELS {
                    problems.push(format!("more than {MAX_PANELS} panels; `{shown}` dropped"));
                } else {
                    kept.push(id.clone());
                }
            }
            kept
        };
        let left = keep(&self.left);
        let right = keep(&self.right);
        (
            Self {
                left,
                right,
                right_open: self.right_open,
            },
            problems,
        )
    }
}

/// A provider of the frontend layout. One instance is used: the one the
/// kernel resolves for [`UI_LAYOUT`].
#[async_trait]
pub trait UiLayout: Send + Sync {
    async fn layout(&self) -> Layout;
}

/// Service name `ui-views` providers declare in their manifest's `provides`.
pub const UI_VIEWS_SERVICE: &str = "ui-views";

/// Views plugins contribute. Several instances may be mounted; frontends read
/// them all with `get_all`.
pub const UI_VIEWS: ServiceKey<dyn UiViews> = ServiceKey::new(UI_VIEWS_SERVICE);

/// The most nodes a view holds, counting every table cell, list item and
/// form field.
pub const MAX_VIEW_NODES: usize = 2000;
/// The largest view, serialized as JSON.
pub const MAX_VIEW_BYTES: usize = 256 * 1024;
/// The longest view id.
pub const MAX_VIEW_ID_BYTES: usize = 64;

/// A view a provider offers: shown by the Web UI as a panel with the id
/// `<instance>/<id>`, which a layout can place.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSpec {
    /// Lowercase letters, digits, `-` and `_`; at most [`MAX_VIEW_ID_BYTES`].
    pub id: String,
    /// The tab label.
    pub title: String,
}

impl ViewSpec {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
        }
    }

    /// Whether `id` is a valid view id.
    pub fn valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= MAX_VIEW_ID_BYTES
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    }
}

/// Emitted on the event bus by a `ui-views` provider when what a view shows
/// has changed; frontends render it again where it is shown. `instance` is
/// the provider's service instance: its plugin id, unless it provides a
/// named instance. Emitting is cheap and may be frequent; frontends coalesce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiChanged {
    pub instance: String,
    pub view: String,
}

/// What a view is rendered or acted on for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewContext {
    /// The session open in the window, if any.
    pub session: Option<rah_protocol::SessionId>,
}

/// The content of a view: nodes from a fixed vocabulary, rendered by the
/// frontend. Text is shown as text; no node carries markup, script, style or
/// a URL the page loads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub nodes: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Node {
    Heading {
        text: String,
    },
    Text {
        text: String,
    },
    /// Rendered like the model's replies: no raw HTML, no images, only
    /// `http(s)` and `mailto` links.
    Markdown {
        text: String,
    },
    /// Labelled values, one per line.
    KeyValue {
        items: Vec<KeyValue>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    List {
        items: Vec<String>,
    },
    /// Runs `action` with no input.
    Button {
        action: String,
        label: String,
    },
    /// Runs `action` with the fields' values as a JSON object.
    Form {
        action: String,
        submit: String,
        fields: Vec<Field>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// One form field. Its value reaches the action as a string, a number or a
/// boolean, by `kind`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub kind: FieldKind,
    /// The initial value, as text (`"true"` for a ticked boolean).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
    /// The choices of a `choice` field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    #[default]
    Text,
    Number,
    Boolean,
    Choice,
}

impl View {
    /// The number of nodes, counting every table cell, list item and field.
    pub fn size(&self) -> usize {
        self.nodes
            .iter()
            .map(|node| match node {
                Node::KeyValue { items } => 1 + items.len(),
                Node::Table { columns, rows } => {
                    1 + columns.len() + rows.iter().map(Vec::len).sum::<usize>()
                }
                Node::List { items } => 1 + items.len(),
                Node::Form { fields, .. } => 1 + fields.len(),
                _ => 1,
            })
            .sum()
    }

    /// Refuses a view over [`MAX_VIEW_NODES`] nodes or [`MAX_VIEW_BYTES`].
    pub fn check(&self) -> Result<(), UiError> {
        let nodes = self.size();
        if nodes > MAX_VIEW_NODES {
            return Err(UiError::TooLarge(format!(
                "{nodes} nodes; at most {MAX_VIEW_NODES}"
            )));
        }
        let bytes = serde_json::to_vec(self).map_or(usize::MAX, |json| json.len());
        if bytes > MAX_VIEW_BYTES {
            return Err(UiError::TooLarge(format!(
                "{bytes} bytes; at most {MAX_VIEW_BYTES}"
            )));
        }
        Ok(())
    }
}

/// What an action did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionOutcome {
    /// One line to show the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Whether the view should be rendered again.
    #[serde(default)]
    pub rerender: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UiError {
    #[error("no view `{0}`")]
    UnknownView(String),
    #[error("no action `{0}` in this view")]
    UnknownAction(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("the view is too large: {0}")]
    TooLarge(String),
    #[error("{0}")]
    Failed(String),
}

/// A provider of views. Views are shown and acted on by the user through a
/// frontend; this is not a tool, so the model cannot reach it.
#[async_trait]
pub trait UiViews: Send + Sync {
    /// The views this instance offers. Cheap: called on every page load.
    fn views(&self) -> Vec<ViewSpec>;

    /// The current content of `view`.
    async fn render(&self, view: &str, cx: &ViewContext) -> Result<View, UiError>;

    /// The user pressed a button (`input` is null) or submitted a form
    /// (`input` is an object of the fields' values) in `view`.
    async fn act(
        &self,
        view: &str,
        action: &str,
        input: serde_json::Value,
        cx: &ViewContext,
    ) -> Result<ActionOutcome, UiError> {
        let _ = (view, input, cx);
        Err(UiError::UnknownAction(action.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in(id: &str) -> bool {
        panels::BUILT_IN.contains(&id)
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn standard_places_every_built_in_panel_once() {
        let (checked, problems) = Layout::standard().checked(built_in);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(checked, Layout::standard());
        let mut placed: Vec<_> = checked.left.iter().chain(&checked.right).cloned().collect();
        placed.sort();
        let mut all = ids(&panels::BUILT_IN);
        all.sort();
        assert_eq!(placed, all);
    }

    #[test]
    fn unknown_duplicate_and_bad_ids_are_dropped() {
        let layout = Layout {
            left: ids(&["plugins", "nope", ""]),
            right: ids(&["sessions", "plugins", &"x".repeat(MAX_PANEL_ID_BYTES + 1)]),
            right_open: Some(true),
        };
        let (checked, problems) = layout.checked(built_in);
        assert_eq!(checked.left, ids(&["plugins"]));
        assert_eq!(checked.right, ids(&["sessions"]));
        assert_eq!(checked.right_open, Some(true));
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().any(|p| p == "unknown panel `nope`"));
        assert!(
            problems
                .iter()
                .any(|p| p == "panel `plugins` is placed twice")
        );
    }

    #[test]
    fn at_most_max_panels_are_kept() {
        let names: Vec<String> = (0..MAX_PANELS + 3).map(|n| format!("p{n}")).collect();
        let layout = Layout {
            left: names,
            ..Layout::default()
        };
        let (checked, problems) = layout.checked(|_| true);
        assert_eq!(checked.left.len(), MAX_PANELS);
        assert_eq!(problems.len(), 3);
    }

    #[test]
    fn layout_json_round_trips_and_refuses_unknown_keys() {
        let json = serde_json::to_value(Layout::standard()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"left": ["sessions"], "right": ["files", "plugins"]})
        );
        let back: Layout = serde_json::from_value(json).unwrap();
        assert_eq!(back, Layout::standard());
        let empty: Layout = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Layout::default());
        assert!(serde_json::from_str::<Layout>(r#"{"center": []}"#).is_err());
    }

    #[test]
    fn view_ids_are_short_lowercase_names() {
        assert!(ViewSpec::valid_id("usage"));
        assert!(ViewSpec::valid_id("tool_calls-2"));
        for bad in [
            "",
            "Usage",
            "a/b",
            "a b",
            &"x".repeat(MAX_VIEW_ID_BYTES + 1),
        ] {
            assert!(!ViewSpec::valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn views_are_tagged_json_and_bounded() {
        let view = View {
            nodes: vec![
                Node::Heading {
                    text: "Usage".into(),
                },
                Node::KeyValue {
                    items: vec![KeyValue::new("turns", "2")],
                },
                Node::Table {
                    columns: ids(&["tool", "calls"]),
                    rows: vec![ids(&["echo", "1"]), ids(&["fs.read", "3"])],
                },
                Node::Button {
                    action: "reset".into(),
                    label: "Reset".into(),
                },
            ],
        };
        assert_eq!(view.size(), 1 + 2 + 7 + 1);
        assert!(view.check().is_ok());
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(
            json["nodes"][0],
            serde_json::json!({"type": "heading", "text": "Usage"})
        );
        assert_eq!(
            json["nodes"][3],
            serde_json::json!({"type": "button", "action": "reset", "label": "Reset"})
        );
        assert_eq!(serde_json::from_value::<View>(json).unwrap(), view);
        assert!(
            serde_json::from_value::<View>(
                serde_json::json!({"nodes": [{"type": "html", "text": "<b>"}]})
            )
            .is_err()
        );

        let many = View {
            nodes: vec![Node::List {
                items: vec![String::new(); MAX_VIEW_NODES],
            }],
        };
        assert!(matches!(many.check(), Err(UiError::TooLarge(_))));
        let big = View {
            nodes: vec![Node::Text {
                text: "x".repeat(MAX_VIEW_BYTES),
            }],
        };
        assert!(matches!(big.check(), Err(UiError::TooLarge(_))));
    }

    #[test]
    fn form_fields_default_to_text() {
        let field: Field =
            serde_json::from_value(serde_json::json!({"name": "q", "label": "Query"})).unwrap();
        assert_eq!(field.kind, FieldKind::Text);
        assert!(field.value.is_empty() && field.options.is_empty());
    }
}
