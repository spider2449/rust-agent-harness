//! Windows-private certified Codex baseline discovery.
//!
//! This module deliberately owns only host configuration validation and executable
//! selection. It neither starts Codex nor grants model or tool authority.

use rah_runtime_codex::SUPPORTED_CODEX_VERSION;
use serde_json::Value;
use std::{
    ffi::{OsStr, OsString, c_void},
    fs,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

const BASELINE_HOME_OVERRIDE: &str = "CODEX_BASELINE_HOME";
const BASELINE_DIRECTORY: &str = "codex-baselines";
const MANIFEST_NAME: &str = "manifest.json";
const BINARY_NAME: &str = "codex.exe";
const CODE_MODE_HOST_NAME: &str = "codex-code-mode-host.exe";
const REPORTED_VERSION_PREFIX: &str = "codex-cli ";
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CodexExecutableSource {
    Override,
    CertifiedBaseline,
    Path,
}

#[derive(Debug)]
pub(super) struct CodexExecutableSelection {
    pub(super) executable: OsString,
    pub(super) source: CodexExecutableSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BaselineError {
    UnsupportedHost,
    Invalid,
}

/// Resolves the explicit host override, then the one certified baseline, then PATH.
pub(super) fn resolve() -> Result<CodexExecutableSelection, BaselineError> {
    let override_executable =
        std::env::var_os("RAH_CODEX_EXECUTABLE").filter(|value| !value.is_empty());
    resolve_with(override_executable, baseline_store_root(), native_version)
}

fn baseline_store_root() -> PathBuf {
    if let Some(root) = std::env::var_os(BASELINE_HOME_OVERRIDE).filter(|value| !value.is_empty()) {
        return PathBuf::from(root);
    }
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|root| root.join(BASELINE_DIRECTORY))
        .unwrap_or_default()
}

fn resolve_with<F>(
    override_executable: Option<OsString>,
    store_root: PathBuf,
    version_reader: F,
) -> Result<CodexExecutableSelection, BaselineError>
where
    F: Fn(&Path) -> Result<String, BaselineError>,
{
    if let Some(executable) = override_executable {
        return Ok(CodexExecutableSelection {
            executable,
            source: CodexExecutableSource::Override,
        });
    }

    let baseline_version = supported_baseline_version()?;
    let directory = store_root.join(baseline_version);
    if !directory.exists() {
        return Ok(CodexExecutableSelection {
            executable: OsString::from("codex"),
            source: CodexExecutableSource::Path,
        });
    }
    if !cfg!(target_arch = "x86_64") {
        return Err(BaselineError::UnsupportedHost);
    }

    let executable = verify_baseline(&directory, version_reader)?;
    Ok(CodexExecutableSelection {
        executable: executable.into_os_string(),
        source: CodexExecutableSource::CertifiedBaseline,
    })
}

fn verify_baseline<F>(directory: &Path, version_reader: F) -> Result<PathBuf, BaselineError>
where
    F: Fn(&Path) -> Result<String, BaselineError>,
{
    let directory_metadata = fs::symlink_metadata(directory).map_err(|_| BaselineError::Invalid)?;
    if !directory_metadata.is_dir()
        || directory_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(BaselineError::Invalid);
    }
    let manifest_path = directory.join(MANIFEST_NAME);
    let manifest_metadata =
        fs::symlink_metadata(&manifest_path).map_err(|_| BaselineError::Invalid)?;
    if !manifest_metadata.is_file()
        || manifest_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(BaselineError::Invalid);
    }
    let baseline_version = supported_baseline_version()?;
    let manifest = parse_manifest(&manifest_path)?;
    let binary = required_string(&manifest, "binary")?;
    let code_mode_host = required_string(&manifest, "code_mode_host")?;
    if binary != BINARY_NAME
        || code_mode_host != CODE_MODE_HOST_NAME
        || manifest.get("manifest_version").and_then(Value::as_u64) != Some(2)
        || required_string(&manifest, "version")? != baseline_version
        || required_string(&manifest, "reported_version")? != SUPPORTED_CODEX_VERSION
        || required_string(&manifest, "platform")? != "windows-x86_64"
        || required_string(&manifest, "architecture")? != "x86_64"
    {
        return Err(BaselineError::Invalid);
    }
    let expected_hash = required_string(&manifest, "sha256")?;
    let expected_code_mode_host_hash = required_string(&manifest, "code_mode_host_sha256")?;
    if !is_lowercase_sha256(expected_hash) || !is_lowercase_sha256(expected_code_mode_host_hash) {
        return Err(BaselineError::Invalid);
    }
    let executable = directory.join(binary);
    let pre_canonical_metadata =
        fs::symlink_metadata(&executable).map_err(|_| BaselineError::Invalid)?;
    if !pre_canonical_metadata.is_file()
        || pre_canonical_metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(BaselineError::Invalid);
    }
    let executable = fs::canonicalize(executable).map_err(|_| BaselineError::Invalid)?;
    if !executable.is_absolute() || executable.extension() != Some(OsStr::new("exe")) {
        return Err(BaselineError::Invalid);
    }
    let metadata = fs::symlink_metadata(&executable).map_err(|_| BaselineError::Invalid)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(BaselineError::Invalid);
    }
    let bytes = fs::read(&executable).map_err(|_| BaselineError::Invalid)?;
    if bytes.get(..2) != Some(b"MZ") || sha256_hex(&bytes)? != expected_hash {
        return Err(BaselineError::Invalid);
    }
    if version_reader(&executable)? != SUPPORTED_CODEX_VERSION {
        return Err(BaselineError::Invalid);
    }
    verify_sibling_executable(directory, code_mode_host, expected_code_mode_host_hash)?;
    Ok(executable)
}

