//! User-facing configuration (ADR 0067): `model`, `model_options`,
//! `permissions` and `root` describe what the user wants, and are compiled to
//! plugins and their configuration here. Nothing in this module adds behavior
//! of its own: it only edits a [`Distribution`] before boot.

use std::{fmt, path::Path, str::FromStr};

use serde_json::{Map, Value};

use crate::{Distribution, PluginSpec};

/// A model provider a `model = "<provider>/<id>"` value can name.
#[derive(Debug)]
pub struct Provider {
    pub name: &'static str,
    /// The model plugin kind it mounts.
    pub kind: &'static str,
    /// Configuration the kind needs for this provider, besides `model`.
    preset: &'static [(&'static str, &'static str)],
    /// Whether the plugin takes a `model` key.
    takes_id: bool,
}

/// Every provider, in the order messages list them.
pub const PROVIDERS: [Provider; 6] = [
    Provider {
        name: "anthropic",
        kind: "model-anthropic",
        preset: &[],
        takes_id: true,
    },
    Provider {
        name: "openai",
        kind: "model-openai",
        preset: &[],
        takes_id: true,
    },
    Provider {
        name: "deepseek",
        kind: "model-chat-completions",
        preset: &[("provider", "deepseek")],
        takes_id: true,
    },
    // Any Chat Completions server; `model_options.base_url` names it.
    Provider {
        name: "openai-compatible",
        kind: "model-chat-completions",
        preset: &[],
        takes_id: true,
    },
    Provider {
        name: "llamacpp",
        kind: "model-llamacpp",
        preset: &[],
        takes_id: true,
    },
    Provider {
        name: "mock",
        kind: "model-mock",
        preset: &[],
        takes_id: false,
    },
];

/// Whether `kind` is a model plugin a provider mounts.
pub fn is_model_kind(kind: &str) -> bool {
    PROVIDERS.iter().any(|provider| provider.kind == kind)
}

fn provider_names() -> String {
    PROVIDERS
        .iter()
        .map(|provider| provider.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The model plugin kind and configuration for `<provider>[/<model id>]`.
/// Without an id, the plugin's own default model is used. An id may contain
/// `/` itself (`llamacpp/org/model`).
pub fn model_plugin(spec: &str) -> Result<(&'static str, Map<String, Value>), String> {
    let spec = spec.trim();
    let (name, id) = spec.split_once('/').unwrap_or((spec, ""));
    let provider = PROVIDERS
        .iter()
        .find(|provider| provider.name == name)
        .ok_or_else(|| {
            format!(
                "unknown model provider `{name}` in `{spec}`; use `<provider>/<model id>` \
                 with a provider of: {}",
                provider_names()
            )
        })?;
    let mut config: Map<String, Value> = provider
        .preset
        .iter()
        .map(|(key, value)| ((*key).to_owned(), Value::String((*value).to_owned())))
        .collect();
    let id = id.trim();
    if !id.is_empty() {
        if !provider.takes_id {
            return Err(format!(
                "provider `{}` takes no model id; use `{}`",
                provider.name, provider.name
            ));
        }
        config.insert("model".to_owned(), Value::String(id.to_owned()));
    }
    Ok((provider.kind, config))
}

/// How much the policy lets tools do without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permissions {
    /// Reading only; writing and running programs are denied.
    ReadOnly,
    /// Reading runs; writing and running programs ask first.
    Ask,
    /// Reading and writing run; running programs asks first.
    AutoEdit,
    /// Every assigned tool runs without asking.
    Full,
}

impl Permissions {
    pub const ALL: [Permissions; 4] = [Self::ReadOnly, Self::Ask, Self::AutoEdit, Self::Full];

    pub fn name(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Ask => "ask",
            Self::AutoEdit => "auto-edit",
            Self::Full => "full",
        }
    }

    /// The policy's `allow` levels.
    fn allow(self) -> &'static [&'static str] {
        match self {
            Self::ReadOnly | Self::Ask => &["none", "read"],
            Self::AutoEdit => &["none", "read", "write"],
            Self::Full => &["none", "read", "write", "execute"],
        }
    }

    /// The policy's `ask` levels; a level in neither list is denied.
    fn ask(self) -> &'static [&'static str] {
        match self {
            Self::ReadOnly | Self::Full => &[],
            Self::Ask => &["write", "execute"],
            Self::AutoEdit => &["execute"],
        }
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Permissions {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == text.trim())
            .ok_or_else(|| {
                let names: Vec<&str> = Self::ALL.iter().map(|mode| mode.name()).collect();
                format!(
                    "unknown permissions `{text}`; use one of: {}",
                    names.join(", ")
                )
            })
    }
}

