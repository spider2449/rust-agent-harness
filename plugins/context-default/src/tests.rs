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
