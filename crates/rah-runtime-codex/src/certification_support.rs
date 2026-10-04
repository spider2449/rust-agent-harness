//! Non-default exact-artifact certification support. Verification identifies an
//! artifact; it does not certify protocol, models, Desktop, or production admission.
//! No configuration/environment switch or Tool authority is supplied here.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use rah_protocol::RuntimeOperation;
use rah_runtime::{
    RuntimeFailure,
    experimental::{ConfiguredRuntimeFactory, RuntimeInstance},
};
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::{CodexAdapterError, CodexModelProvider, experimental::CodexFactory};

/// Required exact identity supplied by a certification consumer, never a model.
#[derive(Clone, Debug)]
pub struct ExactCodexCandidate {
    /// Absolute native executable path; no PATH lookup or launcher substitution.
    pub path: PathBuf,
    /// Exact numeric CLI version, e.g. `0.160.0`.
    pub expected_version: String,
    /// Exact lowercase hexadecimal whole-file SHA-256.
    pub expected_sha256: String,
}

/// Complete process-local identity evidence for one certification measurement.
/// This type is intentionally confined to the explicit certification feature.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactMeasurement {
    pub supplied_path: PathBuf,
    pub canonical_path: Option<PathBuf>,
    pub sha256: String,
    pub bytes_read: u64,
    pub file_length_before: u64,
    pub file_length_after: u64,
    pub creation_time: Option<u64>,
    pub last_write_time: Option<u64>,
    pub file_attributes: Option<u32>,
    pub volume_serial: Option<u32>,
    pub file_id: Option<u128>,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactMeasurementError {
    #[error("artifact measurement I/O failed")]
    Io(#[from] std::io::Error),
    #[error("artifact changed during measurement")]
    Unstable(Box<ArtifactMeasurement>),
    #[error("artifact read was incomplete")]
    Incomplete(Box<ArtifactMeasurement>),
}

#[cfg(windows)]
fn handle_identity(file: &std::fs::File) -> std::io::Result<(u64, u64, u64, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    let creation = (u64::from(info.ftCreationTime.dwHighDateTime) << 32)
        | u64::from(info.ftCreationTime.dwLowDateTime);
    let write = (u64::from(info.ftLastWriteTime.dwHighDateTime) << 32)
        | u64::from(info.ftLastWriteTime.dwLowDateTime);
    let id = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Ok((creation, write, id, info.dwFileAttributes))
}

#[cfg(not(windows))]
fn handle_identity(file: &std::fs::File) -> std::io::Result<(u64, u64, u64, u32)> {
    let metadata = file.metadata()?;
    Ok((
        metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64),
        metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as u64),
        0,
        0,
    ))
}

/// Hash one opened object and require a complete, stable read.
pub fn measure_artifact(path: &Path) -> Result<ArtifactMeasurement, ArtifactMeasurementError> {
    let supplied_path = path.to_path_buf();
    let canonical_path = std::fs::canonicalize(path).ok();
    let mut file = std::fs::File::open(path)?;
    let before = file.metadata()?;
    let file_length_before = before.len();
    let (creation_time, last_write_time, file_id, file_attributes) = handle_identity(&file)?;
    let volume_serial = {
        #[cfg(windows)]
        {
            Some(volume_serial_from_handle(&file)?)
        }
        #[cfg(not(windows))]
        {
            None
        }
    };
    let mut digest = Sha256::new();
    let mut bytes_read = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes_read += count as u64;
        digest.update(&buffer[..count]);
    }
    let after = file.metadata()?;
    let after_identity = handle_identity(&file)?;
    let record = ArtifactMeasurement {
        supplied_path,
        canonical_path,
        sha256: format!("{:x}", digest.finalize()),
        bytes_read,
        file_length_before,
        file_length_after: after.len(),
        creation_time: Some(creation_time),
        last_write_time: Some(last_write_time),
        file_attributes: Some(file_attributes),
        volume_serial,
        file_id: Some(u128::from(file_id)),
    };
    if record.bytes_read != record.file_length_before {
        return Err(ArtifactMeasurementError::Incomplete(Box::new(record)));
    }
    if record.file_length_before != record.file_length_after
        || after_identity != (creation_time, last_write_time, file_id, file_attributes)
        || volume_of(&file)? != volume_serial
        || std::fs::canonicalize(path).ok() != record.canonical_path
    {
        return Err(ArtifactMeasurementError::Unstable(Box::new(record)));
    }
    Ok(record)
}

