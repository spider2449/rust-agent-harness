use std::sync::Mutex;

use rah_api::{
    CancellationToken, Kernel, MountOptions,
    host::HostNotice,
    sandbox::{ExecEnv, ExecError, ExecOutput, ProgramSpec},
};
use serde_json::json;

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("rah-instructions-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn folder(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A sandbox whose root can be moved, as switching the workspace does.
struct MovableRoot(Mutex<PathBuf>);

#[async_trait]
impl ExecEnv for MovableRoot {
    fn root(&self) -> Option<PathBuf> {
        Some(self.0.lock().unwrap().clone())
    }

    async fn run(
        &self,
        _spec: ProgramSpec,
        _cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError> {
        Err(ExecError::InvalidSpec("not in this test".into()))
    }
}

struct SandboxPlugin(PluginManifest, Arc<MovableRoot>);

#[async_trait]
impl Plugin for SandboxPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(SANDBOX, self.1.clone() as Arc<dyn ExecEnv>)?;
        Ok(())
    }
}

/// A kernel with the plugin and, with `root`, a sandbox at that root.
async fn mounted(config: serde_json::Value, root: Option<Arc<MovableRoot>>) -> Kernel {
    let kernel = Kernel::new();
    if let Some(root) = root {
        kernel
            .mount(
                Arc::new(SandboxPlugin(
                    PluginManifest::new("sandbox", "1").provides(["sandbox"]),
                    root,
                )),
                MountOptions::default(),
            )
            .await
            .unwrap();
    }
    kernel
        .mount(
            Arc::new(InstructionsPlugin::new("instructions")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    kernel
}

/// The items of a request that starts with a system prompt and one user
/// message, after the hook.
async fn shaped(kernel: &Kernel) -> Vec<ContextItem> {
    let request = ModelRequest {
        items: vec![
            ContextItem::System {
                text: "prompt".into(),
            },
            ContextItem::User { text: "hi".into() },
        ],
        ..ModelRequest::default()
    };
    kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request)
        .await
        .unwrap()
        .items
}

fn system(text: &str) -> ContextItem {
    ContextItem::System { text: text.into() }
}

#[tokio::test]
async fn the_workspace_file_follows_the_workspace_and_comes_after_the_prompt() {
    let dir = TempDir::new("follow");
    let alpha = dir.folder("alpha");
    let beta = dir.folder("beta");
    let empty = dir.folder("empty");
    std::fs::write(alpha.join("AGENTS.md"), "\u{feff}Run cargo test.\n").unwrap();
    std::fs::write(beta.join("CLAUDE.md"), "Use tabs.").unwrap();
    std::fs::write(beta.join("AGENTS.md"), "Beta rules.").unwrap();
    let root = Arc::new(MovableRoot(Mutex::new(alpha)));
    let kernel = mounted(
        json!({ "names": ["CLAUDE.md", "AGENTS.md"] }),
        Some(root.clone()),
    )
    .await;

    assert_eq!(
        shaped(&kernel).await,
        [
            system("prompt"),
            system("Instructions from `AGENTS.md` in the workspace:\n\nRun cargo test."),
            ContextItem::User { text: "hi".into() },
        ]
    );
    // Only the first name that exists is read.
    *root.0.lock().unwrap() = beta;
    assert_eq!(
        shaped(&kernel).await[1],
        system("Instructions from `CLAUDE.md` in the workspace:\n\nUse tabs.")
    );
    *root.0.lock().unwrap() = empty;
    assert_eq!(shaped(&kernel).await.len(), 2);
}

#[tokio::test]
async fn configured_files_are_added_and_a_missing_one_is_told_to_the_user_once() {
    let dir = TempDir::new("files");
    let rules = dir.0.join("guardrails.md");
    std::fs::write(&rules, "Never push.").unwrap();
    let missing = dir.0.join("missing.md");
    let kernel = mounted(json!({ "files": [rules, missing] }), None).await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = seen.clone();
    let _guard = kernel
        .handle()
        .subscribe(move |notice: &HostNotice| sink.lock().unwrap().push(notice.message.clone()));

    for _ in 0..2 {
        assert_eq!(
            shaped(&kernel).await[1],
            system("Instructions from `guardrails.md`:\n\nNever push.")
        );
    }
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!(seen[0].contains("missing.md") && seen[0].contains("does not exist"));
}

#[tokio::test]
async fn a_long_file_is_cut_at_a_character_boundary_with_a_note() {
    let dir = TempDir::new("long");
    let workspace = dir.folder("ws");
    // 'é' is two bytes, so 9 bytes end inside the fifth one.
    std::fs::write(workspace.join("AGENTS.md"), "é".repeat(20)).unwrap();
    let root = Arc::new(MovableRoot(Mutex::new(workspace)));
    let kernel = mounted(json!({ "max_bytes": 9 }), Some(root)).await;
    let ContextItem::System { text } = &shaped(&kernel).await[1] else {
        panic!("no instructions")
    };
    assert_eq!(
        text,
        &format!(
            "Instructions from `AGENTS.md` in the workspace:\n\n{}\n\
             [The file has 40 bytes; only the first 9 are shown.]",
            "é".repeat(4)
        )
    );
}

#[test]
fn names_must_be_file_names_in_the_root() {
    let plugin = InstructionsPlugin::new("instructions");
    for name in ["../AGENTS.md", "docs/AGENTS.md", "", "/etc/passwd"] {
        assert!(
            plugin.check_config(&json!({ "names": [name] })).is_err(),
            "{name}"
        );
    }
    assert!(
        plugin
            .check_config(&json!({ "names": ["AGENTS.md", ".rules"] }))
            .is_ok()
    );
}
