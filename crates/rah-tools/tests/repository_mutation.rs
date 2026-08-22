use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
use rah_tools::{
    RepositoryMutationFixtureTestMode, RepositoryMutationFixtureTool, Tool, ToolContext, ToolError,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-repository-mutation-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("test base should be created");
        Self(path)
    }

    fn repository(&self) -> PathBuf {
        let root = self.0.join("repository");
        fs::create_dir(&root).expect("fixture repository should be created");
        fs::write(root.join("marker.txt"), "before\n").expect("marker should be created");
        fs::write(root.join("protected.txt"), "protected\n").expect("protected should be created");
        root
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..5 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rah_execute_fixture"))
}

fn copied_fixture(root: &Path) -> PathBuf {
    let extension = fixture()
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let copy = root.join(format!("configured-fixture{extension}"));
    fs::copy(fixture(), &copy).unwrap();
    copy
}

fn json_content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("fixture result should contain one JSON value")
    };
    value
}

async fn run(tool: &RepositoryMutationFixtureTool) -> rah_protocol::ToolOutput {
    tool.execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect("fixture invocation should return a bounded result")
}

#[tokio::test]
async fn authorized_marker_mutation_is_the_only_successful_effect() {
    let base = TestDirectory::new("normal");
    let root = base.repository();
    let tool = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap();

    let definition = tool.definition();
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    let output = run(&tool).await;
    assert!(!output.is_error);
    assert_eq!(
        json_content(&output),
        &json!({"status":"ok","target":"fixture-marker","changed":true,"partial":false,"uncertain":false})
    );
    assert_eq!(fs::read(root.join("marker.txt")).unwrap(), b"after\n");
    assert_eq!(
        fs::read(root.join("protected.txt")).unwrap(),
        b"protected\n"
    );
    assert!(!root.join("extra.txt").exists());
}

#[tokio::test]
async fn model_cannot_supply_paths_or_process_controls() {
    let base = TestDirectory::new("input");
    let root = base.repository();
    let tool = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap();
    for input in [
        json!({"target":"fixture-marker"}),
        json!({"path":"marker.txt"}),
        json!({"path":"../marker.txt"}),
        json!({"path":"C:\\outside"}),
        json!({"executable":"other.exe"}),
        json!({"argv":["anything"]}),
        json!({"cwd":"."}),
        json!({"environment":{"PATH":"attacker"}}),
        json!({"timeout":1}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(input), ToolContext::default()).await,
            Err(ToolError::InvalidInput { .. })
        ));
    }
}

#[tokio::test]
async fn stale_root_and_target_identity_fail_before_spawn() {
    let base = TestDirectory::new("identity");
    let root = base.repository();
    let tool = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap();
    fs::rename(root.join("marker.txt"), root.join("marker-old.txt")).unwrap();
    fs::write(root.join("marker.txt"), "before\n").unwrap();
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("authorized target identity changed")
    );

    let root = base.0.join("replacement-repository");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("marker.txt"), "before\n").unwrap();
    fs::write(root.join("protected.txt"), "protected\n").unwrap();
    let tool = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap();
    let old = base.0.join("old-repository");
    fs::rename(&root, &old).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("marker.txt"), "before\n").unwrap();
    fs::write(root.join("protected.txt"), "protected\n").unwrap();
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("repository root identity changed")
    );
}

#[tokio::test]
async fn executable_substitution_is_rejected_before_mutation() {
    let base = TestDirectory::new("executable");
    let root = base.repository();
    let executable = copied_fixture(&base.0);
    let tool = RepositoryMutationFixtureTool::new(&executable, &root).unwrap();
    let mut replacement = fs::read(&executable).unwrap();
    replacement.push(0);
    fs::write(&executable, replacement).unwrap();
    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(output.is_error);
    assert_eq!(json_content(&output)["status"], "failed_known");
    assert_eq!(fs::read(root.join("marker.txt")).unwrap(), b"before\n");
}

#[tokio::test]
async fn timeout_before_mutation_is_known_not_to_have_changed_the_marker() {
    let base = TestDirectory::new("timeout-before");
    let root = base.repository();
    let tool = RepositoryMutationFixtureTool::new_with_test_mode(
        fixture(),
        &root,
        RepositoryMutationFixtureTestMode::DelayBeforeMutation(Duration::from_millis(250)),
        Duration::from_millis(120),
        None,
    )
    .unwrap();
    let output = run(&tool).await;
    assert!(output.is_error);
    assert_eq!(json_content(&output)["status"], "failed_known");
    assert_eq!(json_content(&output)["changed"], false);
    assert_eq!(json_content(&output)["uncertain"], false);
    assert_eq!(fs::read(root.join("marker.txt")).unwrap(), b"before\n");
}

#[tokio::test]
async fn timeout_after_mutation_reports_an_uncertain_side_effect_without_replay() {
    let base = TestDirectory::new("timeout-after");
    let root = base.repository();
    let tool = RepositoryMutationFixtureTool::new_with_test_mode(
        fixture(),
        &root,
        RepositoryMutationFixtureTestMode::DelayAfterMutation(Duration::from_secs(1)),
        Duration::from_millis(300),
        None,
    )
    .unwrap();
    let output = run(&tool).await;
    assert!(output.is_error);
    assert_eq!(json_content(&output)["status"], "uncertain");
    assert_eq!(json_content(&output)["changed"], true);
    assert_eq!(json_content(&output)["uncertain"], true);
    assert_eq!(fs::read(root.join("marker.txt")).unwrap(), b"after\n");
}

