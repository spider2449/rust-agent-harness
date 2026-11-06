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

    /// The `root` of the user keys, applied again after features so that the
    /// workspace plugins they mount get it too.
    pub fn user_root(&self) -> Option<&Path> {
        self.user
            .as_ref()
            .and_then(|user| user.root.as_deref())
            .map(Path::new)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
