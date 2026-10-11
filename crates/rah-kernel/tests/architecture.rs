//! ADR 0036: the kernel contains no agent domain logic and depends on no
//! other RAH crate.

use std::{fs, path::Path};

#[test]
fn kernel_has_no_rah_crate_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("kernel manifest should be readable");
    let offending: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("rah-") || line.starts_with("rah_"))
        .collect();
    assert!(
        offending.is_empty(),
        "kernel depends on RAH crates: {offending:?}"
    );
}

#[test]
fn kernel_source_stays_within_size_budget() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let lines: usize = fs::read_dir(&src)
        .expect("kernel src should be listable")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| fs::read_to_string(path).expect("source").lines().count())
        .sum();
    assert!(
        lines < 2000,
        "rah-kernel src has {lines} lines; budget is 2000"
    );
}