/// The user keys a distribution file set, as written, for `--show-config`
/// and to apply `root` again after features.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UserSettings {
    pub model: Option<String>,
    pub permissions: Option<Permissions>,
    pub root: Option<String>,
}

fn strings(values: &[&str]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|value| Value::String((*value).to_owned()))
            .collect(),
    )
}

impl Distribution {
    /// Mounts the model `spec` (`<provider>[/<id>]`) with `options` merged
    /// into its configuration. A model plugin of the same kind keeps the rest
    /// of its configuration; one of another kind is swapped in its position;
    /// without one, the model is added before the loop.
    pub fn set_model(&mut self, spec: &str, options: &Map<String, Value>) -> Result<(), String> {
        let (kind, mut config) = model_plugin(spec)?;
        for (key, value) in options {
            config.insert(key.clone(), value.clone());
        }
        let existing = self
            .plugin
            .iter_mut()
            .find(|spec| is_model_kind(&spec.kind));
        match existing {
            Some(spec) if spec.kind == kind => {
                if !spec.config.is_object() {
                    spec.config = Value::Object(Map::new());
                }
                if let Value::Object(map) = &mut spec.config {
                    map.extend(config);
                }
            }
            Some(spec) => {
                spec.kind = kind.to_owned();
                spec.id = None;
                spec.config = Value::Object(config);
                spec.bind.clear();
            }
            None => {
                let at = self
                    .plugin
                    .iter()
                    .position(|spec| {
                        spec.kind.starts_with("loop-") || spec.kind.starts_with("frontend-")
                    })
                    .unwrap_or(self.plugin.len());
                let config = if config.is_empty() {
                    Value::Null
                } else {
                    Value::Object(config)
                };
                self.plugin.insert(
                    at,
                    PluginSpec {
                        kind: kind.to_owned(),
                        id: None,
                        config,
                        bind: Default::default(),
                    },
                );
            }
        }
        Ok(())
    }

    /// Whether a model plugin a provider mounts is in the distribution.
    pub fn has_model(&self) -> bool {
        self.plugin.iter().any(|spec| is_model_kind(&spec.kind))
    }

    /// Sets `allow` and `ask` of every mounted policy to `permissions`. The
    /// permission assignments, decisions and approved commands are kept, so
    /// a tool without an assignment is still denied. Fails without a policy:
    /// nothing would enforce the mode.
    pub fn set_permissions(&mut self, permissions: Permissions) -> Result<(), String> {
        let mut matched = false;
        for spec in self
            .plugin
            .iter_mut()
            .filter(|spec| spec.kind == "policy-authority")
        {
            if !spec.config.is_object() {
                spec.config = Value::Object(Map::new());
            }
            if let Value::Object(config) = &mut spec.config {
                config.insert("allow".to_owned(), strings(permissions.allow()));
                config.insert("ask".to_owned(), strings(permissions.ask()));
                matched = true;
            }
        }
        if !matched {
            return Err(format!(
                "permissions `{permissions}` need a `policy-authority` plugin in the distribution"
            ));
        }
        self.permissions = Some(permissions);
        Ok(())
    }