#[cfg(windows)]
fn volume_serial_from_handle(file: &std::fs::File) -> std::io::Result<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION;
    use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle;
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.assume_init().dwVolumeSerialNumber })
}

fn volume_of(file: &std::fs::File) -> std::io::Result<Option<u32>> {
    #[cfg(windows)]
    {
        Ok(Some(volume_serial_from_handle(file)?))
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Ok(None)
    }
}

/// Snapshot errors retain evidence locally; Display contains no artifact data.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("certification snapshot I/O failed")]
    Io(#[from] std::io::Error),
    #[error("certification source changed during copy")]
    SourceChanged {
        measurement: Box<ArtifactMeasurement>,
        after_identity: (u64, u64, u64, u32),
    },
    #[error("certification source hash mismatch")]
    SourceHash {
        expected: String,
        measurement: Box<ArtifactMeasurement>,
    },
    #[error("certification snapshot measurement failed")]
    Measurement(#[from] ArtifactMeasurementError),
    #[error("certification snapshot identity mismatch")]
    SnapshotMismatch {
        source_measurement: Box<ArtifactMeasurement>,
        snapshot_measurement: Box<ArtifactMeasurement>,
    },
}

/// Frozen certification selector with retained single-handle copy evidence.
#[derive(Debug)]
pub struct CertificationSnapshot {
    candidate: ExactCodexCandidate,
    pub source: ArtifactMeasurement,
    pub snapshot: ArtifactMeasurement,
}
impl CertificationSnapshot {
    pub fn candidate(&self) -> ExactCodexCandidate {
        self.candidate.clone()
    }
}

/// Hash and copy one opened source object into a create-new destination.
/// Rejected outputs are abandoned and never returned as execution descriptors.
pub fn create_snapshot_at(
    source: &ExactCodexCandidate,
    destination: &Path,
) -> Result<CertificationSnapshot, SnapshotError> {
    let mut input = std::fs::File::open(&source.path)?;
    let length = input.metadata()?.len();
    let identity = handle_identity(&input)?;
    let volume = volume_of(&input)?;
    let canonical_path = std::fs::canonicalize(&source.path).ok();
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut digest = Sha256::new();
    let mut bytes_read = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        bytes_read += count as u64;
        output.write_all(&buffer[..count])?;
    }
    output.flush()?;
    output.sync_all()?;
    drop(output);
    let after_identity = handle_identity(&input)?;
    let record = ArtifactMeasurement {
        supplied_path: source.path.clone(),
        canonical_path,
        sha256: format!("{:x}", digest.finalize()),
        bytes_read,
        file_length_before: length,
        file_length_after: input.metadata()?.len(),
        creation_time: Some(identity.0),
        last_write_time: Some(identity.1),
        file_attributes: Some(identity.3),
        volume_serial: volume,
        file_id: Some(u128::from(identity.2)),
    };
    if bytes_read != length
        || record.file_length_after != length
        || identity != after_identity
        || volume_of(&input)? != volume
    {
        return Err(SnapshotError::SourceChanged {
            measurement: Box::new(record),
            after_identity,
        });
    }
    if record.sha256 != source.expected_sha256 {
        return Err(SnapshotError::SourceHash {
            expected: source.expected_sha256.clone(),
            measurement: Box::new(record),
        });
    }
    let snapshot = measure_artifact(destination)?;
    let repeated = measure_artifact(destination)?;
    let distinct = !cfg!(windows)
        || (record.volume_serial, record.file_id) != (snapshot.volume_serial, snapshot.file_id);
    if snapshot.sha256 != record.sha256
        || snapshot.bytes_read != length
        || snapshot.file_length_before != length
        || snapshot != repeated
        || !distinct
    {
        return Err(SnapshotError::SnapshotMismatch {
            source_measurement: Box::new(record),
            snapshot_measurement: Box::new(snapshot),
        });
    }
    let mut permissions = std::fs::metadata(destination)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(destination, permissions)?;
    let snapshot = measure_artifact(destination)?;
    Ok(CertificationSnapshot {
        candidate: ExactCodexCandidate {
            path: destination.to_path_buf(),
            expected_version: source.expected_version.clone(),
            expected_sha256: source.expected_sha256.clone(),
        },
        source: record,
        snapshot,
    })
}

