//! `extends = "<preset>"`: a distribution file that lists only how it differs
//! from a built-in preset (ADR 0058). Without `extends`, the file is the whole
//! plugin list, as before.
//!
//! Over the preset, in this order: `remove = [...]` drops preset plugins by
//! instance id; a `[[plugin]]` whose instance id is in the preset changes that
//! plugin in place, keeping its mount position; any other `[[plugin]]` is
//! added before the frontend. A change merges its `config` into the preset's:
//! tables merge key by key, and any other value (a string, a number, an array)
//! replaces the preset's. `replace = true` starts that plugin's `config` and
//! `bind` over instead.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{BootError, Distribution, PluginSpec};

/// What `extends` changed in a preset, for `--show-config`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extended {
    /// The preset the file extends.
    pub preset: String,
    /// Instance ids `remove` dropped from the preset.
    pub removed: Vec<String>,
    /// Instance ids of preset plugins the file changed.
    pub changed: Vec<String>,
    /// Instance ids of plugins the file added.
    pub added: Vec<String>,
}

/// A distribution file as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DistributionFile {
    extends: Option<String>,
    #[serde(default)]
    remove: Vec<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    plugin: Vec<PluginEntry>,
}

/// One `[[plugin]]` entry as written: a [`PluginSpec`], plus `replace` for a
/// file that extends a preset.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginEntry {
    #[serde(rename = "use")]
    kind: String,
    id: Option<String>,
    #[serde(default)]
    config: Value,
    #[serde(default)]
    bind: BTreeMap<String, String>,
    #[serde(default)]
    replace: bool,
}

impl PluginEntry {
    fn into_spec(self) -> PluginSpec {
        PluginSpec {
            kind: self.kind,
            id: self.id,
            config: self.config,
            bind: self.bind,
        }
    }
}

fn invalid(message: String) -> BootError {
    BootError::Parse(message)
}

/// Parses a distribution file, applying it to its preset when it has
/// `extends`.
pub(crate) fn parse(text: &str) -> Result<Distribution, BootError> {
    let file: DistributionFile =
        toml::from_str(text).map_err(|error| invalid(error.to_string()))?;
    let Some(preset) = file.extends else {
        if !file.remove.is_empty() {
            return Err(invalid("`remove` needs `extends = \"<preset>\"`".into()));
        }
        if file.plugin.iter().any(|entry| entry.replace) {
            return Err(invalid("`replace` needs `extends = \"<preset>\"`".into()));
        }
        return Ok(Distribution {
            features: file.features,
            plugin: file
                .plugin
                .into_iter()
                .map(PluginEntry::into_spec)
                .collect(),
            ..Distribution::default()
        });
    };
    let mut distribution = Distribution::preset(&preset)?;
    let mut extended = Extended {
        preset: preset.clone(),
        ..Extended::default()
    };
    for id in file.remove {
        let at = position(&distribution, &id)
            .ok_or_else(|| invalid(format!("remove: no plugin `{id}` in preset `{preset}`")))?;
        distribution.plugin.remove(at);
        extended.removed.push(id);
    }
    for entry in file.plugin {
        let id = entry.id.clone().unwrap_or_else(|| entry.kind.clone());
        match position(&distribution, &id) {
            Some(at) => {
                let spec = &mut distribution.plugin[at];
                if spec.kind != entry.kind {
                    return Err(invalid(format!(
                        "plugin `{id}` is a `{}` in preset `{preset}`, not a `{}`; \
                         remove it and add the new one with another id",
                        spec.kind, entry.kind
                    )));
                }
                if entry.replace {
                    spec.config = entry.config;
                    spec.bind = entry.bind;
                } else {
                    merge(&mut spec.config, entry.config);
                    spec.bind.extend(entry.bind);
                }
                if !extended.changed.contains(&id) {
                    extended.changed.push(id);
                }
            }
            None if entry.replace => {
                return Err(invalid(format!(
                    "`replace` changes a plugin of preset `{preset}`, which has no `{id}`"
                )));
            }
            None => {
                let at = distribution
                    .plugin
                    .iter()
                    .position(|spec| spec.kind.starts_with("frontend-"))
                    .unwrap_or(distribution.plugin.len());
                distribution.plugin.insert(at, entry.into_spec());
                extended.added.push(id);
            }
        }
    }
    distribution.features = file.features;
    distribution.extended = Some(extended);
    Ok(distribution)
}

/// The position of the plugin with instance id `id`.
fn position(distribution: &Distribution, id: &str) -> Option<usize> {
    distribution
        .plugin
        .iter()
        .position(|spec| spec.plugin_id().as_str() == id)
}