    /// The model as `<provider>/<id>` (or `<provider>`), read back from the
    /// model plugin; the plugin kind when no provider matches its
    /// configuration; `None` without a model plugin.
    pub fn model_label(&self) -> Option<String> {
        let spec = self.plugin.iter().find(|spec| is_model_kind(&spec.kind))?;
        let matches = |provider: &&Provider| {
            provider.kind == spec.kind
                && provider.preset.iter().all(|(key, value)| {
                    spec.config.get(*key).and_then(Value::as_str) == Some(*value)
                })
        };
        // A provider with a preset is more specific than one without.
        let provider = PROVIDERS
            .iter()
            .filter(matches)
            .max_by_key(|provider| provider.preset.len());
        let Some(provider) = provider else {
            return Some(spec.kind.clone());
        };
        Some(match spec.config.get("model").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => format!("{}/{id}", provider.name),
            _ => provider.name.to_owned(),
        })
    }

    /// The permissions mode of the first policy: the one last applied, else
    /// the mode its `allow` and `ask` match; `custom` when none does and
    /// `none` without a policy.
    pub fn permissions_label(&self) -> String {
        let Some(policy) = self
            .plugin
            .iter()
            .find(|spec| spec.kind == "policy-authority")
        else {
            return "none (no policy: UNGUARDED)".to_owned();
        };
        if let Some(permissions) = self.permissions {
            return permissions.name().to_owned();
        }
        let levels = |key: &str, default: &[&str]| -> Vec<String> {
            match policy.config.get(key).and_then(Value::as_array) {
                Some(list) => list
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                None => default.iter().map(|level| (*level).to_owned()).collect(),
            }
        };
        // The policy's own defaults are those of `ask`.
        let allow = levels("allow", Permissions::Ask.allow());
        let ask = levels("ask", Permissions::Ask.ask());
        Permissions::ALL
            .into_iter()
            .find(|mode| allow == mode.allow() && ask == mode.ask())
            .map_or_else(|| "custom".to_owned(), |mode| mode.name().to_owned())
    }

    /// The workspace root of the first workspace plugin that has one.
    pub fn root_label(&self) -> Option<String> {
        crate::WORKSPACE_PLUGINS.iter().find_map(|kind| {
            self.plugin
                .iter()
                .find(|spec| spec.kind == *kind)
                .and_then(|spec| spec.config.get("root"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
    }

    /// The `root` of the user keys, applied again after features so that the
    /// workspace plugins they mount get it too.
    pub fn user_root(&self) -> Option<&Path> {
        self.user
            .as_ref()
            .and_then(|user| user.root.as_deref())
            .map(Path::new)
    }
}

/// A configuration file with only user keys, as `rah config init` writes it.
/// The values are checked first, so the file it writes always parses.
pub fn user_file_text(
    model: &str,
    permissions: Permissions,
    root: Option<&str>,
    features: &[String],
) -> Result<String, String> {
    model_plugin(model)?;
    let quote = |text: &str| Value::String(text.to_owned()).to_string();
    let mut text = String::from(
        "# RAH configuration, written by `rah config init`.\n\
         # `rah config show` prints what runs; docs/CONFIGURATION.md lists every key.\n\
         # Command-line options override these values for one run.\n\n",
    );
    text.push_str("# <provider>/<model id>: anthropic, openai, deepseek, openai-compatible,\n");
    text.push_str("# llamacpp or mock. API keys stay in environment variables.\n");
    text.push_str(&format!("model = {}\n\n", quote(model)));
    text.push_str("# read-only | ask | auto-edit | full\n");
    text.push_str(&format!("permissions = {}\n", quote(permissions.name())));
    match root {
        Some(root) => text.push_str(&format!("\nroot = {}\n", quote(root))),
        None => text.push_str("\n# Workspace folder; the current directory when not set.\n# root = \"D:/work/project\"\n"),
    }
    let features: Vec<String> = features.iter().map(|feature| quote(feature)).collect();
    text.push_str(&format!(
        "\n# Opt-in capabilities; `rah features` lists them.\nfeatures = [{}]\n",
        features.join(", ")
    ));
    Ok(text)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn labels_read_the_composition_back() {
        let mut distribution = Distribution::preset("deepseek").unwrap();
        assert_eq!(distribution.model_label().as_deref(), Some("deepseek"));
        assert_eq!(distribution.permissions_label(), "ask");
        assert_eq!(distribution.root_label().as_deref(), Some("."));
        distribution
            .set_model("openai-compatible/m", &Map::new())
            .unwrap();
        // The `provider` key stays, so it still reads as DeepSeek.
        assert_eq!(distribution.model_label().as_deref(), Some("deepseek/m"));
        distribution.set_model("anthropic/x", &Map::new()).unwrap();
        assert_eq!(distribution.model_label().as_deref(), Some("anthropic/x"));
        distribution.set_permissions(Permissions::Full).unwrap();
        assert_eq!(distribution.permissions_label(), "full");

        let custom = Distribution::parse(
            "extends = \"dev\"\n[[plugin]]\nuse = \"policy-authority\"\nconfig = { allow = [\"none\"] }\n",
        )
        .unwrap();
        assert_eq!(custom.permissions_label(), "custom");
        let auto = Distribution::parse(
            "extends = \"dev\"\n[[plugin]]\nuse = \"policy-authority\"\n\
             config = { allow = [\"none\", \"read\", \"write\"], ask = [\"execute\"] }\n",
        )
        .unwrap();
        assert_eq!(auto.permissions_label(), "auto-edit");
        let minimal = Distribution::preset("minimal").unwrap();
        assert!(minimal.permissions_label().contains("UNGUARDED"));
    }

    #[test]
    fn the_init_file_parses_back_to_its_values() {
        let text = user_file_text(
            "anthropic/claude-sonnet-5-5",
            Permissions::AutoEdit,
            Some("D:\\work \"x\""),
            &["web-search".to_owned(), "shell=git".to_owned()],
        )
        .unwrap();
        let distribution = Distribution::parse(&text).unwrap();
        assert_eq!(
            distribution.user,
            Some(UserSettings {
                model: Some("anthropic/claude-sonnet-5-5".into()),
                permissions: Some(Permissions::AutoEdit),
                root: Some("D:\\work \"x\"".into()),
            })
        );
        assert_eq!(distribution.features, ["web-search", "shell=git"]);
        let bare = user_file_text("mock", Permissions::Ask, None, &[]).unwrap();
        let distribution = Distribution::parse(&bare).unwrap();
        assert_eq!(distribution.user.unwrap().root, None);
        assert!(user_file_text("nope/x", Permissions::Ask, None, &[]).is_err());
    }

    fn spec<'a>(distribution: &'a Distribution, kind: &str) -> &'a PluginSpec {
        distribution
            .plugin
            .iter()
            .find(|spec| spec.kind == kind)
            .unwrap()
    }

    #[test]
    fn every_provider_maps_to_its_plugin() {
        let cases = [
            (
                "anthropic/claude-sonnet-5-5",
                "model-anthropic",
                json!({ "model": "claude-sonnet-5-5" }),
            ),
            ("openai/gpt-x", "model-openai", json!({ "model": "gpt-x" })),
            (
                "deepseek/deepseek-reasoner",
                "model-chat-completions",
                json!({ "provider": "deepseek", "model": "deepseek-reasoner" }),
            ),
            (
                "openai-compatible/m",
                "model-chat-completions",
                json!({ "model": "m" }),
            ),
            (
                "llamacpp/org/model",
                "model-llamacpp",
                json!({ "model": "org/model" }),
            ),
            ("llamacpp", "model-llamacpp", json!({})),
            ("anthropic/", "model-anthropic", json!({})),
            ("mock", "model-mock", json!({})),
        ];
        for (text, kind, config) in cases {
            let (got_kind, got_config) = model_plugin(text).unwrap();
            assert_eq!(got_kind, kind, "{text}");
            assert_eq!(Value::Object(got_config), config, "{text}");
        }
    }

    #[test]
    fn bad_model_values_are_refused_with_the_choices() {
        let unknown = model_plugin("gemini/x").unwrap_err();
        assert!(
            unknown.contains("unknown model provider `gemini`"),
            "{unknown}"
        );
        assert!(
            unknown.contains("anthropic, openai, deepseek, openai-compatible, llamacpp, mock"),
            "{unknown}"
        );
        let bare = model_plugin("claude-opus-5-5").unwrap_err();
        assert!(bare.contains("<provider>/<model id>"), "{bare}");
        let mock = model_plugin("mock/x").unwrap_err();
        assert!(mock.contains("takes no model id"), "{mock}");
    }

    #[test]
    fn set_model_swaps_keeps_or_adds() {
        // Another kind: swapped in place, with the new configuration only.
        let mut distribution = Distribution::preset("default").unwrap();
        let at = distribution
            .plugin
            .iter()
            .position(|spec| spec.kind == "model-llamacpp")
            .unwrap();
        distribution
            .set_model("anthropic/claude-sonnet-5-5", &Map::new())
            .unwrap();
        assert_eq!(distribution.plugin[at].kind, "model-anthropic");
        assert_eq!(
            distribution.plugin[at].config,
            json!({ "model": "claude-sonnet-5-5" })
        );
        assert!(
            !distribution
                .plugin
                .iter()
                .any(|s| s.kind == "model-llamacpp")
        );

        // The same kind keeps its other keys.
        let mut options = Map::new();
        options.insert("max_tokens".into(), json!(1000));
        distribution.set_model("anthropic", &options).unwrap();
        distribution
            .set_model("anthropic/claude-opus-5-5", &Map::new())
            .unwrap();
        assert_eq!(
            spec(&distribution, "model-anthropic").config,
            json!({ "model": "claude-opus-5-5", "max_tokens": 1000 })
        );

        // Without a model plugin: added before the loop.
        let mut bare = Distribution::parse(
            "[[plugin]]\nuse = \"storage-fs\"\n[[plugin]]\nuse = \"loop-react\"\n",
        )
        .unwrap();
        assert!(!bare.has_model());
        bare.set_model("mock", &Map::new()).unwrap();
        let kinds: Vec<&str> = bare.plugin.iter().map(|s| s.kind.as_str()).collect();
        assert_eq!(kinds, ["storage-fs", "model-mock", "loop-react"]);
        assert!(bare.has_model());
    }

    #[test]
    fn permissions_set_allow_and_ask_and_keep_assignments() {
        let expected = [
            (Permissions::ReadOnly, json!(["none", "read"]), json!([])),
            (
                Permissions::Ask,
                json!(["none", "read"]),
                json!(["write", "execute"]),
            ),
            (
                Permissions::AutoEdit,
                json!(["none", "read", "write"]),
                json!(["execute"]),
            ),
            (
                Permissions::Full,
                json!(["none", "read", "write", "execute"]),
                json!([]),
            ),
        ];
        for (mode, allow, ask) in expected {
            let mut distribution = Distribution::preset("dev").unwrap();
            distribution.set_permissions(mode).unwrap();
            let policy = &spec(&distribution, "policy-authority").config;
            assert_eq!(policy["allow"], allow, "{mode}");
            assert_eq!(policy["ask"], ask, "{mode}");
            // Assignments stay: an unassigned tool is still denied.
            assert_eq!(policy["tools"]["repo.create-file"], "write", "{mode}");
            assert_eq!(distribution.permissions, Some(mode));
        }
        assert_eq!(
            "auto-edit".parse::<Permissions>(),
            Ok(Permissions::AutoEdit)
        );
        let error = "yolo".parse::<Permissions>().unwrap_err();
        assert!(error.contains("read-only, ask, auto-edit, full"), "{error}");
        let mut unguarded = Distribution::preset("minimal").unwrap();
        let error = unguarded.set_permissions(Permissions::Ask).unwrap_err();
        assert!(error.contains("policy-authority"), "{error}");
    }
}
