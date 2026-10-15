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
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    println!("home={:?}", std::env::var_os(home));
    println!("program_data={:?}", std::env::var_os("ProgramData"));
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
    // The home directory reaches programs, so they find the user's own
    // configuration (Git's `safe.directory` among it).
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Some(value) = std::env::var_os(home) {
        assert!(text.contains(&format!("home={:?}", Some(value))), "{text}");
    }
    // rustc finds the MSVC linker through `%ProgramData%`.
    if cfg!(windows)
        && let Some(value) = std::env::var_os("ProgramData")
    {
        assert!(
            text.contains(&format!("program_data={:?}", Some(value))),
            "{text}"
        );
    }
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
async fn inherit_env_extra_adds_to_the_default_list() {
    let dir = TempDir::new("extra-env");
    let kernel = Kernel::new();
    // Cargo sets `CARGO_MANIFEST_DIR` for this test binary; it is not in the
    // default list, so only `inherit_env_extra` can pass it on.
    let config = serde_json::json!({
        "root": dir.0,
        "inherit_env_extra": ["CARGO_MANIFEST_DIR"],
    });
    kernel
        .mount(
            Arc::new(SandboxProcessPlugin::new("sandbox")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    let sandbox = kernel.handle().get(SANDBOX).unwrap();
    let output = sandbox
        .run(
            helper("helper_report", Duration::from_secs(30)),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let text = stdout(&output);
    assert!(text.contains("cargo_env_present=true"), "{text}");
    // The defaults still apply.
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Some(value) = std::env::var_os(home) {
        assert!(text.contains(&format!("home={:?}", Some(value))), "{text}");
    }
}

#[tokio::test]
async fn invalid_configuration_fails_the_plugin() {
    let kernel = Kernel::new();
    for config in [
        serde_json::json!({ "inherit_env": ["A=B"] }),
        serde_json::json!({ "inherit_env_extra": ["A=B"] }),
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

/// Echoes stdin lines as `echo:<line>` until end of input.
#[test]
#[ignore = "helper program"]
fn helper_echo() {
    use std::io::{BufRead, Write};
    eprintln!("echo-ready");
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { return };
        writeln!(out, "echo:{line}").unwrap();
        out.flush().unwrap();
    }
}

fn spawn_spec(helper_name: &str) -> SpawnSpec {
    let spec = helper(helper_name, Duration::from_secs(1));
    SpawnSpec::new(spec.program, spec.args)
}

async fn mounted(dir: &TempDir) -> (Kernel, Arc<dyn ExecEnv>) {
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(SandboxProcessPlugin::new("sandbox")),
            MountOptions::with_config(serde_json::json!({ "root": dir.0 })),
        )
        .await
        .unwrap();
    let sandbox = kernel.handle().get(SANDBOX).unwrap();
    (kernel, sandbox)
}

/// Reads stdout until a line starting with `prefix`, or end of output.
async fn read_line_starting(
    stdout: &mut (dyn tokio::io::AsyncRead + Send + Unpin),
    prefix: &str,
) -> Option<String> {
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await.ok()? {
        if line.starts_with(prefix) {
            return Some(line);
        }
    }
    None
}

#[tokio::test]
async fn spawned_programs_talk_over_pipes_and_report_their_exit() {
    use tokio::io::AsyncWriteExt;
    let dir = TempDir::new("spawn-echo");
    let (_kernel, sandbox) = mounted(&dir).await;
    let mut spawned = sandbox.spawn(spawn_spec("helper_echo")).await.unwrap();
    spawned.stdin.write_all(b"hello\n").await.unwrap();
    spawned.stdin.flush().await.unwrap();
    let line = read_line_starting(spawned.stdout.as_mut(), "echo:").await;
    assert_eq!(line.as_deref(), Some("echo:hello"));
    drop(spawned.stdin);
    let code = tokio::time::timeout(Duration::from_secs(20), spawned.process.wait())
        .await
        .unwrap();
    assert_eq!(code, Some(0));
    assert!(spawned.process.stderr().contains("echo-ready"));
}

#[tokio::test]
async fn spawned_programs_end_on_terminate_drop_and_unmount() {
    let dir = TempDir::new("spawn-stop");
    let (kernel, sandbox) = mounted(&dir).await;

    let spawned = sandbox.spawn(spawn_spec("helper_sleep")).await.unwrap();
    spawned.process.terminate();
    let code = tokio::time::timeout(Duration::from_secs(10), spawned.process.wait())
        .await
        .unwrap();
    assert_eq!(code, None);

    // Dropping the handle ends the program: its stdout reaches end of input.
    let mut spawned = sandbox.spawn(spawn_spec("helper_sleep")).await.unwrap();
    drop(spawned.process);
    let eof = tokio::time::timeout(
        Duration::from_secs(10),
        read_line_starting(spawned.stdout.as_mut(), "never"),
    )
    .await
    .unwrap();
    assert_eq!(eof, None);

    let spawned = sandbox.spawn(spawn_spec("helper_sleep")).await.unwrap();
    kernel.unmount(&PluginId::new("sandbox")).await.unwrap();
    let code = tokio::time::timeout(Duration::from_secs(10), spawned.process.wait())
        .await
        .unwrap();
    assert_eq!(code, None);
    assert!(sandbox.spawn(spawn_spec("helper_echo")).await.is_err());
}

#[tokio::test]
async fn an_environment_without_a_scope_refuses_to_spawn() {
    let dir = TempDir::new("spawn-unowned");
    assert!(matches!(
        env(&dir).spawn(spawn_spec("helper_echo")).await,
        Err(ExecError::InvalidSpec(_))
    ));
}

#[cfg(not(any(target_os = "linux", windows)))]
#[tokio::test]
async fn sandbox_os_refuses_to_start_on_this_platform() {
    let dir = TempDir::new("os-unsupported");
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(SandboxOsPlugin::new("sandbox")),
            MountOptions::with_config(serde_json::json!({ "root": dir.0 })),
        )
        .await
        .unwrap();
    assert!(
        matches!(&status, rah_api::PluginStatus::Failed { .. }),
        "{status:?}"
    );
    assert!(kernel.handle().get(SANDBOX).is_none());
}

/// Tries what a confined program must not do, and what it may, and reports
/// each outcome as `<name>=ok`, `<name>=denied` or `<name>=error:<message>`.
#[test]
#[ignore = "helper program"]
fn helper_probe() {
    use std::io::Write;
    fn report(name: &str, result: std::io::Result<()>) {
        match result {
            Ok(()) => println!("{name}=ok"),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                println!("{name}=denied");
            }
            Err(error) => println!("{name}=error:{error}"),
        }
    }
    let var = |name: &str| std::env::var(name).unwrap();
    let tmp = std::env::temp_dir();
    println!("tmpdir={}", tmp.display());
    report("inside", std::fs::write("inside.txt", b"x"));
    report("tmp", std::fs::write(tmp.join("tmp.txt"), b"x"));
    report(
        "outside",
        std::fs::write(PathBuf::from(var("RAH_TEST_OUTSIDE")).join("out.txt"), b"x"),
    );
    report("read", std::fs::read(var("RAH_TEST_READ")).map(|_| ()));
    report(
        "tcp",
        std::net::TcpStream::connect(var("RAH_TEST_ADDR"))
            .and_then(|mut stream| stream.write_all(b"x")),
    );
    report("udp", std::net::UdpSocket::bind("127.0.0.1:0").map(|_| ()));
}

/// Prints each argument after `--` on its own line, as `arg=<argument>`.
#[test]
#[ignore = "helper program"]
fn helper_args() {
    let args: Vec<String> = std::env::args().collect();
    let start = args
        .iter()
        .position(|arg| arg == "--")
        .map_or(args.len(), |at| at + 1);
    for arg in &args[start..] {
        println!("arg={arg}");
    }
}

#[cfg(any(target_os = "linux", windows))]
mod os {
    use super::*;

    /// Whether this host can run `sandbox-os`; says why when it cannot.
    #[cfg(target_os = "linux")]
    fn supported() -> bool {
        match crate::os::linux::landlock_abi() {
            Ok(abi) if abi >= 3 => true,
            other => {
                eprintln!("skipped: sandbox-os needs landlock ABI 3 or later ({other:?})");
                false
            }
        }
    }

    #[cfg(windows)]
    fn supported() -> bool {
        true
    }

    /// The configuration every test starts from: Windows cannot deny the
    /// network, so it must be allowed there.
    fn base(dir: &TempDir) -> serde_json::Value {
        let mut config = serde_json::json!({ "root": dir.0, "max_timeout_ms": 60_000 });
        if cfg!(windows) {
            config["network"] = true.into();
        }
        config
    }

    async fn mount(config: serde_json::Value) -> (Kernel, rah_api::PluginStatus) {
        let kernel = Kernel::new();
        let status = kernel
            .mount(
                Arc::new(SandboxOsPlugin::new("sandbox")),
                MountOptions::with_config(config),
            )
            .await
            .unwrap();
        (kernel, status)
    }

    async fn mounted(config: serde_json::Value) -> (Kernel, Arc<dyn ExecEnv>) {
        let (kernel, status) = mount(config).await;
        assert_eq!(status, rah_api::PluginStatus::Active);
        let sandbox = kernel.handle().get(SANDBOX).unwrap();
        (kernel, sandbox)
    }

    struct Probe {
        text: String,
        outside: TempDir,
    }

    impl Probe {
        fn line(&self, name: &str) -> &str {
            let prefix = format!("{name}=");
            self.text
                .lines()
                .find_map(|line| line.strip_prefix(prefix.as_str()))
                .unwrap_or_else(|| panic!("no `{name}` in {}", self.text))
        }
    }

    /// Runs `helper_probe` in a sandbox rooted at `dir`. `config` is merged
    /// into the base configuration; the value `"<outside>"` stands for a
    /// list holding the directory outside the root.
    async fn probe(dir: &TempDir, name: &str, config: serde_json::Value) -> (Kernel, Probe) {
        let outside = TempDir::new(&format!("{name}-outside"));
        std::fs::write(outside.0.join("secret.txt"), b"readable").unwrap();
        let mut full = base(dir);
        for (key, value) in config.as_object().unwrap() {
            full[key] = match value.as_str() {
                Some("<outside>") => serde_json::json!([outside.0]),
                _ => value.clone(),
            };
        }
        let (kernel, sandbox) = mounted(full).await;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut spec = helper("helper_probe", Duration::from_secs(30));
        let outside_path = outside.0.display().to_string();
        let secret = outside.0.join("secret.txt").display().to_string();
        let address = listener.local_addr().unwrap().to_string();
        spec.env.insert("RAH_TEST_OUTSIDE".into(), outside_path);
        spec.env.insert("RAH_TEST_READ".into(), secret);
        spec.env.insert("RAH_TEST_ADDR".into(), address);
        let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
        let text = format!(
            "{}{}",
            stdout(&output),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.termination, None, "{text}");
        (kernel, Probe { text, outside })
    }

    /// The private temporary directory is under the system one and goes
    /// away with the plugin.
    async fn assert_private_temp(kernel: Kernel, probe: &Probe) {
        let tmpdir = PathBuf::from(probe.line("tmpdir"));
        let system = std::fs::canonicalize(std::env::temp_dir()).unwrap();
        let canonical = std::fs::canonicalize(&tmpdir).unwrap();
        assert!(canonical.starts_with(system), "{}", probe.text);
        kernel.unmount(&PluginId::new("sandbox")).await.unwrap();
        assert!(!tmpdir.exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn confines_writes_and_the_network() {
        if !supported() {
            return;
        }
        let dir = TempDir::new("os-confined");
        let (kernel, probe) = probe(&dir, "os-confined", serde_json::json!({})).await;
        assert_eq!(probe.line("inside"), "ok");
        assert_eq!(probe.line("tmp"), "ok");
        assert_eq!(probe.line("read"), "ok");
        assert_eq!(probe.line("outside"), "denied");
        assert_eq!(probe.line("tcp"), "denied");
        assert_eq!(probe.line("udp"), "denied");
        assert!(dir.0.join("inside.txt").exists());
        assert!(!probe.outside.0.join("out.txt").exists());
        assert_private_temp(kernel, &probe).await;
    }

    /// Writes are confined to the labeled directories; reads and the
    /// network are not restricted on Windows.
    #[cfg(windows)]
    #[tokio::test]
    async fn confines_writes_at_low_integrity() {
        let dir = TempDir::new("os-confined");
        let (kernel, probe) = probe(&dir, "os-confined", serde_json::json!({})).await;
        assert_eq!(probe.line("inside"), "ok");
        assert_eq!(probe.line("tmp"), "ok");
        assert_eq!(probe.line("read"), "ok");
        assert_eq!(probe.line("outside"), "denied");
        assert_eq!(probe.line("tcp"), "ok");
        assert_eq!(probe.line("udp"), "ok");
        assert!(dir.0.join("inside.txt").exists());
        assert!(!probe.outside.0.join("out.txt").exists());
        assert_private_temp(kernel, &probe).await;
    }

    #[tokio::test]
    async fn writable_paths_and_the_network_can_be_allowed() {
        if !supported() {
            return;
        }
        let dir = TempDir::new("os-allowed");
        let config = serde_json::json!({ "writable": "<outside>", "network": true });
        let (_kernel, probe) = probe(&dir, "os-allowed", config).await;
        assert_eq!(probe.line("outside"), "ok");
        assert_eq!(probe.line("tcp"), "ok");
        assert_eq!(probe.line("udp"), "ok");
        assert!(probe.outside.0.join("out.txt").exists());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn readable_paths_can_be_narrowed() {
        if !supported() {
            return;
        }
        let dir = TempDir::new("os-narrow");
        // The helper is this test binary, so its directory stays readable,
        // with the system directories its shared libraries come from.
        let exe = std::env::current_exe().unwrap();
        let mut readable = vec![exe.parent().unwrap().to_path_buf()];
        for system in ["/usr", "/etc", "/lib", "/lib64"] {
            if Path::new(system).exists() {
                readable.push(PathBuf::from(system));
            }
        }
        let config = serde_json::json!({ "readable": readable });
        let (_kernel, probe) = probe(&dir, "os-narrow", config).await;
        assert_eq!(probe.line("read"), "denied");
        assert_eq!(probe.line("inside"), "ok");
    }

    #[tokio::test]
    async fn confined_programs_are_still_terminated() {
        if !supported() {
            return;
        }
        let dir = TempDir::new("os-terminate");
        let (kernel, sandbox) = mounted(base(&dir)).await;

        let output = sandbox
            .run(
                helper("helper_sleep", Duration::from_millis(300)),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(output.termination, Some(Termination::TimedOut));

        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        let (output, ()) = tokio::join!(
            sandbox.run(helper("helper_sleep", Duration::from_secs(30)), cancel),
            async {
                tokio::time::sleep(Duration::from_millis(300)).await;
                trigger.cancel();
            }
        );
        assert_eq!(output.unwrap().termination, Some(Termination::Cancelled));

        // Descendants holding the pipes are terminated after the exit.
        let started = Instant::now();
        let output = sandbox
            .run(
                helper("helper_orphan", Duration::from_secs(60)),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert!(started.elapsed() < Duration::from_secs(20));

        let spawned = sandbox.spawn(spawn_spec("helper_sleep")).await.unwrap();
        kernel.unmount(&PluginId::new("sandbox")).await.unwrap();
        let code = tokio::time::timeout(Duration::from_secs(10), spawned.process.wait())
            .await
            .unwrap();
        assert_eq!(code, None);
    }

    #[tokio::test]
    async fn invalid_configuration_fails_the_plugin() {
        if !supported() {
            return;
        }
        let dir = TempDir::new("os-invalid");
        let mut invalid = vec![
            serde_json::json!({ "writable": ["/rah/does/not/exist"] }),
            serde_json::json!({ "network": "yes" }),
            serde_json::json!({ "unknown": true }),
        ];
        if cfg!(target_os = "linux") {
            invalid.push(serde_json::json!({ "readable": ["missing"] }));
        } else {
            // Reads and the network cannot be restricted on Windows, and the
            // configuration may not pretend otherwise.
            invalid.push(serde_json::json!({ "readable": ["."] }));
            invalid.push(serde_json::json!({ "network": false }));
            // A Low label there would let every Low program write there.
            let drive = dir.0.ancestors().last().unwrap().to_path_buf();
            invalid.push(serde_json::json!({ "writable": [drive] }));
            let profile = std::env::var_os("USERPROFILE").unwrap();
            invalid.push(serde_json::json!({ "writable": [profile] }));
            let system = std::env::var_os("SystemRoot").unwrap();
            invalid.push(serde_json::json!({ "root": system }));
        }
        for extra in invalid {
            let mut config = base(&dir);
            for (key, value) in extra.as_object().unwrap() {
                config[key] = value.clone();
            }
            let (kernel, status) = mount(config.clone()).await;
            assert!(
                matches!(status, rah_api::PluginStatus::Failed { .. }),
                "{config}: {status:?}"
            );
            assert!(kernel.handle().get(SANDBOX).is_none());
        }
    }

    #[cfg(windows)]
    mod windows {
        use std::os::windows::ffi::OsStrExt;

        use tokio::io::AsyncWriteExt;

        use super::*;

        #[tokio::test]
        async fn the_network_must_be_acknowledged() {
            let dir = TempDir::new("os-network");
            let (_kernel, status) = mount(serde_json::json!({ "root": dir.0 })).await;
            let rah_api::PluginStatus::Failed { message: error } = status else {
                panic!("{status:?}");
            };
            assert!(error.contains("network = true"), "{error}");
        }

        #[tokio::test]
        async fn the_root_keeps_its_low_label() {
            let dir = TempDir::new("os-label");
            let old = dir.0.join("sub").join("old.txt");
            std::fs::write(&old, b"x").unwrap();
            let (kernel, sandbox) = mounted(base(&dir)).await;
            // The label reached the files that existed before the mount.
            let args = ["/c", r"echo new> sub\old.txt"].map(str::to_owned).to_vec();
            let spec = ProgramSpec::new("cmd".to_owned(), args, Duration::from_secs(30));
            let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
            assert_eq!(output.exit_code, Some(0), "{output:?}");
            assert!(std::fs::read_to_string(&old).unwrap().starts_with("new"));
            kernel.unmount(&PluginId::new("sandbox")).await.unwrap();
            for path in [dir.0.clone(), dir.0.join("sub")] {
                let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
                let labeled = crate::os::windows::has_low_label(&name).unwrap();
                assert!(labeled, "{}", path.display());
            }
        }

        /// Labeling needs `WRITE_OWNER`; a user holding only Modify (a
        /// standard user on a non-system drive) is granted it first.
        #[tokio::test]
        async fn a_root_the_user_may_only_modify_is_labeled() {
            let dir = TempDir::new("os-modify");
            let old = dir.0.join("sub").join("old.txt");
            std::fs::write(&old, b"x").unwrap();
            let whoami = std::process::Command::new("whoami")
                .args(["/user", "/fo", "csv", "/nh"])
                .output()
                .unwrap();
            let user = String::from_utf8(whoami.stdout).unwrap();
            let sid = user.trim().rsplit(',').next().unwrap().trim_matches('"');
            let modify = format!("*{sid}:(OI)(CI)M");
            let icacls = std::process::Command::new("icacls")
                .arg(&dir.0)
                .args(["/inheritance:r", "/grant:r", &modify, "*S-1-5-18:(OI)(CI)F"])
                .output()
                .unwrap();
            assert!(icacls.status.success(), "{icacls:?}");

            let (_kernel, sandbox) = mounted(base(&dir)).await;
            let args = ["/c", r"echo new> sub\old.txt"].map(str::to_owned).to_vec();
            let spec = ProgramSpec::new("cmd".to_owned(), args, Duration::from_secs(30));
            let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
            assert_eq!(output.exit_code, Some(0), "{output:?}");
            assert!(std::fs::read_to_string(&old).unwrap().starts_with("new"));
        }

        #[tokio::test]
        async fn spawned_programs_talk_over_pipes() {
            let dir = TempDir::new("os-spawn");
            let (_kernel, sandbox) = mounted(base(&dir)).await;
            let mut spawned = sandbox.spawn(spawn_spec("helper_echo")).await.unwrap();
            spawned.stdin.write_all(b"hello\n").await.unwrap();
            let line = read_line_starting(spawned.stdout.as_mut(), "echo:").await;
            assert_eq!(line.as_deref(), Some("echo:hello"));
            drop(spawned.stdin);
            let code = tokio::time::timeout(Duration::from_secs(10), spawned.process.wait())
                .await
                .unwrap();
            assert_eq!(code, Some(0));
            assert!(spawned.process.stderr().contains("echo-ready"));
        }

        /// The program and the console programs it starts share the pseudo
        /// console RAH creates. A console created with the program's token
        /// fails to start for an elevated user (Administrators deny-only),
        /// which every test here then shows as `STATUS_DLL_INIT_FAILED`.
        #[tokio::test]
        async fn descendants_share_the_console() {
            let dir = TempDir::new("os-console");
            let (_kernel, sandbox) = mounted(base(&dir)).await;
            // `CON` is the console itself; what reaches it is discarded.
            let line = r"cmd.exe /c echo nested& echo to the console> CON& exit 7";
            let args = ["/c", line].map(str::to_owned).to_vec();
            let spec = ProgramSpec::new("cmd".to_owned(), args, Duration::from_secs(30));
            let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
            assert_eq!(output.exit_code, Some(7), "{output:?}");
            let text = stdout(&output);
            assert!(text.contains("nested"), "{text}");
            assert!(!text.contains("to the console"), "{text}");
        }

        #[tokio::test]
        async fn arguments_reach_the_program_unchanged() {
            let dir = TempDir::new("os-args");
            let (_kernel, sandbox) = mounted(base(&dir)).await;
            let tricky = [
                "plain",
                "with space",
                "",
                r#"quote " inside"#,
                r"trailing\",
                r"back\\slash and space\",
                "\ttab",
            ];
            let mut spec = helper("helper_args", Duration::from_secs(30));
            spec.args.push("--".into());
            spec.args.extend(tricky.iter().map(|arg| (*arg).to_owned()));
            let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
            let text = stdout(&output);
            let args: Vec<&str> = text
                .lines()
                .filter_map(|line| line.strip_prefix("arg="))
                .collect();
            assert_eq!(args, tricky, "{text}");
        }

        #[tokio::test]
        async fn programs_are_found_on_the_path_and_batch_files_refused() {
            let dir = TempDir::new("os-lookup");
            std::fs::write(dir.0.join("run.cmd"), b"@echo hi\r\n").unwrap();
            let (_kernel, sandbox) = mounted(base(&dir)).await;
            let args = ["/c", "echo", "found"].map(str::to_owned).to_vec();
            let spec = ProgramSpec::new("cmd".to_owned(), args, Duration::from_secs(30));
            let output = sandbox.run(spec, CancellationToken::new()).await.unwrap();
            assert_eq!(output.exit_code, Some(0));
            assert!(stdout(&output).contains("found"), "{output:?}");

            for program in ["run.cmd", "./run.cmd", "missing-program"] {
                let spec = ProgramSpec::new(program.to_owned(), Vec::new(), Duration::from_secs(5));
                let result = sandbox.run(spec, CancellationToken::new()).await;
                assert!(
                    matches!(result, Err(ExecError::Spawn { .. })),
                    "{program}: {result:?}"
                );
            }
        }
    }
}
