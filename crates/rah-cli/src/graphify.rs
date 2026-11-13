//! Explicit, portable installation of the bundled Graphify integration.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rah_api::{
    CancellationToken,
    sandbox::{ExecEnv, ProgramSpec},
};
use rah_plugin_sandbox_process::ProcessEnv;
use serde_json::{Value, json};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, TableLike};

use crate::{Distribution, persist};

const PACKAGE: &str = "graphifyy[mcp]==0.9.107";
const QUERY: &str = "mcp.graphify.query_graph";
const ASSETS: [(&str, &str); 6] = [
    (
        "managed-mcp.mjs",
        include_str!("../../../examples/graphify/managed-mcp.mjs"),
    ),
    (
        "artifact.mjs",
        include_str!("../../../examples/graphify/artifact.mjs"),
    ),
    (
        "extract.py",
        include_str!("../../../examples/graphify/extract.py"),
    ),
    (
        "status.mjs",
        include_str!("../../../examples/graphify/status.mjs"),
    ),
    (
        "status-view.mjs",
        include_str!("../../../examples/graphify/status-view.mjs"),
    ),
    (
        "graph-view.mjs",
        include_str!("../../../examples/graphify/graph-view.mjs"),
    ),
];

/// Programs are discovered on this computer, never copied from another profile.
struct Programs {
    node: PathBuf,
    backend: Backend,
}

enum Backend {
    Uvx(PathBuf),
    Python(PathBuf),
}

#[derive(Default)]
pub struct InstallOptions {
    pub python: Option<PathBuf>,
    pub node: Option<PathBuf>,
    pub runtime_dir: Option<PathBuf>,
}

fn bundle_programs(root: &Path) -> Result<Programs, String> {
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("runtime.json"))
            .map_err(|error| format!("Cannot read bundled Graphify runtime: {error}"))?,
    )
    .map_err(|error| error.to_string())?;
    if manifest["format"] != 1
        || manifest["graphify_version"] != "0.9.107"
        || manifest["platform"] != std::env::consts::OS
        || manifest["arch"] != std::env::consts::ARCH
    {
        return Err(
            "Bundled Graphify runtime has an unsupported format, package version or platform"
                .into(),
        );
    }
    let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let resolve = |key: &str| -> Result<PathBuf, String> {
        let path = Path::new(
            manifest[key]
                .as_str()
                .ok_or("Runtime program path must be text")?,
        );
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Runtime program path escapes its bundle".into());
        }
        let program = fs::canonicalize(root.join(path)).map_err(|error| error.to_string())?;
        if !program.starts_with(&root) || !program.is_file() {
            return Err("Runtime program escapes its bundle or is missing".into());
        }
        // Other programs should receive normal Windows paths rather than verbatim paths.
        let text = program.display().to_string();
        Ok(match text.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
            _ => program,
        })
    };
    Ok(Programs {
        node: resolve("node")?,
        backend: Backend::Python(resolve("python")?),
    })
}

fn select_programs(options: &InstallOptions, executable_dir: &Path) -> Result<Programs, String> {
    if let Some(python) = &options.python {
        return Ok(Programs {
            node: options
                .node
                .as_deref()
                .map(crate::cli::absolute)
                .unwrap_or_else(|| program("node"))?,
            backend: Backend::Python(crate::cli::absolute(python)?),
        });
    }
    let bundled = executable_dir.join("graphify-runtime");
    if let Some(root) = options
        .runtime_dir
        .as_deref()
        .or_else(|| bundled.exists().then_some(bundled.as_path()))
    {
        let mut programs = bundle_programs(root)?;
        if let Some(node) = &options.node {
            programs.node = crate::cli::absolute(node)?;
        }
        return Ok(programs);
    }
    Ok(Programs {
        node: options
            .node
            .as_deref()
            .map(crate::cli::absolute)
            .unwrap_or_else(|| program("node"))?,
        backend: Backend::Uvx(program("uvx")?),
    })
}

pub struct Installed {
    pub config: PathBuf,
    pub assets: PathBuf,
    pub backup: Option<PathBuf>,
}

