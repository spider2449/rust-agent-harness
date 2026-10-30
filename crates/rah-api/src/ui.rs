//! The `ui-layout` service: where a frontend places its panels (ADR 0062).
//!
//! A [`Layout`] is data: the panels of the left and the right column, in
//! order. The conversation is always the center, and the header, the
//! approvals and the status bar are not panels, so no layout can move or hide
//! them. A frontend uses [`Layout::standard`] when no `ui-layout` provider is
//! mounted, and checks a provider's layout with [`Layout::checked`] before
//! using it.

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
}