fn verify_sibling_executable(
    directory: &Path,
    name: &str,
    expected_hash: &str,
) -> Result<PathBuf, BaselineError> {
    let path = directory.join(name);
    let metadata = fs::symlink_metadata(&path).map_err(|_| BaselineError::Invalid)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(BaselineError::Invalid);
    }
    let canonical = fs::canonicalize(path).map_err(|_| BaselineError::Invalid)?;
    let canonical_parent = canonical.parent().ok_or(BaselineError::Invalid)?;
    let canonical_directory = fs::canonicalize(directory).map_err(|_| BaselineError::Invalid)?;
    if canonical_parent != canonical_directory || canonical.file_name() != Some(OsStr::new(name)) {
        return Err(BaselineError::Invalid);
    }
    let metadata = fs::symlink_metadata(&canonical).map_err(|_| BaselineError::Invalid)?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(BaselineError::Invalid);
    }
    let bytes = fs::read(&canonical).map_err(|_| BaselineError::Invalid)?;
    if bytes.get(..2) != Some(b"MZ") || sha256_hex(&bytes)? != expected_hash {
        return Err(BaselineError::Invalid);
    }
    Ok(canonical)
}

/// Converts the adapter's fixed reported-version pin into the one permitted
/// baseline-store directory/manifest version. Any unfamiliar pin fails closed.
fn supported_baseline_version() -> Result<&'static str, BaselineError> {
    let version = SUPPORTED_CODEX_VERSION
        .strip_prefix(REPORTED_VERSION_PREFIX)
        .ok_or(BaselineError::Invalid)?;
    let mut components = version.split('.');
    if (0..3).all(|_| {
        components
            .next()
            .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    }) && components.next().is_none()
    {
        Ok(version)
    } else {
        Err(BaselineError::Invalid)
    }
}

fn parse_manifest(path: &Path) -> Result<serde_json::Map<String, Value>, BaselineError> {
    let value: Value = serde_json::from_slice(&fs::read(path).map_err(|_| BaselineError::Invalid)?)
        .map_err(|_| BaselineError::Invalid)?;
    let object = value.as_object().cloned().ok_or(BaselineError::Invalid)?;
    const ALLOWED: [&str; 12] = [
        "manifest_version",
        "version",
        "reported_version",
        "sha256",
        "platform",
        "architecture",
        "binary",
        "code_mode_host",
        "code_mode_host_sha256",
        "source",
        "source_package",
        "archived_at_utc",
    ];
    const REQUIRED: [&str; 11] = [
        "version",
        "reported_version",
        "sha256",
        "platform",
        "architecture",
        "binary",
        "code_mode_host",
        "code_mode_host_sha256",
        "source",
        "source_package",
        "archived_at_utc",
    ];
    if object.keys().any(|key| !ALLOWED.contains(&key.as_str()))
        || REQUIRED
            .iter()
            .any(|key| required_string(&object, key).is_err())
    {
        return Err(BaselineError::Invalid);
    }
    Ok(object)
}