fn program(name: &str) -> Result<PathBuf, String> {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    std::env::var_os("PATH").into_iter().flat_map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .map(|directory| directory.join(&filename)).find(|file| file.is_file())
        .and_then(|file| std::path::absolute(file).ok())
        .ok_or_else(|| format!("`{name}` was not found on PATH. Install Node.js 20+ and uv, reopen the terminal, then run `rah graphify install` again. No RAH settings were changed."))
}

/// The revision is a content key, not an authenticity or security check.
fn revision() -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for (name, source) in ASSETS {
        for byte in name.bytes().chain(source.bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    format!("{}-{hash:016x}", env!("CARGO_PKG_VERSION"))
}

fn merge(table: &mut dyn TableLike, key: &str, value: &Value) -> Result<(), String> {
    if let Value::Object(values) = value {
        if table.get(key).is_none() {
            table.insert(key, Item::Table(Table::new()));
        }
        let nested = table
            .get_mut(key)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| format!("Graphify setup expected `{key}` to be a table"))?;
        for (key, value) in values {
            merge(nested, key, value)?;
        }
    } else {
        let value = persist::to_toml(value).ok_or("Graphify setup cannot serialize a setting")?;
        table.insert(key, Item::Value(value));
    }
    Ok(())
}

fn upsert(doc: &mut DocumentMut, id: &str, kind: &str, config: &Value) -> Result<(), String> {
    if doc.get("plugin").is_none() {
        doc["plugin"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    let entries = doc["plugin"]
        .as_array_of_tables_mut()
        .ok_or("`plugin` must be an array of tables")?;
    let position = entries.iter().position(|entry| {
        entry
            .get("id")
            .and_then(Item::as_str)
            .or_else(|| entry.get("use").and_then(Item::as_str))
            == Some(id)
    });
    if let Some(position) = position {
        let entry = entries
            .get_mut(position)
            .ok_or("Plugin entry disappeared")?;
        if entry.get("use").and_then(Item::as_str) != Some(kind) {
            return Err(format!(
                "Plugin id `{id}` already belongs to another plugin; no configuration was changed"
            ));
        }
        merge(entry, "config", config)?;
    } else {
        let mut entry = Table::new();
        entry["use"] = toml_edit::value(kind);
        entry["id"] = toml_edit::value(id);
        merge(&mut entry, "config", config)?;
        let position = entries
            .iter()
            .position(|existing| {
                existing
                    .get("use")
                    .and_then(Item::as_str)
                    .is_some_and(|kind| kind.starts_with("frontend-"))
            })
            .unwrap_or(entries.len());
        entries.insert(position, entry);
    }
    Ok(())
}

fn configuration(
    text: &str,
    home: &Path,
    programs: &Programs,
    assets: &Path,
    env: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut current = Distribution::parse(text).map_err(|error| error.to_string())?;
    current.apply_features()?;
    if !current
        .plugin
        .iter()
        .any(|plugin| matches!(plugin.kind.as_str(), "sandbox-process" | "sandbox-os"))
    {
        return Err("Graphify requires a sandbox-process or sandbox-os plugin".into());
    }
    let policy = current
        .plugin
        .iter()
        .find(|plugin| plugin.kind == "policy-authority")
        .ok_or("Graphify setup requires policy-authority to assign its read permission")?;
    for (id, filename) in [
        ("graphify", "managed-mcp.mjs"),
        ("graphify-status", "status-view.mjs"),
    ] {
        if let Some(existing) = current
            .plugin
            .iter()
            .find(|plugin| plugin.plugin_id().as_str() == id)
        {
            let managed = existing.config["args"].as_array().is_some_and(|args| {
                args.iter().any(|arg| {
                    arg.as_str().is_some_and(|arg| {
                        Path::new(arg)
                            .file_name()
                            .is_some_and(|name| name == filename)
                    })
                })
            });
            if !managed {
                return Err(format!(
                    "Plugin `{id}` is a custom mount; refusing to replace it automatically"
                ));
            }
        }
    }
    let runtime = home.join("runtime").join("graphify");
    let mut child_env = env.clone();
    child_env.insert("PYTHONIOENCODING".into(), "utf-8".into());
    child_env.insert(
        "RAH_GRAPHIFY_STATUS_DIR".into(),
        runtime.display().to_string(),
    );
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|error| error.to_string())?;
    if let Some(entries) = doc.get_mut("plugin").and_then(Item::as_array_of_tables_mut) {
        for entry in entries
            .iter_mut()
            .filter(|entry| entry.get("id").and_then(Item::as_str) == Some("graphify"))
        {
            if let Some(env) = entry
                .get_mut("config")
                .and_then(Item::as_table_like_mut)
                .and_then(|config| config.get_mut("env"))
                .and_then(Item::as_table_like_mut)
            {
                for key in ["UV_CACHE_DIR", "UV_PYTHON_INSTALL_DIR", "PYTHONHOME"] {
                    env.remove(key);
                }
            }
        }
    }
    let (backend_flag, backend_path) = match &programs.backend {
        Backend::Uvx(path) => ("--uvx", path),
        Backend::Python(path) => ("--python", path),
    };
    upsert(
        &mut doc,
        "graphify",
        "tools-mcp",
        &json!({
            "server":"graphify", "program":programs.node, "args":[assets.join("managed-mcp.mjs"), backend_flag, backend_path],
            "env":child_env, "tools":{"query_graph":"read"}, "startup_timeout_ms":180000, "call_timeout_ms":180000,
        }),
    )?;
    upsert(
        &mut doc,
        "graphify-status",
        "process",
        &json!({
            "program":programs.node, "args":[assets.join("status-view.mjs")],
            "plugin":{"status_dir":runtime, "status_bar":true, "graph_view":true},
        }),
    )?;
    let assigned = policy.config["tools"].as_object().is_some_and(|tools| {
        tools.keys().any(|pattern| {
            pattern == QUERY
                || pattern
                    .strip_suffix('*')
                    .is_some_and(|prefix| QUERY.starts_with(prefix))
        })
    });
    if !assigned {
        upsert(
            &mut doc,
            policy.plugin_id().as_str(),
            "policy-authority",
            &json!({"tools":{QUERY:"read"}}),
        )?;
    }
    let result = doc.to_string();
    let mut parsed = Distribution::parse(&result).map_err(|error| error.to_string())?;
    parsed.apply_features()?;
    let problems = crate::check_distribution(&parsed);
    if !problems.is_empty() {
        return Err(format!(
            "Graphify setup refused invalid configuration: {problems:?}"
        ));
    }
    Ok(result)
}

fn write_assets(home: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(home).map_err(|error| error.to_string())?;
    let mut parent = fs::canonicalize(home).map_err(|error| error.to_string())?;
    for name in ["integrations", "graphify", &revision()] {
        let next = parent.join(name);
        fs::create_dir_all(&next).map_err(|error| error.to_string())?;
        let resolved = fs::canonicalize(&next).map_err(|error| error.to_string())?;
        if resolved.parent() != Some(parent.as_path()) {
            return Err("Graphify assets directory escapes its data home".into());
        }
        parent = resolved;
    }
    for (name, contents) in ASSETS {
        let file = parent.join(name);
        if file.exists() {
            if fs::symlink_metadata(&file)
                .map_err(|error| error.to_string())?
                .file_type()
                .is_symlink()
                || fs::read(&file).map_err(|error| error.to_string())? != contents.as_bytes()
            {
                return Err(format!(
                    "Managed Graphify asset `{}` was modified; preserve it before reinstalling",
                    file.display()
                ));
            }
        } else {
            persist::write_atomically(&file, contents)?;
        }
    }
    Ok(home.join("integrations").join("graphify").join(revision()))
}

fn install_files(
    home: &Path,
    file: &Path,
    programs: &Programs,
    env: &BTreeMap<String, String>,
) -> Result<Installed, String> {
    let original = fs::read_to_string(file).map_err(|error| {
        format!(
            "Cannot read `{}`: {error}. Run `rah config init` first",
            file.display()
        )
    })?;
    let assets = home.join("integrations").join("graphify").join(revision());
    let updated = configuration(&original, home, programs, &assets, env)?;
    write_assets(home)?;
    let backup = if updated != original {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let directory = home
            .join("config-backups")
            .join(format!("graphify-{stamp}"));
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let backup = directory.join("rah.toml.before");
        fs::write(&backup, &original).map_err(|error| error.to_string())?;
        if fs::read_to_string(file).map_err(|error| error.to_string())? != original {
            return Err(
                "Configuration changed during Graphify setup; refusing to overwrite it".into(),
            );
        }
        persist::write_atomically(file, &updated)?;
        Some(backup)
    } else {
        None
    };
    Ok(Installed {
        config: file.to_owned(),
        assets,
        backup,
    })
}

/// Explicit installation may populate uv's user cache; normal RAH startup never downloads it.
pub async fn install(home: &Path, file: &Path, offline: bool) -> Result<Installed, String> {
    install_with_options(home, file, offline, &InstallOptions::default()).await
}

pub async fn install_with_options(
    home: &Path,
    file: &Path,
    offline: bool,
    options: &InstallOptions,
) -> Result<Installed, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let programs = select_programs(
        options,
        executable
            .parent()
            .ok_or("RAH executable has no directory")?,
    )?;
    let original = fs::read_to_string(file).map_err(|error| {
        format!(
            "Cannot read `{}`: {error}. Run `rah config init` first",
            file.display()
        )
    })?;
    let overrides: BTreeMap<_, _> = ["UV_CACHE_DIR", "UV_PYTHON_INSTALL_DIR"]
        .into_iter()
        .filter(|_| matches!(programs.backend, Backend::Uvx(_)))
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
        .collect();
    configuration(
        &original,
        home,
        &programs,
        &home.join("integrations/graphify").join(revision()),
        &overrides,
    )?;
    let root = fs::canonicalize(
        file.parent()
            .ok_or("Configuration has no parent directory")?,
    )
    .map_err(|error| error.to_string())?;
    let environment = ProcessEnv::new(
        root,
        std::env::vars_os()
            .filter(|(key, _)| key != "PYTHONHOME")
            .collect(),
        Duration::from_secs(180),
    );
    let node = environment
        .run(
            ProgramSpec::new(
                programs.node.display().to_string(),
                vec!["--version".into()],
                Duration::from_secs(10),
            ),
            CancellationToken::new(),
        )
        .await
        .map_err(|error| error.to_string())?;
    let version = String::from_utf8_lossy(&node.stdout);
    if node.exit_code != Some(0)
        || version
            .trim()
            .trim_start_matches('v')
            .split('.')
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .is_none_or(|major| major < 20)
    {
        return Err("Graphify requires Node.js 20 or newer; no configuration was changed".into());
    }
    let check = "import sys,importlib.metadata as m; assert sys.version_info >= (3,10); assert m.version('graphifyy') == '0.9.107'; import graphify.serve,mcp";
    let (backend, args) = match &programs.backend {
        Backend::Python(path) => (path, vec!["-I".into(), "-c".into(), check.into()]),
        Backend::Uvx(path) => {
            let mut args = Vec::new();
            if offline {
                args.push("--offline".into());
            }
            args.extend(["--from", PACKAGE, "python", "-c", check].map(str::to_owned));
            (path, args)
        }
    };
    let output = environment
        .run(
            ProgramSpec::new(
                backend.display().to_string(),
                args,
                Duration::from_secs(180),
            ),
            CancellationToken::new(),
        )
        .await
        .map_err(|error| error.to_string())?;
    if output.exit_code != Some(0) || output.termination.is_some() {
        return Err(format!(
            "Could not prepare pinned Graphify ({:?}): {}. {}No RAH settings were changed",
            output.termination,
            String::from_utf8_lossy(&output.stderr),
            if matches!(programs.backend, Backend::Python(_)) {
                "Use a complete bundled runtime, or prepare this Python environment with pip install 'graphifyy[mcp]==0.9.107'. "
            } else if offline {
                "Run `rah graphify install` without --offline to download its user-local environment. "
            } else {
                "Check uv/Python connectivity and retry. "
            }
        ));
    }
    if fs::read_to_string(file).map_err(|error| error.to_string())? != original {
        return Err("Configuration changed while preparing Graphify; run the command again".into());
    }
    install_files(home, file, &programs, &overrides)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programs() -> Programs {
        Programs {
            node: PathBuf::from("/local/node"),
            backend: Backend::Uvx(PathBuf::from("/local/uvx")),
        }
    }

    struct Profile {
        root: PathBuf,
        home: PathBuf,
        file: PathBuf,
    }

    impl Profile {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "rah-portable-graphify-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            let file = root.join("custom.toml");
            fs::write(
                &file,
                "# Keep this profile\nmodel = \"mock\"\npermissions = \"read-only\"\n",
            )
            .unwrap();
            Self {
                home: root.join("custom-data"),
                root,
                file,
            }
        }
    }

    impl Drop for Profile {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_fresh_profile_installs_embedded_assets_without_a_checkout_and_is_repeatable() {
        let p = Profile::new();
        let before = fs::read_to_string(&p.file).unwrap();
        let first = install_files(&p.home, &p.file, &programs(), &BTreeMap::new()).unwrap();
        assert_eq!(fs::read_to_string(first.backup.unwrap()).unwrap(), before);
        for (name, contents) in ASSETS {
            assert_eq!(
                fs::read(first.assets.join(name)).unwrap(),
                contents.as_bytes()
            );
        }
        let installed = fs::read_to_string(&p.file).unwrap();
        assert!(installed.contains("# Keep this profile"));
        let parsed = Distribution::parse(&installed).unwrap();
        assert_eq!(
            parsed
                .plugin
                .iter()
                .filter(|spec| spec.plugin_id().as_str().starts_with("graphify"))
                .count(),
            2
        );
        let graph = parsed
            .plugin
            .iter()
            .find(|spec| spec.plugin_id().as_str() == "graphify")
            .unwrap();
        assert_eq!(graph.config["program"], json!(programs().node));
        assert_eq!(
            graph.config["args"][0],
            json!(first.assets.join("managed-mcp.mjs"))
        );
        assert_eq!(
            graph.config["env"]["RAH_GRAPHIFY_STATUS_DIR"],
            json!(p.home.join("runtime").join("graphify"))
        );
        assert!(
            parsed
                .plugin
                .iter()
                .find(|spec| spec.kind == "policy-authority")
                .unwrap()
                .config["tools"]
                .get(QUERY)
                .is_some()
        );
        let again = install_files(&p.home, &p.file, &programs(), &BTreeMap::new()).unwrap();
        assert!(again.backup.is_none());
        assert_eq!(fs::read_to_string(&p.file).unwrap(), installed);
    }

    #[test]
    fn explicit_policy_and_unrelated_mounts_survive_setup() {
        let p = Profile::new();
        let text = r#"# User settings
model = "mock"
[[plugin]]
use = "policy-authority"
config = { tools = { "mcp.graphify.query_graph" = "execute" }, decisions = { "mcp.graphify.*" = "deny" } }
[[plugin]]
use = "process"
id = "my-view"
config = { program = "custom-node", args = ["my-view.mjs"] }
"#;
        let updated = configuration(
            text,
            &p.home,
            &programs(),
            &p.home.join("assets"),
            &BTreeMap::new(),
        )
        .unwrap();
        let before = Distribution::parse(text).unwrap();
        let after = Distribution::parse(&updated).unwrap();
        for id in ["policy-authority", "my-view", "model-mock"] {
            assert_eq!(
                before
                    .plugin
                    .iter()
                    .find(|spec| spec.plugin_id().as_str() == id),
                after
                    .plugin
                    .iter()
                    .find(|spec| spec.plugin_id().as_str() == id)
            );
        }
        let wildcard = text.replace(
            "\"mcp.graphify.query_graph\" = \"execute\"",
            "\"mcp.*\" = \"execute\"",
        );
        let updated = configuration(
            &wildcard,
            &p.home,
            &programs(),
            &p.home.join("assets"),
            &BTreeMap::new(),
        )
        .unwrap();
        let parsed = Distribution::parse(&updated).unwrap();
        let policy = &parsed
            .plugin
            .iter()
            .find(|plugin| plugin.kind == "policy-authority")
            .unwrap()
            .config;
        assert_eq!(policy["tools"]["mcp.*"], "execute");
        assert!(policy["tools"].get(QUERY).is_none());
    }

    #[test]
    fn copied_managed_mounts_are_relocated_and_old_machine_cache_paths_removed() {
        let p = Profile::new();
        let text = r#"model = "mock"
[[plugin]]
use = "tools-mcp"
id = "graphify"
config = { server = "graphify", program = "old-node", args = ["old/managed-mcp.mjs"], env = { UV_CACHE_DIR = "old-cache", UV_PYTHON_INSTALL_DIR = "old-python", PYTHONHOME = "old-home", KEEP = "yes" }, tools = { query_graph = "read" } }
"#;
        let updated = configuration(
            text,
            &p.home,
            &programs(),
            &p.home.join("assets"),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(
            !updated.contains("old-cache")
                && !updated.contains("old-python")
                && !updated.contains("old-home")
        );
        let parsed = Distribution::parse(&updated).unwrap();
        let graph = parsed
            .plugin
            .iter()
            .find(|spec| spec.plugin_id().as_str() == "graphify")
            .unwrap();
        assert_eq!(graph.config["env"]["KEEP"], "yes");
        assert_eq!(graph.config["program"], json!(programs().node));
    }

    #[test]
    fn conflicting_ids_and_modified_assets_leave_config_untouched() {
        let p = Profile::new();
        let conflict = "model = \"mock\"\n[[plugin]]\nuse = \"process\"\nid = \"graphify\"\nconfig = { program = \"my-program\" }\n";
        fs::write(&p.file, conflict).unwrap();
        assert!(install_files(&p.home, &p.file, &programs(), &BTreeMap::new()).is_err());
        assert_eq!(fs::read_to_string(&p.file).unwrap(), conflict);
        assert!(!p.home.exists());
        fs::write(&p.file, "model = \"mock\"\n").unwrap();
        let assets = write_assets(&p.home).unwrap();
        fs::write(assets.join("managed-mcp.mjs"), "user-modified").unwrap();
        assert!(install_files(&p.home, &p.file, &programs(), &BTreeMap::new()).is_err());
        assert_eq!(fs::read_to_string(&p.file).unwrap(), "model = \"mock\"\n");
    }

    #[test]
    fn adjacent_bundle_selects_python_without_path_discovery_and_rejects_escapes() {
        let p = Profile::new();
        let root = p.root.join("graphify-runtime");
        fs::create_dir_all(root.join("python")).unwrap();
        fs::write(root.join("node"), "fixture").unwrap();
        fs::write(root.join("python/runtime"), "fixture").unwrap();
        let mut manifest = json!({"format":1,"graphify_version":"0.9.107",
            "platform":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "node":"node","python":"python/runtime"});
        fs::write(root.join("runtime.json"), manifest.to_string()).unwrap();
        let selected = select_programs(&InstallOptions::default(), &p.root).unwrap();
        assert!(matches!(selected.backend, Backend::Python(_)));
        let configured = configuration(
            "model = \"mock\"\n",
            &p.home,
            &selected,
            &p.home.join("assets"),
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(configured.contains("--python") && !configured.contains("--uvx"));
        manifest["python"] = "../outside".into();
        fs::write(root.join("runtime.json"), manifest.to_string()).unwrap();
        assert!(bundle_programs(&root).is_err());
        manifest["python"] = "python/runtime".into();
        manifest["graphify_version"] = "other".into();
        fs::write(root.join("runtime.json"), manifest.to_string()).unwrap();
        assert!(bundle_programs(&root).is_err());
    }
}