#[tokio::test]
async fn postconditions_detect_unauthorized_root_and_outside_effects() {
    for mode in [
        RepositoryMutationFixtureTestMode::MutateSecondFile,
        RepositoryMutationFixtureTestMode::CreateExtraFile,
        RepositoryMutationFixtureTestMode::DeleteProtectedFile,
    ] {
        let base = TestDirectory::new("adversarial");
        let root = base.repository();
        let tool = RepositoryMutationFixtureTool::new_with_test_mode(
            fixture(),
            &root,
            mode,
            Duration::from_secs(1),
            None,
        )
        .unwrap();
        let output = run(&tool).await;
        assert!(output.is_error);
        assert_eq!(json_content(&output)["status"], "policy_violation");
        assert_eq!(json_content(&output)["partial"], true);
    }

    let base = TestDirectory::new("outside");
    let root = base.repository();
    let outside = base.0.join("outside.txt");
    fs::write(&outside, "outside-before\n").unwrap();
    let tool = RepositoryMutationFixtureTool::new_with_test_mode(
        fixture(),
        &root,
        RepositoryMutationFixtureTestMode::MutateOutsideRoot,
        Duration::from_secs(1),
        Some(outside.clone()),
    )
    .unwrap();
    let output = run(&tool).await;
    assert_eq!(json_content(&output)["status"], "policy_violation");
    assert_eq!(fs::read(outside).unwrap(), b"outside-changed\n");
}

#[tokio::test]
async fn same_repository_calls_serialize_and_the_lease_releases_after_timeout() {
    let base = TestDirectory::new("lease");
    let root = base.repository();
    let first = std::sync::Arc::new(
        RepositoryMutationFixtureTool::new_with_test_mode(
            fixture(),
            &root,
            RepositoryMutationFixtureTestMode::DelayAfterMutation(Duration::from_millis(120)),
            Duration::from_secs(1),
            None,
        )
        .unwrap(),
    );
    let second = std::sync::Arc::new(RepositoryMutationFixtureTool::new(fixture(), &root).unwrap());
    let first_task = tokio::spawn({
        let first = first.clone();
        async move { run(&first).await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let second_task = tokio::spawn({
        let second = second.clone();
        async move {
            second
                .execute(ToolInput(json!({})), ToolContext::default())
                .await
        }
    });
    assert!(!first_task.await.unwrap().is_error);
    let error = second_task.await.unwrap().unwrap_err();
    assert!(error.to_string().contains("precondition changed"));

    let retry_root = base.0.join("retry-repository");
    fs::create_dir(&retry_root).unwrap();
    fs::write(retry_root.join("marker.txt"), "before\n").unwrap();
    fs::write(retry_root.join("protected.txt"), "protected\n").unwrap();
    let timeout = RepositoryMutationFixtureTool::new_with_test_mode(
        fixture(),
        &retry_root,
        RepositoryMutationFixtureTestMode::DelayBeforeMutation(Duration::from_millis(200)),
        Duration::from_millis(30),
        None,
    )
    .unwrap();
    let _ = run(&timeout).await;
    let follow_up = RepositoryMutationFixtureTool::new(fixture(), &retry_root).unwrap();
    assert!(
        !run(&follow_up).await.is_error,
        "timeout must release the lease"
    );
}

#[tokio::test]
async fn external_change_during_a_lease_is_detected_but_not_prevented() {
    let base = TestDirectory::new("external-change");
    let root = base.repository();
    let tool = std::sync::Arc::new(
        RepositoryMutationFixtureTool::new_with_test_mode(
            fixture(),
            &root,
            RepositoryMutationFixtureTestMode::DelayAfterMutation(Duration::from_millis(400)),
            Duration::from_secs(1),
            None,
        )
        .unwrap(),
    );
    let task = tokio::spawn({
        let tool = tool.clone();
        async move { run(&tool).await }
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while fs::read(root.join("marker.txt")).unwrap() != b"after\n" {
        assert!(
            std::time::Instant::now() < deadline,
            "fixture did not mutate marker"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    fs::write(root.join("protected.txt"), "external-change\n").unwrap();
    let output = task.await.unwrap();
    assert_eq!(json_content(&output)["status"], "policy_violation");
    assert_eq!(
        fs::read(root.join("protected.txt")).unwrap(),
        b"external-change\n"
    );
}

#[tokio::test]
async fn aborting_before_mutation_attempts_cleanup_and_releases_the_lease() {
    let base = TestDirectory::new("abort");
    let root = base.repository();
    let delayed = std::sync::Arc::new(
        RepositoryMutationFixtureTool::new_with_test_mode(
            fixture(),
            &root,
            RepositoryMutationFixtureTestMode::DelayBeforeMutation(Duration::from_secs(2)),
            Duration::from_secs(5),
            None,
        )
        .unwrap(),
    );
    let task = tokio::spawn({
        let delayed = delayed.clone();
        async move { run(&delayed).await }
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    task.abort();
    assert!(task.await.is_err());
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert_eq!(fs::read(root.join("marker.txt")).unwrap(), b"before\n");
    let follow_up = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap();
    assert!(
        !run(&follow_up).await.is_error,
        "abort must release the lease"
    );
}

#[cfg(unix)]
#[test]
fn symlink_target_is_rejected() {
    use std::os::unix::fs::symlink;

    let base = TestDirectory::new("symlink");
    let root = base.repository();
    fs::remove_file(root.join("marker.txt")).unwrap();
    symlink(root.join("protected.txt"), root.join("marker.txt")).unwrap();
    let error = RepositoryMutationFixtureTool::new(fixture(), &root).unwrap_err();
    assert!(error.to_string().contains("symbolic link"));
}
