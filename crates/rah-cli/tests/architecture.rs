//! ADR 0036 dependency rules, checked against Cargo's package identities.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    // Cargo reports the package name here, even when the manifest uses an alias.
    name: String,
}

#[test]
fn workspace_dependencies_follow_the_plugin_kernel_layers() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("rah-cli lives in <workspace>/crates/rah-cli");
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--locked",
            "--offline",
        ])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Metadata = serde_json::from_slice(&output.stdout).expect("Cargo metadata");
    let members: HashSet<_> = metadata.packages.iter().map(|p| p.name.as_str()).collect();
    let mut checked = HashSet::new();
    let mut plugins = 0;
    for package in &metadata.packages {
        let allowed: &[&str] = match package.name.as_str() {
            "rah-kernel" | "rah-protocol" => &[],
            "rah-api" => &["rah-kernel", "rah-protocol"],
            "rah-plugin-host-process" | "rah-plugin-host-wasm" => &["rah-api", "rah-protocol"],
            _ if package.manifest_path.starts_with(workspace.join("plugins")) => {
                plugins += 1;
                &["rah-api", "rah-protocol"]
            }
            _ => continue,
        };
        checked.insert(package.name.as_str());
        for dependency in &package.dependencies {
            let name = dependency.name.as_str();
            if members.contains(name) || name.starts_with("rah-") {
                assert!(
                    allowed.contains(&name),
                    "{} depends on {name}",
                    package.name
                );
            }
        }
    }
    assert!(plugins >= 5, "Cargo metadata must include the plugins");
    for name in [
        "rah-kernel",
        "rah-protocol",
        "rah-api",
        "rah-plugin-host-process",
        "rah-plugin-host-wasm",
    ] {
        assert!(checked.contains(name), "Cargo metadata must include {name}");
    }
}
