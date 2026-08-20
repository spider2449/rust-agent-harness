use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
};

#[cfg(windows)]
use std::io;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    task::JoinHandle,
};

use crate::{CodexAdapterError, SUPPORTED_CODEX_VERSION, transport::AppServerTransport};

const STDERR_LIMIT: usize = 64 * 1024;
const CONTRACT_JSON: &str = include_str!("../fixtures/schema_contract_0_148_0.json");
static SCHEMA_PROBE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Deserialize)]
struct SchemaContract {
    codex_version: String,
    required_files: Vec<SchemaFile>,
}

#[derive(Deserialize)]
struct SchemaFile {
    path: String,
    required: Vec<String>,
}

pub(crate) struct ProcessTransport {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr: Arc<Mutex<String>>,
    stderr_task: Option<JoinHandle<()>>,
}

impl ProcessTransport {
    pub(crate) async fn start(executable: &Path) -> Result<Self, CodexAdapterError> {
        let executable = resolve_executable(executable)?;
        verify_version(&executable).await?;
        verify_schema(&executable).await?;
        let mut child = Command::new(&executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| CodexAdapterError::ProcessStartup {
                path: executable.clone(),
                source,
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| startup_pipe(&executable, "stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| startup_pipe(&executable, "stdout"))?;
        let stderr_pipe = child
            .stderr
            .take()
            .ok_or_else(|| startup_pipe(&executable, "stderr"))?;
        let stderr = Arc::new(Mutex::new(String::new()));
        let captured = Arc::clone(&stderr);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr_pipe).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut output = captured
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                output.push_str(&line);
                output.push('\n');
                if output.len() > STDERR_LIMIT {
                    let remove = output.len() - STDERR_LIMIT;
                    output.drain(..remove);
                }
            }
        });
        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout).lines(),
            stderr,
            stderr_task: Some(stderr_task),
        })
    }
}

fn resolve_executable(executable: &Path) -> Result<PathBuf, CodexAdapterError> {
    #[cfg(windows)]
    {
        resolve_windows_executable(executable, std::env::var_os("PATH").as_deref())
    }
    #[cfg(not(windows))]
    {
        Ok(executable.to_path_buf())
    }
}

#[cfg(windows)]
fn resolve_windows_executable(
    executable: &Path,
    path: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, CodexAdapterError> {
    let has_directory = executable.is_absolute()
        || executable
            .parent()
            .is_some_and(|parent| !parent.as_os_str().is_empty());
    if has_directory {
        return resolve_windows_candidate(executable)
            .ok_or_else(|| executable_not_found(executable));
    }

    let directories = path.into_iter().flat_map(std::env::split_paths);
    for directory in directories {
        if executable.extension().is_some() {
            if let Some(resolved) = resolve_windows_candidate(&directory.join(executable)) {
                return Ok(resolved);
            }
            continue;
        }
        for extension in ["exe", "cmd", "ps1"] {
            if let Some(resolved) =
                resolve_windows_candidate(&directory.join(executable).with_extension(extension))
            {
                return Ok(resolved);
            }
        }
    }

    Err(executable_not_found(executable))
}

#[cfg(windows)]
fn resolve_windows_candidate(candidate: &Path) -> Option<PathBuf> {
    if !candidate.is_file() {
        return None;
    }
    match candidate
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("exe") => canonical_file(candidate),
        Some("cmd" | "ps1") if candidate.file_stem() == Some(std::ffi::OsStr::new("codex")) => {
            resolve_npm_codex_binary(candidate)
        }
        _ => None,
    }
}

