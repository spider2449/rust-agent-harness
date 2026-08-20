//! Mechanical release gate for RAH dependency and Codex-isolation invariants.

use std::{
    fs,
    path::{Path, PathBuf},
};

const CORE_CRATES: &[&str] = &[
    "rah-core",
    "rah-model",
    "rah-protocol",
    "rah-runtime",
    "rah-sandbox",
    "rah-session",
    "rah-tools",
];
const PROVIDER_PACKAGES: &[&str] = &[
    "codex-core",
    "codex-protocol",
    "codex-app-server",
    "openai",
    "anthropic",
    "deepseek",
];

#[test]
fn cargo_manifests_do_not_depend_on_codex_rust_crates() {
    let root = workspace_root();
    for manifest in files_named(&root, "Cargo.toml") {
        let text = read(&manifest);
        for line in text.lines().map(str::trim) {
            assert!(
                !line.starts_with("codex-") && !line.contains("package = \"codex-"),
                "forbidden Codex Rust dependency in {}: {line}",
                manifest.display()
            );
        }
    }
}

#[test]
fn protocol_remains_dependency_bottom() {
    let manifest = workspace_root().join("crates/rah-protocol/Cargo.toml");
    for line in read(&manifest).lines().map(str::trim) {
        assert!(
            !line.starts_with("rah-"),
            "rah-protocol must not depend on another RAH crate: {line}"
        );
    }
}

#[test]
fn core_crates_have_no_provider_dependencies() {
    let root = workspace_root();
    for crate_name in CORE_CRATES {
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
        let text = read(&manifest).to_ascii_lowercase();
        for provider in PROVIDER_PACKAGES {
            assert!(
                !text.lines().map(str::trim).any(|line| {
                    line.starts_with(provider) || line.contains(&format!("package = \"{provider}"))
                }),
                "provider dependency `{provider}` entered {}",
                manifest.display()
            );
        }
    }
}

#[test]
fn app_server_implementation_details_are_adapter_local() {
    let root = workspace_root();
    let crates = root.join("crates");
    let adapter = crates.join("rah-runtime-codex");
    for path in source_files(&crates) {
        if path.starts_with(&adapter) {
            continue;
        }
        let text = read(&path);
        for forbidden in [
            "AppServerTransport",
            "ProcessTransport",
            "codex_thread_id",
            "CodexThreadId",
            "item/agentMessage/delta",
            "turn/interrupt",
        ] {
            assert!(
                !text.contains(forbidden),
                "Codex process/protocol detail `{forbidden}` escaped into {}",
                path.display()
            );
        }
    }
}

#[test]
fn codex_wire_types_are_not_public() {
    let source = workspace_root().join("crates/rah-runtime-codex/src");
    for path in source_files(&source) {
        let text = read(&path);
        for line in text.lines().map(str::trim) {
            if !line.starts_with("pub ") && !line.starts_with("pub use") {
                continue;
            }
            for private_type in [
                "AppServerTransport",
                "ProcessTransport",
                "ConnectionEvent",
                "Incoming",
                "serde_json::Value",
            ] {
                assert!(
                    !line.contains(private_type),
                    "private Codex type `{private_type}` appears in public API at {}: {line}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn tool_security_regression_test_remains_present() {
    let test = read(&workspace_root().join("crates/rah-runtime-codex/src/runtime_tests.rs"));
    for evidence in [
        "codex_tool_items_fail_without_emitting_rah_tool_events",
        "AgentEvent::ToolRequested",
        "AgentEvent::ToolStarted",
        "AgentEvent::ToolFinished",
    ] {
        assert!(
            test.contains(evidence),
            "Codex tool-boundary regression evidence `{evidence}` is missing"
        );
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("adapter crate is nested under workspace/crates")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn files_named(root: &Path, name: &str) -> Vec<PathBuf> {
    walk(root)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|file_name| file_name == name))
        .collect()
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    walk(root)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect()
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("failed to list {}: {error}", directory.display()))
        {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                if path.file_name().is_none_or(|name| name != "target") {
                    directories.push(path);
                }
            } else {
                files.push(path);
            }
        }
    }
    files
}
