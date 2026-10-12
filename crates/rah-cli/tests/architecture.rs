//! ADR 0036 dependency rules: plugins depend only on `rah-api` and
//! `rah-protocol`; `rah-api` depends only on `rah-kernel` and `rah-protocol`.

use std::{fs, path::Path};

fn rah_dependencies(manifest: &Path) -> Vec<String> {
    let text = fs::read_to_string(manifest).expect("manifest should be readable");
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            let name = line.split(['=', ' ', '.']).next()?;
            name.starts_with("rah-").then(|| name.to_owned())
        })
        .collect()
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("rah-cli lives in <workspace>/crates/rah-cli")
}

#[test]
fn plugins_depend_only_on_the_api_and_protocol() {
    let mut checked = 0;
    for entry in fs::read_dir(workspace().join("plugins")).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        if !manifest.exists() {
            continue;
        }
        for dependency in rah_dependencies(&manifest) {
            assert!(
                ["rah-api", "rah-protocol"].contains(&dependency.as_str()),
                "{} depends on {dependency}",
                manifest.display()
            );
        }
        checked += 1;
    }
    assert!(checked >= 5);
}

#[test]
fn the_process_host_depends_only_on_the_api_and_protocol() {
    let manifest = workspace().join("crates/rah-plugin-host-process/Cargo.toml");
    for dependency in rah_dependencies(&manifest) {
        assert!(
            ["rah-api", "rah-protocol"].contains(&dependency.as_str()),
            "rah-plugin-host-process depends on {dependency}"
        );
    }
}

#[test]
fn api_depends_only_on_kernel_and_protocol() {
    for dependency in rah_dependencies(&workspace().join("crates/rah-api/Cargo.toml")) {
        assert!(
            ["rah-kernel", "rah-protocol"].contains(&dependency.as_str()),
            "rah-api depends on {dependency}"
        );
    }
}
