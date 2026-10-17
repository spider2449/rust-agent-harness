//! `rah --show-config`: what will run and where each part comes from.

use std::path::Path;

use crate::Distribution;

/// Longest configuration shown on one line.
const MAX_CONFIG_CHARS: usize = 400;

/// The Web UI's stored overrides live in this namespace of the storage dir.
const WEB_NAMESPACE: &str = "frontend-web";

/// Describes `distribution`, read from `source`, with its data under `home`.
pub fn show_config(distribution: &Distribution, source: &str, home: &Path) -> String {
    let mut text = format!("source:    {source}\ndata home: {}\n", home.display());
    if distribution.applied.is_empty() {
        text.push_str("features:  none\n");
    } else {
        let requests: Vec<&str> = distribution
            .applied
            .iter()
            .map(|applied| applied.request.as_str())
            .collect();
        text.push_str(&format!("features:  {}\n", requests.join(", ")));
    }
    text.push_str("plugins, in mount order:\n");
    for spec in &distribution.plugin {
        let id = spec.plugin_id();
        let name = if id.as_str() == spec.kind {
            spec.kind.clone()
        } else {
            format!("{} ({id})", spec.kind)
        };
        let origin = distribution
            .applied
            .iter()
            .find(|applied| applied.mounted.as_deref() == Some(spec.kind.as_str()))
            .map(|applied| format!("  <- feature {}", applied.request))
            .unwrap_or_default();
        text.push_str(&format!("  {name}{origin}\n"));
        if !spec.config.is_null() {
            let mut config = spec.config.to_string();
            if config.chars().count() > MAX_CONFIG_CHARS {
                config = config.chars().take(MAX_CONFIG_CHARS).collect::<String>() + "...";
            }
            text.push_str(&format!("      config {config}\n"));
        }
    }
    text.push_str(&overrides(home));
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
    use super::*;

    #[test]
    fn shows_features_origin_and_overrides() {
        let mut distribution = Distribution::preset("dev").unwrap();
        distribution.enable("shell=git").unwrap();
        let dir = std::env::temp_dir().join(format!("rah-show-{}", std::process::id()));
        let web = dir.join("storage").join(WEB_NAMESPACE);
        std::fs::create_dir_all(&web).unwrap();
        std::fs::write(web.join("settings-model"), "{}").unwrap();
        std::fs::write(web.join("preferences"), "{}").unwrap();
        let text = show_config(&distribution, "preset `dev`", &dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(text.contains("features:  shell=git"), "{text}");
        assert!(text.contains("tools-shell  <- feature shell=git"), "{text}");
        assert!(text.contains("allow_programs"), "{text}");
        assert!(
            text.contains("settings-model") && !text.contains("preferences"),
            "{text}"
        );
    }
}
