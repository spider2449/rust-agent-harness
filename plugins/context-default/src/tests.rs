use rah_api::{
    CancellationToken, Kernel, MountOptions,
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolInput};

use super::*;

fn user(text: &str) -> ContextItem {
    ContextItem::User { text: text.into() }
}

fn assistant(text: &str) -> ContextItem {
    ContextItem::Assistant { text: text.into() }
}

fn call(id: &str) -> ContextItem {
    ContextItem::ToolCall {
        call_id: id.into(),
        name: ToolName::new("echo"),
        arguments: json!({}),
    }
}

fn result(id: &str) -> ContextItem {
    ContextItem::ToolResult {
        call_id: id.into(),
        output: ToolOutput {
            content: vec![ToolContent::Text("r".into())],
            is_error: false,
        },
    }
}

fn request(items: Vec<ContextItem>) -> ModelRequest {
    ModelRequest {
        items,
        ..ModelRequest::default()
    }
}

const WIDE: Window = Window {
    max_items: 100,
    max_chars: 100_000,
};

#[test]
fn a_wide_window_without_settings_changes_nothing() {
    let items = vec![user("a"), call("1"), result("1"), assistant("b"), user("c")];
    assert_eq!(shape(request(items.clone()), None, None, WIDE).items, items);
}

#[test]
fn the_window_starts_at_a_user_message_and_marks_the_omission() {
    let items = vec![
        user("one"),
        call("1"),
        result("1"),
        assistant("two"),
        user("three"),
        call("2"),
        result("2"),
        assistant("four"),
        user("five"),
    ];
    // Four items reach back to `result("2")`; the window moves to `five`.
    let window = Window {
        max_items: 4,
        max_chars: 100_000,
    };
    let shaped = shape(request(items.clone()), None, None, window).items;
    assert_eq!(
        shaped[0],
        ContextItem::System {
            text: "[8 earlier conversation items are omitted.]".into()
        }
    );
    assert_eq!(&shaped[1..], &items[8..]);
    // Six items reach `user("three")` exactly.
    let window = Window {
        max_items: 5,
        max_chars: 100_000,
    };
    let shaped = shape(request(items.clone()), None, None, window).items;
    assert_eq!(&shaped[1..], &items[4..]);
}

#[test]
fn the_current_turn_is_kept_even_when_it_exceeds_the_limits() {
    let items = vec![
        user("old"),
        assistant("x"),
        user("now"),
        call("1"),
        result("1"),
        call("2"),
        result("2"),
    ];
    let window = Window {
        max_items: 2,
        max_chars: 100_000,
    };
    let shaped = shape(request(items.clone()), None, None, window).items;
    assert_eq!(&shaped[1..], &items[2..]);
    let window = Window {
        max_items: 100,
        max_chars: 1,
    };
    assert_eq!(
        &shape(request(items.clone()), None, None, window).items[1..],
        &items[2..]
    );
}

fn screenshot(id: &str) -> ContextItem {
    ContextItem::ToolResult {
        call_id: id.into(),
        output: ToolOutput {
            content: vec![
                ToolContent::Text("window".into()),
                ToolContent::Image(rah_protocol::ImageContent {
                    media_type: "image/png".into(),
                    data: "A".repeat(400_000),
                }),
            ],
            is_error: false,
        },
    }
}

#[test]
fn images_are_sent_in_the_current_turn_only_and_count_a_fixed_size() {
    let items = vec![
        user("one"),
        call("1"),
        screenshot("1"),
        assistant("two"),
        user("three"),
        call("2"),
        screenshot("2"),
    ];
    // Two images of 400,000 characters each fit, since each counts as
    // IMAGE_CHARS.
    let window = Window {
        max_items: 100,
        max_chars: 3 * IMAGE_CHARS,
    };
    let shaped = shape(request(items.clone()), None, None, window).items;
    assert_eq!(shaped.len(), items.len(), "{shaped:#?}");
    let ContextItem::ToolResult { output, .. } = &shaped[2] else {
        panic!("{shaped:#?}");
    };
    assert_eq!(
        output.content,
        vec![
            ToolContent::Text("window".into()),
            ToolContent::Text(
                "[image/png image, 293 KiB] (from an earlier message; not shown again)".into()
            ),
        ]
    );
    assert_eq!(shaped[6], items[6], "the current turn keeps its image");
}