/// Merges `patch` into `base`: tables key by key, anything else replaces.
/// A missing `config` (null) changes nothing.
fn merge(base: &mut Value, patch: Value) {
    match (base, patch) {
        (_, Value::Null) => {}
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, patch) => *base = patch,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ids(distribution: &Distribution) -> Vec<String> {
        distribution
            .plugin
            .iter()
            .map(|spec| spec.plugin_id().as_str().to_owned())
            .collect()
    }

    fn spec<'a>(distribution: &'a Distribution, id: &str) -> &'a PluginSpec {
        &distribution.plugin[position(distribution, id).unwrap()]
    }

    #[test]
    fn extends_alone_is_the_preset() {
        let distribution = Distribution::parse("extends = \"dev\"\n").unwrap();
        let preset = Distribution::preset("dev").unwrap();
        assert_eq!(distribution.plugin, preset.plugin);
        assert_eq!(
            distribution.extended,
            Some(Extended {
                preset: "dev".into(),
                ..Extended::default()
            })
        );
    }

    #[test]
    fn a_change_merges_tables_and_keeps_the_position() {
        let distribution = Distribution::parse(
            r#"
extends = "dev"

[[plugin]]
use = "loop-react"
config = { max_rounds = 4 }

[[plugin]]
use = "policy-authority"
config = { tools = { "ddt.*" = "read" }, allow = ["none"] }
"#,
        )
        .unwrap();
        let preset = Distribution::preset("dev").unwrap();
        assert_eq!(ids(&distribution), ids(&preset));
        assert_eq!(
            spec(&distribution, "loop-react").config,
            json!({ "max_rounds": 4 })
        );
        let policy = &spec(&distribution, "policy-authority").config;
        // A table merges: the preset's assignments stay beside the new one.
        assert_eq!(policy["tools"]["ddt.*"], "read");
        assert_eq!(policy["tools"]["repo.*"], "read");
        // An array replaces.
        assert_eq!(policy["allow"], json!(["none"]));
        assert_eq!(policy["ask"], json!(["write", "execute"]));
        let extended = distribution.extended.unwrap();
        assert_eq!(extended.changed, ["loop-react", "policy-authority"]);
        assert!(extended.added.is_empty());
    }

    #[test]
    fn remove_and_add_swap_the_model_before_the_frontend() {
        let distribution = Distribution::parse(
            r#"
extends = "default"
remove = ["model-llamacpp"]
features = ["web-search"]

[[plugin]]
use = "model-anthropic"
config = { model = "claude-opus-5-5" }
"#,
        )
        .unwrap();
        let ids = ids(&distribution);
        assert!(!ids.iter().any(|id| id == "model-llamacpp"), "{ids:?}");
        assert_eq!(ids[ids.len() - 2], "model-anthropic");
        assert_eq!(ids[ids.len() - 1], "frontend-cli");
        assert_eq!(distribution.features, ["web-search"]);
        let extended = distribution.extended.unwrap();
        assert_eq!(extended.removed, ["model-llamacpp"]);
        assert_eq!(extended.added, ["model-anthropic"]);
    }

    #[test]
    fn replace_starts_the_configuration_over() {
        let distribution = Distribution::parse(
            r#"
extends = "dev"

[[plugin]]
use = "policy-authority"
replace = true
config = { tools = { echo = "none" } }
"#,
        )
        .unwrap();
        assert_eq!(
            spec(&distribution, "policy-authority").config,
            json!({ "tools": { "echo": "none" } })
        );
    }

    #[test]
    fn an_entry_without_config_changes_only_what_it_names() {
        let distribution = Distribution::parse(
            r#"
extends = "dev"

[[plugin]]
use = "context-default"
"#,
        )
        .unwrap();
        assert_eq!(
            spec(&distribution, "context-default").config,
            json!({ "memory_dir": "memory" })
        );
    }

    #[test]
    fn mistakes_are_refused_with_the_reason() {
        let cases = [
            ("extends = \"nope\"\n", "unknown preset `nope`"),
            (
                "extends = \"dev\"\nremove = [\"model-llamacpp\"]\n",
                "remove: no plugin `model-llamacpp` in preset `dev`",
            ),
            (
                "extends = \"dev\"\n[[plugin]]\nuse = \"model-openai\"\nid = \"model-mock\"\n",
                "plugin `model-mock` is a `model-mock` in preset `dev`, not a `model-openai`",
            ),
            (
                "extends = \"dev\"\n[[plugin]]\nuse = \"tools-shell\"\nreplace = true\n",
                "which has no `tools-shell`",
            ),
            ("remove = [\"x\"]\n", "`remove` needs `extends"),
            (
                "[[plugin]]\nuse = \"loop-react\"\nreplace = true\n",
                "`replace` needs `extends",
            ),
        ];
        for (text, expected) in cases {
            let error = Distribution::parse(text).unwrap_err().to_string();
            assert!(error.contains(expected), "{text}: {error}");
        }
    }
}
