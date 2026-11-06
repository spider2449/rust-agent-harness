//! Keeps a [`ConfigChange`] in the configuration file (ADR 0067 §5), so a
//! setting saved in the Web UI is what the next start runs. Only a file with
//! user keys is changed: its `[[plugin]]` entries are changes to the base
//! composition, so a setting is written as the difference from what the file
//! composes without it, and later additions to the base still reach it.
//!
//! The file is edited in place with `toml_edit`, keeping comments and order,
//! checked by parsing it again, and replaced atomically.

use std::{
    fs,
    path::{Path, PathBuf},
};

use rah_api::{PluginId, admin::ConfigChange};
use serde_json::{Map, Value};
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table};

use crate::{Distribution, PROVIDERS, Permissions, Provider, base_text, feature, is_model_kind};

const POLICY: &str = "policy-authority";

/// Applies `change` to the configuration file at `path`. `data_home` is the
/// data directory the running plugins resolved their data paths against;
/// the file's own values are resolved the same way before comparing, so an
/// unchanged path is not written out.
pub fn persist(path: &Path, change: &ConfigChange, data_home: Option<&Path>) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    let current =
        Distribution::parse(&text).map_err(|error| format!("`{}`: {error}", path.display()))?;
    if current.user.is_none() {
        return Err(format!(
            "`{}` lists its plugins, so settings are not saved to it; a file with \
             `model = \"<provider>/<id>\"` keeps them (`rah config init` writes one)",
            path.display()
        ));
    }
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|error| format!("`{}`: {error}", path.display()))?;
    match change {
        ConfigChange::Feature { name, enabled } => set_feature(&mut doc, name, *enabled)?,
        ConfigChange::Removed { id } => {
            remove_plugin(&mut doc, id.as_str(), model_id(&current).as_deref())?;
        }
        ConfigChange::Model { kind, config } => {
            if !is_model_kind(kind) {
                return Err(format!("`{kind}` is not a model plugin"));
            }
            set_model(&mut doc, kind, config);
        }
        ConfigChange::Plugin { id, kind, config } => {
            set_plugin(&mut doc, id.as_str(), kind, config, data_home)?;
        }
    }
    let new_text = doc.to_string();
    // What the next start reads must parse; a change that breaks it is
    // refused and the file is left as it was.
    compose(&new_text).map_err(|error| format!("the change would break the file: {error}"))?;
    write_atomically(path, &new_text)
}

/// Settings an earlier version of the Web UI kept in `storage` under its
/// namespace directory `dir`, as changes in the order they were applied, by
/// the file each came from. Stopped plugins are a runtime state and become
/// nothing; an unreadable entry is an error naming its file.
pub fn legacy_changes(dir: &Path) -> Result<Vec<(PathBuf, Vec<ConfigChange>)>, String> {
    let read = |name: &str| -> Result<Option<(PathBuf, Vec<u8>)>, String> {
        let path = dir.join(name);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some((path, bytes))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("cannot read `{}`: {error}", path.display())),
        }
    };
    let json = |path: &Path, bytes: &[u8]| -> Result<Value, String> {
        serde_json::from_slice(bytes).map_err(|error| format!("`{}`: {error}", path.display()))
    };
    let mut found = Vec::new();
    for slot in ["tools", "model", "assistant", "loop"] {
        let Some((path, bytes)) = read(&format!("settings-{slot}"))? else {
            continue;
        };
        let saved = json(&path, &bytes)?;
        let kind = saved["kind"]
            .as_str()
            .ok_or_else(|| format!("`{}` has no `kind`", path.display()))?
            .to_owned();
        let config = saved["config"].clone();
        let mut changes = Vec::new();
        match slot {
            "model" => changes.push(ConfigChange::Model { kind, config }),
            "loop" if kind != "loop-react" => {
                changes.push(ConfigChange::Removed {
                    id: PluginId::new("loop-react"),
                });
                changes.push(ConfigChange::Plugin {
                    id: PluginId::new(kind.clone()),
                    kind,
                    config,
                });
            }
            _ => changes.push(ConfigChange::Plugin {
                id: PluginId::new(kind.clone()),
                kind,
                config,
            }),
        }
        found.push((path, changes));
    }
    if let Some((path, bytes)) = read("plugins")? {
        let entries = json(&path, &bytes)?;
        let entries = entries
            .as_object()
            .ok_or_else(|| format!("`{}` is not a table", path.display()))?;
        let mut changes = Vec::new();
        for (id, desired) in entries {
            let id = PluginId::new(id.clone());
            match desired["state"].as_str() {
                Some("mounted") => changes.push(ConfigChange::Plugin {
                    id,
                    kind: desired["kind"].as_str().unwrap_or_default().to_owned(),
                    config: desired["config"].clone(),
                }),
                Some("removed") => changes.push(ConfigChange::Removed { id }),
                _ => {}
            }
        }
        found.push((path, changes));
    }
    if let Some((path, bytes)) = read("web-search")? {
        let changes = if bytes == b"on" {
            vec![ConfigChange::Feature {
                name: "web-search".into(),
                enabled: true,
            }]
        } else {
            Vec::new()
        };
        found.push((path, changes));
    }
    Ok(found)
}