#[cfg(windows)]
fn resolve_npm_codex_binary(launcher: &Path) -> Option<PathBuf> {
    let launcher_directory = launcher.parent()?;
    let mut node_modules_roots = vec![launcher_directory.join("node_modules")];
    if launcher_directory.file_name() == Some(std::ffi::OsStr::new(".bin"))
        && let Some(node_modules) = launcher_directory.parent()
    {
        node_modules_roots.push(node_modules.to_path_buf());
    }
    let (platform_package, target) = match std::env::consts::ARCH {
        "x86_64" => ("codex-win32-x64", "x86_64-pc-windows-msvc"),
        "aarch64" => ("codex-win32-arm64", "aarch64-pc-windows-msvc"),
        _ => return None,
    };

    for node_modules in node_modules_roots {
        let package_root = node_modules.join("@openai").join("codex");
        let mut package_roots = vec![package_root.clone()];
        if let Ok(canonical) = package_root.canonicalize()
            && canonical != package_root
        {
            package_roots.push(canonical);
        }
        for root in package_roots {
            let nested_package = root
                .join("node_modules")
                .join("@openai")
                .join(platform_package);
            if let Some(binary) = codex_binary_in_package(&nested_package, target) {
                return Some(binary);
            }
            if let Some(binary) = canonical_file(
                &root
                    .join("vendor")
                    .join(target)
                    .join("bin")
                    .join("codex.exe"),
            ) {
                return Some(binary);
            }
        }
        let hoisted_package = node_modules.join("@openai").join(platform_package);
        if let Some(binary) = codex_binary_in_package(&hoisted_package, target) {
            return Some(binary);
        }
    }
    None
}

#[cfg(windows)]
fn codex_binary_in_package(package: &Path, target: &str) -> Option<PathBuf> {
    canonical_file(
        &package
            .join("vendor")
            .join(target)
            .join("bin")
            .join("codex.exe"),
    )
}

#[cfg(windows)]
fn canonical_file(path: &Path) -> Option<PathBuf> {
    path.is_file().then(|| path.canonicalize().ok()).flatten()
}

#[cfg(windows)]
fn executable_not_found(executable: &Path) -> CodexAdapterError {
    CodexAdapterError::ExecutableDiscovery {
        path: executable.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::NotFound,
            "no directly executable Codex binary was found",
        ),
    }
}

#[async_trait]
impl AppServerTransport for ProcessTransport {
    async fn send(&mut self, message: Value) -> Result<(), CodexAdapterError> {
        let bytes =
            serde_json::to_vec(&message).map_err(|error| CodexAdapterError::MalformedFraming {
                message: error.to_string(),
            })?;
        self.stdin.write_all(&bytes).await.map_err(transport)?;
        self.stdin.write_all(b"\n").await.map_err(transport)?;
        self.stdin.flush().await.map_err(transport)
    }

    async fn receive(&mut self) -> Result<Value, CodexAdapterError> {
        match self.stdout.next_line().await.map_err(transport)? {
            Some(line) => {
                serde_json::from_str(&line).map_err(|error| CodexAdapterError::MalformedFraming {
                    message: error.to_string(),
                })
            }
            None => {
                let status = self.child.wait().await.map_err(transport)?;
                if let Some(stderr_task) = self.stderr_task.take() {
                    let _ = stderr_task.await;
                }
                let stderr = self
                    .stderr
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                Err(CodexAdapterError::ProcessExited { status, stderr })
            }
        }
    }

    async fn shutdown(&mut self) -> Result<(), CodexAdapterError> {
        if self.child.try_wait().map_err(transport)?.is_none() {
            self.child.kill().await.map_err(transport)?;
        }
        if let Some(stderr_task) = self.stderr_task.take() {
            stderr_task.abort();
        }
        Ok(())
    }
}

async fn verify_version(executable: &Path) -> Result<(), CodexAdapterError> {
    let output = Command::new(executable)
        .arg("--version")
        .output()
        .await
        .map_err(|source| CodexAdapterError::ExecutableDiscovery {
            path: executable.to_path_buf(),
            source,
        })?;
    let actual = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    check_version(output.status.success(), actual)
}

