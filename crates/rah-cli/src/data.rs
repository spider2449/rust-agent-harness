//! The data home: one directory for what RAH keeps between runs (sessions,
//! storage, memory, skills and telemetry), so it does not depend on the
//! directory `rah` happens to start in.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// Environment variable that moves the data home.
pub const DATA_HOME_ENV: &str = "RAH_HOME";

/// Data paths of catalog plugins: kind, configuration key and the default
/// under the data home. A path without a default is opt-in: it is resolved
/// when set and never added.
const DATA_PATHS: [(&str, &str, Option<&str>); 6] = [
    ("storage-fs", "dir", Some("storage")),
    ("session-jsonl", "dir", Some("sessions")),
    ("skills-fs", "dir", Some("skills")),
    ("telemetry-log", "dir", Some("telemetry")),
    ("context-default", "memory_dir", None),
    ("context-default", "memory_file", None),
];

/// The default data home: `RAH_HOME` when set, otherwise `.rah` in the
/// user's home directory (`USERPROFILE` on Windows, `HOME` elsewhere).
/// `None` when neither is set.
pub fn default_data_home() -> Option<PathBuf> {
    let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    if let Some(home) = set(DATA_HOME_ENV) {
        return Some(PathBuf::from(home));
    }
    let user = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    set(user).map(|home| PathBuf::from(home).join(".rah"))
}

/// Resolves the data paths of a `kind` plugin's configuration against
/// `home`: a missing path with a default is added, a relative path is joined
/// to `home` and an absolute path is kept. Values that are not strings are
/// left for the plugin to reject.
pub fn resolve_data_paths(kind: &str, config: &mut Value, home: &Path) {
    for (_, key, default) in DATA_PATHS.iter().filter(|(owner, ..)| *owner == kind) {
        let path = match config.get(*key) {
            Some(Value::String(path)) if Path::new(path).is_absolute() => continue,
            Some(Value::String(path)) => home.join(path),
            Some(_) => continue,
            None => match default {
                Some(default) => home.join(default),
                None => continue,
            },
        };
        if config.is_null() {
            *config = Value::Object(Map::new());
        }
        if let Value::Object(map) = config {
            map.insert((*key).to_owned(), Value::String(path.display().to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn home() -> PathBuf {
        std::env::temp_dir().join("rah-home")
    }

    fn under_home(name: &str) -> Value {
        json!(home().join(name).display().to_string())
    }

    #[test]
    fn missing_paths_default_under_the_home() {
        let mut config = Value::Null;
        resolve_data_paths("session-jsonl", &mut config, &home());
        assert_eq!(config, json!({ "dir": under_home("sessions") }));

        let mut config = json!({ "keep_days": 3 });
        resolve_data_paths("telemetry-log", &mut config, &home());
        assert_eq!(
            config,
            json!({ "keep_days": 3, "dir": under_home("telemetry") })
        );
    }

    #[test]
    fn relative_paths_resolve_against_the_home_and_absolute_ones_stay() {
        let mut config = json!({ "dir": "kv" });
        resolve_data_paths("storage-fs", &mut config, &home());
        assert_eq!(config, json!({ "dir": under_home("kv") }));

        let absolute = std::env::temp_dir().join("elsewhere");
        let absolute = json!(absolute.display().to_string());
        let mut config = json!({ "dir": absolute.clone() });
        resolve_data_paths("skills-fs", &mut config, &home());
        assert_eq!(config, json!({ "dir": absolute }));
    }

    #[test]
    fn memory_stays_opt_in() {
        let mut config = Value::Null;
        resolve_data_paths("context-default", &mut config, &home());
        assert_eq!(config, Value::Null);

        let mut config = json!({ "memory_dir": "memory" });
        resolve_data_paths("context-default", &mut config, &home());
        assert_eq!(config, json!({ "memory_dir": under_home("memory") }));
    }

    #[test]
    fn other_plugins_and_invalid_values_are_untouched() {
        let mut config = json!({ "root": "." });
        resolve_data_paths("tools-builtin", &mut config, &home());
        assert_eq!(config, json!({ "root": "." }));

        let mut config = json!({ "dir": 7 });
        resolve_data_paths("storage-fs", &mut config, &home());
        assert_eq!(config, json!({ "dir": 7 }));
    }
}