#[cfg(test)]
mod tests {
    use super::{
        BaselineError, CodexExecutableSource, resolve_with, sha256_hex, supported_baseline_version,
    };
    use rah_runtime_codex::SUPPORTED_CODEX_VERSION;
    use serde_json::{Map, Value, json};
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    type ManifestMutation = Box<dyn Fn(&mut Map<String, Value>)>;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "rah-codex-baseline-{timestamp}-{}",
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join(supported_baseline_version().unwrap())).unwrap();
            let fixture = Self(root);
            fixture.write_valid();
            fixture
        }

        fn directory(&self) -> PathBuf {
            self.0.join(supported_baseline_version().unwrap())
        }
        fn binary(&self) -> PathBuf {
            self.directory().join("codex.exe")
        }
        fn code_mode_host(&self) -> PathBuf {
            self.directory().join("codex-code-mode-host.exe")
        }
        fn manifest(&self) -> PathBuf {
            self.directory().join("manifest.json")
        }

        fn write_valid(&self) {
            fs::write(self.binary(), b"MZ certified fixture").unwrap();
            fs::write(self.code_mode_host(), b"MZ code mode host fixture").unwrap();
            let hash = sha256_hex(&fs::read(self.binary()).unwrap()).unwrap();
            let code_mode_host_hash =
                sha256_hex(&fs::read(self.code_mode_host()).unwrap()).unwrap();
            fs::write(
                self.manifest(),
                serde_json::to_vec(&json!({
                    "manifest_version": 2,
                    "version": supported_baseline_version().unwrap(),
                    "reported_version": SUPPORTED_CODEX_VERSION,
                    "sha256": hash,
                    "platform": "windows-x86_64",
                    "architecture": "x86_64",
                    "binary": "codex.exe",
                    "code_mode_host": "codex-code-mode-host.exe",
                    "code_mode_host_sha256": code_mode_host_hash,
                    "source": "fixture",
                    "source_package": "fixture",
                    "archived_at_utc": "2026-08-27T00:00:00Z"
                }))
                .unwrap(),
            )
            .unwrap();
        }

        fn manifest_object(&self) -> Map<String, Value> {
            serde_json::from_slice(&fs::read(self.manifest()).unwrap()).unwrap()
        }

        fn write_manifest(&self, object: Map<String, Value>) {
            fs::write(self.manifest(), serde_json::to_vec(&object).unwrap()).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn exact_version(_: &Path) -> Result<String, BaselineError> {
        Ok(SUPPORTED_CODEX_VERSION.to_owned())
    }
    fn wrong_version(_: &Path) -> Result<String, BaselineError> {
        Ok("codex-cli 9.9.9".to_owned())
    }
    fn resolve(fixture: &Fixture) -> Result<super::CodexExecutableSelection, BaselineError> {
        resolve_with(None, fixture.0.clone(), exact_version)
    }

    #[test]
    fn override_wins_over_baseline_and_path_without_execution() {
        let fixture = Fixture::new();
        let selected = resolve_with(
            Some(OsString::from(r"C:\operator\codex.exe")),
            fixture.0.clone(),
            |_| Err(BaselineError::Invalid),
        )
        .unwrap();
        assert_eq!(selected.source, CodexExecutableSource::Override);
        assert_eq!(
            selected.executable,
            OsString::from(r"C:\operator\codex.exe")
        );
    }

    #[test]
    fn real_adapter_pin_maps_to_the_semantic_baseline_layout_only() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.0.join("9.9.9")).unwrap();
        let selected = resolve(&fixture).unwrap();
        assert_eq!(selected.source, CodexExecutableSource::CertifiedBaseline);
        assert_eq!(
            selected.executable,
            fs::canonicalize(fixture.0.join("0.149.0").join("codex.exe"))
                .unwrap()
                .into_os_string()
        );
        assert_ne!(selected.executable, OsString::from("codex"));
        assert!(!fixture.0.join(SUPPORTED_CODEX_VERSION).exists());

        fs::rename(fixture.directory(), fixture.0.join(SUPPORTED_CODEX_VERSION)).unwrap();
        assert_eq!(
            resolve(&fixture).unwrap().source,
            CodexExecutableSource::Path,
            "a directory named after the reported adapter version is not a baseline layout"
        );
    }

    #[test]
    fn absent_baseline_uses_path_but_invalid_present_baseline_does_not() {
        let fixture = Fixture::new();
        fs::remove_dir_all(fixture.directory()).unwrap();
        assert_eq!(
            resolve_with(None, fixture.0.clone(), exact_version)
                .unwrap()
                .source,
            CodexExecutableSource::Path
        );
        fs::create_dir_all(fixture.directory()).unwrap();
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
    }

    #[test]
    fn manifest_contract_failures_are_rejected() {
        let cases: &[(&str, ManifestMutation)] = &[
            (
                "unknown",
                Box::new(|m| {
                    m.insert("unexpected".into(), json!(true));
                }),
            ),
            (
                "missing",
                Box::new(|m| {
                    m.remove("source_package");
                }),
            ),
            (
                "manifest_version",
                Box::new(|m| {
                    m.insert("manifest_version".into(), json!(1));
                }),
            ),
            (
                "version",
                Box::new(|m| {
                    m.insert("version".into(), json!("9.9.9"));
                }),
            ),
            (
                "reported_version",
                Box::new(|m| {
                    m.insert("reported_version".into(), json!("codex-cli 9.9.9"));
                }),
            ),
            (
                "platform",
                Box::new(|m| {
                    m.insert("platform".into(), json!("linux-x86_64"));
                }),
            ),
            (
                "architecture",
                Box::new(|m| {
                    m.insert("architecture".into(), json!("arm64"));
                }),
            ),
            (
                "binary",
                Box::new(|m| {
                    m.insert("binary".into(), json!("other.exe"));
                }),
            ),
            (
                "code_mode_host",
                Box::new(|m| {
                    m.insert("code_mode_host".into(), json!("other.exe"));
                }),
            ),
            (
                "code_mode_host_sha",
                Box::new(|m| {
                    m.insert("code_mode_host_sha256".into(), json!("A".repeat(64)));
                }),
            ),
            (
                "sha",
                Box::new(|m| {
                    m.insert("sha256".into(), json!("A".repeat(64)));
                }),
            ),
        ];
        for (name, change) in cases {
            let fixture = Fixture::new();
            let mut manifest = fixture.manifest_object();
            change(&mut manifest);
            fixture.write_manifest(manifest);
            assert!(
                matches!(resolve(&fixture), Err(BaselineError::Invalid)),
                "{name}"
            );
        }
    }

    #[test]
    fn malformed_json_hash_mismatch_non_pe_and_version_mismatch_are_rejected() {
        let fixture = Fixture::new();
        fs::write(fixture.manifest(), b"{").unwrap();
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
        fixture.write_valid();
        fs::write(fixture.binary(), b"MZ modified").unwrap();
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
        fixture.write_valid();
        fs::write(fixture.binary(), b"not a PE").unwrap();
        let hash = sha256_hex(&fs::read(fixture.binary()).unwrap()).unwrap();
        let mut manifest = fixture.manifest_object();
        manifest.insert("sha256".into(), json!(hash));
        fixture.write_manifest(manifest);
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
        fixture.write_valid();
        assert!(matches!(
            resolve_with(None, fixture.0.clone(), wrong_version),
            Err(BaselineError::Invalid)
        ));
    }

    #[test]
    fn missing_or_modified_code_mode_host_is_rejected() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.code_mode_host()).unwrap();
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));

        fixture.write_valid();
        fs::write(fixture.code_mode_host(), b"MZ modified host").unwrap();
        assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
    }

    #[test]
    fn code_mode_host_reparse_point_is_rejected_when_supported() {
        use std::os::windows::fs::symlink_file;

        let fixture = Fixture::new();
        let target = fixture.directory().join("host-target.exe");
        fs::write(&target, b"MZ code mode host fixture").unwrap();
        fs::remove_file(fixture.code_mode_host()).unwrap();
        if symlink_file(&target, fixture.code_mode_host()).is_ok() {
            assert!(matches!(resolve(&fixture), Err(BaselineError::Invalid)));
        }
    }

    #[test]
    fn selected_executable_is_private_to_the_resolver() {
        let fixture = Fixture::new();
        let selected = resolve(&fixture).unwrap();
        assert!(Path::new(&selected.executable).is_absolute());
        assert_eq!(selected.source, CodexExecutableSource::CertifiedBaseline);
    }
}

