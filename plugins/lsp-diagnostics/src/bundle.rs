//! Resolve the optional, relocatable distribution beside the executable.
//! This only changes defaults; explicit server commands still win.

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use crate::ServerConfig;

pub(crate) fn discover() -> Result<BTreeMap<String, ServerConfig>, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    match executable.parent() {
        Some(parent) => load(&parent.join("lsp-runtime")),
        None => Ok(BTreeMap::new()),
    }
}

pub(crate) fn load(root: &Path) -> Result<BTreeMap<String, ServerConfig>, String> {
    if !root.exists() {
        return Ok(BTreeMap::new());
    }
    let bytes = std::fs::read(root.join("runtime.json"))
        .map_err(|error| format!("Cannot read bundled LSP runtime: {error}"))?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if manifest["format"] != 1
        || manifest["platform"] != std::env::consts::OS
        || manifest["arch"] != std::env::consts::ARCH
    {
        return Err("Bundled LSP runtime format or platform does not match this executable".into());
    }
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let resolve = |relative: &str| -> Result<String, String> {
        let file = root
            .join(relative)
            .canonicalize()
            .map_err(|error| format!("Bundled LSP file `{relative}` is missing: {error}"))?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(format!("Bundled LSP file `{relative}` escapes its runtime"));
        }
        let text = file.to_string_lossy().into_owned();
        // Node and language servers expect conventional Windows paths.
        Ok(if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{unc}")
        } else {
            text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
        })
    };
    let node = resolve("node.exe")?;
    let tsserver = resolve("node_modules/typescript/lib/tsserver.js")?;
    let mut result = BTreeMap::new();
    for (name, command) in [
        ("rust-analyzer", vec![resolve("rust-analyzer.exe")?]),
        (
            "typescript",
            vec![
                node.clone(),
                resolve("node_modules/typescript-language-server/lib/cli.mjs")?,
                "--stdio".into(),
            ],
        ),
        (
            "pyright",
            vec![
                node,
                resolve("node_modules/pyright/langserver.index.js")?,
                "--stdio".into(),
            ],
        ),
        ("clangd", vec![resolve("clangd/bin/clangd.exe")?]),
    ] {
        result.insert(
            name.into(),
            ServerConfig {
                command: Some(command),
                initialization: (name == "typescript").then(|| {
                    serde_json::json!({
                        "tsserver": { "path": tsserver },
                        "disableAutomaticTypingAcquisition": true
                    })
                }),
                ..ServerConfig::default()
            },
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relocated_bundle_uses_absolute_paths_and_rejects_incomplete_installations() {
        let root = std::env::temp_dir().join(format!("rah lsp {}", uuid::Uuid::new_v4()));
        assert!(load(&root).unwrap().is_empty());
        std::fs::create_dir_all(&root).unwrap();
        let manifest = serde_json::json!({"format": 1, "platform": std::env::consts::OS, "arch": std::env::consts::ARCH});
        std::fs::write(root.join("runtime.json"), manifest.to_string()).unwrap();
        assert!(load(&root).is_err());
        for relative in [
            "node.exe",
            "rust-analyzer.exe",
            "node_modules/typescript-language-server/lib/cli.mjs",
            "node_modules/typescript/lib/tsserver.js",
            "node_modules/pyright/langserver.index.js",
            "clangd/bin/clangd.exe",
        ] {
            let file = root.join(relative);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "fixture").unwrap();
        }
        let servers = load(&root).unwrap();
        let command = servers["typescript"].command.as_ref().unwrap();
        assert!(Path::new(&command[0]).is_absolute());
        assert!(command[1].ends_with("cli.mjs"));
        assert_eq!(command[2], "--stdio");
        std::fs::write(root.join("runtime.json"), r#"{"format":2}"#).unwrap();
        assert!(load(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