pub(crate) fn check_version(success: bool, actual: String) -> Result<(), CodexAdapterError> {
    if success && actual == SUPPORTED_CODEX_VERSION {
        Ok(())
    } else {
        Err(CodexAdapterError::VersionMismatch {
            expected: SUPPORTED_CODEX_VERSION,
            actual,
        })
    }
}

pub(crate) fn validate_captured_contract() -> Result<(), CodexAdapterError> {
    let contract: SchemaContract = serde_json::from_str(CONTRACT_JSON).map_err(schema_error)?;
    if contract.codex_version != SUPPORTED_CODEX_VERSION {
        return Err(schema_error(
            "captured contract version does not match adapter pin",
        ));
    }
    if contract.required_files.is_empty()
        || contract
            .required_files
            .iter()
            .any(|file| file.path.is_empty() || file.required.is_empty())
    {
        return Err(schema_error("captured schema contract is incomplete"));
    }
    Ok(())
}

async fn verify_schema(executable: &Path) -> Result<(), CodexAdapterError> {
    validate_captured_contract()?;
    let contract: SchemaContract = serde_json::from_str(CONTRACT_JSON).map_err(schema_error)?;
    let directory = schema_temp_dir();
    std::fs::create_dir(&directory).map_err(schema_error)?;
    let output = Command::new(executable)
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(&directory)
        .output()
        .await
        .map_err(schema_error)?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&directory);
        return Err(schema_error(String::from_utf8_lossy(&output.stderr)));
    }
    let result = verify_schema_files(&directory, &contract);
    let _ = std::fs::remove_dir_all(&directory);
    result
}

fn verify_schema_files(root: &Path, contract: &SchemaContract) -> Result<(), CodexAdapterError> {
    let mut missing = Vec::new();
    for file in &contract.required_files {
        let path = root.join(&file.path);
        let value: Value = std::fs::read_to_string(&path)
            .map_err(schema_error)
            .and_then(|text| serde_json::from_str(&text).map_err(schema_error))?;
        collect_missing_fields(file, &value, &mut missing);
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CodexAdapterError::SchemaMismatch {
            missing: missing.join(", "),
        })
    }
}

fn collect_missing_fields(file: &SchemaFile, value: &Value, missing: &mut Vec<String>) {
    let required = value.get("required").and_then(Value::as_array);
    for field in &file.required {
        let present = required.is_some_and(|items| items.iter().any(|item| item == field));
        if !present {
            missing.push(format!("{} required field `{field}`", file.path));
        }
    }
}

fn schema_temp_dir() -> PathBuf {
    let probe_id = SCHEMA_PROBE_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "rah-codex-schema-{}-{probe_id}",
        std::process::id()
    ))
}

fn startup_pipe(executable: &Path, pipe: &str) -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: format!(
            "app-server did not expose piped {pipe} for `{}`",
            executable.display()
        ),
    }
}

fn schema_error(error: impl ToString) -> CodexAdapterError {
    CodexAdapterError::SchemaInspection {
        message: error.to_string(),
    }
}

