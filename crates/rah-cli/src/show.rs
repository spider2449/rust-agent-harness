//! `rah config show`: what will run and where each value comes from. The
//! plugin list is shown only on request (`--plugins`).

use std::path::Path;

use crate::{
    Distribution,
    cli::{Origin, Prepared},
    extend::USER_BASE,
};

/// Longest configuration shown on one line.
const MAX_CONFIG_CHARS: usize = 400;

/// The Web UI's stored overrides live in this namespace of the storage dir.
const WEB_NAMESPACE: &str = "frontend-web";

/// Describes what `prepared` runs: the user-facing values and their origin,
/// and with `plugins` every plugin with its configuration.
pub fn show(prepared: &Prepared, plugins: bool) -> String {
    let distribution = &prepared.distribution;
    let file = prepared.file.as_ref().map_or_else(
        || "none (built-in default; `rah config init` writes one)".to_owned(),
        |path| path.display().to_string(),
    );
    let value = |text: String, origin: Origin| format!("{text:<40} ({})", origin.label());
    let model = distribution
        .model_label()
        .unwrap_or_else(|| "none".to_owned());
    let root = distribution
        .root_label()
        .unwrap_or_else(|| "none".to_owned());
    let features: Vec<&str> = distribution
        .applied
        .iter()
        .map(|applied| applied.request.as_str())
        .collect();
    let features = if features.is_empty() {
        "none".to_owned()
    } else {
        features.join(", ")
    };
    let mut text = format!(
        "file:        {file}\n\
         data home:   {}\n\
         model:       {}\n\
         permissions: {}\n\
         root:        {}\n\
         features:    {features}\n",
        prepared.home.display(),
        value(model, prepared.model),
        value(distribution.permissions_label(), prepared.permissions),
        value(root, prepared.root),
    );
    if plugins {
        text.push_str(&plugin_list(distribution));
    } else {
        text.push_str(&format!(
            "plugins:     {} (`rah config show --plugins` lists them)\n",
            distribution.plugin.len()
        ));
    }
    text.push_str(&overrides(&prepared.home));
    text
}

/// The composition and every plugin in mount order with its configuration.
fn plugin_list(distribution: &Distribution) -> String {
    let mut text = String::new();
    if let Some(extended) = &distribution.extended {
        if extended.preset != USER_BASE {
            text.push_str(&format!("extends:     preset `{}`\n", extended.preset));
        }
        if !extended.removed.is_empty() {
            text.push_str(&format!("removed:     {}\n", extended.removed.join(", ")));
        }
    }
    text.push_str("plugins, in mount order:\n");
    for spec in &distribution.plugin {
        let id = spec.plugin_id();
        let name = if id.as_str() == spec.kind {
            spec.kind.clone()
        } else {
            format!("{} ({id})", spec.kind)
        };
        let feature = distribution
            .applied
            .iter()
            .find(|applied| applied.mounted.as_deref() == Some(spec.kind.as_str()))
            .map(|applied| format!("  <- feature {}", applied.request));
        let file = distribution.extended.as_ref().and_then(|extended| {
            let id = id.as_str().to_owned();
            if extended.added.contains(&id) {
                Some("  <- added by the file")
            } else if extended.changed.contains(&id) {
                Some("  <- changed by the file")
            } else {
                None
            }
        });
        let origin = feature.or(file.map(str::to_owned)).unwrap_or_default();
        text.push_str(&format!("  {name}{origin}\n"));
        if !spec.config.is_null() {
            let mut config = spec.config.to_string();
            if config.chars().count() > MAX_CONFIG_CHARS {
                config = config.chars().take(MAX_CONFIG_CHARS).collect::<String>() + "...";
            }
            text.push_str(&format!("      config {config}\n"));
        }
    }
    text
}

/// The entries the Web UI stored, which replace plugin configuration at start.
fn overrides(home: &Path) -> String {
    let dir = home.join("storage").join(WEB_NAMESPACE);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| !name.starts_with(".tmp-") && name != "preferences")
        .collect();
    names.sort();
    if names.is_empty() {
        return "web ui overrides: none\n".to_owned();
    }
    format!(
        "web ui overrides in {} (they replace the configuration above at start; \
         delete a file to undo it):\n  {}\n",
        dir.display(),
        names.join("\n  ")
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn prepared(distribution: Distribution, home: PathBuf) -> Prepared {
        Prepared {
            distribution,
            home,
            file: None,
            model: Origin::Default,
            permissions: Origin::Default,
            root: Origin::Default,
        }
    }

    #[test]
    fn the_summary_names_values_and_origins_and_hides_plugins() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("web-search").unwrap();
        let mut prepared = prepared(distribution, std::env::temp_dir().join("rah-show-none"));
        prepared.model = Origin::Flag;
        let text = show(&prepared, false);
        assert!(
            text.contains("file:        none (built-in default"),
            "{text}"
        );
        assert!(text.contains("model:       mock"), "{text}");
        assert!(text.contains("(command line)"), "{text}");
        assert!(text.contains("permissions: ask"), "{text}");
        assert!(text.contains("root:        ."), "{text}");
        assert!(text.contains("features:    web-search"), "{text}");
        assert!(
            text.contains("`rah config show --plugins` lists them"),
            "{text}"
        );
        assert!(!text.contains("plugins, in mount order"), "{text}");
    }

    #[test]
    fn shows_features_origin_and_overrides() {
        // `minimal` has no `tools-shell`, so the feature mounts it.
        let mut distribution = Distribution::preset("minimal").unwrap();
        distribution.enable("shell=git").unwrap();
        let dir = std::env::temp_dir().join(format!("rah-show-{}", std::process::id()));
        let web = dir.join("storage").join(WEB_NAMESPACE);
        std::fs::create_dir_all(&web).unwrap();
        std::fs::write(web.join("settings-model"), "{}").unwrap();
        std::fs::write(web.join("preferences"), "{}").unwrap();
        let text = show(&prepared(distribution, dir.clone()), true);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(text.contains("features:    shell=git"), "{text}");
        assert!(text.contains("UNGUARDED"), "{text}");
        assert!(text.contains("tools-shell  <- feature shell=git"), "{text}");
        assert!(text.contains("allow_programs"), "{text}");
        assert!(
            text.contains("settings-model") && !text.contains("preferences"),
            "{text}"
        );
    }

    #[test]
    fn shows_what_a_file_changed_in_its_preset() {
        let distribution = Distribution::parse(
            "extends = \"dev\"\nremove = [\"skills-fs\"]\n\
             [[plugin]]\nuse = \"loop-react\"\nconfig = { max_rounds = 4 }\n\
             [[plugin]]\nuse = \"context-compact\"\n",
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("rah-show-extends-{}", std::process::id()));
        let text = show(&prepared(distribution, dir), true);
        assert!(text.contains("extends:     preset `dev`"), "{text}");
        assert!(text.contains("removed:     skills-fs"), "{text}");
        assert!(
            text.contains("loop-react  <- changed by the file"),
            "{text}"
        );
        assert!(
            text.contains("context-compact  <- added by the file"),
            "{text}"
        );
        assert!(!text.contains("\n  skills-fs\n"), "{text}");
    }

    #[test]
    fn a_user_file_does_not_claim_a_preset() {
        let distribution = Distribution::parse("model = \"mock\"\n").unwrap();
        let text = show(
            &prepared(distribution, std::env::temp_dir().join("rah-show-user")),
            true,
        );
        assert!(!text.contains("extends:"), "{text}");
        assert!(text.contains("model-mock"), "{text}");
    }
}
