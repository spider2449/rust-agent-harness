//! `ui-layout-config`: the `ui-layout` service from this plugin's
//! configuration (ADR 0062).
//!
//! The configuration is the layout itself: the panels of the left and the
//! right column, in order, and whether the right column starts open.
//! Without a configuration the standard layout is provided.
//!
//! ```toml
//! [[plugin]]
//! use = "ui-layout-config"
//! config = { left = ["sessions", "plugins"], right = ["files"] }
//! ```
//!
//! The configuration check refuses a panel that is placed twice, an empty or
//! overlong id, more than [`MAX_PANELS`] panels, and a name that is neither a
//! built-in panel (`sessions`, `files`, `plugins`) nor a view id
//! (`instance/view`), so a misspelt panel stops the start instead of
//! silently disappearing.

use std::sync::Arc;

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    ui::{Layout, MAX_PANELS, UI_LAYOUT, UI_LAYOUT_SERVICE, UiLayout, panels},
};

pub struct UiLayoutConfigPlugin {
    manifest: PluginManifest,
}

impl UiLayoutConfigPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([UI_LAYOUT_SERVICE]),
        }
    }

    /// An example configuration, for reference (the Web UI shows it in an
    /// empty configuration editor): the standard layout with the plugins on
    /// the left. It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "left": ["sessions", "plugins"],
  "right": ["files"],
  "right_open": true
}"#;
}

/// The configured layout, or the standard one without a configuration.
fn configured(config: &serde_json::Value) -> Result<Layout, PluginError> {
    if config.is_null() {
        return Ok(Layout::standard());
    }
    let layout: Layout = serde_json::from_value(config.clone())?;
    let (_, problems) = layout.checked(|id| panels::BUILT_IN.contains(&id) || id.contains('/'));
    if let Some(problem) = problems.first() {
        let more = match problems.len() {
            1 => String::new(),
            n => format!(" (and {} more)", n - 1),
        };
        return Err(format!(
            "{problem}{more}; panels are `sessions`, `files`, `plugins` or `instance/view`, \
             at most {MAX_PANELS}"
        )
        .into());
    }
    Ok(layout)
}

struct Fixed(Layout);

#[async_trait]
impl UiLayout for Fixed {
    async fn layout(&self) -> Layout {
        self.0.clone()
    }
}

#[async_trait]
impl Plugin for UiLayoutConfigPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        configured(config).map(drop)
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let layout = configured(scope.config())?;
        let provider: Arc<dyn UiLayout> = Arc::new(Fixed(layout));
        scope.provide(UI_LAYOUT, provider)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rah_api::{Kernel, MountOptions};
    use serde_json::json;

    use super::*;

    #[test]
    fn the_configuration_is_the_layout_and_is_checked() {
        assert_eq!(configured(&json!(null)).unwrap(), Layout::standard());
        let layout =
            configured(&json!({ "left": ["files", "usage/tokens"], "right_open": false })).unwrap();
        assert_eq!(layout.left, ["files", "usage/tokens"]);
        assert!(layout.right.is_empty());
        assert_eq!(layout.right_open, Some(false));

        let error = configured(&json!({ "left": ["plugin"], "right": ["files", "files"] }))
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("unknown panel `plugin` (and 1 more)"),
            "{error}"
        );
        assert!(configured(&json!({ "center": ["sessions"] })).is_err());
        assert!(configured(&json!({ "left": "sessions" })).is_err());
    }

    #[test]
    fn the_config_example_is_a_valid_configuration() {
        let example: serde_json::Value =
            serde_json::from_str(UiLayoutConfigPlugin::CONFIG_EXAMPLE).unwrap();
        let plugin = UiLayoutConfigPlugin::new("ui-layout-config");
        assert!(plugin.check_config(&example).is_ok());
    }

    #[tokio::test]
    async fn a_mounted_instance_provides_its_layout() {
        let kernel = Kernel::new();
        let config = json!({ "left": ["plugins"], "right": ["sessions", "files"] });
        kernel
            .mount(
                Arc::new(UiLayoutConfigPlugin::new("ui-layout-config")),
                MountOptions::with_config(config.clone()),
            )
            .await
            .unwrap();
        let provider = kernel.handle().get(UI_LAYOUT).expect("ui-layout provided");
        assert_eq!(
            serde_json::to_value(provider.layout().await).unwrap(),
            config
        );
        kernel
            .unmount(&PluginId::new("ui-layout-config"))
            .await
            .unwrap();
        assert!(kernel.handle().get(UI_LAYOUT).is_none());
    }
}
