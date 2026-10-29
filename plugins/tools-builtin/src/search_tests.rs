use std::{path::PathBuf, sync::Arc};

use rah_api::{
    CancellationToken,
    tools::{Tool, ToolContext, ToolError},
};
use rah_protocol::{SessionId, ToolContent};
use serde_json::{Value, json};

use super::{FsGlob, FsGrep, FsList, Workspace};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-tools-search-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cx() -> ToolContext {
    ToolContext {
        session: SessionId::new(),
        call: rah_protocol::ToolCallId::new(),
        cancel: CancellationToken::new(),
    }
}

fn write(root: &std::path::Path, path: &str, content: &[u8]) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// A workspace `root/` with sources, a hidden file, a skipped `target/`, a
/// binary file and a large file, next to a `secret.txt` outside it.
fn fixture(name: &str, max_results: usize, max_scan: usize) -> (TempDir, Arc<Workspace>) {
    let dir = TempDir::new(name);
    let root = dir.0.join("root");
    write(&root, "Cargo.toml", b"[package]\nname = \"demo\"\n");
    write(
        &root,
        "src/lib.rs",
        b"pub fn alpha() {}\r\npub fn Beta() {}\n",
    );
    write(&root, "src/nested/deep.rs", b"// alpha again\n");
    write(&root, "docs/notes.md", b"Alpha notes\n");
    write(&root, ".hidden/config.rs", b"alpha hidden\n");
    write(&root, "target/build.rs", b"alpha built\n");
    write(&root, "data.bin", b"alpha\0binary");
    write(&root, "big.txt", &[b'a'; 64]);
    std::fs::write(dir.0.join("secret.txt"), "alpha secret").unwrap();
    let workspace = Arc::new(Workspace {
        root: std::fs::canonicalize(&root).unwrap(),
        max_bytes: 48,
        max_results,
        max_scan,
        skip_dirs: vec![".git".into(), "target".into()],
    });
    (dir, workspace)
}

async fn text(tool: &dyn Tool, input: Value) -> String {
    let output = tool.call(input, cx()).await.unwrap();
    assert!(!output.is_error);
    match output.content.as_slice() {
        [ToolContent::Text(text)] => text.clone(),
        other => panic!("unexpected content {other:?}"),
    }
}

#[tokio::test]
async fn fs_list_lists_one_level_in_name_order() {
    let (_dir, workspace) = fixture("list", 500, 20_000);
    let list = FsList(workspace);
    assert_eq!(
        text(&list, json!({})).await,
        "Cargo.toml (24 bytes)\nbig.txt (64 bytes)\ndata.bin (12 bytes)\ndocs/\nsrc/\ntarget/ (not expanded)"
    );
    assert_eq!(
        text(&list, json!({"path": "src", "depth": 2})).await,
        "src/lib.rs (36 bytes)\nsrc/nested/\nsrc/nested/deep.rs (15 bytes)"
    );
    let all = text(&list, json!({"depth": 3, "hidden": true})).await;
    assert!(all.contains(".hidden/config.rs"), "{all}");
    assert!(!all.contains("target/build.rs"), "{all}");
}

#[tokio::test]
async fn fs_list_rejects_files_bad_depth_and_paths_outside_the_root() {
    let (_dir, workspace) = fixture("list-bounds", 500, 20_000);
    let list = FsList(workspace);
    for input in [
        json!({"path": "Cargo.toml"}),
        json!({"path": "../"}),
        json!({"depth": 0}),
        json!({"depth": "2"}),
        json!({"hidden": "yes"}),
    ] {
        assert!(
            matches!(
                list.call(input.clone(), cx()).await,
                Err(ToolError::InvalidInput(_))
            ),
            "{input}"
        );
    }
}

#[tokio::test]
async fn fs_glob_matches_names_at_any_depth_and_skips_skip_dirs() {
    let (_dir, workspace) = fixture("glob", 500, 20_000);
    let glob = FsGlob(workspace);
    assert_eq!(
        text(&glob, json!({"pattern": "*.rs"})).await,
        "src/lib.rs\nsrc/nested/deep.rs"
    );
    assert_eq!(
        text(&glob, json!({"pattern": "*.rs", "path": "src/nested"})).await,
        "src/nested/deep.rs"
    );
    assert_eq!(
        text(&glob, json!({"pattern": "{docs,src}/*.{md,rs}"})).await,
        "docs/notes.md\nsrc/lib.rs"
    );
    assert_eq!(
        text(&glob, json!({"pattern": "*.rs", "hidden": true})).await,
        ".hidden/config.rs\nsrc/lib.rs\nsrc/nested/deep.rs"
    );
    assert_eq!(
        text(&glob, json!({"pattern": "*.py"})).await,
        "No files matched."
    );
    assert!(matches!(
        glob.call(json!({"pattern": "../*"}), cx()).await,
        Err(ToolError::InvalidInput(_))
    ));
}