/// The distribution `text` composes, features applied, as a start does.
fn compose(text: &str) -> Result<Distribution, String> {
    let mut distribution = Distribution::parse(text).map_err(|error| error.to_string())?;
    distribution.apply_features()?;
    Ok(distribution)
}

fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "rah.toml".to_owned());
    let temp: PathBuf = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&temp, text)
        .map_err(|error| format!("cannot write `{}`: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("cannot replace `{}`: {error}", path.display())
    })
}

/// The top-level array `key`, created empty when missing.
fn array<'a>(doc: &'a mut DocumentMut, key: &str) -> Result<&'a mut Array, String> {
    if doc.get(key).is_none() {
        doc[key] = toml_edit::value(Array::new());
    }
    doc[key]
        .as_array_mut()
        .ok_or_else(|| format!("`{key}` is not a list"))
}

fn set_feature(doc: &mut DocumentMut, request: &str, enabled: bool) -> Result<(), String> {
    let (name, _) = crate::features::parse_request(request);
    if feature(name).is_none() {
        return Err(format!("unknown feature `{name}`"));
    }
    let features = array(doc, "features")?;
    // Any form of the feature (`shell`, `shell=git`) is the same feature.
    features.retain(|item| {
        item.as_str()
            .is_none_or(|text| crate::features::parse_request(text).0 != name)
    });
    if enabled {
        features.push(request);
    }
    features.fmt();
    Ok(())
}

