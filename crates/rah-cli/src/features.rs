//! Features: the opt-in capabilities a user wants, such as program execution
//! or web search, each described once as the plugins to mount and the policy
//! entries they need. A feature is applied to a [`Distribution`] by name, from
//! `--with <feature>[=<args>]`, from `features = [...]` in `rah.toml`, or
//! both, so a user never edits several plugins by hand.
//!
//! Applying a feature only edits the distribution before boot. It adds no
//! behavior of its own, grants nothing the policy does not ask about, and is
//! idempotent: a plugin that is already mounted stays, and a policy entry that
//! already exists is kept.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::{Distribution, PluginSpec};

/// Where a feature's plugin is inserted in the plugin list.
#[derive(Clone, Copy, Debug)]
enum Place {
    /// Right after the last sandbox plugin, which its tools start through.
    AfterSandbox,
    /// Before the agent loop, which is where tool and context plugins sit.
    BeforeLoop,
}

/// What a feature does to the plugin list.
#[derive(Clone, Copy, Debug)]
enum Mount {
    /// Mounts `kind` unless it is already mounted.
    Plugin { kind: &'static str, place: Place },
    /// Swaps a mounted plugin for another one.
    Replace {
        from: &'static str,
        to: &'static str,
    },
}

/// One opt-in capability.
#[derive(Debug)]
pub struct Feature {
    pub name: &'static str,
    pub summary: &'static str,
    /// Usage of the argument, when the feature takes one.
    pub argument: Option<&'static str>,
    /// At least one of these plugin kinds must be mounted, when not empty.
    needs_any: &'static [&'static str],
    mount: Mount,
    /// The configuration key the comma-separated argument fills, as a list.
    argument_key: Option<&'static str>,
    /// Policy permission assignments, `(tool pattern, level)`.
    tools: &'static [(&'static str, &'static str)],
    /// Policy decisions, `(tool pattern, decision)`.
    decisions: &'static [(&'static str, &'static str)],
}

/// Every feature, in the order `rah --list-features` shows them.
pub const FEATURES: [Feature; 7] = [
    Feature {
        name: "shell",
        summary: "run programs (`shell.exec`); the policy asks before each call",
        argument: Some("programs, such as git,cargo; any program when omitted"),
        needs_any: &["sandbox-process", "sandbox-os"],
        mount: Mount::Plugin {
            kind: "tools-shell",
            place: Place::AfterSandbox,
        },
        argument_key: Some("allow_programs"),
        tools: &[("shell.exec", "execute")],
        decisions: &[],
    },
    Feature {
        name: "web-search",
        summary: "web search and page reading (`web.*`); the policy asks before each call",
        argument: None,
        needs_any: &[],
        mount: Mount::Plugin {
            kind: "tools-web-search",
            place: Place::BeforeLoop,
        },
        argument_key: None,
        tools: &[("web.*", "read")],
        decisions: &[("web.*", "ask")],
    },
    Feature {
        name: "format",
        summary: "run the project's formatter on files the write tools changed",
        argument: None,
        needs_any: &["sandbox-process", "sandbox-os"],
        mount: Mount::Plugin {
            kind: "format-on-write",
            place: Place::AfterSandbox,
        },
        argument_key: None,
        tools: &[],
        decisions: &[],
    },
    Feature {
        name: "lsp",
        summary: "append language server diagnostics to the output of file-writing tools",
        argument: None,
        needs_any: &["sandbox-process", "sandbox-os"],
        mount: Mount::Plugin {
            kind: "lsp-diagnostics",
            place: Place::AfterSandbox,
        },
        argument_key: None,
        tools: &[],
        decisions: &[],
    },
    Feature {
        name: "compact",
        summary: "summarize old history with the model instead of dropping it",
        argument: None,
        needs_any: &[],
        mount: Mount::Plugin {
            kind: "context-compact",
            place: Place::BeforeLoop,
        },
        argument_key: None,
        tools: &[],
        decisions: &[],
    },
    Feature {
        name: "telemetry",
        summary: "write one JSON line per turn, model request and tool call",
        argument: None,
        needs_any: &[],
        mount: Mount::Plugin {
            kind: "telemetry-log",
            place: Place::BeforeLoop,
        },
        argument_key: None,
        tools: &[],
        decisions: &[],
    },
    Feature {
        name: "plan",
        summary: "plan first, then execute step by step (`loop-plan` for `loop-react`)",
        argument: None,
        needs_any: &[],
        mount: Mount::Replace {
            from: "loop-react",
            to: "loop-plan",
        },
        argument_key: None,
        tools: &[],
        decisions: &[],
    },
];

/// The feature called `name`.
pub fn feature(name: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|feature| feature.name == name)
}