fn transport(source: std::io::Error) -> CodexAdapterError {
    CodexAdapterError::Transport { source }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        if let Some(stderr_task) = self.stderr_task.take() {
            stderr_task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;

    use crate::{CodexAdapterError, SUPPORTED_CODEX_VERSION};

    use super::{ProcessTransport, SchemaFile, check_version, collect_missing_fields};

    #[cfg(windows)]
    use super::resolve_windows_executable;

    #[test]
    fn exact_version_is_required() {
        assert!(check_version(true, SUPPORTED_CODEX_VERSION.to_owned()).is_ok());
        assert!(matches!(
            check_version(true, "codex-cli 0.149.0".to_owned()),
            Err(CodexAdapterError::VersionMismatch { .. })
        ));
    }

    #[test]
    fn schema_contract_detects_missing_payload_fields() {
        let file = SchemaFile {
            path: "v2/TurnStartParams.json".to_owned(),
            required: vec!["threadId".to_owned(), "input".to_owned()],
        };
        let mut missing = Vec::new();
        collect_missing_fields(
            &file,
            &json!({ "required": ["threadId", "input"] }),
            &mut missing,
        );
        assert!(missing.is_empty());
        collect_missing_fields(&file, &json!({ "required": ["threadId"] }), &mut missing);
        assert_eq!(missing, ["v2/TurnStartParams.json required field `input`"]);
    }

    #[tokio::test]
    async fn missing_executable_is_a_discovery_error() {
        let result = ProcessTransport::start(Path::new(
            "definitely-missing-rah-codex-executable-for-test",
        ))
        .await;
        assert!(matches!(
            result,
            Err(CodexAdapterError::ExecutableDiscovery { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn resolves_direct_windows_executable_from_path() {
        let fixture = WindowsDiscoveryFixture::new("direct");
        let executable = fixture.root.join("codex.exe");
        std::fs::write(&executable, b"fixture").expect("write direct executable");

        let resolved =
            resolve_windows_executable(Path::new("codex"), Some(fixture.root.as_os_str()))
                .expect("resolve direct executable");

        assert_eq!(
            resolved,
            executable.canonicalize().expect("canonical executable")
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolves_global_npm_cmd_launcher_to_native_executable() {
        let fixture = WindowsDiscoveryFixture::new("global-npm");
        std::fs::write(fixture.root.join("codex.cmd"), b"@echo off").expect("write npm launcher");
        let executable = fixture
            .root
            .join("node_modules/@openai/codex/node_modules/@openai")
            .join(windows_platform_package())
            .join("vendor")
            .join(windows_target())
            .join("bin/codex.exe");
        std::fs::create_dir_all(executable.parent().expect("binary parent"))
            .expect("create npm package layout");
        std::fs::write(&executable, b"fixture").expect("write native executable");

        let resolved =
            resolve_windows_executable(Path::new("codex"), Some(fixture.root.as_os_str()))
                .expect("resolve npm launcher");

        assert_eq!(
            resolved,
            executable.canonicalize().expect("canonical executable")
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolves_local_npm_powershell_launcher_to_hoisted_executable() {
        let fixture = WindowsDiscoveryFixture::new("local-npm");
        let launcher_directory = fixture.root.join("node_modules/.bin");
        std::fs::create_dir_all(&launcher_directory).expect("create launcher directory");
        std::fs::write(launcher_directory.join("codex.ps1"), b"# fixture")
            .expect("write npm launcher");
        let executable = fixture
            .root
            .join("node_modules/@openai")
            .join(windows_platform_package())
            .join("vendor")
            .join(windows_target())
            .join("bin/codex.exe");
        std::fs::create_dir_all(executable.parent().expect("binary parent"))
            .expect("create npm package layout");
        std::fs::write(&executable, b"fixture").expect("write native executable");

        let resolved =
            resolve_windows_executable(Path::new("codex"), Some(launcher_directory.as_os_str()))
                .expect("resolve npm launcher");

        assert_eq!(
            resolved,
            executable.canonicalize().expect("canonical executable")
        );
    }

    #[cfg(windows)]
    fn windows_platform_package() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "codex-win32-x64",
            "aarch64" => "codex-win32-arm64",
            architecture => panic!("unsupported test architecture: {architecture}"),
        }
    }

    #[cfg(windows)]
    fn windows_target() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "x86_64-pc-windows-msvc",
            "aarch64" => "aarch64-pc-windows-msvc",
            architecture => panic!("unsupported test architecture: {architecture}"),
        }
    }

    #[cfg(windows)]
    struct WindowsDiscoveryFixture {
        root: std::path::PathBuf,
    }

    #[cfg(windows)]
    impl WindowsDiscoveryFixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-codex-discovery-{name}-{}-{}",
                std::process::id(),
                super::SCHEMA_PROBE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).expect("create discovery fixture");
            Self { root }
        }
    }

    #[cfg(windows)]
    impl Drop for WindowsDiscoveryFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).expect("remove discovery fixture");
        }
    }
}