#[test]
fn system_prompt_and_notes_come_first() {
    let items = vec![
        ContextItem::System {
            text: "from a skill".into(),
        },
        user("hi"),
    ];
    let shaped = shape(
        request(items),
        Some("You are RAH."),
        Some("- likes tea\n"),
        WIDE,
    )
    .items;
    assert_eq!(
        shaped,
        vec![
            ContextItem::System {
                text: "You are RAH.".into()
            },
            ContextItem::System {
                text: "from a skill".into()
            },
            ContextItem::System {
                text: "Notes saved with memory.remember:\n- likes tea".into()
            },
            user("hi"),
        ]
    );
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn remembered_notes_reach_later_requests() {
    let dir = TempDir(std::env::temp_dir().join(format!("rah-context-{}", std::process::id())));
    let file = dir.0.join("nested/memory.md");
    let kernel = Kernel::new();
    let config = json!({ "system": "Be brief.", "memory_file": file, "max_memory_bytes": 40 });
    kernel
        .mount(
            Arc::new(ContextPlugin::new("context")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    for note in ["first   note\nwith a newline", "second note", "third note"] {
        let outcome = invoker
            .invoke(
                Invocation {
                    session: SessionId::new(),
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("memory.remember"),
                        input: ToolInput(json!({ "note": note })),
                    },
                    cancel: CancellationToken::new(),
                },
                || {},
            )
            .await;
        assert!(!outcome.output.is_error, "{:?}", outcome.output);
    }
    let shaped = kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request(vec![user("hi")]))
        .await
        .unwrap()
        .items;
    assert_eq!(
        shaped[0],
        ContextItem::System {
            text: "Be brief.".into()
        }
    );
    // Only the newest whole lines within 40 bytes are shown.
    assert_eq!(
        shaped[1],
        ContextItem::System {
            text: "Notes saved with memory.remember:\n- second note\n- third note".into()
        }
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "- first note with a newline\n- second note\n- third note\n"
    );
}

#[test]
fn workspace_memory_files_are_named_after_the_folder_and_its_path() {
    // `\` separates components only on Windows.
    let (here, elsewhere) = if cfg!(windows) {
        (r"D:\work\My Repo", r"E:\other\My Repo")
    } else {
        ("/work/My Repo", "/other/My Repo")
    };
    let a = workspace_file_name(Path::new(here));
    assert!(a.starts_with("My_Repo-") && a.ends_with(".md"), "{a}");
    // Same folder name elsewhere: another file.
    assert_ne!(a, workspace_file_name(Path::new(elsewhere)));
    // Stable across calls.
    assert_eq!(a, workspace_file_name(Path::new(here)));
    if cfg!(windows) {
        assert_eq!(a, workspace_file_name(Path::new(r"d:\WORK\My Repo")));
    }
    assert!(workspace_file_name(Path::new("/")).starts_with("workspace-"));
}

/// A sandbox whose root the test moves, as switching the workspace does.
struct MovableRoot(std::sync::Mutex<PathBuf>);

#[async_trait]
impl rah_api::sandbox::ExecEnv for MovableRoot {
    fn root(&self) -> Option<PathBuf> {
        Some(self.0.lock().unwrap().clone())
    }

    async fn run(
        &self,
        _spec: rah_api::sandbox::ProgramSpec,
        _cancel: CancellationToken,
    ) -> Result<rah_api::sandbox::ExecOutput, rah_api::sandbox::ExecError> {
        Err(rah_api::sandbox::ExecError::InvalidSpec(
            "not in this test".into(),
        ))
    }
}

struct SandboxPlugin(PluginManifest, Arc<MovableRoot>);

#[async_trait]
impl Plugin for SandboxPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(
            SANDBOX,
            self.1.clone() as Arc<dyn rah_api::sandbox::ExecEnv>,
        )?;
        Ok(())
    }
}

async fn remember(kernel: &Kernel, note: &str) {
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("memory.remember"),
                    input: ToolInput(json!({ "note": note })),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await;
    assert!(!outcome.output.is_error, "{:?}", outcome.output);
}

async fn notes(kernel: &Kernel) -> Vec<String> {
    kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request(vec![user("hi")]))
        .await
        .unwrap()
        .items
        .into_iter()
        .filter_map(|item| match item {
            ContextItem::System { text } => Some(text),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn each_workspace_keeps_its_own_memory() {
    let dir = TempDir(std::env::temp_dir().join(format!("rah-context-ws-{}", std::process::id())));
    let root = Arc::new(MovableRoot(std::sync::Mutex::new(PathBuf::from(
        r"D:\work\alpha",
    ))));
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(SandboxPlugin(
                PluginManifest::new("sandbox", "1").provides(["sandbox"]),
                root.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(ContextPlugin::new("context")),
            MountOptions::with_config(json!({ "memory_dir": dir.0.join("memory") })),
        )
        .await
        .unwrap();

    remember(&kernel, "alpha uses tabs").await;
    assert_eq!(
        notes(&kernel).await,
        ["Notes saved with memory.remember:\n- alpha uses tabs"]
    );

    // Another workspace starts without notes and keeps its own.
    *root.0.lock().unwrap() = PathBuf::from(r"D:\work\beta");
    assert!(notes(&kernel).await.is_empty());
    remember(&kernel, "beta is Python").await;
    assert_eq!(
        notes(&kernel).await,
        ["Notes saved with memory.remember:\n- beta is Python"]
    );

    // Back in the first one, only its notes return.
    *root.0.lock().unwrap() = PathBuf::from(r"D:\work\alpha");
    assert_eq!(
        notes(&kernel).await,
        ["Notes saved with memory.remember:\n- alpha uses tabs"]
    );
    let files = std::fs::read_dir(dir.0.join("memory")).unwrap().count();
    assert_eq!(files, 2);
}

#[tokio::test]
async fn a_memory_file_and_a_memory_dir_are_exclusive() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(ContextPlugin::new("context")),
            MountOptions::with_config(json!({ "memory_file": "a.md", "memory_dir": "b" })),
        )
        .await
        .unwrap();
    assert!(
        matches!(status, rah_api::PluginStatus::Failed { ref message } if message.contains("not both")),
        "{status:?}"
    );
}