/// The feature names, for messages.
fn names() -> String {
    FEATURES
        .iter()
        .map(|feature| feature.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A feature that was applied to a distribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The request as given, such as `shell=git`.
    pub request: String,
    /// The plugin kind it newly mounted, when it did.
    pub mounted: Option<String>,
}

/// A feature request: `name`, or `name=a,b` with an argument.
pub fn parse_request(request: &str) -> (&str, Vec<String>) {
    let (name, argument) = request.split_once('=').unwrap_or((request, ""));
    let arguments = argument
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    (name.trim(), arguments)
}

/// The tool names of the `--approve` values: each value is a name or a comma
/// separated list. A name must be exact, so no value can approve every tool.
pub fn approved_tools(requests: &[String]) -> Result<Vec<String>, String> {
    let mut tools: Vec<String> = Vec::new();
    for name in requests.iter().flat_map(|request| request.split(',')) {
        let name = name.trim();
        if name.is_empty() || name.contains('*') {
            return Err(format!(
                "--approve takes exact tool names, such as `repo.create-file`, not `{name}`"
            ));
        }
        if !tools.iter().any(|known| known == name) {
            tools.push(name.to_owned());
        }
    }
    Ok(tools)
}

impl Distribution {
    /// Applies the feature request `request` (`name` or `name=a,b`).
    pub fn enable(&mut self, request: &str) -> Result<(), String> {
        let (name, arguments) = parse_request(request);
        let feature = feature(name)
            .ok_or_else(|| format!("unknown feature `{name}`; available: {}", names()))?;
        let mounted = feature.apply(self, &arguments)?;
        let applied = Applied {
            request: request.trim().to_owned(),
            mounted: mounted.map(str::to_owned),
        };
        if !self
            .applied
            .iter()
            .any(|done| done.request == applied.request)
        {
            self.applied.push(applied);
        }
        Ok(())
    }

    /// Lets the tools `requests` (`--approve`, each a name or a comma
    /// separated list) run without asking, by adding them to `approve` of
    /// every mounted policy (ADR 0054). Returns the tool names. It fails for
    /// a name that is empty or a pattern, and when no policy is mounted:
    /// nothing asks without one, so the option would mean nothing.
    pub fn approve(&mut self, requests: &[String]) -> Result<Vec<String>, String> {
        let tools = approved_tools(requests)?;
        let mut matched = false;
        for spec in self
            .plugin
            .iter_mut()
            .filter(|spec| spec.kind == "policy-authority")
        {
            if !spec.config.is_object() {
                spec.config = Value::Object(Map::new());
            }
            let Value::Object(config) = &mut spec.config else {
                continue;
            };
            let approved = config
                .entry("approve")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(approved) = approved {
                for tool in &tools {
                    if !approved.iter().any(|known| known == tool.as_str()) {
                        approved.push(Value::String(tool.clone()));
                    }
                }
                matched = true;
            }
        }
        if matched {
            Ok(tools)
        } else {
            Err("--approve needs a `policy-authority` plugin in the distribution".into())
        }
    }

    /// Applies every feature `features` lists in the distribution file, then
    /// forgets them, so a feature is never applied twice.
    pub fn apply_features(&mut self) -> Result<(), String> {
        for request in std::mem::take(&mut self.features) {
            self.enable(&request)
                .map_err(|error| format!("features: {error}"))?;
        }
        Ok(())
    }
}

impl Feature {
    /// Applies the feature; returns the plugin kind it newly mounted, if any.
    fn apply(
        &self,
        distribution: &mut Distribution,
        arguments: &[String],
    ) -> Result<Option<&'static str>, String> {
        if !arguments.is_empty() && self.argument_key.is_none() {
            return Err(format!("feature `{}` takes no argument", self.name));
        }
        if !self.needs_any.is_empty()
            && !distribution
                .plugin
                .iter()
                .any(|spec| self.needs_any.contains(&spec.kind.as_str()))
        {
            return Err(format!(
                "feature `{}` needs a {} plugin in the distribution",
                self.name,
                self.needs_any
                    .iter()
                    .map(|kind| format!("`{kind}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
        }
        // The policy entries go in first, so the tools are never mounted
        // without them.
        self.assign_policy(distribution);
        match self.mount {
            Mount::Plugin { kind, place } => {
                Ok(self.mount_plugin(distribution, kind, place, arguments))
            }
            Mount::Replace { from, to } => {
                if distribution.replace(from, to) {
                    Ok(Some(to))
                } else if distribution.plugin.iter().any(|spec| spec.kind == to) {
                    Ok(None)
                } else {
                    Err(format!(
                        "feature `{}` needs a `{from}` plugin to replace",
                        self.name
                    ))
                }
            }
        }
    }

    /// Mounts `kind` unless it is mounted; returns it when newly mounted.
    fn mount_plugin(
        &self,
        distribution: &mut Distribution,
        kind: &'static str,
        place: Place,
        arguments: &[String],
    ) -> Option<&'static str> {
        let argument = self
            .argument_key
            .filter(|_| !arguments.is_empty())
            .map(|key| {
                let list = arguments.iter().cloned().map(Value::String).collect();
                (key, Value::Array(list))
            });
        if let Some(spec) = distribution
            .plugin
            .iter_mut()
            .find(|spec| spec.kind == kind)
        {
            if let Some((key, value)) = argument {
                set_key(&mut spec.config, key, value);
            }
            return None;
        }
        let mut spec = PluginSpec {
            kind: kind.to_owned(),
            id: None,
            config: Value::Null,
            bind: BTreeMap::new(),
        };
        if let Some((key, value)) = argument {
            set_key(&mut spec.config, key, value);
        }
        let at = match place {
            Place::AfterSandbox => distribution
                .plugin
                .iter()
                .rposition(|spec| spec.kind.starts_with("sandbox-"))
                .map(|index| index + 1),
            Place::BeforeLoop => distribution.plugin.iter().position(|spec| {
                spec.kind.starts_with("loop-") || spec.kind.starts_with("frontend-")
            }),
        };
        let at = at.unwrap_or(distribution.plugin.len());
        distribution.plugin.insert(at, spec);
        Some(kind)
    }

    /// Adds this feature's entries to every policy plugin; an entry the
    /// policy already has is kept.
    fn assign_policy(&self, distribution: &mut Distribution) {
        for spec in distribution
            .plugin
            .iter_mut()
            .filter(|spec| spec.kind == "policy-authority")
        {
            for (key, entries) in [("tools", self.tools), ("decisions", self.decisions)] {
                if entries.is_empty() {
                    continue;
                }
                if !spec.config.is_object() {
                    spec.config = Value::Object(Map::new());
                }
                let Value::Object(config) = &mut spec.config else {
                    continue;
                };
                let table = config
                    .entry(key)
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(table) = table {
                    for (pattern, value) in entries {
                        table
                            .entry(*pattern)
                            .or_insert_with(|| Value::String((*value).to_owned()));
                    }
                }
            }
        }
    }
}

/// Sets `key` in `config`, turning a non-object into an empty object first.
fn set_key(config: &mut Value, key: &str, value: Value) {
    if !config.is_object() {
        *config = Value::Object(Map::new());
    }
    if let Value::Object(map) = config {
        map.insert(key.to_owned(), value);
    }
}

/// The text of `rah --list-features`.
pub fn list() -> String {
    let mut text = String::from(
        "Features (--with <name> on rah or rah-desktop, or features = [...] in rah.toml):\n",
    );
    for feature in &FEATURES {
        text.push_str(&format!("  {:<11} {}\n", feature.name, feature.summary));
        if let Some(argument) = feature.argument {
            text.push_str(&format!("  {:<11} =<{argument}>\n", ""));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(distribution: &Distribution) -> Vec<&str> {
        distribution
            .plugin
            .iter()
            .map(|spec| spec.kind.as_str())
            .collect()
    }

    fn policy(distribution: &Distribution) -> &Value {
        &distribution
            .plugin
            .iter()
            .find(|spec| spec.kind == "policy-authority")
            .unwrap()
            .config
    }

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn approve_adds_exact_tool_names_to_the_policy_once() {
        let mut distribution = Distribution::preset("dev").unwrap();
        let tools = distribution
            .approve(&names(&[
                "repo.create-file, repo.edit-files",
                "repo.create-file",
            ]))
            .unwrap();
        assert_eq!(tools, ["repo.create-file", "repo.edit-files"]);
        assert_eq!(
            policy(&distribution)["approve"],
            serde_json::json!(["repo.create-file", "repo.edit-files"])
        );
        // Again, and with a name the policy already lists: nothing is added.
        distribution.approve(&names(&["repo.edit-files"])).unwrap();
        assert_eq!(
            policy(&distribution)["approve"],
            serde_json::json!(["repo.create-file", "repo.edit-files"])
        );
        // The permission assignments are untouched.
        assert!(policy(&distribution)["tools"]["repo.create-file"] == "write");
    }

    #[test]
    fn approve_refuses_patterns_empty_names_and_a_missing_policy() {
        for bad in ["*", "repo.*", "", "repo.create-file,", " "] {
            let mut distribution = Distribution::preset("dev").unwrap();
            let error = distribution.approve(&names(&[bad])).unwrap_err();
            assert!(error.contains("exact tool names"), "{bad}: {error}");
            assert!(policy(&distribution).get("approve").is_none(), "{bad}");
        }
        let mut bare = Distribution::parse("[[plugin]]\nuse = \"loop-react\"\n").unwrap();
        let error = bare.approve(&names(&["repo.create-file"])).unwrap_err();
        assert!(error.contains("needs a `policy-authority`"), "{error}");
    }

    #[test]
    fn shell_mounts_after_the_sandbox_and_assigns_the_policy() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("shell=git, cargo").unwrap();
        let kinds = kinds(&distribution);
        let sandbox = kinds.iter().position(|k| *k == "sandbox-process").unwrap();
        assert_eq!(kinds[sandbox + 1], "tools-shell");
        assert_eq!(policy(&distribution)["tools"]["shell.exec"], "execute");
        let shell = &distribution.plugin[sandbox + 1];
        assert_eq!(
            shell.config["allow_programs"],
            serde_json::json!(["git", "cargo"])
        );
    }

    #[test]
    fn applying_a_feature_twice_changes_nothing_more() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("web-search").unwrap();
        let once = distribution.clone();
        distribution.enable("web-search").unwrap();
        assert_eq!(distribution, once);
        assert_eq!(policy(&distribution)["decisions"]["web.*"], "ask");
        assert_eq!(policy(&distribution)["tools"]["web.*"], "read");
    }

    #[test]
    fn a_policy_entry_the_user_wrote_is_kept() {
        let mut distribution = Distribution::parse(
            r#"
[[plugin]]
use = "policy-authority"
config = { tools = { "shell.exec" = "write" } }

[[plugin]]
use = "sandbox-process"
"#,
        )
        .unwrap();
        distribution.enable("shell").unwrap();
        assert_eq!(policy(&distribution)["tools"]["shell.exec"], "write");
    }

    #[test]
    fn plugins_land_before_the_loop() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("telemetry").unwrap();
        distribution.enable("compact").unwrap();
        let kinds = kinds(&distribution);
        let loop_at = kinds.iter().position(|k| *k == "loop-react").unwrap();
        assert!(kinds[..loop_at].contains(&"telemetry-log"));
        assert!(kinds[..loop_at].contains(&"context-compact"));
    }

    #[test]
    fn lsp_mounts_after_the_sandbox_and_needs_one() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("lsp").unwrap();
        let kinds = kinds(&distribution);
        let sandbox = kinds.iter().position(|k| *k == "sandbox-process").unwrap();
        assert_eq!(kinds[sandbox + 1], "lsp-diagnostics");
        let mut bare = Distribution::parse("[[plugin]]\nuse = \"loop-react\"\n").unwrap();
        assert!(bare.enable("lsp").unwrap_err().contains("sandbox-process"));
    }

    #[test]
    fn format_mounts_after_the_sandbox_before_the_loop() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("format").unwrap();
        distribution.enable("lsp").unwrap();
        let kinds = kinds(&distribution);
        let sandbox = kinds.iter().position(|k| *k == "sandbox-process").unwrap();
        assert!(kinds[sandbox + 1..sandbox + 3].contains(&"format-on-write"));
        assert!(kinds[sandbox + 1..sandbox + 3].contains(&"lsp-diagnostics"));
    }

    #[test]
    fn plan_swaps_the_loop() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("plan").unwrap();
        let kinds = kinds(&distribution);
        assert!(kinds.contains(&"loop-plan") && !kinds.contains(&"loop-react"));
        distribution.enable("plan").unwrap();
    }

    #[test]
    fn bad_requests_are_refused_with_a_reason() {
        let mut distribution = Distribution::preset("dev").unwrap();
        let unknown = distribution.enable("teleport").unwrap_err();
        assert!(
            unknown.contains("unknown feature") && unknown.contains("shell"),
            "{unknown}"
        );
        let argument = distribution.enable("telemetry=x").unwrap_err();
        assert!(argument.contains("takes no argument"), "{argument}");
        let mut bare = Distribution::parse("[[plugin]]\nuse = \"loop-react\"\n").unwrap();
        let missing = bare.enable("shell").unwrap_err();
        assert!(missing.contains("sandbox-process"), "{missing}");
    }

    #[test]
    fn features_in_the_file_apply_once() {
        let mut distribution = Distribution::parse(
            "features = [\"web-search\", \"shell=git\"]\n\n[[plugin]]\nuse = \"sandbox-process\"\n",
        )
        .unwrap();
        distribution.apply_features().unwrap();
        assert!(distribution.features.is_empty());
        let kinds = kinds(&distribution);
        assert!(kinds.contains(&"tools-web-search") && kinds.contains(&"tools-shell"));
    }
}