pub fn create_isolated_snapshot(
    source: &ExactCodexCandidate,
) -> Result<CertificationSnapshot, SnapshotError> {
    static NEXT_SNAPSHOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    let counter = NEXT_SNAPSHOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "rah-codex-certification-snapshot-{}-{timestamp}-{counter}",
        std::process::id()
    ));
    std::fs::create_dir(&directory)?;
    create_snapshot_at(source, &directory.join("codex.exe"))
}

/// Exact process-local evidence available only in certification builds.
#[doc(hidden)]
#[derive(Debug, thiserror::Error)]
pub enum CertificationVerificationError {
    /// Supplied path does not identify an existing absolute artifact.
    #[error("certification artifact unavailable")]
    MissingArtifact { path: PathBuf },
    /// Measured bytes differ from the required identity.
    #[error("certification artifact hash mismatch")]
    HashMismatch { expected: String, actual: String },
    /// Reported version differs from the required identity.
    #[error("certification artifact version mismatch")]
    VersionMismatch { expected: String, actual: String },
}

impl From<CertificationVerificationError> for CodexAdapterError {
    fn from(source: CertificationVerificationError) -> Self {
        Self::CertificationVerification { source }
    }
}

fn rejection(message: &str) -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: message.into(),
    }
}

impl ExactCodexCandidate {
    fn validate(&self) -> Result<(), CodexAdapterError> {
        if !self.path.is_absolute() || !self.path.is_file() {
            return Err(CertificationVerificationError::MissingArtifact {
                path: self.path.clone(),
            }
            .into());
        }
        #[cfg(windows)]
        if self.path.extension() != Some(std::ffi::OsStr::new("exe")) {
            return Err(rejection(
                "certification requires a native executable, not a launcher",
            ));
        }
        let parts: Vec<_> = self.expected_version.split('.').collect();
        if parts.len() != 3
            || parts
                .iter()
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(rejection("certification requires an exact numeric version"));
        }
        if self.expected_sha256.len() != 64
            || !self
                .expected_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(rejection(
                "certification requires an exact lowercase SHA-256",
            ));
        }
        Ok(())
    }

    pub(crate) async fn verify(&self) -> Result<(), CodexAdapterError> {
        self.validate()?;
        let path = self.path.clone();
        let measurement = tokio::task::spawn_blocking(move || measure_artifact(&path))
            .await
            .map_err(|_| rejection("certification hash worker failed"))?
            .map_err(|error| CodexAdapterError::ExecutableDiscovery {
                path: self.path.clone(),
                source: match error {
                    ArtifactMeasurementError::Io(source) => source,
                    ArtifactMeasurementError::Incomplete(_)
                    | ArtifactMeasurementError::Unstable(_) => {
                        std::io::Error::other("unstable certification measurement")
                    }
                },
            })?;
        if measurement.sha256 != self.expected_sha256 {
            return Err(CertificationVerificationError::HashMismatch {
                expected: self.expected_sha256.clone(),
                actual: measurement.sha256,
            }
            .into());
        }
        let output = tokio::time::timeout(
            Duration::from_secs(10),
            Command::new(&self.path)
                .arg("--version")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| rejection("certification version probe deadline"))?
        .map_err(|source| CodexAdapterError::ExecutableDiscovery {
            path: self.path.clone(),
            source,
        })?;
        let actual = String::from_utf8_lossy(&output.stdout);
        if !output.status.success()
            || actual.trim() != format!("codex-cli {}", self.expected_version)
        {
            return Err(CertificationVerificationError::VersionMismatch {
                expected: self.expected_version.clone(),
                actual: actual.trim().to_owned(),
            }
            .into());
        }
        Ok(())
    }
}

/// Sealed real neutral factory for an artifact under certification. This name
/// deliberately makes no certification claim. Identity is rechecked on create.
pub struct VerifiedCertificationCandidate {
    factory: CodexFactory,
}

/// Verify exact identity before any schema/app-server startup, then construct the
/// real Codex factory with ordinary host-selected provider/workspace configuration.
/// Does not consult or mutate production admission. File checks are measurements,
/// not a running-image or race-free TOCTOU guarantee.
pub async fn verify_and_construct_candidate(
    candidate: ExactCodexCandidate,
    provider: CodexModelProvider,
    workspace: &std::path::Path,
) -> Result<VerifiedCertificationCandidate, RuntimeFailure> {
    candidate
        .verify()
        .await
        .map_err(|e| e.into_runtime_failure(RuntimeOperation::Connection))?;
    let mut factory =
        CodexFactory::new(candidate.path.clone(), provider).with_workspace(workspace)?;
    factory.candidate = Some(candidate);
    Ok(VerifiedCertificationCandidate { factory })
}