fn required_string<'a>(
    manifest: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, BaselineError> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(BaselineError::Invalid)
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

fn native_version(path: &Path) -> Result<String, BaselineError> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|_| BaselineError::Invalid)?;
    if !output.status.success() {
        return Err(BaselineError::Invalid);
    }
    String::from_utf8(output.stdout)
        .map_err(|_| BaselineError::Invalid)
        .map(|value| value.trim().to_owned())
}

// CNG is the Windows OS cryptographic provider. Keeping this FFI private avoids
// widening a RAH API or adding a Cargo dependency for one Desktop-only check.
type BCryptAlgHandle = *mut c_void;
type BCryptHashHandle = *mut c_void;
const BCRYPT_OBJECT_LENGTH: &[u16] = &[79, 98, 106, 101, 99, 116, 76, 101, 110, 103, 116, 104, 0];
const BCRYPT_SHA256_ALGORITHM: &[u16] = &[83, 72, 65, 50, 53, 54, 0];

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptOpenAlgorithmProvider(
        handle: *mut BCryptAlgHandle,
        algorithm: *const u16,
        implementation: *const u16,
        flags: u32,
    ) -> i32;
    fn BCryptGetProperty(
        handle: BCryptAlgHandle,
        property: *const u16,
        output: *mut u8,
        output_size: u32,
        result_size: *mut u32,
        flags: u32,
    ) -> i32;
    fn BCryptCreateHash(
        algorithm: BCryptAlgHandle,
        hash: *mut BCryptHashHandle,
        object: *mut u8,
        object_size: u32,
        secret: *const u8,
        secret_size: u32,
        flags: u32,
    ) -> i32;
    fn BCryptHashData(hash: BCryptHashHandle, input: *const u8, input_size: u32, flags: u32)
    -> i32;
    fn BCryptFinishHash(
        hash: BCryptHashHandle,
        output: *mut u8,
        output_size: u32,
        flags: u32,
    ) -> i32;
    fn BCryptDestroyHash(hash: BCryptHashHandle) -> i32;
    fn BCryptCloseAlgorithmProvider(handle: BCryptAlgHandle, flags: u32) -> i32;
}

