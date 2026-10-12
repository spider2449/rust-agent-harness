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