#[async_trait]
impl ConfiguredRuntimeFactory for VerifiedCertificationCandidate {
    fn validate(&self) -> Result<(), RuntimeFailure> {
        self.factory.validate()
    }
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
        self.factory.create().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_records_complete_stable_fixture_identity() {
        let root =
            std::env::temp_dir().join(format!("rah-artifact-measure-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("fixture.bin");
        let content = b"stable fixture bytes\n";
        std::fs::write(&path, content).unwrap();
        let first = measure_artifact(&path).unwrap();
        let second = measure_artifact(&path).unwrap();
        assert_eq!(first.bytes_read, content.len() as u64);
        assert_eq!(first.file_length_before, content.len() as u64);
        assert_eq!(first.file_length_after, content.len() as u64);
        assert_eq!(first.sha256, format!("{:x}", Sha256::digest(content)));
        assert!(first.file_id.is_some());
        assert_eq!(first, second);
        assert_eq!(first.supplied_path, path);
        assert_eq!(first.canonical_path, std::fs::canonicalize(&path).ok());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn snapshot_fixture_exact_bytes_and_rejections() {
        let root =
            std::env::temp_dir().join(format!("rah-snapshot-fixture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source_path = root.join("source.exe");
        let destination = root.join("snapshot.exe");
        let bytes = b"known snapshot fixture bytes";
        std::fs::write(&source_path, bytes).unwrap();
        let source = ExactCodexCandidate {
            path: source_path.clone(),
            expected_version: "0.160.0".into(),
            expected_sha256: format!("{:x}", Sha256::digest(bytes)),
        };
        let result = create_snapshot_at(&source, &destination).unwrap();
        assert_eq!(result.source.sha256, source.expected_sha256);
        assert_eq!(result.source.bytes_read, bytes.len() as u64);
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert_eq!(result.snapshot.sha256, result.source.sha256);
        assert_eq!(measure_artifact(&destination).unwrap(), result.snapshot);
        #[cfg(windows)]
        assert_ne!(result.source.file_id, result.snapshot.file_id);
        assert_eq!(result.candidate().path, destination);
        assert_ne!(result.candidate().path, source_path);
        assert!(
            matches!(create_snapshot_at(&source, &destination), Err(SnapshotError::Io(e)) if e.kind() == std::io::ErrorKind::AlreadyExists)
        );
        let mut wrong = source.clone();
        wrong.expected_sha256 = "0".repeat(64);
        let rejected = root.join("rejected.exe");
        let error = create_snapshot_at(&wrong, &rejected).unwrap_err();
        assert!(
            matches!(&error, SnapshotError::SourceHash { measurement, .. } if measurement.sha256 == source.expected_sha256)
        );
        for value in [
            source_path.to_str().unwrap(),
            destination.to_str().unwrap(),
            &source.expected_sha256,
            &wrong.expected_sha256,
            &format!("{:032x}", result.source.file_id.unwrap()),
        ] {
            assert!(!error.to_string().contains(value));
        }
        // Preserve fixture files; snapshot disposal is outside this task.
    }

    fn cause(error: &RuntimeFailure) -> &CertificationVerificationError {
        use std::error::Error;
        error
            .source()
            .unwrap()
            .downcast_ref::<CodexAdapterError>()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<CertificationVerificationError>()
            .unwrap()
    }

    fn assert_redacted(error: &RuntimeFailure, values: &[&str]) {
        let public = format!(
            "{} {}",
            error,
            serde_json::to_string(error.diagnostic()).unwrap()
        );
        for value in values {
            assert!(!public.contains(value));
        }
    }

    #[tokio::test]
    async fn exact_factory_version_and_production_separation() {
        let root = std::env::temp_dir().join(format!("rah-certification-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("probe.rs");
        let path = root.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        // Identity-probe fixture only; never substitutes for the real smoke adapter.
        std::fs::write(&source, r#"fn main() {
            let args: Vec<_> = std::env::args().skip(1).collect();
            let log = std::env::current_exe().unwrap().with_extension("log");
            use std::io::Write;
            writeln!(std::fs::OpenOptions::new().create(true).append(true).open(log).unwrap(), "{:?}", args).unwrap();
            assert_eq!(args, ["--version"]);
            println!("codex-cli 0.160.0");
        }"#).unwrap();
        assert!(
            std::process::Command::new("rustc")
                .arg(&source)
                .arg("-o")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let hash = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
        let candidate = ExactCodexCandidate {
            path: path.clone(),
            expected_version: "0.160.0".into(),
            expected_sha256: hash,
        };
        let admission = crate::CURRENT_CERTIFIED_CODEX_VERSIONS.to_vec();
        let factory =
            verify_and_construct_candidate(candidate.clone(), CodexModelProvider::OpenAi, &root)
                .await
                .unwrap();
        factory.validate().unwrap();
        assert_eq!(factory.factory.executable_for_certification_test(), path);
        let mut wrong = candidate.clone();
        wrong.expected_version = "0.157.1".into();
        let error = wrong
            .verify()
            .await
            .unwrap_err()
            .into_runtime_failure(RuntimeOperation::Connection);
        assert_redacted(&error, &["0.157.1", "codex-cli 0.160.0"]);
        assert!(
            matches!(cause(&error), CertificationVerificationError::VersionMismatch { expected, actual } if expected == "0.157.1" && actual == "codex-cli 0.160.0")
        );
        let production = CodexFactory::new(path.clone(), CodexModelProvider::OpenAi);
        let result = production.create().await;
        use std::error::Error;
        assert!(
            matches!(result.err().unwrap().source().and_then(|s| s.downcast_ref::<CodexAdapterError>()), Some(CodexAdapterError::VersionMismatch { actual, .. }) if actual == "codex-cli 0.160.0")
        );
        assert_eq!(
            std::fs::read_to_string(path.with_extension("log")).unwrap(),
            "[\"--version\"]\n".repeat(3)
        );
        assert_eq!(crate::CURRENT_CERTIFIED_CODEX_VERSIONS, admission);
        // Changing bytes after construction fails before any schema/runtime startup.
        std::fs::write(&path, b"changed artifact").unwrap();
        let error = factory.create().await.err().unwrap();
        let actual_hash = format!("{:x}", Sha256::digest(b"changed artifact"));
        assert_redacted(&error, &[&candidate.expected_sha256, &actual_hash]);
        assert!(
            matches!(cause(&error), CertificationVerificationError::HashMismatch { expected, actual } if expected == &candidate.expected_sha256 && actual == &actual_hash)
        );
        assert_eq!(
            std::fs::read_to_string(path.with_extension("log")).unwrap(),
            "[\"--version\"]\n".repeat(3)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_descriptor_rejects_missing_identity_and_path_substitution() {
        let mut candidate = ExactCodexCandidate {
            path: std::env::current_exe().unwrap(),
            expected_version: "0.160.0".into(),
            expected_sha256: "0".repeat(64),
        };
        assert!(candidate.validate().is_ok());
        candidate.path = PathBuf::from("codex");
        assert!(candidate.validate().is_err());
        candidate.path = std::env::temp_dir().join("absent-certification-codex.exe");
        let error = candidate
            .validate()
            .unwrap_err()
            .into_runtime_failure(RuntimeOperation::Connection);
        assert_redacted(&error, &[candidate.path.to_str().unwrap()]);
        assert!(
            matches!(cause(&error), CertificationVerificationError::MissingArtifact { path } if path == &candidate.path)
        );
        candidate.path = std::env::current_exe().unwrap();
        candidate.expected_version.clear();
        assert!(candidate.validate().is_err());
        candidate.expected_version = "0.160.0".into();
        candidate.expected_sha256.clear();
        assert!(candidate.validate().is_err());
    }

    #[tokio::test]
    async fn wrong_hash_cannot_probe_or_start_runtime() {
        let candidate = ExactCodexCandidate {
            path: std::env::current_exe().unwrap(),
            expected_version: "0.160.0".into(),
            expected_sha256: "0".repeat(64),
        };
        let actual_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&candidate.path).unwrap())
        );
        let error = candidate
            .verify()
            .await
            .unwrap_err()
            .into_runtime_failure(RuntimeOperation::Connection);
        assert_redacted(&error, &[&candidate.expected_sha256, &actual_hash]);
        assert!(
            matches!(cause(&error), CertificationVerificationError::HashMismatch { expected, actual } if expected == &candidate.expected_sha256 && actual == &actual_hash)
        );
        assert_eq!(
            crate::CURRENT_CERTIFIED_CODEX_VERSIONS,
            &["codex-cli 0.157.1"]
        );
        assert!(!crate::is_current_certified_codex_version(
            "codex-cli 0.160.0"
        ));
    }
}
