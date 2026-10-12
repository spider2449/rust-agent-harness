//! The helper "programs" are ignored tests of this binary, run as children.

use std::time::Instant;

use rah_api::{
    Kernel, MountOptions,
    sandbox::{OutputLimits, Termination},
};

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-sandbox-process-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("sub")).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base_env() -> BTreeMap<OsString, OsString> {
    DEFAULT_INHERIT_ENV
        .iter()
        .filter_map(|name| Some((OsString::from(name), std::env::var_os(name)?)))
        .collect()
}

fn env(dir: &TempDir) -> ProcessEnv {
    ProcessEnv::new(dir.0.clone(), base_env(), Duration::from_secs(60))
}

fn helper(name: &str, timeout: Duration) -> ProgramSpec {
    let program = std::env::current_exe().unwrap().display().to_string();
    let args = [
        "--ignored",
        "--exact",
        "--nocapture",
        &format!("tests::{name}"),
    ]
    .map(str::to_owned)
    .to_vec();
    ProgramSpec::new(program, args, timeout)
}

fn stdout(output: &ExecOutput) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
#[ignore = "helper program"]
fn helper_report() {
    println!("cwd={}", std::env::current_dir().unwrap().display());
    println!("var={:?}", std::env::var("RAH_TEST_VAR").ok());
    println!(
        "cargo_env_present={}",
        std::env::var_os("CARGO_MANIFEST_DIR").is_some()
    );
    eprintln!("to-stderr");
    std::process::exit(3);
}

#[test]
#[ignore = "helper program"]
fn helper_sleep() {
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
#[ignore = "helper program"]
fn helper_flood() {
    use std::io::Write;
    let block = vec![b'x'; 64 * 1024];
    let mut out = std::io::stdout().lock();
    for _ in 0..64 {
        if out.write_all(&block).is_err() {
            return;
        }
    }
}

/// Starts a sleeping grandchild that inherits stdout, then exits at once.
#[test]
#[ignore = "helper program"]
#[expect(
    clippy::zombie_processes,
    reason = "the grandchild must outlive this helper; the sandbox under test kills it"
)]
fn helper_orphan() {
    let exe = std::env::current_exe().unwrap();
    std::process::Command::new(exe)
        .args(["--ignored", "--exact", "--nocapture", "tests::helper_sleep"])
        .spawn()
        .unwrap();
}

#[tokio::test]
async fn runs_a_program_with_a_cleared_environment_and_bounded_cwd() {
    let dir = TempDir::new("report");
    let mut spec = helper("helper_report", Duration::from_secs(30));
    spec.cwd = Some(PathBuf::from("sub"));
    spec.env.insert("RAH_TEST_VAR".into(), "42".into());
    let output = env(&dir).run(spec, CancellationToken::new()).await.unwrap();
    let text = stdout(&output);
    assert_eq!(output.exit_code, Some(3), "{text}");
    assert_eq!(output.termination, None);
    assert!(text.contains("var=Some(\"42\")"), "{text}");
    assert!(text.contains("cargo_env_present=false"), "{text}");
    let cwd = text
        .lines()
        .find_map(|line| line.strip_prefix("cwd="))
        .unwrap();
    assert_eq!(
        std::fs::canonicalize(cwd).unwrap(),
        dir.0.join("sub"),
        "{text}"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("to-stderr"));
}