/// The `[[plugin]]` entries, created empty when missing.
fn entries(doc: &mut DocumentMut) -> Result<&mut ArrayOfTables, String> {
    if doc.get("plugin").is_none() {
        doc["plugin"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    doc["plugin"]
        .as_array_of_tables_mut()
        .ok_or_else(|| "`plugin` is not a list of `[[plugin]]` tables".to_owned())
}

/// The instance id of a `[[plugin]]` entry: its `id`, else its `use`.
fn entry_id(table: &Table) -> Option<&str> {
    table
        .get("id")
        .and_then(Item::as_str)
        .or_else(|| table.get("use").and_then(Item::as_str))
}

/// Removes the entries for `id`; returns whether there were any.
fn drop_entries(doc: &mut DocumentMut, id: &str) -> Result<bool, String> {
    let entries = entries(doc)?;
    let before = entries.len();
    entries.retain(|table| entry_id(table) != Some(id));
    Ok(entries.len() != before)
}

/// Instance ids of the base composition, which `remove` can name.
fn base_ids() -> Vec<String> {
    Distribution::parse(&base_text())
        .map(|base| {
            base.plugin
                .iter()
                .map(|spec| spec.plugin_id().as_str().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The instance id of the model plugin `distribution` runs.
fn model_id(distribution: &Distribution) -> Option<String> {
    distribution
        .plugin
        .iter()
        .find(|spec| is_model_kind(&spec.kind))
        .map(|spec| spec.plugin_id().as_str().to_owned())
}

fn remove_plugin(doc: &mut DocumentMut, id: &str, model: Option<&str>) -> Result<(), String> {
    if id == POLICY {
        return Err("the policy cannot be removed; tool calls would run unguarded".into());
    }
    if model == Some(id) {
        return Err("the model cannot be removed; choose another model instead".into());
    }
    drop_entries(doc, id)?;
    if base_ids().iter().any(|base| base == id) {
        let remove = array(doc, "remove")?;
        if !remove.iter().any(|item| item.as_str() == Some(id)) {
            remove.push(id);
        }
        remove.fmt();
    }
    Ok(())
}

/// The provider `kind` and `config` read as, and the rest of the
/// configuration (`model_options`).
fn provider_of(kind: &str, config: &Value) -> Option<(&'static Provider, Map<String, Value>)> {
    let object = config.as_object().cloned().unwrap_or_default();
    let matches = |provider: &&Provider| {
        provider.kind == kind
            && provider
                .preset()
                .iter()
                .all(|(key, value)| object.get(*key).and_then(Value::as_str) == Some(*value))
    };
    let provider = PROVIDERS
        .iter()
        .filter(matches)
        .max_by_key(|provider| provider.preset().len())?;
    let mut options = object;
    for (key, _) in provider.preset() {
        options.remove(*key);
    }
    Some((provider, options))
}

fn set_model(doc: &mut DocumentMut, kind: &str, config: &Value) {
    let Some((provider, mut options)) = provider_of(kind, config) else {
        return;
    };
    let model = match options.remove("model") {
        Some(Value::String(id)) if !id.is_empty() => format!("{}/{id}", provider.name),
        _ => provider.name.to_owned(),
    };
    doc["model"] = toml_edit::value(model);
    if options.is_empty() {
        doc.remove("model_options");
    } else {
        let mut table = Table::new();
        for (key, value) in &options {
            if let Some(value) = to_toml(value) {
                table[key.as_str()] = toml_edit::value(value);
            }
        }
        doc["model_options"] = Item::Table(table);
    }
}

fn set_plugin(
    doc: &mut DocumentMut,
    id: &str,
    kind: &str,
    config: &Value,
    data_home: Option<&Path>,
) -> Result<(), String> {
    if !config.is_object() && !config.is_null() {
        return Err(format!("the configuration of `{id}` must be a table"));
    }
    if kind == POLICY
        && let Some(permissions) = permissions_of(config)
    {
        doc["permissions"] = toml_edit::value(permissions.name());
    }
    // It runs again, so `remove` no longer names it.
    if let Some(remove) = doc.get_mut("remove").and_then(Item::as_array_mut) {
        remove.retain(|item| item.as_str() != Some(id));
        remove.fmt();
    }
    drop_entries(doc, id)?;
    // What the file composes for `id` without an entry of its own.
    let without = compose(&doc.to_string())?;
    let base = without
        .plugin
        .iter()
        .find(|spec| spec.plugin_id().as_str() == id);
    let (entry_config, replace) = match base {
        Some(spec) if spec.kind == kind => {
            let mut resolved = spec.config.clone();
            if let Some(home) = data_home {
                crate::resolve_data_paths(kind, &mut resolved, home);
            }
            if resolved == *config || spec.config == *config {
                return Ok(());
            }
            // A merge cannot drop a key; when the difference alone does not
            // give `config`, the entry replaces the configuration.
            let difference = difference(&resolved, config).unwrap_or(Value::Null);
            let mut merged = resolved;
            crate::extend::merge(&mut merged, difference.clone());
            if merged == *config {
                (difference, false)
            } else {
                (config.clone(), true)
            }
        }
        Some(_) => {
            // Another kind under this id in the base: remove it, add this.
            let remove = array(doc, "remove")?;
            if !remove.iter().any(|item| item.as_str() == Some(id)) {
                remove.push(id);
            }
            (config.clone(), false)
        }
        None => (config.clone(), false),
    };
    let mut table = Table::new();
    table["use"] = toml_edit::value(kind);
    if id != kind {
        table["id"] = toml_edit::value(id);
    }
    if replace {
        table["replace"] = toml_edit::value(true);
    }
    if let Some(toml_edit::Value::InlineTable(inline)) = to_toml(&entry_config) {
        table["config"] = toml_edit::value(inline);
    }
    entries(doc)?.push(table);
    Ok(())
}

/// The permissions mode a policy configuration's `allow` and `ask` are.
fn permissions_of(config: &Value) -> Option<Permissions> {
    let levels = |key: &str| -> Option<Vec<String>> {
        config.get(key)?.as_array().map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
    };
    let (allow, ask) = (levels("allow")?, levels("ask")?);
    Permissions::ALL
        .into_iter()
        .find(|mode| allow == mode.allow() && ask == mode.ask())
}

/// What `new` changes in `base`: tables key by key, anything else whole.
/// `None` when nothing changes. Keys `new` lacks are not in it.
fn difference(base: &Value, new: &Value) -> Option<Value> {
    match (base, new) {
        (Value::Object(base), Value::Object(new)) => {
            let mut changed = Map::new();
            for (key, value) in new {
                match base.get(key) {
                    Some(old) => {
                        if let Some(difference) = difference(old, value) {
                            changed.insert(key.clone(), difference);
                        }
                    }
                    None => {
                        changed.insert(key.clone(), value.clone());
                    }
                }
            }
            (!changed.is_empty()).then_some(Value::Object(changed))
        }
        (Value::Null, Value::Object(new)) if new.is_empty() => None,
        _ if base == new => None,
        _ => Some(new.clone()),
    }
}

/// A JSON value as a TOML value; `None` for null, which TOML cannot say.
fn to_toml(value: &Value) -> Option<toml_edit::Value> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(flag) => (*flag).into(),
        Value::Number(number) => match number.as_i64() {
            Some(integer) => integer.into(),
            None => number.as_f64()?.into(),
        },
        Value::String(text) => text.as_str().into(),
        Value::Array(items) => {
            let mut array = Array::new();
            for item in items.iter().filter_map(to_toml) {
                array.push(item);
            }
            array.into()
        }
        Value::Object(map) => {
            let mut table = InlineTable::new();
            for (key, item) in map {
                if let Some(item) = to_toml(item) {
                    table.insert(key, item);
                }
            }
            table.into()
        }
    })
}

#[cfg(test)]
mod tests {
    use rah_api::PluginId;
    use serde_json::json;

    use super::*;

    struct File(PathBuf);

    impl File {
        fn new(name: &str, text: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("rah-persist-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join("rah.toml");
            fs::write(&path, text).unwrap();
            Self(path)
        }

        fn text(&self) -> String {
            fs::read_to_string(&self.0).unwrap()
        }

        fn composed(&self) -> Distribution {
            compose(&self.text()).unwrap()
        }

        fn apply(&self, change: ConfigChange) -> Result<(), String> {
            persist(&self.0, &change, None)
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = fs::remove_dir_all(dir);
            }
        }
    }

    fn config_of(distribution: &Distribution, id: &str) -> Value {
        distribution
            .plugin
            .iter()
            .find(|spec| spec.plugin_id().as_str() == id)
            .map(|spec| spec.config.clone())
            .unwrap_or(Value::Null)
    }

    const START: &str = "# my notes stay\nmodel = \"mock\"\npermissions = \"ask\"\n";

    #[test]
    fn a_model_is_kept_as_the_model_key_and_its_options() {
        let file = File::new("model", START);
        file.apply(ConfigChange::Model {
            kind: "model-anthropic".into(),
            config: json!({ "model": "claude-x", "max_tokens": 1000 }),
        })
        .unwrap();
        let text = file.text();
        assert!(text.starts_with("# my notes stay\n"), "{text}");
        assert!(text.contains("model = \"anthropic/claude-x\""), "{text}");
        assert!(
            text.contains("[model_options]\nmax_tokens = 1000"),
            "{text}"
        );
        assert_eq!(
            config_of(&file.composed(), "model-anthropic"),
            json!({ "model": "claude-x", "max_tokens": 1000 })
        );
        // A preset key is part of the provider, not an option.
        file.apply(ConfigChange::Model {
            kind: "model-chat-completions".into(),
            config: json!({ "provider": "deepseek" }),
        })
        .unwrap();
        let text = file.text();
        assert!(text.contains("model = \"deepseek\""), "{text}");
        assert!(!text.contains("model_options"), "{text}");
    }

    #[test]
    fn a_policy_is_kept_as_permissions_and_only_what_differs() {
        let file = File::new("policy", START);
        let mut policy = config_of(&file.composed(), POLICY);
        policy["allow"] = json!(["none", "read", "write"]);
        policy["ask"] = json!(["execute"]);
        policy["commands"] = json!([["git", "status"]]);
        file.apply(ConfigChange::Plugin {
            id: PluginId::new(POLICY),
            kind: POLICY.into(),
            config: policy.clone(),
        })
        .unwrap();
        let text = file.text();
        assert!(text.contains("permissions = \"auto-edit\""), "{text}");
        assert!(
            text.contains("commands = [[\"git\", \"status\"]]"),
            "{text}"
        );
        // The base's assignments are not copied, so later ones still arrive.
        assert!(!text.contains("repo.create-file"), "{text}");
        assert_eq!(config_of(&file.composed(), POLICY), policy);

        // Saving what the file already composes leaves no entry behind.
        let composed = config_of(&file.composed(), POLICY);
        file.apply(ConfigChange::Plugin {
            id: PluginId::new(POLICY),
            kind: POLICY.into(),
            config: composed,
        })
        .unwrap();
        assert_eq!(file.text().matches("[[plugin]]").count(), 1);
    }

    #[test]
    fn a_dropped_key_makes_the_entry_replace() {
        let file = File::new("replace", START);
        let mut config = config_of(&file.composed(), "context-default");
        config.as_object_mut().unwrap().remove("memory_dir");
        file.apply(ConfigChange::Plugin {
            id: PluginId::new("context-default"),
            kind: "context-default".into(),
            config: config.clone(),
        })
        .unwrap();
        assert!(file.text().contains("replace = true"), "{}", file.text());
        assert_eq!(config_of(&file.composed(), "context-default"), config);
    }

    #[test]
    fn plugins_are_added_swapped_and_removed() {
        let file = File::new("plugins", START);
        file.apply(ConfigChange::Plugin {
            id: PluginId::new("api-a"),
            kind: "tools-http".into(),
            config: json!({ "base_url": "https://example.test" }),
        })
        .unwrap();
        assert_eq!(
            config_of(&file.composed(), "api-a"),
            json!({ "base_url": "https://example.test" })
        );
        // A loop of another kind: the base one is removed, the new one added.
        file.apply(ConfigChange::Removed {
            id: PluginId::new("loop-react"),
        })
        .unwrap();
        file.apply(ConfigChange::Plugin {
            id: PluginId::new("loop-plan"),
            kind: "loop-plan".into(),
            config: Value::Null,
        })
        .unwrap();
        let composed = file.composed();
        let kinds: Vec<&str> = composed.plugin.iter().map(|s| s.kind.as_str()).collect();
        assert!(
            kinds.contains(&"loop-plan") && !kinds.contains(&"loop-react"),
            "{kinds:?}"
        );
        // Running it again takes it out of `remove`.
        file.apply(ConfigChange::Plugin {
            id: PluginId::new("loop-react"),
            kind: "loop-react".into(),
            config: json!({ "max_rounds": 3 }),
        })
        .unwrap();
        assert!(!file.text().contains("\"loop-react\"]"), "{}", file.text());
        assert_eq!(
            config_of(&file.composed(), "loop-react"),
            json!({ "max_rounds": 3 })
        );
        // A plugin the file added goes away with its entry.
        file.apply(ConfigChange::Removed {
            id: PluginId::new("api-a"),
        })
        .unwrap();
        assert!(!file.text().contains("api-a"), "{}", file.text());
    }

    #[test]
    fn an_extra_model_plugin_is_an_entry_and_the_model_stays() {
        let file = File::new("extra-model", START);
        file.apply(ConfigChange::Plugin {
            id: PluginId::new("spare"),
            kind: "model-mock".into(),
            config: json!({ "label": "S" }),
        })
        .unwrap();
        let text = file.text();
        assert!(text.contains("model = \"mock\""), "{text}");
        assert_eq!(
            config_of(&file.composed(), "spare"),
            json!({ "label": "S" })
        );
        // It can go again; the model itself cannot.
        file.apply(ConfigChange::Removed {
            id: PluginId::new("spare"),
        })
        .unwrap();
        assert!(!file.text().contains("spare"));
        let error = file
            .apply(ConfigChange::Model {
                kind: "loop-react".into(),
                config: Value::Null,
            })
            .unwrap_err();
        assert!(error.contains("not a model plugin"), "{error}");
    }

    #[test]
    fn features_switch_on_and_off_in_any_form() {
        let file = File::new("features", "model = \"mock\"\nfeatures = [\"shell=git\"]\n");
        file.apply(ConfigChange::Feature {
            name: "web-search".into(),
            enabled: true,
        })
        .unwrap();
        file.apply(ConfigChange::Feature {
            name: "shell".into(),
            enabled: false,
        })
        .unwrap();
        assert!(
            file.text().contains("features = [\"web-search\"]"),
            "{}",
            file.text()
        );
        let error = file
            .apply(ConfigChange::Feature {
                name: "teleport".into(),
                enabled: true,
            })
            .unwrap_err();
        assert!(error.contains("unknown feature"), "{error}");
    }

    #[test]
    fn refusals_leave_the_file_as_it_was() {
        let whole = File::new("whole", "[[plugin]]\nuse = \"model-mock\"\n");
        let error = whole
            .apply(ConfigChange::Feature {
                name: "todo".into(),
                enabled: true,
            })
            .unwrap_err();
        assert!(error.contains("lists its plugins"), "{error}");
        assert_eq!(whole.text(), "[[plugin]]\nuse = \"model-mock\"\n");

        let file = File::new("refused", START);
        for id in [POLICY, "model-mock"] {
            let error = file
                .apply(ConfigChange::Removed {
                    id: PluginId::new(id),
                })
                .unwrap_err();
            assert!(error.contains("cannot be removed"), "{error}");
        }
        // A configuration that is not a table is refused.
        let error = file
            .apply(ConfigChange::Plugin {
                id: PluginId::new("loop-react"),
                kind: "loop-react".into(),
                config: json!("not a table"),
            })
            .unwrap_err();
        assert!(error.contains("must be a table"), "{error}");
        assert_eq!(file.text(), START);
    }

    #[test]
    fn a_data_path_resolved_at_start_is_not_written_out() {
        let file = File::new("data-home", START);
        let home = file.0.parent().unwrap().join("home");
        // The running plugin has the path the data home resolved.
        let mut config = config_of(&file.composed(), "context-default");
        crate::resolve_data_paths("context-default", &mut config, &home);
        assert_ne!(config, config_of(&file.composed(), "context-default"));
        persist(
            &file.0,
            &ConfigChange::Plugin {
                id: PluginId::new("context-default"),
                kind: "context-default".into(),
                config: config.clone(),
            },
            Some(&home),
        )
        .unwrap();
        assert_eq!(file.text(), START);
        // A real change is written relative to that, without the path.
        config["window"] = json!(12);
        persist(
            &file.0,
            &ConfigChange::Plugin {
                id: PluginId::new("context-default"),
                kind: "context-default".into(),
                config,
            },
            Some(&home),
        )
        .unwrap();
        let text = file.text();
        assert!(text.contains("config = { window = 12 }"), "{text}");
    }

    #[test]
    fn earlier_web_ui_settings_become_changes_that_apply() {
        let file = File::new("legacy", START);
        let dir = file
            .0
            .parent()
            .unwrap()
            .join("storage")
            .join("frontend-web");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("settings-model"),
            r#"{"kind":"model-mock","config":{"label":"OLD"}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("settings-loop"),
            r#"{"kind":"loop-plan","config":{"max_steps":3}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("plugins"),
            r#"{"api-a":{"state":"mounted","kind":"tools-http","config":{"base_url":"https://x.test"}},
                "scheduler-basic":{"state":"stopped","kind":"scheduler-basic"},
                "session-titles":{"state":"removed"}}"#,
        )
        .unwrap();
        fs::write(dir.join("web-search"), "on").unwrap();
        // UI state stays where it is.
        fs::write(dir.join("preferences"), "{}").unwrap();

        let found = legacy_changes(&dir).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["settings-model", "settings-loop", "plugins", "web-search"]
        );
        for (_, changes) in &found {
            for change in changes {
                file.apply(change.clone()).unwrap();
            }
        }
        let composed = file.composed();
        assert_eq!(
            config_of(&composed, "model-mock"),
            json!({ "label": "OLD" })
        );
        assert_eq!(config_of(&composed, "loop-plan"), json!({ "max_steps": 3 }));
        assert_eq!(
            config_of(&composed, "api-a"),
            json!({ "base_url": "https://x.test" })
        );
        let kinds: Vec<&str> = composed.plugin.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"tools-web-search"), "{kinds:?}");
        assert!(kinds.contains(&"scheduler-basic"), "{kinds:?}");
        assert!(
            !kinds.contains(&"session-titles") && !kinds.contains(&"loop-react"),
            "{kinds:?}"
        );

        fs::write(dir.join("settings-tools"), "not json").unwrap();
        assert!(legacy_changes(&dir).unwrap_err().contains("settings-tools"));
    }

    #[test]
    fn differences_are_table_wise() {
        assert_eq!(difference(&json!({ "a": 1 }), &json!({ "a": 1 })), None);
        assert_eq!(
            difference(
                &json!({ "t": { "x": 1, "y": 2 }, "l": [1] }),
                &json!({ "t": { "x": 1, "y": 3 }, "l": [1, 2], "n": true })
            ),
            Some(json!({ "t": { "y": 3 }, "l": [1, 2], "n": true }))
        );
        assert_eq!(difference(&Value::Null, &json!({})), None);
    }
}