fn sha256_hex(bytes: &[u8]) -> Result<String, BaselineError> {
    let input_size = u32::try_from(bytes.len()).map_err(|_| BaselineError::Invalid)?;
    let mut algorithm = std::ptr::null_mut();
    // SAFETY: constants are nul-terminated UTF-16 and output pointers are valid.
    if unsafe {
        BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_SHA256_ALGORITHM.as_ptr(),
            std::ptr::null(),
            0,
        )
    } < 0
    {
        return Err(BaselineError::Invalid);
    }
    let mut object_size = 0_u32;
    let mut returned = 0_u32;
    // SAFETY: algorithm is open and the mutable output has the documented size.
    let property = unsafe {
        BCryptGetProperty(
            algorithm,
            BCRYPT_OBJECT_LENGTH.as_ptr(),
            (&mut object_size as *mut u32).cast(),
            4,
            &mut returned,
            0,
        )
    };
    if property < 0 || returned != 4 {
        unsafe { BCryptCloseAlgorithmProvider(algorithm, 0) };
        return Err(BaselineError::Invalid);
    }
    let mut object = vec![0_u8; object_size as usize];
    let mut hash = std::ptr::null_mut();
    // SAFETY: all buffers remain live through hash finalization.
    let created = unsafe {
        BCryptCreateHash(
            algorithm,
            &mut hash,
            object.as_mut_ptr(),
            object_size,
            std::ptr::null(),
            0,
            0,
        )
    };
    if created < 0 {
        unsafe { BCryptCloseAlgorithmProvider(algorithm, 0) };
        return Err(BaselineError::Invalid);
    }
    // SAFETY: hash is valid and bytes is valid for input_size bytes.
    let hashed = unsafe { BCryptHashData(hash, bytes.as_ptr(), input_size, 0) };
    let mut digest = [0_u8; 32];
    // SAFETY: hash is valid and digest has the SHA-256 output size.
    let finished = if hashed >= 0 {
        unsafe { BCryptFinishHash(hash, digest.as_mut_ptr(), digest.len() as u32, 0) }
    } else {
        hashed
    };
    // SAFETY: both handles were created by CNG and are no longer used after this point.
    unsafe {
        BCryptDestroyHash(hash);
        BCryptCloseAlgorithmProvider(algorithm, 0);
    }
    if finished < 0 {
        return Err(BaselineError::Invalid);
    }
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}