#[tokio::test]
async fn invalid_specs_and_missing_programs_start_nothing() {
    let dir = TempDir::new("invalid");
    let env = env(&dir);
    let base = helper("helper_report", Duration::from_secs(5));
    let mut cases = Vec::new();
    for cwd in ["..", "sub/../..", "missing"] {
        let mut spec = base.clone();
        spec.cwd = Some(PathBuf::from(cwd));
        cases.push(spec);
    }
    let mut spec = base.clone();
    spec.cwd = Some(dir.0.clone());
    cases.push(spec);
    let mut spec = base.clone();
    spec.timeout = Duration::from_secs(61);
    cases.push(spec);
    let mut spec = base.clone();
    spec.program = String::new();
    cases.push(spec);
    let mut spec = base.clone();
    spec.env.insert("A=B".into(), "x".into());
    cases.push(spec);
    let mut spec = base.clone();
    spec.limits.stdout_bytes = 0;
    cases.push(spec);
    for spec in cases {
        let result = env.run(spec.clone(), CancellationToken::new()).await;
        assert!(
            matches!(result, Err(ExecError::InvalidSpec(_))),
            "{spec:?}: {result:?}"
        );
    }
    let spec = ProgramSpec::new("rah-no-such-program-x7", Vec::new(), Duration::from_secs(5));
    assert!(matches!(
        env.run(spec, CancellationToken::new()).await,
        Err(ExecError::Spawn { .. })
    ));
}

#[tokio::test]
async fn timeouts_cancellation_and_output_limits_terminate_the_program() {
    let dir = TempDir::new("terminate");
    let env = env(&dir);

    let started = Instant::now();
    let output = env
        .run(
            helper("helper_sleep", Duration::from_millis(300)),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(output.termination, Some(Termination::TimedOut));
    assert_eq!(output.exit_code, None);
    assert!(started.elapsed() < Duration::from_secs(10));

    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let (output, ()) = tokio::join!(
        env.run(helper("helper_sleep", Duration::from_secs(30)), cancel),
        async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            trigger.cancel();
        }
    );
    assert_eq!(output.unwrap().termination, Some(Termination::Cancelled));

    let mut spec = helper("helper_flood", Duration::from_secs(30));
    spec.limits = OutputLimits {
        stdout_bytes: 1000,
        stderr_bytes: 1000,
    };
    let output = env.run(spec, CancellationToken::new()).await.unwrap();
    assert_eq!(output.termination, Some(Termination::OutputLimit));
    assert_eq!(output.stdout.len(), 1000);
}

#[tokio::test]
async fn descendants_holding_the_pipes_are_terminated_after_exit() {
    let dir = TempDir::new("orphan");
    let started = Instant::now();
    let output = env(&dir)
        .run(
            helper("helper_orphan", Duration::from_secs(60)),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(output.termination, None);
    assert_eq!(output.exit_code, Some(0));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn unmounting_the_plugin_cancels_running_programs() {
    let dir = TempDir::new("unmount");
    let kernel = Kernel::new();
    let config = serde_json::json!({ "root": dir.0, "max_timeout_ms": 60_000 });
    kernel
        .mount(
            Arc::new(SandboxProcessPlugin::new("sandbox")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    let sandbox = kernel.handle().get(SANDBOX).unwrap();
    let handle = kernel.handle();
    let (output, ()) = tokio::join!(
        sandbox.run(
            helper("helper_sleep", Duration::from_secs(30)),
            CancellationToken::new()
        ),
        async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            handle.unmount(&PluginId::new("sandbox")).await.unwrap();
        }
    );
    assert_eq!(output.unwrap().termination, Some(Termination::Cancelled));
    assert!(kernel.handle().get(SANDBOX).is_none());
}

#[tokio::test]
async fn invalid_configuration_fails_the_plugin() {
    let kernel = Kernel::new();
    for config in [
        serde_json::json!({ "inherit_env": ["A=B"] }),
        serde_json::json!({ "max_timeout_ms": 0 }),
        serde_json::json!({ "root": "/rah/does/not/exist" }),
        serde_json::json!({ "unknown": true }),
    ] {
        let status = kernel
            .mount(
                Arc::new(SandboxProcessPlugin::new("sandbox")),
                MountOptions::with_config(config.clone()),
            )
            .await
            .unwrap();
        assert!(
            matches!(status, rah_api::PluginStatus::Failed { .. }),
            "{config}: {status:?}"
        );
        kernel.unmount(&PluginId::new("sandbox")).await.unwrap();
    }
}
