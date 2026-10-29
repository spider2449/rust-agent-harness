//! P6 acceptance for `scheduler-basic`: subagents through the loop service,
//! and background jobs.

mod common;

use std::time::Duration;

use common::{TempDir, headless, kernel, new_session, reply, turn};
use rah_api::{
    KernelHandle,
    scheduler::{JobSpec, JobState, SCHEDULER},
};
use serde_json::json;

async fn wait_for(
    kernel: &KernelHandle,
    id: &str,
    done: impl Fn(&rah_api::scheduler::JobInfo) -> bool,
) -> rah_api::scheduler::JobInfo {
    for _ in 0..200 {
        let job = kernel
            .get(SCHEDULER)
            .unwrap()
            .list()
            .into_iter()
            .find(|job| job.id == id)
            .unwrap();
        if done(&job) {
            return job;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} did not reach the expected state");
}

#[tokio::test]
async fn delegation_runs_a_subagent_turn_in_its_own_session() {
    let dir = TempDir::new("scheduler-delegate");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let text = reply(
        &turn(
            &handle,
            &session,
            r#"/call agent.delegate {"task":"hello"}"#,
        )
        .await,
    );
    assert!(
        text.starts_with("[model-mock] agent.delegate returned: [model-mock] echo returned: hello"),
        "{text}"
    );
    // The parent and the subagent each have a session.
    let sessions = handle
        .get(rah_api::session::SESSION)
        .unwrap()
        .list()
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
}

#[tokio::test]
async fn delegation_depth_is_limited() {
    let dir = TempDir::new("scheduler-depth");
    let mut distribution = headless(&dir);
    distribution.set_config("scheduler-basic", "max_depth", json!(1));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let nested = r#"/call agent.delegate {"task":"/call agent.delegate {\"task\":\"x\"}"}"#;
    let text = reply(&turn(&handle, &session, nested).await);
    assert!(text.contains("at most 1 levels deep"), "{text}");
}

#[tokio::test]
async fn jobs_run_in_the_background_and_can_be_cancelled() {
    let dir = TempDir::new("scheduler-jobs");
    let mut distribution = headless(&dir);
    distribution.set_config(
        "scheduler-basic",
        "jobs",
        json!([{ "input": "/notool from configuration" }]),
    );
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let scheduler = handle.get(SCHEDULER).unwrap();

    let configured = scheduler.list().remove(0);
    let configured = wait_for(&handle, &configured.id, |job| {
        matches!(job.state, JobState::Done { .. })
    })
    .await;
    assert_eq!(
        configured.state,
        JobState::Done {
            answer: "[model-mock] you said: from configuration (0 earlier items)".into()
        }
    );
    assert!(configured.last_session.is_some());

    let once = scheduler
        .submit(JobSpec {
            input: "hello".into(),
            session: None,
            delay: Duration::ZERO,
            every: None,
        })
        .unwrap();
    let once = wait_for(&handle, &once.id, |job| job.runs == 1).await;
    assert_eq!(
        once.state,
        JobState::Done {
            answer: "[model-mock] echo returned: hello".into()
        }
    );
    assert!(
        !scheduler.cancel(&once.id),
        "a finished one-shot job is over"
    );

    let repeating = scheduler
        .submit(JobSpec {
            input: "/notool tick".into(),
            session: None,
            delay: Duration::ZERO,
            every: Some(Duration::from_secs(1)),
        })
        .unwrap();
    let repeated = wait_for(&handle, &repeating.id, |job| job.runs >= 2).await;
    assert_ne!(repeated.last_session, None);
    assert!(scheduler.cancel(&repeating.id));
    let runs = wait_for(&handle, &repeating.id, |job| {
        job.state == JobState::Cancelled
    })
    .await
    .runs;
    tokio::time::sleep(Duration::from_millis(1300)).await;
    let after = scheduler
        .list()
        .into_iter()
        .find(|job| job.id == repeating.id)
        .unwrap();
    assert_eq!((after.runs, after.state), (runs, JobState::Cancelled));

    assert!(
        scheduler
            .submit(JobSpec {
                input: " ".into(),
                session: None,
                delay: Duration::ZERO,
                every: None,
            })
            .is_err()
    );
}

#[tokio::test]
async fn a_named_subagent_starts_from_its_instructions() {
    let dir = TempDir::new("scheduler-agents");
    std::fs::create_dir_all(dir.0.join("agents")).unwrap();
    std::fs::write(
        dir.0.join("agents/reviewer.md"),
        "---\ndescription: Reviews a change.\n---\nCheck the tests first.\n",
    )
    .unwrap();
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let definition = rah_api::tools::ToolInvoker::new(handle.clone())
        .definitions()
        .into_iter()
        .find(|definition| definition.name.as_str() == "agent.delegate")
        .unwrap();
    assert!(
        definition
            .description
            .contains("\n- reviewer: Reviews a change."),
        "{}",
        definition.description
    );
    assert_eq!(
        definition.input_schema["properties"]["agent"]["enum"],
        json!(["reviewer"])
    );

    let session = new_session(&handle).await;
    let text = reply(
        &turn(
            &handle,
            &session,
            r#"/call agent.delegate {"task":"look at main.rs","agent":"reviewer"}"#,
        )
        .await,
    );
    assert!(text.contains("agent.delegate returned"), "{text}");
    let sessions = handle.get(rah_api::session::SESSION).unwrap();
    let child = sessions
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|info| info.id != session)
        .unwrap();
    let records = sessions.read(&child.id).await.unwrap();
    let rah_protocol::SessionEntry::TurnStarted { input, .. } = &records[0].entry else {
        panic!("{records:?}")
    };
    assert_eq!(
        input,
        "You are the `reviewer` subagent. Follow these instructions:\n\n\
         Check the tests first.\n\nYour task:\n\nlook at main.rs"
    );

    let text = reply(
        &turn(
            &handle,
            &session,
            r#"/call agent.delegate {"task":"x","agent":"nobody"}"#,
        )
        .await,
    );
    assert!(
        text.contains("there is no agent `nobody`; use one of: reviewer"),
        "{text}"
    );
}