#[tokio::test]
async fn fs_grep_reports_matching_lines_and_skipped_files() {
    let (_dir, workspace) = fixture("grep", 500, 20_000);
    let grep = FsGrep(workspace);
    assert_eq!(
        text(&grep, json!({"pattern": "alpha"})).await,
        "src/lib.rs:1: pub fn alpha() {}\nsrc/nested/deep.rs:1: // alpha again\n\
         [skipped 1 file(s) over 48 bytes]\n[skipped 1 binary or non-UTF-8 file(s)]"
    );
    assert_eq!(
        text(
            &grep,
            json!({"pattern": "^alpha|beta", "ignore_case": true, "glob": "*.{md,rs}"})
        )
        .await,
        "docs/notes.md:1: Alpha notes\nsrc/lib.rs:2: pub fn Beta() {}"
    );
    assert_eq!(
        text(&grep, json!({"pattern": "fn", "path": "src/lib.rs"})).await,
        "src/lib.rs:1: pub fn alpha() {}\nsrc/lib.rs:2: pub fn Beta() {}"
    );
    assert_eq!(
        text(&grep, json!({"pattern": "zzz", "path": "docs"})).await,
        "No matches."
    );
    for input in [
        json!({"pattern": "("}),
        json!({"pattern": "a", "glob": "{x"}),
        json!({"pattern": "a", "path": "../secret.txt"}),
    ] {
        assert!(
            matches!(
                grep.call(input.clone(), cx()).await,
                Err(ToolError::InvalidInput(_))
            ),
            "{input}"
        );
    }
}

#[tokio::test]
async fn searches_say_when_a_limit_cut_them_short() {
    let (_dir, workspace) = fixture("limits", 1, 20_000);
    let grep = FsGrep(workspace.clone());
    assert_eq!(
        text(&grep, json!({"pattern": "fn", "path": "src"})).await,
        "src/lib.rs:1: pub fn alpha() {}\n[stopped at 1 results; narrow the path or pattern]"
    );
    let glob = FsGlob(workspace);
    assert_eq!(
        text(&glob, json!({"pattern": "*.rs"})).await,
        "src/lib.rs\n[stopped at 1 results; narrow the path or pattern]"
    );

    let (_dir, workspace) = fixture("scan", 500, 2);
    let list = FsList(workspace);
    assert_eq!(
        text(&list, json!({})).await,
        "Cargo.toml (24 bytes)\nbig.txt (64 bytes)\n\
         [stopped after scanning 2 entries; narrow the path or pattern]"
    );
}

#[tokio::test]
async fn a_cancelled_search_stops() {
    let (_dir, workspace) = fixture("cancel", 500, 20_000);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let cx = ToolContext {
        session: SessionId::new(),
        call: rah_protocol::ToolCallId::new(),
        cancel,
    };
    assert!(matches!(
        FsGrep(workspace).call(json!({"pattern": "a"}), cx).await,
        Err(ToolError::Cancelled)
    ));
}

#[tokio::test]
async fn searches_do_not_follow_symbolic_links() {
    let (dir, workspace) = fixture("symlink", 500, 20_000);
    let link = dir.0.join("root").join("outside");
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&dir.0, &link).is_ok();
    #[cfg(windows)]
    // A junction needs no privilege where a symbolic link does.
    let created = std::os::windows::fs::symlink_dir(&dir.0, &link).is_ok()
        || std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(&link)
            .arg(&dir.0)
            .output()
            .is_ok_and(|output| output.status.success());
    if !created {
        eprintln!("skipping: symlink creation is not permitted on this host");
        return;
    }
    let grep = text(&FsGrep(workspace.clone()), json!({"pattern": "secret"})).await;
    assert_eq!(
        grep,
        "No matches.\n[skipped 1 file(s) over 48 bytes]\n[skipped 1 binary or non-UTF-8 file(s)]"
    );
    let list = text(&FsList(workspace.clone()), json!({"depth": 2})).await;
    assert!(list.contains("outside@"), "{list}");
    assert!(matches!(
        FsGrep(workspace).call(json!({"pattern": "a", "path": "outside"}), cx()).await,
        Err(ToolError::InvalidInput(message)) if message.contains("outside the workspace")
    ));
}
