//! Checks every plugin configuration of a distribution before anything is
//! mounted (ADR 0059), so a file with several mistakes reports all of them at
//! once and a refused file mounts nothing.
//!
//! The check is the one the kernel runs at mount (`Plugin::check_config`,
//! ADR 0044), on the configuration `boot` would pass. It starts nothing: a
//! process or WASM plugin gets only its launch configuration checked, as its
//! host does, because describing it would start its program.

use std::{collections::BTreeSet, fmt, panic::AssertUnwindSafe};

use rah_api::PluginId;
use rah_plugin_host_process::ProcessConfig;
use rah_plugin_host_wasm::WasmConfig;
use serde::Deserialize;
use serde_json::Value;

use crate::{Distribution, PROCESS_PLUGIN, PluginSpec, WASM_PLUGIN, catalog, resolve_data_paths};

/// A plugin whose configuration would be refused at mount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigProblem {
    /// The instance id.
    pub id: PluginId,
    /// The `use` name.
    pub kind: String,
    pub reason: String,
}

impl fmt::Display for ConfigProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.id.as_str() == self.kind {
            write!(f, "{}: {}", self.id, self.reason)?;
        } else {
            write!(f, "{} ({}): {}", self.id, self.kind, self.reason)?;
        }
        if self.kind == "policy-authority" {
            write!(f, "; refusing to run unguarded")?;
        }
        Ok(())
    }
}

/// The problems `boot` would stop at, for every plugin of `distribution`, in
/// mount order. Empty when every configuration would be accepted. A
/// configuration that passes can still fail when the plugin starts, for
/// example on a missing API key or a program that does not run.
pub fn check_distribution(distribution: &Distribution) -> Vec<ConfigProblem> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for spec in &distribution.plugin {
        let mut config = spec.config.clone();
        if let Some(home) = &distribution.data_home {
            resolve_data_paths(&spec.kind, &mut config, home);
        }
        let id = spec.plugin_id();
        let problem = |reason: String| ConfigProblem {
            id: id.clone(),
            kind: spec.kind.clone(),
            reason,
        };
        if !seen.insert(id.clone()) {
            problems.push(problem(format!(
                "the instance id `{id}` is used twice; give one of them another `id`"
            )));
            continue;
        }
        if let Err(reason) = check(spec, &config) {
            problems.push(problem(reason));
        }
    }
    problems
}

/// The check of one plugin.
fn check(spec: &PluginSpec, config: &Value) -> Result<(), String> {
    if spec.kind == PROCESS_PLUGIN || spec.kind == WASM_PLUGIN {
        if spec.id.is_none() {
            return Err(format!("a `{}` plugin needs an `id`", spec.kind));
        }
        let launch = if spec.kind == PROCESS_PLUGIN {
            ProcessConfig::deserialize(config).map(drop)
        } else {
            WasmConfig::deserialize(config).map(drop)
        };
        return launch.map_err(|error| error.to_string());
    }
    let catalog = catalog();
    let Some(constructor) = catalog.get(spec.kind.as_str()) else {
        let names: Vec<&str> = catalog.keys().copied().collect();
        return Err(format!(
            "unknown plugin `{}`; available: {}, {PROCESS_PLUGIN}, {WASM_PLUGIN}",
            spec.kind,
            names.join(", ")
        ));
    };
    let plugin = constructor(spec.plugin_id());
    // As in the kernel, a panicking check counts as a refusal.
    match std::panic::catch_unwind(AssertUnwindSafe(|| plugin.check_config(config))) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("the configuration check panicked".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_pass() {
        for preset in crate::PRESETS {
            let mut distribution = Distribution::preset(preset.name).unwrap();
            // `model-openai` has no default model; `rah --model` sets one.
            distribution.set_config("model-openai", "model", Value::from("gpt-x"));
            assert_eq!(check_distribution(&distribution), [], "{}", preset.name);
        }
    }

    #[test]
    fn a_missing_model_is_found_before_the_start() {
        let distribution = Distribution::preset("openai").unwrap();
        let problems = check_distribution(&distribution);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].reason.contains("needs `model`"), "{problems:?}");
    }

    #[test]
    fn every_mistake_is_reported_in_mount_order() {
        let distribution = Distribution::parse(
            r#"
[[plugin]]
use = "policy-authority"
config = { allow = ["read"], ask = ["read"] }

[[plugin]]
use = "loop-react"
config = { max_roundz = 4 }

[[plugin]]
use = "model-gpt9"

[[plugin]]
use = "process"
config = { program = "python" }

[[plugin]]
use = "wasm"
id = "words"
config = { path = "words.wasm", fuel = "lots" }

[[plugin]]
use = "model-mock"
id = "loop-react"

[[plugin]]
use = "frontend-cli"
"#,
        )
        .unwrap();
        let problems = check_distribution(&distribution);
        let ids: Vec<&str> = problems.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "policy-authority",
                "loop-react",
                "model-gpt9",
                "process",
                "words",
                "loop-react"
            ]
        );
        let text: Vec<String> = problems.iter().map(ToString::to_string).collect();
        assert!(
            text[0].ends_with("refusing to run unguarded"),
            "{}",
            text[0]
        );
        assert!(text[1].contains("max_roundz"), "{}", text[1]);
        assert!(
            text[2].contains("unknown plugin `model-gpt9`"),
            "{}",
            text[2]
        );
        assert!(text[3].contains("needs an `id`"), "{}", text[3]);
        assert!(text[4].starts_with("words (wasm): "), "{}", text[4]);
        assert!(text[5].contains("used twice"), "{}", text[5]);
    }
}
