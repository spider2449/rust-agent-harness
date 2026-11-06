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
//!
//! A file with user keys (`model`, `model_options`, `permissions`, `root`;
//! ADR 0067) extends the guarded base composition implicitly: the keys are
//! applied first, then `remove` and `[[plugin]]` as above.

use std::{collections::BTreeMap, path::Path};

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{BootError, Distribution, Permissions, PluginSpec, UserSettings, base_text};

/// What `Extended::preset` says for a file with user keys.
pub const USER_BASE: &str = "base";

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
    model: Option<String>,
    model_options: Option<Map<String, Value>>,
    permissions: Option<String>,
    root: Option<String>,
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
    let has_user_keys = file.model.is_some()
        || file.model_options.is_some()
        || file.permissions.is_some()
        || file.root.is_some();
    if has_user_keys {
        return parse_user(file);
    }
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
    let extended = apply_changes(&mut distribution, &preset, file.remove, file.plugin)?;
    distribution.features = file.features;
    distribution.extended = Some(extended);
    Ok(distribution)
}

/// A file with user keys: the guarded base, then the keys, then the changes.
fn parse_user(file: DistributionFile) -> Result<Distribution, BootError> {
    if file.extends.is_some() {
        return Err(invalid(
            "`extends` cannot be combined with `model`, `model_options`, `permissions` \
             or `root`: those keys already start from the guarded base"
                .into(),
        ));
    }
    let mut distribution = Distribution::parse(&base_text())?;
    let options = file.model_options.unwrap_or_default();
    match &file.model {
        Some(model) => distribution
            .set_model(model, &options)
            .map_err(|error| invalid(format!("model: {error}")))?,
        None if !options.is_empty() => {
            return Err(invalid("`model_options` needs `model`".into()));
        }
        None => {}
    }
    let permissions = file
        .permissions
        .as_deref()
        .map(str::parse::<Permissions>)
        .transpose()
        .map_err(|error| invalid(format!("permissions: {error}")))?;
    if let Some(permissions) = permissions {
        distribution.set_permissions(permissions).map_err(invalid)?;
    }
    if let Some(root) = &file.root {
        distribution.set_root(Path::new(root));
    }
    let extended = apply_changes(&mut distribution, USER_BASE, file.remove, file.plugin)?;
    distribution.features = file.features;
    distribution.extended = Some(extended);
    distribution.user = Some(UserSettings {
        model: file.model,
        permissions,
        root: file.root,
    });
    Ok(distribution)
}

/// Applies `remove` and the `[[plugin]]` changes to `distribution`, which was
/// composed from `preset`.
fn apply_changes(
    distribution: &mut Distribution,
    preset: &str,
    remove: Vec<String>,
    plugins: Vec<PluginEntry>,
) -> Result<Extended, BootError> {
    let mut extended = Extended {
        preset: preset.to_owned(),
        ..Extended::default()
    };
    let source = if preset == USER_BASE {
        "the base composition".to_owned()
    } else {
        format!("preset `{preset}`")
    };
    for id in remove {
        let at = position(distribution, &id)
            .ok_or_else(|| invalid(format!("remove: no plugin `{id}` in {source}")))?;
        distribution.plugin.remove(at);
        extended.removed.push(id);
    }
    for entry in plugins {
        let id = entry.id.clone().unwrap_or_else(|| entry.kind.clone());
        match position(distribution, &id) {
            Some(at) => {
                let spec = &mut distribution.plugin[at];
                if spec.kind != entry.kind {
                    return Err(invalid(format!(
                        "plugin `{id}` is a `{}` in {source}, not a `{}`; \
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
                    "`replace` changes a plugin of {source}, which has no `{id}`"
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
    Ok(extended)
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
pub(crate) fn merge(base: &mut Value, patch: Value) {
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
    fn user_keys_compose_the_guarded_base() {
        let distribution = Distribution::parse(
            r#"
model = "anthropic/claude-sonnet-5-5"
permissions = "auto-edit"
root = "D:/work/project"
features = ["web-search"]

[model_options]
max_tokens = 1000

[[plugin]]
use = "loop-react"
config = { max_rounds = 4 }
"#,
        )
        .unwrap();
        let ids = ids(&distribution);
        assert_eq!(ids[0], "policy-authority");
        assert!(ids.iter().any(|id| id == "tools-shell"), "{ids:?}");
        assert_eq!(
            spec(&distribution, "model-anthropic").config,
            json!({ "model": "claude-sonnet-5-5", "max_tokens": 1000 })
        );
        let policy = &spec(&distribution, "policy-authority").config;
        assert_eq!(policy["allow"], json!(["none", "read", "write"]));
        assert_eq!(policy["ask"], json!(["execute"]));
        assert_eq!(policy["tools"]["shell.exec"], "execute");
        assert_eq!(
            spec(&distribution, "tools-builtin").config["root"],
            "D:/work/project"
        );
        assert_eq!(
            spec(&distribution, "sandbox-process").config["root"],
            "D:/work/project"
        );
        assert_eq!(
            spec(&distribution, "loop-react").config,
            json!({ "max_rounds": 4 })
        );
        assert_eq!(distribution.features, ["web-search"]);
        assert_eq!(
            distribution.extended.as_ref().unwrap().changed,
            ["loop-react"]
        );
        assert_eq!(
            distribution.user,
            Some(UserSettings {
                model: Some("anthropic/claude-sonnet-5-5".into()),
                permissions: Some(Permissions::AutoEdit),
                root: Some("D:/work/project".into()),
            })
        );
    }

    #[test]
    fn user_keys_without_a_model_leave_it_to_the_command_line() {
        let distribution = Distribution::parse("permissions = \"ask\"\n").unwrap();
        assert!(!distribution.has_model());
        assert!(distribution.user.is_some());
    }

    #[test]
    fn user_key_mistakes_are_refused_with_the_reason() {
        let cases = [
            ("model = \"gemini/x\"\n", "unknown model provider `gemini`"),
            ("permissions = \"yolo\"\n", "unknown permissions `yolo`"),
            ("[model_options]\nx = 1\n", "`model_options` needs `model`"),
            (
                "model = \"mock\"\nextends = \"dev\"\n",
                "`extends` cannot be combined with `model`",
            ),
            (
                "model = \"mock\"\nremove = [\"nope\"]\n",
                "remove: no plugin `nope` in the base composition",
            ),
        ];
        for (text, expected) in cases {
            let error = Distribution::parse(text).unwrap_err().to_string();
            assert!(error.contains(expected), "{text}: {error}");
        }
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
                "extends = \"minimal\"\n[[plugin]]\nuse = \"tools-shell\"\nreplace = true\n",
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
