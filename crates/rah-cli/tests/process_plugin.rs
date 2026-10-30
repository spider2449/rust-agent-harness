//! P4 acceptance for process plugins: the Python example runs as a guarded
//! tool provider that calls back into the host's `session` service.

mod common;

use std::process::Command;

use common::{TempDir, entries, new_session, reply, turn};
use rah_api::Kernel;
use rah_cli::{BootError, Distribution, boot};

/// A Python 3 interpreter on PATH, if any.
fn python() -> Option<&'static str> {
    ["python3", "python"].into_iter().find(|program| {
        Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).starts_with("Python 3")
            })
    })
}

fn forward(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn distribution(dir: &TempDir, program: &str) -> Distribution {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/process-plugins/word_tools.py");
    Distribution::parse(&format!(
        r#"
[[plugin]]
use = "policy-authority"
config = {{ tools = {{ "words.*" = "read" }} }}

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "process"
id = "word-tools"
config = {{ program = "{program}", args = ["{script}"] }}
"#,
        sessions = forward(&dir.sessions()),
        script = forward(&script),
    ))
    .unwrap()
}

#[tokio::test]
async fn the_python_example_provides_guarded_tools_and_reads_the_session() {
    let Some(python) = python() else {
        eprintln!("skipping: no Python 3 interpreter on PATH");
        return;
    };
    let dir = TempDir::new("process-python");
    let kernel = Kernel::new();
    boot(&kernel, &distribution(&dir, python)).await.unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let counted = reply(
        &turn(
            &handle,
            &session,
            r#"/call words.count {"text":"one two\nthree"}"#,
        )
        .await,
    );
    assert!(
        counted.contains(r#""words":3"#) && counted.contains(r#""lines":2"#),
        "{counted}"
    );

    let inputs = reply(&turn(&handle, &session, "/call words.session_inputs {}").await);
    assert!(inputs.contains("/call words.count"), "{inputs}");
    assert!(inputs.contains("/call words.session_inputs {}"), "{inputs}");

    let invalid = reply(&turn(&handle, &session, r#"/call words.count {"text":5}"#).await);
    assert!(
        invalid.contains("invalid input: `text` must be a string"),
        "{invalid}"
    );
    assert_eq!(entries(&handle, &session).await.len(), 15);
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
async fn the_python_guard_denies_and_redacts_through_hooks() {
    let Some(python) = python() else {
        eprintln!("skipping: no Python 3 interpreter on PATH");
        return;
    };
    let dir = TempDir::new("process-python-guard");
    let guard = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/process-plugins/tool_guard.py");
    let mut distribution = distribution(&dir, python);
    distribution.plugin.extend(
        Distribution::parse(&format!(
            r#"
[[plugin]]
use = "process"
id = "tool-guard"
config = {{ program = "{python}", args = ["{script}"], plugin = {{ block = ["password"] }} }}
"#,
            script = forward(&guard),
        ))
        .unwrap()
        .plugin,
    );
    let kernel = Kernel::new();
    boot(&kernel, &distribution).await.unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    // The policy would allow `words.count`; the guard runs first and denies.
    let denied = reply(
        &turn(
            &handle,
            &session,
            r#"/call words.count {"text":"my password"}"#,
        )
        .await,
    );
    assert!(
        denied.contains("denied: the arguments mention `password`"),
        "{denied}"
    );
    let counted = reply(&turn(&handle, &session, r#"/call words.count {"text":"a b"}"#).await);
    assert!(counted.contains(r#""words":2"#), "{counted}");

    // The earlier input is in the session log; the guard redacts it.
    let inputs = reply(&turn(&handle, &session, "/call words.session_inputs {}").await);
    assert!(inputs.contains("my [redacted]"), "{inputs}");
    assert!(!inputs.contains("password"), "{inputs}");
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
async fn process_plugins_need_an_id_and_a_startable_program() {
    let dir = TempDir::new("process-errors");
    let mut distribution = distribution(&dir, "rah-no-such-interpreter-x7");
    let error = boot(&Kernel::new(), &distribution).await.unwrap_err();
    assert!(
        matches!(&error, BootError::Process(message) if message.contains("cannot start")),
        "{error}"
    );
    distribution.plugin.last_mut().unwrap().id = None;
    let error = boot(&Kernel::new(), &distribution).await.unwrap_err();
    assert!(error.to_string().contains("needs an `id`"), "{error}");
}

fn todo_distribution(dir: &TempDir, program: &str) -> Distribution {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/process-plugins/todo_board.py");
    Distribution::parse(&format!(
        r#"
[[plugin]]
use = "storage-fs"
config = {{ dir = "{storage}" }}

[[plugin]]
use = "process"
id = "todo"
config = {{ program = "{program}", args = ["{script}"] }}
"#,
        storage = forward(&dir.storage()),
        script = forward(&script),
    ))
    .unwrap()
}

#[tokio::test]
async fn the_python_todo_board_is_a_view_kept_in_storage() {
    use rah_api::ui::{UI_VIEWS, UiChanged, UiError, ViewContext};
    let Some(python) = python() else {
        eprintln!("skipping: no Python 3 interpreter on PATH");
        return;
    };
    let dir = TempDir::new("process-python-todo");
    let kernel = Kernel::new();
    boot(&kernel, &todo_distribution(&dir, python))
        .await
        .unwrap();
    let changes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let _subscription = {
        let changes = changes.clone();
        kernel.handle().subscribe(move |changed: &UiChanged| {
            changes.lock().unwrap().push(changed.view.clone())
        })
    };
    let all = kernel.handle().get_all(UI_VIEWS);
    let (instance, views) = &all[0];
    assert_eq!(instance, "todo");
    assert_eq!(views.views()[0].title, "To-do");
    let cx = ViewContext::default();
    let text = |view: &rah_api::ui::View| serde_json::to_string(view).unwrap();

    assert!(text(&views.render("board", &cx).await.unwrap()).contains("0 item(s)."));
    let added = views
        .act(
            "board",
            "add",
            serde_json::json!({ "item": "write the ADR" }),
            &cx,
        )
        .await
        .unwrap();
    assert!(added.rerender);
    assert!(text(&views.render("board", &cx).await.unwrap()).contains("write the ADR"));
    assert!(matches!(
        views
            .act("board", "add", serde_json::json!({ "item": " " }), &cx)
            .await,
        Err(UiError::InvalidInput(_))
    ));
    assert!(matches!(
        views
            .act("board", "nope", serde_json::Value::Null, &cx)
            .await,
        Err(UiError::UnknownAction(_))
    ));
    for _ in 0..100 {
        if !changes.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(*changes.lock().unwrap(), ["board"]);
    kernel.shutdown().await.unwrap();

    // The items are in `storage`, so a new start shows them.
    let kernel = Kernel::new();
    boot(&kernel, &todo_distribution(&dir, python))
        .await
        .unwrap();
    let views = kernel.handle().get_all(UI_VIEWS).remove(0).1;
    assert!(text(&views.render("board", &cx).await.unwrap()).contains("write the ADR"));
}
